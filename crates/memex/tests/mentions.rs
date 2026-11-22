//! Unlinked mentions (docs/guide.md § 3.4): an item whose text names
//! another item, as a short title, and does not link to it is offered a
//! link, as a suggestion with its evidence.

mod support;

use support::Room;

/// The suggestions shown, in words, once the world is evaluated.
fn offered(r: &mut Room) -> Vec<String> {
    r.app.evaluate().unwrap();
    let shown = r.app.reason.suggestions.shown();
    shown.iter().map(|s| s.describe()).collect()
}

/// A room whose suggestions pane is open, with these items.
fn room(name: &str, items: &[&str]) -> Room {
    let mut r = Room::new(name);
    r.items(items);
    r.command("pane suggestions");
    r
}

#[test]
fn an_item_that_names_another_is_offered_a_link_to_it() {
    // Catches a mention missed, or offered the wrong way round.
    let mut r = room(
        "mention",
        &["Buran flight", "Notes on the buran flight launch"],
    );
    let offers = offered(&mut r);
    let expected = "link “Notes on the buran flight launch” to “Buran flight”";
    assert!(offers.iter().any(|o| o == expected), "{offers:?}");
}

#[test]
fn accepting_a_mention_links_the_items_and_records_the_suggestion() {
    // Catches an acceptance that links nothing, or a link that does not
    // say it adopted a suggestion, so the measures would not count it.
    let mut r = room(
        "mention-accept",
        &["Buran flight", "Notes on the buran flight launch"],
    );
    offered(&mut r);
    r.command("accept 1");
    let order = r.outline().in_order();
    let (out, _) = r.outline().links_of(order[1]);
    assert_eq!(out.iter().map(|l| l.to).collect::<Vec<_>>(), [order[0]]);
    let link = r.last("link");
    assert!(
        r.body_of(link).get("suggestion").is_some(),
        "{}",
        r.body_of(link)
    );
    let offers = offered(&mut r);
    assert!(
        !offers.iter().any(|o| o.contains("Notes on the buran")),
        "{offers:?}"
    );
}

#[test]
fn an_item_that_links_already_is_offered_nothing() {
    let mut r = room(
        "mention-linked",
        &["Buran flight", "Notes on [[Buran flight]] launch"],
    );
    r.app.idle();
    let offers = offered(&mut r);
    assert!(!offers.iter().any(|o| o.contains("Notes on")), "{offers:?}");
}

#[test]
fn only_whole_words_and_titles_are_mentions() {
    // Catches a title found inside a longer word (at its start, or at its
    // end: "energia launch" in "energia launches"), or a sentence taken for
    // a title, which would offer links all through prose.
    let mut r = room(
        "mention-words",
        &[
            "Buran",
            "The Buranflight was planned.",
            "Energia launch",
            "Two energia launches were planned.",
            "The income is high.",
            "Was the income is high. noted",
        ],
    );
    let offers = offered(&mut r);
    assert!(
        !offers.iter().any(|o| o.contains("Buranflight")),
        "{offers:?}"
    );
    assert!(
        !offers.iter().any(|o| o.contains("launches")),
        "a title's last word is whole: {offers:?}"
    );
    assert!(
        !offers.iter().any(|o| o.contains("“The income is high.”")),
        "{offers:?}"
    );
}

#[test]
fn adopt_takes_a_suggestion_as_it_takes_a_claim() {
    // Catches a suggestion that only `accept` adopts: a suggestion is a
    // claim whose source is the memex, so `adopt` holds it as it holds any
    // claim (docs/concepts.md).
    let mut r = room(
        "mention-adopt",
        &["Buran flight", "Notes on the buran flight launch"],
    );
    offered(&mut r);
    r.command("adopt suggestion 1");
    let order = r.outline().in_order();
    let (out, _) = r.outline().links_of(order[1]);
    assert_eq!(out.iter().map(|l| l.to).collect::<Vec<_>>(), [order[0]]);
    assert!(
        r.app.session.message.starts_with("adopted: "),
        "{}",
        r.app.session.message
    );
}
