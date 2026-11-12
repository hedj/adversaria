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
use crate::outline::{Constraint, Outline, Span, TOP, span_json};
use crate::reason::{self, ConstraintEval, Prov, Suggestion, WorldEval, solutions};
use format::record::Entry;
use format::terms::{self, Term};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeLevel {
    Word,
    Statement,
    Branch,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pane {
    pub kind: PaneKind,
    pub cursor: u64,
}

/// What is shown. Restored whole from each `view` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub panes: [Pane; 2],
    pub focus: usize,
    /// Text cursor in the focused item, in characters.
    pub pos: usize,
    pub folded: BTreeSet<u64>,
    pub scope: ScopeLevel,
    /// The claim item being interrogated, shown in the links pane.
    pub interrogate: Option<u64>,
}

impl Default for View {
    fn default() -> Self {
        View {
            panes: [
                Pane {
                    kind: PaneKind::Outline,
                    cursor: TOP,
                },
                Pane {
                    kind: PaneKind::Links,
                    cursor: TOP,
                },
            ],
            focus: 0,
            pos: 0,
            folded: BTreeSet::new(),
            scope: ScopeLevel::Statement,
            interrogate: None,
        }
    }
}

impl View {
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

    fn restore(&mut self, b: &Value) {
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

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
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

/// An open supposition: a world sprouted from the actual one.
#[derive(Debug, Clone, PartialEq)]
pub struct Supposition {
    pub name: String,
    /// The head of the actual world it was sprouted from.
    pub actual: u64,
}

/// Text typed into an item but not yet written.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingText {
    item: u64,
    text: Vec<char>,
}

/// The memex's state, in four parts: the record, the reasoner, the screen
/// and the session.
pub struct App {
    pub record: Record,
    pub reasoner: Reasoner,
    pub screen: Screen,
    pub session: Session,
}

/// The journal, and where the current state is in it.
pub struct Record {
    pub(crate) journal: Journal,
    pub outline: Outline,
    /// The entry the current state is at.
    pub tip: u64,
    pub branch: Branch,
}

/// The branch the state is on.
pub struct Branch {
    /// Entries undone and not redone, most recent last.
    pub redo: Vec<u64>,
    /// The name of the current world: "actual", or the last Suppose.
    pub world: String,
    pub supposition: Option<Supposition>,
}

/// Reasoning over the worlds shown, and what it found.
pub struct Reasoner {
    /// The current world's engine, the actual world's while a supposition
    /// is open, and one for previews.
    pub(crate) engines: [Option<Engine>; 3],
    /// What reasoning found: [0] the current world, [1] the actual world
    /// while a supposition is open.
    pub eval: [Option<WorldEval>; 2],
    pub suggestions: Suggestions,
    pub found: Found,
}

/// Suggestions for the current world, and what the user did with them.
pub struct Suggestions {
    /// Best evidence first, less those dismissed.
    pub list: Vec<Suggestion>,
    /// Keys of dismissed suggestions, from the journal.
    pub dismissed: BTreeSet<String>,
    /// While the suggestion pane has focus, its order is kept: a suggestion
    /// never moves under the pointer. Not journaled, like a held key.
    pub(crate) frozen: Option<Vec<String>>,
    /// Suggestions recorded as offered (shown in the pane), by identity.
    pub offered: BTreeSet<String>,
}

/// What else evaluation found.
pub struct Found {
    /// Interrogation's preview of adopting a claim: violations it would raise.
    pub preview: Option<String>,
    /// Results whose input files changed since: item -> those files.
    pub stale: BTreeMap<u64, Vec<String>>,
    /// The last violations and disagreements recorded in the journal.
    pub(crate) observed: Option<(Vec<String>, Vec<String>)>,
}

/// What is shown, and the input in progress.
pub struct Screen {
    pub view: View,
    /// The view has changed and is not yet written.
    pub(crate) dirty: bool,
    pub message: String,
    pub input: Input,
}

/// Keyboard input in progress.
pub struct Input {
    pub mode: Mode,
    /// Text typed into an item but not yet written.
    pub(crate) pending: Option<PendingText>,
    pub(crate) last_leap: Option<(String, bool)>,
    /// Keys bound to command lines, from the configuration.
    pub bindings: Vec<(Key, String)>,
}

/// This running session: its name, where sources are checked, and whether
/// it is to end.
pub struct Session {
    pub workspace: String,
    /// Where the source check asks Crossref: its works API, unless a test
    /// points it elsewhere.
    pub(crate) crossref: String,
    pub quit: bool,
}

const CONTENT: [&str; 6] = ["edit", "link", "name", "suppose", "adopt", "import"];

impl App {
    /// Open a journal directory, writing its first entry if it is new, and
    /// replay it.
    pub fn open(dir: &Path) -> Result<App, JournalError> {
        let journal = Journal::open(dir)?;
        let workspace = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut app = App {
            record: Record {
                journal,
                outline: Outline::default(),
                tip: 0,
                branch: Branch {
                    redo: vec![],
                    world: "actual".into(),
                    supposition: None,
                },
            },
            reasoner: Reasoner {
                engines: [None, None, None],
                eval: [None, None],
                suggestions: Suggestions {
                    list: vec![],
                    dismissed: BTreeSet::new(),
                    frozen: None,
                    offered: BTreeSet::new(),
                },
                found: Found {
                    preview: None,
                    stale: BTreeMap::new(),
                    observed: None,
                },
            },
            screen: Screen {
                view: View::default(),
                dirty: false,
                message: String::new(),
                input: Input {
                    mode: Mode::Normal,
                    pending: None,
                    last_leap: None,
                    bindings: crate::config::Config::default().bindings,
                },
            },
            session: Session {
                workspace,
                crossref: "https://api.crossref.org/works/".into(),
                quit: false,
            },
        };
        if app.record.journal.entries().is_empty() {
            let e = app.record.journal.append("edit", json!({"op": "init"}))?;
            app.apply(&e);
        } else {
            let entries: Vec<Entry> = app.record.journal.entries().to_vec();
            for e in &entries {
                app.apply(e);
            }
        }
        Ok(app)
    }

    pub fn journal(&self) -> &Journal {
        &self.record.journal
    }

    /// Point the source check at another Crossref-like service (for tests).
    pub fn set_crossref(&mut self, base: &str) {
        self.session.crossref = base.to_string();
    }

    pub fn set_durable(&mut self, d: bool) {
        self.record.journal.durable = d;
    }

    pub(crate) fn entry(&self, seq: u64) -> Option<&Entry> {
        self.record
            .journal
            .entries()
            .get((seq as usize).wrapping_sub(1))
    }

    // ------------------------------------------------------------------
    // Applying entries: the one way state changes.

    pub fn apply(&mut self, e: &Entry) {
        // The order before the change is needed only if an item may vanish.
        let may_remove = match e.kind.as_str() {
            "view" => false,
            k if CONTENT.contains(&k) && e.parent == Some(self.record.tip) => {
                e.body.as_ref().is_some_and(|b| b["op"] == "delete")
            }
            _ => true,
        };
        let before = if may_remove {
            self.record.outline.in_order()
        } else {
            vec![]
        };
        match e.kind.as_str() {
            "view" => {
                let jump = e.parent != Some(self.record.tip);
                if let Some(b) = &e.body {
                    self.screen.view.restore(b);
                    if b["op"] == "leap" {
                        self.screen.input.last_leap = b["pattern"]
                            .as_str()
                            .map(|p| (p.to_string(), b["forward"] != false));
                    }
                    if b["op"] == "world" {
                        self.record.branch.supposition = None;
                    }
                    if b["op"] == "dismiss"
                        && let Some(k) = b["key"].as_str()
                    {
                        self.reasoner.suggestions.dismissed.insert(k.to_string());
                    }
                    if b["op"] == "observed" {
                        let keys = |k: &str| {
                            b[k].as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|x| x.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default()
                        };
                        self.reasoner.found.observed =
                            Some((keys("violations"), keys("disagreements")));
                    }
                    if b["op"] == "offer" {
                        self.reasoner.suggestions.offered.extend(
                            b["keys"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|x| x.as_str().map(String::from)),
                        );
                    }
                }
                self.record.tip = e.seq;
                if jump {
                    self.rebuild();
                }
            }
            "undo" => {
                if let Some(t) = e.body.as_ref().and_then(|b| b["undoes"].as_u64()) {
                    self.record.branch.redo.push(t);
                }
                self.record.tip = e.seq;
                self.rebuild();
            }
            "redo" => {
                self.record.branch.redo.pop();
                self.record.tip = e.seq;
                self.rebuild();
            }
            _ => {
                let incremental =
                    e.parent == Some(self.record.tip) && CONTENT.contains(&e.kind.as_str());
                self.record.tip = e.seq;
                if incremental {
                    if let Some(b) = &e.body {
                        self.record.outline.apply(e.seq, &e.kind, b);
                    }
                } else {
                    self.rebuild();
                }
                if e.kind != "purge" {
                    self.record.branch.redo.clear();
                }
                if e.kind == "suppose"
                    && let Some(b) = &e.body
                {
                    let actual = e.parent.unwrap_or(1);
                    let name = b["name"].as_str().unwrap_or("supposed").to_string();
                    self.record.branch.supposition = Some(Supposition { name, actual });
                }
            }
        }
        self.record.branch.world = self
            .record
            .branch
            .supposition
            .as_ref()
            .map_or("actual".into(), |s| s.name.clone());
        self.repair_cursors(&before);
    }

    /// The entries on the path from the tip back to the root, oldest first.
    pub(crate) fn path(&self, tip: u64) -> Vec<&Entry> {
        let mut out = Vec::new();
        let mut at = Some(tip);
        while let Some(s) = at {
            let Some(e) = self.entry(s) else { break };
            out.push(e);
            at = e.parent;
        }
        out.reverse();
        out
    }

    /// Replay the content of a world, leaving out `skip`; returns the
    /// outline, the world's name, and the entries that no longer applied.
    fn replay(&self, tip: u64, skip: Option<u64>) -> (Outline, String, Vec<u64>) {
        let path = self.path(tip);
        let mut retracted: HashSet<u64> = path
            .iter()
            .filter(|e| e.kind == "retract")
            .filter_map(|e| e.body.as_ref().and_then(|b| b["target"].as_u64()))
            .collect();
        retracted.extend(skip);
        let mut o = Outline::default();
        let mut world = "actual".to_string();
        let mut failed = Vec::new();
        for e in path {
            if !CONTENT.contains(&e.kind.as_str()) || retracted.contains(&e.seq) {
                continue;
            }
            let Some(b) = &e.body else { continue };
            if e.kind == "suppose" {
                world = b["name"].as_str().unwrap_or("supposed").to_string();
            }
            if !o.apply(e.seq, &e.kind, b)
                && !matches!(e.kind.as_str(), "suppose" | "adopt" | "import")
            {
                failed.push(e.seq);
            }
        }
        (o, world, failed)
    }

    fn rebuild(&mut self) {
        let (o, _, _) = self.replay(self.record.tip, None);
        self.record.outline = o;
    }

    /// How many entries the journal has.
    pub(crate) fn entry_count(&self) -> usize {
        self.record.journal.entries().len()
    }

    /// The facts stated in the world whose head is an entry.
    pub(crate) fn facts_at(&self, head: u64) -> Result<Vec<(u64, Term)>, String> {
        crate::store::facts(self.record.journal.entries(), head)
    }

    /// The outline of another world, by its head.
    pub fn outline_at(&self, head: u64) -> Outline {
        self.replay(head, None).0
    }

    /// After the outline changes, a cursor on a vanished item moves to the
    /// nearest surviving item before it, else the first.
    fn repair_cursors(&mut self, before: &[u64]) {
        for i in 0..2 {
            let c = self.screen.view.panes[i].cursor;
            if c != TOP && self.record.outline.items.contains_key(&c) {
                continue;
            }
            if before.is_empty() && c != TOP {
                continue; // nothing could have vanished
            }
            let at = before.iter().position(|x| *x == c).unwrap_or(0);
            let fallback = before[..at]
                .iter()
                .rev()
                .find(|x| self.record.outline.items.contains_key(x))
                .copied();
            self.screen.view.panes[i].cursor = fallback
                .or_else(|| self.record.outline.in_order().first().copied())
                .unwrap_or(TOP);
        }
        self.screen
            .view
            .folded
            .retain(|f| self.record.outline.items.contains_key(f));
        let n = self.record.outline.text(self.cursor()).chars().count();
        self.screen.view.pos = self.screen.view.pos.min(n);
    }

    // ------------------------------------------------------------------
    // Writing entries.

    pub(crate) fn write(
        &mut self,
        parent: Option<u64>,
        kind: &str,
        body: Value,
    ) -> Result<Entry, String> {
        let e = self
            .record
            .journal
            .append_after(parent, kind, body)
            .map_err(|e| e.to_string())?;
        self.apply(&e);
        Ok(e)
    }

    pub(crate) fn write_here(&mut self, kind: &str, body: Value) -> Result<Entry, String> {
        self.write(Some(self.record.tip), kind, body)
    }

    /// Write whatever is pending: typed text, then cursor moves.
    pub fn flush(&mut self) {
        self.flush_text();
        if self.screen.dirty {
            self.screen.dirty = false;
            let body = self.screen.view.to_json("move", json!({}));
            if let Err(m) = self.write_here("view", body) {
                self.screen.message = m;
            }
        }
    }

    pub(crate) fn write_view(&mut self, op: &str, extra: Value) {
        self.flush_text();
        self.screen.dirty = false;
        let body = self.screen.view.to_json(op, extra);
        if let Err(m) = self.write_here("view", body) {
            self.screen.message = m;
        }
    }

    pub(crate) fn flush_text(&mut self) {
        if let Some(p) = self.screen.input.pending.take()
            && let Err(m) = self.write_text(p.item, &p.text)
        {
            self.screen.message = m;
        }
    }

    /// Write an item's new text: unchanged characters at the start and end
    /// keep their spans; the middle is added by this entry.
    pub(crate) fn write_text(&mut self, item: u64, new: &[char]) -> Result<(), String> {
        let old: Vec<char> = self.record.outline.text(item).chars().collect();
        if old == new {
            return Ok(());
        }
        let pre = old.iter().zip(new).take_while(|(a, b)| a == b).count();
        let max_suf = old.len().min(new.len()) - pre;
        let suf = old
            .iter()
            .rev()
            .zip(new.iter().rev())
            .take(max_suf)
            .take_while(|(a, b)| a == b)
            .count();
        let added: String = new[pre..new.len() - suf].iter().collect();
        let mut spans = self.record.outline.slice(item, 0, pre);
        if !added.is_empty() {
            spans.push(Span {
                seq: 0,
                start: 0,
                len: added.chars().count(),
            });
        }
        spans.extend(self.record.outline.slice(item, old.len() - suf, old.len()));
        self.write_here(
            "edit",
            json!({"op": "text", "item": item, "add": added, "spans": span_json(&spans)}),
        )?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // What is shown.

    pub fn cursor(&self) -> u64 {
        self.screen.view.panes[self.screen.view.focus].cursor
    }

    pub(crate) fn set_cursor(&mut self, id: u64) {
        let f = self.screen.view.focus;
        self.screen.view.panes[f].cursor = id;
        self.screen.view.pos = self.text_of(id).len();
    }

    /// The text of an item as shown: pending typing included.
    pub fn text_of(&self, id: u64) -> Vec<char> {
        match &self.screen.input.pending {
            Some(p) if p.item == id => p.text.clone(),
            _ => self.record.outline.text(id).chars().collect(),
        }
    }

    pub fn pending(&self) -> bool {
        self.screen.input.pending.is_some() || self.screen.dirty
    }

    /// The items shown in an outline pane, in order, with their depth.
    pub fn visible(&self) -> Vec<(u64, usize)> {
        let mut out = Vec::new();
        let mut stack: Vec<(u64, usize)> = self
            .record
            .outline
            .top
            .iter()
            .rev()
            .map(|i| (*i, 0))
            .collect();
        while let Some((id, d)) = stack.pop() {
            out.push((id, d));
            if !self.screen.view.folded.contains(&id) {
                stack.extend(
                    self.record
                        .outline
                        .children(id)
                        .iter()
                        .rev()
                        .map(|c| (*c, d + 1)),
                );
            }
        }
        out
    }

    /// Leap's matches for a pattern: items whose text holds it, ignoring
    /// case, in order from the cursor (forward or back), wrapping.
    pub fn leap_matches(&self, pattern: &str, forward: bool) -> Vec<u64> {
        if pattern.is_empty() {
            return vec![];
        }
        let p = pattern.to_lowercase();
        let order = self.record.outline.in_order();
        let at = order
            .iter()
            .position(|x| *x == self.cursor())
            .map_or(0, |i| i + 1);
        let mut rotated: Vec<u64> = order[at.min(order.len())..]
            .iter()
            .chain(&order[..at.min(order.len())])
            .copied()
            .collect();
        if !forward {
            rotated.pop();
            rotated.reverse();
            if let Some(c) = order.iter().find(|x| **x == self.cursor()) {
                rotated.push(*c);
            }
        }
        rotated
            .into_iter()
            .filter(|id| self.record.outline.text(*id).to_lowercase().contains(&p))
            .collect()
    }

    /// Recognition: the longest continuation every match shares, up to a space.
    fn leap_complete(&self, pattern: &str, forward: bool) -> String {
        let p = pattern.to_lowercase();
        let conts: Vec<String> = self
            .leap_matches(pattern, forward)
            .iter()
            .map(|id| {
                let t = self.record.outline.text(*id).to_lowercase();
                let i = t.find(&p).unwrap();
                t[i + p.len()..]
                    .chars()
                    .take_while(|c| !c.is_whitespace())
                    .collect()
            })
            .collect();
        let Some(first) = conts.first() else {
            return pattern.to_string();
        };
        let common: String = first
            .chars()
            .enumerate()
            .take_while(|(i, c)| conts.iter().all(|x| x.chars().nth(*i) == Some(*c)))
            .map(|(_, c)| c)
            .collect();
        format!("{pattern}{common}")
    }

    pub(crate) fn unfold_to(&mut self, id: u64) {
        let mut at = self.record.outline.parent(id);
        while let Some(p) = at {
            self.screen.view.folded.remove(&p);
            at = self.record.outline.parent(p);
        }
    }

    // ------------------------------------------------------------------
    // Keys.

    pub fn handle(&mut self, ev: Event) {
        let mode = std::mem::replace(&mut self.screen.input.mode, Mode::Normal);
        self.screen.input.mode = match mode {
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
        if self.screen.input.mode != Mode::Normal {
            return;
        }
        match c {
            Click::Item { pane, id } => {
                if !self.record.outline.items.contains_key(id) || *pane > 1 {
                    return;
                }
                self.flush_text();
                self.screen.message.clear();
                if self.screen.view.focus != *pane {
                    self.screen.view.focus = *pane;
                    self.write_view("pane", json!({}));
                }
                self.set_cursor(*id);
                self.screen.dirty = true;
            }
            Click::Command(line) => {
                self.flush();
                self.screen.message.clear();
                self.run_simple(line);
            }
        }
    }

    /// How to run a command by key: its bound key, else by name.
    pub fn key_for(&self, command: &str) -> String {
        match self
            .screen
            .input
            .bindings
            .iter()
            .find(|(_, c)| c == command)
        {
            Some((k, _)) => crate::config::key_name(*k),
            None => format!("command {command}"),
        }
    }

    fn normal(&mut self, ev: Event) -> Mode {
        if ev.press == Press::Up {
            return Mode::Normal;
        }
        if let Key::Quasi(q) = ev.key {
            self.flush();
            if q == Quasi::Scope {
                self.screen.view.scope = ScopeLevel::Word;
            }
            return Mode::Quasi(Active {
                q,
                input: String::new(),
                latched: false,
                used: false,
                listing: false,
            });
        }
        if let Some(line) = self
            .screen
            .input
            .bindings
            .iter()
            .find(|(k, _)| *k == ev.key)
            .map(|(_, l)| l.clone())
        {
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
            return std::mem::replace(&mut self.screen.input.mode, Mode::Normal);
        }
        let f = self.screen.view.focus;
        if self.screen.view.panes[f].kind == PaneKind::Links {
            self.screen.message =
                format!("this pane shows links; {} to edit", self.key_for("switch"));
            return Mode::Normal;
        }
        self.screen.message.clear();
        match ev.key {
            Key::Char(c) => self.type_char(c),
            Key::Backspace => self.erase(true),
            Key::Delete => self.erase(false),
            Key::Enter => {
                self.flush();
                if let Err(m) = self.new_item_after(self.cursor()) {
                    self.screen.message = m;
                }
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
                    self.screen.dirty = true;
                }
            }
            Key::Left | Key::Right | Key::Home | Key::End => {
                let n = self.text_of(self.cursor()).len();
                self.screen.view.pos = match ev.key {
                    Key::Left => self.screen.view.pos.saturating_sub(1),
                    Key::Right => (self.screen.view.pos + 1).min(n),
                    Key::Home => 0,
                    _ => n,
                };
                self.screen.dirty = true;
            }
            _ => {}
        }
        Mode::Normal
    }

    fn type_char(&mut self, c: char) {
        if self.cursor() == TOP
            && let Err(m) = self.new_item_after(TOP)
        {
            self.screen.message = m;
            return;
        }
        let item = self.cursor();
        let mut text = self.text_of(item);
        let pos = self.screen.view.pos.min(text.len());
        text.insert(pos, c);
        self.screen.input.pending = Some(PendingText { item, text });
        self.screen.view.pos = pos + 1;
        self.screen.dirty = true;
    }

    fn erase(&mut self, before: bool) {
        let item = self.cursor();
        if item == TOP {
            return;
        }
        let mut text = self.text_of(item);
        let pos = self.screen.view.pos.min(text.len());
        if before && pos > 0 {
            text.remove(pos - 1);
            self.screen.view.pos = pos - 1;
        } else if !before && pos < text.len() {
            text.remove(pos);
        } else {
            return;
        }
        self.screen.input.pending = Some(PendingText { item, text });
        self.screen.dirty = true;
    }

    pub(crate) fn new_item_after(&mut self, id: u64) -> Result<(), String> {
        let (under, after) = if id == TOP {
            (TOP, None)
        } else {
            (self.record.outline.parent(id).unwrap_or(TOP), Some(id))
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
                self.screen.view.scope = match self.screen.view.scope {
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
                    self.screen.message.clear();
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
                    match &self.screen.input.last_leap {
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
                        self.screen.message.clear();
                    }
                    None => self.screen.message = format!("no item holds “{pattern}”"),
                }
                self.screen.input.last_leap = Some((pattern.clone(), forward));
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
                let facts = self.facts_at(self.record.tip).unwrap_or_default();
                let readings = reason::readings(&facts, &text);
                if readings.is_empty() {
                    // No reading: a named world, with nothing supposed in it.
                    if let Err(m) = self.write_here("suppose", json!({"name": text})) {
                        self.screen.message = m;
                    }
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
                        self.screen.message = format!("entry {target} retracted by entry {}", e.seq)
                    }
                    Err(m) => self.screen.message = m,
                }
                Mode::Normal
            }
            Key::Escape => {
                self.screen.message = format!("entry {target} kept");
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
                if let Err(m) = self.suppose(&text, &fact) {
                    self.screen.message = m;
                }
                Mode::Normal
            }
            Key::Tab => {
                let at = (at + 1) % readings.len();
                Mode::ConfirmSuppose { text, readings, at }
            }
            Key::Escape => {
                self.screen.message = "nothing supposed".into();
                Mode::Normal
            }
            _ => Mode::ConfirmSuppose { text, readings, at },
        }
    }

    /// Sprout a world in which `fact` holds; then set aside, in that world
    /// only, each belief it directly contradicts (the supposition is
    /// entrenched over them). See `direct_conflicts` for what "directly"
    /// means; a violation it leaves is shown, for the user to resolve.
    fn suppose(&mut self, name: &str, fact: &Term) -> Result<(), String> {
        let f = terms::encode(fact).map_err(|e| e.0)?;
        let s = self.write_here("suppose", json!({"name": name, "facts": [f]}))?;
        let mut set_aside = Vec::new();
        let mut left = Vec::new();
        for _ in 0..5 {
            self.evaluate()?;
            let eval = self.reasoner.eval[0].as_ref().ok_or("no evaluation")?;
            let (mut targets, l) = direct_conflicts(&eval.constraints, s.seq);
            left = l;
            targets.retain(|t| self.retractable(*t).is_ok());
            if targets.is_empty() {
                break;
            }
            for t in targets {
                self.write_here("retract", json!({"target": t}))?;
                set_aside.push(t);
            }
        }
        self.evaluate()?;
        let places: Vec<String> = set_aside.iter().map(|t| self.fact_place(*t)).collect();
        let mut m = if places.is_empty() {
            "supposed; nothing set aside".to_string()
        } else {
            format!("supposed; set aside in this world: {}", places.join(", "))
        };
        if !left.is_empty() {
            m.push_str(&format!("; left for you: {}", left.join(", ")));
        }
        self.screen.message = m;
        Ok(())
    }

    /// `set`: find, before writing anything, the beliefs the fact directly
    /// contradicts (see `direct_conflicts`), by evaluating the world with
    /// it added in the preview engine; retract them; then state the fact on
    /// the item that held the first of them, or on `target` if none.
    pub(crate) fn set_fact(&mut self, fact: &Term, target: u64) -> Result<(), String> {
        let mut facts = self.facts_at(self.record.tip)?;
        facts.extend(crate::world::outline_facts(&self.record.outline));
        let next = self.record.journal.entries().len() as u64 + 1;
        facts.push((next, fact.clone()));
        let constraints: Vec<(u64, String, String)> = self
            .record
            .outline
            .constraints
            .iter()
            .map(|(s, c)| (*s, c.name.clone(), c.rule.clone()))
            .collect();
        let engine = self.engine(2)?;
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
            .find_map(|t| self.record.outline.fact_item.get(t).copied())
            .unwrap_or(target);
        if item == TOP {
            return Err("there is no item to put the fact on".into());
        }
        let places: Vec<String> = replaced.iter().map(|t| self.fact_place(*t)).collect();
        for t in &replaced {
            self.write_here("retract", json!({"target": t}))?;
        }
        let f = terms::encode(fact).map_err(|e| e.0)?;
        self.write_here("edit", json!({"op": "facts", "item": item, "facts": [f]}))?;
        let mut m = if places.is_empty() {
            format!(
                "set at {}; nothing replaced",
                self.record.outline.number(item)
            )
        } else {
            format!("set; replaced the fact at {}", places.join(", "))
        };
        if !left.is_empty() {
            m.push_str(&format!("; left for you: {}", left.join(", ")));
        }
        self.screen.message = m;
        Ok(())
    }

    /// Where a fact entry's fact is stated, in the actual world: its item's
    /// number, or "entry n".
    pub fn fact_place(&self, seq: u64) -> String {
        let head = self
            .record
            .branch
            .supposition
            .as_ref()
            .map_or(self.record.tip, |s| s.actual);
        let o = self.outline_at(head);
        match o.fact_item.get(&seq) {
            Some(i) => o.number(*i),
            None => format!("entry {seq}"),
        }
    }

    pub(crate) fn engine(&mut self, k: usize) -> Result<&mut Engine, String> {
        crate::world::started(&mut self.reasoner.engines[k])
    }

    /// The engine of the current world, for tests that count evaluations.
    pub fn current_engine(&mut self) -> Result<&mut Engine, String> {
        self.engine(0)
    }

    /// Record in the journal what reasoning showed, when it changes: the
    /// current world's violations and disagreements, and the suggestions
    /// shown for the first time. A query and its answer are journal entries
    /// (§ The journal, rule 6), so the measures report can count them.
    pub(crate) fn record_observations(&mut self) {
        let Some(e) = &self.reasoner.eval[0] else {
            return;
        };
        let key = |c: &ConstraintEval, s: &reason::Solution| {
            let mut f: Vec<String> = s.facts.iter().map(reason::show).collect();
            f.sort();
            format!("{}: {}", c.constraint.name, f.join(" · "))
        };
        let mut v: Vec<String> = e
            .constraints
            .iter()
            .flat_map(|c| c.violations.iter().map(move |s| key(c, s)))
            .collect();
        let mut d: Vec<String> = e
            .constraints
            .iter()
            .flat_map(|c| c.disagreements.iter().map(move |s| key(c, s)))
            .collect();
        v.sort();
        d.sort();
        let now = (v, d);
        let changed = match &self.reasoner.found.observed {
            Some(o) => *o != now,
            None => !now.0.is_empty() || !now.1.is_empty(),
        };
        if changed {
            let world = self.record.branch.world.clone();
            self.write_view(
                "observed",
                json!({"violations": now.0, "disagreements": now.1, "world": world}),
            );
            self.reasoner.found.observed = Some(now);
        }
        if self
            .screen
            .view
            .panes
            .iter()
            .any(|p| p.kind == PaneKind::Suggestions)
        {
            let new: Vec<String> = self
                .shown_suggestions()
                .iter()
                .map(|s| s.identity())
                .filter(|k| !self.reasoner.suggestions.offered.contains(k))
                .collect();
            if !new.is_empty() {
                self.write_view("offer", json!({"keys": new}));
                self.reasoner.suggestions.offered.extend(new);
            }
        }
    }

    /// What adopting an item's claims would raise, computed in a spare
    /// engine: a preview of adopting it, as a world.
    pub(crate) fn preview_adoption(&mut self, item: u64) -> Result<String, String> {
        let mut facts = self.facts_at(self.record.tip)?;
        let next = self.record.journal.entries().len() as u64 + 1;
        let claims: Vec<Term> = self
            .claims_of(item)
            .into_iter()
            .map(|(_, _, p)| p)
            .collect();
        if claims.is_empty() {
            return Ok("no claims to adopt".into());
        }
        facts.extend(claims.into_iter().map(|p| (next, p)));
        let constraints: Vec<(u64, String)> = self
            .record
            .outline
            .constraints
            .iter()
            .map(|(s, c)| (*s, c.name.clone()))
            .collect();
        let engine = self.engine(2)?;
        engine.sync(&facts).map_err(|e| e.to_string())?;
        let mut raised = Vec::new();
        for (seq, name) in constraints {
            let Ok(v) = engine.node(seq) else { continue };
            let n = solutions(&v)
                .iter()
                .filter(|s| {
                    s.prov.iter().all(|p| matches!(p, Prov::Belief(_)))
                        && s.prov.iter().any(|p| p.seq() == next)
                })
                .count();
            if n > 0 {
                raised.push(format!(
                    "{n} violation{} of {name}",
                    if n == 1 { "" } else { "s" }
                ));
            }
        }
        Ok(if raised.is_empty() {
            "adopting it raises no violation".into()
        } else {
            format!("adopting it would raise {}", raised.join(", "))
        })
    }

    /// The Wikidata import in a world: the last import entry on its path,
    /// not retracted, and the file its blob is in.
    pub(crate) fn import_on(&self, head: u64) -> Option<(u64, std::path::PathBuf)> {
        let path = self.path(head);
        let retracted: HashSet<u64> = path
            .iter()
            .filter(|e| e.kind == "retract")
            .filter_map(|e| e.body.as_ref().and_then(|b| b["target"].as_u64()))
            .collect();
        path.iter()
            .rev()
            .filter(|e| e.kind == "import" && !retracted.contains(&e.seq))
            .find_map(|e| {
                let b = e.body.as_ref()?;
                let blob = b["blob"].as_str()?;
                (b["source"] == "Wikidata").then(|| {
                    (
                        e.seq,
                        self.record
                            .journal
                            .dir()
                            .join(format::record::BLOBS)
                            .join(blob),
                    )
                })
            })
    }

    /// The suggestions in the order shown: while the suggestion pane has
    /// focus, those already shown keep their places and new ones go last.
    pub fn shown_suggestions(&self) -> Vec<&Suggestion> {
        let mut out: Vec<&Suggestion> = Vec::new();
        if let Some(order) = &self.reasoner.suggestions.frozen {
            for k in order {
                if let Some(s) = self
                    .reasoner
                    .suggestions
                    .list
                    .iter()
                    .find(|s| &s.identity() == k)
                {
                    out.push(s);
                }
            }
        }
        for s in &self.reasoner.suggestions.list {
            if !out.iter().any(|o| o.identity() == s.identity()) {
                out.push(s);
            }
        }
        out.truncate(5); // few in view
        out
    }

    /// Keep the suggestion pane's order while it has focus.
    pub(crate) fn update_frozen(&mut self) {
        let focused = self.screen.view.panes[self.screen.view.focus].kind == PaneKind::Suggestions;
        self.reasoner.suggestions.frozen = match (focused, self.reasoner.suggestions.frozen.take())
        {
            (true, Some(f)) => Some(f),
            (true, None) => Some(
                self.shown_suggestions()
                    .iter()
                    .map(|s| s.identity())
                    .collect(),
            ),
            (false, _) => None,
        };
    }

    /// The items whose facts are about an entity: facts with it first.
    fn items_about(&self, entity: &Term) -> Vec<u64> {
        let facts = self.facts_at(self.record.tip).unwrap_or_default();
        let mut out: Vec<u64> = facts
            .iter()
            .filter(|(_, f)| matches!(f, Term::Compound(_, a) if a.first() == Some(entity)))
            .filter_map(|(s, _)| self.record.outline.fact_item.get(s).copied())
            .collect();
        out.dedup();
        out
    }

    /// Adopt a suggestion: one entry, marked as an adopted suggestion and
    /// carrying its evidence, so it can be found and retracted later. A
    /// constraint is tried against a planted violation first.
    pub(crate) fn accept(&mut self, s: &Suggestion) -> Result<(), String> {
        let mark = json!({"kind": reason::show(&s.kind), "evidence": s.evidence, "score": s.score});
        let enc = |t: &Term| terms::encode(t).map_err(|e| e.0);
        let seq = self.record.journal.entries().len() as u64 + 1;
        let c = |f: &str, args: Vec<Term>| Term::Compound(f.into(), args);
        match (s.kind_name(), &s.kind) {
            ("constraint", Term::Compound(_, a)) if a.len() == 3 => {
                let planted = self
                    .engine(0)?
                    .plant(&a[1], &a[2])
                    .map_err(|e| e.to_string())?;
                if !matches!(&planted, Term::Compound(n, _) if n == "fires") {
                    return Err(format!(
                        "not adopted: tried against a planted violation, it did not fire ({})",
                        reason::show(&planted)
                    ));
                }
                let name = match &a[0] {
                    Term::Str(x) | Term::Atom(x) => x.clone(),
                    t => reason::show(t),
                };
                let def = c(
                    "node_def",
                    vec![
                        Term::Int(seq as i64),
                        c(
                            "constraint",
                            vec![Term::Str(name.clone()), a[1].clone(), a[2].clone()],
                        ),
                    ],
                );
                self.write_here(
                    "adopt",
                    json!({"suggestion": mark, "op": "constraint", "name": name, "rule": format!("false :- {}", reason::show(&a[2])), "facts": [enc(&def)?]}),
                )?;
            }
            ("rule", Term::Compound(_, a)) if a.len() == 2 => {
                let def = c(
                    "rule_def",
                    vec![Term::Int(seq as i64), a[0].clone(), a[1].clone()],
                );
                self.write_here("adopt", json!({"suggestion": mark, "facts": [enc(&def)?]}))?;
            }
            ("reference", Term::Compound(_, a)) if a.len() == 2 => {
                let from = *self
                    .items_about(&a[0])
                    .first()
                    .ok_or("no item states facts about that entity")?;
                let to = *self
                    .items_about(&a[1])
                    .first()
                    .ok_or("no item is that work")?;
                let fact = c("cites", vec![Term::Int(from as i64), Term::Int(to as i64)]);
                self.write_here("link", json!({"from": from, "to": to, "type": "cites", "facts": [enc(&fact)?], "suggestion": mark}))?;
            }
            _ => {
                let facts: Vec<serde_json::Value> =
                    s.facts.iter().map(enc).collect::<Result<_, _>>()?;
                self.write_here("adopt", json!({"suggestion": mark, "facts": facts}))?;
            }
        }
        Ok(())
    }

    /// Copy a file into blobs/, named by its SHA-256; return the hash.
    pub(crate) fn store_blob(&self, bytes: &[u8]) -> Result<String, String> {
        let hash = format::record::sha256_hex(bytes);
        let blobs = self.record.journal.dir().join(format::record::BLOBS);
        std::fs::create_dir_all(&blobs).map_err(|e| e.to_string())?;
        let dest = blobs.join(&hash);
        if !dest.exists() {
            std::fs::write(&dest, bytes).map_err(|e| e.to_string())?;
        }
        Ok(hash)
    }

    /// The depth of an item in the outline: a top-level item is 0.
    fn depth_of(&self, id: u64) -> usize {
        let mut d = 0;
        let mut at = self.record.outline.parent(id);
        while let Some(p) = at.filter(|p| *p != TOP) {
            d += 1;
            at = self.record.outline.parent(p);
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
    pub(crate) fn import_document(&mut self, file: &str) -> Result<(), String> {
        let path = std::path::Path::new(file);
        let (blocks, losses, _) = crate::docs::read(path)?;
        let bytes = std::fs::read(path).map_err(|e| format!("{file}: {e}"))?;
        let hash = self.store_blob(&bytes)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let earlier: Vec<u64> = self
            .path(self.record.tip)
            .iter()
            .filter(|e| {
                e.kind == "import"
                    && e.body
                        .as_ref()
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
        let mut last_at: Vec<Option<u64>> = vec![self.record.outline.top.last().copied()];
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
        self.screen.message = format!(
            "imported {name}: {} items; losses: {}",
            blocks.len(),
            if lost.is_empty() {
                "none".into()
            } else {
                lost.join(", ")
            }
        );
        Ok(())
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
    ) -> Result<(), String> {
        let old: Vec<u64> = self
            .record
            .outline
            .in_order()
            .into_iter()
            .filter(|i| {
                self.record.outline.items[i]
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
                    &self.record.outline.items[i].style,
                    &self.record.outline.text(*i),
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
                self.record
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
            if !self.record.outline.items.contains_key(&o) {
                continue;
            }
            let mut stack = self.record.outline.children(o).to_vec();
            let mut keeps_something = false;
            while let Some(x) = stack.pop() {
                keeps_something |= kept.contains(&x);
                stack.extend(self.record.outline.children(x));
            }
            if !keeps_something {
                self.write_here("edit", json!({"op": "delete", "item": o}))?;
            }
        }
        self.screen.message = format!(
            "revision of {name}: the changes are in a supposed world; retract any with retract N, then commit or close"
        );
        Ok(())
    }

    /// The changes of a document revision, from the supposition on: (entry
    /// seq, what it does, retracted).
    pub fn revision_changes(&self) -> Vec<(u64, String, bool)> {
        let Some(s) = &self.record.branch.supposition else {
            return vec![];
        };
        let path = self.path(self.record.tip);
        let retracted: HashSet<u64> = path
            .iter()
            .filter(|e| e.kind == "retract")
            .filter_map(|e| e.body.as_ref().and_then(|b| b["target"].as_u64()))
            .collect();
        let actual = self.outline_at(s.actual);
        let mut out = Vec::new();
        for e in path.iter().filter(|e| e.seq > s.actual && e.kind == "edit") {
            let Some(b) = &e.body else { continue };
            let item = b["item"].as_u64().unwrap_or(e.seq);
            let num = |o: &Outline| o.number(item);
            let what = match b["op"].as_str() {
                Some("text") => format!(
                    "changed {}: “{}” → “{}”",
                    num(&actual),
                    actual.text(item),
                    self.outline_at(e.seq).text(item)
                ),
                Some("create") => format!("added: “{}”", b["add"].as_str().unwrap_or("")),
                Some("delete") => format!("deleted {}: “{}”", num(&actual), actual.text(item)),
                _ => continue,
            };
            out.push((e.seq, what, retracted.contains(&e.seq)));
        }
        out
    }

    /// Export a section (an item and its branch), or the whole outline, as a
    /// document. Its metadata lists the hashes of the entries its text came
    /// from, so a published text traces back to the record.
    pub(crate) fn export(&mut self, file: &str, section: Option<u64>) -> Result<(), String> {
        let roots: Vec<u64> = match section {
            Some(s) => vec![s],
            None => self.record.outline.top.clone(),
        };
        let mut blocks = Vec::new();
        let mut seqs = std::collections::BTreeSet::new();
        for r in roots {
            let base = self.depth_of(r);
            let mut stack = vec![r];
            while let Some(id) = stack.pop() {
                let item = &self.record.outline.items[&id];
                let style = if item.style.is_empty() {
                    if item.children.is_empty() {
                        "para"
                    } else {
                        "heading"
                    }
                } else {
                    item.style.as_str()
                };
                let text = self.record.outline.text(id);
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
            .filter_map(|s| self.entry(*s).map(|e| e.hash.clone()))
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
        self.screen.message = format!(
            "exported {} items to {name}, citing {} source entries",
            blocks.len(),
            hashes.len()
        );
        Ok(())
    }

    /// Import results: an exchange file of values with their provenance (W3C
    /// PROV terms), as claims whose source is the analysis.
    pub(crate) fn import_results(&mut self, file: &str) -> Result<(), String> {
        let path = std::path::Path::new(file);
        let bytes = std::fs::read(path).map_err(|e| format!("{file}: {e}"))?;
        let v: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("not a results file: {e}"))?;
        if v["format"] != "memex-results" {
            return Err("not a results file: its format is not memex-results".into());
        }
        let hash = self.store_blob(&bytes)?;
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
        let top = self.write_block(&head, TOP, self.record.outline.top.last().copied(), imp.seq)?;
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
            self.write_here("edit", json!({"op": "facts", "item": id, "source": source, "facts": [terms::encode(&fact).map_err(|e| e.0)?]}))?;
        }
        self.screen.message = format!("imported the results of {source}");
        Ok(())
    }

    /// Items whose results rest on input files that changed since: each with
    /// the files that no longer match their recorded hashes.
    pub fn stale_results(&self) -> BTreeMap<u64, Vec<String>> {
        let mut out = BTreeMap::new();
        for e in self.path(self.record.tip) {
            let Some(b) = e
                .body
                .as_ref()
                .filter(|b| e.kind == "import" && b["format"] == "memex-results")
            else {
                continue;
            };
            let base = std::path::Path::new(b["base"].as_str().unwrap_or(""));
            let mut changed = Vec::new();
            for input in b["prov"]["used"].as_array().cloned().unwrap_or_default() {
                let p = input["path"].as_str().unwrap_or("");
                // Only a regular file is read: a pipe would block forever,
                // and a device such as /dev/zero never ends. Anything else
                // counts as changed.
                let path = base.join(p);
                let now = std::fs::metadata(&path)
                    .ok()
                    .filter(|m| m.is_file())
                    .and_then(|_| std::fs::read(&path).ok())
                    .map(|x| format::record::sha256_hex(&x));
                if now.as_deref() != input["sha256"].as_str() {
                    changed.push(p.to_string());
                }
            }
            if !changed.is_empty() {
                for (id, item) in &self.record.outline.items {
                    if item.origin == Some(e.seq) {
                        out.insert(*id, changed.clone());
                    }
                }
            }
        }
        out
    }

    /// The quote check: does the item's text still appear in the document it
    /// was imported from? Spaces, hyphens and case are ignored.
    pub(crate) fn check_quote(&mut self, item: u64) -> Result<String, String> {
        let origin = self
            .record
            .outline
            .items
            .get(&item)
            .and_then(|i| i.origin)
            .ok_or("the target was not imported from a document")?;
        let e = self.entry(origin).ok_or("no import entry")?;
        let b = e.body.clone().ok_or("the import was purged")?;
        let file = b["file"].as_str().unwrap_or("");
        let fmt = crate::docs::format_of(std::path::Path::new(file))
            .ok_or("the import is not a document")?;
        let blob = self
            .record
            .journal
            .dir()
            .join(format::record::BLOBS)
            .join(b["blob"].as_str().unwrap_or(""));
        let source = crate::docs::plain_text(&blob, fmt)?;
        let span = crate::docs::markdown_to_plain(&self.record.outline.text(item))?;
        let ruling = if crate::docs::normal(&source).contains(&crate::docs::normal(&span)) {
            "quoted"
        } else {
            "misquoted"
        };
        self.write_ruling(item, "quote", ruling, b["blob"].as_str().unwrap_or(""))?;
        Ok(ruling.into())
    }

    /// The source check: fetch the item's DOI from Crossref, never trusting a
    /// copy that came with the claim, and rule: found, misattributed (the
    /// title differs from the one claimed), not_found, unavailable, retracted.
    pub(crate) fn check_source(&mut self, item: u64) -> Result<String, String> {
        let facts = self.facts_at(self.record.tip)?;
        let mine: Vec<&Term> = facts
            .iter()
            .filter(|(s, _)| self.record.outline.fact_item.get(s) == Some(&item))
            .map(|(_, f)| f)
            .collect();
        let field = |name: &str| {
            mine.iter().find_map(|f| match f {
                Term::Compound(n, a) if n == name && a.len() == 2 => match &a[1] {
                    Term::Str(s) => Some(s.clone()),
                    _ => None,
                },
                _ => None,
            })
        };
        let doi = field("doi").ok_or("the target states no doi(Entity, \"10.…\") fact")?;
        let url = format!("{}{}", self.session.crossref, encode_doi(&doi));
        let out = std::process::Command::new("curl")
            .args([
                // No URL patterns ([1-9], {a,b}): one request, for this DOI.
                "--globoff",
                // Only the web, and no more than Crossref's records need.
                "--proto",
                "=http,https",
                "--max-filesize",
                "10000000",
                "-sS",
                "--max-time",
                "30",
                "--noproxy",
                "localhost,127.0.0.1",
                "-A",
                "source-check/0.1 (prototype)",
                "-w",
                "\n%{http_code}",
            ])
            .arg(&url)
            .output()
            .map_err(|e| format!("cannot run curl: {e}"))?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let (body, code) = text.rsplit_once('\n').unwrap_or(("", ""));
        let hash = format::record::sha256_hex(body.as_bytes());
        let ruling = match code.trim() {
            "404" => "not_found",
            "200" => match serde_json::from_str::<serde_json::Value>(body) {
                Ok(v) => {
                    let m = &v["message"];
                    let retracted = m["updated-by"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|u| u["type"] == "retraction"))
                        || m["relation"]["is-retracted-by"]
                            .as_array()
                            .is_some_and(|a| !a.is_empty());
                    let title = m["title"][0].as_str().unwrap_or("");
                    if retracted {
                        "retracted"
                    } else if field("title")
                        .is_some_and(|t| crate::docs::normal(&t) != crate::docs::normal(title))
                    {
                        "misattributed"
                    } else {
                        "found"
                    }
                }
                Err(_) => "unavailable",
            },
            _ => "unavailable",
        };
        self.write_ruling(item, "source", ruling, &hash)?;
        Ok(ruling.into())
    }

    /// A ruling is a fact about the claim, with the hash of what it examined.
    fn write_ruling(
        &mut self,
        item: u64,
        check: &str,
        ruling: &str,
        hash: &str,
    ) -> Result<(), String> {
        let c = |f: &str, a: Vec<Term>| Term::Compound(f.into(), a);
        let fact = c(
            "ruling",
            vec![
                Term::Int(item as i64),
                Term::Atom(check.into()),
                Term::Atom(ruling.into()),
                Term::Str(hash.into()),
            ],
        );
        self.write_here(
            "edit",
            json!({"op": "facts", "item": item, "facts": [terms::encode(&fact).map_err(|e| e.0)?]}),
        )?;
        Ok(())
    }

    /// An item's claims: (entry seq, source, claimed fact).
    pub fn claims_of(&self, item: u64) -> Vec<(u64, String, Term)> {
        let facts = self.facts_at(self.record.tip).unwrap_or_default();
        facts
            .into_iter()
            .filter(|(s, _)| self.record.outline.fact_item.get(s) == Some(&item))
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
            let e = self.entry(s)?;
            if CONTENT.contains(&e.kind.as_str()) || e.kind == "retract" {
                return Some(s);
            }
            at = e.parent;
        }
        None
    }

    /// Check that an entry can be retracted; return the entries that depend
    /// on it: those that would no longer apply without it.
    pub fn retractable(&self, t: u64) -> Result<Vec<u64>, String> {
        let path = self.path(self.record.tip);
        let e = path
            .iter()
            .find(|e| e.seq == t)
            .ok_or(format!("entry {t} is not in this world"))?;
        if t == 1 || !CONTENT.contains(&e.kind.as_str()) {
            return Err(format!("entry {t} ({}) cannot be retracted", e.kind));
        }
        let already = path.iter().any(|x| {
            x.kind == "retract" && x.body.as_ref().and_then(|b| b["target"].as_u64()) == Some(t)
        });
        if already {
            return Err(format!("entry {t} is already retracted"));
        }
        let (_, _, with) = self.replay(self.record.tip, None);
        let (_, _, without) = self.replay(self.record.tip, Some(t));
        Ok(without
            .into_iter()
            .filter(|s| !with.contains(s) && *s != t)
            .collect())
    }
}

/// A DOI as a URL path: every byte but letters, digits, "-._~" and "/"
/// percent-encoded, as Crossref's API asks (RFC 3986's unreserved set).
pub fn encode_doi(doi: &str) -> String {
    doi.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
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
    let mut direct = Vec::new();
    let mut left = Vec::new();
    for c in constraints {
        let mut n = 0;
        for v in &c.violations {
            if !v.prov.iter().any(|p| p.seq() == new) {
                continue;
            }
            let mut others: Vec<u64> = v.prov.iter().map(Prov::seq).filter(|q| *q != new).collect();
            others.sort();
            others.dedup();
            match others.as_slice() {
                [one] => direct.push(*one),
                [] => {}
                _ => n += 1,
            }
        }
        if n > 0 {
            left.push(format!(
                "{n} violation{} of {}",
                if n == 1 { "" } else { "s" },
                c.constraint.name
            ));
        }
    }
    direct.sort();
    direct.dedup();
    (direct, left)
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
