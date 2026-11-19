//! The screen as a whole: its parts in each mode, and what covers what
//! (docs/guide.md § 2.2).

use memex::app::{Click, Key, Quasi};

mod support;
use support::{Room, replayed};

#[test]
fn the_foot_shows_a_held_quasimode_in_a_supposition_too() {
    let mut room = Room::new("screen-foot");
    room.command("suppose that w");
    room.hold(Quasi::Command, "help");
    assert!(room.shows(" COMMAND"));
}

#[test]
fn either_leap_lists_its_matches_over_the_outline() {
    let mut room = Room::new("screen-leaps");
    room.items(&["Alpha", "Beta"]);
    for q in [Quasi::LeapForward, Quasi::LeapBack] {
        room.hold(q, "Al");
        assert!(room.shows("┌"), "{q:?}");
        room.release(q);
    }
}

#[test]
fn no_click_lands_on_the_outline_under_a_list() {
    let mut room = Room::new("screen-covered");
    room.items(&["Alpha", "Beta"]);
    let (x, y) = room.find("Alpha");
    assert!(
        room.hits().at(x, y).is_some(),
        "an item's line is clickable"
    );
    room.hold(Quasi::Command, "?");
    assert!(room.hits().at(x, y).is_none());
}

#[test]
fn a_room_a_crash_left_unfinished_opens_and_says_what_was_set_aside() {
    let mut room = Room::new("screen-torn");
    room.items(&["Alpha"]);
    let file = room.dir.join("journal.jsonl");
    let cut = [std::fs::read(&file).unwrap(), br#"{"body":"#.to_vec()].concat();
    std::fs::write(&file, cut).unwrap();
    let opened = replayed(&room.dir);
    assert!(opened.session.message.ends_with("journal.jsonl.torn"));
    assert_eq!(opened.notebook.outline.text(opened.cursor()), "Alpha");
}

#[test]
fn the_cursors_item_stays_in_view_however_short_the_outline() {
    // Catches the lines kept below the cursor pushing the window past the
    // cursor itself, when the outline has room for fewer than three.
    let mut room = Room::new("short-outline");
    room.items(&["Alpha", "Beta", "Gamma", "Delta", "Epsilon"]);
    room.put_cursor(room.app.notebook.outline.in_order()[1]);
    for h in 6..10 {
        let outline: Vec<String> = room
            .lines_at(60, h)
            .iter()
            .map(|l| l.chars().take(30).collect())
            .collect();
        assert!(
            outline.iter().any(|l| l.contains("2. Beta")),
            "height {h}: {outline:?}"
        );
    }
}

/// The foot's two lines, on a screen 140 by 12.
fn foot(room: &Room) -> Vec<String> {
    let lines = room.lines_at(140, 12);
    lines[10..]
        .iter()
        .map(|l| l.trim_end().to_string())
        .collect()
}

#[test]
fn the_foot_asks_before_a_retraction_and_says_when_nothing_depends_on_it() {
    let mut room = Room::new("foot-retract");
    room.items(&["rent"]);
    room.command("fact cell(rent, 3000)");
    let entry = room.last("edit");
    room.command(&format!("retract {entry}"));
    let asked = format!(" RETRACT  entry {entry}");
    let said = "nothing depends on it · Enter retracts · Esc keeps it";
    assert_eq!(foot(&room), [asked.as_str(), said]);
}

#[test]
fn a_latched_quasimode_says_so_and_how_it_ends() {
    let mut room = Room::new("foot-latched");
    room.items(&["rent"]);
    room.tap(&[Key::Quasi(Quasi::Command)]);
    let ends =
        "Enter, or tap again, to run on the target · Esc completes · ? lists what may come next";
    assert_eq!(foot(&room), [" COMMAND (latched)", ends]);
}

#[test]
fn a_message_takes_the_place_of_the_fading_hint() {
    // The hint shows on an item with marks, while there is no message.
    let mut room = Room::new("foot-hint");
    room.items(&["income", "rent"]);
    room.command("fact cell(rent, 3000)");
    room.put_cursor(room.app.notebook.outline.in_order()[0]);
    room.command("fact cell(income, 1000)");
    room.command("constraint within: false :- cell(income, I), cell(rent, R), R > I.");
    room.app.evaluate().unwrap();
    room.app.session.message.clear();
    assert!(
        foot(&room)[0].starts_with("Try a what-if: hold Super to suppose"),
        "{:?}",
        foot(&room)
    );
    assert!(
        !foot(&room)[1].contains("more time"),
        "no count of its showings"
    );
    room.command("outdent");
    assert_eq!(foot(&room)[1], "a top-level item cannot be outdented");
}

#[test]
fn a_held_command_shows_its_guide_words() {
    let mut room = Room::new("foot-guided");
    room.items(&["rent"]);
    room.hold(Quasi::Command, "link");
    assert_eq!(foot(&room)[0], " COMMAND  link (to)");
}

#[test]
fn suppose_offers_the_next_reading_only_when_there_is_one() {
    let mut room = Room::new("foot-readings");
    room.items(&["Buran"]);
    room.command("fact date(buran_flight, 1988)");
    room.command("fact label(buran_flight, \"Buran's flight\")");
    room.leap(Quasi::Suppose, "the flight was in 1990");
    assert!(!foot(&room)[1].contains("Tab next reading"), "one reading");
    room.tap(&[Key::Escape]);
    room.command("fact date(energia_flight, 1987)");
    room.leap(Quasi::Suppose, "the flight was in 1990");
    assert!(foot(&room)[1].ends_with("· Tab next reading"), "two");
}

/// A room of 40 items, with the cursor on the 21st.
fn long_outline(name: &str) -> Room {
    let mut room = Room::new(name);
    let texts: Vec<String> = (1..=40).map(|i| format!("item {i}")).collect();
    room.items(&texts.iter().map(String::as_str).collect::<Vec<_>>());
    room.put_cursor(twenty_first(&room));
    room
}

/// The 21st item of a room's outline.
fn twenty_first(room: &Room) -> u64 {
    room.app.notebook.outline.in_order()[20]
}

#[test]
fn the_two_items_after_the_cursor_stay_in_view() {
    // Catches a window that keeps fewer lines below the cursor's, or an
    // outline built too short to fill it.
    let room = long_outline("two-after");
    let lines = room.lines_at(60, 12);
    for item in ["21. item 21", "22. item 22", "23. item 23"] {
        assert!(lines.iter().any(|l| l.contains(item)), "{item}: {lines:?}");
    }
}

#[test]
fn a_click_lands_on_the_item_drawn_there_when_the_outline_is_scrolled() {
    // Catches places counted from the outline's first line, not the
    // window's.
    let room = long_outline("scrolled-click");
    let y = room
        .lines_at(60, 12)
        .iter()
        .position(|l| l.contains("21. item 21"));
    let click = room.hits_at(60, 12).at(2, y.unwrap() as u16).cloned();
    let id = twenty_first(&room);
    assert_eq!(click, Some(Click::Item { pane: 0, id }));
}

/// The text inside the list's box at 80 by 30, its rows joined by spaces.
fn listed(room: &Room) -> Vec<String> {
    let rows = room.lines_at(80, 30);
    let inside = rows.iter().filter_map(|l| l.strip_prefix('│'));
    inside
        .map(|l| l.split('│').next().unwrap_or("").trim_end().to_string())
        .collect()
}

#[test]
fn the_commands_listed_are_shown_whole_at_80_columns() {
    // Catches a command's line cut off at the box's edge, as each was
    // when the list gave the whole help.
    let mut room = Room::new("screen-list");
    room.hold(Quasi::Command, "c");
    let shown = listed(&room);
    let starting = memex::actions::COMMANDS
        .iter()
        .filter(|c| c.name.starts_with('c'));
    for c in starting {
        let line = format!("{} — {}", c.name, c.brief);
        assert!(shown.contains(&line), "{line:?} not whole in {shown:#?}");
    }
}

#[test]
fn a_list_too_long_for_its_box_says_how_many_more() {
    // Catches commands dropped from the foot of the box without a word.
    let mut room = Room::new("screen-more");
    room.hold(Quasi::Command, "?");
    let shown = listed(&room);
    let commands = shown.iter().filter(|l| l.contains(" — ")).count();
    let more = memex::actions::COMMANDS.len() - commands;
    let last = format!("… and {more} more: type more to narrow the list");
    assert_eq!(shown.last(), Some(&last), "{shown:#?}");
}

#[test]
fn a_chosen_commands_help_is_wrapped_whole() {
    // Catches the full help of a chosen command cut at the box's edge.
    let mut room = Room::new("screen-usage");
    room.hold(Quasi::Command, "constraint ");
    let shown = listed(&room).join(" ");
    let help = memex::actions::COMMANDS
        .iter()
        .find(|c| c.name == "constraint");
    assert!(shown.contains(help.unwrap().help), "{shown}");
}

#[test]
fn every_brief_help_fits_the_list() {
    // Catches a brief help too long for the list at 80 columns.
    for c in memex::actions::COMMANDS {
        assert!(c.brief.chars().count() <= 32, "{}: {}", c.name, c.brief);
    }
}

const BURAN: &str = "Buran flew once, unmanned, on 15 November 1988, from Baikonur, after many delays, and never flew again because the programme was cancelled in 1993.";

/// The outline's column at 80 by 30: each row up to the pane's border.
fn outline_rows(room: &Room) -> Vec<String> {
    let rows = room.lines_at(80, 30).into_iter().skip(2);
    let left = |l: String| l.split('│').next().unwrap_or("").trim_end().to_string();
    rows.map(left).collect()
}

/// The outline's rows from the one that holds a text, joined by spaces as
/// a reader joins wrapped rows.
fn read_from(room: &Room, start: &str) -> String {
    let rows = outline_rows(room);
    let at = rows.iter().position(|r| r.contains(start)).expect(start);
    let words = rows[at..]
        .iter()
        .flat_map(|r| r.split_whitespace().map(String::from));
    words.collect::<Vec<_>>().join(" ")
}

#[test]
fn a_long_item_wraps_whole_under_its_text() {
    // Catches an item cut at the pane's edge, or its later rows set back
    // to the edge rather than under its text.
    let mut room = Room::new("screen-wrap");
    room.items(&["The programme", BURAN]);
    assert!(read_from(&room, "• 2. Buran").starts_with(&format!("• 2. {BURAN}")));
    let rows = outline_rows(&room);
    let second = rows.iter().position(|r| r.starts_with("• 2.")).unwrap() + 1;
    assert!(rows[second].starts_with("     1988,"), "{rows:#?}");
}

#[test]
fn a_hint_that_the_text_is_not_read_is_shown_whole() {
    // Catches the "not read" hint cut at the pane's edge.
    let mut room = Room::new("screen-hint");
    room.items(&["A fish is an animal."]);
    room.app.idle();
    let hint = room
        .app
        .session
        .prose_hint(room.app.cursor())
        .unwrap()
        .to_string();
    assert!(
        read_from(&room, "not read:").starts_with(&hint),
        "{:#?}",
        outline_rows(&room)
    );
}

#[test]
fn a_click_on_a_wrapped_items_second_row_selects_it() {
    // Catches clicks placed by line, not by row, once lines wrap: the
    // second row of an item would select nothing, or the item below.
    let mut room = Room::new("screen-wrap-click");
    room.items(&[BURAN, "Below"]);
    let rows = room.lines_at(80, 30);
    let y = rows.iter().position(|r| r.contains("     1988,")).unwrap();
    let id = room.outline().in_order()[0];
    let click = room.hits_at(80, 30).at(10, y as u16).cloned();
    assert_eq!(click, Some(Click::Item { pane: 0, id }));
}

#[test]
fn the_text_cursor_at_the_end_of_a_long_item_is_on_its_last_row() {
    // Catches the text cursor put past the pane's edge on the item's first
    // row, as when lines were not wrapped.
    let mut room = Room::new("screen-wrap-cursor");
    room.items(&[BURAN]);
    room.tap(&[Key::End]);
    let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 30)).unwrap();
    t.draw(|f| memex::ui::draw(f, &room.app)).unwrap();
    let at = t.get_cursor_position().unwrap();
    let rows = room.lines_at(80, 30);
    let row = &rows[at.y as usize];
    let end = row.find("1993.").expect("the cursor is on the last row") + "1993.".len();
    assert_eq!(at.x as usize, row[..end].chars().count(), "{row}");
}

#[test]
fn a_held_quasimodes_guide_is_shown_whole_at_80_columns() {
    // Catches the foot's guide cut at the screen's edge.
    let mut room = Room::new("screen-foot-wrap");
    room.hold(Quasi::Command, "");
    let rows = room.lines_at(80, 30);
    let words = rows
        .iter()
        .rev()
        .take(3)
        .rev()
        .flat_map(|r| r.split_whitespace());
    let foot = words.collect::<Vec<_>>().join(" ");
    let guide = "to run on the target · Esc completes · ? lists what may come next";
    assert!(foot.contains(guide), "{foot}");
}

#[test]
fn a_long_message_wraps_to_three_rows_at_most() {
    // Catches a long message cut at the screen's edge, or one that takes
    // as many rows as it likes from the outline.
    let mut room = Room::new("screen-said-wrap");
    room.items(&["The programme"]);
    let said: Vec<String> = (1..=90).map(|n| format!("w{n}")).collect();
    room.app.session.message = said.join(" ");
    let rows = room.lines_at(80, 30);
    let foot = rows[27..].join(" ");
    assert!(foot.trim_start().starts_with("w1 w2"), "{rows:#?}");
    assert!(foot.contains("w40"), "the third row is shown: {foot}");
    assert!(!foot.contains("w90"), "a fourth row is not: {foot}");
    assert!(rows.iter().any(|r| r.contains("• 1. The programme")));
}

/// The words in a column of rows: those left of the first `│` (the left
/// pane or world), or right of it.
fn column_words(rows: &[String], right: bool) -> String {
    let side = |r: &String| {
        let mut parts = r.splitn(2, '│');
        let left = parts.next().unwrap_or("").to_string();
        if right {
            parts.next().unwrap_or("").to_string()
        } else {
            left
        }
    };
    let words = rows.iter().map(side).collect::<Vec<_>>().join(" ");
    words.split_whitespace().collect::<Vec<_>>().join(" ")
}

const LONG: &str = "It is false that the rest is smaller than 0, whatever the rent and the food come to this year.";

#[test]
fn the_right_panes_text_and_title_wrap_whole() {
    // Catches the selected item's text, or the pane's title, cut at the
    // screen's edge.
    let mut room = Room::new("screen-right-wrap");
    room.items(&[LONG]);
    let rows = room.lines_at(60, 24);
    let right = column_words(&rows, true);
    assert!(right.contains("LINKS OF THE SELECTED ITEM"), "{rows:#?}");
    assert!(right.contains(LONG), "{rows:#?}");
}

#[test]
fn the_worlds_wrap_and_stay_row_for_row() {
    // Catches a world's rows cut at its column's edge, or one column's
    // wrapped rows pushing its later rows below the other's. A world shows
    // the items that state facts, so these state some.
    let long = "The date of Buran-flight is 1988. The label of Buran-flight is \"Buran's flight\".";
    let mut room = Room::new("screen-worlds-wrap");
    room.items(&[long, "The income is 2000."]);
    room.app.idle();
    room.command("suppose that w");
    room.app.evaluate().unwrap(); // as the window does
    let rows = room.lines_at(80, 24);
    assert!(column_words(&rows, false).contains(long), "{rows:#?}");
    assert!(column_words(&rows, true).contains(long), "{rows:#?}");
    let below = rows.iter().find(|r| r.starts_with("The income"));
    let right = below.expect("The income").split('│').nth(1).unwrap();
    assert!(right.starts_with("The income"), "{rows:#?}");
}

/// A rent over a limit: one constraint violation, for the top bar.
fn violated(room: &mut Room) {
    room.items(&["rent"]);
    room.command("fact cell(rent, 1200)");
    room.command("constraint limit: false :- cell(rent, R), R > 1000.");
    room.app.evaluate().unwrap();
}

#[test]
fn the_top_bars_status_has_a_row_of_its_own_beside_a_long_room_name() {
    // Catches the status dropped when the room's name leaves no room for
    // it, as it was at 80 columns: the top bar's right is what says a
    // constraint is violated.
    let mut room = Room::new("screen-a-room-with-a-name-long-enough-to-fill-the-top-bar");
    violated(&mut room);
    let rows = room.lines_at(80, 24);
    assert!(
        rows[..3]
            .iter()
            .any(|r| r.trim() == "1 constraint violation"),
        "{rows:#?}"
    );
}

#[test]
fn an_outlines_long_title_wraps_onto_its_second_row() {
    // Catches the outline pane's title, which holds the text of the item
    // named title, cut at the pane's edge.
    let mut room = Room::new("screen-title-wrap");
    room.items(&["Notes on the Soviet shuttle programme"]);
    room.command("name title");
    let rows = room.lines_at(60, 24);
    let at = rows
        .iter()
        .position(|r| r.starts_with("OUTLINE"))
        .expect("a title");
    let title = column_words(&rows[at..at + 2], false);
    assert_eq!(
        title, "OUTLINE · NOTES ON THE SOVIET SHUTTLE PROGRAMME",
        "{rows:#?}"
    );
}

#[test]
fn a_row_one_world_wraps_further_keeps_the_next_rows_together() {
    // Catches the two worlds' rows drifting apart when one side of a pair
    // wraps to more rows: here the supposed side adds "(fact retracted
    // here)" to the row whose fact it sets aside.
    let long = "The date of Buran-flight is 1988. The label of Buran-flight is \"Buran's flight\".";
    let mut room = Room::new("screen-worlds-pad");
    room.items(&[long, "Nothing has more than 1 date.", "The income is 2000."]);
    room.app.idle();
    room.app.evaluate().unwrap();
    room.hold(Quasi::Suppose, "the flight was in 1989");
    room.release(Quasi::Suppose);
    room.tap(&[Key::Enter]);
    room.app.evaluate().unwrap(); // as the window does
    // At 52 columns "(fact set aside here)" takes a row of its own on
    // the supposed side, so that side's pair is a row taller.
    let rows = room.lines_at(52, 30);
    let right = |r: &String| r.split('│').nth(1).unwrap_or("").trim_start().to_string();
    let own_row = rows
        .iter()
        .any(|r| right(r).starts_with("(fact set aside here)"));
    assert!(own_row, "the supposed side is a row taller: {rows:#?}");
    let below = rows.iter().find(|r| r.starts_with("The income"));
    let below = right(below.expect("The income"));
    assert!(below.starts_with("The income"), "{rows:#?}");
}
