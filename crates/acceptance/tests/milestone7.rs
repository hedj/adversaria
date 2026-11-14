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
    app.handle(Event::down(Key::Quasi(Quasi::LeapBack)));
    typ(&mut app, "buran");
    app.handle(Event::up(Key::Quasi(Quasi::LeapBack)));
    app.evaluate().unwrap();
    let wrong = app
        .journal()
        .entries()
        .iter()
        .find(|e| e.body().is_some_and(|b| b.to_string().contains("1988")))
        .unwrap()
        .seq;
    command(&mut app, &format!("retract {wrong}"));
    tap(&mut app, Key::Enter);
    app.evaluate().unwrap();
    for f in ["p(a, b)", "p(c, d)", "q(a, b)", "q(c, d)"] {
        command(&mut app, &format!("fact {f}"));
    }
    command(&mut app, "pane suggestions");
    command(&mut app, "accept 1");
    command(&mut app, "dismiss 1");
    let adopted = app
        .journal()
        .entries()
        .iter()
        .rev()
        .find(|e| e.kind() == "adopt")
        .unwrap()
        .seq;
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
    // counts made from that list by hand: writing 2 4 6 7 8 13 14 15 16;
    // navigating 3 5 17 22; searching 10; deciding 11 19 20 21;
    // observations 9 12 18; entry 1 is the memex's own. Entry 18 offers two
    // rules; 19 adopts one, 20 dismisses one, 21 retracts 19. Entry 9 shows
    // the violation date(buran, 1988) · date(buran, 1989); entry 12, in the
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
    assert_eq!(r.entries, 22);
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
    assert_eq!((r.observations, r.unclassified), (3, 1));
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

#[test]
fn runs_on_any_valid_journal() {
    // Plan: "the report runs on any valid journal". An empty journal; and
    // 10,000 entries of every kind with arbitrary bodies, as milestone 1
    // generates them: every entry is counted once, as an action or not.
    let dir = temp("any");
    let j = memex::journal::Journal::open(&dir).unwrap();
    let r = tools::measures::measure(j.entries());
    assert_eq!(
        (r.entries, r.observations, r.actions.values().sum::<usize>()),
        (0, 0, 0)
    );
    let mut j = j;
    j.durable = false;
    let kinds = [
        "edit", "link", "adopt", "retract", "suppose", "name", "import", "undo", "redo", "view",
    ];
    let mut x: u64 = 7;
    for i in 0..10_000u64 {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let kind = kinds[(x >> 33) as usize % kinds.len()];
        let body = match (x >> 40) % 4 {
            0 => serde_json::json!({}),
            1 => serde_json::json!({"op": "leap", "n": i}),
            2 => serde_json::json!({"op": "observed", "violations": ["v"], "world": "actual"}),
            _ => serde_json::json!({"target": i / 2, "suggestion": {}}),
        };
        j.append(kind, body).unwrap();
    }
    let r = tools::measures::measure(j.entries());
    let actions: usize = r.actions.values().sum();
    assert_eq!(r.entries, 10_000);
    assert_eq!(
        actions + r.observations + r.unclassified,
        10_000,
        "every entry counted once"
    );
    let unknown = j
        .entries()
        .iter()
        .filter(|e| {
            tools::measures::kind(e).is_none()
                && !(e.kind() == "view" && e.body().unwrap()["op"] == "observed")
        })
        .count();
    assert_eq!(r.unclassified, unknown);
    // The program: a report for a valid journal, a refusal for a broken one.
    let bin = env!("CARGO_MANIFEST_DIR").to_string() + "/../../target/debug/measures";
    let ok = std::process::Command::new("cargo")
        .args(["build", "-q", "-p", "tools", "--bin", "measures"])
        .status()
        .unwrap();
    assert!(ok.success());
    let out = std::process::Command::new(&bin).arg(&dir).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("journal: 10000 entries"));
    let p = dir.join("journal.jsonl");
    let text = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, text.replacen("\"n\":3", "\"n\":4", 1)).unwrap();
    let out = std::process::Command::new(&bin).arg(&dir).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "a journal that does not check gets no report"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
