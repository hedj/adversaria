//! The suggestion pane: what it lists, and how it says to act on it.

use memex::app::Key;
use memex::present::Presentation;

mod support;
use support::Room;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/wikidata-buran.pl"
);

/// A room with the suggestion pane open, and a dated, labelled flight that
/// Wikidata's import suggests matches for.
fn suggested(name: &str) -> Room {
    let mut room = Room::new(name);
    room.items(&["Buran flew once, unmanned, in 1989."]);
    room.command("fact date(buran_flight, 1989)");
    room.command("fact label(buran_flight, \"Buran's flight\")");
    room.command(&format!("import wikidata {FIXTURE}"));
    room.command("pane suggestions");
    room.app.evaluate().unwrap();
    room
}

#[test]
fn an_empty_pane_says_so_and_how_to_act_on_suggestions() {
    let mut room = Room::new("suggest-none");
    room.command("pane suggestions");
    assert!(room.shows("│  SUGGESTIONS · 0 OF 0"), "inset in its pane");
    assert!(room.shows("│  nothing to suggest"));
    assert!(room.shows("│  adopt N adopts · dismiss N dismisses"));
}

#[test]
fn the_pane_counts_those_shown_of_all() {
    let room = suggested("suggest-some");
    assert!(room.shows("SUGGESTIONS · 3 OF 3"));
    assert!(!room.shows("nothing to suggest"));
}

/// What a click would act on at a column, down some rows.
fn clicks(room: &Room, x: u16, rows: std::ops::Range<u16>) -> Vec<Option<Presentation>> {
    let hits = room.hits();
    rows.map(|y| hits.at(x, y).cloned()).collect()
}

#[test]
fn a_click_on_any_row_of_a_suggestion_wrapped_rows_too_accepts_it() {
    let room = suggested("suggest-wrapped");
    let (x, top) = room.find("suggestion 1  buran_flight");
    let rows = top..room.find("suggestion 2  buran_flight").1;
    assert!(rows.len() > 2, "its evidence wraps");
    let accept = Some(Presentation::Suggestion(1));
    assert!(clicks(&room, x, rows).iter().all(|c| *c == accept));
}

#[test]
fn no_rule_is_mined_from_the_outline_s_own_description_of_itself() {
    // Catches a rule suggested between the user's facts and the facts the
    // memex states of the outline (item_under), which no one stated: it
    // names nothing the user believes, and mining them scanned every item
    // at each change.
    let mut r = Room::new("no-outline-rules");
    r.items(&["Parent", "First", "Second"]);
    let [parent, first, second] = r.outline().in_order()[..] else {
        panic!("three items");
    };
    for child in [first, second] {
        r.put_cursor(child);
        r.tap(&[Key::Tab]);
        r.command(&format!("fact under({child}, {parent})"));
    }
    r.app.evaluate().unwrap();
    let list = &r.app.reason.suggestions.list;
    let mined = list
        .iter()
        .map(|s| s.identity())
        .filter(|s| s.contains("item_under"));
    assert_eq!(mined.collect::<Vec<_>>(), Vec::<String>::new());
}
