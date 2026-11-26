//! The journal format pinned (docs/design.md, The journal format): the
//! journals the memex wrote at version 1 and at version 2, kept in
//! tests/fixtures, must check and replay (version 1 migrated) to the same
//! outline and facts, whatever the code becomes. A change that fails
//! these tests changes what a journal already written means: give it a
//! new version, a migration and a reason, and keep these fixtures.

mod support;

use format::record::Entry;
use memex::app::{App, Key};
use support::{Room, replayed};

const VERSION1: &str = "tests/fixtures/version1";
const VERSION2: &str = "tests/fixtures/version2";
const VERSION3: &str = "tests/fixtures/version3";

/// The items the sessions write: the version-1 session the first seven,
/// the version-2 session also one that mentions another unlinked, so that
/// a suggestion is offered.
const ITEMS: [&str; 8] = [
    "Buran",
    "The date of Buran-flight is 1988.",
    "Talk-notes says that the date of Buran-flight is 1989.",
    "Nothing has more than 1 date.",
    "Notes #draft",
    "See [[Buran]].",
    "The rent is 1200.",
    "More on Buran",
];

/// A session that writes each kind of entry the memex writes: items and
/// their text, readings with their facts, a claim and its adoption, a
/// rule, a label, a [[link]], a link by command, a name, a move, a delete
/// undone and redone, a retraction, a supposition left, a find and jumps.
fn session(r: &mut Room) {
    r.items(&ITEMS[..7]);
    r.app.idle();
    for line in [
        "go Talk-notes",
        "adopt",
        "go Buran",
        "name launch",
        "go Notes",
        "link 1",
        "go See",
        "move after 1",
        "go The rent",
        "delete",
        "undo",
        "redo",
        "go Talk-notes",
    ] {
        r.command(line);
    }
    r.command("retract");
    r.tap(&[Key::Enter]);
    r.command("suppose the date of Buran-flight is 1990");
    r.tap(&[Key::Enter]);
    for line in ["dismiss world", "find #draft", "go Buran", "go back"] {
        r.command(line);
    }
    r.app.flush();
}

/// The version-1 session's steps, reasoned over after each as the window
/// does, and what version 2 adds: a verdict on the finding the adoption
/// raises, which goes when the adoption is retracted, and a visit to the
/// notebook while the inspector shows, which offers the suggestion it
/// lists.
fn session2(r: &mut Room) {
    r.items(&ITEMS);
    reason(r);
    for line in [
        "go Talk-notes",
        "adopt",
        "verdict known",
        "go parent",
        "go Buran",
        "name launch",
        "go Notes",
        "link 1",
        "go See",
        "move after 1",
        "go The rent",
        "delete",
        "undo",
        "redo",
        "go Talk-notes",
        "retract",
        "suppose the date of Buran-flight is 1990",
        "dismiss world",
        "find #draft",
        "go Buran",
        "go back",
    ] {
        r.command(line);
        if line == "retract" || line.starts_with("suppose") {
            r.tap(&[Key::Enter]);
        }
        reason(r);
    }
}

/// A pause, then reasoning, as the window does once the journal moves.
fn reason(r: &mut Room) {
    r.app.idle();
    r.app.evaluate().unwrap();
    r.app.flush();
}

#[test]
#[ignore = "writes the fixture: run it by hand when version 3 changes, before stage 1"]
fn write_version3() {
    let mut r = Room::new("fixture-version3");
    session2(&mut r);
    std::fs::create_dir_all(VERSION3).unwrap();
    let journal = r.dir.join("journal.jsonl");
    std::fs::copy(journal, format!("{VERSION3}/journal.jsonl")).unwrap();
}

#[test]
#[ignore = "writes the fixture: run it by hand, once, when a version is made"]
fn write_version2() {
    let mut r = Room::new("fixture-version2");
    session2(&mut r);
    std::fs::create_dir_all(VERSION2).unwrap();
    let journal = r.dir.join("journal.jsonl");
    std::fs::copy(journal, format!("{VERSION2}/journal.jsonl")).unwrap();
}

#[test]
#[ignore = "writes the fixture: run it by hand, once, when a version is made"]
fn write_version1() {
    let mut r = Room::new("fixture-version1");
    session(&mut r);
    std::fs::create_dir_all(VERSION1).unwrap();
    let journal = r.dir.join("journal.jsonl");
    std::fs::copy(journal, format!("{VERSION1}/journal.jsonl")).unwrap();
}

/// The outline of an app: each item's number and text, in order.
fn outline(app: &App) -> Vec<String> {
    let o = &app.notebook.outline;
    o.in_order()
        .iter()
        .map(|i| format!("{} {}", o.number(*i), o.text(*i)))
        .collect()
}

/// The facts of the world at the tip, shown and sorted.
fn facts(app: &mut App) -> Vec<String> {
    app.evaluate().unwrap();
    let e = app.reason.eval[0].as_ref().unwrap();
    let mut shown: Vec<String> = e
        .facts
        .iter()
        .map(|(_, t)| memex::reason::show(t))
        .collect();
    shown.sort();
    shown
}

#[test]
fn the_version_1_journal_checks() {
    // Catches a change to the record's format that a journal already
    // written no longer passes.
    let text = std::fs::read_to_string(format!("{VERSION1}/journal.jsonl")).unwrap();
    let entries = format::chain::check_journal(&text).unwrap();
    assert_eq!(entries.len(), 61);
}

#[test]
fn the_version_1_journal_replays_to_the_same_outline() {
    let app = replayed(std::path::Path::new(VERSION1));
    let expected = [
        "1 Buran",
        "2 See [[Buran]].",
        "3 The date of Buran-flight is 1988.",
        "4 Talk-notes says that the date of Buran-flight is 1989.",
        "5 Nothing has more than 1 date.",
        "6 Notes #draft",
    ];
    assert_eq!(outline(&app), expected);
}

#[test]
fn the_version_1_journal_replays_to_the_same_facts() {
    // Catches a change to what a journal's entries mean: here, the
    // adoption it retracted, and the fact of the item it deleted, hold no
    // more; its rule, its claim and its links do.
    let mut app = replayed(std::path::Path::new(VERSION1));
    let expected = [
        r#"claims("Talk-notes", date(buran_flight, 1989))"#,
        "date(buran_flight, 1988)",
        "link(18, 2)",
        "link(22, 2)",
        r#"node_def(36, constraint("Nothing has more than 1 date", [_0, _1, _2], date(_0, _1), date(_0, _2), \==(_1, _2)))"#,
    ];
    assert_eq!(facts(&mut app), expected);
}

/// A copy of the fixture of a version, with seals and public keys beside
/// its journal (stand-ins: a migration moves and copies them, never reads
/// them), in a directory of this test's own.
fn version_room(name: &str, version: u64) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("adversaria-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let fixture = [VERSION1, VERSION2][version as usize - 1];
    std::fs::copy(
        format!("{fixture}/journal.jsonl"),
        dir.join("journal.jsonl"),
    )
    .unwrap();
    let seals = format!("the seals of version {version}\n");
    std::fs::write(dir.join("seals.jsonl"), seals).unwrap();
    std::fs::write(dir.join("keys.jsonl"), "the public keys\n").unwrap();
    dir
}

/// A directory's file as text.
fn text_of(dir: &std::path::Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap()
}

#[test]
fn opening_a_version_1_journal_migrates_it_and_keeps_the_old_one() {
    // Catches a journal of version 1 appended to as if it were version 2,
    // the old journal or its seals lost or changed, or the seals left
    // beside a journal they no longer seal.
    let dir = version_room("migrate", 1);
    let before = text_of(&dir, "journal.jsonl");
    let app = App::open(&dir).unwrap();
    let said = &app.session.message;
    assert!(said.contains("migrated to version 3"), "{said}");
    drop(app);
    let header = format::record::header(format::record::JOURNAL_FORMAT);
    let after = text_of(&dir, "journal.jsonl");
    assert_eq!(after.lines().next(), Some(header.as_str()));
    assert_eq!(text_of(&dir, "v1/journal.jsonl"), before, "kept unchanged");
    assert_eq!(text_of(&dir, "v1/seals.jsonl"), "the seals of version 1\n");
    assert_eq!(text_of(&dir, "v1/keys.jsonl"), "the public keys\n");
    let beside = dir.join("seals.jsonl").exists();
    assert!(!beside, "no seals beside a journal they do not seal");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn opening_a_version_2_journal_migrates_it_and_keeps_the_old_one() {
    // Catches the journal of version 2 or its seals lost, or kept where
    // those of version 1 are.
    let dir = version_room("migrate-2", 2);
    let before = text_of(&dir, "journal.jsonl");
    let said = App::open(&dir).unwrap().session.message.clone();
    assert!(
        said.contains("of version 2: it is migrated to version 3"),
        "{said}"
    );
    assert_eq!(text_of(&dir, "v2/journal.jsonl"), before, "kept unchanged");
    assert_eq!(text_of(&dir, "v2/seals.jsonl"), "the seals of version 2\n");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_version_2_journal_migrated_has_its_offers_marked_drawn() {
    // Catches a journal of version 2 appended to as version 3: its offers,
    // recorded only once drawn, then counted as every suggestion made.
    let dir = version_room("migrate-2-drawn", 2);
    let app = App::open(&dir).unwrap();
    let bodies = app.journal().entries().iter().filter_map(|e| e.body());
    let offers: Vec<_> = bodies.filter(|b| b["op"] == "offer").cloned().collect();
    assert!(!offers.is_empty(), "the fixture records an offer");
    assert!(offers.iter().all(|b| b["drawn"] == true), "{offers:?}");
    let first = app.journal().entries()[0].body().unwrap().clone();
    assert_eq!(first["migrated_from"]["version"], 2, "{first}");
    drop(app);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_migrated_journal_continues_the_old_one_once() {
    // Catches the migrated entries cut loose from the old ones, or a
    // second migration on opening again.
    let dir = version_room("migrate-continues", 1);
    let old = entries_of(&dir);
    drop(App::open(&dir).unwrap());
    let new = entries_of(&dir);
    let last = old.last().unwrap().hash.as_str();
    let from = &new[0].body().unwrap()["migrated_from"];
    assert_eq!(from, &serde_json::json!({"version": 1, "last": last}));
    assert_eq!(
        shape(&new),
        shape(&old),
        "the same entries, after the same parents"
    );
    drop(App::open(&dir).unwrap());
    assert_eq!(entries_of(&dir), new, "migrated once");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A directory's journal, checked.
fn entries_of(dir: &std::path::Path) -> Vec<Entry> {
    format::chain::check_journal(&text_of(dir, "journal.jsonl")).unwrap()
}

/// Each entry's kind and parent.
fn shape(es: &[Entry]) -> Vec<(String, Option<u64>)> {
    es.iter()
        .map(|e| (e.kind().to_string(), e.parent_seq()))
        .collect()
}

#[test]
fn migrating_marks_the_memex_s_entries_and_where_writing_left_the_cursor() {
    // Catches a reading, its statements, its link or an observation left
    // as the user's; the retraction Suppose wrote (56) taken for the
    // memex's; a move after typing left a move; or any other change.
    let dir = version_room("migrate-infers", 1);
    let old = entries_of(&dir);
    drop(App::open(&dir).unwrap());
    let new = entries_of(&dir);
    let pairs = || old[1..].iter().zip(&new[1..]);
    let by = |e: &Entry| e.body().unwrap()["by"].clone();
    let marked = pairs().filter(|(o, n)| by(o) != by(n));
    let marked: Vec<u64> = marked.map(|(o, _)| o.seq).collect();
    let mut own: Vec<u64> = (30..=39).collect();
    own.extend([55, 57]);
    assert_eq!(marked, own);
    let written = pairs().filter(|(_, n)| n.body().unwrap()["op"] == "written");
    let written: Vec<u64> = written.map(|(o, _)| o.seq).collect();
    assert_eq!(written, (3..=29).step_by(2).collect::<Vec<u64>>());
    for (o, n) in pairs() {
        assert_eq!(as_version_1(n), o.body().cloned(), "entry {}", o.seq);
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A migrated entry's body with what migration adds taken off again.
fn as_version_1(e: &Entry) -> Option<serde_json::Value> {
    let mut b = e.body()?.clone();
    if b["op"] == "written" {
        b["op"] = serde_json::json!("move");
    }
    b.as_object_mut()?.remove("by");
    Some(b)
}

#[test]
fn a_version_1_journal_with_a_torn_line_is_kept_as_version_1() {
    // Catches the torn line set aside by writing the old journal under
    // the new header, so that the journal kept in v1/ claims a version
    // its entries are not in.
    let dir = version_room("migrate-torn", 1);
    let whole = text_of(&dir, "journal.jsonl");
    std::fs::write(dir.join("journal.jsonl"), format!("{whole}{{\"seq\":62,")).unwrap();
    drop(App::open(&dir).unwrap());
    assert_eq!(text_of(&dir, "v1/journal.jsonl"), whole, "version 1, whole");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_version_2_journal_checks_at_version_2() {
    // Catches a change to the record's format that a journal of version
    // 2 already written no longer passes.
    let text = text_of(std::path::Path::new(VERSION2), "journal.jsonl");
    let header = r#"{"format":"memex-journal","version":2}"#;
    assert_eq!(text.lines().next(), Some(header));
    assert_eq!(format::chain::check_journal(&text).unwrap().len(), 73);
}

#[test]
fn the_version_2_journal_migrated_has_the_same_outline_and_facts() {
    // Catches a change to what a journal of version 2 means, by its
    // migration to version 3 or otherwise.
    let mut app = replayed(std::path::Path::new(VERSION2));
    let items = [
        "1 Buran",
        "2 See [[Buran]].",
        "3 The date of Buran-flight is 1988.",
        "4 Talk-notes says that the date of Buran-flight is 1989.",
        "5 Nothing has more than 1 date.",
        "6 Notes #draft",
        "7 More on Buran",
    ];
    assert_eq!(outline(&app), items);
    let expected = [
        r#"claims("Talk-notes", date(buran_flight, 1989))"#,
        "date(buran_flight, 1988)",
        "link(19, 3)",
        "link(23, 3)",
        r#"node_def(41, constraint("Nothing has more than 1 date", [_0, _1, _2], date(_0, _1), date(_0, _2), \==(_1, _2)))"#,
    ];
    assert_eq!(facts(&mut app), expected);
    assert!(app.journal().dir().join("v2").exists(), "migrated");
}

#[test]
fn the_version_3_journal_checks_at_version_3() {
    // Catches a change to the record's format that a journal of version
    // 3 already written no longer passes.
    let text = text_of(std::path::Path::new(VERSION3), "journal.jsonl");
    let header = r#"{"format":"memex-journal","version":3}"#;
    assert_eq!(text.lines().next(), Some(header));
    assert!(format::chain::check_journal(&text).is_ok());
}

#[test]
fn the_version_3_journal_records_each_suggestion_in_its_words_and_no_view() {
    // Catches an offer of version 3 recorded only once drawn, without its
    // words, or with the view; or an observation with the view.
    let es = entries_of(std::path::Path::new(VERSION3));
    let bodies: Vec<&serde_json::Value> = es.iter().filter_map(|e| e.body()).collect();
    let records = |op: &str| bodies.iter().filter(|b| b["op"] == op).count();
    assert!(records("offer") > 0 && records("observed") > 0);
    let with_view = bodies
        .iter()
        .filter(|b| b["op"] == "offer" || b["op"] == "observed");
    assert!(
        with_view.clone().all(|b| b.get("panes").is_none()),
        "no view"
    );
    let offers = bodies.iter().filter(|b| b["op"] == "offer");
    for o in offers {
        let (keys, said) = (o["keys"].as_array().unwrap(), o["said"].as_array().unwrap());
        assert_eq!(keys.len(), said.len(), "each in its words: {o}");
    }
}

#[test]
fn the_version_2_journal_holds_each_record_version_2_adds() {
    // Catches the fixture written again without one of them: the build,
    // the memex's marks, views written, an offer, a verdict, a reason.
    let es = entries_of(std::path::Path::new(VERSION2));
    let bodies: Vec<&serde_json::Value> = es.iter().filter_map(|e| e.body()).collect();
    let count = |key: &str, value: &str| bodies.iter().filter(|b| b[key] == value).count();
    let counts = [
        count("by", "memex"),
        count("op", "written"),
        count("op", "offer"),
    ];
    assert_eq!(counts, [18, 16, 1]);
    assert_eq!(count("verdict", "known"), 1);
    let start = bodies.iter().find(|b| b["op"] == "start").unwrap();
    assert!(
        start["build"].is_string() && start.get("schema").is_none(),
        "{start}"
    );
    let gone = bodies.iter().filter_map(|b| b["gone"].as_array()).flatten();
    let why: Vec<&serde_json::Value> = gone.map(|g| &g["why"]).collect();
    assert_eq!(why, ["corrected"]);
}
