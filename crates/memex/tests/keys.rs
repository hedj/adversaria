//! What each key does in Normal mode, beside typing: quasimodes, bound
//! commands, and moving the cursor (docs/plan.md, Milestone 3).

mod support;

use memex::app::{Key, Mode, Quasi};
use ratatui::style::Modifier;
use support::Room;

#[test]
fn the_palette_key_opens_the_command_quasimode_latched_and_listing() {
    let mut r = Room::new("palette");
    r.tap(&[Key::F(1)]);
    let open = matches!(&r.app.session.mode, Mode::Quasi(a) if a.q() == Quasi::Command && a.latched() && a.listing);
    assert!(open, "{:?}", r.app.session.mode);
}

#[test]
fn a_bound_command_that_asks_keeps_its_question() {
    let mut r = Room::new("asks");
    r.items(&["one"]);
    let retract = r.last("edit");
    r.bind(&format!("F9 = \"retract {retract}\""));
    r.tap(&[Key::F(9)]);
    assert!(matches!(r.app.session.mode, Mode::ConfirmRetract { .. }));
}

#[test]
fn an_editing_key_clears_the_message() {
    let mut r = Room::new("clears");
    r.items(&["one"]);
    r.refused("outdent");
    assert_eq!(
        r.app.session.message,
        "a top-level item cannot be outdented"
    );
    r.typ("x");
    assert_eq!(r.app.session.message, "");
}

#[test]
fn down_at_the_last_item_stays_there_at_the_end_of_its_text() {
    let mut r = Room::new("last");
    r.items(&["one", "two"]);
    r.tap(&[Key::Home, Key::Down]);
    r.typ("!");
    assert_eq!(r.here(), "two!");
}

#[test]
fn down_from_a_cursor_on_no_item_goes_to_the_first() {
    let mut r = Room::new("none");
    r.items(&["one", "two"]);
    r.put_cursor(memex::outline::TOP);
    r.tap(&[Key::Down]);
    assert_eq!(r.here(), "one");
}

#[test]
fn end_goes_to_the_end_of_the_text() {
    let mut r = Room::new("end");
    r.items(&["abc"]);
    r.tap(&[Key::Home, Key::End]);
    r.typ("X");
    assert_eq!(r.here(), "abcX");
}

#[test]
fn right_at_the_end_of_the_text_stays() {
    let mut r = Room::new("right");
    r.items(&["abc"]);
    r.tap(&[Key::Right, Key::Left]);
    r.typ("X");
    assert_eq!(r.here(), "abXc");
}

#[test]
fn the_top_bar_names_the_notebook_and_its_world() {
    // Catches the journal's jargon back in the top bar ("room", the count
    // of entries).
    let r = Room::new("top");
    let top = r.lines().remove(0);
    let room = r.app.journal().room();
    assert!(
        top.starts_with(&format!("notebook {room}   world actual")),
        "{top}"
    );
    assert!(!top.contains("journal") && !top.contains("room "), "{top}");
}

#[test]
fn the_pane_with_focus_has_its_heading_bright() {
    // Catches focus shown nowhere, or given to the links: the focused
    // outline's heading is bold and not dim, the links' dim, and F6 there
    // leaves it as it was.
    let mut r = Room::new("focus-heading");
    r.items(&["Alpha"]);
    let lit = |r: &Room, text: &str| {
        let m = r.style_of(text).add_modifier;
        m.contains(Modifier::BOLD) && !m.contains(Modifier::DIM)
    };
    assert!(lit(&r, "OUTLINE") && !lit(&r, "LINKS OF 1"));
    r.tap(&[Key::F(6)]);
    assert!(lit(&r, "OUTLINE") && !lit(&r, "LINKS OF 1"));
}

#[test]
fn the_top_bar_puts_a_right_side_that_does_not_fit_on_a_row_of_its_own() {
    // Catches the right side drawn over the left, or dropped, as it was
    // until 19 November 2026: the status it shows (a violation, say) was
    // then not shown at all at 80 columns.
    let mut r = Room::new("top-narrow");
    r.items(&["rent"]);
    r.command("fact box(rent, 1200)");
    r.command("constraint limit: false :- box(rent, R), R > 1000.");
    r.app.evaluate().unwrap();
    let rows = r.lines_at(16, 12);
    assert!(
        rows[0].starts_with("notebook"),
        "the left is not drawn over: {rows:#?}"
    );
    let right = rows
        .iter()
        .position(|r| r.trim().starts_with("1 constraint"));
    assert!(right.is_some(), "the right has its own row: {rows:#?}");
}

#[test]
fn enter_in_leap_with_no_match_makes_an_item_holding_the_pattern() {
    // Catches Enter that only says nothing matched, an item made in the
    // wrong place or without the pattern, or the cursor left behind.
    let mut r = Room::new("leap-create");
    r.items(&["Alpha", "Beta"]);
    let alpha = r.outline().in_order()[0];
    r.put_cursor(alpha);
    r.hold(Quasi::LeapDown, "Gamma ray");
    r.tap(&[Key::Enter]);
    r.release(Quasi::LeapDown);
    r.app.flush();
    let order: Vec<String> = r.outline().in_order().iter().map(|i| r.text(*i)).collect();
    assert_eq!(order, ["Alpha", "Gamma ray", "Beta"]);
    assert_eq!(r.here(), "Gamma ray");
    assert!(matches!(r.app.session.mode, memex::app::Mode::Normal));
}

#[test]
fn enter_in_leap_with_a_match_leaps_and_makes_nothing() {
    let mut r = Room::new("leap-enter-found");
    r.items(&["Alpha", "Beta"]);
    let alpha = r.outline().in_order()[0];
    r.put_cursor(alpha);
    r.hold(Quasi::LeapDown, "bet");
    r.tap(&[Key::Enter]);
    r.release(Quasi::LeapDown);
    assert_eq!(r.outline().in_order().len(), 2);
    assert_eq!(r.here(), "Beta");
}

#[test]
fn releasing_leap_with_no_match_makes_nothing() {
    // Catches a mistyped pattern turned into an item.
    let mut r = Room::new("leap-release-none");
    r.items(&["Alpha"]);
    r.hold(Quasi::LeapDown, "Gamma");
    r.release(Quasi::LeapDown);
    assert_eq!(r.outline().in_order().len(), 1);
    assert!(r.app.session.message.contains("no item holds"));
}

#[test]
fn an_item_made_from_leap_is_read_like_typed_text() {
    // Catches the new item's text never read: it was not typed, so only
    // Leap can say it waits to be read.
    let mut r = Room::new("leap-create-read");
    r.items(&["Alpha"]);
    r.hold(Quasi::LeapDown, "The income is 2000.");
    r.tap(&[Key::Enter]);
    r.release(Quasi::LeapDown);
    r.app.idle();
    assert!(
        r.facts().contains(&"box(income, 2000)".to_string()),
        "{:?}",
        r.facts()
    );
}

#[test]
fn a_pane_that_shows_links_or_suggestions_takes_no_focus() {
    // Catches focus given to such a pane, where no text cursor is drawn:
    // keys pressed there changed a note unseen, or nothing, and a list
    // took the keyboard (docs/concepts.md, decision 4).
    for kind in ["links", "suggestions"] {
        let mut r = beside(kind);
        r.refused("switch");
        let said = format!("only an outline takes keys; the other pane shows {kind}");
        assert_eq!(r.app.session.message, said);
        the_outline_takes_keys(r);
    }
}

/// Two items, and the other pane showing this kind.
fn beside(kind: &str) -> Room {
    let mut r = Room::new(&format!("beside-{kind}"));
    r.items(&["first", "second"]);
    r.command(&format!("pane {kind}"));
    r
}

/// The focus is on the outline, and a key types into it.
fn the_outline_takes_keys(mut r: Room) {
    assert_eq!(r.view().focus, 0);
    let cursor = r.app.cursor();
    r.tap(&[Key::Char('x')]);
    r.app.flush();
    assert_eq!(r.text(cursor), "secondx");
}

/// Hold Command, type one letter, and release it: a Ctrl chord.
fn chord(r: &mut Room, letter: &str) {
    r.hold(Quasi::Command, letter);
    r.release(Quasi::Command);
}

#[test]
fn ctrl_z_and_ctrl_y_undo_and_redo_as_everywhere_else() {
    // Catches Ctrl+Z read as the prefix z of a command, which names none,
    // so a habit from every other program did nothing but complain.
    let mut r = Room::new("chord-undo");
    r.items(&["Alpha", "Beta"]);
    r.app.flush();
    let text = |r: &Room| r.text(r.outline().in_order()[1]);
    chord(&mut r, "z");
    assert_eq!(text(&r), "", "Beta's text undone: {:?}", r.lines());
    chord(&mut r, "y");
    assert_eq!(text(&r), "Beta", "redone");
    chord(&mut r, "z");
    chord(&mut r, "Z");
    assert_eq!(text(&r), "Beta", "Ctrl+Shift+Z redoes too");
}

#[test]
fn ctrl_f_opens_leap() {
    let mut r = Room::new("chord-find");
    r.items(&["Alpha"]);
    chord(&mut r, "f");
    let leaping =
        matches!(&r.app.session.mode, Mode::Quasi(a) if a.q() == Quasi::LeapDown && a.latched());
    assert!(leaping, "{:?}", r.app.session.mode);
}

#[test]
fn another_ctrl_chord_runs_no_command_by_surprise() {
    // Catches Ctrl+O, held from habit, running outdent: the prefix o names
    // it alone.
    let mut r = Room::new("chord-other");
    r.items(&["Alpha", "Beta"]);
    r.tap(&[Key::Tab]);
    chord(&mut r, "o");
    assert_eq!(r.outline().number(r.app.cursor()), "1.1", "not outdented");
    assert!(
        r.app
            .session
            .message
            .starts_with("a command held needs two letters")
    );
}

#[test]
fn a_latched_command_still_takes_one_letter() {
    let mut r = Room::new("chord-latched");
    r.items(&["Alpha", "Beta"]);
    r.tap(&[Key::Quasi(Quasi::Command)]);
    r.typ("n");
    r.tap(&[Key::Enter]);
    assert_eq!(r.outline().items.len(), 3, "a new item");
}

#[test]
fn help_opens_the_list_f1_opens() {
    // Catches two kinds of help: `help` printed names on the message line,
    // while F1 opened the grouped list with what each does.
    let mut r = Room::new("help-list");
    r.items(&["one"]);
    r.command("help");
    let listing = matches!(&r.app.session.mode, Mode::Quasi(a) if a.q() == Quasi::Command && a.latched() && a.listing);
    assert!(listing, "{:?}", r.app.session.mode);
    let mut f1 = Room::new("help-f1");
    f1.items(&["one"]);
    f1.tap(&[Key::F(1)]);
    assert_eq!(r.app.session.mode, f1.app.session.mode);
}

#[test]
fn esc_cancels_a_leap_and_puts_the_cursor_back() {
    // Catches Esc completing the pattern, as it did, where every other
    // program cancels: the Leap must end, and act on nothing.
    let mut r = Room::new("esc-leap");
    r.items(&["Alpha", "Beta"]);
    let alpha = r.outline().in_order()[0];
    r.put_cursor(alpha);
    r.tap(&[Key::Quasi(Quasi::LeapDown)]);
    r.typ("Be");
    r.tap(&[Key::Escape]);
    assert_eq!(r.app.session.mode, Mode::Normal);
    assert_eq!(r.app.cursor(), alpha, "back where it began");
}

#[test]
fn esc_cancels_a_command_and_tab_completes_it() {
    let mut r = Room::new("esc-command");
    r.items(&["Alpha", "Beta"]);
    r.tap(&[Key::Quasi(Quasi::Command)]);
    r.typ("dele");
    r.tap(&[Key::Tab]);
    assert!(
        matches!(&r.app.session.mode, Mode::Quasi(a) if a.input == "delete "),
        "{:?}",
        r.app.session.mode
    );
    r.tap(&[Key::Escape]);
    assert_eq!(r.app.session.mode, Mode::Normal);
    assert_eq!(r.outline().items.len(), 2, "nothing ran");
}
