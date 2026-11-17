//! Controlled English through the memex: APE, with the words the current
//! world gives it. These tests need APE: run them in the Nix shell, which
//! sets APE_HOME.

mod support;
use support::Room;

#[test]
fn a_word_of_the_notebook_makes_a_sentence_ace() {
    // Catches a lexicon not taken from the world: buran_flight is in no
    // lexicon of APE's until a fact names it.
    let mut r = Room::new("english-notebook-word");
    r.items(&["flight"]);
    r.command("ace The date of buran_flight is 1989.");
    let before = r.app.session.message.clone();
    assert!(
        before.starts_with("not ACE: The date of buran_flight <>"),
        "{before}"
    );
    r.command("fact date(buran_flight, 1988)");
    r.command("ace The date of buran_flight is 1989.");
    assert_eq!(
        r.app.session.message,
        "ACE: A date of buran_flight is 1989."
    );
}

#[test]
fn a_sentence_not_ace_says_where_and_why() {
    // Catches a failure without its place or APE's advice: ACE has no past
    // tense, so the parse stops before "was", which APE would make "as".
    let mut r = Room::new("english-not-ace");
    r.command("ace The flight was in 1989.");
    let said = &r.app.session.message;
    assert!(
        said.starts_with("not ACE: The flight <> was in 1989."),
        "{said}"
    );
    assert!(said.contains("was: as"), "{said}");
}

#[test]
fn a_command_without_a_sentence_says_what_it_wants() {
    // Catches the empty text sent to APE.
    let mut r = Room::new("english-empty");
    r.command("ace");
    assert_eq!(
        r.app.session.message,
        "ace (sentence) a sentence of controlled English"
    );
}
