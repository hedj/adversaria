//! Each session's start entry (docs/design.md, The journal format): the
//! first entry a session writes follows one, which records the build that
//! wrote the session, so which build wrote each entry, and where each
//! session began, are in the record. The header declares the version.

mod support;

use format::record::EntryKind;
use memex::app::{App, Key};
use support::{Room, replayed};

/// The start entries of a journal, by their bodies.
fn starts(app: &App) -> Vec<serde_json::Value> {
    let entries = app.journal().entries().iter();
    let start = entries
        .filter(|e| e.kind() == EntryKind::View && e.body().is_some_and(|b| b["op"] == "start"));
    start.filter_map(|e| e.body().cloned()).collect()
}

#[test]
fn opening_a_journal_writes_nothing() {
    // Catches a start entry written by a look alone, so that replaying a
    // journal, or opening it to read, changed it.
    let r = Room::new("session-open");
    assert_eq!(r.entry_count(), 1, "the journal's first entry alone");
    assert!(starts(&r.app).is_empty());
}

#[test]
fn the_first_entry_a_session_writes_follows_its_start() {
    let mut r = Room::new("session-start");
    r.items(&["Alder"]);
    let entries = r.app.journal().entries();
    let first = entries[1].body().unwrap();
    assert_eq!(
        (entries[1].kind(), &first["op"]),
        (EntryKind::View, &serde_json::json!("start"))
    );
    assert_eq!(first.get("schema"), None, "the header declares the version");
    let build = first["build"].as_str().unwrap();
    let hex = |s: &str| s.len() == 12 && s.chars().all(|c| c.is_ascii_hexdigit());
    assert!(
        build == "unknown" || hex(build.trim_end_matches("-dirty")),
        "{build}"
    );
    assert_eq!(
        entries[2].parent_seq(),
        Some(2),
        "the item's entry follows it"
    );
    assert_eq!(starts(&r.app).len(), 1, "one for the session");
}

#[test]
fn each_session_that_writes_has_its_own_start() {
    let mut r = Room::new("session-again");
    r.items(&["Alder"]);
    r.app.flush();
    let mut again = replayed(&r.dir);
    assert_eq!(starts(&again).len(), 1, "opened, it has written nothing");
    again.run_command("new");
    assert_eq!(starts(&again).len(), 2);
}

#[test]
fn an_undo_as_a_session_s_first_entry_keeps_the_record_whole() {
    // Catches the start entry put where the undo's own parent is meant to
    // be, so that the undo undid the start entry, or the journal failed
    // its check.
    let mut r = Room::new("session-undo");
    r.items(&["Alder", "Birch"]);
    r.app.flush();
    let mut again = replayed(&r.dir);
    again.run_command("undo");
    let o = &again.notebook.outline;
    let texts: Vec<String> = o.in_order().iter().map(|i| o.text(*i)).collect();
    assert_eq!(texts, ["Alder", ""], "Birch's text undone, not the start");
    assert_eq!(starts(&again).len(), 2);
    let text = std::fs::read_to_string(again.journal().dir().join("journal.jsonl"));
    assert!(text.is_ok_and(|t| format::chain::check_journal(&t).is_ok()));
}

/// Each entry's kind and op, and whether it is marked as the memex's own.
fn marks(app: &App) -> Vec<(String, String, bool)> {
    let entries = app.journal().entries().iter();
    let mark = |e: &format::record::Entry| {
        let b = e.body().unwrap();
        let op = b["op"].as_str().unwrap_or("").to_string();
        (e.kind().to_string(), op, b["by"] == "memex")
    };
    entries.skip(1).map(mark).collect()
}

/// Whether every entry of a kind and op is marked as the memex's, if
/// there is one; None if there is none.
fn all_marked(all: &[(String, String, bool)], kind: &str, op: &str) -> Option<bool> {
    let these = all.iter().filter(|(k, o, _)| k == kind && o == op);
    let marks: Vec<bool> = these.map(|e| e.2).collect();
    let first = *marks.first()?;
    Some(first && marks.iter().all(|m| *m == first))
}

#[test]
fn what_the_memex_writes_on_its_own_is_marked_and_the_user_s_is_not() {
    // Catches a reading, or an observation, counted as the user's work,
    // or the user's typing marked as the memex's.
    let mut r = Room::new("session-marks");
    r.items(&[
        "The date of Buran-flight is 1988.",
        "The date of Buran-flight is 1989.",
        "Nothing has more than 1 date.",
    ]);
    r.app.idle();
    r.app.evaluate().unwrap();
    let all = marks(&r.app);
    let own = ["start", "facts", "constraint", "read", "observed"];
    let kinds = ["view", "edit", "edit", "edit", "view"];
    for (kind, op) in kinds.into_iter().zip(own) {
        assert_eq!(
            all_marked(&all, kind, op),
            Some(true),
            "{kind} {op}: {all:#?}"
        );
    }
    for (kind, op) in [("edit", "create"), ("edit", "text"), ("view", "written")] {
        assert_eq!(
            all_marked(&all, kind, op),
            Some(false),
            "{kind} {op}: {all:#?}"
        );
    }
}

#[test]
fn a_name_s_relinking_is_the_memex_s_and_the_name_the_user_s() {
    let mut r = Room::new("session-relink");
    r.items(&["Alder", "See [[tree]]."]);
    r.app.idle();
    r.command("go 1");
    r.command("name tree");
    let all = marks(&r.app);
    let found = |kind: &str, op: &str| all.iter().find(|(k, o, _)| k == kind && o == op).cloned();
    assert_eq!(found("name", "").map(|f| f.2), Some(false), "{all:#?}");
    assert_eq!(found("edit", "relink").map(|f| f.2), Some(true), "{all:#?}");
    assert_eq!(
        found("link", "").map(|f| f.2),
        Some(true),
        "the link the name made: {all:#?}"
    );
}

/// The ops of the view entries written since entry `from`, in order.
fn views_since(r: &Room, from: usize) -> Vec<String> {
    let after = &r.app.journal().entries()[from..];
    let views = after.iter().filter(|e| e.kind() == EntryKind::View);
    views
        .map(|e| e.body().unwrap()["op"].as_str().unwrap_or("").to_string())
        .collect()
}

#[test]
fn where_writing_leaves_the_cursor_is_recorded_apart_from_a_move() {
    // Catches the view entry after each burst of typing recorded as a
    // move of the user's, which the measures count as navigating.
    let mut r = Room::new("written");
    r.items(&["Alder", "Birch"]);
    let from = r.entry_count();
    r.typ(" bark");
    r.app.flush();
    assert_eq!(views_since(&r, from), ["written"], "typing alone");
    let from = r.entry_count();
    r.tap(&[Key::Up]);
    r.typ("s");
    r.app.flush();
    assert_eq!(views_since(&r, from), ["move"], "a move, then typing");
    let from = r.entry_count();
    r.typ("!");
    r.tap(&[Key::Down]);
    r.app.flush();
    assert_eq!(views_since(&r, from), ["move"], "typing, then a move");
    let from = r.entry_count();
    r.tap(&[Key::Enter]);
    r.tap(&[Key::Backspace]);
    r.app.flush();
    let made_then_erased = ["written", "written"];
    assert_eq!(
        views_since(&r, from),
        made_then_erased,
        "an item made, then erased"
    );
}

#[test]
fn a_session_s_first_act_may_name_the_entry_it_writes() {
    // Catches a rule, or a split, that reserves its entry's number before
    // the session's start entry takes it: the rule's node then names an
    // entry that is not the rule's, and reasoning finds no value for it;
    // the split's run starts at the start entry, which undo then takes.
    let mut r = Room::new("first-act");
    r.items(&["Alder. Birch.", "Cedar"]);
    let mut again = replayed(&r.dir);
    let app: &mut App = &mut again;
    let rule = "constraint one date: false :- date(E, A), date(E, B), A \\= B.";
    run(app, rule);
    assert_eq!(app.evaluate(), Ok(()), "{}", app.session.message);
    let mut again = replayed(&r.dir);
    let app: &mut App = &mut again;
    run(app, "rule dated(E) :- date(E, _).");
    let kept = app.journal().entries().last().unwrap().clone();
    let def = &kept.body().unwrap()["facts"][0]["args"][0];
    assert_eq!(def.as_u64(), Some(kept.seq), "the rule names its own entry");
    let mut again = replayed(&r.dir);
    let app: &mut App = &mut again;
    let first = app.notebook.outline.in_order()[0];
    app.session.view.panes[0].cursor = first;
    run(app, "split");
    let split = app
        .journal()
        .entries()
        .last()
        .unwrap()
        .body()
        .unwrap()
        .clone();
    let start = starts_at(app);
    assert_eq!(split["op"], "split");
    assert_eq!(split["of"].as_u64(), Some(start + 1), "{split}");
}

/// Run a command line, as typed in the Command quasimode.
fn run(app: &mut App, line: &str) {
    app.handle(memex::app::Event::down(Key::Quasi(
        memex::app::Quasi::Command,
    )));
    for c in line.chars() {
        app.handle(memex::app::Event::down(Key::Char(c)));
    }
    app.handle(memex::app::Event::up(Key::Quasi(
        memex::app::Quasi::Command,
    )));
}

/// The number of the last start entry.
fn starts_at(app: &App) -> u64 {
    let entries = app.journal().entries().iter().rev();
    let start = entries
        .into_iter()
        .find(|e| e.body().is_some_and(|b| b["op"] == "start"));
    start.unwrap().seq
}
