//! Presentations (docs/design.md, Presentations): what the screen draws
//! that stands for an object of the model is recorded with that object,
//! where it is drawn, and a click on it does that object's default.

mod support;

use memex::present::Presentation;
use support::Room;

/// What is presented where a text starts on the screen.
fn presented_at(r: &Room, text: &str) -> Option<Presentation> {
    let (x, y) = r.find(text);
    r.hits().at(x, y).cloned()
}

/// A click where a text starts.
fn click_on(r: &mut Room, text: &str) {
    let (x, y) = r.find(text);
    let hits = r.hits();
    r.app.click_at(&hits, (x, y));
}

/// Two dates for one flight, one a claim, and a rule an item states.
fn dated(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&[
        "The date of Buran-flight is 1988.",
        "Talk-notes says that the date of Buran-flight is 1989.",
        "Nothing has more than 1 date.",
    ]);
    r.app.idle();
    r.app.evaluate().unwrap();
    r
}

#[test]
fn a_mark_presents_its_constraint_and_a_click_goes_to_the_rule() {
    // Catches a mark line presenting nothing, or its item, or a click on
    // it going anywhere but the item whose text states the rule.
    let mut r = dated("present-mark");
    let rule = r.app.notebook.outline.in_order()[2];
    let mark = presented_at(&r, "? sources disagree");
    assert!(
        matches!(mark, Some(Presentation::Constraint { item: Some(i), .. }) if i == rule),
        "{mark:?}"
    );
    click_on(&mut r, "? sources disagree");
    assert_eq!(r.here(), "Nothing has more than 1 date.");
}

#[test]
fn the_constraint_pane_presents_its_constraint() {
    // Catches the right pane's heading drawn but not presented.
    let mut r = dated("present-pane");
    let first = r.app.notebook.outline.in_order()[0];
    r.put_cursor(first);
    let heading = presented_at(&r, "CONSTRAINT · NOTHING HAS MORE");
    assert!(
        matches!(heading, Some(Presentation::Constraint { .. })),
        "{heading:?}"
    );
}

#[test]
fn a_link_presents_the_item_at_its_other_end() {
    // Catches a link in the right pane that a click cannot follow.
    let mut r = Room::new("present-link");
    r.items(&["Buran", "Energia"]);
    let items = r.outline().in_order();
    let (buran, energia) = (items[0], items[1]);
    r.put_cursor(buran);
    r.command("link to 2");
    let link = presented_at(&r, "Energia  1");
    assert_eq!(
        link,
        Some(Presentation::Item {
            pane: 0,
            id: energia
        })
    );
    click_on(&mut r, "Energia  1");
    assert_eq!(r.app.cursor(), energia);
}

#[test]
fn a_layer_presents_itself_with_its_view() {
    // Catches a view's layer in the right pane presenting nothing.
    let mut r = Room::new("present-layer");
    r.items(&["chart"]);
    r.command("fact cell(rent, 1200)");
    r.command("layer bar(x(C), y(V)) where cell(C, V)");
    r.app.evaluate().unwrap();
    let view = r.app.notebook.outline.in_order()[0];
    let layer = presented_at(&r, "layer bar(x(C), y(V))");
    assert!(
        matches!(layer, Some(Presentation::Layer { view: v, .. }) if v == view),
        "{layer:?}"
    );
}

#[test]
fn a_revisions_change_presents_its_entry_and_a_click_does_nothing() {
    // Catches a change presenting nothing, or a click on it acting: what
    // to do with a change (retract it) is offered, never done by a click.
    let mut r = Room::new("present-change");
    let file = r.dir.join("a.md");
    std::fs::write(&file, "# A\n\nOne.\n").unwrap();
    r.command(&format!("import document {}", file.display()));
    std::fs::write(&file, "# A\n\nTwo.\n").unwrap();
    r.command(&format!("import document {}", file.display()));
    let change = presented_at(&r, "changed");
    let Some(Presentation::Change(entry)) = change else {
        panic!("{change:?}: {:#?}", r.lines());
    };
    let before = r.entry_count();
    click_on(&mut r, "changed");
    assert_eq!(r.entry_count(), before, "entry {entry} untouched");
}
