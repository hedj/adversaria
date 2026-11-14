//! Milestone 5's acceptance checks (docs/plan.md, Milestone 5): Wikidata and
//! suggestions without a model. tests/fixtures/wikidata-buran.pl is real
//! Wikidata (CC0), made by `wikidata --items Q2548,Q4273185,Q1010251,Q80811`.

use memex::app::{App, Event, Key, PaneKind, Quasi};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::path::{Path, PathBuf};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/wikidata-buran.pl"
);

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("adversaria-m5-{name}-{}", std::process::id()));
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
    assert!(
        ![
            "no command",
            "not a",
            "there is no",
            "import (",
            "cannot",
            "no item"
        ]
        .iter()
        .any(|p| app.message.starts_with(p)),
        "{line}: {}",
        app.message
    );
    app.evaluate().unwrap();
}

fn item(app: &mut App, text: &str) {
    tap(app, Key::Enter);
    typ(app, text);
    app.flush();
}

fn screen(app: &App) -> Vec<String> {
    let (w, h) = (200, 45);
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| memex::ui::draw(f, app)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

fn assert_shows(s: &[String], text: &str) {
    assert!(
        s.iter().any(|l| l.contains(text)),
        "screen lacks {text:?}:\n{}",
        s.join("\n")
    );
}

fn described(app: &App) -> Vec<String> {
    app.shown_suggestions()
        .iter()
        .map(|s| s.describe())
        .collect()
}

/// Accept the shown suggestion whose description starts so.
fn accept(app: &mut App, start: &str) {
    let n = described(app)
        .iter()
        .position(|d| d.starts_with(start))
        .unwrap_or_else(|| panic!("no suggestion {start:?} in {:?}", described(app)))
        + 1;
    command(app, &format!("accept {n}"));
}

fn open(dir: &Path) -> App {
    let mut app = App::open(dir).unwrap();
    app.set_durable(false);
    app
}

#[test]
fn a_planted_wrong_date_is_reported_against_wikidata() {
    // Plan: "A planted wrong date in the user's notes is reported against
    // Wikidata." The notes say Buran flew in 1989; Wikidata says 1988.
    let dir = temp("planted-date");
    let mut app = open(&dir);
    typ(&mut app, "Buran flew once, unmanned, in 1989.");
    app.flush();
    command(&mut app, "fact date(buran_flight, 1989)");
    command(&mut app, "fact label(buran_flight, \"Buran's flight\")");
    command(
        &mut app,
        "constraint one date per event: false :- date(E, D1), date(E, D2), D1 \\= D2.",
    );
    command(&mut app, &format!("import wikidata {FIXTURE}"));
    let import = app.journal().entries().last().unwrap().clone();
    assert_eq!(import.kind(), "import");
    assert_eq!(import.body().unwrap()["source"], "Wikidata");
    assert!(
        dir.join("blobs")
            .join(import.body().unwrap()["blob"].as_str().unwrap())
            .exists()
    );
    // The spaceplane, not the village of the same name, comes first.
    assert_eq!(
        described(&app).first().map(String::as_str),
        Some("buran_flight is Wikidata Q2548"),
        "{:?}",
        described(&app)
    );
    assert!(
        app.shown_suggestions()[0]
            .evidence
            .contains("within five years")
    );
    command(&mut app, "pane suggestions");
    accept(&mut app, "buran_flight is Wikidata Q2548");
    let adopt = app
        .journal()
        .entries()
        .iter()
        .rev()
        .find(|e| e.kind() == "adopt")
        .unwrap();
    assert!(
        adopt.body().unwrap()["suggestion"]["evidence"]
            .as_str()
            .unwrap()
            .contains("Q2548"),
        "the adoption keeps its evidence"
    );
    accept(&mut app, "date is Wikidata's P606");
    let s = screen(&app);
    assert_shows(&s, "1 disagreement between sources");
    assert_shows(
        &s,
        "⚑ one date per event, sources disagree: 1989 here, 1988 in Wikidata",
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn wikidata_offers_its_own_constraint() {
    // Plan: "Wikidata's property constraints offered as constraints". On
    // real data: P619 (UTC date of spacecraft launch) has a single-value
    // constraint. Tried against a planted violation before it is kept.
    let dir = temp("wd-constraint");
    let mut app = open(&dir);
    typ(&mut app, "Sputnik 1 was launched in 1957.");
    app.flush();
    command(&mut app, "fact launched(sputnik_1, 1957)");
    command(&mut app, "fact label(sputnik_1, \"Sputnik 1\")");
    command(&mut app, &format!("import wikidata {FIXTURE}"));
    accept(&mut app, "sputnik_1 is Wikidata Q80811");
    accept(&mut app, "launched is Wikidata's P619");
    accept(
        &mut app,
        "constraint one launched per entity (Wikidata P619)",
    );
    assert!(
        app.outline
            .constraints
            .values()
            .any(|c| c.name == "one launched per entity (Wikidata P619)")
    );
    let e = app.journal().entries().last().unwrap();
    assert_eq!(
        (e.kind(), &e.body().unwrap()["op"]),
        ("adopt", &serde_json::json!("constraint"))
    );
    assert!(
        !described(&app).iter().any(|d| d.starts_with("constraint")),
        "not offered again once adopted"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_mined_rule_shows_its_evidence_and_derives_once_adopted() {
    // Plan: "rules mined from the facts (... a simpler miner with support
    // and confidence)". Catches a rule offered without its support and
    // exceptions, or one that derives nothing once adopted.
    let dir = temp("rules");
    let mut app = open(&dir);
    typ(&mut app, "Places");
    app.flush();
    for f in [
        "part_of(baikonur, kazakhstan)",
        "part_of(plesetsk, russia)",
        "located_in(baikonur, kazakhstan)",
        "located_in(plesetsk, russia)",
    ] {
        command(&mut app, &format!("fact {f}"));
    }
    accept(&mut app, "rule located_in(_");
    let e = app.journal().entries().last().unwrap();
    assert!(
        e.body().unwrap()["suggestion"]["evidence"]
            .as_str()
            .unwrap()
            .contains("support 2, confidence 1.00, 0 exceptions")
    );
    item(&mut app, "Vostochny");
    command(&mut app, "fact part_of(vostochny, russia)");
    item(&mut app, "derived");
    command(
        &mut app,
        "derive Vostochny is in {C}. where located_in(vostochny, C)",
    );
    assert_shows(&screen(&app), "Vostochny is in russia. [derived]");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn no_suggested_reference_names_a_work_outside_the_store() {
    // Plan: "No suggested reference names a work outside the store (test
    // with a store that lacks the obvious answer)." The works here are
    // synthetic, ids QTEST1 to QTEST3: QTEST3 is about Buran and cited by
    // both others, the obvious answer, and it is not in the store.
    let dir = temp("refs");
    let subset = dir.with_extension("pl");
    std::fs::write(
        &subset,
        "wd('Q2548', 'P606', date(1988, 11, 15)).\nwd_label('Q2548', \"Buran\").\n\
         wd('QTEST1', 'P31', 'Q13442814').\nwd('QTEST1', 'P921', 'Q2548').\nwd('QTEST1', 'P2860', 'QTEST3').\n\
         wd('QTEST2', 'P31', 'Q13442814').\nwd('QTEST2', 'P2860', 'QTEST1').\nwd('QTEST2', 'P2860', 'QTEST3').\n\
         wd('QTEST3', 'P31', 'Q13442814').\nwd('QTEST3', 'P921', 'Q2548').\nwd_label('QTEST3', \"The obvious answer\").\n",
    )
    .unwrap();
    let mut app = open(&dir);
    typ(&mut app, "Buran flew once, in 1988.");
    app.flush();
    command(&mut app, "fact date(buran_flight, 1988)");
    command(&mut app, "fact same_as(buran_flight, 'Q2548')");
    item(&mut app, "Paper A");
    command(&mut app, "fact same_as(paper_a, 'QTEST1')");
    item(&mut app, "Paper B");
    command(&mut app, "fact same_as(paper_b, 'QTEST2')");
    command(&mut app, &format!("import wikidata {}", subset.display()));
    let refs: Vec<String> = app
        .suggestions
        .iter()
        .filter(|s| s.kind_name() == "reference")
        .map(|s| s.describe())
        .collect();
    assert_eq!(
        refs,
        vec!["cite paper_a for the claims about buran_flight".to_string()]
    );
    for s in &app.suggestions {
        assert!(
            !s.describe().contains("QTEST3") && !s.evidence.contains("QTEST3"),
            "names a work outside the store: {s:?}"
        );
    }
    accept(&mut app, "cite paper_a");
    let link = app.outline.links.last().unwrap();
    assert_eq!(
        (link.from, link.to, link.kind.as_str()),
        (
            app.outline.by_number("1").unwrap(),
            app.outline.by_number("2").unwrap(),
            "cites"
        )
    );
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::remove_file(&subset).unwrap();
}

#[test]
fn the_pane_keeps_its_rules() {
    // Plan: "never takes the keyboard, never reorders under the pointer,
    // ranked by evidence, few in view, dismissals recorded".
    let dir = temp("pane");
    let mut app = open(&dir);
    typ(&mut app, "Places");
    app.flush();
    let pairs = [("a", "x"), ("b", "y"), ("c", "z")];
    for (p, q) in ["p1", "p2", "p3", "p4"]
        .iter()
        .zip(["q1", "q2", "q3", "q4"])
    {
        for (x, y) in pairs {
            command(&mut app, &format!("fact {p}({x}, {y})"));
            command(&mut app, &format!("fact {q}({x}, {y})"));
        }
    }
    assert!(
        app.suggestions.len() > 5,
        "{} suggestions",
        app.suggestions.len()
    );
    assert_eq!(app.shown_suggestions().len(), 5, "few in view");
    // Ranked by evidence: scores never rise down the list.
    let scores: Vec<i64> = app.suggestions.iter().map(|s| s.score).collect();
    assert!(scores.windows(2).all(|w| w[0] >= w[1]), "{scores:?}");
    // It never takes the keyboard: typing goes to the outline.
    typ(&mut app, " here");
    app.flush();
    assert_eq!(app.outline.text(app.cursor()), "Places here");
    // Dismissed suggestions do not come back, and the dismissal is journaled.
    let first = described(&app)[0].clone();
    command(&mut app, "dismiss 1");
    assert!(!described(&app).contains(&first));
    assert_eq!(
        app.journal().entries().last().unwrap().body().unwrap()["op"],
        "dismiss"
    );
    // With the pane in focus, the order holds while new evidence arrives.
    command(&mut app, "pane suggestions");
    command(&mut app, "switch");
    assert_eq!(app.view.panes[app.view.focus].kind, PaneKind::Suggestions);
    let before = described(&app);
    for (x, y) in [("d", "w"), ("e", "v"), ("f", "u")] {
        command(&mut app, &format!("fact p4({x}, {y})"));
        command(&mut app, &format!("fact q1({x}, {y})"));
    }
    let during = described(&app);
    // A suggestion whose evidence no longer holds may leave; those that stay
    // keep their order, ahead of new ones.
    let kept: Vec<&String> = before.iter().filter(|b| during.contains(b)).collect();
    assert!(!kept.is_empty(), "the test needs some suggestions to stay");
    assert_eq!(
        during.iter().take(kept.len()).collect::<Vec<_>>(),
        kept,
        "reordered under the pointer"
    );
    // A replay keeps the dismissal.
    app.flush();
    let mut r = App::open(&dir).unwrap();
    r.evaluate().unwrap();
    assert_eq!(r.dismissed, app.dismissed);
    assert!(!r.suggestions.iter().any(|s| s.describe() == first));
    std::fs::remove_dir_all(&dir).unwrap();
}
