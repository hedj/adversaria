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

#[test]
fn a_statement_with_no_reading_starts_a_named_world_and_says_so() {
    // Catches the world started in silence, under whatever an earlier
    // action said.
    let mut r = Room::new("no-reading");
    r.items(&["Notes"]);
    r.command("outdent");
    r.hold(Quasi::Suppose, "zzz qqq");
    r.release(Quasi::Suppose);
    assert_eq!(r.app.reason.world(), "zzz qqq");
    let said = "no reading of “zzz qqq”: a world of that name, with nothing supposed";
    assert_eq!(r.app.session.message, said);
}

#[test]
fn supposing_what_is_believed_starts_a_world_with_nothing_changed() {
    // Catches the world started under "no reading of", though the
    // statement reads as a belief: there is a reading, and it is so.
    let mut r = Room::new("believed");
    r.items(&["Buran"]);
    r.command("fact date(buran_flight, 1988)");
    r.command("fact label(buran_flight, \"Buran's flight\")");
    let before = r.facts();
    r.hold(Quasi::Suppose, "the flight was in 1988");
    r.release(Quasi::Suppose);
    assert_eq!(r.app.reason.world(), "the flight was in 1988");
    let said =
        "“the flight was in 1988” is believed already: a world of that name, with nothing changed";
    assert_eq!(r.app.session.message, said);
    assert_eq!(r.facts(), before);
}

#[test]
fn only_a_belief_both_named_and_numbered_is_believed_already() {
    // Catches a statement taken for a belief by its number alone, or by
    // its words alone.
    let mut r = Room::new("not-believed");
    r.items(&["Buran"]);
    r.command("fact date(buran_flight, 1988)");
    for text in ["it was in 1988", "the flight was early"] {
        r.hold(Quasi::Suppose, text);
        r.release(Quasi::Suppose);
        assert!(r.app.session.message.starts_with("no reading"), "{text}");
        r.command("close");
    }
}

#[test]
fn the_foot_says_what_holding_suppose_does() {
    // Catches the foot asking for a world's name, where Suppose reads a
    // statement and offers its readings (guide § 2.3, § 3.2).
    let mut r = Room::new("suppose-foot");
    r.items(&["Notes"]);
    r.hold(Quasi::Suppose, "");
    let said =
        "type a statement · release Suppose offers its readings, or a world of that name if none";
    assert!(r.shows(said), "{:?}", r.lines_at(160, 40).last());
}
