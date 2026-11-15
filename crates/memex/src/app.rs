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

use crate::actions::dispatch::{complete_command, in_quotes};
use crate::engine::Engine;
use crate::journal::{Error as JournalError, Journal};
use crate::notebook::{CONTENT, Notebook};
use crate::outline::{Constraint, Outline, TOP, TextChange, span_json};
use crate::reason::{self, Prov, WorldEval, solutions};
use crate::reasoning::Reasoning;
use crate::session::Session;
use format::record::Entry;
use format::terms::{self, Term};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};
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
    /// Whether a pane of a kind is shown.
    pub(crate) fn shows(&self, kind: PaneKind) -> bool {
        self.panes.iter().any(|p| p.kind == kind)
    }

    /// The pane with focus.
    pub fn focused(&self) -> &Pane {
        &self.panes[self.focus]
    }

    /// Put the focused pane's cursor on an item, the text cursor at a
    /// place in it.
    pub(crate) fn set_cursor(&mut self, id: u64, pos: usize) {
        self.panes[self.focus].cursor = id;
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
        let n = outline.text(self.focused().cursor).chars().count();
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

/// A quasimode in progress, held or latched.
#[derive(Debug, Clone, PartialEq)]
pub struct Active {
    pub q: Quasi,
    pub input: String,
    /// Tapped rather than held: it stays until Enter, or a second tap.
    pub latched: bool,
    /// Another key was pressed while the quasimode key was down.
    pub used: bool,
    /// `?` was pressed: list everything that may come next.
    pub listing: bool,
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
        let notebook = Notebook::open(dir)?;
        let mut app = App {
            notebook,
            reason: Reasoning::default(),
            session: Session::default(),
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

    pub fn cursor(&self) -> u64 {
        self.session.view.focused().cursor
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

    /// Leap's matches for a pattern: items whose text holds it, ignoring
    /// case, in order from the cursor (forward or back), wrapping.
    pub fn leap_matches(&self, pattern: &str, forward: bool) -> Vec<u64> {
        if pattern.is_empty() {
            return vec![];
        }
        let outline = &self.notebook.outline;
        let round = outline.round_from(self.cursor(), forward);
        outline.holding(round, pattern)
    }

    /// Recognition: the pattern, and the longest continuation every match
    /// shares, up to a space.
    fn leap_complete(&self, pattern: &str, forward: bool) -> String {
        let matches = self.leap_matches(pattern, forward).into_iter();
        let texts: Vec<String> = matches.map(|id| self.notebook.outline.text(id)).collect();
        recognised(pattern, &texts)
    }

    pub(crate) fn unfold_to(&mut self, id: u64) {
        let mut at = self.notebook.outline.parent(id);
        while let Some(p) = at {
            self.session.view.folded.remove(&p);
            at = self.notebook.outline.parent(p);
        }
    }

    // ------------------------------------------------------------------
    // Keys.

    pub fn handle(&mut self, ev: Event) {
        let mode = std::mem::replace(&mut self.session.mode, Mode::Normal);
        self.session.mode = match mode {
            Mode::Normal => self.normal(ev),
            Mode::Quasi(a) => self.quasi(a, ev),
            Mode::ConfirmRetract { target, dependents } => self.confirm(target, dependents, ev),
            Mode::ConfirmSuppose { text, readings, at } => {
                self.confirm_suppose(text, readings, at, ev)
            }
        };
    }

    /// A click, from the mouse. It acts only when no quasimode or question
    /// is open, and does what the keyboard would: moving the cursor is a
    /// view change like an arrow key; a command is run as a bound key runs
    /// it, keeping any confirmation it asks for.
    pub fn click(&mut self, c: &Click) {
        if self.session.mode != Mode::Normal {
            return;
        }
        match c {
            Click::Item { pane, id } => {
                if !self.notebook.outline.items.contains_key(id) || *pane > 1 {
                    return;
                }
                self.flush_text();
                self.session.message.clear();
                if self.session.view.focus != *pane {
                    self.session.view.focus = *pane;
                    self.write_view("pane", json!({}));
                }
                self.set_cursor(*id);
                self.session.view_dirty = true;
            }
            Click::Command(line) => {
                self.flush();
                self.session.message.clear();
                self.run_simple(line);
            }
        }
    }

    fn normal(&mut self, ev: Event) -> Mode {
        if ev.press == Press::Up {
            return Mode::Normal;
        }
        if let Key::Quasi(q) = ev.key {
            self.flush();
            if q == Quasi::Scope {
                self.session.view.scope = ScopeLevel::Word;
            }
            return Mode::Quasi(Active {
                q,
                input: String::new(),
                latched: false,
                used: false,
                listing: false,
            });
        }
        if let Some(line) = self.session.keys.command_for(ev.key) {
            self.flush();
            if line == "palette" {
                return Mode::Quasi(Active {
                    q: Quasi::Command,
                    input: String::new(),
                    latched: true,
                    used: false,
                    listing: true,
                });
            }
            self.run_simple(&line);
            // Keep what the command asked for, such as a confirmation.
            return std::mem::replace(&mut self.session.mode, Mode::Normal);
        }
        let f = self.session.view.focus;
        if self.session.view.panes[f].kind == PaneKind::Links {
            self.session.message = format!(
                "this pane shows links; {} to edit",
                self.session.keys.key_for("switch")
            );
            return Mode::Normal;
        }
        self.session.message.clear();
        match ev.key {
            Key::Char(c) => self.type_char(c),
            Key::Backspace => self.erase(true),
            Key::Delete => self.erase(false),
            Key::Enter => {
                self.flush();
                let made = self.new_item_after(self.cursor());
                self.session.report(made);
            }
            Key::Tab => self.run_simple("indent"),
            Key::BackTab => self.run_simple("outdent"),
            Key::Up | Key::Down => {
                self.flush_text();
                let rows = self.visible();
                let i = rows.iter().position(|(id, _)| *id == self.cursor());
                let j = match (i, ev.key) {
                    (Some(i), Key::Up) => i.saturating_sub(1),
                    (Some(i), _) => (i + 1).min(rows.len().saturating_sub(1)),
                    (None, _) => 0,
                };
                if let Some((id, _)) = rows.get(j) {
                    self.set_cursor(*id);
                    self.session.view_dirty = true;
                }
            }
            Key::Left | Key::Right | Key::Home | Key::End => {
                let n = self.text_of(self.cursor()).len();
                self.session.view.pos = match ev.key {
                    Key::Left => self.session.view.pos.saturating_sub(1),
                    Key::Right => (self.session.view.pos + 1).min(n),
                    Key::Home => 0,
                    _ => n,
                };
                self.session.view_dirty = true;
            }
            _ => {}
        }
        Mode::Normal
    }

    fn type_char(&mut self, c: char) {
        if self.cursor() == TOP
            && let Err(m) = self.new_item_after(TOP)
        {
            self.session.message = m;
            return;
        }
        let item = self.cursor();
        let text = self.text_of(item);
        self.session.type_into(item, text, c);
    }

    fn erase(&mut self, before: bool) {
        let item = self.cursor();
        if item == TOP {
            return;
        }
        let text = self.text_of(item);
        self.session.erase_in(item, text, before);
    }

    pub(crate) fn new_item_after(&mut self, id: u64) -> Result<(), String> {
        let (under, after) = if id == TOP {
            (TOP, None)
        } else {
            (self.notebook.outline.parent(id).unwrap_or(TOP), Some(id))
        };
        let e = self.write_here(
            "edit",
            json!({"op": "create", "under": under, "after": after, "add": "", "spans": []}),
        )?;
        self.set_cursor(e.seq);
        self.write_view("move", json!({}));
        Ok(())
    }

    fn quasi(&mut self, mut a: Active, ev: Event) -> Mode {
        // The quasimode's own key: release ends a held quasimode; a tap latches.
        if ev.key == Key::Quasi(a.q) {
            match ev.press {
                Press::Up if !a.used && !a.latched => {
                    a.latched = true;
                    return Mode::Quasi(a);
                }
                Press::Up if a.latched => return Mode::Quasi(a),
                Press::Up => return self.finish(a),
                Press::Down if a.latched => {
                    // A second tap ends a latched quasimode; on an empty Leap it repeats the last one.
                    return self.finish(a);
                }
                Press::Down => return Mode::Quasi(a),
            }
        }
        if ev.press == Press::Up {
            return Mode::Quasi(a);
        }
        a.used = true;
        match ev.key {
            Key::Char('?')
                if a.q != Quasi::Suppose && !(a.q == Quasi::Command && in_quotes(&a.input)) =>
            {
                a.listing = !a.listing
            }
            Key::Char(' ') if a.q == Quasi::Scope => {
                self.session.view.scope = match self.session.view.scope {
                    ScopeLevel::Word => ScopeLevel::Statement,
                    _ => ScopeLevel::Branch,
                };
            }
            Key::Char(c) if a.q != Quasi::Scope => a.input.push(c),
            Key::Backspace => {
                // Erase a character; with none left, Backspace leaves a
                // latched quasimode without acting.
                let erased = a.input.pop().is_some();
                if !erased && a.latched {
                    self.session.message.clear();
                    return Mode::Normal;
                }
            }
            Key::Escape => match a.q {
                Quasi::LeapForward | Quasi::LeapBack => {
                    a.input = self.leap_complete(&a.input, a.q == Quasi::LeapForward)
                }
                Quasi::Command => a.input = complete_command(&a.input),
                _ => {}
            },
            Key::Enter => return self.finish(a),
            _ => {}
        }
        Mode::Quasi(a)
    }

    /// End a quasimode: do what it was for.
    fn finish(&mut self, a: Active) -> Mode {
        match a.q {
            Quasi::LeapForward | Quasi::LeapBack => {
                let forward = a.q == Quasi::LeapForward;
                let pattern = if a.input.is_empty() {
                    match &self.session.last_leap {
                        Some((p, _)) => p.clone(),
                        None => return Mode::Normal,
                    }
                } else {
                    a.input.clone()
                };
                let found = self.leap_matches(&pattern, forward).first().copied();
                match found {
                    Some(id) => {
                        self.unfold_to(id);
                        self.set_cursor(id);
                        self.session.message.clear();
                    }
                    None => self.session.message = format!("no item holds “{pattern}”"),
                }
                self.session.last_leap = Some((pattern.clone(), forward));
                self.write_view(
                    "leap",
                    json!({"pattern": pattern, "forward": forward, "found": found}),
                );
            }
            Quasi::Scope => self.write_view("scope", json!({})),
            Quasi::Suppose => {
                let text = a.input.trim().to_string();
                if text.is_empty() {
                    return Mode::Normal;
                }
                let facts = self
                    .notebook
                    .facts_at(self.notebook.tip)
                    .unwrap_or_default();
                let readings = reason::readings(&facts, &text);
                if readings.is_empty() {
                    // No reading: a named world, with nothing supposed in it.
                    let written = self.write_here("suppose", json!({"name": text}));
                    self.session.report(written);
                } else {
                    return Mode::ConfirmSuppose {
                        text,
                        readings,
                        at: 0,
                    };
                }
            }
            Quasi::Command => return self.run_command(&a.input),
        }
        Mode::Normal
    }

    fn confirm(&mut self, target: u64, dependents: Vec<u64>, ev: Event) -> Mode {
        if ev.press == Press::Up {
            return Mode::ConfirmRetract { target, dependents };
        }
        match ev.key {
            Key::Enter => {
                match self.write_here("retract", json!({"target": target})) {
                    Ok(e) => {
                        self.session.message =
                            format!("entry {target} retracted by entry {}", e.seq)
                    }
                    Err(m) => self.session.message = m,
                }
                Mode::Normal
            }
            Key::Escape => {
                self.session.message = format!("entry {target} kept");
                Mode::Normal
            }
            _ => Mode::ConfirmRetract { target, dependents },
        }
    }

    fn confirm_suppose(&mut self, text: String, readings: Vec<Term>, at: usize, ev: Event) -> Mode {
        if ev.press == Press::Up {
            return Mode::ConfirmSuppose { text, readings, at };
        }
        match ev.key {
            Key::Enter => {
                let fact = readings[at].clone();
                let said = self.suppose(&text, &fact).unwrap_or_else(|m| m);
                self.session.message = said;
                Mode::Normal
            }
            Key::Tab => {
                let at = (at + 1) % readings.len();
                Mode::ConfirmSuppose { text, readings, at }
            }
            Key::Escape => {
                self.session.message = "nothing supposed".into();
                Mode::Normal
            }
            _ => Mode::ConfirmSuppose { text, readings, at },
        }
    }

    /// Sprout a world in which `fact` holds; then set aside, in that world
    /// only, each belief it directly contradicts (the supposition is
    /// entrenched over them). See `direct_conflicts` for what "directly"
    /// means; a violation it leaves is shown, for the user to resolve.
    fn suppose(&mut self, name: &str, fact: &Term) -> Result<String, String> {
        let f = terms::encode(fact).map_err(|e| e.0)?;
        let s = self.write_here("suppose", json!({"name": name, "facts": [f]}))?;
        let (set_aside, left) = self.set_aside_conflicts(s.seq)?;
        self.evaluate()?;
        let places: Vec<String> = set_aside.iter().map(|t| self.fact_place(*t)).collect();
        Ok(with_left(supposed(&places), &left))
    }

    /// Retract, round by round, five at most, each belief that the fact of
    /// an entry directly contradicts: those set aside, and the violations
    /// left.
    fn set_aside_conflicts(&mut self, seq: u64) -> Result<(Vec<u64>, Vec<String>), String> {
        let mut set_aside = Vec::new();
        let mut left = Vec::new();
        for _ in 0..5 {
            let targets;
            (targets, left) = self.retractable_conflicts(seq)?;
            if targets.is_empty() {
                break;
            }
            self.retract_all(&targets)?;
            set_aside.extend(targets);
        }
        Ok((set_aside, left))
    }

    /// After reasoning again, the beliefs that the fact of an entry directly
    /// contradicts and that can be retracted, and the violations left.
    fn retractable_conflicts(&mut self, seq: u64) -> Result<(Vec<u64>, Vec<String>), String> {
        self.evaluate()?;
        let eval = self.reason.eval[0].as_ref().ok_or("no evaluation")?;
        let (mut targets, left) = direct_conflicts(&eval.constraints, seq);
        targets.retain(|t| self.retractable(*t).is_ok());
        Ok((targets, left))
    }

    fn retract_all(&mut self, targets: &[u64]) -> Result<(), String> {
        for t in targets {
            self.write_here("retract", json!({"target": t}))?;
        }
        Ok(())
    }

    /// `set`: find, before writing anything, the beliefs the fact directly
    /// contradicts (see `direct_conflicts`), by evaluating the world with
    /// it added in the preview engine; retract them; then state the fact on
    /// the item that held the first of them, or on `target` if none.
    pub(crate) fn set_fact(&mut self, fact: &Term, target: u64) -> Result<String, String> {
        let mut facts = self.notebook.facts_at(self.notebook.tip)?;
        facts.extend(crate::world::outline_facts(&self.notebook.outline));
        let next = self.notebook.journal.entries().len() as u64 + 1;
        facts.push((next, fact.clone()));
        let constraints: Vec<(u64, String, String)> = self
            .notebook
            .outline
            .constraints
            .iter()
            .map(|(s, c)| (*s, c.name.clone(), c.rule.clone()))
            .collect();
        let engine = self.reason.engine(2)?;
        engine.sync(&facts).map_err(|e| e.to_string())?;
        let mut evals = Vec::new();
        for (seq, name, rule) in constraints {
            let Ok(v) = engine.node(seq) else { continue };
            let (violations, disagreements): (Vec<_>, Vec<_>) = solutions(&v)
                .into_iter()
                .partition(|s| s.prov.iter().all(|p| matches!(p, Prov::Belief(_))));
            evals.push(reason::ConstraintEval {
                seq,
                constraint: Constraint { name, rule },
                violations,
                disagreements,
            });
        }
        let (mut replaced, left) = direct_conflicts(&evals, next);
        replaced.retain(|t| self.retractable(*t).is_ok());
        let item = replaced
            .iter()
            .find_map(|t| self.notebook.outline.fact_item.get(t).copied())
            .unwrap_or(target);
        if item == TOP {
            return Err("there is no item to put the fact on".into());
        }
        let places: Vec<String> = replaced.iter().map(|t| self.fact_place(*t)).collect();
        self.retract_all(&replaced)?;
        self.write_fact(item, fact, None)?;
        let m = if places.is_empty() {
            format!(
                "set at {}; nothing replaced",
                self.notebook.outline.number(item)
            )
        } else {
            format!("set; replaced the fact at {}", places.join(", "))
        };
        Ok(with_left(m, &left))
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

    /// The engine of the current world, for tests that count evaluations.
    pub fn current_engine(&mut self) -> Result<&mut Engine, String> {
        self.reason.engine(0)
    }

    /// Record in the journal what reasoning showed, when it changes: the
    /// current world's violations and disagreements, and the suggestions
    /// shown for the first time. A query and its answer are journal entries
    /// (§ The journal, rule 6), so the measures report can count them.
    pub(crate) fn record_observations(&mut self) {
        self.record_shown();
        self.record_offers();
    }

    /// Record what reasoning shows of the current world, when it changes.
    fn record_shown(&mut self) {
        let Some(now) = self.reason.eval[0].as_ref().map(WorldEval::observed) else {
            return;
        };
        if !self.session.observed_changed(&now) {
            return;
        }
        let world = self.reason.world().to_string();
        let body = json!({"violations": now.0, "disagreements": now.1, "world": world});
        self.write_view("observed", body);
        self.session.set_observed(now);
    }

    /// Record the suggestions shown for the first time, while the pane
    /// shows them.
    fn record_offers(&mut self) {
        if !self.session.view.shows(PaneKind::Suggestions) {
            return;
        }
        let new = self.reason.suggestions.unoffered();
        if new.is_empty() {
            return;
        }
        self.write_view("offer", json!({"keys": new}));
        self.reason.suggestions.offer(new);
    }

    /// What adopting an item's claims would raise, computed in a spare
    /// engine: a preview of adopting it, as a world.
    pub(crate) fn preview_adoption(&mut self, item: u64) -> Result<String, String> {
        let claims: Vec<Term> = self.claims_of(item).into_iter().map(|c| c.2).collect();
        if claims.is_empty() {
            return Ok("no claims to adopt".into());
        }
        let next = self.notebook.entry_count() as u64 + 1;
        let facts = self.notebook.facts_adopting(next, claims)?;
        let constraints = self.notebook.outline.constraint_names();
        let raised = self.reason.raised(&facts, &constraints, next)?;
        Ok(adoption_preview(&raised))
    }

    /// Keep the suggestion pane's order while it has focus.
    pub(crate) fn update_frozen(&mut self) {
        let focused = self.session.view.focused().kind == PaneKind::Suggestions;
        self.reason.suggestions.hold_order(focused);
    }

    /// The items whose facts are about an entity: facts with it first.
    pub(crate) fn items_about(&self, entity: &Term) -> Vec<u64> {
        let facts = self
            .notebook
            .facts_at(self.notebook.tip)
            .unwrap_or_default();
        let mut out: Vec<u64> = facts
            .iter()
            .filter(|(_, f)| matches!(f, Term::Compound(_, a) if a.first() == Some(entity)))
            .filter_map(|(s, _)| self.notebook.outline.fact_item.get(s).copied())
            .collect();
        out.dedup();
        out
    }

    /// The depth of an item in the outline: a top-level item is 0.
    fn depth_of(&self, id: u64) -> usize {
        let mut d = 0;
        let mut at = self.notebook.outline.parent(id);
        while let Some(p) = at.filter(|p| *p != TOP) {
            d += 1;
            at = self.notebook.outline.parent(p);
        }
        d
    }

    /// Write an item for a document block, with its citations as facts.
    fn write_block(
        &mut self,
        b: &crate::docs::Block,
        under: u64,
        after: Option<u64>,
        import: u64,
    ) -> Result<u64, String> {
        let n = b.text.chars().count();
        let e = self.write_here(
            "edit",
            json!({"op": "create", "under": under, "after": after, "add": b.text, "spans": [[0, 0, n]], "style": b.style, "import": import}),
        )?;
        if !b.cites.is_empty() {
            let facts: Vec<serde_json::Value> = b
                .cites
                .iter()
                .map(|k| {
                    terms::encode(&Term::Compound(
                        "cites".into(),
                        vec![Term::Int(e.seq as i64), Term::Atom(k.clone())],
                    ))
                    .map_err(|x| x.0)
                })
                .collect::<Result<_, _>>()?;
            self.write_here(
                "edit",
                json!({"op": "facts", "item": e.seq, "facts": facts}),
            )?;
        }
        Ok(e.seq)
    }

    /// Import a document: its blocks become items, its citations facts, and
    /// what the outline cannot hold is listed as losses. Importing a file of
    /// the same name again is a revision: it opens a world (see `revise`).
    pub(crate) fn import_document(&mut self, file: &str) -> Result<String, String> {
        let path = std::path::Path::new(file);
        let (blocks, losses, _) = crate::docs::read(path)?;
        let bytes = std::fs::read(path).map_err(|e| format!("{file}: {e}"))?;
        let hash = self.notebook.store_blob(&bytes)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let earlier: Vec<u64> = self
            .notebook
            .path(self.notebook.tip)
            .iter()
            .filter(|e| {
                e.kind() == "import"
                    && e.body()
                        .is_some_and(|b| b["format"] == "document" && b["file"] == name.as_str())
            })
            .map(|e| e.seq)
            .collect();
        let lost: Vec<String> = losses.iter().map(|(k, n)| format!("{k} {n}")).collect();
        if !earlier.is_empty() {
            return self.revise(&name, &hash, &blocks, &earlier, &losses);
        }
        let imp = self.write_here(
            "import",
            json!({"blob": hash, "format": "document", "file": name, "source": name, "losses": losses,
                   "prov": {"prov:wasDerivedFrom": name, "prov:wasGeneratedBy": "pandoc"}}),
        )?;
        let mut last_at: Vec<Option<u64>> = vec![self.notebook.outline.top.last().copied()];
        let mut parents: Vec<u64> = vec![TOP];
        for b in &blocks {
            let d = b.depth.min(parents.len() - 1);
            parents.truncate(d + 1);
            last_at.truncate(d + 1);
            if last_at.len() <= d {
                last_at.resize(d + 1, None);
            }
            let under = parents[d];
            let id = self.write_block(b, under, last_at[d], imp.seq)?;
            last_at[d] = Some(id);
            parents.push(id);
            last_at.push(None);
        }
        Ok(format!(
            "imported {name}: {} items; losses: {}",
            blocks.len(),
            if lost.is_empty() {
                "none".into()
            } else {
                lost.join(", ")
            }
        ))
    }

    /// A revised document: align its blocks with the items of the earlier
    /// imports, and write the changes in a supposed world, one entry each,
    /// so each can be kept or retracted before the world is committed.
    fn revise(
        &mut self,
        name: &str,
        hash: &str,
        blocks: &[crate::docs::Block],
        earlier: &[u64],
        losses: &crate::docs::Losses,
    ) -> Result<String, String> {
        let old: Vec<u64> = self
            .notebook
            .outline
            .in_order()
            .into_iter()
            .filter(|i| {
                self.notebook.outline.items[i]
                    .block
                    .origin
                    .is_some_and(|o| earlier.contains(&o))
            })
            .collect();
        let base = old.iter().map(|i| self.depth_of(*i)).min().unwrap_or(0);
        let key = |style: &str, text: &str| (style.to_string(), text.to_string());
        let old_keys: Vec<(String, String)> = old
            .iter()
            .map(|i| {
                key(
                    &self.notebook.outline.items[i].block.style,
                    &self.notebook.outline.text(*i),
                )
            })
            .collect();
        let new_keys: Vec<(String, String)> =
            blocks.iter().map(|b| key(&b.style, &b.text)).collect();
        let matched = crate::docs::align(&old_keys, &new_keys);
        self.write_here("suppose", json!({"name": format!("revision of {name}")}))?;
        let imp = self.write_here(
            "import",
            json!({"blob": hash, "format": "document", "file": name, "source": name, "losses": losses, "revises": earlier.last(),
                   "prov": {"prov:wasDerivedFrom": name, "prov:wasGeneratedBy": "pandoc"}}),
        )?;
        let mut id_of: Vec<Option<u64>> = vec![None; blocks.len()];
        let mut deletes = Vec::new();
        let mut creates = Vec::new();
        let (mut po, mut pn) = (0, 0);
        for &(oi, nj) in matched
            .iter()
            .chain(std::iter::once(&(old.len(), blocks.len())))
        {
            let (olds, news): (Vec<usize>, Vec<usize>) = ((po..oi).collect(), (pn..nj).collect());
            for k in 0..olds.len().max(news.len()) {
                match (olds.get(k), news.get(k)) {
                    (Some(&o), Some(&n)) if old_keys[o].0 == blocks[n].style => {
                        // Changed text: keep the item, write its new text.
                        let new_text: Vec<char> = blocks[n].text.chars().collect();
                        self.write_text(old[o], &new_text)?;
                        id_of[n] = Some(old[o]);
                    }
                    (o, n) => {
                        if let Some(&o) = o {
                            deletes.push(old[o]);
                        }
                        if let Some(&n) = n {
                            creates.push(n);
                        }
                    }
                }
            }
            if oi < old.len() {
                id_of[nj] = Some(old[oi]);
            }
            (po, pn) = (oi + 1, nj + 1);
        }
        for n in creates.iter().copied() {
            let d = blocks[n].depth;
            let parent = (0..n)
                .rev()
                .find(|j| blocks[*j].depth + 1 == d)
                .and_then(|j| id_of[j])
                .unwrap_or(TOP);
            let after = (0..n)
                .rev()
                .take_while(|j| blocks[*j].depth >= d)
                .filter(|j| blocks[*j].depth == d)
                .find_map(|j| id_of[j]);
            let parent = if d == 0 || base > 0 && parent == TOP {
                self.notebook
                    .outline
                    .parent(old.first().copied().unwrap_or(TOP))
                    .unwrap_or(TOP)
            } else {
                parent
            };
            id_of[n] = Some(self.write_block(&blocks[n], parent, after, imp.seq)?);
        }
        let kept: HashSet<u64> = id_of.iter().flatten().copied().collect();
        for o in deletes {
            if !self.notebook.outline.items.contains_key(&o) {
                continue;
            }
            let mut stack = self.notebook.outline.children(o).to_vec();
            let mut keeps_something = false;
            while let Some(x) = stack.pop() {
                keeps_something |= kept.contains(&x);
                stack.extend(self.notebook.outline.children(x));
            }
            if !keeps_something {
                self.write_here("edit", json!({"op": "delete", "item": o}))?;
            }
        }
        Ok(format!(
            "revision of {name}: the changes are in a supposed world; retract any with retract N, then commit or close"
        ))
    }

    /// The changes of a document revision, from the supposition on: (entry
    /// seq, what it does, retracted).
    pub fn revision_changes(&self) -> Vec<(u64, String, bool)> {
        match &self.reason.supposition {
            Some(s) => self.notebook.changes_since(s.actual),
            None => vec![],
        }
    }

    /// Export a section (an item and its branch), or the whole outline, as a
    /// document. Its metadata lists the hashes of the entries its text came
    /// from, so a published text traces back to the record.
    pub(crate) fn export(&mut self, file: &str, section: Option<u64>) -> Result<String, String> {
        let roots: Vec<u64> = match section {
            Some(s) => vec![s],
            None => self.notebook.outline.top.clone(),
        };
        let mut blocks = Vec::new();
        let mut seqs = std::collections::BTreeSet::new();
        for r in roots {
            let base = self.depth_of(r);
            let mut stack = vec![r];
            while let Some(id) = stack.pop() {
                let item = &self.notebook.outline.items[&id];
                let style = if item.block.style.is_empty() {
                    if item.children.is_empty() {
                        "para"
                    } else {
                        "heading"
                    }
                } else {
                    item.block.style.as_str()
                };
                let text = self.notebook.outline.text(id);
                blocks.push(crate::docs::Block {
                    depth: self.depth_of(id) - base,
                    style: style.into(),
                    cites: crate::docs::cite_keys(&text),
                    text,
                });
                seqs.extend(item.spans.iter().map(|s| s.seq));
                seqs.insert(id);
                stack.extend(item.children.iter().rev());
            }
        }
        let hashes: Vec<String> = seqs
            .iter()
            .filter_map(|s| self.notebook.entry(*s).map(|e| e.hash.clone()))
            .collect();
        crate::docs::write(
            &crate::docs::markdown(&blocks),
            std::path::Path::new(file),
            &hashes,
        )?;
        let name = std::path::Path::new(file)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.write_view(
            "export",
            json!({"file": name, "items": blocks.len(), "sources": hashes.len()}),
        );
        Ok(format!(
            "exported {} items to {name}, citing {} source entries",
            blocks.len(),
            hashes.len()
        ))
    }

    /// Import results: an exchange file of values with their provenance (W3C
    /// PROV terms), as claims whose source is the analysis.
    pub(crate) fn import_results(&mut self, file: &str) -> Result<String, String> {
        let path = std::path::Path::new(file);
        let bytes = std::fs::read(path).map_err(|e| format!("{file}: {e}"))?;
        let v: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("not a results file: {e}"))?;
        if v["format"] != "memex-results" {
            return Err("not a results file: its format is not memex-results".into());
        }
        let hash = self.notebook.store_blob(&bytes)?;
        let act = &v["prov"]["activity"];
        let source = format!(
            "{}@{}",
            act["script"].as_str().unwrap_or("analysis"),
            act["commit"].as_str().unwrap_or("uncommitted")
        );
        let base = std::fs::canonicalize(path)
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let imp = self.write_here("import", json!({"blob": hash, "format": "memex-results", "source": source, "base": base, "prov": v["prov"]}))?;
        let head = crate::docs::Block {
            depth: 0,
            style: "heading".into(),
            text: format!("Results of {source}"),
            cites: vec![],
        };
        let top = self.write_block(
            &head,
            TOP,
            self.notebook.outline.top.last().copied(),
            imp.seq,
        )?;
        let mut after = None;
        for r in v["results"].as_array().cloned().unwrap_or_default() {
            let name = r["name"].as_str().ok_or("a result needs a name")?;
            let num = |k: &str| {
                r[k].as_f64()
                    .ok_or(format!("result {name}: {k} must be a number"))
            };
            let (value, u) = (num("value")?, num("uncertainty")?);
            let unit = r["unit"].as_str().unwrap_or("");
            let text = format!("{name} = {value} ± {u} {unit}");
            let b = crate::docs::Block {
                depth: 1,
                style: "para".into(),
                text,
                cites: vec![],
            };
            let id = self.write_block(&b, top, after, imp.seq)?;
            after = Some(id);
            let c = |f: &str, a: Vec<Term>| Term::Compound(f.into(), a);
            let fact = c(
                "claims",
                vec![
                    Term::Str(source.clone()),
                    c(
                        "result",
                        vec![
                            Term::Atom(name.into()),
                            c(
                                "measured",
                                vec![Term::Float(value), Term::Float(u), Term::Str(unit.into())],
                            ),
                        ],
                    ),
                ],
            );
            self.write_fact(id, &fact, Some(&source))?;
        }
        Ok(format!("imported the results of {source}"))
    }

    /// An item's claims: (entry seq, source, claimed fact).
    pub fn claims_of(&self, item: u64) -> Vec<(u64, String, Term)> {
        let facts = self
            .notebook
            .facts_at(self.notebook.tip)
            .unwrap_or_default();
        facts
            .into_iter()
            .filter(|(s, _)| self.notebook.outline.fact_item.get(s) == Some(&item))
            .filter_map(|(s, f)| match f {
                Term::Compound(n, a) if n == "claims" && a.len() == 2 => {
                    let src = match &a[0] {
                        Term::Str(x) | Term::Atom(x) => x.clone(),
                        t => reason::show(t),
                    };
                    Some((s, src, a[1].clone()))
                }
                _ => None,
            })
            .collect()
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

/// A pattern, and the longest continuation after it that every text
/// shares, up to a space; each text holds the pattern.
fn recognised(pattern: &str, texts: &[String]) -> String {
    let p = pattern.to_lowercase();
    let conts: Vec<String> = texts.iter().map(|t| continuation(t, &p)).collect();
    format!("{pattern}{}", common_prefix(&conts))
}

/// The longest start that every word shares; none for no words.
fn common_prefix(words: &[String]) -> String {
    let Some(first) = words.first() else {
        return String::new();
    };
    let rest = words.iter().skip(1);
    rest.fold(first.clone(), |prefix, w| shared_start(&prefix, w))
}

/// The longest start that two words share.
fn shared_start(a: &str, b: &str) -> String {
    let pairs = a.chars().zip(b.chars());
    pairs.take_while(|(x, y)| x == y).map(|(x, _)| x).collect()
}

/// What follows a lower-case pattern in a text, ignoring case, up to a
/// space.
fn continuation(text: &str, p: &str) -> String {
    let t = text.to_lowercase();
    let after = &t[t.find(p).unwrap() + p.len()..];
    after.chars().take_while(|c| !c.is_whitespace()).collect()
}

/// The beliefs a new fact (entry `new`) directly contradicts, and the
/// violations it is part of that have no single repair. A violation that
/// rests on the new fact and exactly one other belief is a direct
/// contradiction: giving up that belief is the only way to keep the new
/// fact and the constraint. A violation resting on two or more other
/// beliefs could be repaired by giving up any of them; which one is the
/// user's choice, so it is left, counted by constraint, as "1 violation
/// of NAME".
fn direct_conflicts(constraints: &[reason::ConstraintEval], new: u64) -> (Vec<u64>, Vec<String>) {
    let found: Vec<_> = constraints.iter().map(|c| conflicts_of(c, new)).collect();
    let mut direct: Vec<u64> = found.iter().flat_map(|(one, _)| one.clone()).collect();
    direct.sort();
    direct.dedup();
    (
        direct,
        found.into_iter().filter_map(|(_, left)| left).collect(),
    )
}

/// Of a constraint's violations that rest on a new fact: the beliefs that
/// those resting on one other belief contradict, and how many rest on
/// more, if any do.
fn conflicts_of(c: &reason::ConstraintEval, new: u64) -> (Vec<u64>, Option<String>) {
    let rests = c.violations.iter().filter_map(|v| others_beside(v, new));
    let (one, many): (Vec<_>, Vec<_>) = rests.partition(|o| o.len() == 1);
    (
        one.concat(),
        reason::violations_of(many.len(), &c.constraint.name),
    )
}

/// The other beliefs a violation rests on, beside a new fact; none if it
/// does not rest on the new fact, or rests on it alone.
fn others_beside(v: &reason::Solution, new: u64) -> Option<Vec<u64>> {
    let seqs = v.prov.iter().map(Prov::seq);
    if !seqs.clone().any(|s| s == new) {
        return None;
    }
    let mut others: Vec<u64> = seqs.filter(|s| *s != new).collect();
    others.sort();
    others.dedup();
    (!others.is_empty()).then_some(others)
}

/// What a supposition says it set aside, at these places.
fn supposed(places: &[String]) -> String {
    if places.is_empty() {
        return "supposed; nothing set aside".into();
    }
    format!("supposed; set aside in this world: {}", places.join(", "))
}

/// A reply, and the violations left for the user, if any are.
fn with_left(said: String, left: &[String]) -> String {
    if left.is_empty() {
        return said;
    }
    format!("{said}; left for you: {}", left.join(", "))
}

/// What adopting a claim would raise, as interrogation shows it.
fn adoption_preview(raised: &[String]) -> String {
    if raised.is_empty() {
        return "adopting it raises no violation".into();
    }
    format!("adopting it would raise {}", raised.join(", "))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reason::{ConstraintEval, Solution, Suggestion};

    /// A constraint, by name, violated once on each of these sets of
    /// beliefs.
    fn violated(name: &str, rests: &[&[u64]]) -> ConstraintEval {
        let constraint = Constraint {
            name: name.into(),
            rule: String::new(),
        };
        let violations = rests.iter().map(|r| on_beliefs(r)).collect();
        let disagreements = vec![];
        ConstraintEval {
            seq: 1,
            constraint,
            violations,
            disagreements,
        }
    }

    /// A solution resting on these beliefs.
    fn on_beliefs(seqs: &[u64]) -> Solution {
        let prov = seqs.iter().map(|s| Prov::Belief(*s)).collect();
        Solution {
            values: vec![],
            prov,
            facts: vec![],
        }
    }

    /// A memex in a directory of this test's own, with an item stating a
    /// fact about an entity x.
    fn about_x(name: &str) -> (std::path::PathBuf, App) {
        let dir =
            std::env::temp_dir().join(format!("adversaria-app-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::open(&dir).unwrap();
        app.new_item_after(TOP).unwrap();
        app.write_fact(
            app.cursor(),
            &Term::Compound("p".into(), vec![Term::Atom("x".into())]),
            None,
        )
        .unwrap();
        (dir, app)
    }

    /// Accept a suggestion of a kind, with nothing else.
    fn accepting(app: &mut App, kind: &str, args: Vec<Term>) -> Result<(), String> {
        let kind = Term::Compound(kind.into(), args);
        let (facts, evidence) = (vec![], String::new());
        app.accept(&Suggestion {
            kind,
            facts,
            evidence,
            score: 0,
        })
    }

    #[test]
    fn a_suggested_constraint_that_does_not_fire_is_not_adopted() {
        let (dir, mut app) = about_x("misfire");
        let x = Term::Var(0);
        let never = Term::Compound("<".into(), vec![x.clone(), x.clone()]);
        let body = Term::Compound(",".into(), vec![Term::Compound("p".into(), vec![x]), never]);
        let args = vec![Term::Str("never".into()), Term::Atom("[]".into()), body];
        let refused = accepting(&mut app, "constraint", args);
        assert!(
            matches!(&refused, Err(e) if e.starts_with("not adopted: tried against a planted")),
            "{refused:?}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_reference_to_a_work_with_no_item_is_not_adopted() {
        let (dir, mut app) = about_x("no-work");
        let args = vec![Term::Atom("x".into()), Term::Atom("y".into())];
        let refused = accepting(&mut app, "reference", args);
        assert_eq!(refused, Err("no item is that work".into()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_belief_named_twice_in_one_violation_is_contradicted_directly() {
        let c = violated("c", &[&[9, 3, 3]]);
        assert_eq!(direct_conflicts(&[c], 9), (vec![3], vec![]));
    }

    #[test]
    fn a_belief_contradicted_twice_is_named_once() {
        let c = violated("c", &[&[9, 3], &[3, 9]]);
        assert_eq!(direct_conflicts(&[c], 9).0, vec![3]);
    }

    #[test]
    fn a_violation_without_the_new_fact_is_none_of_its_conflicts() {
        let c = violated("c", &[&[1, 2]]);
        assert_eq!(direct_conflicts(&[c], 9), (vec![], vec![]));
    }

    #[test]
    fn a_violation_of_the_new_fact_alone_is_not_left() {
        let c = violated("c", &[&[9]]);
        assert_eq!(direct_conflicts(&[c], 9), (vec![], vec![]));
    }

    #[test]
    fn the_violations_left_are_counted_by_constraint() {
        let c = violated("one date", &[&[9, 1, 2], &[9, 3, 4]]);
        assert_eq!(direct_conflicts(&[c], 9).1, ["2 violations of one date"]);
    }
}
