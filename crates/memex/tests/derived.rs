//! Derived text: what an item shows, computed from the facts (docs/guide.md
//! § 3.1, `derive`).

mod support;
use support::Room;

#[test]
fn a_derived_text_whose_goal_raises_an_error_says_so() {
    // Catches the error dropped: the item showed the text typed into it,
    // as if nothing were derived.
    let mut r = Room::new("derive-error");
    r.items(&["typed"]);
    r.command("fact box(rent, 1200)");
    r.command("derive {N} broken where box(rent, R), N is R + foo");
    r.app.flush();
    r.app.evaluate().unwrap();
    let error = "(cannot answer: type_error(evaluable, foo / 0))";
    assert!(r.shows(&format!("1. {{N}} broken {error} [derived]")));
}
