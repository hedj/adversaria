//! A spreadsheet that is not built in: boxes (items) holding values (facts),
//! rules relating them (derived text and constraints), and values changed
//! and tried in worlds. Nothing here is special to spreadsheets; the
//! tutorial's second part follows these steps.

use memex::app::{App, Event, Key, Mode, Quasi};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

mod support;
use support::{Room, replayed};

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
    support::command(app, line);
    app.evaluate().unwrap();
    app.session.message.clone()
}

fn suppose(app: &mut App, text: &str) -> String {
    app.handle(Event::down(Key::Quasi(Quasi::Suppose)));
    typ(app, text);
    app.handle(Event::up(Key::Quasi(Quasi::Suppose)));
    assert!(
        matches!(app.session.mode, Mode::ConfirmSuppose { .. }),
        "no reading of {text:?}: {}",
        app.session.message
    );
    tap(app, Key::Enter);
    app.evaluate().unwrap();
    app.session.message.clone()
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
fn put_box(app: &mut App, name: &str, label: &str, value: i64) {
    tap(app, Key::Enter);
    typ(app, name);
    command(app, &format!("fact box({name}, {value})"));
    command(app, &format!("derive {label}: {{V}} where box({name}, V)"));
}

/// The budget: three boxes, two derived from them, and two rules.
fn budget(name: &str) -> Room {
    let mut app = Room::new(&format!("sheet-{name}"));
    app.set_durable(false);
    typ(&mut app, "Budget");
    put_box(&mut app, "income", "Income", 2000);
    tap(&mut app, Key::Tab);
    put_box(&mut app, "rent", "Rent", 1200);
    put_box(&mut app, "food", "Food", 400);
    tap(&mut app, Key::Enter);
    typ(&mut app, "left");
    command(
        &mut app,
        "derive Left over: {L} where box(income, I), box(rent, R), box(food, F), L is I - R - F",
    );
    command(
        &mut app,
        "constraint one value per cell: false :- box(C, A), box(C, B), A \\= B.",
    );
    command(
        &mut app,
        "constraint spending within income: false :- box(income, I), box(rent, R), box(food, F), R + F > I.",
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
    let mut app = budget("suppose");
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
}

#[test]
fn set_replaces_what_a_new_value_directly_contradicts() {
    // `set` states a fact and retracts the beliefs it directly contradicts,
    // in the actual world, and puts the new fact on the item that held the
    // one it replaces. Catches a set that leaves the old value (two values,
    // a violation), removes income and rent, or attaches the new value to
    // whatever item the cursor happens to be on.
    let mut app = budget("set");
    // The cursor is on the last item ("Left over"), not on Food.
    assert!(app.here().starts_with("left"));
    over_budget_on_the_food_box(&mut app);
    back_within_income(&mut app);
    // A fact nothing contradicts is simply added, on the cursor's item.
    let m = command(&mut app, "set box(savings, 300)");
    assert!(m.contains("nothing replaced"), "{m}");
    the_journal_replays_to_the_same_values(&app);
}

/// Food set to 900: the old value replaced, income and rent kept, the
/// violation left, and the new value on the Food box.
fn over_budget_on_the_food_box(app: &mut Room) {
    let m = command(app, "set box(food, 900)");
    assert!(m.contains("replaced the fact at 1.3"), "{m}");
    let left = "left for you: 1 violation of spending within income";
    assert!(m.contains(left), "{m}");
    for text in [
        "Food: 900 [derived]",
        "Income: 2000 [derived]",
        "Left over: -100 [derived]",
        "1 constraint violation",
    ] {
        shows(app, text);
    }
    let food = app.notebook.outline.fact_item[&app.last("edit")];
    assert_eq!(app.text(food), "food", "the new value is on the Food box");
}

/// Food set to 500: back within income, the violation goes, nothing else
/// changes.
fn back_within_income(app: &mut Room) {
    let m = command(app, "set box(food, 500)");
    let only = m.contains("replaced the fact at 1.3") && !m.contains("left for you");
    assert!(only, "{m}");
    shows(app, "Left over: 300 [derived]");
    let s = screen(app);
    let violated = s.iter().any(|l| l.contains("constraint violation"));
    assert!(!violated, "{}", s.join("\n"));
}

/// The journal replays to the same tip and values.
fn the_journal_replays_to_the_same_values(app: &Room) {
    let mut again = replayed(&app.dir);
    again.evaluate().unwrap();
    assert_eq!(again.notebook.tip, app.notebook.tip);
    shows(&again, "Food: 500 [derived]");
    shows(&again, "Left over: 300 [derived]");
}

#[test]
fn a_rule_is_written_once_and_used_everywhere() {
    // `rule` defines a predicate from others. Catches a rule not seen by
    // derived text or constraints, a recursive rule accepted (it would loop
    // the reasoner), a rule whose head has a variable its body never binds,
    // and a rule body that calls what definitions may not.
    let mut app = budget("rule");
    let m = command(
        &mut app,
        "rule spent(S) :- box(rent, R), box(food, F), S is R + F.",
    );
    assert!(m.contains("rule spent/1 kept"), "{m}");
    tap(&mut app, Key::Enter);
    typ(&mut app, "spent");
    command(&mut app, "derive Spent: {S} where spent(S)");
    shows(&app, "Spent: 1600 [derived]");
    command(
        &mut app,
        "constraint within income: false :- spent(S), box(income, I), S > I.",
    );
    command(&mut app, "set box(food, 900)");
    shows(&app, "Spent: 2100 [derived]");
    shows(&app, "breaks “within income”");
    each_bad_rule_is_refused(&mut app);
}

/// A recursive rule, directly or through another, a rule whose head has a
/// variable its body never binds, one whose body calls what definitions
/// may not, and one of a reserved predicate are each refused, and why.
fn each_bad_rule_is_refused(app: &mut Room) {
    for (line, why) in [
        ("rule above(X, Y) :- above(X, Z), above(Z, Y).", "recursive"),
        ("rule loop1(X) :- loop2(X).", ""),
        ("rule loop2(X) :- loop1(X).", "recursive"),
        ("rule free(X, Y) :- box(X, _).", "Y"),
        ("rule bad(X) :- box(X, _), shell(ls).", "not allowed"),
        ("rule rule_def(a, b, c) :- true.", "reserved"),
    ] {
        app.typed_command(line);
        let m = &app.session.message;
        let refused = m.contains("not kept") && m.contains(why);
        assert!(
            if why.is_empty() {
                m.contains("kept")
            } else {
                refused
            },
            "{line}: {m}"
        );
    }
}

#[test]
fn rules_work_under_negation_disjunction_and_findall() {
    // Catches control constructs that call a rule's head directly, where
    // only facts are defined, so a rule inside \+, ; or findall raises an
    // unknown-procedure error or never holds.
    let mut app = budget("rule-control");
    command(&mut app, "rule big(C) :- box(C, V), V >= 1000.");
    tap(&mut app, Key::Enter);
    typ(&mut app, "small");
    command(
        &mut app,
        "derive Small: {N} where findall(C, (box(C, _), \\+ big(C)), L), length(L, N)",
    );
    shows(&app, "Small: 1 [derived]");
    tap(&mut app, Key::Enter);
    typ(&mut app, "either");
    command(
        &mut app,
        "derive Either: {N} where findall(C, (big(C) ; C = none), L), length(L, N)",
    );
    shows(&app, "Either: 3 [derived]");
}

#[test]
fn a_violation_through_a_rule_rests_on_the_facts_under_it() {
    // Catches a violation that names the rule, not the facts the user would
    // change, so marks appear on the rule's item and views drawing those
    // facts are not drawn loudly.
    let mut app = budget("rule-prov");
    command(
        &mut app,
        "rule spent(S) :- box(rent, R), box(food, F), S is R + F.",
    );
    let rule = app.journal().entries().last().unwrap().seq;
    command(
        &mut app,
        "constraint within income: false :- spent(S), box(income, I), S > I.",
    );
    command(&mut app, "set box(food, 900)");
    let facts = violation_facts(&app, "within income");
    assert!(
        facts.iter().all(|(s, _)| *s != rule),
        "not the rule: {facts:?}"
    );
    for f in ["box(rent, 1200)", "box(food, 900)", "box(income, 2000)"] {
        assert!(facts.iter().any(|(_, x)| x == f), "{f} in {facts:?}");
    }
    shows(&app, "breaks “within income”");
}

/// The facts the one violation of a constraint rests on: each entry, and
/// its fact shown.
fn violation_facts(app: &App, name: &str) -> Vec<(u64, String)> {
    let eval = app.reason.eval[0].as_ref().unwrap();
    let seqs = violation_seqs(eval, name);
    seqs.into_iter().map(|s| (s, fact_shown(eval, s))).collect()
}

/// The entries a constraint's violation rests on, which must be its only
/// one.
fn violation_seqs(eval: &memex::reason::WorldEval, name: &str) -> Vec<u64> {
    let c = eval
        .constraints
        .iter()
        .find(|c| c.constraint.name == name)
        .unwrap();
    assert_eq!(c.violations.len(), 1);
    c.violations[0].prov.iter().map(|p| p.seq()).collect()
}

/// The fact an entry states in a world, shown.
fn fact_shown(eval: &memex::reason::WorldEval, seq: u64) -> String {
    memex::reason::show(eval.fact(seq).unwrap())
}
