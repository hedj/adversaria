//! How an item is drawn in the outline: its text as typed, the text it
//! shows from other items, and the word scope.

use memex::app::Quasi;
use ratatui::style::Modifier;

mod support;
use support::Room;

/// Whether a text is first drawn with these modifiers.
fn drawn(room: &Room, text: &str, m: Modifier) -> bool {
    room.style_of(text).add_modifier.contains(m)
}

#[test]
fn an_item_shows_its_text_as_typed_before_it_is_written() {
    let mut room = Room::new("item-typed");
    room.items(&["Alpha"]);
    room.typ(" beta");
    assert!(room.shows("Alpha beta"));
}

#[test]
fn a_transcluded_text_is_marked_with_the_item_it_is_from() {
    let mut room = Room::new("item-transcluded");
    room.items(&["Alpha", "Beta"]);
    room.command("transclude 1");
    assert!(!drawn(&room, "Alpha", Modifier::ITALIC), "the item's own");
    assert!(drawn(
        &room,
        "Alpha↖1",
        Modifier::ITALIC | Modifier::UNDERLINED
    ));
    assert!(drawn(&room, "↖1", Modifier::DIM));
}

#[test]
fn holding_scope_underlines_the_items_text_as_its_target() {
    let mut room = Room::new("item-scope");
    room.items(&["Alpha", "Beta"]);
    room.command("pane suggestions"); // the links pane would show Beta too
    assert!(!drawn(&room, "Beta", Modifier::UNDERLINED));
    room.hold(Quasi::Scope, "");
    assert!(drawn(&room, "Beta", Modifier::UNDERLINED), "at the cursor");
    assert!(!drawn(&room, "Alpha", Modifier::UNDERLINED), "elsewhere");
}

#[test]
fn an_items_source_is_drawn_before_what_it_cites() {
    let mut room = Room::new("item-notes");
    room.items(&["claim", "paper"]);
    room.put_cursor(room.app.notebook.outline.in_order()[0]);
    room.command("claim said(a) from Smith 2020");
    room.command("link 1 2 as cites");
    assert!(
        room.shows("claim [Smith 2020] [paper]"),
        "{}",
        room.app.session.message
    );
}

#[test]
fn the_text_cursor_counts_the_cells_of_wide_characters() {
    // Catches the cursor placed by characters: each of 日本語 takes two
    // cells, so the cursor at the end of "日本語 text" fell on the "e".
    use ratatui::{Terminal, backend::Backend, backend::TestBackend};
    let mut r = Room::new("wide-cursor");
    r.items(&["日本語 text"]);
    let mut t = Terminal::new(TestBackend::new(80, 10)).unwrap();
    t.draw(|f| memex::ui::draw(f, &r.app)).unwrap();
    let at = t.backend_mut().get_cursor_position().unwrap();
    let (x, y) = r.find("日");
    assert_eq!((at.x, at.y), (x + 6 + 5, y), "after 日本語 and \" text\"");
}

#[test]
fn a_folded_item_shows_how_many_items_it_hides() {
    // Catches a folded item drawn as open, or without its count.
    let mut r = Room::new("folded");
    r.items(&["Alpha", "Beta", "Gamma"]);
    r.leap(Quasi::LeapBack, "Gamma");
    r.tap(&[memex::app::Key::Tab]);
    r.leap(Quasi::LeapBack, "Beta");
    r.tap(&[memex::app::Key::Tab]);
    r.leap(Quasi::LeapBack, "Alpha");
    r.command("fold");
    assert!(r.shows("▶ 1. Alpha 2"), "{:?}", r.lines());
}

#[test]
fn the_cursors_line_is_bold_too_in_branch_scope() {
    // Catches the branch scope drawn as the statement scope is.
    let mut r = Room::new("branch-bold");
    r.items(&["Alpha"]);
    assert!(!drawn(&r, "1. Alpha", Modifier::BOLD));
    r.app.session.view.scope = memex::app::ScopeLevel::Branch;
    assert!(drawn(&r, "1. Alpha", Modifier::BOLD | Modifier::REVERSED));
}

#[test]
fn an_items_marks_are_set_in_under_it() {
    // Catches a mark drawn at another indent than one level in from its
    // item.
    let mut r = Room::new("marks-indent");
    r.items(&["rent"]);
    r.command("fact cell(rent, 1200)");
    r.command("constraint limit: false :- cell(C, R), R > 1000.");
    r.app.evaluate().unwrap();
    let line = "    ⚑ limit, violation: cell(rent, 1200) here";
    assert!(
        r.lines().iter().any(|l| l.starts_with(line)),
        "{:?}",
        r.lines()
    );
}

/// A room whose rent and food a bar chart under a top-level item shows.
fn charted(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["rent", "chart"]);
    r.command("layer bar(x(C), y(V)) where cell(C, V)");
    r.leap(Quasi::LeapBack, "rent");
    r.command("fact cell(rent, 1200)");
    r.app.evaluate().unwrap();
    r
}

#[test]
fn a_view_under_its_item_is_as_wide_as_the_pane_leaves_it() {
    // Catches a view drawn narrower than the pane, less its indent and a
    // column.
    let r = charted("view-width");
    let lines = r.lines_at(160, 40);
    let axis: Vec<char> = lines
        .iter()
        .find(|l| l.contains('└'))
        .unwrap()
        .chars()
        .collect();
    let last = axis.iter().rposition(|c| *c == '─').unwrap();
    let divider = axis.iter().position(|c| *c == '│').unwrap();
    assert_eq!(last + 2, divider);
}

#[test]
fn each_cell_of_a_mark_in_a_view_is_a_click_on_its_facts_item() {
    // Catches a view's places not moved by the view's indent.
    let r = charted("view-hits");
    let rent = r.app.notebook.outline.in_order()[0];
    let rent = memex::app::Click::Item { pane: 0, id: rent };
    let (bar, hits) = (bar_cells(&r), r.hits());
    assert!(
        bar.iter().all(|(x, y)| hits.at(*x, *y) == Some(&rent)),
        "{bar:?}"
    );
}

/// Where a bar is drawn on a wide screen: each cell of its top row.
fn bar_cells(r: &Room) -> Vec<(u16, u16)> {
    let lines = r.lines_at(160, 40);
    let y = lines.iter().position(|l| l.contains('█')).unwrap();
    let cells = lines[y].chars().enumerate().filter(|(_, c)| *c == '█');
    cells.map(|(x, _)| (x as u16, y as u16)).collect()
}
