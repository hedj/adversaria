//! Keeping a constraint: it must fire on a planted violation before it is
//! kept (docs/guide.md § 2.4, `constraint`; design.md § Testing claims).

mod support;
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
