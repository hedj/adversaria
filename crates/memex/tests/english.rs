//! Controlled English through the memex: APE, with the words the current
//! world gives it. These tests need APE: run them in the Nix shell, which
//! sets APE_HOME.

use memex::app::{Key, Mode, Quasi};

mod support;
use support::{Room, replayed};

/// What is typed in the quasimode held, if one is.
fn typed(r: &Room) -> String {
    match &r.app.session.mode {
        Mode::Quasi(a) => a.sentence().unwrap_or(&a.input).to_string(),
        other => panic!("no quasimode: {other:?}"),
    }
}

/// The screen's line that holds a text, trimmed; none if none does.
fn line_with(r: &Room, text: &str) -> Option<String> {
    let lines = r.lines_at(160, 30);
    lines
        .into_iter()
        .find(|l| l.contains(text))
        .map(|l| l.trim_end().to_string())
}

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
    r.refused("ace");
    assert_eq!(
        r.app.session.message,
        "ace (sentence) a sentence of controlled English"
    );
}

#[test]
fn a_supposition_being_typed_is_checked_as_it_is_typed() {
    // Catches the hint left as it was before APE: nothing said of what is
    // typed. Without a full stop yet, a sentence of ACE is said to be one.
    let mut r = Room::new("english-suppose-ace");
    r.hold(Quasi::Suppose, "John waits");
    let hint = line_with(&r, "ACE: John waits.");
    assert!(hint.is_some_and(|l| l.contains("release Suppose offers its readings")));
}

#[test]
fn a_supposition_not_ace_shows_where_and_tab_mends_it() {
    // Catches the failure's place or the repair left unsaid, Tab doing
    // nothing in Suppose, or the hint not following what is typed.
    let mut r = Room::new("english-suppose-mend");
    r.hold(Quasi::Suppose, "The flight was in 1989");
    let hint = line_with(&r, "not ACE: The flight <> was in 1989.");
    assert!(hint.is_some_and(|l| l.contains("Tab: was → is")));
    r.tap(&[Key::Tab]);
    assert_eq!(typed(&r), "The flight is in 1989");
    assert!(line_with(&r, "ACE: There is a flight X1. The flight X1 is in 1989.").is_some());
}

#[test]
fn a_supposition_with_no_repair_names_the_words_ace_does_not_know() {
    // Catches a failure with no repair and no reason, and Tab changing
    // the text when there is nothing to apply. APE takes the capitalised
    // Qzx for a proper name, so only vbn is unknown.
    let mut r = Room::new("english-suppose-unknown");
    r.hold(Quasi::Suppose, "Qzx vbn.");
    assert!(line_with(&r, "not ACE: Qzx <> vbn. · words ACE does not know: vbn").is_some());
    r.tap(&[Key::Tab]);
    assert_eq!(typed(&r), "Qzx vbn.");
}

/// A room whose notebook names buran_flight and its date.
fn buran(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["flight"]);
    r.command("fact date(buran_flight, 1988)");
    r
}

#[test]
fn a_supposition_still_being_typed_shows_what_may_come_next() {
    // Catches a sentence still being typed reported as not ACE, or the
    // notebook's words not offered first.
    let mut r = buran("english-next");
    r.hold(Quasi::Suppose, "The date of ");
    let hint = line_with(&r, "next: buran_flight, a name");
    assert!(hint.is_some(), "{:?}", r.lines_at(160, 30).last());
}

#[test]
fn tab_completes_the_word_being_typed_when_one_alone_fits() {
    // Catches the completion unsaid, or Tab leaving the word as typed.
    let mut r = buran("english-complete");
    r.hold(Quasi::Suppose, "The date of bur");
    assert!(line_with(&r, "Tab: buran_flight").is_some());
    r.tap(&[Key::Tab]);
    assert_eq!(typed(&r), "The date of buran_flight ");
    assert!(line_with(&r, "next: ").is_some());
}

/// Whether `?` lists in the quasimode held.
fn listing(r: &Room) -> bool {
    matches!(&r.app.session.mode, Mode::Quasi(a) if a.listing)
}

#[test]
fn question_mark_with_nothing_typed_lists_how_a_sentence_may_begin() {
    // Catches `?` typed into the statement in Suppose, or nothing listed:
    // the notebook's words, and each kind of word with its help.
    let mut r = buran("english-list-start");
    r.hold(Quasi::Suppose, "?");
    assert_eq!(typed(&r), "");
    assert!(line_with(&r, "WHAT MAY COME NEXT").is_some());
    assert!(line_with(&r, "from your notebook: buran_flight").is_some());
    assert!(line_with(&r, "a name — one thing by its name").is_some());
}

#[test]
fn question_mark_after_a_space_lists_and_again_hides_the_list() {
    // Catches the list not following what is typed, or `?` not hiding it.
    let mut r = buran("english-list-toggle");
    r.hold(Quasi::Suppose, "The date of ?");
    assert_eq!(typed(&r), "The date of ");
    assert!(line_with(&r, "a variable — a thing named").is_some());
    r.typ("?");
    assert!(!listing(&r));
    assert!(line_with(&r, "WHAT MAY COME NEXT").is_none());
}

#[test]
fn question_mark_after_a_word_ends_a_question() {
    // Catches `?` taken for help where it ends a question of ACE.
    let mut r = Room::new("english-question");
    r.hold(Quasi::Suppose, "Does John wait?");
    assert_eq!(typed(&r), "Does John wait?");
    assert!(!listing(&r));
    assert!(line_with(&r, "ACE: (APE does not restate this sentence.)").is_some());
}

/// Type free English in Suppose, mend it with Tab, release, and accept the
/// first reading if one is offered: the pair is confirmed.
fn mend_and_suppose(r: &mut Room, free: &str) {
    r.hold(Quasi::Suppose, free);
    r.tap(&[Key::Tab]);
    r.release(Quasi::Suppose);
    if matches!(r.app.session.mode, Mode::ConfirmSuppose { .. }) {
        r.tap(&[Key::Enter]);
    }
}

/// The free English and the name of the last supposition entry.
fn last_pair(r: &Room) -> (Option<String>, String) {
    let e = r
        .app
        .journal()
        .entries()
        .iter()
        .rev()
        .find(|e| e.kind() == "suppose")
        .unwrap();
    let body = e.body().unwrap();
    let english = body
        .get("english")
        .and_then(|v| v.as_str())
        .map(String::from);
    (english, body["name"].as_str().unwrap().to_string())
}

#[test]
fn free_english_mended_into_ace_and_supposed_is_kept_as_a_pair() {
    // Catches the free English lost when the supposition is written, or
    // the mended text recorded as the free English.
    let mut r = buran("pairs-kept");
    mend_and_suppose(&mut r, "the flight was in 1989");
    let pair = (
        Some("the flight was in 1989".to_string()),
        "the flight is in 1989".to_string(),
    );
    assert_eq!(last_pair(&r), pair);
}

#[test]
fn a_supposition_typed_in_ace_from_the_start_keeps_no_pair() {
    // Catches a pair of a text with itself, or of a text never mended.
    let mut r = buran("pairs-none");
    r.hold(Quasi::Suppose, "John waits");
    r.release(Quasi::Suppose);
    if matches!(r.app.session.mode, Mode::ConfirmSuppose { .. }) {
        r.tap(&[Key::Enter]);
    }
    assert_eq!(last_pair(&r).0, None);
}

#[test]
fn a_text_like_an_earlier_pairs_is_offered_its_ace_adapted() {
    // Catches the memory not consulted, its ACE offered unadapted, or Tab
    // not putting it in place; and the pairs lost on reopening.
    let mut r = buran("pairs-recalled");
    mend_and_suppose(&mut r, "the flight was in 1989");
    r.command("close");
    r.hold(Quasi::Suppose, "the flight was in 1990");
    assert!(line_with(&r, "as before: the flight is in 1990 · Tab").is_some());
    r.tap(&[Key::Tab]);
    assert_eq!(typed(&r), "the flight is in 1990");
    let again = replayed(&r.dir);
    let pairs = memex::pairs::pairs_of(again.journal().entries());
    assert_eq!(pairs[0].english, "the flight was in 1989");
}

#[test]
fn the_suppose_key_opens_command_with_suppose_typed() {
    // Catches Suppose as a quasimode of its own, with keys and a foot of
    // its own: the fourth review found it one more mode to learn, where it
    // is the command `suppose` with its name typed for you.
    let mut r = Room::new("suppose-is-command");
    r.items(&["Notes"]);
    r.tap(&[Key::Quasi(Quasi::Suppose)]);
    let opened = matches!(&r.app.session.mode, Mode::Quasi(a) if a.q() == Quasi::Command && a.input == "suppose ");
    assert!(opened, "{:?}", r.app.session.mode);
    assert!(
        r.shows(" COMMAND (latched)  suppose (that)"),
        "{:#?}",
        r.lines()
    );
}

#[test]
fn a_held_suppose_key_still_ends_on_its_own_release() {
    let mut r = Room::new("suppose-release");
    r.items(&["ants"]);
    r.command("fact date(ant, 1)");
    r.hold(Quasi::Suppose, "w");
    r.release(Quasi::Suppose);
    assert!(
        !matches!(r.app.session.mode, Mode::Quasi(_)),
        "{:?}",
        r.app.session.mode
    );
    assert_eq!(
        r.app.reason.supposition.as_ref().map(|s| s.name.as_str()),
        Some("w")
    );
}

#[test]
fn ace_gets_the_live_sentence_line_suppose_has() {
    // Catches the line that checks a sentence as it is typed given to
    // Suppose alone: `ace` takes a sentence too.
    let mut r = Room::new("ace-live");
    r.items(&["Notes"]);
    r.tap(&[Key::Quasi(Quasi::Command)]);
    r.typ("ace the flight was in 1989");
    assert!(
        line_with(&r, "not ACE: the flight <> was in 1989.").is_some(),
        "{:#?}",
        r.lines()
    );
    r.tap(&[Key::Tab]);
    assert_eq!(typed(&r), "the flight is in 1989");
}
