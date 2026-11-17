//! Leap: the order in which it finds matches, and recognition, which
//! completes a pattern to what every match shares (docs/plan.md,
//! Milestone 3).

mod support;

use memex::app::{Key, Mode, Quasi};
use ratatui::style::Modifier;
use support::Room;

/// Items a1, b, a2 and a3, the cursor on a3.
fn room(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["a1", "b", "a2", "a3"]);
    r
}

/// What recognition makes of a pattern typed into a leap.
fn recognised(r: &mut Room, pattern: &str) -> String {
    r.hold(Quasi::LeapForward, pattern);
    r.tap(&[Key::Escape]);
    match &r.app.session.mode {
        Mode::Quasi(a) => a.input.clone(),
        m => panic!("{m:?}"),
    }
}

#[test]
fn leaping_back_finds_the_nearest_match_before_the_cursor() {
    let mut r = room("back");
    r.leap(Quasi::LeapBack, "a");
    assert_eq!(r.here(), "a2");
}

#[test]
fn leaping_forward_wraps_to_the_first_match() {
    let mut r = room("forward");
    r.leap(Quasi::LeapForward, "a");
    assert_eq!(r.here(), "a1");
}

#[test]
fn the_item_at_the_cursor_is_the_last_match_back() {
    let r = room("last");
    let a3 = r.app.cursor();
    assert_eq!(r.app.leap_matches("a3", false), [a3]);
}

#[test]
fn an_empty_pattern_matches_nothing() {
    let r = room("empty");
    assert!(r.app.leap_matches("", true).is_empty());
}

#[test]
fn recognition_completes_up_to_a_space() {
    let mut r = Room::new("space");
    r.items(&["Energia rocket", "Energia booster"]);
    assert_eq!(recognised(&mut r, "ener"), "energia");
}

#[test]
fn recognition_with_no_match_keeps_the_pattern() {
    let mut r = room("none");
    assert_eq!(recognised(&mut r, "zzz"), "zzz");
}

#[test]
fn recognition_stops_where_the_matches_differ() {
    let mut r = Room::new("differ");
    r.items(&["Energia", "Enigma"]);
    assert_eq!(recognised(&mut r, "en"), "en");
}

#[test]
fn a_pattern_whose_small_letters_are_longer_lists_without_a_crash() {
    // Catches the match's end found by the pattern's own length, where
    // its small letters take more bytes ("İ" is "i̇"): the cut fell inside
    // a character of the item's text, and the memex panicked.
    let mut r = Room::new("leap-dotted");
    r.items(&["ai\u{307}b", "plain"]);
    r.hold(Quasi::LeapForward, "İ");
    let lines = r.lines_at(80, 12);
    assert!(
        lines.iter().any(|l| l.contains("LEAP · 1 MATCHES")),
        "{lines:?}"
    );
}

/// A room of items holding "energy", one two levels down, with a Leap for
/// "ener" held: its list is drawn.
fn listing(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Alpha energy", "Beta"]);
    r.tap(&[Key::Tab]);
    r.items(&["", "Nested energy note"]);
    r.tap(&[Key::Tab]);
    r.items(&["", "Deeper Energy"]);
    for k in 0..6 {
        r.items(&["", &format!("energy {k}")]);
    }
    r.hold(Quasi::LeapForward, "ener");
    r
}

/// The rows of the list's box on a screen 100 by 30, each with the
/// modifiers of its second cell (the first is the box's border).
fn box_rows(r: &Room) -> Vec<(u16, String, Modifier)> {
    let buf = r.screen(100, 30);
    let row = |y: u16| (0..60).map(|x| buf[(x, y)].symbol()).collect::<String>();
    let inside = |y: &u16| row(*y).starts_with('│');
    let rows = (0..30).filter(inside);
    rows.map(|y| (y, row(y), buf[(1, y)].modifier)).collect()
}

#[test]
fn a_leap_lists_five_matches_unless_asked_for_all() {
    let mut r = listing("leap-five");
    assert_eq!(box_rows(&r).len(), 1 + 5, "the heading and five");
    r.typ("?");
    assert_eq!(box_rows(&r).len(), 1 + 9, "the heading and every match");
}

#[test]
fn the_match_a_leap_goes_to_first_is_reversed_and_each_match_is_bold() {
    let r = listing("leap-marks");
    let rows = box_rows(&r);
    assert!(rows[1].2.contains(Modifier::REVERSED) && !rows[2].2.contains(Modifier::REVERSED));
    let (y, text, _) = &rows[2];
    let at = text
        .rfind("ener")
        .map(|i| text[..i].chars().count() as u16)
        .unwrap();
    assert!(
        r.screen(100, 30)[(at, *y)]
            .modifier
            .contains(Modifier::BOLD)
    );
}

#[test]
fn a_match_is_listed_under_the_items_above_it_from_the_top() {
    let rows = box_rows(&listing("leap-trail"));
    let trail = "1. Alpha energy › Beta › Nested energy note";
    assert!(rows.iter().any(|(_, t, _)| t.contains(trail)), "{rows:?}");
}

#[test]
fn a_leaps_list_sits_at_the_foot_of_the_outline() {
    let rows = box_rows(&listing("leap-foot"));
    let bottom = rows.last().unwrap().0 + 1;
    assert_eq!(bottom, 30 - 3, "its border on the outline's last row");
}
