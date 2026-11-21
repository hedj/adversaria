//! Moving by the outline's structure (docs/design.md, N4): next,
//! previous, first, last, parent and child.

use memex::app::{Key, Special};

mod support;
use support::Room;

/// A, with B1, B2 and B3 under it, B2 with C under it, then D; the cursor
/// on B2.
fn tree(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["A", "B1", "B2", "C", "B3", "D"]);
    let ids = r.outline().in_order();
    for (item, levels) in [(1, 1), (2, 1), (3, 2), (4, 1)] {
        r.put_cursor(ids[item]);
        (0..levels).for_each(|_| r.command("indent"));
    }
    r.put_cursor(ids[2]);
    r
}

/// The text of the item the cursor is on.
fn at(r: &Room) -> String {
    r.text(r.app.cursor())
}

#[test]
fn next_and_previous_step_over_the_items_under_one() {
    // Catches a structural move that goes line by line, as Down does:
    // from B2, next passes over C, under it, to B3.
    let mut r = tree("structure-next");
    assert_eq!(r.outline().number(r.app.cursor()), "1.2");
    r.command("next");
    assert_eq!(at(&r), "B3");
    r.command("previous");
    assert_eq!(at(&r), "B2");
}

#[test]
fn first_and_last_go_to_the_ends_of_the_list() {
    let mut r = tree("structure-ends");
    r.command("last");
    assert_eq!(at(&r), "B3");
    r.command("first");
    assert_eq!(at(&r), "B1");
}

#[test]
fn parent_and_child_go_up_and_down_a_level() {
    let mut r = tree("structure-levels");
    r.command("child");
    assert_eq!(at(&r), "C");
    r.command("parent");
    assert_eq!(at(&r), "B2");
    r.command("parent");
    assert_eq!(at(&r), "A");
}

#[test]
fn a_move_with_nowhere_to_go_says_so_and_stays() {
    let mut r = tree("structure-none");
    r.command("last");
    r.command("next");
    assert_eq!(r.app.session.message, "no item after it at its level");
    assert_eq!(at(&r), "B3");
    r.command("child");
    assert_eq!(r.app.session.message, "nothing is under it");
}

#[test]
fn child_shows_a_folded_branch() {
    let mut r = tree("structure-folded");
    r.command("fold");
    r.command("child");
    assert_eq!(at(&r), "C");
    assert!(r.shows("C"), "{:#?}", r.lines());
}

#[test]
fn a_structural_move_is_not_a_jump() {
    // Catches next put on the trail, so back would step through each.
    let mut r = tree("structure-trail");
    r.command("last");
    r.command("go 1.1");
    r.command("next");
    assert_eq!(at(&r), "B2");
    r.command("back");
    assert_eq!(at(&r), "B3", "back to where go came from, not next");
}

#[test]
fn shift_and_an_arrow_home_or_end_make_the_six_moves() {
    // Catches the moves with no key, so each had to be typed: the fourth
    // UX review's finding. From B2, by default bindings alone.
    let mut r = tree("structure-keys");
    let press = |r: &mut Room, s| r.tap(&[Key::Special(s)]);
    for (key, lands) in [
        (Special::ShiftDown, "B3"),
        (Special::ShiftUp, "B2"),
        (Special::ShiftHome, "B1"),
        (Special::ShiftEnd, "B3"),
        (Special::ShiftLeft, "A"),
        (Special::ShiftRight, "B1"),
    ] {
        press(&mut r, key);
        assert_eq!(at(&r), lands, "{key:?}");
    }
}
