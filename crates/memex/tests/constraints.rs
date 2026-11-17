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
    let mut r = Room::new(name);
    r.items(&["rent", "rent again", "defined"]);
    definitions.iter().for_each(|d| r.command(d));
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
