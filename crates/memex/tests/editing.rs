//! Typing and erasing in an item, and where the cursors go when the
//! outline changes (docs/plan.md, Milestone 3).

use memex::app::{App, Event, Key, Quasi};
use std::path::PathBuf;

/// A memex in a directory of this test's own, removed after it.
struct Room {
    dir: PathBuf,
    app: App,
}

impl Room {
    fn new(name: &str) -> Room {
        let id = std::process::id();
        let dir = std::env::temp_dir().join(format!("adversaria-editing-{name}-{id}"));
        let _ = std::fs::remove_dir_all(&dir);
        let app = App::open(&dir).unwrap();
        Room { dir, app }
    }

    fn tap(&mut self, keys: &[Key]) {
        for k in keys {
            self.app.handle(Event::down(*k));
            self.app.handle(Event::up(*k));
        }
    }

    fn typ(&mut self, text: &str) {
        for c in text.chars() {
            self.tap(&[Key::Char(c)]);
        }
    }

    fn command(&mut self, line: &str) {
        self.app.handle(Event::down(Key::Quasi(Quasi::Command)));
        self.typ(line);
        self.app.handle(Event::up(Key::Quasi(Quasi::Command)));
    }

    /// Items of these texts, each after the last, written.
    fn items(&mut self, texts: &[&str]) {
        for (i, text) in texts.iter().enumerate() {
            if i > 0 {
                self.tap(&[Key::Enter]);
            }
            self.typ(text);
        }
        self.app.flush();
    }

    /// The text of an item, as shown.
    fn text(&self, item: u64) -> String {
        self.app.text_of(item).iter().collect()
    }

    /// The text of the item at the cursor, as shown.
    fn here(&self) -> String {
        self.text(self.app.cursor())
    }
}

impl Drop for Room {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn backspace_erases_before_the_text_cursor_and_moves_it_back() {
    let mut r = Room::new("backspace");
    r.items(&["abc"]);
    r.tap(&[Key::Left, Key::Left, Key::Backspace]);
    r.typ("X");
    assert_eq!(r.here(), "Xbc");
}

#[test]
fn delete_erases_after_the_text_cursor() {
    let mut r = Room::new("delete");
    r.items(&["abc"]);
    r.tap(&[Key::Left, Key::Left, Key::Delete]);
    assert_eq!(r.here(), "ac");
}

#[test]
fn text_typed_into_one_item_is_shown_in_that_item_alone() {
    let mut r = Room::new("typed");
    r.items(&["one", "two"]);
    r.typ("!");
    let first = r.app.visible()[0].0;
    assert_eq!((r.text(first), r.here()), ("one".into(), "two!".into()));
}

#[test]
fn a_moved_cursor_waits_to_be_written() {
    let mut r = Room::new("moved");
    r.items(&["one", "two"]);
    r.tap(&[Key::Up]);
    assert!(r.app.pending(), "the move is not yet written");
    r.app.flush();
    assert!(!r.app.pending());
}

#[test]
fn moving_to_an_item_puts_the_text_cursor_at_its_end() {
    let mut r = Room::new("end");
    r.items(&["one", "two"]);
    r.tap(&[Key::Home, Key::Up]);
    r.typ("!");
    assert_eq!(r.here(), "one!");
}

#[test]
fn deleting_the_item_at_the_cursor_moves_it_to_the_item_before() {
    let mut r = Room::new("before");
    r.items(&["one", "two", "three"]);
    r.command("delete");
    assert_eq!(r.here(), "two");
}

#[test]
fn deleting_the_first_item_moves_the_cursor_to_the_next() {
    let mut r = Room::new("first");
    r.items(&["one", "two"]);
    r.tap(&[Key::Up]);
    r.command("delete");
    assert_eq!(r.here(), "two");
}

#[test]
fn a_deleted_item_leaves_the_folds() {
    let mut r = Room::new("folds");
    r.items(&["one", "two"]);
    let two = r.app.cursor();
    r.command("fold");
    r.command("delete");
    assert!(!r.app.view.folded.contains(&two));
}

#[test]
fn the_text_cursor_keeps_within_the_item_it_moves_to() {
    let mut r = Room::new("pos");
    r.items(&["a", "bcdef"]);
    r.command("delete");
    assert_eq!(r.app.view.pos, 1);
}

#[test]
fn a_write_that_fails_is_shown() {
    let mut r = Room::new("fails");
    r.typ("one");
    std::fs::remove_dir_all(&r.dir).unwrap();
    r.app.flush();
    assert!(r.app.message.contains("os error"), "{}", r.app.message);
}
