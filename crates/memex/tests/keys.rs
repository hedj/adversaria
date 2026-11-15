//! What each key does in Normal mode, beside typing: quasimodes, bound
//! commands, and moving the cursor (docs/plan.md, Milestone 3).

mod support;

use memex::app::{Event, Key, Mode, Quasi, ScopeLevel};
use support::Room;

#[test]
fn the_scope_key_starts_at_a_word() {
    let mut r = Room::new("scope");
    r.items(&["one two"]);
    r.app.handle(Event::down(Key::Quasi(Quasi::Scope)));
    assert_eq!(r.view().scope, ScopeLevel::Word);
}

#[test]
fn the_palette_key_opens_the_command_quasimode_latched_and_listing() {
    let mut r = Room::new("palette");
    r.tap(&[Key::F(1)]);
    let open = matches!(&r.app.session.mode, Mode::Quasi(a) if a.q == Quasi::Command && a.latched && a.listing);
    assert!(open, "{:?}", r.app.session.mode);
}

#[test]
fn a_bound_command_that_asks_keeps_its_question() {
    let mut r = Room::new("asks");
    r.items(&["one"]);
    let retract = r.last("edit");
    r.bind(&format!("F9 = \"retract {retract}\""));
    r.tap(&[Key::F(9)]);
    assert!(matches!(r.app.session.mode, Mode::ConfirmRetract { .. }));
}

#[test]
fn an_editing_key_clears_the_message() {
    let mut r = Room::new("clears");
    r.items(&["one"]);
    r.command("help");
    r.typ("x");
    assert_eq!(r.app.session.message, "");
}

#[test]
fn down_at_the_last_item_stays() {
    let mut r = Room::new("last");
    r.items(&["one", "two"]);
    r.tap(&[Key::Down]);
    assert_eq!(r.here(), "two");
}

#[test]
fn down_from_a_cursor_on_no_item_goes_to_the_first() {
    let mut r = Room::new("none");
    r.items(&["one", "two"]);
    r.put_cursor(memex::outline::TOP);
    r.tap(&[Key::Down]);
    assert_eq!(r.here(), "one");
}

#[test]
fn end_goes_to_the_end_of_the_text() {
    let mut r = Room::new("end");
    r.items(&["abc"]);
    r.tap(&[Key::Home, Key::End]);
    r.typ("X");
    assert_eq!(r.here(), "abcX");
}

#[test]
fn right_at_the_end_of_the_text_stays() {
    let mut r = Room::new("right");
    r.items(&["abc"]);
    r.tap(&[Key::Right, Key::Left]);
    r.typ("X");
    assert_eq!(r.here(), "abXc");
}
