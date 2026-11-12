//! Views: layers of marks over goals (crate::view). These tests drive the
//! memex as a user would and check the marks a view computes, then what it
//! draws.

use format::terms::Term;
use memex::app::{App, Event, Key, Quasi};
use std::path::{Path, PathBuf};

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("adversaria-views-{name}-{}", std::process::id()));
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

/// Run a command; return the message it left.
fn run(app: &mut App, line: &str) -> String {
    app.handle(Event::down(Key::Quasi(Quasi::Command)));
    typ(app, line);
    app.handle(Event::up(Key::Quasi(Quasi::Command)));
    app.evaluate().unwrap();
    app.message.clone()
}

/// Run a command that must succeed.
fn command(app: &mut App, line: &str) -> String {
    let m = run(app, line);
    assert!(
        !m.starts_with("no command")
            && !m.starts_with("not a")
            && !m.contains("not kept")
            && !m.contains("not allowed"),
        "{line}: {m}"
    );
    m
}

fn item(app: &mut App, text: &str) {
    tap(app, Key::Enter);
    typ(app, text);
    app.flush();
}

fn open(dir: &Path) -> App {
    let mut app = App::open(dir).unwrap();
    app.set_durable(false);
    app
}

/// Three values, as the budget in the tutorial's part 2.
fn budget(dir: &Path) -> App {
    let mut app = open(dir);
    typ(&mut app, "Budget");
    for (name, v) in [("income", 2000), ("rent", 1200), ("food", 400)] {
        item(&mut app, name);
        command(&mut app, &format!("fact cell({name}, {v})"));
    }
    command(
        &mut app,
        "constraint one value per cell: false :- cell(C, A), cell(C, B), A \\= B.",
    );
    app
}

/// The marks of the view at an item, as text, in order.
fn marks(app: &App, item: u64) -> Vec<String> {
    let eval = app.eval[0].as_ref().unwrap();
    app.outline.views[&item]
        .iter()
        .flat_map(|l| eval.layers.get(&l.seq).cloned().unwrap_or_default())
        .map(|(m, _)| memex::reason::show(&m))
        .collect()
}

fn find(app: &App, text: &str) -> u64 {
    app.outline
        .in_order()
        .into_iter()
        .find(|i| app.outline.text(*i) == text)
        .unwrap()
}

#[test]
fn a_layer_gives_one_mark_per_solution_and_follows_the_facts() {
    // Catches a layer that misses solutions, loses provenance, or does not
    // change when a fact it rests on changes.
    let dir = temp("layer");
    let mut app = budget(&dir);
    item(&mut app, "chart");
    let m = command(&mut app, "layer bar(x(C), y(V)) where cell(C, V)");
    assert!(m.starts_with("layer kept"), "{m}");
    let chart = find(&app, "chart");
    assert_eq!(
        marks(&app, chart),
        [
            "bar(x(food), y(400))",
            "bar(x(income), y(2000))",
            "bar(x(rent), y(1200))"
        ]
    );
    // Each mark rests on the fact it shows.
    let eval = app.eval[0].as_ref().unwrap();
    let layer = app.outline.views[&chart][0].seq;
    for (mark, prov) in &eval.layers[&layer] {
        assert_eq!(prov.len(), 1, "{mark:?}");
        let fact = &eval.facts.iter().find(|(s, _)| *s == prov[0]).unwrap().1;
        let Term::Compound(_, a) = mark else { panic!() };
        let Term::Compound(_, x) = &a[0] else {
            panic!()
        };
        assert_eq!(
            fact,
            &Term::Compound(
                "cell".into(),
                vec![
                    x[0].clone(),
                    match &a[1] {
                        Term::Compound(_, y) => y[0].clone(),
                        _ => panic!(),
                    }
                ]
            )
        );
    }
    // A second layer, constant: a reference line.
    command(&mut app, "layer rule(y(1500))");
    assert_eq!(marks(&app, chart).last().unwrap(), "rule(y(1500))");
    // Change a fact: the layer follows.
    command(&mut app, "set cell(food, 900)");
    assert!(marks(&app, chart).contains(&"bar(x(food), y(900))".to_string()));
    assert!(!marks(&app, chart).contains(&"bar(x(food), y(400))".to_string()));
    // A layer over a rule rests on the facts under the rule.
    command(
        &mut app,
        "rule spent(S) :- cell(rent, R), cell(food, F), S is R + F.",
    );
    item(&mut app, "spent");
    command(&mut app, "layer text(x(1), y(1), text(S)) where spent(S)");
    let spent = find(&app, "spent");
    assert_eq!(marks(&app, spent), ["text(x(1), y(1), text(2100))"]);
    let eval = app.eval[0].as_ref().unwrap();
    let l = app.outline.views[&spent][0].seq;
    assert_eq!(eval.layers[&l][0].1.len(), 2, "rent and food, not the rule");
    // Retracting a layer removes it.
    run(&mut app, &format!("retract {l}"));
    tap(&mut app, Key::Enter);
    app.evaluate().unwrap();
    assert!(!app.outline.views.contains_key(&spent));
    // Replay gives the same marks.
    let before = marks(&app, chart);
    drop(app);
    let mut again = App::open(&dir).unwrap();
    again.evaluate().unwrap();
    assert_eq!(marks(&again, chart), before);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_layer_that_could_not_be_drawn_or_would_act_is_refused() {
    // Catches a layer kept with an unknown mark or channel, a variable the
    // goal never binds, or a goal that calls what definitions may not.
    let dir = temp("refuse");
    let mut app = budget(&dir);
    item(&mut app, "chart");
    for (line, why) in [
        ("layer pie(x(C)) where cell(C, _)", "no mark “pie”"),
        ("layer bar(x(C)) where cell(C, _)", "bar needs y(…)"),
        (
            "layer bar(x(C), y(V), colour(red)) where cell(C, V)",
            "no channel colour",
        ),
        (
            "layer bar(x(C), y(W)) where cell(C, V)",
            "W in the mark is not bound",
        ),
        (
            "layer bar(x(C), y(V)) where cell(C, V), shell(ls)",
            "shell/1 is not allowed",
        ),
    ] {
        let m = run(&mut app, line);
        assert!(
            m.contains("layer not kept") && m.contains(why),
            "{line}: {m}"
        );
    }
    assert!(!app.outline.views.contains_key(&find(&app, "chart")));
    std::fs::remove_dir_all(&dir).unwrap();
}
