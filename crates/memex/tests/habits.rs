//! Editing by habit (docs/concepts-next.md § How far, move 2): Backspace
//! deletes an empty item; Ctrl+X, Ctrl+C and Ctrl+V cut, copy and paste
//! items and text.

mod support;

use memex::app::{Key, Quasi, Special};
use ratatui::style::Modifier;
use support::Room;

/// Three items, A, B and C, the cursor at the end of C.
fn abc(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Alder", "Birch", "Cedar"]);
    r
}

/// The texts of the items, in order.
fn texts(r: &Room) -> Vec<String> {
    let order = r.outline().in_order();
    order.iter().map(|i| r.text(*i)).collect()
}

/// Command held for one letter: a Ctrl chord.
fn chord(r: &mut Room, letter: &str) {
    r.hold(Quasi::Command, letter);
    r.release(Quasi::Command);
}

#[test]
fn backspace_in_an_empty_item_deletes_it_and_goes_to_the_one_above() {
    // Catches Backspace doing nothing in an empty item, so Enter could not
    // be taken back by habit.
    let mut r = abc("habit-empty");
    r.command("go 1");
    r.tap(&[Key::Enter, Key::Backspace]);
    assert_eq!(texts(&r), ["Alder", "Birch", "Cedar"]);
    assert_eq!(r.text(r.app.cursor()), "Alder", "above, not below");
    assert_eq!(r.app.session.view.pos, 5, "at the end of its text");
}

#[test]
fn a_branch_cut_with_the_item_under_it_moves_whole() {
    // Catches the item under one cut moved on its own, out of its parent.
    let mut r = abc("habit-cut-branch");
    r.command("go 2");
    r.command("indent");
    r.command("go 1");
    r.tap(&[Key::Special(Special::ShiftDown)]);
    chord(&mut r, "x");
    r.command("go 2");
    chord(&mut r, "v");
    assert_eq!(texts(&r), ["Cedar", "Alder", "Birch"]);
    let birch = r.outline().in_order()[2];
    assert_eq!(r.outline().number(birch), "2.1", "still under Alder");
}

#[test]
fn backspace_in_an_empty_item_with_items_under_it_deletes_nothing() {
    // Catches a branch deleted by a key that erases a character.
    let mut r = abc("habit-branch");
    r.command("go 2");
    r.tap(&[Key::End]);
    r.command("indent");
    r.command("go 1");
    (0..5).for_each(|_| r.tap(&[Key::Backspace]));
    r.tap(&[Key::Backspace]);
    assert_eq!(texts(&r), ["", "Birch", "Cedar"]);
}

#[test]
fn ctrl_x_then_ctrl_v_moves_the_item_itself_after_the_cursor() {
    // Catches a cut that deletes and a paste that makes a new item, so the
    // item lost its identity and its entries.
    let mut r = abc("habit-cut");
    let alder = r.outline().in_order()[0];
    r.command("go 1");
    chord(&mut r, "x");
    assert!(r.app.session.message.starts_with("cut 1 item:"));
    let dim = r.style_of("Alder").add_modifier.contains(Modifier::DIM);
    assert!(dim, "a cut item is marked until it is pasted");
    r.command("go 3");
    chord(&mut r, "v");
    assert_eq!(texts(&r), ["Birch", "Cedar", "Alder"]);
    assert_eq!(r.outline().in_order()[2], alder, "the same item");
    chord(&mut r, "v");
    assert!(
        r.app
            .session
            .message
            .starts_with("nothing is cut or copied")
    );
}

#[test]
fn ctrl_c_then_ctrl_v_copies_the_items_selected_and_pastes_again() {
    let mut r = abc("habit-copy");
    r.command("go 1");
    r.tap(&[Key::Special(Special::ShiftDown)]);
    chord(&mut r, "c");
    assert!(r.app.session.message.starts_with("copied 2 items"));
    r.command("go 3");
    chord(&mut r, "v");
    chord(&mut r, "v");
    let want = [
        "Alder", "Birch", "Cedar", "Alder", "Birch", "Alder", "Birch",
    ];
    assert_eq!(texts(&r), want);
}

#[test]
fn text_cut_or_copied_is_pasted_at_the_text_cursor() {
    // Catches Ctrl+X on characters selected leaving them, or Ctrl+V
    // putting text anywhere but the text cursor.
    let mut r = abc("habit-text");
    (0..3).for_each(|_| r.tap(&[Key::Special(Special::ShiftLeft)]));
    chord(&mut r, "x");
    assert_eq!(texts(&r)[2], "Ce");
    r.command("go 1");
    r.tap(&[Key::Home]);
    chord(&mut r, "v");
    r.app.flush();
    assert_eq!(texts(&r), ["darAlder", "Birch", "Ce"]);
}

#[test]
fn a_cut_pasted_into_itself_is_refused() {
    let mut r = abc("habit-into");
    r.command("go 3");
    chord(&mut r, "x");
    chord(&mut r, "v");
    assert_eq!(texts(&r), ["Alder", "Birch", "Cedar"]);
    let said = &r.app.session.message;
    assert_eq!(
        said,
        "the cursor is in what was cut: move it out, then Ctrl+V"
    );
}
