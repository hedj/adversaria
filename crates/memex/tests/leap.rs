//! Leap: the order in which it finds matches, and recognition, which
//! completes a pattern to what every match shares (docs/plan.md,
//! Milestone 3).

mod support;

use memex::app::{Key, Mode, Quasi};
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
