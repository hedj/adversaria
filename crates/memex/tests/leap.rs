//! Leap: the order in which it finds matches, and recognition, which
//! completes a pattern to what every match shares (docs/plan.md,
//! Milestone 3).

mod support;

use memex::app::{Key, Mode, Quasi};
use ratatui::style::Modifier;
use support::Room;

/// Items a1, b, a2 and a3, the cursor on a3.
fn room(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["a1", "b", "a2", "a3"]);
    r
}

/// What recognition makes of a pattern typed into a leap.
fn recognised(r: &mut Room, pattern: &str) -> String {
    r.hold(Quasi::LeapForward, pattern);
    r.tap(&[Key::Tab]);
    match &r.app.session.mode {
        Mode::Quasi(a) => a.input.clone(),
        m => panic!("{m:?}"),
    }
}

#[test]
fn leaping_back_finds_the_nearest_match_before_the_cursor() {
    let mut r = room("back");
    r.leap(Quasi::LeapBack, "a");
    assert_eq!(r.here(), "a2");
}

#[test]
fn leaping_forward_wraps_to_the_first_match() {
    let mut r = room("forward");
    r.leap(Quasi::LeapForward, "a");
    assert_eq!(r.here(), "a1");
}

#[test]
fn the_item_at_the_cursor_is_the_last_match_back() {
    let r = room("last");
    let a3 = r.app.cursor();
    assert_eq!(r.app.leap_matches("a3", false), [a3]);
}

#[test]
fn an_empty_pattern_matches_nothing() {
    let r = room("empty");
    assert!(r.app.leap_matches("", true).is_empty());
}

#[test]
fn recognition_completes_up_to_a_space() {
    let mut r = Room::new("space");
    r.items(&["Energia rocket", "Energia booster"]);
    assert_eq!(recognised(&mut r, "ener"), "energia");
}

#[test]
fn recognition_with_no_match_keeps_the_pattern() {
    let mut r = room("none");
    assert_eq!(recognised(&mut r, "zzz"), "zzz");
}

#[test]
fn recognition_stops_where_the_matches_differ() {
    let mut r = Room::new("differ");
    r.items(&["Energia", "Enigma"]);
    assert_eq!(recognised(&mut r, "en"), "en");
}

#[test]
fn a_pattern_whose_small_letters_are_longer_lists_without_a_crash() {
    // Catches the match's end found by the pattern's own length, where
    // its small letters take more bytes ("İ" is "i̇"): the cut fell inside
    // a character of the item's text, and the memex panicked.
    let mut r = Room::new("leap-dotted");
    r.items(&["ai\u{307}b", "plain"]);
    r.hold(Quasi::LeapForward, "İ");
    let lines = r.lines_at(80, 12);
    assert!(
        lines.iter().any(|l| l.contains("LEAP · 1 MATCHES")),
        "{lines:?}"
    );
}

/// A room of items holding "energy", one two levels down, with a Leap for
/// "ener" held: its list is drawn.
fn listing(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Alpha energy", "Beta"]);
    r.tap(&[Key::Tab]);
    r.items(&["", "Nested energy note"]);
    r.tap(&[Key::Tab]);
    r.items(&["", "Deeper Energy"]);
    for k in 0..6 {
        r.items(&["", &format!("energy {k}")]);
    }
    r.hold(Quasi::LeapForward, "ener");
    r
}

/// The rows of the list's box on a screen 100 by 30, each with the
/// modifiers of its second cell (the first is the box's border).
fn box_rows(r: &Room) -> Vec<(u16, String, Modifier)> {
    let buf = r.screen(100, 30);
    let row = |y: u16| (0..60).map(|x| buf[(x, y)].symbol()).collect::<String>();
    let inside = |y: &u16| row(*y).starts_with('│');
    let rows = (0..30).filter(inside);
    rows.map(|y| (y, row(y), buf[(1, y)].modifier)).collect()
}

#[test]
fn a_leap_lists_five_matches_unless_asked_for_all() {
    let mut r = listing("leap-five");
    assert_eq!(box_rows(&r).len(), 1 + 5, "the heading and five");
    r.typ("?");
    assert_eq!(box_rows(&r).len(), 1 + 9, "the heading and every match");
}

#[test]
fn the_match_a_leap_goes_to_first_is_reversed_and_each_match_is_bold() {
    let r = listing("leap-marks");
    let rows = box_rows(&r);
    assert!(rows[1].2.contains(Modifier::REVERSED) && !rows[2].2.contains(Modifier::REVERSED));
    let (y, text, _) = &rows[2];
    let at = text
        .rfind("ener")
        .map(|i| text[..i].chars().count() as u16)
        .unwrap();
    assert!(
        r.screen(100, 30)[(at, *y)]
            .modifier
            .contains(Modifier::BOLD)
    );
}

#[test]
fn a_match_is_listed_under_the_items_above_it_from_the_top() {
    let rows = box_rows(&listing("leap-trail"));
    let trail = "1. Alpha energy › Beta › Nested energy note";
    assert!(rows.iter().any(|(_, t, _)| t.contains(trail)), "{rows:?}");
}

#[test]
fn a_leaps_list_sits_at_the_foot_of_the_outline() {
    // The foot is as many rows as its guide wraps to, so the outline's
    // last row is the one above the foot's first, " LEAP ".
    let r = listing("leap-foot");
    let rows = box_rows(&r);
    let bottom = rows.last().unwrap().0 + 1;
    let lines = r.lines_at(100, 30);
    let foot = lines.iter().position(|l| l.starts_with(" LEAP ")).unwrap() as u16;
    assert_eq!(bottom, foot - 1, "its border on the outline's last row");
}

/// Items Alpha, "Ener a" and "Ener b", the cursor on Alpha.
fn ener(name: &str) -> (Room, [u64; 3]) {
    let mut r = Room::new(name);
    r.items(&["Alpha", "Ener a", "Ener b"]);
    let items = r.outline().in_order();
    r.put_cursor(items[0]);
    (r, [items[0], items[1], items[2]])
}

#[test]
fn the_cursor_moves_to_the_first_match_as_each_letter_is_typed() {
    // Catches a Leap that moves only when it ends: the design (Raskin's
    // LEAP) moves the cursor as each letter is typed.
    let (mut r, [_, a, _]) = ener("leap-incremental");
    r.hold(Quasi::LeapForward, "E");
    assert_eq!(r.app.cursor(), a, "after one letter, still held");
}

#[test]
fn each_letter_counts_matches_from_where_the_leap_began() {
    // Catches a search from the moved cursor: the second letter would
    // pass over "Ener a", where the cursor already is, to "Ener b".
    let (mut r, [_, a, _]) = ener("leap-from-start");
    r.hold(Quasi::LeapForward, "En");
    assert_eq!(r.app.cursor(), a);
    r.typ("er b");
    assert_eq!(r.app.cursor(), r.outline().in_order()[2]);
    r.release(Quasi::LeapForward);
    r.command("back");
    assert_eq!(r.app.cursor(), r.outline().in_order()[0], "back to Alpha");
}

#[test]
fn with_no_match_the_cursor_stays_at_the_last() {
    let (mut r, [_, a, _]) = ener("leap-no-match");
    r.hold(Quasi::LeapForward, "Enerx");
    assert_eq!(r.app.cursor(), a);
}

#[test]
fn erasing_the_pattern_or_leaving_puts_the_cursor_back() {
    let (mut r, [alpha, _, _]) = ener("leap-leave");
    r.tap(&[Key::Quasi(Quasi::LeapForward)]);
    r.typ("E");
    r.tap(&[Key::Backspace]);
    assert_eq!(r.app.cursor(), alpha, "nothing typed");
    r.typ("E");
    r.tap(&[Key::Backspace, Key::Backspace]);
    assert!(matches!(r.app.session.mode, Mode::Normal), "left");
    assert_eq!(r.app.cursor(), alpha, "left without acting");
}

/// A click where a text starts.
fn click_on(r: &mut Room, text: &str) {
    let at = r.find(text);
    let hits = r.hits();
    r.app.click_at(&hits, at);
}

#[test]
fn the_matches_pane_keeps_the_list_and_where_the_cursor_is_in_it() {
    // Catches matches that vanish once a Leap ends: the third review's
    // fifth finding, working through fourteen matches by leaping again.
    let (mut r, [_, a, b]) = ener("leap-matches-pane");
    r.leap(Quasi::LeapForward, "ener");
    r.command("pane matches");
    assert!(
        r.shows("MATCHES OF “ener” · 2 · at 1 of 2"),
        "{:#?}",
        r.lines_at(160, 40)
    );
    assert!(r.shows("▸ 1. 2. Ener a"), "{:#?}", r.lines_at(160, 40));
    click_on(&mut r, "2. 3. Ener b");
    assert_eq!(r.app.cursor(), b, "a click jumps");
    assert!(r.shows("· at 2 of 2"), "{:#?}", r.lines_at(160, 40));
    r.command("back");
    assert_eq!(r.app.cursor(), a);
}

#[test]
fn before_any_leap_the_matches_pane_says_how_to_fill_it() {
    let (mut r, _) = ener("leap-matches-empty");
    r.command("pane matches");
    let rows = r.lines_at(160, 40);
    let said = "(find, or Leap, and what it finds is listed here)";
    assert!(rows.iter().any(|l| l.contains(said)), "{rows:#?}");
}

#[test]
fn a_reopened_notebook_keeps_the_matches_pane() {
    // Catches a pane kind the view entry names but replay does not read.
    let (mut r, _) = ener("leap-matches-replay");
    r.leap(Quasi::LeapForward, "ener");
    r.command("pane matches");
    r.app.flush();
    let again = support::replayed(&r.dir);
    assert_eq!(
        again.session.view.panes[1].kind,
        memex::app::PaneKind::Matches
    );
}

/// Items Alpha and "Korolev's notes", the cursor on Alpha, Leap latched
/// with a pattern typed.
fn near(name: &str, pattern: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Alpha", "Korolev's notes"]);
    let first = r.outline().in_order()[0];
    r.put_cursor(first);
    r.tap(&[Key::Quasi(Quasi::LeapForward)]);
    r.typ(pattern);
    r
}

#[test]
fn enter_on_a_near_miss_warns_before_it_makes_an_item() {
    // Catches a mistyped pattern, one letter off, made into a junk item by
    // one Enter: the third review's seventh finding.
    let mut r = near("leap-near", "Korolov");
    r.tap(&[Key::Enter]);
    assert_eq!(r.outline().in_order().len(), 2, "nothing made yet");
    let warning = "no item holds “Korolov”; 2 holds “Korolev” · Enter again makes “Korolov”";
    let foot = r.lines_at(160, 40).join(" ");
    assert!(foot.contains(warning), "{foot}");
    r.tap(&[Key::Enter]);
    assert_eq!(r.outline().in_order().len(), 3, "made on the second Enter");
    assert_eq!(r.here(), "Korolov");
}

#[test]
fn a_changed_pattern_is_checked_again() {
    let mut r = near("leap-near-changed", "Korolov");
    r.tap(&[Key::Enter]);
    r.typ("x");
    let foot = r.lines_at(160, 40).join(" ");
    assert!(!foot.contains("Enter again makes"), "{foot}");
    // Eight letters may be two edits off: Korolovx is still near Korolev,
    // so Enter warns again, for the new pattern, and makes nothing.
    r.tap(&[Key::Enter]);
    assert_eq!(r.outline().in_order().len(), 2);
    let foot = r.lines_at(160, 40).join(" ");
    assert!(foot.contains("Enter again makes “Korolovx”"), "{foot}");
}

#[test]
fn a_pattern_near_nothing_is_made_at_once() {
    let mut r = near("leap-near-none", "Polyus");
    r.tap(&[Key::Enter]);
    assert_eq!(r.outline().in_order().len(), 3);
}

#[test]
fn enter_finds_what_it_would_make_rather_than_make_it_again() {
    // Catches Enter searching "a3 " with its space, finding nothing, and
    // making a second "a3", the text trimmed (the code review, F-B7).
    let mut r = room("leap-trailing-space");
    r.put_cursor(r.outline().in_order()[0]);
    r.hold(Quasi::LeapForward, "a3 ");
    r.tap(&[Key::Enter]);
    assert_eq!(r.outline().in_order().len(), 4, "nothing made");
    assert_eq!(r.here(), "a3");
}
