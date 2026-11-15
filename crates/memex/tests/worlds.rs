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

#[test]
fn a_supposition_that_cannot_be_written_says_why() {
    let mut r = Room::new("aside-fails");
    r.items(&["ants"]);
    r.command("fact date(ant, 1)");
    std::fs::remove_dir_all(&r.dir).unwrap();
    suppose(&mut r, "the ant 2");
    assert!(
        r.app.session.message.contains("os error"),
        "{}",
        r.app.session.message
    );
}
