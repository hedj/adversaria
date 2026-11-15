//! What the memex's tests share: a memex in a room of its own, driven by
//! keys as a user drives it. Each test file uses some of it.
#![allow(dead_code)]

use memex::app::{App, Event, Key, Quasi, View};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::path::PathBuf;

/// A memex in a directory of this test's own, removed after it.
pub struct Room {
    pub dir: PathBuf,
    pub app: App,
}

impl Room {
    pub fn new(name: &str) -> Room {
        let id = std::process::id();
        let dir = std::env::temp_dir().join(format!("adversaria-room-{name}-{id}"));
        let _ = std::fs::remove_dir_all(&dir);
        let app = App::open(&dir).unwrap();
        Room { dir, app }
    }

    pub fn tap(&mut self, keys: &[Key]) {
        for k in keys {
            self.app.handle(Event::down(*k));
            self.app.handle(Event::up(*k));
        }
    }

    pub fn typ(&mut self, text: &str) {
        for c in text.chars() {
            self.tap(&[Key::Char(c)]);
        }
    }

    /// Hold a quasimode's key while typing, and keep holding it.
    pub fn hold(&mut self, q: Quasi, text: &str) {
        self.app.handle(Event::down(Key::Quasi(q)));
        self.typ(text);
    }

    pub fn release(&mut self, q: Quasi) {
        self.app.handle(Event::up(Key::Quasi(q)));
    }

    pub fn command(&mut self, line: &str) {
        self.hold(Quasi::Command, line);
        self.release(Quasi::Command);
    }

    /// Leap to the next item, forward or back, whose text holds a pattern.
    pub fn leap(&mut self, q: Quasi, pattern: &str) {
        self.hold(q, pattern);
        self.release(q);
    }

    /// Items of these texts, each after the last, written.
    pub fn items(&mut self, texts: &[&str]) {
        for (i, text) in texts.iter().enumerate() {
            if i > 0 {
                self.tap(&[Key::Enter]);
            }
            self.typ(text);
        }
        self.app.flush();
    }

    /// The text of an item, as shown.
    pub fn text(&self, item: u64) -> String {
        self.app.text_of(item).iter().collect()
    }

    /// The lines drawn on a screen of 80 by 20.
    pub fn lines(&self) -> Vec<String> {
        let mut t = Terminal::new(TestBackend::new(80, 20)).unwrap();
        t.draw(|f| memex::ui::draw(f, &self.app)).unwrap();
        let buf = t.backend().buffer().clone();
        let row = |y| (0..80).map(|x| buf[(x, y)].symbol()).collect::<String>();
        (0..20).map(row).collect()
    }

    pub fn view(&self) -> &View {
        &self.app.session.view
    }

    /// The spans of the item at the cursor, as written: for each, the
    /// entry it came from and its length.
    pub fn spans(&self) -> Vec<(u64, usize)> {
        let item = &self.app.notebook.outline.items[&self.app.cursor()];
        item.spans.iter().map(|s| (s.seq, s.len)).collect()
    }

    /// How many entries of a kind the journal holds.
    pub fn count(&self, kind: &str) -> usize {
        let entries = self.app.journal().entries().iter();
        entries.filter(|e| e.kind() == kind).count()
    }

    /// The text of the item at the cursor, as shown.
    pub fn here(&self) -> String {
        self.text(self.app.cursor())
    }
}

impl Drop for Room {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
