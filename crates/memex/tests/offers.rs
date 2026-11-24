//! One row for an offer (docs/concepts-seven.md, change 2): the
//! notebook's inspector, the suggestions list, a claim's questions and a
//! revision's changes draw an offer in the same two rows.

mod support;

use ratatui::style::Modifier;
use support::Room;

/// The inspector's lines, at 160 columns: what is right of the rule.
fn inspector(r: &Room) -> Vec<String> {
    let lines = r.lines_at(160, 40);
    let right = |l: &String| l.split_once('│').map(|(_, r)| r.trim_end().to_string());
    lines.iter().filter_map(right).collect()
}

/// An offer's rows in the inspector, wrapped rows too: from the row
/// that starts so to the first after it that says where it came from.
fn rows(r: &Room, start: &str) -> Vec<String> {
    let shown = inspector(r);
    let at = shown.iter().position(|l| l.trim_start().starts_with(start));
    let at = at.unwrap_or_else(|| panic!("{start}: {shown:#?}"));
    let from = shown[at..]
        .iter()
        .position(|l| l.trim_start().starts_with("from "));
    let from = from.unwrap_or_else(|| panic!("no source: {shown:#?}"));
    shown[at..=at + from].to_vec()
}

#[test]
fn a_suggestion_takes_the_same_rows_in_the_notebook_and_in_its_list() {
    // Catches a list that draws an offer its own way.
    let mut r = Room::new("offer-rows");
    r.items(&["Buran flight", "Notes on the buran flight launch"]);
    r.app.idle();
    r.app.evaluate().unwrap();
    r.command("go parent");
    let in_notebook = rows(&r, "suggestion 1  ");
    r.command("go 1");
    r.command("find suggestions");
    assert_eq!(rows(&r, "suggestion 1  "), in_notebook);
    let from = in_notebook.last().unwrap().trim_start();
    assert!(from.starts_with("from the memex: "), "{in_notebook:?}");
}

#[test]
fn an_adopted_claim_is_dim_and_says_so_among_its_questions() {
    // Catches an adopted claim drawn as if it were still open.
    let mut r = Room::new("offer-adopted");
    r.items(&["Talk-notes says that the date of Buran-flight is 1989."]);
    r.app.idle();
    r.app.evaluate().unwrap();
    let open = rows(&r, "claim  1: ");
    let dim = |r: &Room, row: &str| r.style_of(row.trim()).add_modifier.contains(Modifier::DIM);
    assert!(
        !open[0].contains("(adopted)") && !dim(&r, &open[0]),
        "{open:?}"
    );
    r.command("adopt");
    r.app.evaluate().unwrap();
    let adopted = rows(&r, "claim  1: ");
    assert!(
        adopted[0].ends_with("  (adopted)") && dim(&r, &adopted[0]),
        "{adopted:?}"
    );
}

#[test]
fn a_change_of_a_revision_says_where_it_came_from() {
    let mut r = Room::new("offer-change");
    let file = r.dir.join("a.md");
    std::fs::write(&file, "# A\n\nOne.\n").unwrap();
    r.command(&format!("import document {}", file.display()));
    std::fs::write(&file, "# A\n\nTwo.\n").unwrap();
    r.command(&format!("import document {}", file.display()));
    assert!(r.shows("   from the revision of a.md"), "{:#?}", r.lines());
}

#[test]
fn the_notebook_records_the_suggestions_it_lists_as_offered_once() {
    // Catches a suggestion the notebook shows and the measures never count.
    let mut r = Room::new("offer-recorded");
    r.items(&["Buran flight", "Notes on the buran flight launch"]);
    r.app.idle();
    r.app.evaluate().unwrap();
    assert!(
        offers(&r).is_empty(),
        "offered before the notebook is drawn"
    );
    r.command("go parent");
    r.app.evaluate().unwrap();
    let listed = inspector(&r);
    let listed = listed
        .iter()
        .filter(|l| l.trim_start().starts_with("suggestion "));
    let recorded = offers(&r);
    assert_eq!(recorded.len(), 1, "{recorded:?}");
    let keys = recorded[0]["keys"].as_array().unwrap();
    assert!(!keys.is_empty(), "the test needs a suggestion listed");
    assert_eq!(keys.len(), listed.count(), "{recorded:?}");
    assert_eq!(
        recorded[0]["by"], "memex",
        "the memex offered it on its own"
    );
    r.app.evaluate().unwrap();
    assert_eq!(offers(&r).len(), 1, "offered again");
}

#[test]
fn a_supposition_hides_the_notebook_so_its_suggestions_are_not_offered() {
    // Catches offers recorded from a notebook that the worlds cover.
    let mut r = Room::new("offer-supposed");
    r.items(&["Buran flight", "Notes on the buran flight launch"]);
    r.app.idle();
    r.command("suppose that w");
    r.command("go parent");
    r.app.evaluate().unwrap();
    assert!(
        r.app.reason.supposition.is_some(),
        "the test needs a supposition"
    );
    assert!(
        !r.app.reason.suggestions.list.is_empty(),
        "the test needs suggestions"
    );
    assert!(offers(&r).is_empty(), "{:?}", offers(&r));
}

/// The bodies of the offer entries, in the journal's order.
fn offers(r: &Room) -> Vec<serde_json::Value> {
    let bodies = r.app.journal().entries().iter().filter_map(|e| e.body());
    bodies.filter(|b| b["op"] == "offer").cloned().collect()
}
