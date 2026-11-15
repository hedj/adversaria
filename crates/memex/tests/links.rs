//! The links pane: an item's links, grouped by kind, each numbered as
//! `follow` numbers it (docs/guide.md § 2.2, § 2.4).

use memex::app::{Key, Quasi};

mod support;
use support::Room;

/// A claim linked to three items, the kinds interleaved: cites,
/// supports, cites; the cursor on the claim.
fn claim(name: &str) -> Room {
    let mut room = Room::new(name);
    room.items(&["Claim", "Paper A", "Data", "Paper B"]);
    for (to, kind) in [(2, "cites"), (3, "supports"), (4, "cites")] {
        room.command(&format!("link 1 to {to} as {kind}"));
    }
    room.put_cursor(room.app.notebook.outline.in_order()[0]);
    room
}

#[test]
fn the_pane_lists_each_kind_once() {
    let room = claim("links-kinds");
    let lines = room.lines_at(160, 20).concat();
    assert_eq!(lines.matches("cites →").count(), 1);
    assert!(lines.contains("Paper A  1") && !lines.contains("Paper A  4"));
}

#[test]
fn follow_n_goes_to_the_link_the_pane_numbers_n() {
    for n in 1..=3 {
        let mut room = claim(&format!("links-follow-{n}"));
        let line = room
            .lines_at(160, 20)
            .into_iter()
            .find(|l| l.trim_end().ends_with(&format!("  {n}")));
        let named = line.expect("a link numbered n");
        room.command(&format!("follow {n}"));
        assert!(
            named.contains(&room.here()),
            "{n}: {named:?} -> {}",
            room.here()
        );
    }
}

#[test]
fn the_pane_shows_the_selected_items_links_with_the_outline_on_the_right() {
    let mut room = Room::new("links-left");
    room.items(&["Alpha", "Beta", "Gamma"]);
    for line in ["pane outline", "switch", "pane links"] {
        room.command(line);
    }
    room.tap(&[Key::Up]);
    assert_eq!(room.here(), "Beta");
    assert!(
        room.lines_at(160, 10)[3].starts_with("  Beta "),
        "the item's text"
    );
}

#[test]
fn with_focus_on_the_links_pane_a_command_acts_on_the_item_it_shows() {
    let mut room = Room::new("links-focus");
    room.items(&["Alpha", "Beta", "Gamma"]);
    room.tap(&[Key::Up]);
    room.command("switch");
    assert!(
        room.lines_at(160, 10)[3].trim_end().ends_with("  Beta"),
        "the pane shows Beta"
    );
    room.command("fact mark(1)");
    let outline = &room.app.notebook.outline;
    let items: Vec<String> = outline
        .fact_item
        .values()
        .map(|i| outline.text(*i))
        .collect();
    assert_eq!(items, ["Beta"]);
}

#[test]
fn with_focus_on_the_links_pane_a_leap_selects_what_it_finds() {
    let mut room = Room::new("links-leap");
    room.items(&["Alpha", "Beta", "Gamma"]);
    room.command("switch");
    room.leap(Quasi::LeapBack, "Alp");
    assert_eq!(room.here(), "Alpha");
    assert!(
        room.lines_at(160, 10)[3].trim_end().ends_with("  Alpha"),
        "the pane shows it"
    );
}
