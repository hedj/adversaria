//! History (docs/concepts-seven.md, change 3): the notebook's inspector
//! ends with the latest changes and jumps of this branch, newest first,
//! each by its entry's number.

mod support;

use memex::app::{Key, Mode, Quasi};
use memex::present::Presentation;
use ratatui::style::Modifier;
use support::Room;

/// Three items, the third a sentence, read; a link from the first to the second, a jump to
/// the third and back; the cursor on the notebook.
fn walked(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Alder", "Birch", "The rent is 1200."]);
    r.app.idle();
    r.command("go 1");
    r.command("link 2");
    r.command("go 3");
    r.command("go back");
    r.command("go parent");
    r
}

/// The history's rows: what follows its heading, right of the rule.
fn history(r: &Room) -> Vec<String> {
    let lines = r.lines_at(160, 50);
    let right = lines
        .iter()
        .filter_map(|l| l.split_once('│').map(|(_, r)| r.trim().to_string()));
    let after = right.skip_while(|l| l != "HISTORY · NEWEST FIRST").skip(1);
    after.filter(|l| !l.is_empty()).collect()
}

/// The open menu's offers, by command line.
fn offered(r: &Room) -> Vec<String> {
    match &r.app.session.mode {
        Mode::Menu(m) => m.offers.iter().map(|o| o.line.clone()).collect(),
        _ => vec![],
    }
}

/// A right-click on the history's row that starts so: its offers.
fn right_click_on(r: &mut Room, start: &str) -> Vec<String> {
    let at = r.find(start);
    let hits = r.hits();
    r.app.right_click(&hits, at, "entry", false);
    offered(r)
}

#[test]
fn the_history_lists_changes_and_jumps_newest_first() {
    // Catches a jump or a change left out, or the oldest first.
    let r = walked("history-order");
    let shown = history(&r);
    let at = |t: &str| {
        shown
            .iter()
            .position(|l| l.contains(t))
            .unwrap_or_else(|| panic!("{t}: {shown:#?}"))
    };
    assert!(at("went back to 1: “Alder” from 3") < at("went to 3: “The rent is 1200.” from 1"));
    assert!(at("went to 3: “The rent is 1200.” from 1") < at("linked 1: “Alder” to 2: “Birch”"));
    assert!(at("linked 1") < at("added 3: “The rent is 1200.”"));
    assert!(shown.iter().all(|l| l.starts_with("entry ")), "{shown:#?}");
    assert!(
        ops(&r, "read") > 0 && ops(&r, "facts") > 0,
        "the sentence was read"
    );
    let reading = |l: &&String| l.contains("stated facts") || l.contains("read");
    assert!(
        !shown.iter().any(|l| reading(&l)),
        "a reading follows its edit: {shown:#?}"
    );
}

/// How many entries of the journal have an op.
fn ops(r: &Room, op: &str) -> usize {
    let entries = r.app.journal().entries();
    entries
        .iter()
        .filter(|e| e.body().is_some_and(|b| b["op"] == op))
        .count()
}

#[test]
fn the_history_lists_eight_and_says_when_there_is_none() {
    let r = walked("history-eight");
    assert_eq!(history(&r).len(), 8, "{:#?}", history(&r));
    let empty = Room::new("history-none");
    assert_eq!(history(&empty), vec!["nothing done yet"]);
}

/// The entry number of the history's row that holds a text.
fn entry_of(r: &Room, text: &str) -> String {
    let row = history(r).into_iter().find(|l| l.contains(text)).unwrap();
    row.split_whitespace().nth(1).unwrap().to_string()
}

#[test]
fn a_right_click_on_a_change_retracts_it() {
    let mut r = walked("history-retract");
    let entry = entry_of(&r, "linked 1");
    let offers = right_click_on(&mut r, &format!("entry {entry} "));
    assert_eq!(offers, vec![format!("retract entry {entry}")]);
    r.tap(&[Key::Enter]);
    r.tap(&[Key::Enter]);
    let alder = r.outline().in_order()[0];
    let links = r.outline().links_of(alder).0.len();
    assert_eq!(links, 0, "{}", r.app.session.message);
}

#[test]
fn a_change_retracted_is_dim_and_offers_nothing() {
    let mut r = walked("history-retracted");
    let entry = entry_of(&r, "linked 1");
    r.command(&format!("retract entry {entry}"));
    r.tap(&[Key::Enter]);
    let row = format!("entry {entry} ");
    let shown = history(&r)
        .into_iter()
        .find(|l| l.starts_with(&row))
        .unwrap();
    assert!(shown.ends_with("  (retracted)"), "{shown}");
    assert!(r.style_of(&shown).add_modifier.contains(Modifier::DIM));
    assert!(
        right_click_on(&mut r, &shown).is_empty(),
        "nothing to retract again"
    );
}

#[test]
fn a_right_click_on_a_jump_goes_back_to_where_it_went() {
    let mut r = walked("history-jump");
    let row = history(&r)
        .into_iter()
        .find(|l| l.contains("went to 3"))
        .unwrap();
    assert_eq!(right_click_on(&mut r, &row[..12]), vec!["go 3".to_string()]);
    r.tap(&[Key::Enter]);
    assert_eq!(r.text(r.app.cursor()), "The rent is 1200.");
}

#[test]
fn pointing_at_a_step_supplies_its_entry_to_the_line() {
    // Catches an entry's number that must be read off and typed.
    let mut r = walked("history-point");
    let row = history(&r)
        .into_iter()
        .find(|l| l.contains("linked 1"))
        .unwrap();
    let entry = row.split_whitespace().nth(1).unwrap().to_string();
    r.tap(&[Key::Quasi(Quasi::Command)]);
    r.typ("retract ");
    let at = r.find(&row[..12]);
    let hits = r.hits();
    assert_eq!(
        hits.at(at.0, at.1),
        Some(&Presentation::Entry(entry.parse().unwrap()))
    );
    r.app.click_at(&hits, at);
    let line = match &r.app.session.mode {
        Mode::Quasi(a) => a.input.clone(),
        other => format!("{other:?}"),
    };
    assert_eq!(line, format!("retract {entry} "));
}

#[test]
fn facts_stated_by_hand_are_a_step_and_a_reading_s_are_not() {
    // Catches every facts entry left out, as a reading's are, so a fact
    // stated by command never showed.
    let mut r = walked("history-facts");
    r.command("go 2");
    r.command("fact box(birch, 3)");
    r.command("go parent");
    let shown = history(&r);
    assert!(
        shown[0].contains("stated facts in 2: “Birch”"),
        "{shown:#?}"
    );
}

#[test]
fn a_retraction_s_own_row_offers_nothing() {
    // Catches retract offered for an entry that is no change, as a
    // retraction, an undo or a redo is: the command would refuse it.
    let mut r = walked("history-retraction");
    let entry = entry_of(&r, "linked 1");
    r.command(&format!("retract entry {entry}"));
    r.tap(&[Key::Enter]);
    let said = format!("retracted entry {entry}");
    let row = history(&r).into_iter().find(|l| l.contains(&said)).unwrap();
    assert!(right_click_on(&mut r, &row).is_empty(), "{row}");
}
