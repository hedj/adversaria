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
fn a_link_ignores_case_beyond_ascii() {
    // Catches [[élan]] not linked to "Élan": the text was compared
    // ignoring ASCII case only, though `go élan` finds it (the code
    // review, E2).
    let mut r = Room::new("inline-unicode-case");
    r.items(&["Élan", "See [[élan]]."]);
    r.app.idle();
    let from = titled(&r, "See [[élan]].");
    assert_eq!(inline_to(&r, from), [titled(&r, "Élan")]);
}

#[test]
fn a_name_of_a_deleted_item_gives_way_to_a_text() {
    // Catches a [[link]] resolved to a deleted item by the name it had:
    // the item whose text it is was never linked, and `make` said there
    // was nothing to make (the code review, E1).
    let mut r = Room::new("inline-deleted-name");
    r.items(&["The rocket", "Energia"]);
    let rocket = titled(&r, "The rocket");
    r.put_cursor(rocket);
    r.command("name energia");
    r.command("delete");
    r.put_cursor(titled(&r, "Energia"));
    r.tap(&[Key::Enter]);
    r.typ("See [[energia]].");
    r.app.idle();
    let from = titled(&r, "See [[energia]].");
    assert_eq!(inline_to(&r, from), [titled(&r, "Energia")]);
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
fn a_link_to_nothing_makes_nothing_and_says_so() {
    // Catches an item made without asking, or a link to nothing dropped
    // without a word.
    let mut r = Room::new("inline-unmade");
    r.items(&["Notes", "See [[Energia]]."]);
    r.app.idle();
    assert_eq!(r.outline().in_order().len(), 2, "no item made");
    assert!(
        r.shows("not yet items: Energia · adopt fix makes them"),
        "{:#?}",
        r.lines()
    );
}

#[test]
fn make_makes_the_items_a_text_links_and_links_them() {
    // Catches make not making the item, putting it anywhere but the foot
    // of the top level, or not linking it.
    let mut r = Room::new("inline-make");
    r.items(&["Notes", "See [[Energia]]."]);
    r.app.idle();
    r.command("make");
    let last = *r.outline().in_order().last().unwrap();
    assert_eq!(r.text(last), "Energia");
    assert_eq!(r.outline().parent_or_top(last), memex::outline::TOP);
    assert_eq!(inline_to(&r, titled(&r, "See [[Energia]].")), [last]);
    assert!(!r.shows("not yet items"), "{:#?}", r.lines());
}

#[test]
fn adopt_fix_makes_the_items_as_make_did() {
    // Catches the fix the finding names (docs/concepts.md: fixes are
    // offered, not commanded) doing otherwise than make.
    let mut r = Room::new("inline-adopt-fix");
    r.items(&["Notes", "See [[Energia]]."]);
    r.app.idle();
    r.command("adopt fix");
    let last = *r.outline().in_order().last().unwrap();
    assert_eq!(r.text(last), "Energia");
    assert_eq!(inline_to(&r, titled(&r, "See [[Energia]].")), [last]);
}

#[test]
fn one_undo_takes_make_back_and_keeps_the_text() {
    let mut r = Room::new("inline-make-undo");
    r.items(&["See [[Energia]]."]);
    r.app.idle();
    r.command("make");
    r.app.idle();
    r.command("undo");
    r.app.idle();
    let order = r.outline().in_order();
    let texts: Vec<String> = order.iter().map(|i| r.text(*i)).collect();
    assert_eq!(texts, ["See [[Energia]]."]);
    assert!(inline_to(&r, order[0]).is_empty());
}

#[test]
fn an_item_made_later_is_linked_at_the_pause() {
    // Catches a [[link]] left unresolved after an item of its name is
    // made by hand: only make would then link it.
    let mut r = Room::new("inline-later");
    r.items(&["See [[Energia]]."]);
    r.app.idle();
    r.tap(&[Key::Enter]);
    r.typ("Energia");
    r.app.idle();
    let from = titled(&r, "See [[Energia]].");
    assert_eq!(inline_to(&r, from), [titled(&r, "Energia")]);
}

#[test]
fn undoing_a_later_edit_keeps_an_item_make_made() {
    // Catches the gap this replaces: a [[link]] made its item as part of
    // the text's reading, so undoing the text took the item back too.
    let mut r = Room::new("inline-keep");
    r.items(&["See [[Energia]]."]);
    r.app.idle();
    r.command("make");
    let from = titled(&r, "See [[Energia]].");
    r.put_cursor(from);
    r.tap(&[Key::End]);
    r.typ(" Soon.");
    r.app.idle();
    r.command("undo");
    r.app.idle();
    let texts: Vec<String> = r.outline().in_order().iter().map(|i| r.text(*i)).collect();
    assert_eq!(texts, ["See [[Energia]].", "Energia"]);
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

#[test]
fn a_name_given_later_links_the_links_that_use_it() {
    // Catches a [[link]] left unresolved after an item is named so, or a
    // link left on the item that had the name before, once it moves.
    let mut r = Room::new("inline-name-later");
    r.items(&["See [[buran]].", "The shuttle", "Another"]);
    r.app.idle();
    let from = titled(&r, "See [[buran]].");
    let shuttle = titled(&r, "The shuttle");
    r.put_cursor(shuttle);
    r.command("name buran");
    assert_eq!(inline_to(&r, from), [shuttle]);
    let another = titled(&r, "Another");
    r.put_cursor(another);
    r.command("name buran");
    assert_eq!(
        inline_to(&r, from),
        [another],
        "the link moves with the name"
    );
}

#[test]
fn one_undo_takes_a_name_and_its_links_back() {
    let mut r = Room::new("inline-name-undo");
    r.items(&["See [[buran]].", "The shuttle"]);
    r.app.idle();
    let from = titled(&r, "See [[buran]].");
    r.put_cursor(titled(&r, "The shuttle"));
    r.command("name buran");
    r.command("undo");
    assert!(inline_to(&r, from).is_empty());
    assert!(
        !r.outline().names.contains_key("buran"),
        "the name is taken back too"
    );
}

/// Items Energia, "Energy launch" and Buran, the cursor at the end of a
/// new item with an open link typed.
fn completing(name: &str, typed: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Energia", "Energy launch", "Buran flew once.", typed]);
    r
}

#[test]
fn an_open_link_lists_the_names_that_fit() {
    // Catches a [[link]] typed with nothing to say which names exist, so a
    // typo goes unseen: the third review's fourth finding.
    let r = completing("complete-list", "See [[ener");
    assert!(
        r.shows("[[ Energia · Energy launch · Tab completes"),
        "{:#?}",
        r.lines()
    );
}

#[test]
fn tab_completes_as_far_as_the_names_agree_then_to_one() {
    let mut r = completing("complete-tab", "See [[ener");
    r.tap(&[Key::Tab]);
    assert_eq!(r.here(), "See [[Energ");
    r.typ("i");
    r.tap(&[Key::Tab]);
    assert_eq!(r.here(), "See [[Energia]]", "one name: closed");
}

#[test]
fn tab_inside_a_closed_link_does_not_close_it_again() {
    // Catches `]]` added before a `]]` already there, when the text cursor
    // is moved back into a closed link: "See [[Energia]]]]" (the code
    // review, F-B5).
    let mut r = completing("complete-inside", "See [[Energi]]");
    r.tap(&[Key::Left, Key::Left, Key::Tab]);
    assert_eq!(r.here(), "See [[Energia]]");
}

#[test]
fn a_sentence_is_not_offered_as_a_name() {
    // Catches every item's text offered, a long note among the names.
    let r = completing("complete-sentence", "See [[bur");
    assert!(r.shows("[[ no item is named “bur…”"), "{:#?}", r.lines());
}

#[test]
fn a_closed_link_offers_nothing() {
    let mut r = completing("complete-closed", "See [[Energia]] ener");
    r.tap(&[Key::Tab]);
    assert_eq!(r.here(), "See [[Energia]] ener");
    assert!(!r.shows("Tab completes"), "{:#?}", r.lines());
    assert!(!r.shows("[[ no item"), "{:#?}", r.lines());
    assert_eq!(r.app.session.message, "", "Tab said nothing");
}
