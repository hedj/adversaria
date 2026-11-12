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
        .flat_map(|l| eval.nodes.layers.get(&l.seq).cloned().unwrap_or_default())
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
    for (mark, prov) in &eval.nodes.layers[&layer] {
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
    assert_eq!(
        eval.nodes.layers[&l][0].1.len(),
        2,
        "rent and food, not the rule"
    );
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

/// The screen, its text and its click regions, as the terminal would draw it.
fn screen(app: &App) -> (Vec<String>, memex::ui::Hits, ratatui::buffer::Buffer) {
    use ratatui::{Terminal, backend::TestBackend};
    let (w, h) = (160, 60);
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let mut hits = memex::ui::Hits::default();
    t.draw(|f| hits = memex::ui::draw_hits(f, app)).unwrap();
    let buf = t.backend().buffer().clone();
    let lines = (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect();
    (lines, hits, buf)
}

fn at(lines: &[String], text: &str) -> (u16, u16) {
    below(lines, text, 0)
}

/// Where text first appears on or after row `from`.
fn below(lines: &[String], text: &str, from: u16) -> (u16, u16) {
    for (y, l) in lines.iter().enumerate().skip(from as usize) {
        if let Some(b) = l.find(text) {
            return (l[..b].chars().count() as u16, y as u16);
        }
    }
    panic!("no {text:?} on screen:\n{}", lines.join("\n"))
}

#[test]
fn a_view_is_drawn_under_its_item_and_its_marks_lead_to_their_facts() {
    // Catches a view not drawn, drawn away from its item, a click on a mark
    // that goes nowhere or to the wrong item, a mark on a violated fact
    // drawn quietly, and a view missing from a supposition's two worlds.
    let dir = temp("screen");
    let mut app = budget(&dir);
    command(
        &mut app,
        "constraint spending within income: false :- cell(income, I), cell(rent, R), cell(food, F), R + F > I.",
    );
    item(&mut app, "Budget chart");
    command(&mut app, "layer bar(x(C), y(V)) where cell(C, V)");
    command(&mut app, "layer rule(y(I)) where cell(income, I)");
    let (s, hits, _) = screen(&app);
    let (_, title) = at(&s, "Budget chart");
    let (fx, fy) = below(&s, " food ", title + 1);
    assert!(
        fy > title && fy < title + 15,
        "the chart under its item:\n{}",
        s.join("\n")
    );
    at(&s, "VIEW · 2 LAYERS");
    at(&s, "layer bar(x(C), y(V)) where cell(C, V)");
    // Click on the food bar: the column above its label.
    let bar_y = (title + 1..fy)
        .rev()
        .find(|y| s[*y as usize].chars().nth(fx as usize + 2) == Some('█'))
        .unwrap();
    let c = hits
        .at(fx + 2, bar_y)
        .cloned()
        .expect("the bar is clickable");
    app.click(&c);
    assert_eq!(
        app.outline.text(app.cursor()),
        "food",
        "the item stating cell(food, 400)"
    );
    // Break the rule: the bars on the facts in the violation are drawn bold.
    command(&mut app, "set cell(food, 900)");
    let (s, _, buf) = screen(&app);
    let (_, title) = at(&s, "Budget chart");
    let (fx, fy) = below(&s, " food ", title + 1);
    let y = (0..fy)
        .rev()
        .find(|y| s[*y as usize].chars().nth(fx as usize + 2) == Some('█'))
        .unwrap();
    assert!(
        buf[(fx + 2, y)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD),
        "loud bar"
    );
    // A supposition: the view in both worlds, each from its own facts.
    command(&mut app, "set cell(food, 500)");
    app.handle(Event::down(Key::Quasi(Quasi::Suppose)));
    typ(&mut app, "rent is 1600");
    app.handle(Event::up(Key::Quasi(Quasi::Suppose)));
    tap(&mut app, Key::Enter);
    app.evaluate().unwrap();
    let (s, _, _) = screen(&app);
    let both = s
        .iter()
        .filter(|l| l.matches("Budget chart").count() == 2)
        .count()
        + s.iter().filter(|l| l.contains("Budget chart")).count();
    assert!(both >= 2, "the view in each world:\n{}", s.join("\n"));
    assert!(
        s.iter().any(|l| l.contains("1200")) && s.iter().any(|l| l.contains("1600")),
        "rent before and after:\n{}",
        s.join("\n")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The lines of the view at the item with this text, as the outline draws
/// it on a 160-column screen (the outline pane 95 wide, less the indent),
/// then its notes.
fn figure(app: &App, title: &str) -> Vec<String> {
    let id = find(app, title);
    let depth = {
        let mut d = 0;
        let mut p = app.outline.parent(id);
        while let Some(x) = p.filter(|x| app.outline.items.contains_key(x)) {
            d += 1;
            p = app.outline.parent(x);
        }
        d
    };
    let f = memex::ui::view_figure(app, 0, id, 95 - 2 * depth - 3, 12);
    let mut out = f.text();
    out.extend(f.notes);
    out
}

#[test]
fn gallery_a_spreadsheet_with_a_computed_box() {
    // Boxes at grid places, one computed by a rule: a spreadsheet. Catches
    // a grid that loses a box, a computed box that does not follow its
    // inputs, or a broken rule's box drawn quietly.
    let dir = temp("gallery-sheet");
    let mut app = budget(&dir);
    for (b, c, r) in [
        ("income", "b", 1),
        ("rent", "b", 2),
        ("food", "b", 3),
        ("left", "b", 4),
    ] {
        command(&mut app, &format!("fact at({b}, {c}, {r})"));
    }
    command(&mut app, "rule value(B, V) :- cell(B, V).");
    command(
        &mut app,
        "rule value(left, L) :- cell(income, I), cell(rent, R), cell(food, F), L is I - R - F.",
    );
    command(
        &mut app,
        "constraint not overspent: false :- value(left, L), L < 0.",
    );
    item(&mut app, "Sheet");
    command(
        &mut app,
        "layer cell(x(a), y(R), text(B)) where at(B, _, R)",
    );
    command(
        &mut app,
        "layer cell(x(C), y(R), text(V)) where at(B, C, R), value(B, V)",
    );
    let f = figure(&app, "Sheet");
    assert_eq!(
        &f[..6],
        [
            "  │ a      │ b",
            "──┼────────┼─────",
            "1 │ income │ 2000",
            "2 │ rent   │ 1200",
            "3 │ food   │  400",
            "4 │ left   │  400"
        ],
        "{f:#?}"
    );
    command(&mut app, "set cell(food, 900)");
    let f = figure(&app, "Sheet");
    assert!(
        f.iter().any(|l| l.contains("left") && l.contains("-100!")),
        "computed, and loud:\n{}",
        f.join("\n")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn gallery_a_line_chart_a_timeline_and_a_gantt_chart() {
    // Catches a series out of order, a timeline label lost or misplaced, or
    // a task's span on the wrong lane.
    let dir = temp("gallery-time");
    let mut app = open(&dir);
    typ(&mut app, "Data");
    for (m, v) in [(1, 300), (2, 450), (3, 380), (4, 520)] {
        command(&mut app, &format!("fact spend({m}, {v})"));
    }
    for (e, y, l) in [
        ("energia", 1987, "Energia"),
        ("buran", 1988, "Buran flight"),
        ("end", 1993, "Programme ends"),
    ] {
        command(&mut app, &format!("fact event({e}, {y}, \"{l}\")"));
    }
    for (t, a, b) in [("design", 1, 4), ("build", 3, 9), ("test", 8, 10)] {
        command(&mut app, &format!("fact task({t}, {a}, {b})"));
    }
    item(&mut app, "Spending");
    command(&mut app, "layer line(x(M), y(V)) where spend(M, V)");
    let f = figure(&app, "Spending");
    assert!(
        f.iter().any(|l| l.contains("520")) && f.iter().any(|l| l.contains("300")),
        "{}",
        f.join("\n")
    );
    assert_eq!(
        f.iter().map(|l| l.matches('●').count()).sum::<usize>(),
        4,
        "{}",
        f.join("\n")
    );
    item(&mut app, "Timeline");
    command(&mut app, "layer tick(x(Y), text(L)) where event(_, Y, L)");
    let f = figure(&app, "Timeline");
    let x = |t: &str| {
        f.iter()
            .find_map(|l| l.find(t))
            .unwrap_or_else(|| panic!("{t}:\n{}", f.join("\n")))
    };
    assert!(x("Energia") < x("Buran flight") && x("Buran flight") < x("Programme ends"));
    item(&mut app, "Plan");
    command(
        &mut app,
        "layer span(y(T), x1(A), x2(B), text(T)) where task(T, A, B)",
    );
    let f = figure(&app, "Plan");
    let row = |t: &str| {
        f.iter()
            .position(|l| l.trim_start().starts_with(t))
            .unwrap_or_else(|| panic!("{t}:\n{}", f.join("\n")))
    };
    assert!(
        row("build") < row("design") && row("design") < row("test"),
        "{}",
        f.join("\n")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn gallery_a_flowchart_from_steps_and_choices() {
    // Steps, decisions and transitions as facts; nodes and edges as layers.
    // Catches a decision drawn as a plain step, a choice's label lost, or
    // a step added by a fact not appearing.
    let dir = temp("gallery-flow");
    let mut app = open(&dir);
    typ(&mut app, "Process");
    command(&mut app, "fact step(start, \"Start\")");
    command(&mut app, "fact decision(late, \"Is it late?\")");
    command(&mut app, "fact step(home, \"Go home\")");
    command(&mut app, "fact step(work, \"Keep working\")");
    command(&mut app, "fact next(start, late, \"\")");
    command(&mut app, "fact next(late, home, \"yes\")");
    command(&mut app, "fact next(late, work, \"no\")");
    command(&mut app, "fact next(work, late, \"\")");
    item(&mut app, "Flow");
    command(&mut app, "layer node(id(S), text(T)) where step(S, T)");
    command(
        &mut app,
        "layer node(id(S), text(T), shape(decision)) where decision(S, T)",
    );
    command(
        &mut app,
        "layer edge(from(A), to(B), text(L)) where next(A, B, L)",
    );
    let f = figure(&app, "Flow");
    let s = f.join("\n");
    assert!(s.contains("< Is it late? >"), "{s}");
    assert!(s.contains("▼ yes") && s.contains("▼ no"), "{s}");
    assert!(s.contains('◀'), "the loop back: {s}");
    command(&mut app, "fact step(sleep, \"Sleep\")");
    command(&mut app, "fact next(home, sleep, \"\")");
    let s = figure(&app, "Flow").join("\n");
    assert!(s.contains("Sleep"), "{s}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn gallery_the_outline_itself_as_a_tree() {
    // The outline is offered to goals as item_text/2 and item_under/2.
    // Catches a tree missing an item, a child drawn above its parent, or a
    // click on a box that does not go to its item.
    let dir = temp("gallery-tree");
    let mut app = open(&dir);
    typ(&mut app, "Plan");
    item(&mut app, "Research");
    tap(&mut app, Key::Tab);
    item(&mut app, "Read papers");
    tap(&mut app, Key::Tab);
    item(&mut app, "Interview");
    item(&mut app, "Write");
    tap(&mut app, Key::BackTab);
    command(&mut app, "fact draft(1)");
    item(&mut app, "Tree");
    tap(&mut app, Key::BackTab);
    command(
        &mut app,
        "layer node(id(I), text(T)) where item_text(I, T), \\+ item_text(I, \"Tree\")",
    );
    command(
        &mut app,
        "layer edge(from(P), to(C)) where item_under(C, P)",
    );
    let f = figure(&app, "Tree");
    let row = |t: &str| {
        f.iter()
            .position(|l| l.contains(t))
            .unwrap_or_else(|| panic!("{t}:\n{}", f.join("\n")))
    };
    assert!(
        row("│ Plan │") < row("Research") && row("Research") < row("Read papers"),
        "{}",
        f.join("\n")
    );
    assert_eq!(row("Read papers"), row("Interview"));
    assert_eq!(row("Research"), row("Write"));
    // Click the Interview box: the cursor goes to that item.
    let (s, hits, _) = screen(&app);
    let (x, y) = below(&s, "│ Interview │", 0);
    let x = x + 2;
    app.click(&hits.at(x, y).cloned().expect("clickable"));
    assert_eq!(app.outline.text(app.cursor()), "Interview");
    std::fs::remove_dir_all(&dir).unwrap();
}
