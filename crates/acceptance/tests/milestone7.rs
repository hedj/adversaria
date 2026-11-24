//! Milestone 7's acceptance checks (docs/plan.md, Milestone 7): the
//! measures report "runs on any valid journal, and its counts match a hand
//! count on a small journal".

use memex::app::{App, Event, Key, Quasi};
use std::path::{Path, PathBuf};

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("adversaria-m7-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn tap(app: &mut App, k: Key) {
    app.handle(Event::down(k));
    app.handle(Event::up(k));
}

fn typ(app: &mut App, s: &str) {
    for c in s.chars() {
        tap(app, Key::Char(c));
    }
}

fn command(app: &mut App, line: &str) {
    app.handle(Event::down(Key::Quasi(Quasi::Command)));
    typ(app, line);
    app.handle(Event::up(Key::Quasi(Quasi::Command)));
    app.evaluate().unwrap();
}

/// The first entry whose body holds a text.
fn first_containing(app: &App, text: &str) -> u64 {
    let entries = app.journal().entries().iter();
    entries
        .into_iter()
        .find(|e| holds_text(e, text))
        .unwrap()
        .seq
}

fn holds_text(e: &format::record::Entry, text: &str) -> bool {
    e.body().is_some_and(|b| b.to_string().contains(text))
}

/// The last entry of a kind.
fn last_of_kind(app: &App, kind: &str) -> u64 {
    let entries = app.journal().entries().iter().rev();
    entries.into_iter().find(|e| e.kind() == kind).unwrap().seq
}

/// A small session that touches every measure.
pub fn session(dir: &Path) -> App {
    let mut app = App::open(dir).unwrap();
    app.set_durable(false);
    typ(&mut app, "Buran flew in 1989");
    app.flush();
    command(&mut app, "fact date(buran, 1989)");
    command(&mut app, "fact date(buran, 1988)");
    command(
        &mut app,
        "constraint one date: false :- date(E, A), date(E, B), A \\= B.",
    );
    app.handle(Event::down(Key::Quasi(Quasi::LeapUp)));
    typ(&mut app, "buran");
    app.handle(Event::up(Key::Quasi(Quasi::LeapUp)));
    app.evaluate().unwrap();
    let wrong = first_containing(&app, "1988");
    command(&mut app, &format!("retract {wrong}"));
    tap(&mut app, Key::Enter);
    app.evaluate().unwrap();
    for f in ["p(a, b)", "p(c, d)", "q(a, b)", "q(c, d)"] {
        command(&mut app, &format!("fact {f}"));
    }
    command(&mut app, "pane suggestions");
    command(&mut app, "accept 1");
    command(&mut app, "dismiss 1");
    let adopted = last_of_kind(&app, "adopt");
    command(&mut app, &format!("retract {adopted}"));
    tap(&mut app, Key::Enter);
    app.evaluate().unwrap();
    tap(&mut app, Key::Down);
    app.flush();
    app
}

#[test]
fn counts_match_a_hand_count() {
    // The entries of the small session, read by hand (kind and op), and the
    // counts made from that list by hand: writing 3 5 7 8 9 14 15 16 17;
    // navigating 4 6 18 23; searching 11; deciding 12 20 21 22;
    // observations 10 13 19; entries 1 and 2 are the memex's own: the
    // journal's first, unclassified, and the session's start, marked as
    // written by the memex on its own. Entry 19 offers two rules;
    // 20 adopts one, 21 dismisses one, 22 retracts 20. Entry 10 shows the
    // violation date(buran, 1988) · date(buran, 1989); entry 13, in the
    // actual world after the retraction, shows none.
    let dir = temp("hand");
    let app = session(&dir);
    let seen: Vec<(String, String)> = app
        .journal()
        .entries()
        .iter()
        .map(|e| {
            (
                e.kind().to_string(),
                e.body().unwrap()["op"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    let by_hand = [
        ("edit", "init"),
        ("view", "start"),
        ("edit", "create"),
        ("view", "move"),
        ("edit", "text"),
        ("view", "move"),
        ("edit", "facts"),
        ("edit", "facts"),
        ("edit", "constraint"),
        ("view", "observed"),
        ("view", "leap"),
        ("retract", ""),
        ("view", "observed"),
        ("edit", "facts"),
        ("edit", "facts"),
        ("edit", "facts"),
        ("edit", "facts"),
        ("view", "pane"),
        ("view", "offer"),
        ("adopt", ""),
        ("view", "dismiss"),
        ("retract", ""),
        ("view", "move"),
    ];
    assert_eq!(
        seen,
        by_hand
            .iter()
            .map(|(k, o)| (k.to_string(), o.to_string()))
            .collect::<Vec<_>>(),
        "the journal is not the one counted by hand"
    );
    let r = tools::measures::measure(app.journal().entries());
    assert_eq!(r.entries, 23);
    assert_eq!(r.actions["writing"], 9);
    assert_eq!(r.actions["navigating"], 4);
    assert_eq!(r.actions["searching"], 1);
    assert_eq!(r.actions["deciding"], 4);
    assert_eq!(
        r.actions["following links"]
            + r.actions["checking"]
            + r.actions["importing"]
            + r.actions["exporting"],
        0
    );
    assert_eq!((r.observations, r.by_memex, r.unclassified), (3, 1, 1));
    assert_eq!(
        (r.offered, r.adopted, r.dismissed, r.adopted_then_retracted),
        (2, 1, 1, 1)
    );
    assert_eq!(
        (
            r.violations_seen,
            r.violations_corrected,
            r.disagreements_seen
        ),
        (1, 1, 0)
    );
    assert!(
        r.to_string()
            .contains("clerical share (searching, following links, navigating): 5 of 18, 27.8%")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

// The next three: the plan's "the report runs on any valid journal". An
// empty journal; and 10,000 entries of every kind with arbitrary bodies, as
// milestone 1 generates them: every entry is counted once, as an action or
// not.

#[test]
fn the_report_of_an_empty_journal_counts_nothing() {
    let dir = temp("empty");
    let j = memex::journal::Journal::open(&dir).unwrap();
    let r = tools::measures::measure(j.entries());
    assert_eq!(totals(&r), (0, 0, 0));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn every_entry_of_any_journal_is_counted_once() {
    let dir = temp("any");
    let j = arbitrary_journal(&dir);
    let r = tools::measures::measure(j.entries());
    assert_eq!(r.entries, 10_000);
    assert_eq!(counted(&r), 10_000, "every entry counted once");
    assert_eq!(r.unclassified, unclassified_by_hand(j.entries()));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_program_reports_a_valid_journal_and_refuses_a_broken_one() {
    let dir = temp("program");
    arbitrary_journal(&dir);
    let out = measures_program().arg(&dir).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("journal: 10000 entries"));
    let p = dir.join("journal.jsonl");
    let text = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, text.replacen("\"n\":3", "\"n\":4", 1)).unwrap();
    let out = measures_program().arg(&dir).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "a journal that does not check gets no report"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Every entry a report counts: as an action, an observation, the
/// memex's own, or none of these.
fn counted(r: &tools::measures::Report) -> usize {
    let actions: usize = r.actions.values().sum();
    actions + r.observations + r.by_memex + r.unclassified
}

/// A report's entries, observations and actions.
fn totals(r: &tools::measures::Report) -> (usize, usize, usize) {
    (r.entries, r.observations, r.actions.values().sum())
}

/// A journal of 10,000 entries of every kind, with arbitrary bodies.
fn arbitrary_journal(dir: &Path) -> memex::journal::Journal {
    let mut j = memex::journal::Journal::open(dir).unwrap();
    j.durable = false;
    let mut x: u64 = 7;
    for i in 0..10_000u64 {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let (kind, body) = arbitrary_entry(x, i);
        j.append(kind, body).unwrap();
    }
    j
}

/// Entry i's kind and body, from the generator's state x.
fn arbitrary_entry(x: u64, i: u64) -> (&'static str, serde_json::Value) {
    const KINDS: [&str; 10] = [
        "edit", "link", "adopt", "retract", "suppose", "name", "import", "undo", "redo", "view",
    ];
    let body = match (x >> 40) % 4 {
        0 => serde_json::json!({}),
        1 => serde_json::json!({"op": "leap", "n": i}),
        2 => serde_json::json!({"op": "observed", "violations": ["v"], "world": "actual"}),
        _ => serde_json::json!({"target": i / 2, "suggestion": {}}),
    };
    (KINDS[(x >> 33) as usize % KINDS.len()], body)
}

/// The entries that are neither actions nor observations, counted by hand.
fn unclassified_by_hand(entries: &[format::record::Entry]) -> usize {
    let observed =
        |e: &format::record::Entry| e.kind() == "view" && e.body().unwrap()["op"] == "observed";
    let neither = entries
        .iter()
        .filter(|e| tools::measures::kind(e).is_none() && !observed(e));
    neither.count()
}

/// The measures program, built.
fn measures_program() -> std::process::Command {
    let built = std::process::Command::new("cargo")
        .args(["build", "-q", "-p", "tools", "--bin", "measures"])
        .status()
        .unwrap();
    assert!(built.success());
    std::process::Command::new(
        env!("CARGO_MANIFEST_DIR").to_string() + "/../../target/debug/measures",
    )
}
