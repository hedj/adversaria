//! Setting a fact: it replaces the beliefs it directly contradicts under a
//! constraint, tried first in a spare engine (docs/plan.md, Milestone 4).

mod support;

use support::Room;

#[test]
fn a_fact_with_nowhere_to_go_is_refused() {
    let mut r = Room::new("set-nowhere");
    r.refused("set date(f, 1)");
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

#[test]
fn adopt_instead_retracts_the_belief_a_claim_contradicts() {
    // Catches `set`'s job lost with it: adopting a claim in place of the
    // belief it contradicts, the claim's source kept (docs/concepts.md).
    let mut r = Room::new("set-adopt-instead");
    r.items(&["our date", "the talk's date"]);
    let [ours, talk] = [0, 1].map(|i| r.outline().in_order()[i]);
    r.command("constraint one date: false :- date(E, D1), date(E, D2), D1 \\= D2.");
    r.put_cursor(ours);
    r.command("fact date(f, 1988)");
    r.put_cursor(talk);
    r.command("claim date(f, 1989) from talk");
    r.command("adopt instead");
    assert!(
        r.app.session.message.starts_with("adopted"),
        "{}",
        r.app.session.message
    );
    let facts = r.facts();
    assert!(!facts.iter().any(|f| f == "date(f, 1988)"), "{facts:?}");
    assert!(facts.iter().any(|f| f == "date(f, 1989)"), "{facts:?}");
}
