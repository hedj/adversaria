//! Supposing: a world sprouted from the actual one, and what it sets aside
//! there (docs/plan.md, Milestone 4).

mod support;

use memex::app::{Key, Quasi};
use support::Room;

/// Suppose a text, held in the Suppose quasimode, and take the reading
/// "do you mean?" offers first.
fn suppose(r: &mut Room, text: &str) {
    r.hold(Quasi::Suppose, text);
    r.release(Quasi::Suppose);
    r.tap(&[Key::Enter]);
}

#[test]
fn a_supposition_that_contradicts_nothing_sets_nothing_aside() {
    let mut r = Room::new("aside-nothing");
    r.items(&["ants"]);
    r.command("fact date(ant, 1)");
    suppose(&mut r, "the ant 2");
    assert_eq!(r.app.session.message, "supposed; nothing set aside");
}
