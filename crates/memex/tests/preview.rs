//! Interrogation's preview of adopting a claim: the violations that
//! adopting it would raise, as a world of its own (docs/plan.md,
//! Milestone 4).

mod support;

use format::record::EntryKind;
use support::Room;

/// A room with a constraint of one date per event, and these items, each
/// stating its own fact or claim; the cursor on the last.
fn room(name: &str, statements: &[&str]) -> Room {
    let mut r = Room::new(name);
    r.items(&["start"]);
    r.command("constraint one date: false :- date(E, D1), date(E, D2), D1 \\= D2.");
    for (i, statement) in statements.iter().enumerate() {
        r.tap(&[memex::app::Key::Enter]);
        r.typ(&format!("item {i}"));
        r.app.flush();
        r.command(statement);
    }
    r
}

/// The preview of adopting the claims at the cursor, which the inspector
/// shows there without a command.
fn preview(r: &mut Room) -> String {
    r.app.evaluate().unwrap();
    let cursor = r.app.cursor();
    match r.app.reason.preview.clone() {
        Some((item, said)) if item == cursor => said,
        other => panic!("no preview for the cursor's item: {other:?}"),
    }
}

#[test]
fn a_claim_retracted_while_interrogated_leaves_none_to_adopt() {
    // Catches a preview kept for a claim no longer stated: interrogate
    // (an alias) holds the item, so its preview says why it is empty.
    let mut r = room("none", &["claim date(flight, 1989) from talk"]);
    let claim = r.last(EntryKind::Edit);
    r.command("interrogate");
    preview(&mut r);
    r.command(&format!("retract {claim}"));
    r.tap(&[memex::app::Key::Enter]);
    assert_eq!(preview(&mut r), "no claims to adopt");
}

#[test]
fn the_inspector_shows_no_preview_once_its_item_states_no_claim() {
    // Catches a claim's questions kept beside an item that no longer
    // states one, when nothing holds the item.
    let mut r = room("gone", &["claim date(flight, 1989) from talk"]);
    let claim = r.last(EntryKind::Edit);
    preview(&mut r);
    r.command(&format!("retract {claim}"));
    r.tap(&[memex::app::Key::Enter]);
    r.app.evaluate().unwrap();
    assert_eq!(r.app.reason.preview, None);
}

#[test]
fn a_claim_that_conflicts_with_nothing_raises_no_violation() {
    let mut r = room("nothing", &["claim date(flight, 1989) from talk"]);
    assert_eq!(preview(&mut r), "adopting it raises no violation");
}

#[test]
fn only_violations_of_the_adopted_claim_are_counted_and_plural() {
    let statements = [
        "fact date(flight, 1987)",
        "fact date(flight, 1988)",
        "claim date(flight, 1989) from talk",
    ];
    let mut r = room("plural", &statements);
    assert_eq!(
        preview(&mut r),
        "adopting it would raise 2 violations of one date"
    );
}

#[test]
fn a_conflict_with_another_claim_is_no_violation() {
    let statements = [
        "fact date(flight, 1988)",
        "claim date(flight, 1990) from other",
        "claim date(flight, 1989) from talk",
    ];
    let mut r = room("claims", &statements);
    assert_eq!(
        preview(&mut r),
        "adopting it would raise 1 violation of one date"
    );
}
