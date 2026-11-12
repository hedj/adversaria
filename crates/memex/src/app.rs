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
use crate::reason::{self, ConstraintEval, Prov, WorldEval, solutions};
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
    Quasi(Quasi),
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
        let kind = |k: PaneKind| {
            if k == PaneKind::Outline {
                "outline"
            } else {
                "links"
            }
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
                self.panes[i].kind = if p["kind"] == "links" {
                    PaneKind::Links
                } else {
                    PaneKind::Outline
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
        name: "adopt",
        slots: &[],
        help: "make the target's claims beliefs, keeping their source (F3)",
    },
    Command {
        name: "claim",
        slots: &[("that", Slot::Text)],
        help: "the target states a claim: a Prolog fact, then from and its source",
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
        name: "derive",
        slots: &[("text", Slot::Text)],
        help: "the target shows a computed text: a template with {Var}, then where and a Prolog goal",
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
        name: "indent",
        slots: &[],
        help: "make the target a child of the item above it",
    },
    Command {
        name: "interrogate",
        slots: &[],
        help: "ask of the target's claim: who else claims it, who disputes it, its source and record (F2)",
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
        name: "retract",
        slots: &[("entry", Slot::Word)],
        help: "reverse one earlier entry, keeping later work; lists what depends on it first",
    },
    Command {
        name: "suppose",
        slots: &[("that", Slot::Text)],
        help: "start a named world (milestone 4 gives it meaning)",
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
        help: "retract the target's last adoption (F4)",
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
            view_dirty: false,
            last_leap: None,
            quit: false,
            supposition: None,
            engines: [None, None, None],
            eval: [None, None],
            preview: None,
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
        let f = self.view.focus;
        if self.view.panes[f].kind == PaneKind::Links && !matches!(ev.key, Key::F(_)) {
            self.message = "this pane shows links; switch (F6) to edit".into();
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
            Key::F(1) => {
                self.flush();
                return Mode::Quasi(Active {
                    q: Quasi::Command,
                    input: String::new(),
                    latched: true,
                    used: false,
                    listing: true,
                });
            }
            Key::F(2) => self.run_simple("interrogate"),
            Key::F(3) => self.run_simple("adopt"),
            Key::F(4) => self.run_simple("unadopt"),
            Key::F(6) => self.run_simple("switch"),
            Key::F(10) => self.run_simple("quit"),
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
            Key::Char('?') if a.q != Quasi::Suppose => a.listing = !a.listing,
            Key::Char(' ') if a.q == Quasi::Scope => {
                self.view.scope = match self.view.scope {
                    ScopeLevel::Word => ScopeLevel::Statement,
                    _ => ScopeLevel::Branch,
                };
            }
            Key::Char(c) if a.q != Quasi::Scope => a.input.push(c),
            Key::Backspace => {
                if a.input.pop().is_none() && a.latched {
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
    /// only, each belief that breaks a constraint together with it (the
    /// supposition is entrenched over the beliefs it contradicts).
    fn suppose(&mut self, name: &str, fact: &Term) -> Result<(), String> {
        let f = terms::encode(fact).map_err(|e| e.0)?;
        let s = self.write_here("suppose", json!({"name": name, "facts": [f]}))?;
        let mut set_aside = Vec::new();
        for _ in 0..5 {
            self.evaluate()?;
            let eval = self.eval[0].as_ref().ok_or("no evaluation")?;
            let mut targets: Vec<u64> = Vec::new();
            for c in &eval.constraints {
                for v in &c.violations {
                    if v.prov.iter().any(|p| p.seq() == s.seq) {
                        targets.extend(v.prov.iter().map(Prov::seq).filter(|q| *q != s.seq));
                    }
                }
            }
            targets.sort();
            targets.dedup();
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
        self.message = if places.is_empty() {
            "supposed; nothing set aside".into()
        } else {
            format!("supposed; set aside in this world: {}", places.join(", "))
        };
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
            let engine = self.engine(k)?;
            engine.sync(&facts).map_err(|e| e.to_string())?;
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
            self.eval[k] = Some(eval);
        }
        self.preview = None;
        if let Some(item) = self.view.interrogate {
            self.preview = Some(self.preview_adoption(item)?);
        }
        Ok(())
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
                | "suppose"
                | "new"
                | "constraint"
                | "close"
                | "commit"
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
                    _ => return Err("pane (shows) outline, or links".into()),
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
            "derive" => {
                let text = words.join(" ");
                let (template, goal) = text
                    .split_once(" where ")
                    .ok_or("derive (text) a template with {Var}, where a goal")?;
                let (g, names) = self
                    .engine(0)?
                    .parse(goal.trim().trim_end_matches('.'))
                    .map_err(|e| format!("not a Prolog goal: {e}"))?;
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
