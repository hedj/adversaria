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
    let marks = [
        "sum, violation: a, 1 here, b, 2 in 2",
        "sum, violation: a, 1 here, c, 9 in 3",
    ];
    assert_eq!(first_marks(&room), marks);
}

#[test]
fn an_item_stating_two_facts_of_a_finding_is_marked_by_its_first() {
    let mut room = three_cells("marks-first");
    room.put_cursor(room.app.notebook.outline.in_order()[0]);
    room.command("fact cell(b, 5)");
    room.app.evaluate().unwrap();
    let marks = first_marks(&room);
    assert!(marks.iter().all(|m| m.contains(": a, 1 here")), "{marks:?}");
}
