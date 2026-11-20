//! What the memex's tests share: a memex in a room of its own, driven by
//! keys as a user drives it. Each test file uses some of it.
#![allow(dead_code)]

use memex::app::{App, Event, Key, Quasi, View};
use memex::ui::Presentations;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

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
        self.lines_at(80, 20)
    }

    /// What is drawn on a screen of a width and a height.
    pub fn screen(&self, width: u16, height: u16) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| memex::ui::draw(f, &self.app)).unwrap();
        t.backend().buffer().clone()
    }

    /// The lines drawn on a screen of a width and a height.
    pub fn lines_at(&self, width: u16, height: u16) -> Vec<String> {
        let buf = self.screen(width, height);
        let row = |y| (0..width).map(|x| buf[(x, y)].symbol()).collect::<String>();
        (0..height).map(row).collect()
    }

    /// The style a text is first drawn in on a wide screen: its first
    /// cell's.
    pub fn style_of(&self, text: &str) -> Style {
        let (x, y) = self.find(text);
        self.screen(160, 40)[(x, y)].style()
    }

    /// Where a text is first drawn on a wide screen: its column and row.
    pub fn find(&self, text: &str) -> (u16, u16) {
        let lines = self.lines_at(160, 40);
        let y = lines.iter().position(|l| l.contains(text)).expect(text);
        let before = lines[y].split(text).next().unwrap_or_default();
        (before.chars().count() as u16, y as u16)
    }

    /// What a click would act on where, on a wide screen.
    pub fn hits(&self) -> Presentations {
        self.hits_at(160, 40)
    }

    /// What a click would act on where, on a screen of a width and a height.
    pub fn hits_at(&self, width: u16, height: u16) -> Presentations {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut hits = Presentations::default();
        t.draw(|f| hits = memex::ui::draw_hits(f, &self.app))
            .unwrap();
        hits
    }

    /// Whether a line of a wide screen holds this text.
    pub fn shows(&self, text: &str) -> bool {
        self.lines_at(160, 40).iter().any(|l| l.contains(text))
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

    /// Bind keys as a configuration's [keys] section does.
    pub fn bind(&mut self, keys: &str) {
        let config = memex::config::Config::parse(&format!("[keys]\n{keys}\n"));
        self.app.session.keys = config.unwrap().keys();
    }

    /// Put the focused pane's cursor on an item, or on none (TOP).
    pub fn put_cursor(&mut self, item: u64) {
        let view = &mut self.app.session.view;
        view.panes[view.focus].cursor = item;
    }

    /// The facts stated in the world at the tip, as shown.
    pub fn facts(&self) -> Vec<String> {
        let entries = self.app.journal().entries();
        let facts = memex::store::facts(entries, self.app.notebook.tip).unwrap();
        facts.iter().map(|(_, f)| memex::reason::show(f)).collect()
    }

    /// The body of an entry, by its number.
    pub fn body_of(&self, seq: u64) -> serde_json::Value {
        let entry = &self.app.journal().entries()[seq as usize - 1];
        entry.body().cloned().unwrap_or_default()
    }

    /// The number of the last entry of a kind.
    pub fn last(&self, kind: &str) -> u64 {
        let mut entries = self.app.journal().entries().iter().rev();
        entries.find(|e| e.kind() == kind).unwrap().seq
    }

    /// How many entries of a kind the journal holds.
    pub fn count(&self, kind: &str) -> usize {
        let entries = self.app.journal().entries().iter();
        entries.filter(|e| e.kind() == kind).count()
    }

    /// The outline as it stands.
    pub fn outline(&self) -> &memex::outline::Outline {
        &self.app.notebook.outline
    }

    /// How many entries the journal holds.
    pub fn entry_count(&self) -> usize {
        self.app.journal().entries().len()
    }

    /// The text of the item at the cursor, as shown.
    pub fn here(&self) -> String {
        self.text(self.app.cursor())
    }
}

/// A room is its memex, to the helpers that drive an App.
impl std::ops::Deref for Room {
    type Target = App;
    fn deref(&self) -> &App {
        &self.app
    }
}

impl std::ops::DerefMut for Room {
    fn deref_mut(&mut self) -> &mut App {
        &mut self.app
    }
}

impl Drop for Room {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A room's journal opened afresh, as replay builds it. The room stays open
/// in its own memex, which holds it alone, so a copy of its directory is
/// opened: the same journal, under the same name.
pub fn replayed(dir: &Path) -> App {
    static COPIES: AtomicUsize = AtomicUsize::new(0);
    let n = COPIES.fetch_add(1, Ordering::Relaxed);
    let id = std::process::id();
    let home = std::env::temp_dir().join(format!("adversaria-replay-{id}-{n}"));
    let copy = home.join(dir.file_name().expect("a room has a name"));
    let _ = std::fs::remove_dir_all(&home);
    copy_dir(dir, &copy);
    App::open(&copy).unwrap()
}

/// Copy a directory and what it holds.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for item in std::fs::read_dir(from).unwrap() {
        let path = item.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        match path.is_dir() {
            true => copy_dir(&path, &target),
            false => drop(std::fs::copy(&path, &target).unwrap()),
        }
    }
}
