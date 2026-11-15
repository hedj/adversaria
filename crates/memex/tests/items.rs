//! How an item is drawn in the outline: its text as typed, the text it
//! shows from other items, and the word scope.

use memex::app::Quasi;
use ratatui::style::Modifier;

mod support;
use support::Room;

/// Whether a text is first drawn with these modifiers.
fn drawn(room: &Room, text: &str, m: Modifier) -> bool {
    room.style_of(text).add_modifier.contains(m)
}

#[test]
fn an_item_shows_its_text_as_typed_before_it_is_written() {
    let mut room = Room::new("item-typed");
    room.items(&["Alpha"]);
    room.typ(" beta");
    assert!(room.shows("Alpha beta"));
}

#[test]
fn a_transcluded_text_is_marked_with_the_item_it_is_from() {
    let mut room = Room::new("item-transcluded");
    room.items(&["Alpha", "Beta"]);
    room.command("transclude 1");
    assert!(!drawn(&room, "Alpha", Modifier::ITALIC), "the item's own");
    assert!(drawn(
        &room,
        "Alpha↖1",
        Modifier::ITALIC | Modifier::UNDERLINED
    ));
    assert!(drawn(&room, "↖1", Modifier::DIM));
}

#[test]
fn holding_scope_underlines_the_items_text_as_its_target() {
    let mut room = Room::new("item-scope");
    room.items(&["Alpha", "Beta"]);
    room.command("pane suggestions"); // the links pane would show Beta too
    assert!(!drawn(&room, "Beta", Modifier::UNDERLINED));
    room.hold(Quasi::Scope, "");
    assert!(drawn(&room, "Beta", Modifier::UNDERLINED), "at the cursor");
    assert!(!drawn(&room, "Alpha", Modifier::UNDERLINED), "elsewhere");
}

#[test]
fn an_items_source_is_drawn_before_what_it_cites() {
    let mut room = Room::new("item-notes");
    room.items(&["claim", "paper"]);
    room.put_cursor(room.app.notebook.outline.in_order()[0]);
    room.command("claim said(a) from Smith 2020");
    room.command("link 1 2 as cites");
    assert!(
        room.shows("claim [Smith 2020] [paper]"),
        "{}",
        room.app.session.message
    );
}
