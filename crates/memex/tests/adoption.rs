//! Adopting an item's claims as beliefs, with their sources, and taking
//! the adoptions back (docs/guide.md § 1.3, § 2.4).

mod support;
use memex::app::{Key, Mode};
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
fn adopting_claims_again_adds_no_belief_twice() {
    // Catches a second adopt of the same claims: each belief twice, and
    // the source's record counting them twice.
    let mut r = two_claims("adopt-twice");
    r.command("adopt");
    r.refused("adopt");
    assert_eq!(
        r.app.session.message,
        "the target's claims are beliefs already"
    );
    assert_eq!(dates(&mut r), 2);
}

/// `retract` alone, confirmed with Enter: what it retracted.
fn retract_last(r: &mut Room) -> String {
    r.command("retract");
    r.tap(&[Key::Enter]);
    r.app.session.message.clone()
}

#[test]
fn retract_alone_takes_back_each_adoption_in_turn() {
    // Catches `retract` alone stopping at the last adopt entry once it is
    // retracted, so that an adoption before it could never be taken back;
    // after the adoptions it offers the item's own last claim.
    let mut r = two_claims("unadopt-each");
    let claims = seqs(&r, |e| {
        e.kind() == "edit" && e.body().is_some_and(|b| b["source"].is_string())
    });
    r.command("adopt");
    let adoptions = seqs(&r, |e| e.kind() == "adopt");
    assert_eq!(adoptions.len(), 2, "one adoption a claim");
    for a in adoptions.iter().rev() {
        let said = retract_last(&mut r);
        assert!(said.starts_with(&format!("entry {a} retracted")), "{said}");
    }
    assert_eq!(dates(&mut r), 0, "both adoptions taken back");
    r.command("retract");
    let asks = |t: u64| matches!(r.app.session.mode, memex::app::Mode::ConfirmRetract { target, .. } if target == t);
    assert!(
        asks(claims[1]),
        "then the item's last claim: {:?}",
        r.app.session.mode
    );
}

/// The entries of the room's journal that pass a test, by seq.
fn seqs(r: &Room, test: impl Fn(&format::record::Entry) -> bool) -> Vec<u64> {
    let entries = r.app.journal().entries();
    entries.iter().filter(|e| test(e)).map(|e| e.seq).collect()
}

#[test]
fn a_claim_taken_back_can_be_adopted_again() {
    let mut r = two_claims("adopt-again");
    r.command("adopt");
    retract_last(&mut r);
    r.command("adopt");
    assert_eq!(
        r.app.session.message,
        "adopted: a belief now, with its source kept"
    );
    assert_eq!(dates(&mut r), 2);
}

/// Whether Command is open, latched, with this line typed: a prompt.
fn asks_with(r: &Room, line: &str) -> bool {
    matches!(&r.app.session.mode, Mode::Quasi(a) if a.latched() && a.input == line)
}

#[test]
fn a_fact_read_from_the_text_is_not_offered_but_a_layer_is() {
    // Catches `retract` alone taking back a statement its item's text was
    // read as, which the text would no longer say; and missing a view's
    // layer, an entry of the item's own.
    let mut r = Room::new("retract-own");
    r.items(&["The date of Buran-flight is 1988.", "chart"]);
    r.app.idle();
    let order = r.outline().in_order();
    r.put_cursor(order[0]);
    r.command("retract");
    let why = "the target has nothing to retract: no adoption, fact or layer of its own";
    assert!(
        r.app.session.message.starts_with(why),
        "{}",
        r.app.session.message
    );
    assert!(asks_with(&r, "retract "), "it asks for the entry");
    r.tap(&[Key::Escape]);
    r.put_cursor(order[1]);
    r.command("layer bar(x(C), y(V)) where box(C, V)");
    let layer = r.outline().views[&order[1]][0].seq;
    r.command("retract");
    let asks = matches!(r.app.session.mode, memex::app::Mode::ConfirmRetract { target, .. } if target == layer);
    assert!(asks, "{:?} (layer {layer})", r.app.session.mode);
}

#[test]
fn retract_takes_an_entry_as_entry_n_and_as_before() {
    // Catches `retract entry 12` refused, its guide word taken for the
    // number, or the old forms lost (docs/concepts.md, decision 7).
    let mut r = two_claims("retract-entry");
    r.command("adopt");
    let adoption = r.last("adopt");
    for line in [
        format!("retract entry {adoption}"),
        format!("retract {adoption}"),
        format!("retract #{adoption}"),
    ] {
        r.command(&line);
        let asks = matches!(r.app.session.mode, memex::app::Mode::ConfirmRetract { target, .. } if target == adoption);
        assert!(asks, "{line}: {:?}", r.app.session.mode);
        r.tap(&[Key::Escape]);
    }
    r.refused("retract entry");
    assert!(
        r.app.session.message.starts_with("retract (entry)"),
        "{}",
        r.app.session.message
    );
}
