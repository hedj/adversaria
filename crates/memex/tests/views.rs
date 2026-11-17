//! Views: layers of marks over goals (crate::view). These tests drive the
//! memex as a user would and check the marks a view computes, then what it
//! draws.

use format::terms::Term;
use memex::app::{App, Event, Key, Quasi};
use std::path::PathBuf;

mod support;
use support::{Room, replayed};

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
    app.session.message.clone()
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

fn open(name: &str) -> Room {
    let mut app = Room::new(&format!("views-{name}"));
    app.set_durable(false);
    app
}

/// Three values, as the budget in the tutorial's part 2.
fn budget(name: &str) -> Room {
    let mut app = open(name);
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
    let eval = app.reason.eval[0].as_ref().unwrap();
    let layers = &app.notebook.outline.views[&item];
    layers
        .iter()
        .flat_map(|l| layer_marks(eval, l.seq))
        .collect()
}

/// The marks one layer gives in a world, as text, in order.
fn layer_marks(eval: &memex::reason::WorldEval, seq: u64) -> Vec<String> {
    let Some(marks) = eval.nodes.layers.get(&seq) else {
        return Vec::new();
    };
    marks.iter().map(|(m, _)| memex::reason::show(m)).collect()
}

fn find(app: &App, text: &str) -> u64 {
    let outline = &app.notebook.outline;
    let mut items = outline.in_order().into_iter();
    items.find(|i| outline.text(*i) == text).unwrap()
}

#[test]
fn a_layer_gives_one_mark_per_solution_and_follows_the_facts() {
    // Catches a layer that misses solutions, loses provenance, or does not
    // change when a fact it rests on changes.
    let mut app = budget("layer");
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
    each_mark_rests_on_the_fact_it_shows(&app, chart);
    // A second layer, constant: a reference line.
    command(&mut app, "layer rule(y(1500))");
    assert_eq!(marks(&app, chart).last().unwrap(), "rule(y(1500))");
    // Change a fact: the layer follows.
    command(&mut app, "set cell(food, 900)");
    assert!(marks(&app, chart).contains(&"bar(x(food), y(900))".to_string()));
    assert!(!marks(&app, chart).contains(&"bar(x(food), y(400))".to_string()));
    a_layer_over_a_rule_rests_on_the_facts_under_it(&mut app);
    // Replay gives the same marks.
    let before = marks(&app, chart);
    let mut again = replayed(&app.dir);
    again.evaluate().unwrap();
    assert_eq!(marks(&again, chart), before);
}

/// Each mark of the item's first layer rests on one fact: the one it shows.
fn each_mark_rests_on_the_fact_it_shows(app: &App, item: u64) {
    let layer = first_layer(app, item);
    for (mark, prov) in solutions(app, layer) {
        assert_eq!(prov.len(), 1, "{mark:?}");
        assert_eq!(stated(app, prov[0]), Some(&shown_fact(mark)), "{mark:?}");
    }
}

/// The entry of the first layer of the view at an item.
fn first_layer(app: &App, item: u64) -> u64 {
    app.notebook.outline.views[&item][0].seq
}

/// A layer's solutions in the actual world: each mark, and the entries of
/// the facts it rests on.
fn solutions(app: &App, layer: u64) -> &[(Term, Vec<u64>)] {
    let eval = app.reason.eval[0].as_ref().unwrap();
    &eval.nodes.layers[&layer]
}

/// The fact an entry states in the actual world.
fn stated(app: &App, seq: u64) -> Option<&Term> {
    app.reason.eval[0].as_ref().unwrap().fact(seq)
}

/// The fact a bar shows: bar(x(C), y(V)) shows cell(C, V).
fn shown_fact(bar: &Term) -> Term {
    let x = arg(&arg(bar, 0), 0);
    let y = arg(&arg(bar, 1), 0);
    Term::Compound("cell".into(), vec![x, y])
}

/// A compound term's argument at this place.
fn arg(t: &Term, i: usize) -> Term {
    let Term::Compound(_, args) = t else {
        panic!("not a compound: {t:?}")
    };
    args[i].clone()
}

/// A layer over a rule rests on the facts under the rule, not the rule;
/// retracting the layer removes the view.
fn a_layer_over_a_rule_rests_on_the_facts_under_it(app: &mut Room) {
    command(
        app,
        "rule spent(S) :- cell(rent, R), cell(food, F), S is R + F.",
    );
    item(app, "spent");
    command(app, "layer text(x(1), y(1), text(S)) where spent(S)");
    let spent = find(app, "spent");
    assert_eq!(marks(app, spent), ["text(x(1), y(1), text(2100))"]);
    let l = first_layer(app, spent);
    let rests_on = &solutions(app, l)[0].1;
    assert_eq!(rests_on.len(), 2, "rent and food, not the rule");
    run(app, &format!("retract {l}"));
    tap(app, Key::Enter);
    app.evaluate().unwrap();
    assert!(!app.notebook.outline.views.contains_key(&spent));
}

#[test]
fn a_layer_that_could_not_be_drawn_or_would_act_is_refused() {
    // Catches a layer kept with an unknown mark or channel, a variable the
    // goal never binds, or a goal that calls what definitions may not.
    let mut app = budget("refuse");
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
    assert!(
        !app.notebook
            .outline
            .views
            .contains_key(&find(&app, "chart"))
    );
}

/// The screen, its text and its click regions, as the terminal would draw it.
fn screen(app: &App) -> (Vec<String>, memex::ui::Hits, ratatui::buffer::Buffer) {
    use ratatui::{Terminal, backend::TestBackend};
    let mut t = Terminal::new(TestBackend::new(160, 60)).unwrap();
    let mut hits = memex::ui::Hits::default();
    t.draw(|f| hits = memex::ui::draw_hits(f, app)).unwrap();
    let buf = t.backend().buffer().clone();
    (rows_of(&buf), hits, buf)
}

/// Each row of a screen, as text, without trailing spaces.
fn rows_of(buf: &ratatui::buffer::Buffer) -> Vec<String> {
    let area = buf.area;
    let row = |y| {
        (0..area.width)
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
    };
    (0..area.height)
        .map(|y| row(y).trim_end().to_string())
        .collect()
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
    let mut app = budget("screen");
    command(
        &mut app,
        "constraint spending within income: false :- cell(income, I), cell(rent, R), cell(food, F), R + F > I.",
    );
    item(&mut app, "Budget chart");
    command(&mut app, "layer bar(x(C), y(V)) where cell(C, V)");
    command(&mut app, "layer rule(y(I)) where cell(income, I)");
    the_chart_is_drawn_under_its_item(&app);
    the_food_bar_leads_to_its_fact(&mut app);
    // Break the rule: the bars on the facts in the violation are drawn bold.
    command(&mut app, "set cell(food, 900)");
    let (s, _, buf) = screen(&app);
    let bar = food_bar(&s);
    let loud = buf[bar].modifier.contains(ratatui::style::Modifier::BOLD);
    assert!(loud, "loud bar");
    a_supposition_draws_the_view_in_each_world(&mut app);
}

/// The chart is drawn under its item, with its panel.
fn the_chart_is_drawn_under_its_item(app: &App) {
    let (s, _, _) = screen(app);
    let (_, title) = at(&s, "Budget chart");
    let label = food_label(&s);
    let under = label.1 > title && label.1 < title + 15;
    assert!(under, "the chart under its item:\n{}", s.join("\n"));
    at(&s, "VIEW · 2 LAYERS");
    at(&s, "layer bar(x(C), y(V)) where cell(C, V)");
}

/// A click on the food bar goes to the item stating cell(food, 400).
fn the_food_bar_leads_to_its_fact(app: &mut Room) {
    let (s, hits, _) = screen(app);
    let (x, y) = food_bar(&s);
    let c = hits.at(x, y).cloned().expect("the bar is clickable");
    app.click(&c);
    let text = app.notebook.outline.text(app.cursor());
    assert_eq!(text, "food", "the item stating cell(food, 400)");
}

/// Where the label of the food bar is: the first " food " under the
/// chart's item.
fn food_label(s: &[String]) -> (u16, u16) {
    let (_, title) = at(s, "Budget chart");
    below(s, " food ", title + 1)
}

/// A cell of the food bar: the nearest bar cell above its label, in the
/// column the bar is drawn in.
fn food_bar(s: &[String]) -> (u16, u16) {
    let (x, y) = food_label(s);
    let column = x as usize + 2;
    let filled = |row: &u16| s[*row as usize].chars().nth(column) == Some('█');
    let row = (0..y).rev().find(filled).expect("a bar above its label");
    (x + 2, row)
}

/// A supposition: the view in both worlds, each from its own facts.
fn a_supposition_draws_the_view_in_each_world(app: &mut Room) {
    command(app, "set cell(food, 500)");
    app.handle(Event::down(Key::Quasi(Quasi::Suppose)));
    typ(app, "rent is 1600");
    app.handle(Event::up(Key::Quasi(Quasi::Suppose)));
    tap(app, Key::Enter);
    app.evaluate().unwrap();
    let (s, _, _) = screen(app);
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
}

/// The lines of the view at the item with this text, as the outline draws
/// it on a 160-column screen (the outline pane 95 wide, less the indent),
/// then its notes.
fn figure(app: &App, title: &str) -> Vec<String> {
    let id = find(app, title);
    let depth = app.notebook.outline.depth_of(id);
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
    let mut app = budget("gallery-sheet");
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
}

#[test]
fn gallery_a_line_chart_a_timeline_and_a_gantt_chart() {
    // Catches a series out of order, a timeline label lost or misplaced, or
    // a task's span on the wrong lane.
    let mut app = open("gallery-time");
    typ(&mut app, "Data");
    state_times(&mut app);
    a_line_chart_draws_each_value(&mut app);
    a_timeline_orders_its_labels(&mut app);
    a_gantt_chart_puts_each_task_on_its_lane(&mut app);
}

/// Spending by month, events by year, and tasks by their first and last
/// weeks.
fn state_times(app: &mut Room) {
    for (m, v) in [(1, 300), (2, 450), (3, 380), (4, 520)] {
        command(app, &format!("fact spend({m}, {v})"));
    }
    for (e, y, l) in [
        ("energia", 1987, "Energia"),
        ("buran", 1988, "Buran flight"),
        ("end", 1993, "Programme ends"),
    ] {
        command(app, &format!("fact event({e}, {y}, \"{l}\")"));
    }
    for (t, a, b) in [("design", 1, 4), ("build", 3, 9), ("test", 8, 10)] {
        command(app, &format!("fact task({t}, {a}, {b})"));
    }
}

/// A line through the spending: its range on the axis, and one point a
/// month.
fn a_line_chart_draws_each_value(app: &mut Room) {
    item(app, "Spending");
    command(app, "layer line(x(M), y(V)) where spend(M, V)");
    let f = figure(app, "Spending");
    let range = f.iter().any(|l| l.contains("520")) && f.iter().any(|l| l.contains("300"));
    assert!(range, "{}", f.join("\n"));
    let points: usize = f.iter().map(|l| l.matches('●').count()).sum();
    assert_eq!(points, 4, "{}", f.join("\n"));
}

/// The events on a timeline, their labels left to right by year.
fn a_timeline_orders_its_labels(app: &mut Room) {
    item(app, "Timeline");
    command(app, "layer tick(x(Y), text(L)) where event(_, Y, L)");
    let f = figure(app, "Timeline");
    let (e, b, p) = (
        column_of(&f, "Energia"),
        column_of(&f, "Buran flight"),
        column_of(&f, "Programme ends"),
    );
    assert!(e < b && b < p, "{}", f.join("\n"));
}

/// The tasks on a Gantt chart, each on its own lane, the lanes in order of
/// name.
fn a_gantt_chart_puts_each_task_on_its_lane(app: &mut Room) {
    item(app, "Plan");
    command(
        app,
        "layer span(y(T), x1(A), x2(B), text(T)) where task(T, A, B)",
    );
    let f = figure(app, "Plan");
    let (b, d, t) = (
        row_of(&f, "build"),
        row_of(&f, "design"),
        row_of(&f, "test"),
    );
    assert!(b < d && d < t, "{}", f.join("\n"));
}

/// Where text first starts in a figure's lines, in bytes from the left.
fn column_of(f: &[String], text: &str) -> usize {
    let found = f.iter().find_map(|l| l.find(text));
    found.unwrap_or_else(|| panic!("{text}:\n{}", f.join("\n")))
}

/// The line of a figure that starts with text, after its indent.
fn row_of(f: &[String], text: &str) -> usize {
    let found = f.iter().position(|l| l.trim_start().starts_with(text));
    found.unwrap_or_else(|| panic!("{text}:\n{}", f.join("\n")))
}

#[test]
fn gallery_a_flowchart_from_steps_and_choices() {
    // Steps, decisions and transitions as facts; nodes and edges as layers.
    // Catches a decision drawn as a plain step, a choice's label lost, or
    // a step added by a fact not appearing.
    let mut app = open("gallery-flow");
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
}

#[test]
fn gallery_the_outline_itself_as_a_tree() {
    // The outline is offered to goals as item_text/2 and item_under/2.
    // Catches a tree missing an item, a child drawn above its parent, or a
    // click on a box that does not go to its item.
    let mut app = open("gallery-tree");
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
    assert_eq!(app.notebook.outline.text(app.cursor()), "Interview");
}

#[test]
fn the_room_is_named_after_its_journal_directory() {
    // Catches a top bar that names no room, or names another.
    let dir = temp("room");
    let app = App::open(&dir).unwrap();
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    let top = screen(&app).0.remove(0);
    assert!(top.contains(&format!("room {name}")), "{top}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A room with an item for each cell, stated as a fact, then a chart of
/// them all, evaluated.
fn chart(name: &str, cells: &[&str]) -> Room {
    let mut room = Room::new(name);
    room.items(&["Budget"]);
    for cell in cells {
        room.items(&["", cell]);
        room.command(&format!("fact {cell}"));
    }
    room.items(&["", "Chart"]);
    room.command("layer bar(x(C), y(V)) where cell(C, V)");
    room.app.evaluate().unwrap();
    room
}

#[test]
fn the_view_panel_counts_a_layers_marks_by_its_entry() {
    let room = chart("panel-marks", &["cell(rent, 1200)", "cell(food, 400)"]);
    let layer = room.last("edit");
    assert!(room.shows(&format!("entry {layer} · 2 marks")));
}

#[test]
fn the_view_panel_counts_one_mark_as_one() {
    let room = chart("panel-mark", &["cell(rent, 1200)"]);
    let layer = room.last("edit");
    assert!(room.shows(&format!("entry {layer} · 1 mark ")));
}

#[test]
fn the_view_panel_shows_the_views_notes_and_how_to_follow_a_mark() {
    let room = chart("panel-notes", &[]);
    let notes = room.lines_at(160, 40).concat();
    assert_eq!(notes.matches("(no marks: no solutions yet)").count(), 2);
    assert!(room.shows("a click on a mark goes to the item that states its fact"));
}

/// The view at the cursor, the room's chart, in world k, at a width.
fn chart_figure(room: &Room, k: usize, width: usize) -> memex::view::Figure {
    memex::ui::view_figure(&room.app, k, room.app.cursor(), width, 12)
}

/// Whether a figure draws any mark loudly.
fn loud(fig: &memex::view::Figure) -> bool {
    fig.rows
        .concat()
        .iter()
        .any(|(_, ink)| *ink == memex::view::Ink::Loud)
}

#[test]
fn an_item_without_a_view_draws_nothing() {
    let room = chart("no-view", &["cell(rent, 1200)"]);
    let budget = room.app.notebook.outline.in_order()[0];
    let fig = memex::ui::view_figure(&room.app, 0, budget, 60, 12);
    assert!(fig.rows.is_empty() && fig.notes.is_empty());
}

#[test]
fn a_view_is_loud_only_in_the_world_whose_facts_are_violated() {
    let mut room = chart("loud-world", &["cell(income, 2000)", "cell(food, 400)"]);
    room.command("constraint within income: false :- cell(income, I), cell(food, F), F > I.");
    room.command("suppose that famine");
    room.command("fact cell(food, 3000)");
    room.app.evaluate().unwrap();
    assert!(loud(&chart_figure(&room, 0, 60)), "the supposed world");
    assert!(!loud(&chart_figure(&room, 1, 60)), "the actual world");
}

#[test]
fn a_view_is_drawn_in_the_width_it_is_given() {
    let room = chart("view-width", &["cell(rent, 1200)", "cell(food, 400)"]);
    let fig = chart_figure(&room, 0, 24);
    assert!(fig.rows.iter().all(|row| row.len() <= 24));
}

#[test]
fn categories_drawn_over_others_are_named_under_the_view() {
    // Catches more categories than columns: some bars drawn over others,
    // with no note, though marks that do not fit are named (§ 3.3).
    let cells: Vec<String> = (1..=12).map(|k| format!("cell(c{k:02}, {k})")).collect();
    let cells: Vec<&str> = cells.iter().map(String::as_str).collect();
    let room = chart("crowded", &cells);
    let notes = chart_figure(&room, 0, 12).notes;
    assert!(
        notes.iter().any(|n| n.starts_with("drawn over others")),
        "{notes:?}"
    );
    assert!(
        chart_figure(&room, 0, 60).notes.is_empty(),
        "room for all at 60"
    );
}

#[test]
fn boxes_of_a_graph_cut_off_for_want_of_room_are_named() {
    // Catches a graph drawn wider than its view: the pane cut its right
    // side off, boxes and all, with no note (§ 3.3).
    let mut room = Room::new("graph-cut");
    room.items(&["Steps"]);
    for n in ["alpha", "beta", "gamma", "delta"] {
        room.command(&format!("fact n({n})"));
    }
    room.command("fact e(alpha, beta)");
    room.items(&["", "Graph"]);
    room.command("layer node(id(N), text(N)) where n(N)");
    room.command("layer edge(from(A), to(B)) where e(A, B)");
    room.app.evaluate().unwrap();
    let notes = chart_figure(&room, 0, 14).notes;
    assert!(notes.iter().any(|n| n.starts_with("cut off")), "{notes:?}");
    assert!(
        chart_figure(&room, 0, 80).notes.is_empty(),
        "room for all at 80"
    );
}

#[test]
fn columns_of_a_grid_cut_off_for_want_of_room_are_named() {
    // Catches a grid wider than its view: the columns past its width were
    // left out, with no note (§ 3.3).
    let mut room = Room::new("grid-cut");
    room.items(&["Sheet"]);
    for k in 1..=12 {
        room.command(&format!("fact g(c{k:02}, 1, v{k})"));
    }
    room.items(&["", "Grid"]);
    room.command("layer cell(x(X), y(Y), text(T)) where g(X, Y, T)");
    room.app.evaluate().unwrap();
    let notes = chart_figure(&room, 0, 20).notes;
    assert!(
        notes.iter().any(|n| n.contains("c04") && n.contains("c12")),
        "{notes:?}"
    );
    assert!(
        chart_figure(&room, 0, 200).notes.is_empty(),
        "room for all at 200"
    );
}

#[test]
fn a_layer_whose_goal_raises_an_error_says_so() {
    // Catches the error dropped: the view said "(no marks: no solutions
    // yet)" of a goal that has facts to read.
    let mut r = Room::new("layer-error");
    r.items(&["rent", "chart"]);
    r.command("layer bar(x(C), y(V)) where cell(C, R), V is R + foo");
    let layer = r.last("edit");
    r.leap(Quasi::LeapBack, "rent");
    r.command("fact cell(rent, 1200)");
    r.app.flush();
    r.app.evaluate().unwrap();
    let why = "cannot answer: type_error(evaluable, foo / 0)";
    assert!(r.shows(&format!("(layer {layer} {why})")));
    assert!(!r.shows("no solutions yet"));
}

#[test]
fn a_layer_that_cannot_answer_is_noted_under_the_marks_of_the_others() {
    // Catches the note given only to a view with nothing else to draw.
    let mut r = Room::new("layer-error-beside");
    r.items(&["rent", "chart"]);
    r.command("layer bar(x(C), y(V)) where cell(C, V)");
    r.command("layer bar(x(C), y(V)) where cell(C, R), V is R + foo");
    r.leap(Quasi::LeapBack, "rent");
    r.command("fact cell(rent, 1200)");
    r.app.flush();
    r.app.evaluate().unwrap();
    assert!(r.shows("1200 │"));
    assert!(r.shows("cannot answer: type_error(evaluable, foo / 0))"));
}
