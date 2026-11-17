//! Supposing: a world sprouted from the actual one, and what it sets aside
//! there (docs/plan.md, Milestone 4).

mod support;

use memex::app::{Key, Quasi};
use ratatui::style::Modifier;
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
    // statement and offers its readings, or leaving out that `?` lists
    // what may come next (guide § 2.3, § 3.2).
    let mut r = Room::new("suppose-foot");
    r.items(&["Notes"]);
    r.hold(Quasi::Suppose, "");
    let said = "type a statement · ? lists what may come next · release Suppose offers its readings, or a world of that name if none";
    assert!(r.shows(said), "{:?}", r.lines_at(160, 40).last());
}

#[test]
fn a_derived_text_that_cannot_answer_says_so_in_both_worlds() {
    // Catches the worlds' rows filled from answers alone: the item showed
    // the text typed into it.
    let mut r = Room::new("worlds-cannot-answer");
    r.items(&["rent", "broken"]);
    r.command("derive {N} broken where cell(rent, R), N is R + foo");
    r.leap(Quasi::LeapBack, "rent");
    r.command("fact cell(rent, 1200)");
    r.app.evaluate().unwrap();
    suppose(&mut r, "cell(food, 300)");
    r.app.evaluate().unwrap();
    let rows = r.lines_at(160, 10);
    let row = rows
        .iter()
        .find(|l| l.contains("{N} broken"))
        .expect("a row");
    assert_eq!(row.matches("(cannot answer: type_error").count(), 2);
}

/// A room of rent and food, the sum they make, and a chart of them, with
/// rent supposed to be 1300.
fn rent_supposed(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["rent", "food", "chart", "spent"]);
    r.command("derive Spent: {S} where cell(rent, R), cell(food, F), S is R + F");
    r.leap(Quasi::LeapBack, "chart");
    r.command("layer bar(x(C), y(V)) where cell(C, V)");
    r.leap(Quasi::LeapBack, "food");
    r.command("fact cell(food, 300)");
    r.leap(Quasi::LeapBack, "rent");
    r.command("fact cell(rent, 1200)");
    r.command("constraint one value: false :- cell(C, A), cell(C, B), A \\= B.");
    r.app.evaluate().unwrap();
    suppose(&mut r, "cell(rent, 1300)");
    r.app.evaluate().unwrap();
    r
}

/// The lines of one world's column, by its place, on a wide screen.
fn column(r: &Room, k: usize) -> Vec<String> {
    let half = |l: &String| l.chars().skip(80 * k).take(79).collect::<String>();
    r.lines_at(160, 40).iter().map(half).collect()
}

#[test]
fn a_row_that_differs_is_bold_in_the_supposed_world_alone() {
    // Catches the bold mark put on a row that is the same in both worlds,
    // or left off one that differs.
    let r = rent_supposed("worlds-bold");
    let bold = |text: &str| r.style_of(text).add_modifier.contains(Modifier::BOLD);
    assert!(bold("Spent: 1600"));
    assert!(!bold("Spent: 1500"));
    let food = column(&r, 1).iter().position(|l| l.trim_end() == "food");
    let food = r.screen(160, 40)[(80, food.expect("a row of food") as u16)].style();
    assert!(!food.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn each_row_and_view_is_drawn_once_in_each_world() {
    // Catches a row of both worlds drawn twice, or each item drawn as a
    // view.
    let r = rent_supposed("worlds-once");
    for k in 0..2 {
        let lines = column(&r, k);
        let count = |text: &str| lines.iter().filter(|l| l.trim_end() == text).count();
        assert_eq!((count("food"), count("chart")), (1, 1), "world {k}");
    }
}

#[test]
fn a_view_in_each_world_is_drawn_from_that_worlds_facts() {
    // Catches each world's view drawn from the other's facts.
    let r = rent_supposed("worlds-views");
    let shows = |k, text: &str| column(&r, k).iter().any(|l| l.starts_with(text));
    assert!(shows(0, "  1200 │") && !shows(0, "  1300 │"));
    assert!(shows(1, "  1300 │") && !shows(1, "  1200 │"));
}

#[test]
fn each_fact_set_aside_is_marked_on_its_own_row() {
    // Catches the mark given only to a row that holds every fact set
    // aside: with two, on two items, neither was marked.
    let mut r = Room::new("worlds-two-aside");
    r.items(&["rent", "rent again"]);
    r.command("fact cell(rent, 1250)");
    r.leap(Quasi::LeapBack, "rent");
    r.command("fact cell(rent, 1200)");
    r.command("constraint one value: false :- cell(C, A), cell(C, B), A \\= B.");
    r.app.evaluate().unwrap();
    suppose(&mut r, "cell(rent, 1300)");
    r.app.evaluate().unwrap();
    let marked = column(&r, 1)
        .iter()
        .filter(|l| l.contains("(fact retracted here)"))
        .count();
    assert_eq!(marked, 2, "{}", r.app.session.message);
}

#[test]
fn a_fact_typed_for_an_entity_the_world_lacks_is_supposed_as_typed() {
    // Catches a typed fact read as another entity's: "cell(fuel, 80)",
    // where only rent and food have cells, was offered as "the cell of
    // rent is 80" and "the cell of food is 80", never as typed.
    let mut r = Room::new("worlds-typed-fact");
    r.items(&["rent", "food"]);
    r.command("fact cell(food, 300)");
    r.leap(Quasi::LeapBack, "rent");
    r.command("fact cell(rent, 1200)");
    r.app.evaluate().unwrap();
    suppose(&mut r, "cell(fuel, 80)");
    r.app.evaluate().unwrap();
    let said = &r.app.session.message;
    assert_eq!(said, "supposed; nothing set aside");
    assert!(supposed_facts(&r).contains(&"cell(fuel, 80)".to_string()));
}

#[test]
fn a_typed_fact_already_held_is_believed_already() {
    // Catches a typed fact that is held read as another entity's: with
    // rent at 1200 and food at 300, "cell(rent, 1200)" was offered as "the
    // cell of food is 1200".
    let mut r = Room::new("worlds-typed-held");
    r.items(&["rent", "food"]);
    r.command("fact cell(food, 300)");
    r.leap(Quasi::LeapBack, "rent");
    r.command("fact cell(rent, 1200)");
    r.app.evaluate().unwrap();
    r.hold(Quasi::Suppose, "cell(rent, 1200)");
    r.release(Quasi::Suppose);
    let said = &r.app.session.message;
    assert!(said.contains("is believed already"), "{said}");
}

#[test]
fn english_with_brackets_is_read_through_the_worlds_facts() {
    // Catches English taken as a typed fact because Prolog reads it too:
    // "rent is f(1300)" is is(rent, f(1300)), which no one meant.
    let mut r = Room::new("worlds-english-brackets");
    r.items(&["rent"]);
    r.command("fact cell(rent, 1200)");
    r.app.evaluate().unwrap();
    suppose(&mut r, "rent is f(1300)");
    r.app.evaluate().unwrap();
    let facts = supposed_facts(&r);
    assert!(facts.contains(&"cell(rent, 1300)".to_string()), "{facts:?}");
}

/// The facts of the world shown first, as text.
fn supposed_facts(r: &Room) -> Vec<String> {
    let facts = &r.app.reason.eval[0].as_ref().unwrap().facts;
    facts.iter().map(|(_, f)| memex::reason::show(f)).collect()
}

#[test]
fn a_fact_set_aside_is_named_on_the_line_of_its_constraint_alone() {
    // Catches every fact set aside named on every constraint line: with
    // food supposed at 900, the old food value is set aside under "one
    // value", but "within income", still violated, said it too.
    let mut r = Room::new("worlds-aside-by-constraint");
    r.items(&["income", "rent", "food"]);
    r.command("fact cell(food, 400)");
    r.leap(Quasi::LeapBack, "rent");
    r.command("fact cell(rent, 1200)");
    r.leap(Quasi::LeapBack, "income");
    r.command("fact cell(income, 2000)");
    r.command("constraint one value: false :- cell(C, A), cell(C, B), A \\= B.");
    r.command("constraint within income: false :- cell(income, I), cell(rent, R), cell(food, F), R + F > I.");
    r.app.evaluate().unwrap();
    suppose(&mut r, "cell(food, 900)");
    r.app.evaluate().unwrap();
    let lines = column(&r, 1);
    let line = |name: &str| {
        lines
            .iter()
            .find(|l| l.contains(name))
            .cloned()
            .unwrap_or_default()
    };
    assert!(
        line("Constraint one value:").contains("retracted here"),
        "{lines:#?}"
    );
    assert!(
        !line("Constraint within income:").contains("retracted here"),
        "{lines:#?}"
    );
}

#[test]
fn a_fact_retracted_by_hand_is_named_on_a_line_of_its_own() {
    // Catches a retraction that records no constraint named on every
    // constraint line, or not at all: no constraint asked for it.
    let mut r = Room::new("worlds-aside-by-hand");
    r.items(&["food"]);
    r.command("fact cell(food, 400)");
    let fact = r.last("edit");
    r.command("constraint one value: false :- cell(C, A), cell(C, B), A \\= B.");
    r.app.evaluate().unwrap();
    suppose(&mut r, "plan b");
    r.command(&format!("retract {fact}"));
    r.tap(&[Key::Enter]);
    r.app.evaluate().unwrap();
    let lines = column(&r, 1);
    let on_constraint = lines
        .iter()
        .any(|l| l.starts_with("Constraint") && l.contains("retracted here"));
    let own = lines
        .iter()
        .filter(|l| l.starts_with("Retracted here: "))
        .count();
    assert_eq!((on_constraint, own), (false, 1), "{lines:#?}");
}
