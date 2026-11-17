//! The memex's interface state, apart from the terminal: key events in,
//! journal entries out (docs/design.md § The outline model, § Quasimodes).
//!
//! Every change of state goes through `apply(entry)`, both live and when a
//! journal is replayed, so replay reproduces the screen. Two things are not
//! journaled: a quasimode while it is held (its input is shown, then becomes
//! one entry on release), and typing or cursor moves not yet written, which
//! are merged into one entry until another kind of action, `flush`, or quit.
//!
//! Entry kinds the interface writes: `edit`, `link`, `name` and `suppose`
//! change the outline; `undo` and `redo` move along the current branch (an
//! undo entry's parent is the state it returns to); `retract` reverses one
//! earlier entry and keeps all later work; `view` records what is shown
//! (cursor, folds, panes, scope) and, in its `op`, the action: move, leap,
//! follow, fold, pane, scope. Counting actions by kind reads these.

use crate::journal::{Error as JournalError, Journal};
use crate::notebook::{CONTENT, Notebook};
use crate::outline::{Outline, TOP, TextChange, span_json};
use crate::reasoning::Reasoning;
use crate::session::Session;
use format::record::Entry;
use format::terms::Term;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quasi {
    LeapForward,
    LeapBack,
    Command,
    Scope,
    Suppose,
}

impl Quasi {
    /// Every quasimode, in order.
    pub const ALL: [Quasi; 5] = [
        Quasi::LeapForward,
        Quasi::LeapBack,
        Quasi::Command,
        Quasi::Scope,
        Quasi::Suppose,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Quasi::LeapForward => "LEAP",
            Quasi::LeapBack => "LEAP BACK",
            Quasi::Command => "COMMAND",
            Quasi::Scope => "SCOPE",
            Quasi::Suppose => "SUPPOSE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Escape,
    Tab,
    BackTab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    F(u8),
    /// A key with no editing job, free to bind (crate::config).
    Special(Special),
    Quasi(Quasi),
}

/// What a mouse click acts on (crate::ui::Hits says where each is drawn).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Click {
    /// An item's line in an outline pane: the cursor goes there.
    Item { pane: usize, id: u64 },
    /// A hint or a suggestion: the command line it stands for runs.
    Command(String),
}

/// Keys the memex leaves free for the user's bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Special {
    Insert,
    PageUp,
    PageDown,
    Menu,
    Pause,
    ScrollLock,
    PrintScreen,
    CapsLock,
    NumLock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub key: Key,
    pub press: Press,
}

impl Event {
    pub fn down(key: Key) -> Event {
        Event {
            key,
            press: Press::Down,
        }
    }
    pub fn up(key: Key) -> Event {
        Event {
            key,
            press: Press::Up,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    Outline,
    Links,
    Suggestions,
}

impl PaneKind {
    /// The kind of pane a word names, as the pane command takes it.
    pub(crate) fn named(word: Option<&str>) -> Option<PaneKind> {
        match word {
            Some("outline") => Some(Self::Outline),
            Some("links") => Some(Self::Links),
            Some("suggestions") => Some(Self::Suggestions),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScopeLevel {
    Word,
    #[default]
    Statement,
    Branch,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pane {
    pub kind: PaneKind,
    pub cursor: u64,
}

/// The two panes, side by side: at first the outline, and beside it its
/// links.
#[derive(Debug, Clone, PartialEq)]
pub struct Panes(pub [Pane; 2]);

/// What is shown. Restored whole from each `view` entry.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct View {
    pub panes: Panes,
    pub focus: usize,
    /// Text cursor in the focused item, in characters.
    pub pos: usize,
    pub folded: BTreeSet<u64>,
    pub scope: ScopeLevel,
    /// The claim item being interrogated, shown in the links pane.
    pub interrogate: Option<u64>,
}

impl Default for Panes {
    fn default() -> Panes {
        let pane = |kind| Pane { kind, cursor: TOP };
        Panes([pane(PaneKind::Outline), pane(PaneKind::Links)])
    }
}

impl std::ops::Deref for Panes {
    type Target = [Pane; 2];
    fn deref(&self) -> &[Pane; 2] {
        &self.0
    }
}

impl std::ops::DerefMut for Panes {
    fn deref_mut(&mut self) -> &mut [Pane; 2] {
        &mut self.0
    }
}

impl View {
    /// The selected item: the cursor of the outline with focus, else of
    /// the other pane. A pane that is not an outline has no cursor of its
    /// own: it shows what is selected.
    pub(crate) fn selected(&self) -> u64 {
        self.panes[self.selecting()].cursor
    }

    /// The pane whose cursor is the selection: the one with focus if it is
    /// an outline, else the other.
    fn selecting(&self) -> usize {
        let outline = self.panes[self.focus].kind == PaneKind::Outline;
        if outline { self.focus } else { 1 - self.focus }
    }

    /// What pane i shows.
    pub(crate) fn pane_kind(&self, i: usize) -> PaneKind {
        self.panes[i].kind
    }

    /// Whether a pane of a kind is shown.
    pub(crate) fn shows(&self, kind: PaneKind) -> bool {
        self.panes.iter().any(|p| p.kind == kind)
    }

    /// The pane with focus.
    pub fn focused(&self) -> &Pane {
        &self.panes[self.focus]
    }

    /// Select an item, the text cursor at a place in it.
    pub(crate) fn set_cursor(&mut self, id: u64, pos: usize) {
        let i = self.selecting();
        self.panes[i].cursor = id;
        self.pos = pos;
    }

    /// After the outline changes, each cursor keeps to an item that is
    /// left, folds to the items left, and the text cursor to its item's
    /// text.
    fn repair(&mut self, outline: &Outline, before: &[u64]) {
        for pane in self.panes.iter_mut() {
            pane.cursor = surviving(outline, before, pane.cursor);
        }
        self.folded.retain(|f| outline.items.contains_key(f));
        let n = outline.text(self.selected()).chars().count();
        self.pos = self.pos.min(n);
    }

    /// The pane without focus.
    fn other(&mut self) -> &mut Pane {
        &mut self.panes[1 - self.focus]
    }

    /// Show a kind of pane, at an item, in the pane without focus.
    pub(crate) fn show_other(&mut self, kind: PaneKind, at: u64) {
        let p = self.other();
        p.kind = kind;
        p.cursor = at;
    }

    /// Move focus to the other pane; it starts at this pane's cursor if it
    /// has none.
    pub(crate) fn switch_focus(&mut self) {
        let from = self.panes[self.focus].cursor;
        self.focus = 1 - self.focus;
        let p = &mut self.panes[self.focus];
        if p.cursor == TOP {
            p.cursor = from;
        }
    }

    /// Interrogate a claim item: show it in the links pane.
    pub(crate) fn show_claim(&mut self, item: u64) {
        self.interrogate = Some(item);
        self.panes[1].kind = PaneKind::Links;
    }

    /// Whether commands act on the word at the cursor.
    pub(crate) fn in_word(&self) -> bool {
        self.scope == ScopeLevel::Word
    }

    pub(crate) fn to_json(&self, op: &str, extra: Value) -> Value {
        let kind = |k: PaneKind| match k {
            PaneKind::Outline => "outline",
            PaneKind::Links => "links",
            PaneKind::Suggestions => "suggestions",
        };
        let scope = match self.scope {
            ScopeLevel::Word => "word",
            ScopeLevel::Statement => "statement",
            ScopeLevel::Branch => "branch",
        };
        let mut v = json!({
            "op": op,
            "panes": self.panes.iter().map(|p| json!({"kind": kind(p.kind), "cursor": p.cursor})).collect::<Vec<_>>(),
            "focus": self.focus, "pos": self.pos, "folded": self.folded.iter().collect::<Vec<_>>(), "scope": scope, "interrogate": self.interrogate,
        });
        if let (Value::Object(m), Value::Object(x)) = (&mut v, extra) {
            m.extend(x);
        }
        v
    }

    pub(crate) fn restore(&mut self, b: &Value) {
        if let Some(ps) = b.get("panes").and_then(|p| p.as_array()) {
            for (i, p) in ps.iter().take(2).enumerate() {
                self.panes[i].kind = match p["kind"].as_str() {
                    Some("links") => PaneKind::Links,
                    Some("suggestions") => PaneKind::Suggestions,
                    _ => PaneKind::Outline,
                };
                self.panes[i].cursor = p["cursor"].as_u64().unwrap_or(TOP);
            }
        }
        self.focus = (b["focus"].as_u64().unwrap_or(0) as usize).min(1);
        self.pos = b["pos"].as_u64().unwrap_or(0) as usize;
        self.folded = b["folded"]
            .as_array()
            .map(|f| f.iter().filter_map(|x| x.as_u64()).collect())
            .unwrap_or_default();
        self.interrogate = b["interrogate"].as_u64();
        self.scope = match b["scope"].as_str() {
            Some("word") => ScopeLevel::Word,
            Some("branch") => ScopeLevel::Branch,
            _ => ScopeLevel::Statement,
        };
    }
}

/// Where a cursor goes after the outline changes: it stays on an item
/// that is left, or where nothing could have vanished; else it goes to the
/// nearest item before it that is left, else the first.
fn surviving(outline: &Outline, before: &[u64], c: u64) -> u64 {
    if c != TOP && (outline.contains(c) || before.is_empty()) {
        return c;
    }
    let at = before.iter().position(|x| *x == c).unwrap_or(0);
    let earlier = before[..at].iter().rev().find(|x| outline.contains(**x));
    let first = || outline.in_order().first().copied();
    earlier.copied().or_else(first).unwrap_or(TOP)
}

/// Where a new item goes: under an item, and after one of its children,
/// or first.
pub(crate) struct Spot {
    pub(crate) under: u64,
    pub(crate) after: Option<u64>,
}

/// A quasimode in progress, held or latched.
#[derive(Debug, Clone, PartialEq)]
pub struct Active {
    pub q: Quasi,
    pub input: String,
    /// How its key is held.
    pub hold: Hold,
    /// `?` was pressed: list everything that may come next.
    pub listing: bool,
    /// In Suppose, what APE makes of what is typed.
    pub hint: Option<Hint>,
}

/// What APE makes of a supposition being typed, in words (or why there is
/// no parser; none if nothing is typed), the text Esc puts in its place,
/// and, while `?` lists, what may come next with a line of help.
#[derive(Debug, Clone, PartialEq)]
pub struct Hint {
    pub said: Option<String>,
    pub mended: Option<String>,
    pub listing: Vec<String>,
}

/// How a quasimode's key is held: down, with nothing else pressed yet;
/// down after another key was pressed (its release ends the quasimode);
/// or latched, tapped rather than held, so it stays until Enter or a
/// second tap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hold {
    Down,
    Used,
    Latched,
}

impl Active {
    /// Whether it was tapped rather than held, and stays.
    pub fn latched(&self) -> bool {
        self.hold == Hold::Latched
    }

    /// Whether it lists over the outline: a Leap or Command, listing
    /// everything or with something typed.
    pub(crate) fn lists(&self) -> bool {
        let lister = matches!(
            self.q,
            Quasi::LeapForward | Quasi::LeapBack | Quasi::Command
        );
        let suppose = self.q == Quasi::Suppose && self.listing;
        suppose || lister && (self.listing || !self.input.is_empty())
    }

    /// A quasimode whose key is held, nothing typed yet.
    pub(crate) fn held(q: Quasi) -> Active {
        let input = String::new();
        Active {
            q,
            input,
            hold: Hold::Down,
            listing: false,
            hint: None,
        }
    }

    /// Put the first repair of what is typed in its place, if there is
    /// one (Esc in Suppose).
    pub(crate) fn apply_repair(&mut self) {
        let mended = self.hint.as_ref().and_then(|h| h.mended.clone());
        if let Some(m) = mended {
            self.input = m;
        }
    }

    /// The command palette: the Command quasimode, latched, listing what
    /// may come next.
    pub(crate) fn palette() -> Active {
        let held = Active::held(Quasi::Command);
        Active {
            hold: Hold::Latched,
            listing: true,
            ..held
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Quasi(Active),
    /// A retraction waits for Enter; the list is what depends on the entry.
    ConfirmRetract {
        target: u64,
        dependents: Vec<u64>,
    },
    /// "Do you mean?": a supposition's readings, the one shown, waiting for
    /// Enter (yes), Esc (no) or Tab (the next reading).
    ConfirmSuppose {
        text: String,
        readings: Vec<Term>,
        at: usize,
    },
}

/// Text typed into an item but not yet written.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingText {
    pub(crate) item: u64,
    pub(crate) text: Vec<char>,
}

/// The memex: the notebook, what reasoning found in it, and what this
/// session holds that the journal does not.
pub struct App {
    pub notebook: Notebook,
    pub reason: Reasoning,
    pub session: Session,
}

impl App {
    /// Open a journal directory, writing its first entry if it is new, and
    /// replay it.
    pub fn open(dir: &Path) -> Result<App, JournalError> {
        let (notebook, note) = Notebook::open(dir)?;
        let session = Session::saying(note.unwrap_or_default());
        let mut app = App {
            notebook,
            reason: Reasoning::default(),
            session,
        };
        app.apply_all();
        Ok(app)
    }

    /// Apply each entry of the journal, in order.
    fn apply_all(&mut self) {
        let entries = self.notebook.journal.entries().to_vec();
        for e in &entries {
            self.apply(e);
        }
    }

    pub fn journal(&self) -> &Journal {
        &self.notebook.journal
    }

    pub fn set_durable(&mut self, d: bool) {
        self.notebook.journal.durable = d;
    }

    // ------------------------------------------------------------------
    // Applying entries: the one way state changes.

    pub fn apply(&mut self, e: &Entry) {
        let before = self.notebook.order_before(e);
        self.notebook.apply(e);
        self.reason.apply(e);
        self.session.apply(e);
        self.session.view.repair(&self.notebook.outline, &before);
    }

    // ------------------------------------------------------------------
    // Writing entries.

    pub(crate) fn write(
        &mut self,
        parent: Option<u64>,
        kind: &str,
        body: Value,
    ) -> Result<Entry, String> {
        let e = self.notebook.append(parent, kind, body)?;
        self.apply(&e);
        Ok(e)
    }

    pub(crate) fn write_here(&mut self, kind: &str, body: Value) -> Result<Entry, String> {
        self.write(Some(self.notebook.tip), kind, body)
    }

    /// Write whatever is pending: typed text, then cursor moves.
    pub fn flush(&mut self) {
        self.flush_text();
        if self.session.view_dirty {
            self.session.view_dirty = false;
            let body = self.session.view.to_json("move", json!({}));
            let written = self.write_here("view", body);
            self.session.report(written);
        }
    }

    pub(crate) fn write_view(&mut self, op: &str, extra: Value) {
        self.flush_text();
        self.session.view_dirty = false;
        let body = self.session.view.to_json(op, extra);
        let written = self.write_here("view", body);
        self.session.report(written);
    }

    pub(crate) fn flush_text(&mut self) {
        let Some(p) = self.session.take_text() else {
            return;
        };
        let written = self.write_text(p.item, &p.text);
        self.session.report(written);
    }

    /// Write an item's new text: unchanged characters at the start and end
    /// keep their spans; the middle is added by this entry.
    pub(crate) fn write_text(&mut self, item: u64, new: &[char]) -> Result<(), String> {
        let outline = &self.notebook.outline;
        let old: Vec<char> = outline.text(item).chars().collect();
        let Some(change) = TextChange::between(&old, new) else {
            return Ok(());
        };
        let spans = span_json(&outline.spans_after(item, &change));
        let body = json!({"op": "text", "item": item, "add": change.added, "spans": spans});
        self.write_here("edit", body)?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // What is shown.

    /// The selected item (View::selected).
    pub fn cursor(&self) -> u64 {
        self.session.view.selected()
    }

    pub(crate) fn set_cursor(&mut self, id: u64) {
        let end = self.text_of(id).len();
        self.session.view.set_cursor(id, end);
    }

    /// The text of an item as shown: pending typing included.
    pub fn text_of(&self, id: u64) -> Vec<char> {
        match self.session.typed(id) {
            Some(text) => text.to_vec(),
            None => self.notebook.outline.text(id).chars().collect(),
        }
    }

    pub fn pending(&self) -> bool {
        self.session.pending()
    }

    /// The items shown in an outline pane, in order, with their depth.
    pub fn visible(&self) -> Vec<(u64, usize)> {
        let folded = &self.session.view.folded;
        self.notebook.outline.visible(folded)
    }

    /// Where a fact entry's fact is stated, in the actual world: its item's
    /// number, or "entry n".
    pub fn fact_place(&self, seq: u64) -> String {
        let head = self.reason.actual_head(self.notebook.tip);
        let o = self.notebook.outline_at(head);
        match o.fact_item.get(&seq) {
            Some(i) => o.number(*i),
            None => format!("entry {seq}"),
        }
    }

    /// The items whose facts are about an entity: facts with it first.
    pub(crate) fn items_about(&self, entity: &Term) -> Vec<u64> {
        let facts = self.notebook.facts_now().unwrap_or_default();
        let mut out: Vec<u64> = facts
            .iter()
            .filter(|(_, f)| matches!(f, Term::Compound(_, a) if a.first() == Some(entity)))
            .filter_map(|(s, _)| self.notebook.outline.fact_item.get(s).copied())
            .collect();
        out.dedup();
        out
    }

    /// The changes of a document revision, from the supposition on: (entry
    /// seq, what it does, retracted).
    pub fn revision_changes(&self) -> Vec<(u64, String, bool)> {
        match &self.reason.supposition {
            Some(s) => self.notebook.changes_since(s.actual),
            None => vec![],
        }
    }

    // ------------------------------------------------------------------
    // Commands.

    /// The last entry on the path that changed content: what undo undoes.
    pub(crate) fn content_tip(&self, from: u64) -> Option<u64> {
        let mut at = Some(from);
        while let Some(s) = at {
            let e = self.notebook.entry(s)?;
            if CONTENT.contains(&e.kind()) || e.kind() == "retract" {
                return Some(s);
            }
            at = e.parent_seq();
        }
        None
    }

    /// Check that an entry can be retracted; return the entries that depend
    /// on it: those that would no longer apply without it.
    pub fn retractable(&self, t: u64) -> Result<Vec<u64>, String> {
        let branch = self.notebook.branch(self.notebook.tip);
        let e = branch
            .entries
            .iter()
            .find(|e| e.seq == t)
            .ok_or(format!("entry {t} is not in this world"))?;
        if t == 1 || !CONTENT.contains(&e.kind()) {
            return Err(format!("entry {t} ({}) cannot be retracted", e.kind()));
        }
        if branch.retracts(t) {
            return Err(format!("entry {t} is already retracted"));
        }
        let with = self.notebook.replay(self.notebook.tip, None).failed;
        let without = self.notebook.replay(self.notebook.tip, Some(t)).failed;
        Ok(without
            .into_iter()
            .filter(|s| !with.contains(s) && *s != t)
            .collect())
    }
}

/// The word around a text position: [start, end).
pub(crate) fn word_at(text: &[char], pos: usize) -> (usize, usize) {
    let pos = pos.min(text.len());
    let mut a = pos;
    while a > 0 && !text[a - 1].is_whitespace() {
        a -= 1;
    }
    let mut b = pos;
    while b < text.len() && !text[b].is_whitespace() {
        b += 1;
    }
    while b < text.len() && text[b].is_whitespace() {
        b += 1;
    }
    (a, b)
}
