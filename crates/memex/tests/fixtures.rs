//! The journal format pinned (docs/design.md, The journal format): a
//! journal the memex wrote at schema 1, kept in tests/fixtures, must check
//! and replay to the same outline and facts, whatever the code becomes. A
//! change that fails these tests changes what a journal already written
//! means: give it a new schema and a reason, and keep this fixture.

mod support;

use memex::app::{App, Key};
use support::{Room, replayed};

const SCHEMA1: &str = "tests/fixtures/schema1";

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
#[ignore = "writes the fixture: run it by hand, once, when a schema is made"]
fn write_schema1() {
    let mut r = Room::new("fixture-schema1");
    session(&mut r);
    std::fs::create_dir_all(SCHEMA1).unwrap();
    let journal = r.dir.join("journal.jsonl");
    std::fs::copy(journal, format!("{SCHEMA1}/journal.jsonl")).unwrap();
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
fn the_schema_1_journal_checks() {
    // Catches a change to the record's format that a journal already
    // written no longer passes.
    let text = std::fs::read_to_string(format!("{SCHEMA1}/journal.jsonl")).unwrap();
    let entries = format::chain::check_journal(&text).unwrap();
    assert_eq!(entries.len(), 61);
}

#[test]
fn the_schema_1_journal_replays_to_the_same_outline() {
    let app = replayed(std::path::Path::new(SCHEMA1));
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
fn the_schema_1_journal_replays_to_the_same_facts() {
    // Catches a change to what a journal's entries mean: here, the
    // adoption it retracted, and the fact of the item it deleted, hold no
    // more; its rule, its claim and its links do.
    let mut app = replayed(std::path::Path::new(SCHEMA1));
    let expected = [
        r#"claims("Talk-notes", date(buran_flight, 1989))"#,
        "date(buran_flight, 1988)",
        "link(18, 2)",
        "link(22, 2)",
        r#"node_def(36, constraint("Nothing has more than 1 date", [_0, _1, _2], date(_0, _1), date(_0, _2), \==(_1, _2)))"#,
    ];
    assert_eq!(facts(&mut app), expected);
}
