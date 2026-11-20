//! The mouse: where clicks land (ui::Presentations), and what a click does
//! (App::click). The window's end, a click turned into a cell, is checked
//! by tests/memex_in_a_window.py.

use memex::app::{App, Event, Key, Mode, PaneKind, Quasi};
use memex::present::Presentation;
use memex::ui::Presentations;

mod support;
use support::{Room, replayed};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/wikidata-buran.pl"
);

fn tap(app: &mut App, k: Key) {
    app.handle(Event::down(k));
    app.handle(Event::up(k));
}

fn typ(app: &mut App, s: &str) {
    for c in s.chars() {
        tap(app, Key::Char(c));
    }
}

fn command(app: &mut App, line: &str) {
    app.handle(Event::down(Key::Quasi(Quasi::Command)));
    typ(app, line);
    app.handle(Event::up(Key::Quasi(Quasi::Command)));
    assert!(
        !app.session.message.starts_with("no command") && !app.session.message.starts_with("not a"),
        "{line}: {}",
        app.session.message
    );
    app.evaluate().unwrap();
}

/// What a click where a text is first drawn, on a wide screen, would act
/// on.
fn hit_on(r: &Room, text: &str) -> Option<Presentation> {
    let (x, y) = r.find(text);
    r.hits().at(x, y).cloned()
}

/// Click where a text is drawn, as the terminal loop would.
fn click_on(r: &mut Room, text: &str) -> Presentation {
    let c = hit_on(r, text).unwrap_or_else(|| panic!("nothing to click at {text:?}"));
    r.app.click(&c);
    r.app.evaluate().unwrap();
    c
}

/// The kind of the last entry that is not a view record.
fn last_kind(app: &App) -> String {
    app.journal()
        .entries()
        .iter()
        .rev()
        .find(|e| e.kind() != "view")
        .unwrap()
        .kind()
        .to_string()
}

/// Three items, the middle one under the first, with a claim to adopt.
fn notes(name: &str) -> Room {
    let mut app = Room::new(&format!("mouse-{name}"));
    app.set_durable(false);
    typ(&mut app, "The programme");
    tap(&mut app, Key::Enter);
    typ(&mut app, "Buran flew once, in 1988.");
    tap(&mut app, Key::Tab);
    command(&mut app, "fact date(buran_flight, 1988)");
    command(&mut app, "fact label(buran_flight, \"Buran's flight\")");
    tap(&mut app, Key::Enter);
    typ(&mut app, "Talk notes: it flew in 1989.");
    command(
        &mut app,
        "claim date(buran_flight, 1989) from my notes, conference talk",
    );
    command(
        &mut app,
        "constraint one date per event: false :- date(E, D1), date(E, D2), D1 \\= D2.",
    );
    app.flush();
    app.evaluate().unwrap();
    app
}

#[test]
fn a_click_on_an_item_moves_the_cursor_there() {
    // Catches clicks mapped to the wrong row, for instance one that counts
    // an item's mark lines as items.
    let mut app = notes("item");
    let talk = app.cursor();
    let c = click_on(&mut app, "The programme");
    assert!(matches!(c, Presentation::Item { pane: 0, .. }));
    assert_eq!(app.here(), "The programme");
    // The Buran item has a mark line under it; the next row is still Talk notes.
    click_on(&mut app, "Talk notes");
    assert_eq!(app.cursor(), talk);
    click_on(&mut app, "Buran flew");
    assert_eq!(app.here(), "Buran flew once, in 1988.");
    // The mark line presents its constraint. This one a command made, so
    // no item states it, and a click on it leaves the cursor where it is.
    let mark = hit_on(&app, "? sources disagree under");
    assert!(
        matches!(mark, Some(Presentation::Constraint { item: None, .. })),
        "{mark:?}"
    );
    let here = app.cursor();
    click_on(&mut app, "? sources disagree under");
    assert_eq!(app.cursor(), here);
    // Typing after a click goes into the clicked item.
    typ(&mut app, " Once.");
    app.flush();
    assert_eq!(app.here(), "Buran flew once, in 1988. Once.");
    // The click is a view change in the journal, so replay agrees.
    let again = replayed(&app.dir);
    let here = "replay puts the cursor where the click did";
    assert_eq!(again.cursor(), app.cursor(), "{here}");
}

#[test]
fn a_click_on_the_hint_adopts_and_retracts() {
    // Plan: "Adopt and retract with one click". Catches a hint that is drawn
    // but not clickable, or whose two halves run the wrong commands.
    let mut app = notes("hint");
    tap(&mut app, Key::F(2));
    app.evaluate().unwrap();
    let adopt = Presentation::Command("adopt".into());
    assert_eq!(click_on(&mut app, "F3 adopts"), adopt);
    assert_eq!(last_kind(&app), "adopt");
    assert!(app.shows("1 constraint violation"));
    let unadopt = Presentation::Command("unadopt".into());
    assert_eq!(click_on(&mut app, "F4 retracts an adoption"), unadopt);
    assert_eq!(last_kind(&app), "retract");
    // The " · " between them does nothing.
    let (x, y) = app.find("F3 adopts");
    assert_eq!(app.hits().at(x + "F3 adopts".len() as u16 + 1, y), None);
}

#[test]
fn a_click_on_a_suggestion_accepts_it() {
    // Plan: suggestions "adopted ... by a click". Catches a click that
    // accepts a different suggestion from the one under it, which wrapped
    // evidence lines would cause if heights were guessed.
    let mut app = buran_with_suggestions("mouse-suggestion");
    let shown = shown_suggestions(&app);
    assert!(shown.len() >= 2, "{shown:?}");
    let accept_2 = Presentation::Suggestion(2);
    // Click the second suggestion's last line: its evidence, not its title.
    the_second_spans_its_evidence(&app, &shown[1], &accept_2);
    app.click(&accept_2);
    assert_eq!(app.session.message, format!("adopted: {}", shown[1]));
}

/// Notes that Buran flew in 1989, Wikidata imported, and the suggestions
/// in the second pane.
fn buran_with_suggestions(name: &str) -> Room {
    let mut app = Room::new(name);
    app.set_durable(false);
    typ(&mut app, "Buran flew once, unmanned, in 1989.");
    app.flush();
    command(&mut app, "fact date(buran_flight, 1989)");
    command(&mut app, "fact label(buran_flight, \"Buran's flight\")");
    command(&mut app, &format!("import wikidata {FIXTURE}"));
    command(&mut app, "pane suggestions");
    assert_eq!(app.session.view.panes[1].kind, PaneKind::Suggestions);
    app
}

/// The second suggestion's title, and its evidence lines below, are each
/// a click that accepts it.
fn the_second_spans_its_evidence(app: &Room, title: &str, accept: &Presentation) {
    let at = app.find(&format!("2. {title}"));
    let hits = app.hits();
    assert_eq!(
        hits.at(at.0, at.1),
        Some(accept),
        "the title is part of suggestion 2"
    );
    let below = last_row_of(&hits, at, accept) > at.1;
    assert!(below, "the evidence lines are part of suggestion 2");
}

/// The suggestions shown, described.
fn shown_suggestions(app: &App) -> Vec<String> {
    let shown = app.reason.suggestions.shown();
    shown.iter().map(|s| s.describe()).collect()
}

/// The last of the five rows below a place in its column where a click
/// acts as this one does.
fn last_row_of(hits: &Presentations, (x, y): (u16, u16), click: &Presentation) -> u16 {
    let rows = (y + 1..y + 6).rev();
    let mut same = rows.filter(|r| hits.at(x, *r) == Some(click));
    same.next().expect("a row below acts so")
}

#[test]
fn a_click_does_nothing_while_a_quasimode_or_question_is_open() {
    // Catches a click that cuts across a held quasimode or a confirmation.
    let mut app = notes("modal");
    let c = hit_on(&app, "The programme").unwrap();
    let before = app.cursor();
    app.hold(Quasi::Command, "");
    app.click(&c);
    assert_eq!(app.cursor(), before);
    // Released with nothing typed, Command latches; the click still does nothing.
    app.release(Quasi::Command);
    assert!(matches!(app.session.mode, Mode::Quasi(_)));
    app.click(&c);
    assert_eq!(app.cursor(), before);
    tap(&mut app, Key::Escape);
    tap(&mut app, Key::Enter);
    assert!(matches!(app.session.mode, Mode::Normal));
    app.click(&c);
    assert_eq!(app.here(), "The programme", "and acts once it is closed");
}
