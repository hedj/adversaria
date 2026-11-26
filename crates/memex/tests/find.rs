//! Finding: one verb for every list (docs/concepts.md, concept 10).

mod support;

use memex::app::PaneKind;
use support::Room;

/// A room of labelled items, one of them only holding the label's text.
fn labelled(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["first #draft", "second", "third #Draft", "fourth #drafts"]);
    r
}

#[test]
fn find_a_label_lists_the_items_that_carry_it_and_no_other() {
    // Catches a label found as text: #drafts holds the text "#draft".
    let mut r = labelled("find-label");
    r.command("find #draft");
    assert!(r.shows("LINKED TO #draft · 2"), "{:#?}", r.lines());
    assert!(r.shows("1. 1. first #draft") && r.shows("2. 3. third #Draft"));
    assert!(!r.shows("3. 4. fourth"), "{:#?}", r.lines());
}

#[test]
fn find_a_text_lists_the_items_that_hold_it_as_leap_does() {
    let mut r = labelled("find-text");
    r.command("find #draft");
    r.command("find dra");
    assert!(r.shows("MATCHES OF “dra” · 3"), "{:#?}", r.lines());
}

#[test]
fn find_violations_lists_the_items_a_violation_marks() {
    // Catches a list of the findings that misses one, or lists the
    // disagreements with the violations.
    let mut r = Room::new("find-violations");
    r.items(&[
        "Nothing has more than 1 date.",
        "The date of Buran-flight is 1988.",
        "The date of Buran-flight is 1989.",
        "Talk-notes says that the date of Energia is 1987.",
        "The date of Energia is 1986.",
    ]);
    r.app.idle();
    r.app.evaluate().unwrap();
    r.command("find violations");
    assert!(r.shows("MARKED BY A VIOLATION · 2"), "{:#?}", r.lines());
    assert!(r.shows("1. 2. The date of Buran"), "{:#?}", r.lines());
    r.command("find disagreements");
    assert!(r.shows("MARKED BY A DISAGREEMENT · 2"), "{:#?}", r.lines());
    assert!(r.shows("1. 4. Talk-notes says"), "{:#?}", r.lines());
}

#[test]
fn find_suggestions_and_find_alone_show_those_lists_beside_the_outline() {
    let mut r = labelled("find-panes");
    r.command("find suggestions");
    assert_eq!(r.view().panes[1].kind, PaneKind::Suggestions);
    r.command("find");
    assert_eq!(r.view().panes[1].kind, PaneKind::Links);
}

#[test]
fn a_question_found_is_answered_once_and_not_kept() {
    // Catches a question kept as an item's derived text, as one typed in an
    // item is, or answered from the facts alone.
    let mut r = Room::new("find-question");
    r.items(&["The rent is 1200.", "`income(2000).`"]);
    r.app.idle();
    let entries = r.entry_count();
    r.command("find What is the rent?");
    assert_eq!(r.app.session.message, "1 answer: Answer = 1200");
    r.command("find `income(I)`");
    assert_eq!(r.app.session.message, "1 answer: I = 2000");
    assert_eq!(r.entry_count(), entries, "nothing written");
}

#[test]
fn a_question_that_does_not_read_says_so() {
    let mut r = Room::new("find-unread");
    r.items(&["notes"]);
    r.refused("find Whither the frobnication?");
    let said = &r.app.session.message;
    assert!(said.starts_with("not read as a question"), "{said}");
}

#[test]
fn a_reopened_notebook_lists_what_was_last_found() {
    // Catches a find entry that replay does not read: the list showed the
    // last leap's matches, or nothing.
    let mut r = labelled("find-replay");
    r.command("find #draft");
    r.app.flush();
    let again = support::replayed(&r.dir);
    let found = again.session.found.clone();
    assert_eq!(found, Some(memex::found::Found::Label("draft".into())));
}

#[test]
fn a_question_mark_after_a_word_of_find_is_typed_and_after_a_space_lists() {
    // Catches `?` taken as help at the end of a question, so it could not
    // be typed, or help lost after `find `.
    use memex::app::{Mode, Quasi};
    let mut r = Room::new("find-mark");
    r.hold(Quasi::Command, "find What is it?");
    let Mode::Quasi(a) = &r.app.session.mode else {
        panic!("held")
    };
    assert_eq!((a.input.as_str(), a.listing), ("find What is it?", false));
    r.release(Quasi::Command);
    r.hold(Quasi::Command, "find ?");
    let Mode::Quasi(a) = &r.app.session.mode else {
        panic!("held")
    };
    assert_eq!((a.input.as_str(), a.listing), ("find ", true));
}
