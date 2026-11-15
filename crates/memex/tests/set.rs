//! Setting a fact: it replaces the beliefs it directly contradicts under a
//! constraint, tried first in a spare engine (docs/plan.md, Milestone 4).

mod support;

use support::Room;

#[test]
fn a_fact_with_nowhere_to_go_is_refused() {
    let mut r = Room::new("set-nowhere");
    r.command("set date(f, 1)");
    assert_eq!(r.app.session.message, "there is no item to put the fact on");
}

#[test]
fn a_fact_set_against_a_claim_replaces_nothing() {
    // A conflict with a claim is a disagreement, not a violation.
    let mut r = Room::new("set-claim");
    r.items(&["notes"]);
    r.command("constraint one date: false :- date(E, D1), date(E, D2), D1 \\= D2.");
    r.command("claim date(f, 1988) from talk");
    r.command("set date(f, 1989)");
    assert_eq!(r.app.session.message, "set at 1; nothing replaced");
}
