//! A spreadsheet that is not built in: boxes (items) holding values (facts),
//! rules relating them (derived text and constraints), and values changed
//! and tried in worlds. Nothing here is special to spreadsheets; the
//! tutorial's second part follows these steps.

use memex::app::{App, Event, Key, Mode, Quasi};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::path::{Path, PathBuf};

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("adversaria-sheet-{name}-{}", std::process::id()));
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

fn command(app: &mut App, line: &str) -> String {
    app.handle(Event::down(Key::Quasi(Quasi::Command)));
    typ(app, line);
    app.handle(Event::up(Key::Quasi(Quasi::Command)));
    app.evaluate().unwrap();
    let m = app.message.clone();
    assert!(
        !m.starts_with("no command") && !m.starts_with("not a") && !m.contains("not allowed"),
        "{line}: {m}"
    );
    m
}

fn suppose(app: &mut App, text: &str) -> String {
    app.handle(Event::down(Key::Quasi(Quasi::Suppose)));
    typ(app, text);
    app.handle(Event::up(Key::Quasi(Quasi::Suppose)));
    assert!(
        matches!(app.mode, Mode::ConfirmSuppose { .. }),
        "no reading of {text:?}: {}",
        app.message
    );
    tap(app, Key::Enter);
    app.evaluate().unwrap();
    app.message.clone()
}

fn screen(app: &App) -> Vec<String> {
    let (w, h) = (160, 40);
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

fn shows(app: &App, text: &str) {
    let s = screen(app);
    assert!(
        s.iter().any(|l| l.contains(text)),
        "screen lacks {text:?}:\n{}",
        s.join("\n")
    );
}

/// A box: a new item whose text is shown from a value it holds.
fn cell(app: &mut App, name: &str, label: &str, value: i64) {
    tap(app, Key::Enter);
    typ(app, name);
    command(app, &format!("fact cell({name}, {value})"));
    command(app, &format!("derive {label}: {{V}} where cell({name}, V)"));
}

/// The budget: three boxes, two derived from them, and two rules.
fn budget(dir: &Path) -> App {
    let mut app = App::open(dir).unwrap();
    app.set_durable(false);
    typ(&mut app, "Budget");
    cell(&mut app, "income", "Income", 2000);
    tap(&mut app, Key::Tab);
    cell(&mut app, "rent", "Rent", 1200);
    cell(&mut app, "food", "Food", 400);
    tap(&mut app, Key::Enter);
    typ(&mut app, "left");
    command(
        &mut app,
        "derive Left over: {L} where cell(income, I), cell(rent, R), cell(food, F), L is I - R - F",
    );
    command(
        &mut app,
        "constraint one value per cell: false :- cell(C, A), cell(C, B), A \\= B.",
    );
    command(
        &mut app,
        "constraint spending within income: false :- cell(income, I), cell(rent, R), cell(food, F), R + F > I.",
    );
    app.flush();
    app.evaluate().unwrap();
    shows(&app, "Left over: 400 [derived]");
    app
}

#[test]
fn a_supposition_sets_aside_only_what_it_directly_contradicts() {
    // Supposing food is 900 contradicts the old food value directly (one
    // value per cell: two facts, one repair) and breaks spending within
    // income together with income and rent (three facts: no single repair).
    // Catches a supposition that sets aside income and rent too, so every
    // formula has no answer in the supposed world, and commit removes them.
    let dir = temp("suppose");
    let mut app = budget(&dir);
    let m = suppose(&mut app, "food is 900");
    assert!(m.contains("set aside in this world: 1.3"), "{m}");
    assert!(
        !m.contains("1.1") && !m.contains("1.2"),
        "income and rent kept: {m}"
    );
    assert!(
        m.contains("left for you: 1 violation of spending within income"),
        "{m}"
    );
    shows(&app, "Left over: -100");
    shows(&app, "Income: 2000");
    command(&mut app, "commit");
    shows(&app, "Food: 900 [derived]");
    shows(&app, "Rent: 1200 [derived]");
    shows(&app, "Left over: -100 [derived]");
    shows(&app, "1 constraint violation");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn set_replaces_what_a_new_value_directly_contradicts() {
    // `set` states a fact and retracts the beliefs it directly contradicts,
    // in the actual world, and puts the new fact on the item that held the
    // one it replaces. Catches a set that leaves the old value (two values,
    // a violation), removes income and rent, or attaches the new value to
    // whatever item the cursor happens to be on.
    let dir = temp("set");
    let mut app = budget(&dir);
    // The cursor is on the last item ("Left over"), not on Food.
    assert!(app.outline.text(app.cursor()).starts_with("left"));
    let m = command(&mut app, "set cell(food, 900)");
    assert!(m.contains("replaced the fact at 1.3"), "{m}");
    assert!(
        m.contains("left for you: 1 violation of spending within income"),
        "{m}"
    );
    shows(&app, "Food: 900 [derived]");
    shows(&app, "Income: 2000 [derived]");
    shows(&app, "Left over: -100 [derived]");
    shows(&app, "1 constraint violation");
    let new = app
        .journal()
        .entries()
        .iter()
        .rev()
        .find(|e| e.kind() == "edit")
        .unwrap()
        .seq;
    let food = app.outline.fact_item[&new];
    assert_eq!(
        app.outline.text(food),
        "food",
        "the new value is on the Food box"
    );
    // Back within income: the violation goes, nothing else changes.
    let m = command(&mut app, "set cell(food, 500)");
    assert!(
        m.contains("replaced the fact at 1.3") && !m.contains("left for you"),
        "{m}"
    );
    shows(&app, "Left over: 300 [derived]");
    let s = screen(&app);
    assert!(
        !s.iter().any(|l| l.contains("constraint violation")),
        "{}",
        s.join("\n")
    );
    // A fact nothing contradicts is simply added, on the cursor's item.
    let m = command(&mut app, "set cell(savings, 300)");
    assert!(m.contains("nothing replaced"), "{m}");
    // The journal replays to the same values.
    let tip = app.tip;
    drop(app);
    let mut again = App::open(&dir).unwrap();
    again.evaluate().unwrap();
    assert_eq!(again.tip, tip);
    shows(&again, "Food: 500 [derived]");
    shows(&again, "Left over: 300 [derived]");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_rule_is_written_once_and_used_everywhere() {
    // `rule` defines a predicate from others. Catches a rule not seen by
    // derived text or constraints, a recursive rule accepted (it would loop
    // the reasoner), a rule whose head has a variable its body never binds,
    // and a rule body that calls what definitions may not.
    let dir = temp("rule");
    let mut app = budget(&dir);
    let m = command(
        &mut app,
        "rule spent(S) :- cell(rent, R), cell(food, F), S is R + F.",
    );
    assert!(m.contains("rule spent/1 kept"), "{m}");
    tap(&mut app, Key::Enter);
    typ(&mut app, "spent");
    command(&mut app, "derive Spent: {S} where spent(S)");
    shows(&app, "Spent: 1600 [derived]");
    command(
        &mut app,
        "constraint within income: false :- spent(S), cell(income, I), S > I.",
    );
    command(&mut app, "set cell(food, 900)");
    shows(&app, "Spent: 2100 [derived]");
    shows(&app, "within income, violation");
    for (line, why) in [
        ("rule above(X, Y) :- above(X, Z), above(Z, Y).", "recursive"),
        ("rule loop1(X) :- loop2(X).", ""),
        ("rule loop2(X) :- loop1(X).", "recursive"),
        ("rule free(X, Y) :- cell(X, _).", "Y"),
        ("rule bad(X) :- cell(X, _), shell(ls).", "not allowed"),
        ("rule rule_def(a, b, c) :- true.", "reserved"),
    ] {
        app.handle(Event::down(Key::Quasi(Quasi::Command)));
        typ(&mut app, line);
        app.handle(Event::up(Key::Quasi(Quasi::Command)));
        let m = app.message.clone();
        if why.is_empty() {
            assert!(m.contains("kept"), "{line}: {m}");
        } else {
            assert!(m.contains("not kept") && m.contains(why), "{line}: {m}");
        }
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn rules_work_under_negation_disjunction_and_findall() {
    // Catches control constructs that call a rule's head directly, where
    // only facts are defined, so a rule inside \+, ; or findall raises an
    // unknown-procedure error or never holds.
    let dir = temp("rule-control");
    let mut app = budget(&dir);
    command(&mut app, "rule big(C) :- cell(C, V), V >= 1000.");
    tap(&mut app, Key::Enter);
    typ(&mut app, "small");
    command(
        &mut app,
        "derive Small: {N} where findall(C, (cell(C, _), \\+ big(C)), L), length(L, N)",
    );
    shows(&app, "Small: 1 [derived]");
    tap(&mut app, Key::Enter);
    typ(&mut app, "either");
    command(
        &mut app,
        "derive Either: {N} where findall(C, (big(C) ; C = none), L), length(L, N)",
    );
    shows(&app, "Either: 3 [derived]");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_violation_through_a_rule_rests_on_the_facts_under_it() {
    // Catches a violation that names the rule, not the facts the user would
    // change, so marks appear on the rule's item and views drawing those
    // facts are not drawn loudly.
    let dir = temp("rule-prov");
    let mut app = budget(&dir);
    command(
        &mut app,
        "rule spent(S) :- cell(rent, R), cell(food, F), S is R + F.",
    );
    let rule = app.journal().entries().last().unwrap().seq;
    command(
        &mut app,
        "constraint within income: false :- spent(S), cell(income, I), S > I.",
    );
    command(&mut app, "set cell(food, 900)");
    let eval = app.eval[0].as_ref().unwrap();
    let c = eval
        .constraints
        .iter()
        .find(|c| c.constraint.name == "within income")
        .unwrap();
    assert_eq!(c.violations.len(), 1);
    let seqs: Vec<u64> = c.violations[0].prov.iter().map(|p| p.seq()).collect();
    assert!(!seqs.contains(&rule), "not the rule: {seqs:?}");
    let facts: Vec<String> = seqs
        .iter()
        .map(|s| memex::reason::show(&eval.facts.iter().find(|(q, _)| q == s).unwrap().1))
        .collect();
    for f in ["cell(rent, 1200)", "cell(food, 900)", "cell(income, 2000)"] {
        assert!(facts.contains(&f.to_string()), "{f} in {facts:?}");
    }
    shows(&app, "within income, violation");
    std::fs::remove_dir_all(&dir).unwrap();
}
