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
        foot(&room)[1].starts_with("fading hint"),
        "{:?}",
        foot(&room)
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
