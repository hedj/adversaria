//! The screen as a whole: its parts in each mode, and what covers what
//! (docs/guide.md § 2.2).

use memex::app::Quasi;

mod support;
use support::{Room, replayed};

#[test]
fn the_foot_shows_a_held_quasimode_in_a_supposition_too() {
    let mut room = Room::new("screen-foot");
    room.command("suppose that w");
    room.hold(Quasi::Command, "help");
    assert!(room.shows(" COMMAND"));
}

#[test]
fn either_leap_lists_its_matches_over_the_outline() {
    let mut room = Room::new("screen-leaps");
    room.items(&["Alpha", "Beta"]);
    for q in [Quasi::LeapForward, Quasi::LeapBack] {
        room.hold(q, "Al");
        assert!(room.shows("┌"), "{q:?}");
        room.release(q);
    }
}

#[test]
fn no_click_lands_on_the_outline_under_a_list() {
    let mut room = Room::new("screen-covered");
    room.items(&["Alpha", "Beta"]);
    let (x, y) = room.find("Alpha");
    assert!(
        room.hits().at(x, y).is_some(),
        "an item's line is clickable"
    );
    room.hold(Quasi::Command, "?");
    assert!(room.hits().at(x, y).is_none());
}

#[test]
fn a_room_a_crash_left_unfinished_opens_and_says_what_was_set_aside() {
    let mut room = Room::new("screen-torn");
    room.items(&["Alpha"]);
    let file = room.dir.join("journal.jsonl");
    let cut = [std::fs::read(&file).unwrap(), br#"{"body":"#.to_vec()].concat();
    std::fs::write(&file, cut).unwrap();
    let opened = replayed(&room.dir);
    assert!(opened.session.message.ends_with("journal.jsonl.torn"));
    assert_eq!(opened.notebook.outline.text(opened.cursor()), "Alpha");
}
