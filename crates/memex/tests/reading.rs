//! Reading what the user writes (docs/design.md § Reading controlled
//! English): an item whose text is ACE is read as its statements, as
//! written; prose is left as prose, with a hint at the cursor. These need
//! APE: the Nix shell sets APE_HOME.

mod support;

use format::terms::Term;
use memex::ace::Ace;
use memex::app::{App, Key};
use ratatui::style::Color;
use support::Room;

/// The facts the world states, with the entries that state them.
fn stated(app: &App) -> Vec<(u64, Term)> {
    let entries = app.journal().entries();
    memex::store::facts(entries, app.notebook.tip).unwrap()
}

/// The facts the world states, as Prolog writes them.
fn facts(app: &App) -> Vec<String> {
    let facts = stated(app);
    facts.iter().map(|(_, f)| memex::reason::show(f)).collect()
}

/// The facts the world states that came from an item.
fn facts_of(app: &App, item: u64) -> Vec<String> {
    let theirs = stated(app)
        .into_iter()
        .filter(|(s, _)| states(app, item, *s));
    theirs.map(|(_, f)| memex::reason::show(&f)).collect()
}

/// Whether an item states the fact of an entry.
fn states(app: &App, item: u64, seq: u64) -> bool {
    app.notebook.outline.states(item, seq)
}

#[test]
fn a_lexicon_of_twenty_thousand_names_is_sent_in_parts_then_by_what_changed() {
    // Catches the lexicon sent as one message, which holds at most 65,535
    // words (13,000 names did not fit), or a name kept once removed.
    let mut ace = Ace::start().unwrap();
    let many: Vec<Term> = (0..20_000)
        .map(|i| proper_name(&format!("event_{i}")))
        .collect();
    let text = "The date of event_19999 is 1988.";
    assert!(is_ace(&mut ace, &many, text));
    assert!(!is_ace(&mut ace, &many[..19_999], text));
}

#[test]
fn a_lexicon_changed_while_it_is_sent_in_parts_ends_as_the_one_asked_last() {
    // Catches what is left to send kept for entries no longer asked for:
    // a name of the first list sent and not taken out, or one of the
    // second never put in.
    let mut ace = Ace::start().unwrap();
    let first: Vec<Term> = (0..2_000)
        .map(|i| proper_name(&format!("event_{i}")))
        .collect();
    assert!(ace.lexicon_part(&first).unwrap().is_some(), "compared");
    assert!(ace.lexicon_part(&first).unwrap().is_some(), "a first part");
    let mut second = first[1_000..].to_vec();
    second.push(proper_name("buran_flight"));
    assert!(is_ace(
        &mut ace,
        &second,
        "The date of buran_flight is 1988."
    ));
    assert!(is_ace(&mut ace, &second, "The date of event_1999 is 1988."));
    assert!(!is_ace(&mut ace, &second, "The date of event_0 is 1988."));
}

#[test]
fn after_ape_refuses_an_entry_the_lexicon_is_sent_whole_again() {
    // Catches the lexicon taken as still held after APE refused a change,
    // which empties it: the same lexicon set again was not sent.
    let mut ace = Ace::start().unwrap();
    let flight = [proper_name("buran_flight")];
    let malformed = Term::Compound("pn_sg".into(), vec![Term::Atom("energia".into())]);
    let text = "The date of buran_flight is 1988.";
    assert!(is_ace(&mut ace, &flight, text));
    assert!(ace.set_lexicon(&[flight[0].clone(), malformed]).is_err());
    assert!(is_ace(&mut ace, &flight, text));
}

/// An entity as a proper name for itself, as the notebook's lexicon
/// gives it.
fn proper_name(name: &str) -> Term {
    let n = Term::Atom(name.into());
    Term::Compound(
        "pn_sg".into(),
        vec![n.clone(), n, Term::Atom("neutr".into())],
    )
}

/// Whether a text is ACE with this user lexicon.
fn is_ace(ace: &mut Ace, lexicon: &[Term], text: &str) -> bool {
    ace.parse_with(lexicon, text).unwrap().ace.is_some()
}

#[test]
fn an_item_written_in_ace_is_read_as_its_facts() {
    // Catches a sentence of ACE left unread, its fact not tied to its
    // item, or its text not marked as read, in the outline or on screen.
    let mut r = Room::new("read-fact");
    r.items(&["The date of Buran-flight is 1988."]);
    r.app.idle();
    let item = r.app.cursor();
    assert_eq!(facts_of(&r.app, item), ["date(buran_flight, 1988)"]);
    assert!(r.app.notebook.outline.has_meaning(item));
    let shown = r.style_of("The date of Buran-flight is 1988.");
    assert_eq!(shown.fg, Some(Color::Cyan), "read text has its colour");
}

#[test]
fn a_sentence_waits_to_be_read_until_the_user_pauses() {
    // Catches a keystroke that reads, which waits for APE (a second to
    // start) and so breaks the plan's 0.1 s from keystroke to screen.
    let mut r = Room::new("read-wait");
    r.items(&["The income is 2000."]);
    let item = r.app.cursor();
    assert!(facts_of(&r.app, item).is_empty(), "read before a pause");
    assert!(r.app.idle_work());
    r.app.idle();
    assert_eq!(facts_of(&r.app, item), ["box(income, 2000)"]);
    assert!(!r.app.idle_work());
}

#[test]
fn a_pause_is_taken_a_step_at_a_time() {
    // Catches a pause that writes the text and reads it in one step: a key
    // pressed during the pause waited for all of it, as the window could
    // not stop between.
    let mut r = Room::new("read-steps");
    r.typ("The income is 2000.");
    let item = r.app.cursor();
    assert!(r.app.pause_step(), "writing the text leaves its reading");
    assert!(facts_of(&r.app, item).is_empty(), "read in the first step");
    while r.app.pause_step() {}
    assert_eq!(facts_of(&r.app, item), ["box(income, 2000)"]);
}

#[test]
fn a_thing_with_no_name_is_not_read_and_says_why() {
    // Catches the hint for a sentence about one unnamed thing ("a fish"),
    // which ACE reads as "there is a fish", not saying so.
    let mut r = Room::new("read-unnamed");
    r.items(&["A fish is an animal."]);
    r.app.idle();
    let item = r.app.cursor();
    let hint = r
        .app
        .session
        .prose_hint(item)
        .unwrap_or_default()
        .to_string();
    let expected = "not read: ACE takes this as \"There is a fish X1. The fish X1 is an animal.\", which is about something with no name; write \"every\" to mean all of them, or give it a name";
    assert_eq!(hint, expected);
}

#[test]
fn a_text_with_a_part_the_notebook_cannot_state_says_so() {
    // Catches the hint for a sentence the notebook has no statement for
    // ("some fin of each fish") told as one about an unnamed thing.
    let mut r = Room::new("read-unstated");
    r.items(&["Every fish has a fin."]);
    r.app.idle();
    let item = r.app.cursor();
    let hint = r
        .app
        .session
        .prose_hint(item)
        .unwrap_or_default()
        .to_string();
    let expected = "not read: ACE takes this as \"Every fish has a fin.\", but the notebook has no statement for all of it";
    assert_eq!(hint, expected);
}

#[test]
fn a_kind_and_a_rule_on_it_are_read_and_kept() {
    // Catches "Nemo is a fish" read as a box, or a rule over a kind that
    // the memex's own checks refuse.
    let mut r = Room::new("read-kind");
    r.items(&["Nemo is a fish.", "Every fish is an animal."]);
    r.app.idle();
    assert!(
        facts(&r.app).contains(&"fish(nemo)".to_string()),
        "{:?}",
        facts(&r.app)
    );
    let said = &r.app.session.message;
    assert_eq!(said, "rule animal/1 kept");
}

#[test]
fn a_changed_sentence_is_read_again_in_place_of_the_old() {
    // Catches the old reading kept beside the new, so the item states two
    // dates, or the new text not read.
    let mut r = Room::new("read-again");
    r.items(&["The date of Buran-flight is 1988."]);
    r.app.idle();
    let item = r.app.cursor();
    r.tap(&[Key::Backspace; 5]);
    r.typ("1989.");
    r.app.idle();
    assert_eq!(facts_of(&r.app, item), ["date(buran_flight, 1989)"]);
}

#[test]
fn prose_is_not_read_and_says_why_at_the_cursor() {
    // Catches prose read as statements, a reading left when ACE is made
    // prose, or no hint of why the text was not read.
    let mut r = Room::new("read-prose");
    r.items(&["The date of Buran-flight is 1988."]);
    r.app.idle();
    let item = r.app.cursor();
    r.tap(&[Key::End]);
    r.typ(" It flew once, unmanned.");
    r.app.idle();
    assert!(
        facts_of(&r.app, item).is_empty(),
        "{:?}",
        facts_of(&r.app, item)
    );
    assert!(!r.app.notebook.outline.has_meaning(item));
    let shown = r.style_of("The date of Buran-flight is 1988.");
    assert_ne!(
        shown.fg,
        Some(Color::Cyan),
        "prose is not in read text's colour"
    );
    let hint = r
        .app
        .session
        .prose_hint(item)
        .unwrap_or_default()
        .to_string();
    assert!(hint.starts_with("prose: not ACE"), "{hint}");
    assert!(r.shows("prose: not ACE"));
}

#[test]
fn a_sentence_deleted_takes_its_reading_with_it() {
    // Catches an emptied item left stating what its text said.
    let mut r = Room::new("read-empty");
    r.items(&["The income is 2000."]);
    r.app.idle();
    let item = r.app.cursor();
    r.tap(&[Key::Backspace; 19]);
    r.app.idle();
    assert_eq!(r.text(item), "");
    assert!(
        facts_of(&r.app, item).is_empty(),
        "{:?}",
        facts_of(&r.app, item)
    );
}

#[test]
fn boxes_and_a_rule_between_them_are_read() {
    // Catches a box read as anything but cell/2, a rule not kept (a rule
    // for the box it reads is refused as recursive), or a later sentence
    // on the rule's value reading the box, so its constraint never fires.
    let mut r = Room::new("read-rule");
    r.items(&[
        "The income is 2000.",
        "The rent is 1200.",
        "If the income is X and the rent is Y then the rest is X - Y.",
        "It is false that the rest is smaller than 0.",
    ]);
    r.app.idle();
    let all = facts(&r.app);
    assert!(all.contains(&"box(income, 2000)".to_string()), "{all:?}");
    assert!(all.contains(&"box(rent, 1200)".to_string()), "{all:?}");
    let rules = stated(&r.app);
    let ruled = rules
        .iter()
        .any(|(_, f)| matches!(f, Term::Compound(n, _) if n == "rule_def"));
    assert!(ruled, "the rule is kept: {}", r.app.session.message);
    let outline = &r.app.notebook.outline;
    let on_rest: Vec<&str> = outline
        .constraints
        .values()
        .map(|c| c.rule.as_str())
        .collect();
    assert_eq!(on_rest.len(), 1, "{on_rest:?} {}", r.app.session.message);
    assert!(on_rest[0].contains("rest("), "{on_rest:?}");
}

#[test]
fn a_constraint_read_is_kept_by_its_text() {
    // Catches a constraint read but not kept, or kept under another name.
    let mut r = Room::new("read-constraint");
    r.items(&["It is false that the rent is greater than the income."]);
    r.app.idle();
    let names: Vec<&String> = r.outline().constraints.values().map(|c| &c.name).collect();
    assert_eq!(
        names,
        ["It is false that the rent is greater than the income"]
    );
    let said = &r.app.session.message;
    assert!(
        said.ends_with("kept: it fires on a planted violation"),
        "{said}"
    );
}

#[test]
fn each_constraint_of_a_text_is_kept_under_a_name_of_its_own() {
    // Catches two constraints of one text given one name, so the second
    // takes the first's place.
    let mut r = Room::new("read-constraints");
    r.items(&["It is false that the rent is greater than the income. It is false that the food is greater than the income."]);
    r.app.idle();
    let mut names: Vec<&String> = r.outline().constraints.values().map(|c| &c.name).collect();
    names.sort();
    let text = "It is false that the rent is greater than the income. It is false that the food is greater than the income";
    assert_eq!(names, [&format!("{text} (1)"), &format!("{text} (2)")]);
}

#[test]
fn what_a_source_says_is_read_as_its_claim() {
    // Catches what a source says taken for the user's own belief.
    let mut r = Room::new("read-claim");
    r.items(&["Talk-notes says that the date of Buran-flight is 1989."]);
    r.app.idle();
    let claims = r.app.claims_of(r.app.cursor());
    assert_eq!(claims.len(), 1, "{claims:?}");
    assert_eq!(claims[0].1, "Talk-notes");
}

#[test]
fn undo_takes_back_a_sentence_and_its_reading_together() {
    // Catches undo taking back the record of a reading and leaving its
    // facts, so the item states what its text no longer says.
    let mut r = Room::new("read-undo");
    r.items(&["The income is 2000."]);
    r.app.idle();
    r.command("undo");
    r.app.idle();
    assert!(
        facts(&r.app).iter().all(|f| !f.contains("income")),
        "{:?}",
        facts(&r.app)
    );
}

#[test]
fn a_reading_is_kept_in_the_journal() {
    // Catches a reading known only to the session: reopened, the item's
    // text is still marked as read, with its fact.
    let mut r = Room::new("read-reopen");
    r.items(&["The date of Buran-flight is 1988."]);
    r.app.idle();
    let item = r.app.cursor();
    let app = support::replayed(&r.dir);
    assert!(app.notebook.outline.has_meaning(item));
    assert_eq!(facts_of(&app, item), ["date(buran_flight, 1988)"]);
}

/// A room with fish, a rule that every fish is an animal, and a question,
/// read and evaluated.
fn asking(name: &str, question: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&[
        "Nemo is a fish.",
        "Dory is a fish.",
        "Every fish is an animal.",
        question,
    ]);
    r.app.idle();
    r.app.evaluate().unwrap();
    r
}

#[test]
fn a_question_is_answered_under_it_and_the_answer_follows_the_facts() {
    // Catches a question read as a statement, its answer drawn in place of
    // its text, or an answer that does not change with the facts.
    let mut r = asking("ask-which", "What is an animal?");
    assert!(r.shows("What is an animal?"), "{:#?}", r.lines());
    assert!(r.shows("→ dory, nemo"), "{:#?}", r.lines());
    r.tap(&[Key::Enter]);
    r.typ("Bubbles is a fish.");
    r.app.idle();
    r.app.evaluate().unwrap();
    assert!(r.shows("→ bubbles, dory, nemo"), "{:#?}", r.lines());
}

#[test]
fn a_question_of_whether_is_answered_yes_or_no() {
    let r = asking("ask-whether", "Is Nemo an animal?");
    assert!(r.shows("→ yes"), "{:#?}", r.lines());
    let r = asking("ask-whether-not", "Is Bubbles an animal?");
    assert!(r.shows("→ no"), "{:#?}", r.lines());
}

#[test]
fn a_question_of_a_box_is_answered_with_its_value() {
    // Catches "What is the income?" asked of a kind, income(X), not of the
    // box the world holds: the answer would be none.
    let mut r = Room::new("ask-box");
    r.items(&["The income is 2000.", "What is the income?"]);
    r.app.idle();
    r.app.evaluate().unwrap();
    assert!(r.shows("→ 2000"), "{:#?}", r.lines());
}

#[test]
fn a_question_made_prose_loses_its_answer() {
    // Catches the answer of a question left after its text changed.
    let mut r = asking("ask-unsaid", "What is an animal?");
    r.tap(&[Key::End, Key::Backspace]);
    r.typ(", I wonder");
    r.app.idle();
    r.app.evaluate().unwrap();
    assert!(!r.shows("→ dory, nemo"), "{:#?}", r.lines());
}

#[test]
fn a_questions_answer_is_kept_in_the_journal() {
    // Catches a question known only to the session: reopened, it is
    // answered again.
    let r = asking("ask-reopen", "Who is an animal?");
    let mut app = support::replayed(&r.dir);
    app.evaluate().unwrap();
    let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 20)).unwrap();
    t.draw(|f| memex::ui::draw(f, &app)).unwrap();
    let screen = format!("{:?}", t.backend().buffer());
    assert!(screen.contains("→ dory, nemo"), "{screen}");
}

#[test]
fn in_a_supposition_each_world_answers_the_question() {
    // Catches a world showing a question's answer without its question,
    // or in place of it.
    let mut r = asking("ask-worlds", "What is an animal?");
    r.command("suppose that w");
    r.app.evaluate().unwrap();
    let rows = r.lines_at(120, 24);
    let row = rows.iter().find(|l| l.starts_with("4. What is an animal?"));
    let row = row.unwrap_or_else(|| panic!("{rows:#?}"));
    let sides: Vec<&str> = row.split('│').map(str::trim).collect();
    assert_eq!(
        sides, ["4. What is an animal? → dory, nemo"; 2],
        "{rows:#?}"
    );
}

#[test]
fn an_answer_is_shown_by_its_label() {
    // Catches an answer shown by its Prolog name though its thing has a
    // label, or a goal the memex refuses to run (label/2, atom_string/2).
    let mut r = Room::new("ask-label");
    r.items(&[
        "Nemo is a fish.",
        "Dory is a fish.",
        "The label of Nemo is \"Nemo the clownfish\".",
        "Every fish is an animal.",
        "What is an animal?",
    ]);
    r.app.idle();
    r.app.evaluate().unwrap();
    assert!(r.shows("→ Nemo the clownfish, dory"), "{:#?}", r.lines());
}

#[test]
fn which_things_are_animals_is_answered() {
    // Catches the question left prose (ACE has no plural after "are"; the
    // reader asks it in the singular) or "things" asked as a kind.
    let r = asking("ask-plural", "Which things are animals?");
    assert!(r.shows("→ dory, nemo"), "{:#?}", r.lines());
}

// Statements written in Prolog, between backticks (docs/concepts.md): what
// the statement commands typed, written in the item instead.

#[test]
fn a_fact_in_backticks_beside_prose_is_stated_by_its_item() {
    // Catches backticked Prolog left as text, or the prose around it
    // taken for the reason the item is not read.
    let mut r = Room::new("formal-fact");
    r.items(&["Buran flew once, in `date(buran_flight, 1988).` by our source"]);
    r.app.idle();
    let item = r.app.cursor();
    assert_eq!(facts_of(&r.app, item), ["date(buran_flight, 1988)"]);
    assert!(
        !r.app.session.message.contains("not read"),
        "{}",
        r.app.session.message
    );
}

#[test]
fn a_rule_a_constraint_and_a_claim_in_backticks_are_kept_as_their_kinds() {
    // Catches a clause stated as the wrong kind: a rule or a denial as a
    // fact, or a source's claim as the user's belief.
    let mut r = Room::new("formal-kinds");
    r.items(&[
        "`animal(X) :- fish(X).`",
        "One date per event: `false :- date(E, A), date(E, B), A \\= B.`",
        "`claims(\"Talk-notes\", date(buran_flight, 1989)).`",
    ]);
    r.app.idle();
    let rule = |f: &Term| matches!(f, Term::Compound(n, _) if n == "rule_def");
    assert!(stated(&r.app).iter().any(|(_, f)| rule(f)), "a rule");
    let names: Vec<&String> = r.outline().constraints.values().map(|c| &c.name).collect();
    assert_eq!(names, ["One date per event"], "named by the prose");
    let claims = r.app.claims_of(r.app.cursor());
    assert_eq!(claims.len(), 1, "{claims:?}");
    assert_eq!(claims[0].1, "Talk-notes");
}

#[test]
fn a_question_in_backticks_is_answered_and_a_template_is_filled() {
    // Catches a written question or template not shown with its answer.
    let mut r = Room::new("formal-asked");
    r.items(&[
        "`fish(nemo).`",
        "`?- fish(Answer).`",
        "The fish is {F}. `fish(F)`",
    ]);
    r.app.idle();
    r.app.evaluate().unwrap();
    assert!(r.shows("→ nemo"), "{:#?}", r.lines());
    assert!(r.shows("The fish is nemo."), "{:#?}", r.lines());
}

#[test]
fn an_item_deleted_before_the_pause_is_not_read() {
    // Catches a reading of an item that is gone: its facts retracted a
    // second time and its empty text recorded as read, as a redo of its
    // deletion did.
    let mut r = Room::new("read-deleted");
    r.items(&["The rent is 1200.", "Other"]);
    r.app.idle();
    r.put_cursor(r.outline().in_order()[0]);
    for line in ["delete", "undo", "redo"] {
        r.command(line);
    }
    let before = r.entry_count();
    r.app.idle();
    assert_eq!(r.entry_count(), before, "nothing to read: the item is gone");
}
