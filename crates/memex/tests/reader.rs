//! One reader for every line (docs/concepts-next.md § How far, move 4;
//! crate::actions::reader): a Command line whose first word names no
//! command is read by its form. These tests need APE: run them in the Nix
//! shell, which sets APE_HOME.

mod support;

use memex::app::{Key, Mode, Quasi};
use support::Room;

/// An item, and a belief that Buran flew in 1988.
fn buran(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["flight"]);
    r.command("fact date(buran_flight, 1988)");
    r
}

/// Command latched, a line typed, and Enter.
fn entered(r: &mut Room, line: &str) {
    r.tap(&[Key::Quasi(Quasi::Command)]);
    r.typ(line);
    r.tap(&[Key::Enter]);
}

/// The Command line typed, if Command is open and latched.
fn kept(r: &Room) -> Option<String> {
    match &r.app.session.mode {
        Mode::Quasi(a) if a.latched() => Some(a.input.clone()),
        _ => None,
    }
}

#[test]
fn a_statement_typed_with_no_command_offers_its_readings() {
    // Catches a statement refused as no command, so it needed `suppose`
    // before it.
    let mut r = buran("reader-statement");
    entered(&mut r, "the flight was in 1989");
    let offered = matches!(&r.app.session.mode, Mode::ConfirmSuppose { .. });
    assert!(
        offered,
        "{:?} {}",
        r.app.session.mode, r.app.session.message
    );
    r.tap(&[Key::Enter]);
    assert!(r.app.reason.world().contains("the flight was in 1989"));
}

#[test]
fn a_question_typed_with_no_command_is_answered_once() {
    let mut r = Room::new("reader-question");
    r.items(&["The rent is 1200."]);
    r.app.idle();
    let entries = r.entry_count();
    entered(&mut r, "What is the rent?");
    assert_eq!(r.app.session.message, "1 answer: Answer = 1200");
    assert_eq!(r.entry_count(), entries, "nothing written");
}

#[test]
fn a_line_with_no_reading_is_refused_and_kept_and_makes_no_world() {
    // Catches a mistyped command supposed as the name of a world, as
    // `suppose` before it would make one.
    let mut r = buran("reader-refused");
    let entries = r.entry_count();
    entered(&mut r, "xyzzy plugh");
    let said = &r.app.session.message;
    assert!(
        said.starts_with("no command “xyzzy”, and it has no reading"),
        "{said}"
    );
    assert_eq!(
        kept(&r).as_deref(),
        Some("xyzzy plugh"),
        "kept to be mended"
    );
    assert_eq!(r.entry_count(), entries, "nothing written");
    assert_eq!(r.app.reason.world(), "actual");
}

#[test]
fn a_statement_believed_already_is_refused_and_says_so() {
    let mut r = buran("reader-believed");
    entered(&mut r, "the flight was in 1988");
    let said = &r.app.session.message;
    assert!(said.contains("it is believed already"), "{said}");
}

#[test]
fn a_sentence_that_starts_a_is_no_adopt() {
    // Catches "A" taken for adopt, which it alone starts, so a sentence
    // adopted the target's claims.
    let mut r = buran("reader-article");
    r.hold(Quasi::Command, "A flight was in 1989");
    let reads = matches!(&r.app.session.mode, Mode::Quasi(a) if a.reads());
    assert!(reads, "{:?}", r.app.session.mode);
    assert!(
        r.shows("no command, so read as a statement"),
        "{:#?}",
        r.lines()
    );
}

#[test]
fn a_command_named_first_is_run_whatever_follows() {
    // Catches a command line read as a sentence: a command's name, or a
    // start of two letters, always wins.
    let mut r = buran("reader-command");
    entered(&mut r, "go 1");
    assert_eq!(kept(&r), None);
    entered(&mut r, "fi flight");
    assert!(r.shows("MATCHES OF “flight”"), "{:#?}", r.lines());
}

#[test]
fn tab_mends_a_line_read_by_its_form_in_place() {
    // Catches the repair put after the line's first word, as after a
    // verb, so the mended line began with a word of the old one.
    let mut r = buran("reader-mend");
    r.hold(Quasi::Command, "The flight was in 1989");
    r.tap(&[Key::Tab]);
    let line = match &r.app.session.mode {
        Mode::Quasi(a) => a.input.clone(),
        other => format!("{other:?}"),
    };
    assert_eq!(line, "The flight is in 1989");
}
