//! The inspector: the pane beside the outline shows what is known about
//! the item the cursor is on, with no command (docs/concepts.md,
//! decision 4). At a claim, it asks the claim's questions.

mod support;

use memex::app::{Key, Special};
use memex::present::Presentation;
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

/// The screen at 100 by 24, drawn and told to the memex, as the window
/// tells it each frame: its rows.
fn drawn(r: &mut Room) -> Vec<String> {
    let hits = r.hits_at(100, 24);
    r.app.fit_inspector(&hits);
    r.lines_at(100, 24)
}

/// Whether a row holds a text.
fn holds(rows: &[String], text: &str) -> bool {
    rows.iter().any(|l| l.contains(text))
}

/// The claim inspected, on a screen too short for its questions.
fn claim_too_tall(name: &str) -> Room {
    let mut r = dated(name);
    r.tap(&[Key::Up]);
    r
}

#[test]
fn an_inspector_too_tall_says_what_is_below_and_keeps_its_hint() {
    // Catches an inspector cut off with no word of it: at 100 by 24 none
    // of a claim's questions showed, nor what adopts it (the UX review,
    // finding 4).
    let mut r = claim_too_tall("inspect-tall");
    let rows = drawn(&mut r);
    assert!(holds(&rows, "rows below · PageDown"), "{rows:#?}");
    assert!(holds(&rows, "F3 adopts · F4 retracts"), "{rows:#?}");
    assert!(!holds(&rows, "rows above"), "{rows:#?}");
}

#[test]
fn page_down_and_page_up_scroll_the_inspector_to_its_end_and_back() {
    // Catches keys that do not scroll it, a scroll past its end, or one
    // that does not come back.
    let mut r = claim_too_tall("inspect-pages");
    drawn(&mut r);
    r.tap(&[Key::Special(Special::PageDown)]);
    let down = drawn(&mut r);
    assert!(holds(&down, "rows above · PageUp"), "{down:#?}");
    r.tap(&[Key::Special(Special::PageDown); 5]);
    let end = drawn(&mut r);
    assert!(holds(&end, "if adopted"), "{end:#?}");
    assert!(!holds(&end, "rows below"), "{end:#?}");
    r.tap(&[Key::Special(Special::PageUp)]);
    let back = drawn(&mut r);
    assert!(
        holds(&back, "rows below"),
        "one page back from the end: {back:#?}"
    );
    r.tap(&[Key::Special(Special::PageUp); 5]);
    let top = drawn(&mut r);
    assert!(holds(&top, "CONSTRAINT · "), "{top:#?}");
    assert!(!holds(&top, "rows above"), "{top:#?}");
}

#[test]
fn a_page_key_bound_to_a_command_runs_it_and_scrolls_nothing() {
    // Catches the scroll taking PageDown from the configuration: the guide
    // says a bound key runs its command instead.
    let mut r = claim_too_tall("inspect-bound-page");
    let toml = "[keys]\nPageDown = \"go 1\"\n";
    r.app.session.keys = memex::config::Config::parse(toml).unwrap().keys();
    let claim = r.app.cursor();
    drawn(&mut r);
    r.tap(&[Key::Special(Special::PageDown)]);
    assert_eq!(r.here(), "The date of Buran-flight is 1988.");
    assert_eq!(r.app.session.inspector.rows_for(claim), 0);
}

#[test]
fn another_item_s_inspector_starts_at_its_top() {
    // Catches the scroll of one item's inspector kept for the next: two
    // claims, each too tall for the screen.
    let mut r = Room::new("inspect-reset");
    r.items(&[
        "The date of Buran-flight is 1988.",
        "Talk-notes says that the date of Buran-flight is 1989.",
        "Wiki says that the date of Buran-flight is 1990.",
        "Nothing has more than 1 date.",
    ]);
    r.app.idle();
    r.app.evaluate().unwrap();
    r.tap(&[Key::Up]);
    drawn(&mut r);
    r.tap(&[Key::Special(Special::PageDown)]);
    assert!(
        holds(&drawn(&mut r), "rows above"),
        "the first claim scrolled"
    );
    r.tap(&[Key::Up]);
    let other = drawn(&mut r);
    assert!(holds(&other, "rows below"), "too tall itself: {other:#?}");
    assert!(!holds(&other, "rows above"), "{other:#?}");
}

#[test]
fn the_wheel_over_the_inspector_scrolls_it_and_elsewhere_does_not() {
    // Catches a wheel that does nothing over the inspector, or one that
    // scrolls it from over the outline.
    let mut r = claim_too_tall("inspect-wheel");
    let hits = r.hits_at(100, 24);
    r.app.fit_inspector(&hits);
    r.app.wheel(&hits, (5, 10), 3);
    assert!(!holds(&drawn(&mut r), "rows above"), "over the outline");
    r.app.wheel(&hits, (80, 10), 3);
    let scrolled = drawn(&mut r);
    assert!(holds(&scrolled, "↑ 3 rows above"), "{scrolled:#?}");
}

#[test]
fn a_click_on_the_inspector_scrolled_lands_on_the_row_drawn_there() {
    // Catches a click on a scrolled inspector left where its row was
    // before it scrolled: the claim's row, below the screen unscrolled,
    // is drawn near the top once scrolled, and a click there is on it.
    let mut r = claim_too_tall("inspect-click");
    drawn(&mut r);
    r.tap(&[Key::Special(Special::PageDown); 6]);
    let rows = drawn(&mut r);
    let (x, y) = cell_of(&rows, "claim  2: the date of");
    let hits = r.hits_at(100, 24);
    let claim = Presentation::Item {
        pane: 0,
        id: r.app.cursor(),
    };
    assert_eq!(hits.at(x, y), Some(&claim), "{rows:#?}");
}

/// The cell a text starts in, on the rows of a screen: its column in
/// characters, as the cells hold them.
fn cell_of(rows: &[String], text: &str) -> (u16, u16) {
    let y = rows.iter().position(|l| l.contains(text)).expect(text);
    let byte = rows[y].find(text).unwrap();
    (rows[y][..byte].chars().count() as u16, y as u16)
}
