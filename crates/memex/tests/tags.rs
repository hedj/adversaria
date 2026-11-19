//! Tags (docs/guide.md § 3.3): each #tag in an item's text is offered to
//! goals as item_tag(Item, Tag), so views, derived text and constraints
//! can use it; it follows the text, with nothing to write or retract.

mod support;

use memex::app::Key;
use support::Room;

#[test]
fn a_constraint_over_tags_fires_and_clears_as_the_text_changes() {
    // Catches tags not offered to constraints, or kept after the text
    // lost them.
    let mut r = Room::new("tags-constraint");
    r.items(&["Paper #draft #final"]);
    r.command(
        "constraint not both draft and final: false :- item_tag(I, draft), item_tag(I, final).",
    );
    assert!(
        r.app.session.message.contains("kept"),
        "{}",
        r.app.session.message
    );
    r.app.evaluate().unwrap();
    assert!(r.shows("1 constraint violation"), "{:#?}", r.lines());
    r.tap(&[Key::End]);
    r.tap(&[Key::Backspace; 7]);
    r.app.flush();
    r.app.evaluate().unwrap();
    assert_eq!(r.here(), "Paper #draft");
    assert!(!r.shows("constraint violation"), "{:#?}", r.lines());
}

#[test]
fn derived_text_reads_tags() {
    let mut r = Room::new("tags-derive");
    r.items(&[
        "Buran #reusable",
        "Energia #rocket",
        "Soyuz #rocket",
        "count",
    ]);
    r.command("derive Rockets: {N} where findall(T, (item_tag(I, rocket), item_text(I, T)), L), length(L, N)");
    r.app.evaluate().unwrap();
    assert!(r.shows("Rockets: 2 [derived]"), "{:#?}", r.lines());
}
