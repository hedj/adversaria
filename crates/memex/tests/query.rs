//! Raw Prolog queries (docs/guide.md § 2.4, query).

mod support;

use support::Room;

/// A room with fish and a rule that every fish is an animal, read.
fn fish(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&[
        "Nemo is a fish.",
        "Dory is a fish.",
        "Every fish is an animal.",
    ]);
    r.app.idle();
    r
}

#[test]
fn a_query_is_answered_through_rules() {
    // Catches a query of the facts alone, missing what a rule derives, or
    // answers without their variables.
    let mut r = fish("query-rule");
    r.command("query animal(X)");
    let said = &r.app.session.message;
    assert!(said.starts_with("2 answers: "), "{said}");
    assert!(
        said.contains("X = nemo") && said.contains("X = dory"),
        "{said}"
    );
}

#[test]
fn a_goal_without_variables_is_answered_yes_or_no() {
    let mut r = fish("query-yes");
    r.command("query fish(nemo).");
    assert_eq!(r.app.session.message, "yes");
    r.command("query fish(bubbles)");
    assert_eq!(r.app.session.message, "no");
}

#[test]
fn a_query_that_would_act_is_refused() {
    // Catches a query that runs what a definition may not: writing a
    // fact, or running a program.
    let mut r = fish("query-refused");
    for goal in ["assertz(fish(bubbles))", "shell(\"ls\")"] {
        r.command(&format!("query {goal}"));
        assert!(
            r.app.session.message.starts_with("not asked"),
            "{goal}: {}",
            r.app.session.message
        );
    }
    r.command("query fish(bubbles)");
    assert_eq!(r.app.session.message, "no", "nothing was asserted");
}

#[test]
fn a_query_shows_at_most_twenty_answers_and_says_so() {
    // Catches an endless or huge query run to its end, or more answers
    // than the message can hold.
    let mut r = fish("query-many");
    r.command("query between(1, 100000000, N)");
    let said = &r.app.session.message;
    assert!(
        said.starts_with("more than 20 answers: N = 1 · N = 2"),
        "{said}"
    );
    assert!(
        said.contains("N = 20 · …") && !said.contains("N = 21"),
        "{said}"
    );
}

#[test]
fn a_query_writes_nothing() {
    let mut r = fish("query-nothing");
    let before = r.entry_count();
    r.command("query animal(X)");
    assert_eq!(r.entry_count(), before);
}
