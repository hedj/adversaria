//! The mouse: where clicks land (ui::Hits), and what a click does
//! (App::click). The terminal end, crossterm's mouse events, is checked by
//! tests/memex_in_a_pty.py.

use memex::app::{App, Click, Event, Key, Mode, PaneKind, Quasi};
use memex::ui::{Hits, draw_hits};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::path::PathBuf;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/wikidata-buran.pl"
);

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("adversaria-mouse-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

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
        !app.screen.message.starts_with("no command") && !app.screen.message.starts_with("not a"),
        "{line}: {}",
        app.screen.message
    );
    app.evaluate().unwrap();
}

/// Draw, as the terminal loop does, and return the screen's text and hits.
fn draw(app: &App) -> (Vec<String>, Hits) {
    let (w, h) = (160, 40);
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    let mut hits = Hits::default();
    t.draw(|f| hits = draw_hits(f, app)).unwrap();
    let buf = t.backend().buffer().clone();
    let lines = (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect();
    (lines, hits)
}

/// Where on screen a text is: (column, row) of its first character.
fn find(lines: &[String], text: &str) -> (u16, u16) {
    for (y, l) in lines.iter().enumerate() {
        if let Some(i) = l.find(text) {
            return (l[..i].chars().count() as u16, y as u16);
        }
    }
    panic!("no {text:?} on screen:\n{}", lines.join("\n"))
}

/// Click where a text is drawn, as the terminal loop would.
fn click_on(app: &mut App, text: &str) -> Click {
    let (lines, hits) = draw(app);
    let (x, y) = find(&lines, text);
    let c = hits
        .at(x, y)
        .unwrap_or_else(|| panic!("nothing to click at {text:?}"))
        .clone();
    app.click(&c);
    app.evaluate().unwrap();
    c
}

/// The kind of the last entry that is not a view record.
fn last_kind(app: &App) -> String {
    app.journal()
        .entries()
        .iter()
        .rev()
        .find(|e| e.kind != "view")
        .unwrap()
        .kind
        .clone()
}

/// Three items, the middle one under the first, with a claim to adopt.
fn notes(dir: &std::path::Path) -> App {
    let mut app = App::open(dir).unwrap();
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
    let dir = temp("item");
    let mut app = notes(&dir);
    let talk = app.cursor();
    let c = click_on(&mut app, "The programme");
    assert!(matches!(c, Click::Item { pane: 0, .. }));
    assert_eq!(app.record.outline.text(app.cursor()), "The programme");
    // The Buran item has a mark line under it; the next row is still Talk notes.
    click_on(&mut app, "Talk notes");
    assert_eq!(app.cursor(), talk);
    click_on(&mut app, "Buran flew");
    assert_eq!(
        app.record.outline.text(app.cursor()),
        "Buran flew once, in 1988."
    );
    // A click on the mark line itself acts on nothing.
    let (lines, hits) = draw(&app);
    let (x, y) = find(&lines, "⚑ one date per event");
    assert_eq!(hits.at(x, y), None);
    // Typing after a click goes into the clicked item.
    typ(&mut app, " Once.");
    app.flush();
    assert_eq!(
        app.record.outline.text(app.cursor()),
        "Buran flew once, in 1988. Once."
    );
    // The click is a view change in the journal, so replay agrees.
    let here = app.cursor();
    drop(app);
    let again = App::open(&dir).unwrap();
    assert_eq!(
        again.cursor(),
        here,
        "replay puts the cursor where the click did"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_click_on_the_hint_adopts_and_retracts() {
    // Plan: "Adopt and retract with one click". Catches a hint that is drawn
    // but not clickable, or whose two halves run the wrong commands.
    let dir = temp("hint");
    let mut app = notes(&dir);
    tap(&mut app, Key::F(2));
    app.evaluate().unwrap();
    assert_eq!(
        click_on(&mut app, "F3 adopts"),
        Click::Command("adopt".into())
    );
    assert_eq!(last_kind(&app), "adopt");
    let (lines, _) = draw(&app);
    assert!(lines.iter().any(|l| l.contains("1 constraint violation")));
    assert_eq!(
        click_on(&mut app, "F4 retracts an adoption"),
        Click::Command("unadopt".into())
    );
    assert_eq!(last_kind(&app), "retract");
    // The " · " between them does nothing.
    let (lines, hits) = draw(&app);
    let (x, y) = find(&lines, "F3 adopts");
    assert_eq!(hits.at(x + "F3 adopts".len() as u16 + 1, y), None);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_click_on_a_suggestion_accepts_it() {
    // Plan: suggestions "adopted ... by a click". Catches a click that
    // accepts a different suggestion from the one under it, which wrapped
    // evidence lines would cause if heights were guessed.
    let dir = temp("suggestion");
    let mut app = App::open(&dir).unwrap();
    app.set_durable(false);
    typ(&mut app, "Buran flew once, unmanned, in 1989.");
    app.flush();
    command(&mut app, "fact date(buran_flight, 1989)");
    command(&mut app, "fact label(buran_flight, \"Buran's flight\")");
    command(&mut app, &format!("import wikidata {FIXTURE}"));
    command(&mut app, "pane suggestions");
    assert_eq!(app.screen.view.panes[1].kind, PaneKind::Suggestions);
    let shown: Vec<String> = app
        .shown_suggestions()
        .iter()
        .map(|s| s.describe())
        .collect();
    assert!(shown.len() >= 2, "{shown:?}");
    // Click the second suggestion's last line: its evidence, not its title.
    let (lines, hits) = draw(&app);
    let (x, y) = find(&lines, &format!("2. {}", shown[1]));
    let evidence_row = (y + 1..y + 6)
        .rev()
        .find(|r| hits.at(x, *r) == Some(&Click::Command("accept 2".into())))
        .expect("the evidence lines are part of suggestion 2");
    assert!(evidence_row > y);
    app.click(&hits.at(x, evidence_row).unwrap().clone());
    assert!(
        app.screen.message.starts_with("adopted: "),
        "{}",
        app.screen.message
    );
    assert_eq!(app.screen.message, format!("adopted: {}", shown[1]));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_click_does_nothing_while_a_quasimode_or_question_is_open() {
    // Catches a click that cuts across a held quasimode or a confirmation.
    let dir = temp("modal");
    let mut app = notes(&dir);
    let (lines, hits) = draw(&app);
    let (x, y) = find(&lines, "The programme");
    let c = hits.at(x, y).unwrap().clone();
    let before = app.cursor();
    app.handle(Event::down(Key::Quasi(Quasi::Command)));
    app.click(&c);
    assert_eq!(app.cursor(), before);
    // Released with nothing typed, Command latches; the click still does nothing.
    app.handle(Event::up(Key::Quasi(Quasi::Command)));
    assert!(matches!(app.screen.input.mode, Mode::Quasi(_)));
    app.click(&c);
    assert_eq!(app.cursor(), before);
    tap(&mut app, Key::Escape);
    tap(&mut app, Key::Enter);
    assert!(matches!(app.screen.input.mode, Mode::Normal));
    app.click(&c);
    assert_eq!(
        app.record.outline.text(app.cursor()),
        "The programme",
        "and acts once it is closed"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
