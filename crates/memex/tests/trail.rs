//! Back and forward along the trail of jumps: follow, Leap, go, and a
//! click on a link each record where they came from, in the journal.

use memex::app::{Key, Quasi};

mod support;
use support::{Room, replayed};

/// Three items, Buran linked to Energia, the cursor on Buran.
fn linked(name: &str) -> (Room, [u64; 3]) {
    let mut r = Room::new(name);
    r.items(&["Buran", "Energia", "Polyus"]);
    let items = r.outline().in_order();
    r.put_cursor(items[0]);
    r.command("link to 2");
    (r, [items[0], items[1], items[2]])
}

#[test]
fn back_returns_from_a_follow_and_forward_goes_again() {
    // Catches a follow with no way back, the reviewer's first finding.
    let (mut r, [buran, energia, _]) = linked("trail-follow");
    r.command("follow 1");
    assert_eq!(r.app.cursor(), energia);
    r.command("back");
    assert_eq!(r.app.cursor(), buran);
    r.command("forward");
    assert_eq!(r.app.cursor(), energia);
}

#[test]
fn a_leap_and_a_go_are_jumps_back_returns_from() {
    let (mut r, [buran, energia, polyus]) = linked("trail-leap-go");
    r.leap(Quasi::LeapForward, "Polyus");
    assert_eq!(r.app.cursor(), polyus);
    r.command("go 2");
    assert_eq!(r.app.cursor(), energia);
    r.command("back");
    assert_eq!(r.app.cursor(), polyus);
    r.command("back");
    assert_eq!(r.app.cursor(), buran);
}

#[test]
fn an_arrow_key_is_not_a_jump() {
    // Catches every cursor move put on the trail, so back would step
    // through each line moved over, not return from a jump.
    let (mut r, [buran, _, polyus]) = linked("trail-arrow");
    r.command("go 2");
    r.tap(&[Key::Down]);
    assert_eq!(r.app.cursor(), polyus);
    r.command("back");
    assert_eq!(r.app.cursor(), buran, "Down is not on the trail");
}

#[test]
fn a_new_jump_empties_the_way_forward() {
    let (mut r, [buran, _, _]) = linked("trail-new-jump");
    r.command("follow 1");
    r.command("back");
    r.command("go 3");
    r.command("forward");
    assert_eq!(r.app.session.message, "nothing to go forward to");
    r.command("back");
    assert_eq!(r.app.cursor(), buran);
}

#[test]
fn with_no_jump_back_says_so() {
    let mut r = Room::new("trail-empty");
    r.items(&["Buran"]);
    r.command("back");
    assert_eq!(r.app.session.message, "nothing to go back to");
}

#[test]
fn back_passes_over_an_item_since_deleted() {
    let (mut r, [buran, _, polyus]) = linked("trail-deleted");
    r.command("follow 1");
    r.command("go 3");
    r.command("go 2");
    r.put_cursor(polyus);
    r.command("delete");
    r.command("go 2");
    r.command("back");
    assert_eq!(r.app.cursor(), r.outline().in_order()[1], "back to Energia");
    r.command("back");
    assert_eq!(r.app.cursor(), buran, "Polyus, deleted, passed over");
}

#[test]
fn a_reopened_notebook_keeps_the_trail() {
    // Catches a trail kept only in memory: the journal records each jump's
    // start, so replay builds it again.
    let (mut r, [buran, energia, _]) = linked("trail-replay");
    r.command("follow 1");
    r.app.flush();
    let mut again = replayed(&r.dir);
    assert_eq!(again.cursor(), energia);
    again.run_command("back");
    assert_eq!(again.cursor(), buran);
}

/// A click where a text starts.
fn click_on(r: &mut Room, text: &str) {
    let at = r.find(text);
    let hits = r.hits();
    r.app.click_at(&hits, at);
}

#[test]
fn a_click_on_a_link_is_a_jump_back_returns_from() {
    let (mut r, [buran, energia, _]) = linked("trail-click");
    click_on(&mut r, "Energia  1");
    assert_eq!(r.app.cursor(), energia);
    r.app.mouse_command("back");
    assert_eq!(r.app.cursor(), buran, "the mouse's back button");
}

#[test]
fn the_back_key_is_f7_and_forward_f8() {
    let (mut r, [buran, energia, _]) = linked("trail-keys");
    r.command("follow 1");
    r.tap(&[Key::F(7)]);
    assert_eq!(r.app.cursor(), buran);
    r.tap(&[Key::F(8)]);
    assert_eq!(r.app.cursor(), energia);
}

#[test]
fn follow_with_no_number_is_refused_not_taken_for_one() {
    // Catches `follow x`, or `follow link 2`, silently following link 1
    // (the code review, F-B6).
    let (mut r, [buran, _, _]) = linked("trail-follow-word");
    for line in ["follow x", "follow link 2"] {
        r.app.run_command(line);
        assert_eq!(r.app.cursor(), buran, "{line}");
        assert!(r.app.session.message.contains("number"), "{line}");
    }
}

#[test]
fn go_link_go_back_and_go_forward_do_as_follow_back_and_forward_did() {
    // Catches the one verb for going (docs/concepts.md, decision 5) taking
    // `back` or `link 1` for an address or a text to look for.
    let (mut r, [buran, energia, _]) = linked("trail-go");
    r.command("go link 1");
    assert_eq!(r.app.cursor(), energia);
    r.command("go back");
    assert_eq!(r.app.cursor(), buran);
    r.command("go forward");
    assert_eq!(r.app.cursor(), energia);
}

#[test]
fn go_link_with_no_item_to_go_from_says_so() {
    let mut r = Room::new("trail-go-nothing");
    r.command("go link 1");
    assert_eq!(r.app.session.message, "there is no item to go from");
}
