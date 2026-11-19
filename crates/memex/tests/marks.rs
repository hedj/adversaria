//! The marks under an item: what the constraints' findings say against
//! its facts (memex::ui::item_marks).

mod support;
use support::Room;

/// Three items, each stating a cell, and a constraint that the three
/// break together, evaluated.
fn three_cells(name: &str) -> Room {
    let mut room = Room::new(name);
    room.items(&["a"]);
    room.command("fact cell(a, 1)");
    for (name, cell) in [("b", "cell(b, 2)"), ("c", "cell(c, 9)")] {
        room.items(&["", name]);
        room.command(&format!("fact {cell}"));
    }
    room.command("constraint sum: false :- cell(a, X), cell(b, Y), cell(c, Z), X + Y < Z.");
    room.app.evaluate().unwrap();
    room
}

/// The text of each mark under the first item.
fn first_marks(room: &Room) -> Vec<String> {
    let outline = &room.app.notebook.outline;
    let eval = room.app.reason.eval[0].as_ref().unwrap();
    let marks = memex::ui::item_marks(eval, outline, outline.in_order()[0]);
    marks.into_iter().map(|(_, text)| text).collect()
}

#[test]
fn an_items_marks_name_each_other_fact_of_a_finding_once() {
    let room = three_cells("marks-each");
    // One mark for the rule the item breaks, naming each other fact,
    // not one mark for each.
    let marks = ["breaks “sum”: a 1 here; b 2 in 2; c 9 in 3"];
    assert_eq!(first_marks(&room), marks);
}

#[test]
fn findings_of_one_rule_on_an_item_are_one_mark() {
    // Catches a mark for each solution of a rule: three dates for one
    // event are two disagreements on the first, shown as one mark.
    let mut room = Room::new("marks-merged");
    room.items(&["first"]);
    room.command("fact date(e, 1988)");
    for (name, year) in [("talk", 1989), ("book", 1990)] {
        room.items(&["", name]);
        room.command(&format!("claim date(e, {year}) from {name}"));
    }
    room.command("constraint one date: false :- date(E, A), date(E, B), A \\== B.");
    room.app.evaluate().unwrap();
    let marks = ["sources disagree under “one date”: 1988 here; 1989 in 2; 1990 in 3"];
    assert_eq!(first_marks(&room), marks);
}

#[test]
fn an_item_stating_two_facts_of_a_finding_is_marked_by_its_first() {
    let mut room = three_cells("marks-first");
    room.put_cursor(room.app.notebook.outline.in_order()[0]);
    room.command("fact cell(b, 5)");
    room.app.evaluate().unwrap();
    let marks = first_marks(&room);
    assert!(marks.iter().all(|m| m.contains(": a 1 here")), "{marks:?}");
}
