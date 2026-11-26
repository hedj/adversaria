//! Stating facts, claims and definitions: what each command says when its
//! text cannot be read (docs/guide.md § 3.1).

mod support;
use support::Room;

#[test]
fn a_statement_that_cannot_be_read_is_told_as_prolog_writes_it() {
    // Catches the error shown as the memex's own data structure:
    // "Compound(\"error\", [Compound(\"syntax_error\", ...".
    let mut r = Room::new("unreadable");
    r.items(&["x"]);
    for (command, said) in [
        (
            "fact box(rent, 1200",
            "not a Prolog fact: syntax_error(operator_expected)",
        ),
        (
            "claim x( from y",
            "not a Prolog fact: syntax_error(end_of_clause)",
        ),
        (
            "derive {N} where foo(",
            "not a Prolog goal: syntax_error(end_of_clause)",
        ),
        (
            "constraint c: false :- box(",
            "not a Prolog rule: syntax_error(end_of_clause)",
        ),
        (
            "rule a(X) :- b(",
            "not a Prolog rule: syntax_error(end_of_clause)",
        ),
        (
            "layer foo where bar(",
            "not a mark and a goal: syntax_error(operator_expected)",
        ),
    ] {
        r.refused(command);
        assert_eq!(r.app.session.message, said, "{command}");
    }
}
