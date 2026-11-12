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

use crate::engine::{Engine, variables};
use crate::journal::{Error as JournalError, Journal};
use crate::outline::{Outline, Span, TOP, span_json};
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
    fn to_json(&self, op: &str, extra: Value) -> Value {
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
struct PendingText {
    item: u64,
    text: Vec<char>,
}

pub struct App {
    journal: Journal,
    pub outline: Outline,
    pub view: View,
    pub mode: Mode,
    /// The entry the current state is at.
    pub tip: u64,
    /// Entries undone and not redone, most recent last.
    pub redo: Vec<u64>,
    /// The name of the current world: "actual", or the last Suppose.
    pub world: String,
    pub message: String,
    pub workspace: String,
    pending_text: Option<PendingText>,
    view_dirty: bool,
    last_leap: Option<(String, bool)>,
    pub quit: bool,
    pub supposition: Option<Supposition>,
    /// Reasoning: the current world's engine, the actual world's while a
    /// supposition is open, and one for previews.
    engines: [Option<Engine>; 3],
    /// What reasoning found: [0] the current world, [1] the actual world
    /// while a supposition is open.
    pub eval: [Option<WorldEval>; 2],
    /// Interrogation's preview of adopting a claim: violations it would raise.
    pub preview: Option<String>,
    /// Suggestions for the current world, best evidence first, less those
    /// dismissed.
    pub suggestions: Vec<Suggestion>,
    /// Keys of dismissed suggestions, from the journal.
    pub dismissed: BTreeSet<String>,
    /// While the suggestion pane has focus, its order is kept: a suggestion
    /// never moves under the pointer. Not journaled, like a held key.
    frozen: Option<Vec<String>>,
    /// Where the source check asks Crossref: its works API, unless a test
    /// points it elsewhere.
    crossref: String,
    /// Results whose input files changed since: item -> those files.
    pub stale: BTreeMap<u64, Vec<String>>,
    /// The last violations and disagreements recorded in the journal.
    observed: Option<(Vec<String>, Vec<String>)>,
    /// Suggestions recorded as offered (shown in the pane), by identity.
    pub offered: BTreeSet<String>,
    /// Keys bound to command lines, from the configuration.
    pub bindings: Vec<(Key, String)>,
}

const CONTENT: [&str; 6] = ["edit", "link", "name", "suppose", "adopt", "import"];

#[derive(Debug, Clone, Copy)]
enum Slot {
    Addr,
    Word,
    Text,
}

pub struct Command {
    pub name: &'static str,
    slots: &'static [(&'static str, Slot)],
    pub help: &'static str,
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "accept",
        slots: &[("suggestion", Slot::Word)],
        help: "adopt the nth suggestion shown, with its evidence",
    },
    Command {
        name: "adopt",
        slots: &[],
        help: "make the target's claims beliefs, keeping their source",
    },
    Command {
        name: "claim",
        slots: &[("that", Slot::Text)],
        help: "the target states a claim: a Prolog fact, then from and its source",
    },
    Command {
        name: "check",
        slots: &[("which", Slot::Word)],
        help: "check quote (the text is in its source) or check source (the DOI, through Crossref)",
    },
    Command {
        name: "close",
        slots: &[],
        help: "close the supposition and return to the actual world; it stays in the journal",
    },
    Command {
        name: "commit",
        slots: &[],
        help: "make the supposed world the actual one",
    },
    Command {
        name: "constraint",
        slots: &[("named", Slot::Text)],
        help: "a constraint: its name, a colon, then a Prolog denial false :- ...",
    },
    Command {
        name: "copy",
        slots: &[("after", Slot::Addr)],
        help: "copy the target's text to a new item after an item",
    },
    Command {
        name: "delete",
        slots: &[],
        help: "delete the target: a word, or an item and its branch",
    },
    Command {
        name: "fold",
        slots: &[],
        help: "hide the target's branch",
    },
    Command {
        name: "dismiss",
        slots: &[("suggestion", Slot::Word)],
        help: "dismiss the nth suggestion shown; it returns only if its evidence changes",
    },
    Command {
        name: "derive",
        slots: &[("text", Slot::Text)],
        help: "the target shows a computed text: a template with {Var}, then where and a Prolog goal",
    },
    Command {
        name: "export",
        slots: &[("to", Slot::Text)],
        help: "export the outline, or one section, as a document (md, docx, odt, tex, html)",
    },
    Command {
        name: "fact",
        slots: &[("that", Slot::Text)],
        help: "the target states a belief: a Prolog fact",
    },
    Command {
        name: "follow",
        slots: &[("link", Slot::Word)],
        help: "go to the item at the end of the target's nth link",
    },
    Command {
        name: "help",
        slots: &[],
        help: "list every command",
    },
    Command {
        name: "import",
        slots: &[("from", Slot::Text)],
        help: "import wikidata, document or results, then a file: what it says arrives as claims",
    },
    Command {
        name: "indent",
        slots: &[],
        help: "make the target a child of the item above it",
    },
    Command {
        name: "interrogate",
        slots: &[],
        help: "ask of the target's claim: who else claims it, who disputes it, its source and record",
    },
    Command {
        name: "layer",
        slots: &[("mark", Slot::Text)],
        help: "add a layer to the target's view: a mark, then where and a goal; one mark per solution",
    },
    Command {
        name: "link",
        slots: &[("from", Slot::Addr), ("to", Slot::Addr), ("as", Slot::Word)],
        help: "link one item to another; the type defaults to link",
    },
    Command {
        name: "move",
        slots: &[("after", Slot::Addr)],
        help: "move the target's branch to follow an item",
    },
    Command {
        name: "name",
        slots: &[("as", Slot::Word)],
        help: "give the target a name that addresses it",
    },
    Command {
        name: "new",
        slots: &[],
        help: "a new item after the target",
    },
    Command {
        name: "outdent",
        slots: &[],
        help: "make the target a sibling of its parent",
    },
    Command {
        name: "pane",
        slots: &[("shows", Slot::Word)],
        help: "set the other pane to outline or links",
    },
    Command {
        name: "quit",
        slots: &[],
        help: "write what is pending and leave",
    },
    Command {
        name: "redo",
        slots: &[],
        help: "redo what undo undid, along the current branch",
    },
    Command {
        name: "rule",
        slots: &[("that", Slot::Text)],
        help: "define a predicate from others: Head :- Body. Usable in derive, constraint, layer and other rules",
    },
    Command {
        name: "set",
        slots: &[("that", Slot::Text)],
        help: "state a fact and retract the beliefs it directly contradicts under a constraint; the fact goes where the replaced one was",
    },
    Command {
        name: "retract",
        slots: &[("entry", Slot::Word)],
        help: "reverse one earlier entry, keeping later work; lists what depends on it first",
    },
    Command {
        name: "suppose",
        slots: &[("that", Slot::Text)],
        help: "start a named world with nothing supposed; hold Suppose to suppose a statement",
    },
    Command {
        name: "switch",
        slots: &[],
        help: "move to the other pane",
    },
    Command {
        name: "transclude",
        slots: &[("from", Slot::Addr)],
        help: "show an item's text in place, in a new item after the target",
    },
    Command {
        name: "unadopt",
        slots: &[],
        help: "retract the target's last adoption",
    },
    Command {
        name: "undo",
        slots: &[],
        help: "undo the last change on this branch",
    },
    Command {
        name: "unfold",
        slots: &[],
        help: "show the target's branch",
    },
];

fn lower(s: &str) -> String {
    s.to_lowercase()
}

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
            journal,
            outline: Outline::default(),
            view: View::default(),
            mode: Mode::Normal,
            tip: 0,
            redo: vec![],
            world: "actual".into(),
            message: String::new(),
            workspace,
            pending_text: None,
            bindings: crate::config::Config::default().bindings,
            view_dirty: false,
            last_leap: None,
            quit: false,
            supposition: None,
            engines: [None, None, None],
            eval: [None, None],
            preview: None,
            suggestions: vec![],
            dismissed: BTreeSet::new(),
            frozen: None,
            crossref: "https://api.crossref.org/works/".into(),
            stale: BTreeMap::new(),
            observed: None,
            offered: BTreeSet::new(),
        };
        if app.journal.entries().is_empty() {
            let e = app.journal.append("edit", json!({"op": "init"}))?;
            app.apply(&e);
        } else {
            let entries: Vec<Entry> = app.journal.entries().to_vec();
            for e in &entries {
                app.apply(e);
            }
        }
        Ok(app)
    }

    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    /// Point the source check at another Crossref-like service (for tests).
    pub fn set_crossref(&mut self, base: &str) {
        self.crossref = base.to_string();
    }

    pub fn set_durable(&mut self, d: bool) {
        self.journal.durable = d;
    }

    fn entry(&self, seq: u64) -> Option<&Entry> {
        self.journal.entries().get((seq as usize).wrapping_sub(1))
    }

    // ------------------------------------------------------------------
    // Applying entries: the one way state changes.

    pub fn apply(&mut self, e: &Entry) {
        // The order before the change is needed only if an item may vanish.
        let may_remove = match e.kind.as_str() {
            "view" => false,
            k if CONTENT.contains(&k) && e.parent == Some(self.tip) => {
                e.body.as_ref().is_some_and(|b| b["op"] == "delete")
            }
            _ => true,
        };
        let before = if may_remove {
            self.outline.in_order()
        } else {
            vec![]
        };
        match e.kind.as_str() {
            "view" => {
                let jump = e.parent != Some(self.tip);
                if let Some(b) = &e.body {
                    self.view.restore(b);
                    if b["op"] == "leap" {
                        self.last_leap = b["pattern"]
                            .as_str()
                            .map(|p| (p.to_string(), b["forward"] != false));
                    }
                    if b["op"] == "world" {
                        self.supposition = None;
                    }
                    if b["op"] == "dismiss"
                        && let Some(k) = b["key"].as_str()
                    {
                        self.dismissed.insert(k.to_string());
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
                        self.observed = Some((keys("violations"), keys("disagreements")));
                    }
                    if b["op"] == "offer" {
                        self.offered.extend(
                            b["keys"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|x| x.as_str().map(String::from)),
                        );
                    }
                }
                self.tip = e.seq;
                if jump {
                    self.rebuild();
                }
            }
            "undo" => {
                if let Some(t) = e.body.as_ref().and_then(|b| b["undoes"].as_u64()) {
                    self.redo.push(t);
                }
                self.tip = e.seq;
                self.rebuild();
            }
            "redo" => {
                self.redo.pop();
                self.tip = e.seq;
                self.rebuild();
            }
            _ => {
                let incremental = e.parent == Some(self.tip) && CONTENT.contains(&e.kind.as_str());
                self.tip = e.seq;
                if incremental {
                    if let Some(b) = &e.body {
                        self.outline.apply(e.seq, &e.kind, b);
                    }
                } else {
                    self.rebuild();
                }
                if e.kind != "purge" {
                    self.redo.clear();
                }
                if e.kind == "suppose"
                    && let Some(b) = &e.body
                {
                    let actual = e.parent.unwrap_or(1);
                    let name = b["name"].as_str().unwrap_or("supposed").to_string();
                    self.supposition = Some(Supposition { name, actual });
                }
            }
        }
        self.world = self
            .supposition
            .as_ref()
            .map_or("actual".into(), |s| s.name.clone());
        self.repair_cursors(&before);
    }

    /// The entries on the path from the tip back to the root, oldest first.
    fn path(&self, tip: u64) -> Vec<&Entry> {
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
        let (o, _, _) = self.replay(self.tip, None);
        self.outline = o;
    }

    /// The outline of another world, by its head.
    pub fn outline_at(&self, head: u64) -> Outline {
        self.replay(head, None).0
    }

    /// After the outline changes, a cursor on a vanished item moves to the
    /// nearest surviving item before it, else the first.
    fn repair_cursors(&mut self, before: &[u64]) {
        for i in 0..2 {
            let c = self.view.panes[i].cursor;
            if c != TOP && self.outline.items.contains_key(&c) {
                continue;
            }
            if before.is_empty() && c != TOP {
                continue; // nothing could have vanished
            }
            let at = before.iter().position(|x| *x == c).unwrap_or(0);
            let fallback = before[..at]
                .iter()
                .rev()
                .find(|x| self.outline.items.contains_key(x))
                .copied();
            self.view.panes[i].cursor = fallback
                .or_else(|| self.outline.in_order().first().copied())
                .unwrap_or(TOP);
        }
        self.view
            .folded
            .retain(|f| self.outline.items.contains_key(f));
        let n = self.outline.text(self.cursor()).chars().count();
        self.view.pos = self.view.pos.min(n);
    }

    // ------------------------------------------------------------------
    // Writing entries.

    fn write(&mut self, parent: Option<u64>, kind: &str, body: Value) -> Result<Entry, String> {
        let e = self
            .journal
            .append_after(parent, kind, body)
            .map_err(|e| e.to_string())?;
        self.apply(&e);
        Ok(e)
    }

    fn write_here(&mut self, kind: &str, body: Value) -> Result<Entry, String> {
        self.write(Some(self.tip), kind, body)
    }

    /// Write whatever is pending: typed text, then cursor moves.
    pub fn flush(&mut self) {
        self.flush_text();
        if self.view_dirty {
            self.view_dirty = false;
            let body = self.view.to_json("move", json!({}));
            if let Err(m) = self.write_here("view", body) {
                self.message = m;
            }
        }
    }

    fn write_view(&mut self, op: &str, extra: Value) {
        self.flush_text();
        self.view_dirty = false;
        let body = self.view.to_json(op, extra);
        if let Err(m) = self.write_here("view", body) {
            self.message = m;
        }
    }

    fn flush_text(&mut self) {
        if let Some(p) = self.pending_text.take()
            && let Err(m) = self.write_text(p.item, &p.text)
        {
            self.message = m;
        }
    }

    /// Write an item's new text: unchanged characters at the start and end
    /// keep their spans; the middle is added by this entry.
    fn write_text(&mut self, item: u64, new: &[char]) -> Result<(), String> {
        let old: Vec<char> = self.outline.text(item).chars().collect();
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
        let mut spans = self.outline.slice(item, 0, pre);
        if !added.is_empty() {
            spans.push(Span {
                seq: 0,
                start: 0,
                len: added.chars().count(),
            });
        }
        spans.extend(self.outline.slice(item, old.len() - suf, old.len()));
        self.write_here(
            "edit",
            json!({"op": "text", "item": item, "add": added, "spans": span_json(&spans)}),
        )?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // What is shown.

    pub fn cursor(&self) -> u64 {
        self.view.panes[self.view.focus].cursor
    }

    fn set_cursor(&mut self, id: u64) {
        let f = self.view.focus;
        self.view.panes[f].cursor = id;
        self.view.pos = self.text_of(id).len();
    }

    /// The text of an item as shown: pending typing included.
    pub fn text_of(&self, id: u64) -> Vec<char> {
        match &self.pending_text {
            Some(p) if p.item == id => p.text.clone(),
            _ => self.outline.text(id).chars().collect(),
        }
    }

    pub fn pending(&self) -> bool {
        self.pending_text.is_some() || self.view_dirty
    }

    /// The items shown in an outline pane, in order, with their depth.
    pub fn visible(&self) -> Vec<(u64, usize)> {
        let mut out = Vec::new();
        let mut stack: Vec<(u64, usize)> = self.outline.top.iter().rev().map(|i| (*i, 0)).collect();
        while let Some((id, d)) = stack.pop() {
            out.push((id, d));
            if !self.view.folded.contains(&id) {
                stack.extend(self.outline.children(id).iter().rev().map(|c| (*c, d + 1)));
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
        let p = lower(pattern);
        let order = self.outline.in_order();
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
            .filter(|id| lower(&self.outline.text(*id)).contains(&p))
            .collect()
    }

    /// Recognition: the longest continuation every match shares, up to a space.
    fn leap_complete(&self, pattern: &str, forward: bool) -> String {
        let p = lower(pattern);
        let conts: Vec<String> = self
            .leap_matches(pattern, forward)
            .iter()
            .map(|id| {
                let t = lower(&self.outline.text(*id));
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

    fn unfold_to(&mut self, id: u64) {
        let mut at = self.outline.parent(id);
        while let Some(p) = at {
            self.view.folded.remove(&p);
            at = self.outline.parent(p);
        }
    }

    // ------------------------------------------------------------------
    // Keys.

    pub fn handle(&mut self, ev: Event) {
        let mode = std::mem::replace(&mut self.mode, Mode::Normal);
        self.mode = match mode {
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
        if self.mode != Mode::Normal {
            return;
        }
        match c {
            Click::Item { pane, id } => {
                if !self.outline.items.contains_key(id) || *pane > 1 {
                    return;
                }
                self.flush_text();
                self.message.clear();
                if self.view.focus != *pane {
                    self.view.focus = *pane;
                    self.write_view("pane", json!({}));
                }
                self.set_cursor(*id);
                self.view_dirty = true;
            }
            Click::Command(line) => {
                self.flush();
                self.message.clear();
                self.run_simple(line);
            }
        }
    }

    /// How to run a command by key: its bound key, else by name.
    pub fn key_for(&self, command: &str) -> String {
        match self.bindings.iter().find(|(_, c)| c == command) {
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
                self.view.scope = ScopeLevel::Word;
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
            return std::mem::replace(&mut self.mode, Mode::Normal);
        }
        let f = self.view.focus;
        if self.view.panes[f].kind == PaneKind::Links {
            self.message = format!("this pane shows links; {} to edit", self.key_for("switch"));
            return Mode::Normal;
        }
        self.message.clear();
        match ev.key {
            Key::Char(c) => self.type_char(c),
            Key::Backspace => self.erase(true),
            Key::Delete => self.erase(false),
            Key::Enter => {
                self.flush();
                if let Err(m) = self.new_item_after(self.cursor()) {
                    self.message = m;
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
                    self.view_dirty = true;
                }
            }
            Key::Left | Key::Right | Key::Home | Key::End => {
                let n = self.text_of(self.cursor()).len();
                self.view.pos = match ev.key {
                    Key::Left => self.view.pos.saturating_sub(1),
                    Key::Right => (self.view.pos + 1).min(n),
                    Key::Home => 0,
                    _ => n,
                };
                self.view_dirty = true;
            }
            _ => {}
        }
        Mode::Normal
    }

    fn type_char(&mut self, c: char) {
        if self.cursor() == TOP
            && let Err(m) = self.new_item_after(TOP)
        {
            self.message = m;
            return;
        }
        let item = self.cursor();
        let mut text = self.text_of(item);
        let pos = self.view.pos.min(text.len());
        text.insert(pos, c);
        self.pending_text = Some(PendingText { item, text });
        self.view.pos = pos + 1;
        self.view_dirty = true;
    }

    fn erase(&mut self, before: bool) {
        let item = self.cursor();
        if item == TOP {
            return;
        }
        let mut text = self.text_of(item);
        let pos = self.view.pos.min(text.len());
        if before && pos > 0 {
            text.remove(pos - 1);
            self.view.pos = pos - 1;
        } else if !before && pos < text.len() {
            text.remove(pos);
        } else {
            return;
        }
        self.pending_text = Some(PendingText { item, text });
        self.view_dirty = true;
    }

    fn new_item_after(&mut self, id: u64) -> Result<(), String> {
        let (under, after) = if id == TOP {
            (TOP, None)
        } else {
            (self.outline.parent(id).unwrap_or(TOP), Some(id))
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
                self.view.scope = match self.view.scope {
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
                    self.message.clear();
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
                    match &self.last_leap {
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
                        self.message.clear();
                    }
                    None => self.message = format!("no item holds “{pattern}”"),
                }
                self.last_leap = Some((pattern.clone(), forward));
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
                let facts =
                    crate::store::facts(self.journal.entries(), self.tip).unwrap_or_default();
                let readings = reason::readings(&facts, &text);
                if readings.is_empty() {
                    // No reading: a named world, with nothing supposed in it.
                    if let Err(m) = self.write_here("suppose", json!({"name": text})) {
                        self.message = m;
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
                    Ok(e) => self.message = format!("entry {target} retracted by entry {}", e.seq),
                    Err(m) => self.message = m,
                }
                Mode::Normal
            }
            Key::Escape => {
                self.message = format!("entry {target} kept");
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
                    self.message = m;
                }
                Mode::Normal
            }
            Key::Tab => {
                let at = (at + 1) % readings.len();
                Mode::ConfirmSuppose { text, readings, at }
            }
            Key::Escape => {
                self.message = "nothing supposed".into();
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
            let eval = self.eval[0].as_ref().ok_or("no evaluation")?;
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
        self.message = m;
        Ok(())
    }

    /// `set`: find, before writing anything, the beliefs the fact directly
    /// contradicts (see `direct_conflicts`), by evaluating the world with
    /// it added in the preview engine; retract them; then state the fact on
    /// the item that held the first of them, or on `target` if none.
    fn set_fact(&mut self, fact: &Term, target: u64) -> Result<(), String> {
        let mut facts = crate::store::facts(self.journal.entries(), self.tip)?;
        facts.extend(outline_facts(&self.outline));
        let next = self.journal.entries().len() as u64 + 1;
        facts.push((next, fact.clone()));
        let constraints: Vec<(u64, String, String)> = self
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
                name,
                rule,
                violations,
                disagreements,
            });
        }
        let (mut replaced, left) = direct_conflicts(&evals, next);
        replaced.retain(|t| self.retractable(*t).is_ok());
        let item = replaced
            .iter()
            .find_map(|t| self.outline.fact_item.get(t).copied())
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
            format!("set at {}; nothing replaced", self.outline.number(item))
        } else {
            format!("set; replaced the fact at {}", places.join(", "))
        };
        if !left.is_empty() {
            m.push_str(&format!("; left for you: {}", left.join(", ")));
        }
        self.message = m;
        Ok(())
    }

    /// Where a fact entry's fact is stated, in the actual world: its item's
    /// number, or "entry n".
    pub fn fact_place(&self, seq: u64) -> String {
        let head = self.supposition.as_ref().map_or(self.tip, |s| s.actual);
        let o = self.outline_at(head);
        match o.fact_item.get(&seq) {
            Some(i) => o.number(*i),
            None => format!("entry {seq}"),
        }
    }

    fn engine(&mut self, k: usize) -> Result<&mut Engine, String> {
        if self.engines[k].is_none() {
            self.engines[k] =
                Some(Engine::start().map_err(|e| format!("cannot start Prolog: {e}"))?);
        }
        Ok(self.engines[k].as_mut().unwrap())
    }

    /// The engine of the current world, for tests that count evaluations.
    pub fn current_engine(&mut self) -> Result<&mut Engine, String> {
        self.engine(0)
    }

    /// Reason over the worlds shown: sync each world's facts into its engine,
    /// and read its constraints and derived values. Nothing here changes the
    /// journal; it is computed from it.
    pub fn evaluate(&mut self) -> Result<(), String> {
        let heads: Vec<u64> = match &self.supposition {
            Some(s) => vec![self.tip, s.actual],
            None => vec![self.tip],
        };
        self.eval = [None, None];
        for (k, head) in heads.into_iter().enumerate() {
            let facts = crate::store::facts(self.journal.entries(), head)?;
            let outline = if k == 0 {
                self.outline.clone()
            } else {
                self.outline_at(head)
            };
            let mut eval = WorldEval {
                facts: facts.clone(),
                at: self.journal.entries().len(),
                ..Default::default()
            };
            if facts.is_empty() {
                self.eval[k] = Some(eval);
                continue;
            }
            let import = self.import_on(head);
            let mut with_outline = facts.clone();
            with_outline.extend(outline_facts(&outline));
            let engine = self.engine(k)?;
            engine.sync(&with_outline).map_err(|e| e.to_string())?;
            engine
                .wikidata(import.as_ref().map(|(s, f)| (*s, f.as_path())))
                .map_err(|e| e.to_string())?;
            for (seq, c) in &outline.constraints {
                if !facts.iter().any(|(s, _)| s == seq) {
                    continue; // retracted in this world
                }
                let v = engine.node(*seq).map_err(|e| e.to_string())?;
                let (violations, disagreements) = solutions(&v)
                    .into_iter()
                    .partition(|s| s.prov.iter().all(|p| matches!(p, Prov::Belief(_))));
                eval.constraints.push(ConstraintEval {
                    seq: *seq,
                    name: c.name.clone(),
                    rule: c.rule.clone(),
                    violations,
                    disagreements,
                });
            }
            for item in outline.derived.keys() {
                if let Ok(v) = engine.node(*item) {
                    eval.derived.insert(*item, v);
                }
            }
            for layer in outline.views.values().flatten() {
                if let Ok(v) = engine.node(layer.seq) {
                    eval.layers.insert(layer.seq, reason::marks(&v));
                }
            }
            self.eval[k] = Some(eval);
        }
        self.suggestions = match &self.engines[0] {
            Some(_) if self.eval[0].as_ref().is_some_and(|e| !e.facts.is_empty()) => {
                let all = self.engine(0)?.suggestions().map_err(|e| e.to_string())?;
                all.into_iter()
                    .filter(|s| !self.dismissed.contains(&s.key()))
                    .collect()
            }
            _ => vec![],
        };
        self.update_frozen();
        self.stale = self.stale_results();
        self.record_observations();
        self.preview = None;
        if let Some(item) = self.view.interrogate {
            self.preview = Some(self.preview_adoption(item)?);
        }
        Ok(())
    }

    /// Record in the journal what reasoning showed, when it changes: the
    /// current world's violations and disagreements, and the suggestions
    /// shown for the first time. A query and its answer are journal entries
    /// (§ The journal, rule 6), so the measures report can count them.
    fn record_observations(&mut self) {
        let Some(e) = &self.eval[0] else { return };
        let key = |c: &ConstraintEval, s: &reason::Solution| {
            let mut f: Vec<String> = s.facts.iter().map(reason::show).collect();
            f.sort();
            format!("{}: {}", c.name, f.join(" · "))
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
        let changed = match &self.observed {
            Some(o) => *o != now,
            None => !now.0.is_empty() || !now.1.is_empty(),
        };
        if changed {
            let world = self.world.clone();
            self.write_view(
                "observed",
                json!({"violations": now.0, "disagreements": now.1, "world": world}),
            );
            self.observed = Some(now);
        }
        if self
            .view
            .panes
            .iter()
            .any(|p| p.kind == PaneKind::Suggestions)
        {
            let new: Vec<String> = self
                .shown_suggestions()
                .iter()
                .map(|s| s.identity())
                .filter(|k| !self.offered.contains(k))
                .collect();
            if !new.is_empty() {
                self.write_view("offer", json!({"keys": new}));
                self.offered.extend(new);
            }
        }
    }

    /// What adopting an item's claims would raise, computed in a spare
    /// engine: a preview of adopting it, as a world.
    fn preview_adoption(&mut self, item: u64) -> Result<String, String> {
        let mut facts = crate::store::facts(self.journal.entries(), self.tip)?;
        let next = self.journal.entries().len() as u64 + 1;
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
    fn import_on(&self, head: u64) -> Option<(u64, std::path::PathBuf)> {
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
                        self.journal.dir().join(format::record::BLOBS).join(blob),
                    )
                })
            })
    }

    /// The suggestions in the order shown: while the suggestion pane has
    /// focus, those already shown keep their places and new ones go last.
    pub fn shown_suggestions(&self) -> Vec<&Suggestion> {
        let mut out: Vec<&Suggestion> = Vec::new();
        if let Some(order) = &self.frozen {
            for k in order {
                if let Some(s) = self.suggestions.iter().find(|s| &s.identity() == k) {
                    out.push(s);
                }
            }
        }
        for s in &self.suggestions {
            if !out.iter().any(|o| o.identity() == s.identity()) {
                out.push(s);
            }
        }
        out.truncate(5); // few in view
        out
    }

    /// Keep the suggestion pane's order while it has focus.
    fn update_frozen(&mut self) {
        let focused = self.view.panes[self.view.focus].kind == PaneKind::Suggestions;
        self.frozen = match (focused, self.frozen.take()) {
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
        let facts = crate::store::facts(self.journal.entries(), self.tip).unwrap_or_default();
        let mut out: Vec<u64> = facts
            .iter()
            .filter(|(_, f)| matches!(f, Term::Compound(_, a) if a.first() == Some(entity)))
            .filter_map(|(s, _)| self.outline.fact_item.get(s).copied())
            .collect();
        out.dedup();
        out
    }

    /// Adopt a suggestion: one entry, marked as an adopted suggestion and
    /// carrying its evidence, so it can be found and retracted later. A
    /// constraint is tried against a planted violation first.
    fn accept(&mut self, s: &Suggestion) -> Result<(), String> {
        let mark = json!({"kind": reason::show(&s.kind), "evidence": s.evidence, "score": s.score});
        let enc = |t: &Term| terms::encode(t).map_err(|e| e.0);
        let seq = self.journal.entries().len() as u64 + 1;
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
    fn store_blob(&self, bytes: &[u8]) -> Result<String, String> {
        let hash = format::record::sha256_hex(bytes);
        let blobs = self.journal.dir().join(format::record::BLOBS);
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
        let mut at = self.outline.parent(id);
        while let Some(p) = at.filter(|p| *p != TOP) {
            d += 1;
            at = self.outline.parent(p);
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
    fn import_document(&mut self, file: &str) -> Result<(), String> {
        let path = std::path::Path::new(file);
        let (blocks, losses, _) = crate::docs::read(path)?;
        let bytes = std::fs::read(path).map_err(|e| format!("{file}: {e}"))?;
        let hash = self.store_blob(&bytes)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let earlier: Vec<u64> = self
            .path(self.tip)
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
        let mut last_at: Vec<Option<u64>> = vec![self.outline.top.last().copied()];
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
        self.message = format!(
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
            .outline
            .in_order()
            .into_iter()
            .filter(|i| {
                self.outline.items[i]
                    .origin
                    .is_some_and(|o| earlier.contains(&o))
            })
            .collect();
        let base = old.iter().map(|i| self.depth_of(*i)).min().unwrap_or(0);
        let key = |style: &str, text: &str| (style.to_string(), text.to_string());
        let old_keys: Vec<(String, String)> = old
            .iter()
            .map(|i| key(&self.outline.items[i].style, &self.outline.text(*i)))
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
                self.outline
                    .parent(old.first().copied().unwrap_or(TOP))
                    .unwrap_or(TOP)
            } else {
                parent
            };
            id_of[n] = Some(self.write_block(&blocks[n], parent, after, imp.seq)?);
        }
        let kept: HashSet<u64> = id_of.iter().flatten().copied().collect();
        for o in deletes {
            if !self.outline.items.contains_key(&o) {
                continue;
            }
            let mut stack = self.outline.children(o).to_vec();
            let mut keeps_something = false;
            while let Some(x) = stack.pop() {
                keeps_something |= kept.contains(&x);
                stack.extend(self.outline.children(x));
            }
            if !keeps_something {
                self.write_here("edit", json!({"op": "delete", "item": o}))?;
            }
        }
        self.message = format!(
            "revision of {name}: the changes are in a supposed world; retract any with retract N, then commit or close"
        );
        Ok(())
    }

    /// The changes of a document revision, from the supposition on: (entry
    /// seq, what it does, retracted).
    pub fn revision_changes(&self) -> Vec<(u64, String, bool)> {
        let Some(s) = &self.supposition else {
            return vec![];
        };
        let path = self.path(self.tip);
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
    fn export(&mut self, file: &str, section: Option<u64>) -> Result<(), String> {
        let roots: Vec<u64> = match section {
            Some(s) => vec![s],
            None => self.outline.top.clone(),
        };
        let mut blocks = Vec::new();
        let mut seqs = std::collections::BTreeSet::new();
        for r in roots {
            let base = self.depth_of(r);
            let mut stack = vec![r];
            while let Some(id) = stack.pop() {
                let item = &self.outline.items[&id];
                let style = if item.style.is_empty() {
                    if item.children.is_empty() {
                        "para"
                    } else {
                        "heading"
                    }
                } else {
                    item.style.as_str()
                };
                let text = self.outline.text(id);
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
        self.message = format!(
            "exported {} items to {name}, citing {} source entries",
            blocks.len(),
            hashes.len()
        );
        Ok(())
    }

    /// Import results: an exchange file of values with their provenance (W3C
    /// PROV terms), as claims whose source is the analysis.
    fn import_results(&mut self, file: &str) -> Result<(), String> {
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
        let top = self.write_block(&head, TOP, self.outline.top.last().copied(), imp.seq)?;
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
        self.message = format!("imported the results of {source}");
        Ok(())
    }

    /// Items whose results rest on input files that changed since: each with
    /// the files that no longer match their recorded hashes.
    pub fn stale_results(&self) -> BTreeMap<u64, Vec<String>> {
        let mut out = BTreeMap::new();
        for e in self.path(self.tip) {
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
                for (id, item) in &self.outline.items {
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
    fn check_quote(&mut self, item: u64) -> Result<String, String> {
        let origin = self
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
            .journal
            .dir()
            .join(format::record::BLOBS)
            .join(b["blob"].as_str().unwrap_or(""));
        let source = crate::docs::plain_text(&blob, fmt)?;
        let span = crate::docs::markdown_to_plain(&self.outline.text(item))?;
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
    fn check_source(&mut self, item: u64) -> Result<String, String> {
        let facts = crate::store::facts(self.journal.entries(), self.tip)?;
        let mine: Vec<&Term> = facts
            .iter()
            .filter(|(s, _)| self.outline.fact_item.get(s) == Some(&item))
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
        let url = format!("{}{}", self.crossref, encode_doi(&doi));
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
        let facts = crate::store::facts(self.journal.entries(), self.tip).unwrap_or_default();
        facts
            .into_iter()
            .filter(|(s, _)| self.outline.fact_item.get(s) == Some(&item))
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

    fn run_simple(&mut self, name: &str) {
        self.mode = Mode::Normal;
        let m = self.run_command(name);
        self.mode = m;
    }

    /// Run a command line, as typed in the Command quasimode.
    pub fn run_command(&mut self, line: &str) -> Mode {
        self.flush();
        let mode = match self.command(line.trim()) {
            Ok(mode) => mode,
            Err(m) => {
                self.message = m;
                Mode::Normal
            }
        };
        self.flush();
        self.update_frozen();
        mode
    }

    /// An item named by its outline number ("3.2"), its name, "#seq", or the
    /// start of its text if only one item's text starts so.
    pub fn resolve(&self, addr: &str) -> Option<u64> {
        let a = addr.trim();
        if a.is_empty() {
            return None;
        }
        if let Some(id) = a
            .strip_prefix('#')
            .and_then(|s| s.parse().ok())
            .filter(|i| self.outline.items.contains_key(i))
        {
            return Some(id);
        }
        if let Some(id) = self.outline.by_number(a) {
            return Some(id);
        }
        if let Some(id) = self
            .outline
            .names
            .get(a)
            .filter(|i| self.outline.items.contains_key(i))
        {
            return Some(*id);
        }
        let la = lower(a);
        let hits: Vec<u64> = self
            .outline
            .in_order()
            .into_iter()
            .filter(|id| lower(&self.outline.text(*id)).starts_with(&la))
            .collect();
        (hits.len() == 1).then(|| hits[0])
    }

    /// Split words into addresses: the first split where every part resolves.
    fn addresses(&self, words: &[&str], n: usize) -> Option<Vec<u64>> {
        let words: Vec<&str> = words
            .iter()
            .copied()
            .filter(|w| !["to", "from", "after", "as"].contains(w))
            .collect();
        if n == 1 {
            return self.resolve(&words.join(" ")).map(|a| vec![a]);
        }
        (1..words.len()).find_map(|k| {
            let a = self.resolve(&words[..k].join(" "))?;
            let b = self.resolve(&words[k..].join(" "))?;
            Some(vec![a, b])
        })
    }

    fn command(&mut self, line: &str) -> Result<Mode, String> {
        let mut words: Vec<&str> = line.split_whitespace().collect();
        if words.is_empty() {
            return Ok(Mode::Normal);
        }
        let name = find_command(words[0])
            .ok_or_else(|| format!("no command “{}”; ? lists them", words[0]))?;
        words.remove(0);
        let target = self.cursor();
        let need_target = !matches!(
            name,
            "help"
                | "quit"
                | "redo"
                | "undo"
                | "switch"
                | "pane"
                | "retract"
                | "set"
                | "rule"
                | "suppose"
                | "new"
                | "constraint"
                | "close"
                | "commit"
                | "import"
                | "accept"
                | "dismiss"
                | "export"
        );
        if need_target && target == TOP {
            return Err("there is no item to act on".into());
        }
        match name {
            "help" => {
                self.message = COMMANDS
                    .iter()
                    .map(|c| c.name)
                    .collect::<Vec<_>>()
                    .join(" · ");
            }
            "quit" => self.quit = true,
            "new" => self.new_item_after(target)?,
            "switch" => {
                self.view.focus = 1 - self.view.focus;
                if self.view.panes[self.view.focus].cursor == TOP {
                    self.view.panes[self.view.focus].cursor =
                        self.view.panes[1 - self.view.focus].cursor;
                }
                self.write_view("pane", json!({}));
            }
            "pane" => {
                let other = 1 - self.view.focus;
                self.view.panes[other].kind = match words.first().copied() {
                    Some("outline") => PaneKind::Outline,
                    Some("links") => PaneKind::Links,
                    Some("suggestions") => PaneKind::Suggestions,
                    _ => return Err("pane (shows) outline, links, or suggestions".into()),
                };
                self.view.panes[other].cursor = target;
                self.write_view("pane", json!({}));
            }
            "fold" | "unfold" => {
                if name == "fold" {
                    self.view.folded.insert(target);
                } else {
                    self.view.folded.remove(&target);
                }
                self.write_view("fold", json!({}));
            }
            "undo" => {
                let Some(t) = self.content_tip(self.tip).filter(|t| *t > 1) else {
                    return Err("nothing to undo on this branch".into());
                };
                let parent = self.entry(t).and_then(|e| e.parent);
                self.write(parent, "undo", json!({"undoes": t}))?;
                self.message = format!("undid entry {t}");
            }
            "redo" => {
                let Some(&t) = self.redo.last() else {
                    return Err("nothing to redo".into());
                };
                self.write(Some(t), "redo", json!({"redoes": t}))?;
                self.message = format!("redid entry {t}");
            }
            "retract" => {
                let t: u64 = words
                    .first()
                    .and_then(|w| w.trim_start_matches('#').parse().ok())
                    .ok_or("retract (entry) seq")?;
                let dependents = self.retractable(t)?;
                self.message.clear();
                return Ok(Mode::ConfirmRetract {
                    target: t,
                    dependents,
                });
            }
            "delete" => {
                if self.view.scope == ScopeLevel::Word {
                    let text = self.text_of(target);
                    let (a, b) = word_at(&text, self.view.pos);
                    let mut t = text.clone();
                    t.drain(a..b);
                    self.view.pos = a;
                    self.view_dirty = true;
                    self.write_text(target, &t)?;
                } else {
                    self.write_here("edit", json!({"op": "delete", "item": target}))?;
                }
            }
            "indent" => {
                let p = self.outline.parent(target).unwrap_or(TOP);
                let sibs = self.outline.children(p).to_vec();
                let i = sibs.iter().position(|x| *x == target).unwrap_or(0);
                if i == 0 {
                    return Err("the first item of a list cannot be indented".into());
                }
                let under = sibs[i - 1];
                let after = self.outline.children(under).last().copied();
                self.write_here(
                    "edit",
                    json!({"op": "move", "item": target, "under": under, "after": after}),
                )?;
                if self.view.folded.remove(&under) {
                    self.view_dirty = true;
                }
            }
            "outdent" => {
                let p = self.outline.parent(target).unwrap_or(TOP);
                if p == TOP {
                    return Err("a top-level item cannot be outdented".into());
                }
                let gp = self.outline.parent(p).unwrap_or(TOP);
                self.write_here(
                    "edit",
                    json!({"op": "move", "item": target, "under": gp, "after": p}),
                )?;
            }
            "move" => {
                let to = self
                    .addresses(&words, 1)
                    .ok_or("move (after) an item: a number, a name, or the start of its text")?[0];
                if to == target
                    || self.outline.in_order().contains(&to) && self.is_in_branch(to, target)
                {
                    return Err("an item cannot move into its own branch".into());
                }
                let under = self.outline.parent(to).unwrap_or(TOP);
                self.write_here(
                    "edit",
                    json!({"op": "move", "item": target, "under": under, "after": to}),
                )?;
            }
            "copy" => {
                let to = self.addresses(&words, 1).ok_or("copy (after) an item")?[0];
                let text = self.outline.text(target);
                let n = text.chars().count();
                let under = self.outline.parent(to).unwrap_or(TOP);
                self.write_here("edit", json!({"op": "create", "under": under, "after": to, "add": text, "spans": [[0, 0, n]]}))?;
            }
            "transclude" => {
                let from = self
                    .addresses(&words, 1)
                    .ok_or("transclude (from) an item")?[0];
                let spans = self.outline.items[&from].spans.clone();
                let under = self.outline.parent(target).unwrap_or(TOP);
                let e = self.write_here("edit", json!({"op": "create", "under": under, "after": target, "add": "", "spans": span_json(&spans)}))?;
                self.set_cursor(e.seq);
                self.write_view("move", json!({}));
            }
            "link" => {
                let (addr_words, kind) = match words.iter().position(|w| *w == "as") {
                    Some(i) => (&words[..i], words.get(i + 1).copied().unwrap_or("link")),
                    None => (&words[..], "link"),
                };
                let ends = self
                    .addresses(addr_words, 2)
                    .or_else(|| self.addresses(addr_words, 1).map(|t| vec![target, t[0]]))
                    .ok_or("link (from) an item (to) an item, (as) a type")?;
                let fact = format::terms::Term::Compound(
                    kind.into(),
                    vec![
                        format::terms::Term::Int(ends[0] as i64),
                        format::terms::Term::Int(ends[1] as i64),
                    ],
                );
                let fact = format::terms::encode(&fact).map_err(|e| e.0)?;
                self.write_here(
                    "link",
                    json!({"from": ends[0], "to": ends[1], "type": kind, "facts": [fact]}),
                )?;
                self.message = format!(
                    "linked {} to {} as {kind}",
                    self.outline.number(ends[0]),
                    self.outline.number(ends[1])
                );
            }
            "name" => {
                let n = words.join(" ");
                if n.is_empty() {
                    return Err("name (as) a word".into());
                }
                self.write_here("name", json!({"name": n, "item": target}))?;
            }
            "follow" => {
                let k: usize = words.first().and_then(|w| w.parse().ok()).unwrap_or(1);
                let (out, into) = self.outline.links_of(target);
                let ends: Vec<u64> = out
                    .iter()
                    .map(|l| l.to)
                    .chain(into.iter().map(|l| l.from))
                    .collect();
                let to = *ends
                    .get(k.wrapping_sub(1))
                    .ok_or(format!("the target has {} links", ends.len()))?;
                self.unfold_to(to);
                self.set_cursor(to);
                self.write_view("follow", json!({"to": to}));
            }
            "import" if words.first() == Some(&"document") => {
                let file = words[1..].join(" ");
                self.import_document(&file)?;
            }
            "import" if words.first() == Some(&"results") => {
                let file = words[1..].join(" ");
                self.import_results(&file)?;
            }
            "import" => {
                let (what, file) = match words.split_first() {
                    Some((w, rest)) if *w == "wikidata" && !rest.is_empty() => (*w, rest.join(" ")),
                    _ => {
                        return Err(
                            "import (from) wikidata and a file made by the wikidata tool".into(),
                        );
                    }
                };
                let bytes = std::fs::read(&file).map_err(|e| format!("{file}: {e}"))?;
                self.engine(0)?
                    .check_wikidata(std::path::Path::new(&file))
                    .map_err(|e| format!("{file} is not a Wikidata subset: {e}"))?;
                let hash = format::record::sha256_hex(&bytes);
                let blobs = self.journal.dir().join(format::record::BLOBS);
                std::fs::create_dir_all(&blobs).map_err(|e| e.to_string())?;
                let dest = blobs.join(&hash);
                if !dest.exists() {
                    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
                }
                let name = std::path::Path::new(&file)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // Provenance after W3C PROV: what it was derived from, under
                // what licence, and the file it came in.
                self.write_here(
                    "import",
                    json!({"blob": hash, "format": "wikidata-subset", "source": "Wikidata", "file": name,
                           "prov": {"prov:wasDerivedFrom": "https://www.wikidata.org/", "license": "CC0-1.0", "prov:wasGeneratedBy": "crates/tools/src/bin/wikidata.rs"}}),
                )?;
                self.message = format!(
                    "imported {what}: {} bytes, blob {}",
                    bytes.len(),
                    &hash[..12]
                );
            }
            "export" => {
                let (file, section) = match words.split_last() {
                    Some((last, rest))
                        if !rest.is_empty() && self.outline.by_number(last).is_some() =>
                    {
                        (rest.join(" "), self.outline.by_number(last))
                    }
                    _ => (words.join(" "), None),
                };
                if file.is_empty() {
                    return Err("export (to) a file, and an item number for one section".into());
                }
                self.export(&file, section)?;
            }
            "check" => {
                let ruling = match words.first().copied() {
                    Some("quote") => self.check_quote(target)?,
                    Some("source") => self.check_source(target)?,
                    _ => return Err("check quote, or check source".into()),
                };
                self.message = format!("{} check: {ruling}", words[0]);
            }
            "accept" | "dismiss" => {
                let n: usize = words
                    .first()
                    .and_then(|w| w.parse().ok())
                    .ok_or(format!("{name} (suggestion) a number"))?;
                let s = self
                    .shown_suggestions()
                    .get(n.wrapping_sub(1))
                    .map(|s| (*s).clone())
                    .ok_or(format!("there is no suggestion {n}"))?;
                if name == "dismiss" {
                    self.write_view("dismiss", json!({"key": s.key()}));
                    self.message = format!("dismissed: {}", s.describe());
                } else {
                    self.accept(&s)?;
                    self.message = format!("adopted: {}", s.describe());
                }
            }

            "fact" | "claim" => {
                let text = words.join(" ");
                let (fact_text, source) = match (name, text.rfind(" from ")) {
                    ("claim", Some(i)) => (
                        text[..i].to_string(),
                        Some(text[i + 6..].trim().to_string()),
                    ),
                    ("claim", None) => return Err("claim (that) a fact, from a source".into()),
                    _ => (text, None),
                };
                let (t, _) = self
                    .engine(0)?
                    .parse(fact_text.trim_end_matches('.'))
                    .map_err(|e| format!("not a Prolog fact: {e}"))?;
                if !matches!(t, Term::Atom(_) | Term::Compound(..)) || !variables(&t).is_empty() {
                    return Err("a fact is an atom or a compound term, with no variables".into());
                }
                if reserved(&t) {
                    return Err(format!(
                        "{} is reserved: definitions are made by constraint, derive and accept",
                        functor_of(&t)
                    ));
                }
                let fact = match &source {
                    Some(src) => Term::Compound("claims".into(), vec![Term::Str(src.clone()), t]),
                    None => t,
                };
                let f = terms::encode(&fact).map_err(|e| e.0)?;
                let mut body = json!({"op": "facts", "item": target, "facts": [f]});
                if let Some(src) = source {
                    body["source"] = json!(src);
                }
                self.write_here("edit", body)?;
            }
            "rule" => {
                let text = words.join(" ");
                let (t, names) = self
                    .engine(0)?
                    .parse(text.trim().trim_end_matches('.'))
                    .map_err(|e| format!("not a Prolog rule: {e}"))?;
                let (head, body) = match &t {
                    Term::Compound(n, a) if n == ":-" && a.len() == 2 => {
                        (a[0].clone(), a[1].clone())
                    }
                    _ => return Err("rule (that) Head :- Body.".into()),
                };
                let pi = functor_of(&head);
                if reserved(&head)
                    || matches!(&head, Term::Compound(f, _) | Term::Atom(f) if f == "rule_def" || f == "node_def" || f == "claims")
                {
                    return Err(format!("rule not kept: {pi} is reserved"));
                }
                // Every head variable must be bound by the body; name those
                // that are not. `names` are in order of first appearance.
                let in_body = variables(&body);
                let free: Vec<String> = variables(&head)
                    .into_iter()
                    .filter(|v| !in_body.contains(v))
                    .map(|v| {
                        names
                            .get(v as usize)
                            .cloned()
                            .unwrap_or_else(|| format!("_{v}"))
                    })
                    .collect();
                if !free.is_empty() {
                    return Err(format!(
                        "rule not kept: {} in the head {} not bound by the body",
                        free.join(", "),
                        if free.len() == 1 { "is" } else { "are" }
                    ));
                }
                // The check reads the world's rules: bring the engine up to
                // date first, in case it has not been evaluated since.
                let facts = crate::store::facts(self.journal.entries(), self.tip)?;
                self.engine(0)?.sync(&facts).map_err(|e| e.to_string())?;
                self.engine(0)?
                    .check_rule(&head, &body)
                    .map_err(|w| format!("rule not kept: {w}"))?;
                let seq = self.journal.entries().len() as u64 + 1;
                let def =
                    Term::Compound("rule_def".into(), vec![Term::Int(seq as i64), head, body]);
                let f = terms::encode(&def).map_err(|e| e.0)?;
                let mut b = json!({"op": "facts", "rule": text.trim(), "facts": [f]});
                if target != TOP {
                    b["item"] = json!(target);
                }
                self.write_here("edit", b)?;
                self.message = format!("rule {pi} kept");
            }
            "set" => {
                let text = words.join(" ");
                let (t, _) = self
                    .engine(0)?
                    .parse(text.trim_end_matches('.'))
                    .map_err(|e| format!("not a Prolog fact: {e}"))?;
                if !matches!(t, Term::Atom(_) | Term::Compound(..)) || !variables(&t).is_empty() {
                    return Err("a fact is an atom or a compound term, with no variables".into());
                }
                if reserved(&t) {
                    return Err(format!(
                        "{} is reserved: definitions are made by constraint, derive and accept",
                        functor_of(&t)
                    ));
                }
                self.set_fact(&t, target)?;
            }
            "constraint" => {
                let text = words.join(" ");
                let (cname, rule) = text
                    .split_once(": ")
                    .ok_or("constraint (named) a name: false :- body.")?;
                let (t, _) = self
                    .engine(0)?
                    .parse(rule.trim().trim_end_matches('.'))
                    .map_err(|e| format!("not a Prolog rule: {e}"))?;
                let body = match &t {
                    Term::Compound(n, a)
                        if n == ":-" && a.len() == 2 && a[0] == Term::Atom("false".into()) =>
                    {
                        a[1].clone()
                    }
                    _ => return Err("a constraint is a denial: false :- body.".into()),
                };
                self.engine(0)?
                    .check_definition(&body)
                    .map_err(|w| format!("constraint not kept: {w}"))?;
                let vars = reason::make_list(variables(&body).into_iter().map(Term::Var).collect());
                // Tried against a planted violation before it is kept.
                let planted = self
                    .engine(0)?
                    .plant(&vars, &body)
                    .map_err(|e| e.to_string())?;
                if !matches!(&planted, Term::Compound(n, _) if n == "fires") {
                    return Err(format!(
                        "constraint not kept: tried against a planted violation, it did not fire ({})",
                        reason::show(&planted)
                    ));
                }
                let seq = self.journal.entries().len() as u64 + 1;
                let def = Term::Compound(
                    "node_def".into(),
                    vec![
                        Term::Int(seq as i64),
                        Term::Compound(
                            "constraint".into(),
                            vec![Term::Str(cname.trim().into()), vars, body],
                        ),
                    ],
                );
                let f = terms::encode(&def).map_err(|e| e.0)?;
                self.write_here("edit", json!({"op": "constraint", "name": cname.trim(), "rule": rule.trim(), "facts": [f]}))?;
                self.message = format!(
                    "constraint {} kept: it fires on a planted violation",
                    cname.trim()
                );
            }
            "layer" => {
                let text = words.join(" ");
                let (mark_text, goal_text) = match text.split_once(" where ") {
                    Some((m, g)) => (
                        m.trim().to_string(),
                        g.trim().trim_end_matches('.').to_string(),
                    ),
                    None => (
                        text.trim().trim_end_matches('.').to_string(),
                        "true".to_string(),
                    ),
                };
                // One term, so the mark and the goal share their variables.
                let (t, names) = self
                    .engine(0)?
                    .parse(&format!("layer(({mark_text}), ({goal_text}))"))
                    .map_err(|e| format!("not a mark and a goal: {e}"))?;
                let Term::Compound(_, a) = &t else {
                    unreachable!()
                };
                let (mark, goal) = (a[0].clone(), a[1].clone());
                crate::view::channels(&mark).map_err(|e| format!("layer not kept: {e}"))?;
                let in_goal = variables(&goal);
                let free: Vec<String> = variables(&mark)
                    .into_iter()
                    .filter(|v| !in_goal.contains(v))
                    .map(|v| {
                        names
                            .get(v as usize)
                            .cloned()
                            .unwrap_or_else(|| format!("_{v}"))
                    })
                    .collect();
                if !free.is_empty() {
                    return Err(format!(
                        "layer not kept: {} in the mark {} not bound by the goal",
                        free.join(", "),
                        if free.len() == 1 { "is" } else { "are" }
                    ));
                }
                self.engine(0)?
                    .check_definition(&goal)
                    .map_err(|w| format!("layer not kept: {w}"))?;
                let seq = self.journal.entries().len() as u64 + 1;
                let def = Term::Compound(
                    "node_def".into(),
                    vec![
                        Term::Int(seq as i64),
                        Term::Compound("layer".into(), vec![mark, goal]),
                    ],
                );
                let f = terms::encode(&def).map_err(|e| e.0)?;
                self.write_here("edit", json!({"op": "layer", "item": target, "mark": mark_text, "goal": goal_text, "facts": [f]}))?;
                self.message = format!("layer kept: {}", mark_text);
            }
            "derive" => {
                let text = words.join(" ");
                let (template, goal) = text
                    .split_once(" where ")
                    .ok_or("derive (text) a template with {Var}, where a goal")?;
                let (g, names) = self
                    .engine(0)?
                    .parse(goal.trim().trim_end_matches('.'))
                    .map_err(|e| format!("not a Prolog goal: {e}"))?;
                self.engine(0)?
                    .check_definition(&g)
                    .map_err(|w| format!("not derived: {w}"))?;
                let vars = reason::make_list(variables(&g).into_iter().map(Term::Var).collect());
                let def = Term::Compound(
                    "node_def".into(),
                    vec![
                        Term::Int(target as i64),
                        Term::Compound("query".into(), vec![vars, g]),
                    ],
                );
                let f = terms::encode(&def).map_err(|e| e.0)?;
                self.write_here("edit", json!({"op": "derive", "item": target, "template": template.trim(), "names": names, "facts": [f]}))?;
            }
            "interrogate" => {
                if self.claims_of(target).is_empty() {
                    return Err("the target states no claim".into());
                }
                self.view.interrogate = Some(target);
                self.view.panes[1].kind = PaneKind::Links;
                self.write_view("interrogate", json!({}));
            }
            "adopt" => {
                let claims = self.claims_of(target);
                if claims.is_empty() {
                    return Err("the target states no claim to adopt".into());
                }
                if let Some((_, _, p)) = claims.iter().find(|(_, _, p)| reserved(p)) {
                    return Err(format!(
                        "not adopted: the claim is a {}, which is reserved for constraint, derive and accept",
                        functor_of(p)
                    ));
                }
                for (seq, src, p) in claims {
                    let f = terms::encode(&p).map_err(|e| e.0)?;
                    self.write_here(
                        "adopt",
                        json!({"claim": seq, "item": target, "source": src, "facts": [f]}),
                    )?;
                }
                self.message = "adopted: a belief now, with its source kept".into();
            }
            "unadopt" => {
                let last = self
                    .path(self.tip)
                    .iter()
                    .rev()
                    .find(|e| {
                        e.kind == "adopt"
                            && e.body.as_ref().and_then(|b| b["item"].as_u64()) == Some(target)
                    })
                    .map(|e| e.seq)
                    .filter(|s| self.retractable(*s).is_ok())
                    .ok_or("the target has no adoption to retract")?;
                self.write_here("retract", json!({"target": last}))?;
                self.message = format!("adoption {last} retracted");
            }
            "close" | "commit" => {
                let s = self.supposition.clone().ok_or("no supposition is open")?;
                let parent = if name == "close" { s.actual } else { self.tip };
                self.view.interrogate = None;
                self.flush_text();
                let body = self.view.to_json("world", json!({name: true}));
                self.write(Some(parent), "view", body)?;
                self.message = if name == "close" {
                    format!("closed “{}”; it stays in the journal", s.name)
                } else {
                    format!("“{}” is now the actual world", s.name)
                };
            }
            "suppose" => {
                let n = words.join(" ");
                if n.is_empty() {
                    return Err("suppose (that) a name for the world".into());
                }
                self.write_here("suppose", json!({"name": n}))?;
            }
            _ => unreachable!(),
        }
        if self.view.scope != ScopeLevel::Statement {
            self.view.scope = ScopeLevel::Statement;
            self.view_dirty = true;
        }
        Ok(Mode::Normal)
    }

    fn is_in_branch(&self, id: u64, top: u64) -> bool {
        let mut at = Some(id);
        while let Some(x) = at {
            if x == top {
                return true;
            }
            at = self.outline.parent(x);
        }
        false
    }

    /// The command line as shown: once the command is recognised, each slot
    /// is preceded by its guide word in parentheses ("link 3.2 (to) source 9").
    pub fn guided(&self, input: &str) -> String {
        let mut words: Vec<&str> = input.split_whitespace().collect();
        if words.is_empty() {
            return String::new();
        }
        let Some(name) =
            find_command(words[0]).filter(|n| *n == lower(words[0]) || input.ends_with(' '))
        else {
            return input.to_string();
        };
        let cmd = COMMANDS.iter().find(|c| c.name == name).unwrap();
        words.remove(0);
        let open = input.ends_with(' ');
        if name == "link" {
            let (addr, kind) = match words.iter().position(|w| *w == "as") {
                Some(i) => (&words[..i], Some(words[i + 1..].join(" "))),
                None => (&words[..], None),
            };
            let shown = match self.addresses(addr, 2) {
                Some(_) => {
                    let k = (1..addr.len())
                        .find(|k| {
                            self.resolve(&addr[..*k].join(" ")).is_some()
                                && self.resolve(&addr[*k..].join(" ")).is_some()
                        })
                        .unwrap();
                    format!("link {} (to) {}", addr[..k].join(" "), addr[k..].join(" "))
                }
                None if addr.is_empty() => "link (to)".to_string(),
                None => format!("link (to) {}", addr.join(" ")),
            };
            return match kind {
                Some(k) => format!("{shown} (as) {k}"),
                None if open && self.addresses(addr, 2).is_some() => format!("{shown} (as)"),
                None => shown,
            };
        }
        let mut out = name.to_string();
        if let Some((noise, _)) = cmd.slots.first() {
            if !words.is_empty() || open {
                out.push_str(&format!(" ({noise})"));
            }
            if !words.is_empty() {
                out.push(' ');
                out.push_str(&words.join(" "));
            }
        }
        out
    }

    /// The last entry on the path that changed content: what undo undoes.
    fn content_tip(&self, from: u64) -> Option<u64> {
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
        let path = self.path(self.tip);
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
        let (_, _, with) = self.replay(self.tip, None);
        let (_, _, without) = self.replay(self.tip, Some(t));
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

/// The outline as facts, for goals: item_text(Item, Text) and
/// item_under(Item, Parent), each resting on the entry that created the
/// item (its number). Computed from the outline, not stated by the user,
/// so they go to the reasoner but not into the world's stated facts.
pub fn outline_facts(o: &Outline) -> Vec<(u64, Term)> {
    let mut out = Vec::new();
    for id in o.in_order() {
        let i = Term::Int(id as i64);
        out.push((
            id,
            Term::Compound("item_text".into(), vec![i.clone(), Term::Str(o.text(id))]),
        ));
        if let Some(p) = o.parent(id).filter(|p| *p != TOP) {
            out.push((
                id,
                Term::Compound("item_under".into(), vec![i, Term::Int(p as i64)]),
            ));
        }
    }
    out
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
                c.name
            ));
        }
    }
    direct.sort();
    direct.dedup();
    (direct, left)
}

/// Facts only constraint, derive and accept may write: definitions the
/// reasoner runs, checked by those commands (prolog/dataflow.pl, vet/1).
fn reserved(t: &Term) -> bool {
    matches!(t, Term::Compound(f, a) if (f == "node_def" && a.len() == 2) || (f == "rule_def" && a.len() == 3))
}

fn functor_of(t: &Term) -> String {
    match t {
        Term::Compound(f, a) => format!("{f}/{}", a.len()),
        Term::Atom(a) => format!("{a}/0"),
        _ => "term".into(),
    }
}

/// Whether a command line ends inside a quoted string ("…" or '…'), so a
/// ? typed now is part of the string, not a request for help.
fn in_quotes(input: &str) -> bool {
    let mut open: Option<char> = None;
    let mut escaped = false;
    for c in input.chars() {
        match (open, c) {
            (Some(_), '\\') if !escaped => {
                escaped = true;
                continue;
            }
            (Some(q), c) if c == q && !escaped => open = None,
            (None, '"' | '\'') => open = Some(c),
            _ => {}
        }
        escaped = false;
    }
    open.is_some()
}

fn find_command(w: &str) -> Option<&'static str> {
    let w = lower(w);
    if let Some(c) = COMMANDS.iter().find(|c| c.name == w) {
        return Some(c.name);
    }
    let hits: Vec<&Command> = COMMANDS.iter().filter(|c| c.name.starts_with(&w)).collect();
    (hits.len() == 1).then(|| hits[0].name)
}

/// Recognition on Escape: complete the command word if it is unambiguous.
fn complete_command(input: &str) -> String {
    let mut parts = input.splitn(2, ' ');
    let first = parts.next().unwrap_or("");
    let rest = parts.next();
    match find_command(first) {
        Some(name) if rest.is_none() => format!("{name} "),
        Some(name) => format!("{name} {}", rest.unwrap()),
        None => input.to_string(),
    }
}

/// What may come next in a command line, for `?`.
pub fn command_help(input: &str) -> Vec<String> {
    let first = input.split_whitespace().next().unwrap_or("");
    match find_command(first).filter(|_| input.contains(' ')) {
        Some(name) => {
            let c = COMMANDS.iter().find(|c| c.name == name).unwrap();
            let slots: Vec<String> = c.slots.iter().map(|(n, _)| format!("({n}) …")).collect();
            vec![format!("{name} {} — {}", slots.join(" "), c.help)]
        }
        None => COMMANDS
            .iter()
            .filter(|c| c.name.starts_with(&lower(first)))
            .map(|c| format!("{} — {}", c.name, c.help))
            .collect(),
    }
}

/// The word around a text position: [start, end).
fn word_at(text: &[char], pos: usize) -> (usize, usize) {
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
