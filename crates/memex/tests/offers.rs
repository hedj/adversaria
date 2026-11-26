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
fn each_suggestion_made_is_recorded_once_in_its_words_with_no_view() {
    // Catches a suggestion recorded only once drawn, as version 2 did,
    // which counted fewer than were made; a record that holds the screen's
    // view; or a suggestion recorded twice.
    let mut r = Room::new("offer-recorded");
    r.items(&["Buran flight", "Notes on the buran flight launch"]);
    r.app.idle();
    r.app.evaluate().unwrap();
    let made = r.app.reason.suggestions.list.len();
    assert!(made > 0, "the test needs a suggestion made");
    let recorded = offers(&r);
    assert_eq!(
        recorded.len(),
        1,
        "made, though no pane shows it: {recorded:?}"
    );
    let offer = &recorded[0];
    assert_eq!(offer["keys"].as_array().unwrap().len(), made, "{offer}");
    let said = offer["said"][0].as_str().unwrap_or("");
    assert!(said.starts_with("link “"), "its words: {offer}");
    assert!(
        offer.get("panes").is_none() && offer.get("pos").is_none(),
        "no view: {offer}"
    );
    assert_eq!(offer["by"], "memex", "the memex offered it on its own");
    r.command("go parent");
    r.app.evaluate().unwrap();
    assert_eq!(offers(&r).len(), 1, "offered again");
}

#[test]
fn a_record_replayed_leaves_the_view_as_it_was() {
    // Catches a record of what reasoning found, which holds no view,
    // taken on replay for an empty view: the text cursor put back at the
    // start of its item.
    let mut r = Room::new("offer-replayed");
    r.items(&["Buran flight", "Notes on the buran flight launch"]);
    r.app.idle();
    r.app.evaluate().unwrap();
    let last = r
        .app
        .journal()
        .entries()
        .last()
        .unwrap()
        .body()
        .unwrap()
        .clone();
    assert_eq!(last["op"], "offer", "the test needs an offer last: {last}");
    let pos = r.app.session.view.pos;
    assert!(pos > 0, "the test needs the text cursor past the start");
    let again = support::replayed(&r.dir);
    assert_eq!(again.session.view.pos, pos);
}

/// The bodies of the offer entries, in the journal's order.
fn offers(r: &Room) -> Vec<serde_json::Value> {
    let bodies = r.app.journal().entries().iter().filter_map(|e| e.body());
    bodies.filter(|b| b["op"] == "offer").cloned().collect()
}
