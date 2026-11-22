//! The inspector: the pane beside the outline shows what is known about
//! the item the cursor is on, with no command (docs/concepts.md,
//! decision 4). At a claim, it asks the claim's questions.

mod support;

use memex::app::Key;
use support::Room;

/// Two dates for one flight, one a claim, and a rule an item states; the
/// cursor on the rule.
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

/// The row of the screen that holds a text, if one does.
fn row_of(r: &Room, text: &str) -> Option<usize> {
    r.lines_at(160, 40).iter().position(|l| l.contains(text))
}

#[test]
fn a_claim_item_shows_the_findings_that_mark_it_then_its_questions() {
    // Catches the claim's questions shown in place of the constraint its
    // mark comes from, which scene two of the user story asks to see.
    let mut r = dated("inspect-claim");
    r.tap(&[Key::Up]);
    let constraint = row_of(&r, "CONSTRAINT · ").expect("the constraint");
    let claims = row_of(&r, "CLAIMS OF 2 · WHO SAYS THEM").expect("the questions");
    assert!(constraint < claims, "{:#?}", r.lines_at(160, 40));
    assert!(r.shows("who claims the contrary"));
}

#[test]
fn an_item_that_states_no_claim_shows_its_links() {
    let mut r = dated("inspect-belief");
    r.tap(&[Key::Up, Key::Up]);
    assert!(!r.shows("WHO SAYS THEM"), "{:#?}", r.lines_at(160, 40));
}

#[test]
fn a_preview_is_shown_beside_its_own_item_alone() {
    // Catches the preview of one claim shown beside another, between a
    // move and the evaluation that follows it.
    let mut r = Room::new("inspect-preview");
    r.items(&[
        "Nothing has more than 1 date.",
        "The date of Buran-flight is 1988.",
        "Talk-notes says that the date of Buran-flight is 1989.",
        "Wiki says that the date of Energia is 1987.",
    ]);
    r.app.idle();
    r.tap(&[Key::Up]);
    r.app.evaluate().unwrap();
    assert!(r.shows("adopting it would raise 1 violation"));
    r.tap(&[Key::Down]);
    assert!(r.shows("not yet computed"), "{:#?}", r.lines_at(160, 40));
    r.app.evaluate().unwrap();
    assert!(r.shows("adopting it raises no violation"));
}

#[test]
fn a_move_records_the_claim_item_the_inspector_shows() {
    // Catches a check made by selecting a claim left out of the journal,
    // so the measures report would count it as navigating.
    let mut r = dated("inspect-journal");
    r.tap(&[Key::Up]);
    r.app.flush();
    let claim = r.app.cursor();
    assert_eq!(r.body_of(r.last("view"))["inspects"], claim);
    r.tap(&[Key::Up]);
    r.app.flush();
    assert!(r.body_of(r.last("view"))["inspects"].is_null());
}
