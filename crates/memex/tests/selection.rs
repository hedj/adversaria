//! The selection (docs/concepts-next.md § The grammar): Shift with an
//! arrow, Home or End selects; Backspace, Delete and typing act on what
//! is selected in an item; a plain move ends it.

mod support;

use memex::app::{Key, Quasi, Special};
use memex::session::Selection;
use ratatui::style::Modifier;
use support::Room;

/// Two items, "Alpha beta" and "Gamma", the cursor at the end of the
/// second.
fn two(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Alpha beta", "Gamma"]);
    r
}

fn shift(r: &mut Room, s: Special, times: usize) {
    (0..times).for_each(|_| r.tap(&[Key::Special(s)]));
}

/// The text of the item the cursor is on, as typed.
fn here(r: &Room) -> String {
    r.app.text_of(r.app.cursor()).iter().collect()
}

#[test]
fn shift_with_left_selects_characters_and_backspace_erases_them() {
    // Catches Shift+Left taken as Left, so Backspace erased one character.
    let mut r = two("select-erase");
    shift(&mut r, Special::ShiftLeft, 3);
    assert_eq!(r.app.selected_range(), Some(2..5));
    r.tap(&[Key::Backspace]);
    assert_eq!(here(&r), "Ga");
    assert_eq!(r.app.session.view.pos, 2);
    assert_eq!(r.app.selected_range(), None, "the selection ends");
}

#[test]
fn delete_erases_the_selection_too_and_a_character_replaces_it() {
    // Catches Delete erasing after the cursor, or a character typed
    // inserted beside the selection rather than in its place.
    let mut r = two("select-replace");
    r.tap(&[Key::Home]);
    shift(&mut r, Special::ShiftRight, 2);
    r.tap(&[Key::Delete]);
    assert_eq!(here(&r), "mma");
    shift(&mut r, Special::ShiftEnd, 1);
    r.typ("x");
    assert_eq!(here(&r), "x");
}

#[test]
fn a_plain_move_ends_the_selection() {
    // Catches a selection kept after Left, so the next Backspace erased
    // text the user no longer saw selected.
    let mut r = two("select-move");
    shift(&mut r, Special::ShiftHome, 1);
    r.tap(&[Key::Left, Key::Backspace]);
    assert_eq!(here(&r), "Gamma", "Backspace at the start erases nothing");
    assert_eq!(r.app.selected_range(), None);
}

#[test]
fn shift_with_up_selects_items_and_backspace_refuses_them() {
    // Catches a key that erases a character deleting a branch, or items
    // selected not drawn so.
    let mut r = two("select-items");
    shift(&mut r, Special::ShiftUp, 1);
    let order = r.outline().in_order();
    assert_eq!(r.app.selected_items(), order);
    r.tap(&[Key::Backspace]);
    assert_eq!(r.outline().in_order(), order, "no item deleted");
    assert!(
        r.app
            .session
            .message
            .contains("selected with Shift in one item")
    );
    assert!(reversed(&r, "1. Alpha", 3) && reversed(&r, "2. Gamma", 3));
}

/// Whether the cell a number of cells after where a text is first drawn
/// is reversed.
fn reversed(r: &Room, text: &str, after: u16) -> bool {
    let (x, y) = r.find(text);
    let style = r.screen(160, 40)[(x + after, y)].style();
    style.add_modifier.contains(Modifier::REVERSED)
}

#[test]
fn characters_selected_are_drawn_reversed_and_the_rest_not() {
    let mut r = two("select-drawn");
    shift(&mut r, Special::ShiftLeft, 2);
    let gamma = "2. Gamma";
    let drawn: Vec<bool> = (3..8).map(|i| reversed(&r, gamma, i)).collect();
    assert_eq!(drawn, [false, false, false, true, true], "Gam, then ma");
}

#[test]
fn a_command_ends_the_selection() {
    let mut r = two("select-command");
    shift(&mut r, Special::ShiftLeft, 2);
    r.command("fold");
    assert_eq!(r.app.selected_range(), None);
}

#[test]
fn a_leap_ends_the_selection() {
    // Catches the selection kept while a Leap moves the cursor, so it
    // grew to wherever the Leap went.
    let mut r = two("select-leap");
    shift(&mut r, Special::ShiftLeft, 2);
    r.leap(Quasi::LeapUp, "Alpha");
    assert_eq!(r.app.selected_items(), [] as [u64; 0]);
    assert_eq!(r.app.session.selection, None);
}

/// Four items, A to D, the cursor on A.
fn four(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Apple", "Banana", "Cherry", "Damson"]);
    r.command("go 1");
    r
}

#[test]
fn the_other_leap_key_selects_the_items_a_leap_went_over() {
    // Catches the other Leap key taken for nothing, or a span that leaves
    // out where the Leap began or where it reached.
    let mut r = four("select-leapt");
    r.hold(Quasi::LeapDown, "Cher");
    r.app
        .handle(memex::app::Event::down(Key::Quasi(Quasi::LeapUp)));
    let order = r.outline().in_order();
    assert_eq!(r.app.selected_items(), order[..3]);
    assert_eq!(r.text(r.app.cursor()), "Cherry");
    assert_eq!(r.app.session.mode, memex::app::Mode::Normal);
    r.release(Quasi::LeapDown);
    r.release(Quasi::LeapUp);
    assert_eq!(r.app.selected_items(), order[..3], "the releases keep it");
}

#[test]
fn a_leap_that_finds_nothing_selects_nothing() {
    let mut r = four("select-leapt-none");
    r.hold(Quasi::LeapDown, "Quince");
    r.app
        .handle(memex::app::Event::down(Key::Quasi(Quasi::LeapUp)));
    assert_eq!(r.app.session.selection, None);
}

#[test]
fn text_dragged_is_selected_and_backspace_leaves_it() {
    // Catches Backspace erasing at the text cursor, beside the text shown
    // selected, and a drag's selection kept after a key that moves.
    let mut r = two("select-drawn-text");
    r.app.select_text(" beta ");
    let drawn = Selection::Drawn("beta".into());
    assert_eq!(r.app.session.selection, Some(drawn));
    r.tap(&[Key::Backspace]);
    assert_eq!(here(&r), "Gamma");
    assert!(r.app.session.message.contains("selected with Shift"));
    r.tap(&[Key::Left]);
    assert_eq!(r.app.session.selection, None);
}

#[test]
fn a_click_ends_a_drag_selection_but_not_one_made_with_keys() {
    let mut r = two("select-drawn-click");
    r.app.select_text("beta");
    r.app.select_text("");
    assert_eq!(r.app.session.selection, None);
    shift(&mut r, Special::ShiftLeft, 1);
    r.app.select_text("");
    assert_eq!(r.app.selected_range(), Some(4..5));
}

#[test]
fn items_selected_from_the_start_of_one_hold_no_characters_of_the_next() {
    // Catches the place where the selection began taken as a place in the
    // cursor's item, so Backspace erased "Gamma" from its start.
    let mut r = two("select-items-down");
    r.command("go 1");
    r.tap(&[Key::Home]);
    shift(&mut r, Special::ShiftDown, 1);
    assert_eq!(r.app.selected_range(), None);
    r.tap(&[Key::Backspace]);
    assert_eq!(here(&r), "Gamma");
}
