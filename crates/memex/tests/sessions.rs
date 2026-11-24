//! Each session's start entry (docs/design.md, The journal format): the
//! first entry a session writes follows one, which records the schema the
//! session writes in and the build that wrote it, so which build wrote each
//! entry, and where each session began, are in the record.

mod support;

use memex::app::App;
use support::{Room, replayed};

/// The start entries of a journal, by their bodies.
fn starts(app: &App) -> Vec<serde_json::Value> {
    let entries = app.journal().entries().iter();
    let start =
        entries.filter(|e| e.kind() == "view" && e.body().is_some_and(|b| b["op"] == "start"));
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
        ("view", &serde_json::json!("start"))
    );
    assert_eq!(first["schema"], 2);
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
