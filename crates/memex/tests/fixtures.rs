//! The journal format pinned (docs/design.md, The journal format): a
//! journal the memex wrote at version 1, kept in tests/fixtures, must check
//! and, migrated, replay to the same outline and facts, whatever the code
//! becomes. A change that fails these tests changes what a journal
//! already written means: give it a new version, a migration and a
//! reason, and keep this fixture.

mod support;

use format::record::Entry;
use memex::app::{App, Key};
use support::{Room, replayed};

const VERSION1: &str = "tests/fixtures/version1";

/// A session that writes each kind of entry the memex writes: items and
/// their text, readings with their facts, a claim and its adoption, a
/// rule, a label, a [[link]], a link by command, a name, a move, a delete
/// undone and redone, a retraction, a supposition left, a find and jumps.
fn session(r: &mut Room) {
    r.items(&[
        "Buran",
        "The date of Buran-flight is 1988.",
        "Talk-notes says that the date of Buran-flight is 1989.",
        "Nothing has more than 1 date.",
        "Notes #draft",
        "See [[Buran]].",
        "The rent is 1200.",
    ]);
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

/// A copy of the version-1 fixture, with seals and public keys beside its
/// journal (stand-ins: a migration moves and copies them, never reads
/// them), in a directory of this test's own.
fn version_1_room(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("adversaria-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        format!("{VERSION1}/journal.jsonl"),
        dir.join("journal.jsonl"),
    )
    .unwrap();
    std::fs::write(dir.join("seals.jsonl"), "the seals of version 1\n").unwrap();
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
    let dir = version_1_room("migrate");
    let before = text_of(&dir, "journal.jsonl");
    let app = App::open(&dir).unwrap();
    let said = &app.session.message;
    assert!(said.contains("migrated to version 2"), "{said}");
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
fn a_migrated_journal_continues_the_old_one_once() {
    // Catches the migrated entries cut loose from the old ones, or a
    // second migration on opening again.
    let dir = version_1_room("migrate-continues");
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
    let dir = version_1_room("migrate-infers");
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
    let dir = version_1_room("migrate-torn");
    let whole = text_of(&dir, "journal.jsonl");
    std::fs::write(dir.join("journal.jsonl"), format!("{whole}{{\"seq\":62,")).unwrap();
    drop(App::open(&dir).unwrap());
    assert_eq!(text_of(&dir, "v1/journal.jsonl"), whole, "version 1, whole");
    std::fs::remove_dir_all(&dir).unwrap();
}
