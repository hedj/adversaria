//! Labels address (docs/concepts.md, decision 5): a label that one item
//! alone carries names it in commands, as `name` did; `#title` heads the
//! outline. A [[link]] of the label's name goes to the label's page, not
//! to the item that carries it (docs/concepts-seven.md, change 4;
//! tests/pages.rs).

mod support;

use support::Room;

/// The item whose text this is.
fn titled(r: &Room, text: &str) -> u64 {
    let order = r.outline().in_order();
    *order.iter().find(|i| r.text(**i) == text).expect(text)
}

/// The items an item's [[links]] go to.
fn inline_to(r: &Room, item: u64) -> Vec<u64> {
    let links = r.outline().inline_links_of(item);
    links.iter().map(|l| l.to).collect()
}

#[test]
fn a_label_one_item_alone_carries_addresses_it_with_its_hash_or_without() {
    // Catches a label not taken for an address, or one taken for the start
    // of a text: "Drafting" starts with "draft".
    let mut r = Room::new("label-address");
    r.items(&["Drafting notes", "The essay #draft", "Other"]);
    let essay = titled(&r, "The essay #draft");
    for address in ["draft", "#draft", "#Draft"] {
        assert_eq!(r.app.resolve(address), Some(essay), "{address}");
    }
    r.command("go draft");
    assert_eq!(r.app.cursor(), essay);
}

#[test]
fn an_address_longer_than_a_label_is_not_the_label() {
    // Catches "draft essay" taken for the label draft, its first word,
    // where an item's text starts with it.
    let mut r = Room::new("label-longer");
    r.items(&["Draft essay plan", "The essay #draft"]);
    let plan = titled(&r, "Draft essay plan");
    assert_eq!(r.app.resolve("draft essay"), Some(plan));
}

#[test]
fn a_label_two_items_carry_addresses_neither() {
    let mut r = Room::new("label-shared");
    r.items(&["One #draft", "Two #draft"]);
    assert_eq!(r.app.resolve("#draft"), None);
}

#[test]
fn a_label_and_a_link_of_one_name_go_to_its_page_not_to_each_other() {
    // Catches a [[link]] still written to the item that carries the label
    // it names, as decision 5 had it before labels became links.
    let mut r = Room::new("label-and-link");
    r.items(&["See [[draft]].", "The essay #draft"]);
    r.app.idle();
    let from = titled(&r, "See [[draft]].");
    assert_eq!(inline_to(&r, from), [] as [u64; 0]);
    r.command("go draft");
    assert!(r.shows("LINKED TO #draft · 2"), "{:#?}", r.lines());
}

#[test]
fn the_item_labelled_title_heads_the_outline_without_its_label() {
    let mut r = Room::new("label-title");
    r.items(&["Buran's flight #title", "notes"]);
    r.app.evaluate().unwrap();
    assert!(r.shows("OUTLINE · BURAN'S FLIGHT "), "{:#?}", r.lines());
    assert!(!r.shows("BURAN'S FLIGHT #TITLE"));
}
