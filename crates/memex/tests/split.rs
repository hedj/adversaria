//! Splitting an item into its sentences (docs/guide.md § 2.4, split).

mod support;

use support::Room;

/// The texts of the outline, in order.
fn texts(r: &Room) -> Vec<String> {
    let order = r.outline().in_order();
    order.iter().map(|i| r.text(*i)).collect()
}

/// The items an item links to, with each link's type.
fn links_from(r: &Room, item: u64) -> Vec<(u64, String)> {
    let (out, _) = r.outline().links_of(item);
    out.iter().map(|l| (l.to, l.kind.clone())).collect()
}

/// The entries an item's characters come from.
fn sources(r: &Room, item: u64) -> Vec<u64> {
    r.outline().items[&item]
        .spans
        .iter()
        .map(|s| s.seq)
        .collect()
}

/// A part links back to the item it was split from, and its characters
/// are the item's, from the entries they came from before.
fn assert_part(r: &Room, part: u64, item: u64, before: &[u64]) {
    assert_eq!(links_from(r, part), [(item, "split_from".to_string())]);
    let kept = |s: &u64| before.contains(s);
    assert!(
        sources(r, part).iter().all(kept),
        "kept from the item's text {before:?}"
    );
}

#[test]
fn an_item_splits_into_its_sentences_linked_back() {
    // Catches parts out of order, the item not keeping its first
    // sentence, a part not linked back, or a part's characters written
    // anew rather than kept from the item's text.
    let mut r = Room::new("split");
    r.items(&["Buran flew once. It was unmanned. Why only once?", "After"]);
    let item = r.outline().in_order()[0];
    let before = sources(&r, item);
    r.put_cursor(item);
    r.command("split");
    assert_eq!(
        texts(&r),
        [
            "Buran flew once.",
            "It was unmanned.",
            "Why only once?",
            "After"
        ]
    );
    let parts = r.outline().in_order()[1..3].to_vec();
    for part in parts {
        assert_part(&r, part, item, &before);
    }
}

#[test]
fn one_undo_takes_a_split_back_before_or_after_the_pause() {
    // Catches undo taking back one part of a split at a time, the more so
    // once the parts' readings are written after it.
    for pause in [false, true] {
        let mut r = Room::new(if pause {
            "split-undo-pause"
        } else {
            "split-undo"
        });
        r.items(&["The income is 2000. The rent is 1200."]);
        r.app.idle();
        r.command("split");
        if pause {
            r.app.idle();
        }
        r.command("undo");
        r.app.idle();
        assert_eq!(
            texts(&r),
            ["The income is 2000. The rent is 1200."],
            "pause {pause}"
        );
    }
}

/// The item that states a fact whose entry's body holds a word.
fn item_stating(r: &Room, word: &str) -> Option<u64> {
    let facts = r.outline().fact_item.iter();
    let mut stating = facts.filter(|(s, _)| r.body_of(**s).to_string().contains(word));
    stating.next().map(|(_, i)| *i)
}

#[test]
fn each_part_is_read_as_its_own_statements() {
    let mut r = Room::new("split-read");
    r.items(&["The income is 2000. The rent is 1200."]);
    r.app.idle();
    r.command("split");
    r.app.idle();
    let second = r.outline().in_order()[1];
    assert_eq!(
        item_stating(&r, "rent"),
        Some(second),
        "the rent is stated by the second item"
    );
    assert!(
        r.facts().contains(&"cell(income, 2000)".to_string()),
        "{:?}",
        r.facts()
    );
}

#[test]
fn one_sentence_is_not_split() {
    let mut r = Room::new("split-one");
    r.items(&["Only one sentence here."]);
    r.command("split");
    assert!(
        r.app.session.message.contains("nothing to split"),
        "{}",
        r.app.session.message
    );
    assert_eq!(texts(&r).len(), 1);
}
