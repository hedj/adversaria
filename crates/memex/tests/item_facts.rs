//! An item's statements hold only while the item does: deleting it takes
//! its facts out of its world, and undoing the deletion puts them back
//! (docs/design.md, The journal format: an entry's facts hold while each
//! item it names is in the world).

mod support;

use memex::app::App;
use support::{Room, replayed};

/// The facts of the world at the tip, as evaluated, shown.
fn facts(app: &mut App) -> Vec<String> {
    app.idle();
    app.evaluate().unwrap();
    let e = app.reason.eval[0].as_ref().unwrap();
    e.facts
        .iter()
        .map(|(_, t)| memex::reason::show(t))
        .collect()
}

/// Whether a fact about the flight stands.
fn dated(app: &mut App) -> bool {
    facts(app).contains(&"date(buran_flight, 1988)".to_string())
}

#[test]
fn deleting_an_item_takes_its_facts_out_and_undo_puts_them_back() {
    // Catches a deleted item's facts kept in force, unseen, as they were.
    let mut r = Room::new("item-facts-delete");
    r.items(&["The date of Buran-flight is 1988.", "Other"]);
    assert!(dated(&mut r.app), "read");
    r.command("go 1");
    r.command("delete");
    assert!(!dated(&mut r.app), "deleted");
    r.command("undo");
    assert!(dated(&mut r.app), "undone");
}

#[test]
fn deleting_a_branch_takes_the_facts_of_the_items_under_it() {
    let mut r = Room::new("item-facts-branch");
    r.items(&["Flights", "The date of Buran-flight is 1988.", "Other"]);
    r.command("go 2");
    r.tap(&[memex::app::Key::Tab]);
    assert!(dated(&mut r.app), "read, under 1");
    r.command("go 1");
    r.command("delete");
    assert!(!dated(&mut r.app));
}

#[test]
fn a_link_s_fact_goes_with_either_end() {
    let mut r = Room::new("item-facts-link");
    r.items(&["Alder", "Birch", "Cedar"]);
    r.command("go 1");
    r.command("link 2");
    r.command("go 3");
    r.command("link 2");
    let links = |app: &mut App| facts(app).iter().filter(|f| f.starts_with("link(")).count();
    assert_eq!(links(&mut r.app), 2);
    r.command("go 1");
    r.command("delete");
    assert_eq!(links(&mut r.app), 1, "the link from the item deleted");
    r.command("go 1");
    r.command("delete");
    assert_eq!(links(&mut r.app), 0, "the link to the item deleted");
}

#[test]
fn a_finding_goes_with_the_item_it_rests_on() {
    // Catches the top bar still counting a disagreement after the item
    // that made it is gone.
    let mut r = Room::new("item-facts-finding");
    r.items(&[
        "The date of Buran-flight is 1988.",
        "Talk-notes says that the date of Buran-flight is 1989.",
        "Nothing has more than 1 date.",
    ]);
    facts(&mut r.app);
    assert!(
        r.shows("1 disagreement between sources"),
        "{:#?}",
        r.lines()
    );
    r.command("go 1");
    r.command("delete");
    facts(&mut r.app);
    assert!(!r.shows("disagreement"), "{:#?}", r.lines());
}

#[test]
fn a_journal_reopened_reads_a_deleted_item_s_facts_as_gone() {
    // Catches the rule kept to the session, so a journal opened again, or
    // one written before the rule, kept a deleted item's facts.
    let mut r = Room::new("item-facts-replay");
    r.items(&["The date of Buran-flight is 1988.", "Other"]);
    r.app.idle();
    r.command("go 1");
    r.command("delete");
    r.app.flush();
    let mut again = replayed(&r.dir);
    assert!(!dated(&mut again));
}
