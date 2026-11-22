//! `show`: an item as a chart, a table or a graph of the world's facts,
//! and behind it the view grammar (docs/concepts.md, decision 6).

mod support;

use support::Room;

/// A budget of two cells, and an item for the view; the cursor on it.
fn budget(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["The rent is 1200.", "The food is 300.", "Budget"]);
    r.app.idle();
    r
}

/// The marks and goals of an item's view's layers.
fn layers(r: &Room, item: u64) -> Vec<(String, String)> {
    let views = &r.outline().views;
    let of = views.get(&item).into_iter().flatten();
    of.map(|l| (l.mark.clone(), l.goal.clone())).collect()
}

#[test]
fn show_as_chart_makes_the_target_a_bar_chart_of_the_cells() {
    // Catches a chart that is not the guide's recipe, or not drawn.
    let mut r = budget("show-chart");
    let view = r.app.cursor();
    r.command("show as chart");
    let bar = ("bar(x(C), y(V))".to_string(), "cell(C, V)".to_string());
    assert_eq!(layers(&r, view), [bar]);
    r.app.evaluate().unwrap();
    let rows = r.lines_at(160, 40);
    let axis = rows.iter().any(|l| l.contains("food") && l.contains("rent"));
    assert!(axis && r.shows("████████"), "a bar for each cell: {rows:#?}");
    assert!(!r.shows("cannot answer"), "{:#?}", r.lines());
}

#[test]
fn show_as_table_and_as_graph_write_their_recipes_layers() {
    let mut r = budget("show-table");
    let view = r.app.cursor();
    r.command("show as table");
    assert_eq!(layers(&r, view).len(), 2);
    assert!(layers(&r, view).iter().all(|(m, _)| m.starts_with("cell(")));
    r.command("new");
    let graph = r.app.cursor();
    r.command("show as graph");
    let marks: Vec<String> = layers(&r, graph).into_iter().map(|(m, _)| m).collect();
    assert!(
        marks[0].starts_with("node(") && marks[1].starts_with("edge("),
        "{marks:?}"
    );
}

#[test]
fn show_with_an_address_makes_that_item_the_view() {
    let mut r = budget("show-address");
    let view = r.app.cursor();
    r.command("show 1 as chart");
    let first = r.outline().in_order()[0];
    assert_eq!(layers(&r, first).len(), 1);
    assert!(layers(&r, view).is_empty());
}

#[test]
fn show_with_a_mark_adds_that_layer_as_layer_did() {
    let mut r = budget("show-mark");
    let view = r.app.cursor();
    r.command("show rule(y(1500))");
    let rule = ("rule(y(1500))".to_string(), "true".to_string());
    assert_eq!(layers(&r, view), [rule]);
}

#[test]
fn show_as_a_kind_it_does_not_know_says_what_it_takes() {
    let mut r = budget("show-unknown");
    let view = r.app.cursor();
    r.command("show as pie");
    let said = &r.app.session.message;
    assert_eq!(
        said,
        "show (as) chart, table or graph, or a mark where a goal"
    );
    assert!(layers(&r, view).is_empty());
}
