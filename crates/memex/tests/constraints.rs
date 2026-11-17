//! Keeping a constraint: it must fire on a planted violation before it is
//! kept (docs/guide.md § 2.4, `constraint`; design.md § Testing claims).

mod support;
use memex::app::Quasi;
use support::Room;

/// What stating a constraint says, in a room where rent is 1200.
fn stated(name: &str, constraint: &str) -> String {
    let mut r = Room::new(name);
    r.items(&["rent"]);
    r.command("fact cell(rent, 1200)");
    r.command(&format!("constraint {constraint}"));
    r.app.session.message.clone()
}

#[test]
fn a_constraint_that_compares_with_a_number_is_kept() {
    // Catches values planted only near zero, which no limit such as
    // R > 1000 is true of: the constraint was refused as never firing.
    for (n, body) in ["R > 1000", "R >= 1000, R =< 5000", "R =:= 1200", "R > 0.5"]
        .iter()
        .enumerate()
    {
        let said = stated(
            &format!("limit-{n}"),
            &format!("limit: false :- cell(C, R), {body}."),
        );
        assert_eq!(
            said, "constraint limit kept: it fires on a planted violation",
            "{body}"
        );
    }
}

#[test]
fn a_constraint_without_variables_is_tried_too() {
    // Catches a body with no variables, which the planting did not answer.
    let said = stated("no-variables", "exact: false :- cell(rent, 1200).");
    assert_eq!(
        said,
        "constraint exact kept: it fires on a planted violation"
    );
}

#[test]
fn a_constraint_that_can_never_fire_is_still_refused() {
    let said = stated("never", "never: false :- cell(C, R), R > 5, R < 3.");
    assert!(said.starts_with("constraint not kept"), "{said}");
}

/// A constraint that a cell has one value.
const ONE_VALUE: &str = "constraint one value: false :- cell(C, A), cell(C, B), A \\= B.";

/// A room with two rents that break a constraint, and definitions made at
/// a third item; the cursor on the second rent, so the right pane is the
/// constraint's.
fn rents_with(name: &str, definitions: &[&str]) -> Room {
    rents_defined(name, &[definitions])
}

/// A room with two rents that break a constraint, and an item after them
/// for each list of definitions, made there; the cursor on the second
/// rent.
fn rents_defined(name: &str, items: &[&[&str]]) -> Room {
    let mut r = Room::new(name);
    r.items(&["rent", "rent again"]);
    for definitions in items {
        r.items(&["", "defined"]);
        definitions.iter().for_each(|d| r.command(d));
    }
    r.leap(Quasi::LeapBack, "rent again");
    r.command("fact cell(rent, 1300)");
    r.leap(Quasi::LeapBack, "rent");
    r.command("fact cell(rent, 1200)");
    r.leap(Quasi::LeapForward, "rent again");
    r.app.flush();
    r.app.evaluate().unwrap();
    r
}

#[test]
fn a_derived_text_that_reads_the_facts_within_a_construct_depends_on_them() {
    // Catches reads looked for only within ',', ';' and '\+'.
    for (n, goal) in [
        "findall(C, cell(C, _), L), length(L, N)",
        "findall(C, cell(C, _), L, []), length(L, N)",
        "(cell(rent, N) -> true ; N = 0)",
        "(cell(rent, N) *-> true ; N = 0)",
        "N = 1, forall(cell(_, V), V > 0)",
    ]
    .iter()
    .enumerate()
    {
        let derive = format!("derive {{N}} where {goal}");
        let r = rents_with(&format!("deps-{n}"), &[ONE_VALUE, &derive]);
        assert!(r.shows("depends on these facts"), "{goal}");
    }
}

#[test]
fn a_derived_text_that_reads_the_facts_through_a_rule_depends_on_them() {
    // Catches a rule taken as the end of what a definition reads.
    for (n, (rule, goal)) in [
        ("rule costly(C) :- cell(C, V), V > 1000.", "costly(_)"),
        ("rule costly :- cell(_, V), V > 1000.", "costly"),
    ]
    .iter()
    .enumerate()
    {
        let derive = format!("derive {{N}} where {goal}, N = 1");
        let r = rents_with(&format!("deps-rule-{n}"), &[ONE_VALUE, rule, &derive]);
        assert!(r.shows("depends on these facts"), "{rule}");
    }
}

#[test]
fn a_derived_text_that_shares_only_a_builtin_does_not_depend_on_the_facts() {
    // Catches a builtin, which reads no fact, taken as a read.
    let constraint =
        "constraint one value: false :- cell(C, A), cell(C, B), A \\= B, atom_length(C, N), N > 0.";
    let derive = "derive {L} letters where atom_length(word, L)";
    let r = rents_with("deps-builtin", &[constraint, derive]);
    assert!(r.shows("CONSTRAINT · ONE VALUE"));
    assert!(!r.shows("depends on these facts"));
}

/// A room where rent breaks a limit and food keeps it, with the cursor on
/// the rent.
fn over_the_limit(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["rent", "food"]);
    r.command("fact cell(food, 300)");
    r.leap(Quasi::LeapBack, "rent");
    r.command("fact cell(rent, 1200)");
    r.command("constraint limit: false :- cell(C, R), R > 1000.");
    r.app.flush();
    r.app.evaluate().unwrap();
    r
}

#[test]
fn a_finding_on_one_fact_is_marked_under_its_item() {
    // Catches a mark only for a fact against another: a violation of one
    // fact was counted in the top bar and shown nowhere.
    let r = over_the_limit("limit-marked");
    let marks = r
        .lines_at(160, 40)
        .iter()
        .filter(|l| l.contains('⚑'))
        .count();
    assert_eq!(marks, 1);
    assert!(r.shows("⚑ limit, violation: cell(rent, 1200) here"));
    assert!(r.shows("CONSTRAINT · LIMIT"));
}

#[test]
fn a_finding_on_no_fact_is_marked_under_no_item() {
    // Catches a finding with no facts taken as resting on each item's.
    let mut r = Room::new("no-fact-marks");
    r.items(&["rent"]);
    r.command("fact cell(rent, 1200)");
    r.command("constraint always: false :- 2 > 1.");
    assert!(
        r.app.session.message.contains("kept"),
        "{}",
        r.app.session.message
    );
    r.app.flush();
    r.app.evaluate().unwrap();
    assert!(r.shows("1 constraint violation"));
    assert!(!r.shows("⚑"));
}

#[test]
fn a_finding_on_one_fact_is_listed_as_that_fact() {
    // Catches a row of the arguments its facts share and of how they
    // differ, which for one fact is "rent, 1200:  (1)".
    let r = over_the_limit("limit-listed");
    assert!(r.shows("  cell(rent, 1200) (1)"));
}

#[test]
fn a_finding_whose_facts_share_nothing_is_listed_without_a_key() {
    // Catches a row that begins with the key of what its facts share,
    // empty here: ": rent, 1200 (1) · food, 300 (2)".
    let mut r = over_the_limit("no-key");
    r.command("constraint rent first: false :- cell(rent, R), cell(food, F), R > F.");
    r.app.flush();
    r.app.evaluate().unwrap();
    assert!(r.shows("│  rent, 1200 (1) · food, 300 (2)"));
}

#[test]
fn the_constraints_of_an_item_are_drawn_apart() {
    // Catches a constraint's heading drawn straight under the list of
    // what depends on the one before.
    let derive = "derive {N} cells where findall(C, cell(C, _), L), length(L, N)";
    let limit = "constraint limit: false :- cell(C, R), R > 1000.";
    let r = rents_with("drawn-apart", &[ONE_VALUE, limit, derive]);
    let lines = r.lines_at(160, 40);
    let at = lines.iter().position(|l| l.contains("CONSTRAINT · LIMIT"));
    let above = &lines[at.expect("the second constraint") - 1];
    assert_eq!(above.split('│').nth(1).map(str::trim), Some(""));
}

#[test]
fn a_kind_with_no_findings_has_no_heading() {
    // Catches the heading of each kind drawn whether or not it has rows.
    let r = over_the_limit("no-kind");
    assert!(r.shows("violations · recomputed at journal"));
    assert!(!r.shows("disagreements between sources ·"));
}

/// A derived text, at an item of its own, that reads the rents.
fn counting(template: &str) -> String {
    format!("derive {template} where findall(C, cell(C, _), L), length(L, N)")
}

#[test]
fn what_depends_on_the_facts_is_listed_in_outline_order() {
    // Catches the derived items listed in the order the memex holds them.
    let first = counting("{N} first");
    let second = counting("{N} second");
    let r = rents_defined("deps-order", &[&[ONE_VALUE, &first], &[&second]]);
    let lines = r.lines_at(160, 40);
    let at = |text| lines.iter().position(|l| l.contains(text));
    assert!(at("  3 2 first") < at("  4 2 second"));
    assert!(at("  3 2 first").is_some());
}

#[test]
fn a_derived_text_longer_than_40_characters_is_cut() {
    // Catches a cut at another length: the first text has 40 characters,
    // the second 41.
    let forty = counting("{N} cells are stated in this tiny notebook");
    let more = counting("{N} cells are stated in this tiny notebook.");
    let r = rents_defined("deps-cut", &[&[ONE_VALUE, &forty], &[&more]]);
    assert!(r.shows("  3 2 cells are stated in this tiny notebook"));
    assert!(!r.shows("  3 2 cells are stated in this tiny notebook …"));
    assert!(r.shows("  4 2 cells are stated in this tiny notebook …"));
}

#[test]
fn a_derived_text_with_no_answer_is_listed_as_the_outline_shows_it() {
    // Catches the text of a derived item taken from its answer alone: a
    // goal that raises an error has none.
    let derive = "derive {N} broken where cell(rent, R), N is R + foo";
    let r = rents_with("deps-no-answer", &[ONE_VALUE, derive]);
    assert!(r.shows("  3 defined"));
}

#[test]
fn a_finding_whose_facts_share_two_arguments_is_keyed_by_both() {
    // Catches a key of the first shared argument alone.
    let one_price = "constraint one price: false :- price(I, M, A), price(I, M, B), A \\= B.";
    let mut r = rents_with("two-keys", &[one_price]);
    r.command("fact price(rent, june, 1300)");
    r.leap(Quasi::LeapBack, "rent");
    r.command("fact price(rent, june, 1200)");
    r.app.flush();
    r.app.evaluate().unwrap();
    assert!(r.shows("rent, june: 1200 (1) · 1300 (2)"));
}
