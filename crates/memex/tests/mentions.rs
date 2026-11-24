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

#[test]
fn a_mention_keeps_the_identity_prolog_gave_it() {
    // Catches a mention named another way than when Prolog found it: the
    // journal records offers and dismissals by that name, so a dismissed
    // mention would come back.
    let fixture = std::path::Path::new("tests/fixtures/version2");
    let mut app = support::replayed(fixture);
    let entries = app.journal().entries().iter().filter_map(|e| e.body());
    let offer = entries
        .into_iter()
        .find(|b| b["op"] == "offer")
        .unwrap()
        .clone();
    app.evaluate().unwrap();
    let list = &app.reason.suggestions.list;
    let names: Vec<String> = list.iter().map(|s| s.identity()).collect();
    for key in offer["keys"].as_array().unwrap() {
        assert!(names.iter().any(|n| key == n), "{key} not in {names:?}");
    }
}

#[test]
fn a_long_text_is_named_by_its_start_and_a_label_links() {
    // Catches the excerpt not cut at 37 characters, as Prolog cut it, or a
    // label already joining the items ignored.
    let mut r = room(
        "mention-excerpt",
        &[
            "Energia launch",
            "A long note that mentions the energia launch near its end",
            "Buran",
            "More on Buran #buran",
        ],
    );
    let keys = suggested_keys(&mut r);
    let cut = r#"mention(7, 3, "A long note that mentions the energia…", "Energia launch") |  | its text names “Energia launch” and does not link to it"#;
    assert!(
        keys.iter().any(|k| k == cut),
        "as Prolog named it: {keys:?}"
    );
    assert!(
        !keys.iter().any(|k| k.contains("More on Buran")),
        "{keys:?}"
    );
}

#[test]
fn a_title_has_at_most_60_characters_and_six_words() {
    // Catches a title of 61 characters, or of seven words, taken for one.
    let long_title = format!("Buran {}", "x".repeat(55));
    assert_eq!(long_title.chars().count(), 61);
    let naming_it = format!("See {long_title}");
    let seven = "One two three four five six seven";
    let items = [
        long_title.as_str(),
        &naming_it,
        seven,
        "Notes on one two three four five six seven",
    ];
    let keys = suggested_keys(&mut room("mention-titles", &items));
    let titled = |title: &str| {
        keys.iter()
            .any(|k| k.contains(&format!(", \"{title}\") | ")))
    };
    assert!(!titled(&long_title), "61 characters: {keys:?}");
    assert!(!titled(seven), "seven words: {keys:?}");
}

/// The keys of the suggestions, read and reasoned over.
fn suggested_keys(r: &mut Room) -> Vec<String> {
    r.app.idle();
    r.app.evaluate().unwrap();
    r.app
        .reason
        .suggestions
        .list
        .iter()
        .map(|s| s.key())
        .collect()
}

#[test]
fn a_mention_is_ranked_among_the_engine_s_suggestions_by_its_evidence() {
    // Catches the mentions, worked out apart from the engine's
    // suggestions, put after them whatever their evidence: a mention
    // scores 3, so it comes before a rule seen twice, which scores 2.
    let mut r = room(
        "mention-rank",
        &["Buran flight", "Notes on the buran flight launch"],
    );
    for f in ["p(a, b)", "p(c, d)", "q(a, b)", "q(c, d)"] {
        r.command(&format!("fact {f}"));
    }
    r.app.evaluate().unwrap();
    let list = &r.app.reason.suggestions.list;
    let kinds: Vec<String> = list.iter().map(|s| s.identity()).collect();
    let first_rule = kinds.iter().position(|k| k.starts_with("rule("));
    let mention = kinds.iter().position(|k| k.starts_with("mention("));
    assert!(mention.is_some() && first_rule.is_some(), "{kinds:?}");
    assert!(mention < first_rule, "{kinds:?}");
}
