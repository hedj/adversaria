//! Reading what the user writes (docs/design.md § Reading controlled
//! English): an item whose text is ACE is read as its statements, as
//! written; prose is left as prose, with a hint at the cursor. These need
//! APE: the Nix shell sets APE_HOME.

mod support;

use format::terms::Term;
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
    assert_eq!(facts_of(&r.app, item), ["cell(income, 2000)"]);
    assert!(!r.app.idle_work());
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
    assert!(all.contains(&"cell(income, 2000)".to_string()), "{all:?}");
    assert!(all.contains(&"cell(rent, 1200)".to_string()), "{all:?}");
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
