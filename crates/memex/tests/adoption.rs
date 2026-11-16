//! Adopting an item's claims as beliefs, with their sources, and taking
//! the adoptions back (docs/guide.md § 1.3, § 2.4).

mod support;
use support::Room;

/// An item stating two claims of a date, from two sources.
fn two_claims(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Buran"]);
    r.command("claim date(buran, 1988) from Encyclopedia");
    r.command("claim date(buran, 1989) from Wikipedia");
    r
}

/// How many of the world's facts are beliefs of a date of Buran.
fn dates(r: &mut Room) -> usize {
    r.app.evaluate().unwrap();
    r.facts()
        .iter()
        .filter(|f| f.starts_with("date(buran"))
        .count()
}

#[test]
fn unadopt_takes_back_each_adoption_in_turn() {
    // Catches unadopt stopping at the last adopt entry once it is
    // retracted, so that an adoption before it could never be taken back.
    let mut r = two_claims("unadopt-each");
    r.command("adopt");
    for _ in 0..2 {
        r.command("unadopt");
        assert!(
            r.app.session.message.ends_with("retracted"),
            "{}",
            r.app.session.message
        );
    }
    r.command("unadopt");
    assert_eq!(
        r.app.session.message,
        "the target has no adoption to retract"
    );
    assert_eq!(dates(&mut r), 0);
}
