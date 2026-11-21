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

/// An item stating a claim, interrogated.
fn interrogated(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Buran"]);
    r.command("claim date(buran, 1988) from Encyclopedia");
    r.command("interrogate");
    r.app.evaluate().unwrap();
    r
}

#[test]
fn each_part_of_the_hint_under_an_interrogation_is_a_click_on_its_command() {
    // Catches the second part's click set off from its text.
    let room = interrogated("hint-parts");
    let (x, y) = room.find("F4 retracts an adoption");
    let hits = room.hits();
    let unadopt = Some(memex::present::Presentation::Command("unadopt".into()));
    assert_eq!(hits.at(x, y).cloned(), unadopt, "its first cell");
    assert_eq!(hits.at(x - 1, y), None, "the gap before it");
}

#[test]
fn a_hint_with_no_room_under_the_interrogation_is_no_click() {
    // Catches the hint's clicks placed below the pane, over the foot.
    let room = interrogated("hint-no-room");
    let lines = room.lines_at(80, 7);
    assert!(!lines.concat().contains("adopts"), "no room for the hint");
    let clicks = room.hits_at(80, 7).0;
    assert!(
        !clicks
            .iter()
            .any(|(_, c)| format!("{c:?}").contains("adopt"))
    );
}

/// The right pane's rows, one per line.
fn right_pane(r: &Room) -> String {
    let rows = r.lines_at(160, 40);
    let right = rows.iter().map(|l| l.split('│').nth(1).unwrap_or(""));
    right.collect::<Vec<_>>().join("\n")
}

/// Energia, an item that names it, one with it inside a longer word, and
/// a sentence; the cursor on Energia.
fn mentioned(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&[
        "Energia",
        "The launch of energia went well.",
        "Energiamax is another thing.",
    ]);
    let first = r.outline().in_order()[0];
    r.put_cursor(first);
    r
}

#[test]
fn the_links_pane_lists_the_items_that_mention_it_unlinked() {
    // Catches mentions found only among the five suggestions of the whole
    // notebook: the third review asked for them by the item.
    let r = mentioned("links-mentions");
    let right = right_pane(&r);
    assert!(right.contains("mentioned in, not linked ←"), "{right}");
    assert!(
        right.contains("2. The launch of energia went well."),
        "{right}"
    );
    assert!(
        !right.contains("3. Energiamax"),
        "inside a longer word: {right}"
    );
    assert!(right.contains("no links"), "{right}");
}

#[test]
fn a_mention_linked_is_listed_no_more() {
    let mut r = mentioned("links-mentions-linked");
    let second = r.outline().in_order()[1];
    r.put_cursor(second);
    r.command("link to 1");
    let first = r.outline().in_order()[0];
    r.put_cursor(first);
    let right = right_pane(&r);
    assert!(!right.contains("mentioned in, not linked"), "{right}");
}

#[test]
fn a_click_on_a_mention_jumps_to_it() {
    let mut r = mentioned("links-mentions-click");
    let at = r.find("2. The launch of energia");
    let hits = r.hits();
    r.app.click_at(&hits, at);
    assert_eq!(r.app.cursor(), r.outline().in_order()[1]);
}
