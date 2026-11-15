//! The marks under an item: what the constraints' findings say against
//! its facts (memex::ui::item_marks).

mod support;
use support::Room;

/// Three items, each stating a cell, and a constraint that the three
/// break together, evaluated.
fn three_cells() -> Room {
    let mut room = Room::new("marks-three");
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
    let room = three_cells();
    let marks = [
        "sum, violation: a, 1 here, b, 2 in 2",
        "sum, violation: a, 1 here, c, 9 in 3",
    ];
    assert_eq!(first_marks(&room), marks);
}
