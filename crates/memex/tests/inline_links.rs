//! Links typed in an item's text, as [[Buran flight]] (docs/guide.md
//! § 2.4): made at the pause, to the item named or holding the text, or a
//! new one; taken back when the text no longer has them.

mod support;

use memex::app::Key;
use support::Room;

/// The items an item's [[links]] go to.
fn inline_to(r: &Room, item: u64) -> Vec<u64> {
    let links = r.outline().inline_links_of(item);
    links.iter().map(|l| l.to).collect()
}

/// The item whose text this is.
fn titled(r: &Room, text: &str) -> u64 {
    let order = r.outline().in_order();
    *order.iter().find(|i| r.text(**i) == text).expect(text)
}

#[test]
fn a_link_goes_to_the_item_whose_text_it_names() {
    // Catches a [[link]] left as text, or linked to the wrong item.
    let mut r = Room::new("inline-titled");
    r.items(&["Buran flight", "See [[buran flight]] for the date."]);
    r.app.idle();
    let from = titled(&r, "See [[buran flight]] for the date.");
    assert_eq!(inline_to(&r, from), [titled(&r, "Buran flight")]);
}

#[test]
fn a_link_goes_to_the_item_named_so_first() {
    let mut r = Room::new("inline-named");
    r.items(&["The shuttle", "buran"]);
    let shuttle = titled(&r, "The shuttle");
    r.put_cursor(shuttle);
    r.command("name buran");
    r.tap(&[Key::Down, Key::Down]);
    r.tap(&[Key::Enter]);
    r.typ("See [[buran]].");
    r.app.idle();
    let from = titled(&r, "See [[buran]].");
    assert_eq!(
        inline_to(&r, from),
        [shuttle],
        "the name, not the item holding the word"
    );
}

#[test]
fn a_link_to_nothing_makes_the_item_it_names() {
    // Catches a [[link]] to a missing item dropped, or the new item put
    // anywhere but the foot of the top level.
    let mut r = Room::new("inline-new");
    r.items(&["Notes", "See [[Energia]]."]);
    r.app.idle();
    let order = r.outline().in_order();
    let last = *order.last().unwrap();
    assert_eq!(r.text(last), "Energia");
    assert_eq!(r.outline().parent_or_top(last), memex::outline::TOP);
    assert_eq!(inline_to(&r, titled(&r, "See [[Energia]].")), [last]);
}

#[test]
fn a_link_the_text_no_longer_has_is_taken_back() {
    let mut r = Room::new("inline-gone");
    r.items(&["Energia", "See [[Energia]]"]);
    r.app.idle();
    let from = titled(&r, "See [[Energia]]");
    assert_eq!(inline_to(&r, from).len(), 1);
    r.tap(&[Key::End, Key::Backspace, Key::Backspace]);
    r.app.idle();
    assert!(inline_to(&r, from).is_empty(), "{}", r.text(from));
}

#[test]
fn a_sentence_with_a_link_is_still_read() {
    // Catches the brackets handed to the reader, which reads them as not
    // ACE, so the sentence states nothing.
    let mut r = Room::new("inline-read");
    r.items(&["The date of [[Buran-flight]] is 1988."]);
    r.app.idle();
    assert!(
        r.facts().contains(&"date(buran_flight, 1988)".to_string()),
        "{:?}",
        r.facts()
    );
}

#[test]
fn undo_takes_back_a_text_and_its_link_together() {
    // Catches undo taking back only the link, or only the text: a prose
    // item records no reading, so the link would come last.
    let mut r = Room::new("inline-undo");
    r.items(&["Energia"]);
    r.app.idle();
    r.tap(&[Key::Enter]);
    r.typ("See [[Energia]]");
    r.app.idle();
    r.command("undo");
    r.app.idle();
    let order = r.outline().in_order();
    let texts: Vec<String> = order.iter().map(|i| r.text(*i)).collect();
    assert!(!texts.iter().any(|t| t.contains("[[")), "{texts:?}");
    let energia = titled(&r, "Energia");
    assert!(
        r.outline().links_of(energia).1.is_empty(),
        "no link into Energia is left"
    );
}
