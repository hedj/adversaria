//! Moving by the outline's structure (docs/design.md, N4): next,
//! previous, first, last, parent and child.

use memex::app::{Key, Mode, Quasi};

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
fn ctrl_and_an_arrow_home_or_end_make_the_six_moves() {
    // Catches the moves with no key, so each had to be typed: the fourth
    // UX review's finding. From B2, Command held throughout, with nothing
    // typed: each key moves, and the release runs nothing.
    let mut r = tree("structure-keys");
    r.hold(Quasi::Command, "");
    for (key, lands) in [
        (Key::Down, "B3"),
        (Key::Up, "B2"),
        (Key::Home, "B1"),
        (Key::End, "B3"),
        (Key::Left, "A"),
        (Key::Right, "B1"),
    ] {
        r.tap(&[key]);
        assert_eq!(at(&r), lands, "{key:?}");
    }
    r.release(Quasi::Command);
    assert_eq!(at(&r), "B1");
    assert_eq!(r.app.session.mode, Mode::Normal);
    assert_eq!(r.app.session.message, "");
}

#[test]
fn a_move_refused_with_ctrl_held_still_says_why_after_its_release() {
    // Catches the release running the empty line, which clears what the
    // move said.
    let mut r = tree("structure-refused");
    r.command("go 1.2.1");
    r.hold(Quasi::Command, "");
    r.tap(&[Key::Right]);
    r.release(Quasi::Command);
    assert_eq!(r.app.session.message, "nothing is under it");
}

#[test]
fn an_arrow_in_a_latched_or_typed_command_does_not_move() {
    // Catches the moves taken in a Command line being typed, where an
    // arrow is no structural move, or in one latched, where Ctrl is not
    // held.
    let mut r = tree("structure-latched");
    r.tap(&[Key::Quasi(Quasi::Command), Key::Down]);
    assert_eq!(at(&r), "B2", "latched");
    r.tap(&[Key::Escape]);
    r.hold(Quasi::Command, "g");
    r.tap(&[Key::Down]);
    assert_eq!(at(&r), "B2", "typed");
}

#[test]
fn go_and_a_way_moves_as_that_ways_command_did() {
    // Catches `go next` and the rest taken for addresses or texts to look
    // for, or moving otherwise than next and the rest did (docs/concepts.md,
    // decision 5).
    for way in ["next", "previous", "first", "last", "parent", "child"] {
        let mut old = tree(&format!("structure-old-{way}"));
        old.command(way);
        let mut new = tree(&format!("structure-go-{way}"));
        new.command(&format!("go {way}"));
        assert_eq!(at(&new), at(&old), "{way}");
    }
}

#[test]
fn go_next_moves_by_structure_even_where_an_item_is_called_next() {
    // Catches a way's word taken for an item's address: the words of the
    // ways are kept for them, and such an item is reached by its number.
    let mut r = tree("structure-go-named");
    r.items(&["next"]);
    r.put_cursor(r.outline().in_order()[0]);
    r.command("go next");
    assert_eq!(at(&r), "D", "from A, the next at its level");
}
