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

/// A right-click where a text starts, on the word there, as the window
/// makes it: the menu's offers, by command line.
fn right_click_on(r: &mut Room, text: &str) -> Vec<String> {
    let at = r.find(text);
    let word = text.split_whitespace().next().unwrap_or("");
    let hits = r.hits();
    r.app.right_click(&hits, at, word, false);
    offered(r)
}

/// The open menu's offers, by command line.
fn offered(r: &Room) -> Vec<String> {
    match &r.app.session.mode {
        memex::app::Mode::Menu(m) => m.offers.iter().map(|o| o.line.clone()).collect(),
        _ => vec![],
    }
}

#[test]
fn a_claim_offers_what_applies_to_it_first() {
    // Catches an item's commands not offered, offered where they do not
    // apply (unadopt with no adoption), or after plumbing's.
    let mut r = dated("translate-claim");
    let offers = right_click_on(&mut r, "Talk-notes says");
    assert_eq!(offers[..2], ["interrogate", "adopt"], "{offers:?}");
    assert!(!offers.contains(&"unadopt".to_string()), "{offers:?}");
    assert_eq!(
        r.here(),
        "Talk-notes says that the date of Buran-flight is 1989."
    );
    r.tap(&[memex::app::Key::Char('2')]);
    let offers = right_click_on(&mut r, "Talk-notes says");
    assert!(
        offers.contains(&"unadopt".to_string()),
        "adopted: {offers:?}"
    );
}

#[test]
fn a_mark_offers_its_rule_and_retracting_its_constraint() {
    // Catches a constraint's commands missing, or the menu acting on the
    // item the mark is under rather than on the constraint.
    let mut r = dated("translate-mark");
    let offers = right_click_on(&mut r, "? sources disagree");
    assert_eq!(offers[0], "go 3", "{offers:?}");
    assert!(offers[1].starts_with("retract "), "{offers:?}");
    r.tap(&[memex::app::Key::Enter]);
    assert_eq!(r.here(), "Nothing has more than 1 date.");
}

#[test]
fn a_change_offers_rejecting_it_which_asks_first() {
    // Catches a revision's change offering nothing, or rejecting it
    // without the question retract asks.
    let mut r = Room::new("translate-change");
    let file = r.dir.join("a.md");
    std::fs::write(&file, "# A\n\nOne.\n").unwrap();
    r.command(&format!("import document {}", file.display()));
    std::fs::write(&file, "# A\n\nTwo.\n").unwrap();
    r.command(&format!("import document {}", file.display()));
    let offers = right_click_on(&mut r, "changed");
    assert!(offers[0].starts_with("retract "), "{offers:?}");
    assert_eq!(offers[1], "commit");
    r.tap(&[memex::app::Key::Enter]);
    let asks = matches!(r.app.session.mode, memex::app::Mode::ConfirmRetract { .. });
    assert!(asks, "{:?}", r.app.session.mode);
}

#[test]
fn a_selection_is_plumbed_whatever_is_presented_under_it() {
    // Catches a selection's text ignored for the object under the pointer.
    let mut r = dated("translate-selected");
    let (at, hits) = (r.find("Talk-notes says"), r.hits());
    r.app.right_click(&hits, at, "1989", true);
    assert_eq!(offered(&r), ["go 1989"]);
}
