//! How an item is drawn in the outline: its text as typed, the text it
//! shows from other items.

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
fn a_nested_item_shows_its_address_and_the_cursor_follows_it() {
    // Catches an address drawn at the top level only: the foot, the menus
    // and the commands quote "1.1", which the outline then never showed;
    // and the text cursor placed as if no address came before the text.
    use ratatui::{Terminal, backend::Backend, backend::TestBackend};
    let mut r = Room::new("nested-address");
    r.items(&["Alpha", "Beta"]);
    r.leap(Quasi::LeapUp, "Beta");
    r.tap(&[memex::app::Key::Tab]);
    assert!(r.shows("• 1.1. Beta"), "{:?}", r.lines());
    let mut t = Terminal::new(TestBackend::new(160, 40)).unwrap();
    t.draw(|f| memex::ui::draw(f, &r.app)).unwrap();
    let at = t.backend_mut().get_cursor_position().unwrap();
    let (x, y) = r.find("1.1. Beta");
    assert_eq!((at.x, at.y), (x + 9, y), "after \"1.1. Beta\"");
}

#[test]
fn a_folded_item_shows_how_many_items_it_hides() {
    // Catches a folded item drawn as open, or without its count.
    let mut r = Room::new("folded");
    r.items(&["Alpha", "Beta", "Gamma"]);
    r.leap(Quasi::LeapUp, "Gamma");
    r.tap(&[memex::app::Key::Tab]);
    r.leap(Quasi::LeapUp, "Beta");
    r.tap(&[memex::app::Key::Tab]);
    r.leap(Quasi::LeapUp, "Alpha");
    r.command("fold");
    assert!(r.shows("▶ 1. Alpha 2"), "{:?}", r.lines());
}

#[test]
fn the_cursors_line_keeps_its_colours_on_a_tinted_ground() {
    // Catches the cursor's line reversed, which drew read text as a cyan
    // block, prose as a white one, and a source's dim name on light grey.
    let mut r = Room::new("cursor-ground");
    r.items(&["Alpha", "Beta"]);
    let at = r.style_of("2. Beta");
    assert_eq!(at.bg, memex::ui::cursor().bg, "the cursor's line is tinted");
    assert!(!at.add_modifier.contains(Modifier::REVERSED));
    assert_eq!(
        at.fg,
        r.style_of("1. Alpha").fg,
        "its text keeps its colour"
    );
    assert_ne!(
        r.style_of("1. Alpha").bg,
        at.bg,
        "other lines are not tinted"
    );
    let (x, y) = r.find("2. Beta");
    let far = r.screen(160, 40)[(x + 40, y)].style();
    assert_eq!(
        far.bg,
        memex::ui::cursor().bg,
        "the tint runs past the text"
    );
}

#[test]
fn an_items_marks_are_set_in_under_it() {
    // Catches a mark drawn at another indent than one level in from its
    // item.
    let mut r = Room::new("marks-indent");
    r.items(&["rent"]);
    r.command("fact box(rent, 1200)");
    r.command("constraint limit: false :- box(C, R), R > 1000.");
    r.app.evaluate().unwrap();
    let line = "    ‼ breaks “limit”: box(rent, 1200) here";
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
    r.command("layer bar(x(C), y(V)) where box(C, V)");
    r.leap(Quasi::LeapUp, "rent");
    r.command("fact box(rent, 1200)");
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
fn each_cell_of_a_mark_in_a_view_presents_its_fact() {
    // Catches a view's places not moved by the view's indent, or a mark
    // presenting its item rather than the fact the item states.
    let r = charted("view-hits");
    let rent = r.app.notebook.outline.in_order()[0];
    let entry = fact_on(&r, rent);
    let fact = memex::present::Presentation::Fact {
        pane: 0,
        entry: entry.expect("rent's fact"),
        item: rent,
    };
    let (bar, hits) = (bar_cells(&r), r.hits());
    assert!(
        bar.iter().all(|(x, y)| hits.at(*x, *y) == Some(&fact)),
        "{bar:?}"
    );
}

/// The entry of a fact an item states, if it states one.
fn fact_on(r: &Room, item: u64) -> Option<u64> {
    let stated = r.outline().fact_item.iter().find(|(_, i)| **i == item);
    stated.map(|(entry, _)| *entry)
}

/// Where a bar is drawn on a wide screen: each cell of its top row.
fn bar_cells(r: &Room) -> Vec<(u16, u16)> {
    let lines = r.lines_at(160, 40);
    let y = lines.iter().position(|l| l.contains('█')).unwrap();
    let cells = lines[y].chars().enumerate().filter(|(_, c)| *c == '█');
    cells.map(|(x, _)| (x as u16, y as u16)).collect()
}

/// The screen, 80 by 12, with the cursor on the first item.
fn at_the_first(r: &mut Room) -> Vec<String> {
    let first = r.app.notebook.outline.in_order()[0];
    r.put_cursor(first);
    r.lines_at(80, 12)
}

/// A room of thirty items, the cursor on the last.
fn thirty_items(name: &str) -> Room {
    let mut r = Room::new(name);
    let texts: Vec<String> = (1..=30).map(|i| format!("item {i}")).collect();
    r.items(&texts.iter().map(String::as_str).collect::<Vec<_>>());
    r
}

#[test]
fn an_outline_longer_than_its_pane_says_how_many_rows_are_out_of_view() {
    // Catches rows cut off at the pane's foot, or above it, with no sign.
    let mut r = thirty_items("more-rows");
    let lines = r.lines_at(80, 12);
    assert!(
        lines.iter().any(|l| l.contains("↑ 26 items above")),
        "{lines:#?}"
    );
    let lines = at_the_first(&mut r);
    assert!(
        lines
            .iter()
            .any(|l| l.contains("↓") && l.contains("items below")),
        "{lines:#?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("↑")),
        "nothing above the first"
    );
    let short = Room::new("no-more");
    let shown = short.lines_at(80, 12);
    assert!(
        !shown.iter().any(|l| l.contains("↓") || l.contains("↑")),
        "{shown:#?}"
    );
}

#[test]
fn an_empty_notebook_says_how_to_begin() {
    // Catches an empty notebook that names no key, or names the palette's
    // default key when the configuration moved it.
    let mut r = Room::new("first-steps");
    assert!(r.shows("Type to write the first item."));
    assert!(
        r.shows("tap Left Ctrl (Command), type a command, press Enter to run it · F1 lists them")
    );
    assert!(r.shows("THE NOTEBOOK · 0 OPEN OFFERS"), "{:#?}", r.lines());
    assert!(r.shows("nothing waits for you to decide"));
    assert!(r.shows("right-click anything for what can act on it"));
    r.bind("F1 = \"\"\nF12 = \"palette\"");
    assert!(r.shows("· F12 lists them"), "{:#?}", r.lines());
    r.items(&["first"]);
    assert!(
        !r.shows("right-click anything"),
        "gone once there is an item"
    );
}

#[test]
fn the_first_steps_name_the_keys_configured() {
    // Catches the hints naming the default keys after the configuration
    // moved them: here Suppose to Right Alt, Leap to Right Ctrl, and
    // Command to F11.
    let mut r = Room::new("first-steps-keys");
    let toml = "[quasimodes]\nsuppose = [\"RightAlt\", \"IsoLevel3Shift\"]\nleap_down = [\"RightControl\"]\ncommand = [\"F11\"]\n";
    r.app.session.keys = memex::config::Config::parse(toml).unwrap().keys();
    assert!(
        r.shows("tap F11 (Command), type a command"),
        "{:#?}",
        r.lines()
    );
    assert!(
        r.shows("hold Right Ctrl (Leap) and type"),
        "{:#?}",
        r.lines()
    );
    assert!(r.shows("a mistake? tap F11, type undo"), "{:#?}", r.lines());
}

#[test]
fn a_heading_has_no_prose_hint_and_a_sentence_has_one() {
    // Catches "prose: not ACE" under every heading and name, which said
    // nothing worth a row, or under no sentence.
    let mut r = Room::new("prose-hint");
    r.items(&["Spending chart"]);
    r.app.idle();
    assert!(!r.shows("prose:"), "{:#?}", r.lines());
    r.items(&["Buran flew once, unmanned."]);
    r.app.idle();
    assert!(r.shows("prose: not ACE"), "{:#?}", r.lines());
}
