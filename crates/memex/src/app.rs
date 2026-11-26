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
use format::record::{Entry, EntryKind};
use format::terms::Term;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quasi {
    LeapDown,
    LeapUp,
    #[default]
    Command,
    Suppose,
}

impl Quasi {
    /// Every quasimode, in order.
    pub const ALL: [Quasi; 4] = [
        Quasi::LeapDown,
        Quasi::LeapUp,
        Quasi::Command,
        Quasi::Suppose,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Quasi::LeapDown => "LEAP DOWN",
            Quasi::LeapUp => "LEAP UP",
            Quasi::Command => "COMMAND",
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

/// Keys free for the user's bindings; unbound, a few have a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Special {
    Insert,
    PageUp,
    PageDown,
    /// The Menu key, or Shift+F10: unless bound, the right-click's menu
    /// at the cursor's item.
    Menu,
    Pause,
    ScrollLock,
    PrintScreen,
    CapsLock,
    NumLock,
    /// An arrow, Home or End with Shift held: unless bound, it selects
    /// (crate::selection).
    ShiftUp,
    ShiftDown,
    ShiftLeft,
    ShiftRight,
    ShiftHome,
    ShiftEnd,
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
    /// The items that hold the last Leap's pattern.
    Matches,
}

impl PaneKind {
    /// The kind of pane a word names, as the pane command takes it.
    pub(crate) fn named(word: Option<&str>) -> Option<PaneKind> {
        match word {
            Some("outline") => Some(Self::Outline),
            Some("links") => Some(Self::Links),
            Some("suggestions") => Some(Self::Suggestions),
            Some("matches") => Some(Self::Matches),
            _ => None,
        }
    }

    /// The word for the kind, as the pane command takes it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Outline => "outline",
            Self::Links => "links",
            Self::Suggestions => "suggestions",
            Self::Matches => "matches",
        }
    }
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
    /// The selected item: the cursor of the outline with focus. Only an
    /// outline takes focus; the pane beside it shows the selected item's
    /// links, or a list, and has no cursor of its own (docs/concepts.md,
    /// decision 4).
    pub(crate) fn selected(&self) -> u64 {
        self.panes[self.focus].cursor
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

    /// Move focus to the other pane if it is an outline, which starts at
    /// this pane's cursor if it has none; else say what the other shows.
    pub(crate) fn switch_focus(&mut self) -> Result<(), PaneKind> {
        let from = self.panes[self.focus].cursor;
        let other = &mut self.panes[1 - self.focus];
        if other.kind != PaneKind::Outline {
            return Err(other.kind);
        }
        if other.cursor == TOP {
            other.cursor = from;
        }
        self.focus = 1 - self.focus;
        Ok(())
    }

    /// Focus on an outline: a journal written before only an outline took
    /// focus may have left it on another pane, or shown no outline.
    fn focus_an_outline(&mut self) {
        if self.panes[self.focus].kind == PaneKind::Outline {
            return;
        }
        self.focus = 1 - self.focus;
        self.panes[self.focus].kind = PaneKind::Outline;
    }

    /// Interrogate a claim item: show it in the links pane.
    pub(crate) fn show_claim(&mut self, item: u64) {
        self.interrogate = Some(item);
        self.panes[1].kind = PaneKind::Links;
    }

    pub(crate) fn to_json(&self, op: &str, extra: Value) -> Value {
        let mut v = json!({
            "op": op,
            "panes": self.panes.iter().map(|p| json!({"kind": p.kind.name(), "cursor": p.cursor})).collect::<Vec<_>>(),
            "focus": self.focus, "pos": self.pos, "folded": self.folded.iter().collect::<Vec<_>>(), "interrogate": self.interrogate,
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
                    Some("matches") => PaneKind::Matches,
                    _ => PaneKind::Outline,
                };
                self.panes[i].cursor = p["cursor"].as_u64().unwrap_or(TOP);
            }
        }
        self.focus = (b["focus"].as_u64().unwrap_or(0) as usize).min(1);
        self.focus_an_outline();
        self.pos = b["pos"].as_u64().unwrap_or(0) as usize;
        self.folded = b["folded"]
            .as_array()
            .map(|f| f.iter().filter_map(|x| x.as_u64()).collect())
            .unwrap_or_default();
        self.interrogate = b["interrogate"].as_u64();
    }
}

/// Where a cursor goes after the outline changes: it stays on the
/// notebook, above the first item, on an item that is left, or where
/// nothing could have vanished; else it goes to the nearest item before
/// it that is left, else the first.
fn surviving(outline: &Outline, before: &[u64], c: u64) -> u64 {
    if c == TOP || outline.contains(c) || before.is_empty() {
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
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Active {
    pub kind: Kind,
    pub input: String,
    /// How its key is held.
    pub hold: Hold,
    /// `?` was pressed: list everything that may come next.
    pub listing: bool,
    /// The key that opened it, whose release ends it: Suppose's opens
    /// Command with `suppose ` typed.
    pub key: Quasi,
}

/// Which quasimode, with what only it keeps: a Leap, where it began and
/// the near match it warned of; Command, while its line holds a sentence
/// (`suppose …`, `ace …`), what APE makes of it and the free English it
/// began as.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// A Leap: its direction, the item the cursor was on when it began,
    /// which its matches are counted from as the cursor moves, and the
    /// near match it warned of, if Enter found none.
    Leap {
        forward: bool,
        from: u64,
        near: Option<Near>,
    },
    Command(English),
}

impl Default for Kind {
    fn default() -> Kind {
        Kind::Command(English::default())
    }
}

impl Kind {
    /// A Leap that warned of a near match.
    pub(crate) fn warn(&mut self, warned: Near) {
        if let Kind::Leap { near, .. } = self {
            *near = Some(warned);
        }
    }

    /// The kind of a quasimode entered with the cursor on an item.
    pub(crate) fn of(q: Quasi, from: u64) -> Kind {
        match q {
            Quasi::LeapDown => Kind::Leap {
                forward: true,
                from,
                near: None,
            },
            Quasi::LeapUp => Kind::Leap {
                forward: false,
                from,
                near: None,
            },
            Quasi::Command | Quasi::Suppose => Kind::default(),
        }
    }
}

/// What Leap warned of when Enter found no match: the pattern, and the
/// place and words of an item that holds something a few edits from it.
#[derive(Debug, Clone, PartialEq)]
pub struct Near {
    pub pattern: String,
    pub place: String,
    pub found: String,
}

/// The English typed in Suppose: what APE makes of it, and the fullest
/// text typed that was not ACE, the free English, which a supposition
/// mended into ACE keeps as a confirmed pair (crate::pairs).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct English {
    pub hint: Option<Hint>,
    pub free: Option<String>,
    /// A line refused, and the line its refusal offered in its place, its
    /// first word put right as a command: Tab takes it while the line is
    /// as it was refused (crate::actions::reader).
    pub offered: Option<(String, String)>,
    /// Whether the line is read by its form, its first word naming no
    /// command (crate::actions::reader), as last checked.
    pub reads: bool,
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
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Hold {
    #[default]
    Down,
    Used,
    Latched,
}

impl Active {
    /// Which quasimode it is.
    pub fn q(&self) -> Quasi {
        match self.kind {
            Kind::Leap { forward: true, .. } => Quasi::LeapDown,
            Kind::Leap { forward: false, .. } => Quasi::LeapUp,
            Kind::Command(_) => Quasi::Command,
        }
    }

    /// The sentence a Command line holds, if it holds one: the text after
    /// its verb, `suppose` or `ace`, or the whole line, if it is read by
    /// its form (crate::actions::reader).
    pub fn sentence(&self) -> Option<&str> {
        let verbs = ["suppose ", "ace "];
        let command = matches!(self.kind, Kind::Command(_));
        let rest = verbs.iter().find_map(|v| self.input.strip_prefix(v));
        let whole = self.reads().then_some(self.input.as_str());
        rest.or(whole).filter(|_| command)
    }

    /// Whether a Command line is read by its form, its first word naming
    /// no command.
    pub fn reads(&self) -> bool {
        matches!(&self.kind, Kind::Command(e) if e.reads)
    }

    /// Whether its sentence is a supposition's.
    pub fn supposes(&self) -> bool {
        self.sentence().is_some() && self.input.starts_with("suppose ")
    }

    /// While a sentence is typed, what APE makes of it.
    pub fn hint(&self) -> Option<&Hint> {
        let Kind::Command(english) = &self.kind else {
            return None;
        };
        self.sentence().and(english.hint.as_ref())
    }

    /// In a Leap, the near match it warned of for what is typed now.
    pub fn near(&self) -> Option<&Near> {
        match &self.kind {
            Kind::Leap { near: Some(n), .. } if n.pattern == self.input.trim() => Some(n),
            _ => None,
        }
    }

    /// In a Leap, the item the cursor was on when it began.
    pub fn leap_from(&self) -> Option<u64> {
        match self.kind {
            Kind::Leap { from, .. } => Some(from),
            _ => None,
        }
    }

    /// Whether it was tapped rather than held, and stays.
    pub fn latched(&self) -> bool {
        self.hold == Hold::Latched
    }

    /// Whether it lists over the outline: a Leap or Command, listing
    /// everything or with something typed.
    pub(crate) fn lists(&self) -> bool {
        if self.sentence().is_some() {
            return self.listing;
        }
        self.listing || !self.input.is_empty()
    }

    /// A quasimode latched, as if its key were tapped, nothing typed yet.
    pub(crate) fn tapped(kind: Kind) -> Active {
        let held = Active::held(kind);
        Active {
            hold: Hold::Latched,
            ..held
        }
    }

    /// A quasimode whose key is held, nothing typed yet.
    pub(crate) fn held(kind: Kind) -> Active {
        let key = match kind {
            Kind::Leap { forward: true, .. } => Quasi::LeapDown,
            Kind::Leap { forward: false, .. } => Quasi::LeapUp,
            Kind::Command(_) => Quasi::Command,
        };
        Active::held_on(key, kind, "")
    }

    /// A quasimode a key opened, held, with a text typed in already: the
    /// Suppose key opens Command with `suppose ` (guide § 2.3).
    pub(crate) fn held_on(key: Quasi, kind: Kind, typed: &str) -> Active {
        let input = typed.into();
        Active {
            kind,
            input,
            key,
            ..Active::default()
        }
    }

    /// Put the first repair of the sentence typed in its place, after its
    /// verb; whether there was one (Tab in a sentence).
    /// Offer a line in place of this one, if there is one: Tab takes it
    /// while the line stays as it is.
    pub(crate) fn offer_mend(&mut self, mended: Option<String>) {
        if let Kind::Command(english) = &mut self.kind {
            english.offered = mended.map(|m| (self.input.clone(), m));
        }
    }

    /// Take the line offered in place of this one, if this is the line it
    /// was offered for: whether one was taken.
    pub(crate) fn take_offer(&mut self) -> bool {
        let Kind::Command(english) = &mut self.kind else {
            return false;
        };
        match english.offered.take() {
            Some((refused, mended)) if refused == self.input => self.input = mended,
            _ => return false,
        }
        true
    }

    pub(crate) fn apply_repair(&mut self) -> bool {
        let Some(mended) = self.hint().and_then(|h| h.mended.clone()) else {
            return false;
        };
        let sentence = self.sentence().map_or(0, str::len);
        let before = &self.input[..self.input.len() - sentence];
        self.input = format!("{before}{mended}");
        true
    }

    /// The command palette: the Command quasimode, latched, listing what
    /// may come next.
    /// Command latched with a line typed, prompting for what it needs.
    pub(crate) fn prompting(line: String) -> Active {
        Active {
            input: line,
            ..Active::tapped(Kind::default())
        }
    }

    pub(crate) fn palette() -> Active {
        let held = Active::held(Kind::default());
        Active {
            hold: Hold::Latched,
            listing: true,
            ..held
        }
    }
}

/// The command a retraction answers: `retract`, of an entry, or
/// `dismiss change`, of a revision's change; the retraction says its
/// verb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeBack {
    Retract,
    Dismiss,
}

impl TakeBack {
    /// What it is called while it asks, and what it asks of: the entry,
    /// and what the entry did.
    pub fn asking(self, target: u64, what: &str) -> (&'static str, String) {
        let [name, noun, ..] = self.words();
        (name, format!(" {noun} {target}: {what}"))
    }

    /// How it is answered.
    pub fn answers(self) -> String {
        format!("Enter {} it · Esc keeps it", self.words()[2])
    }

    /// What it says once done, with the entry that records it.
    pub fn done(self, target: u64, what: &str, by: u64) -> String {
        let [_, noun, _, did] = self.words();
        format!("{did} {noun} {target}: {what} (entry {by})")
    }

    /// What it says once the entry is kept.
    pub fn kept(self, target: u64, what: &str) -> String {
        format!("kept {} {target}: {what}", self.words()[1])
    }

    /// What it is called asking, what it calls the entry, and its verb
    /// now and done.
    fn words(self) -> [&'static str; 4] {
        match self {
            TakeBack::Retract => ["RETRACT", "entry", "retracts", "retracted"],
            TakeBack::Dismiss => ["DISMISS", "change", "dismisses", "dismissed"],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Quasi(Active),
    /// A retraction waits for Enter: of which entry, what depends on it,
    /// and the command that asked.
    ConfirmRetract {
        target: u64,
        dependents: Vec<u64>,
        asked: TakeBack,
    },
    /// "Do you mean?": a supposition's readings, the one shown, waiting for
    /// Enter (yes), Esc (no) or Tab (the next reading); and the free
    /// English its text, a sentence of ACE, was mended from, if any.
    ConfirmSuppose {
        text: String,
        readings: Vec<Term>,
        at: usize,
        english: Option<String>,
    },
    /// What a right-click offers (crate::session::Menu).
    Menu(crate::session::Menu),
}

impl Mode {
    /// The Command line typed so far, if Command is on.
    pub(crate) fn command_line(&self) -> Option<&str> {
        match self {
            Mode::Quasi(a) if a.q() == Quasi::Command => Some(&a.input),
            _ => None,
        }
    }

    /// Text added to a quasimode's line, as if typed, if one is on.
    pub(crate) fn type_in(&mut self, text: &str) {
        if let Mode::Quasi(a) = self {
            a.input.push_str(text);
        }
    }
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
        kind: EntryKind,
        body: Value,
    ) -> Result<Entry, String> {
        let parent = self.started_before(parent);
        let body = self.marked(body);
        let e = self.notebook.append(parent, kind, body)?;
        self.apply(&e);
        Ok(e)
    }

    /// Do what the memex does on its own, not at a command or a key: read
    /// a text, link what it names, record what it observed. What it writes
    /// then is marked `"by": "memex"` (docs/design.md, The journal
    /// format), so the record tells the user's entries from its own; text
    /// typed is the user's whenever it is written (flush_text).
    pub(crate) fn on_its_own<T>(&mut self, act: impl FnOnce(&mut Self) -> T) -> T {
        let was = std::mem::replace(&mut self.session.on_its_own, true);
        let done = act(self);
        self.session.on_its_own = was;
        done
    }

    /// A body, marked as the memex's own if it writes on its own.
    fn marked(&self, mut body: Value) -> Value {
        if self.session.on_its_own {
            body["by"] = json!("memex");
        }
        body
    }

    /// The parent of an entry, once the session's start entry is in the
    /// record: the first entry a session writes follows one, which records
    /// the build that wrote the session (docs/design.md, The journal
    /// format), so which build wrote each entry, and where each session
    /// began, are in the record; opening a journal alone writes nothing.
    /// The journal's header declares the version its entries are in.
    /// An entry meant to follow the tip follows the start entry.
    fn started_before(&mut self, parent: Option<u64>) -> Option<u64> {
        let tip = self.notebook.tip;
        match self.start_session() {
            Some(start) if parent == Some(tip) => Some(start),
            _ => parent,
        }
    }

    /// Write the session's start entry, if it is not yet written: its
    /// number, if this wrote it.
    fn start_session(&mut self) -> Option<u64> {
        if self.session.started {
            return None;
        }
        self.session.started = true;
        let tip = self.notebook.tip;
        let start = json!({"build": crate::notebook::BUILD, "by": "memex"});
        let body = self.view_body("start", start);
        match self.write(Some(tip), EntryKind::View, body) {
            Ok(e) => Some(e.seq),
            Err(m) => {
                self.session.message = m;
                None
            }
        }
    }

    /// The number the next entry will have, the session's start entry
    /// written first if it is not yet. An entry that names itself, or a
    /// record that names where its run began, takes its number from here,
    /// not from Notebook::next_seq, which the start entry would take.
    pub(crate) fn next_entry(&mut self) -> u64 {
        self.start_session();
        self.notebook.next_seq()
    }

    pub(crate) fn write_here(&mut self, kind: EntryKind, body: Value) -> Result<Entry, String> {
        self.write(Some(self.notebook.tip), kind, body)
    }

    /// What the memex does when the user pauses, all of it: write whatever
    /// is pending, then read the text that waits to be read (pause_step).
    pub fn idle(&mut self) {
        while self.pause_step() {}
    }

    /// One step of what the memex does when the user pauses: write what is
    /// pending, or send a part of the parser's lexicon, or read one item's
    /// text, or keep a copy of the outline that an undo can start from;
    /// whether more remains. Reading is slower than a keystroke may
    /// be, so the window takes a step at a time, and stops when a key
    /// comes: a key waits for one step at most, and text is read only
    /// while the user pauses.
    pub fn pause_step(&mut self) -> bool {
        if self.pending() {
            self.flush();
        } else if self.session.waits_to_read() {
            self.read_step();
        } else {
            self.notebook.keep_outline();
        }
        self.idle_work()
    }

    /// Whether a pause has anything to do.
    pub fn idle_work(&self) -> bool {
        self.pending() || self.session.waits_to_read() || !self.notebook.outline_kept()
    }

    /// Write whatever is pending: typed text, then cursor moves.
    pub fn flush(&mut self) {
        self.flush_text();
        if let Some(change) = self.session.view_change.take() {
            let body = self.view_body(change.op(), json!({}));
            let written = self.write_here(EntryKind::View, body);
            self.session.report(written);
        }
    }

    pub(crate) fn write_view(&mut self, op: &str, extra: Value) {
        self.flush_text();
        self.session.view_change = None;
        let body = self.view_body(op, extra);
        let written = self.write_here(EntryKind::View, body);
        self.session.report(written);
    }

    /// A view entry's body: what is shown, the claim item the inspector
    /// shows the questions of among it, and what the action adds.
    pub(crate) fn view_body(&self, op: &str, mut extra: Value) -> Value {
        extra["inspects"] = json!(self.inspected_claim());
        self.session.view.to_json(op, extra)
    }

    pub(crate) fn flush_text(&mut self) {
        let Some(p) = self.session.take_text() else {
            return;
        };
        let before = self.notebook.tip;
        let mine = std::mem::replace(&mut self.session.on_its_own, false);
        let written = self.write_text(p.item, &p.text);
        self.session.on_its_own = mine;
        self.session.report(written);
        if self.notebook.tip != before {
            self.will_read(p.item, self.notebook.tip);
        }
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
        self.write_here(EntryKind::Edit, body)?;
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

    /// Check that an entry can be retracted; return the entries that depend
    /// on it: those that would no longer apply without it.
    pub fn retractable(&self, t: u64) -> Result<Vec<u64>, String> {
        if let Some(why) = self.notebook.unretractable(t) {
            return Err(why);
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

impl Notebook {
    /// The items whose facts are about an entity: facts with it first.
    pub(crate) fn items_about(&self, entity: &Term) -> Vec<u64> {
        let facts = self.facts_now().unwrap_or_default();
        let mut out: Vec<u64> = facts
            .iter()
            .filter(|(_, f)| matches!(f, Term::Compound(_, a) if a.first() == Some(entity)))
            .filter_map(|(s, _)| self.outline.fact_item.get(s).copied())
            .collect();
        out.dedup();
        out
    }

    /// Why an entry cannot be retracted, if it cannot: it is not in this
    /// world, is its first, is no change, or is retracted already. It
    /// reads the branch, not a replay, so it is quick enough to ask as the
    /// pointer moves.
    pub(crate) fn unretractable(&self, t: u64) -> Option<String> {
        let branch = self.branch(self.tip);
        let Some(e) = branch.entries.iter().find(|e| e.seq == t) else {
            return Some(format!("entry {t} is not in this world"));
        };
        if t == 1 || !CONTENT.contains(&e.kind()) {
            return Some(format!("entry {t} ({}) cannot be retracted", e.kind()));
        }
        branch
            .retracts(t)
            .then(|| format!("entry {t} is already retracted"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_typed_is_the_user_s_though_the_memex_writes_it_on_its_own() {
        // Catches the user's typing marked as the memex's, when its own
        // work, such as recording what it observed, writes the text
        // still waiting.
        let dir = std::env::temp_dir().join(format!("adversaria-own-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::open(&dir).unwrap();
        app.new_item_after(TOP).unwrap();
        let item = app.cursor();
        app.session.type_into(item, vec![], 'x');
        app.on_its_own(|app| app.flush_text());
        let last = app.journal().entries().last().unwrap().clone();
        let body = last.body().unwrap();
        assert_eq!(
            (last.kind(), body["op"].as_str()),
            (EntryKind::Edit, Some("text"))
        );
        assert!(body.get("by").is_none(), "{body}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_pause_after_a_change_keeps_a_copy_of_the_outline_in_one_step() {
        // Catches a pause with nothing to do after a change, or one that
        // never keeps the copy: an undo then replays from the root
        // (Notebook::rebuild).
        let dir = std::env::temp_dir().join(format!("adversaria-pause-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::open(&dir).unwrap();
        app.idle();
        app.new_item_after(TOP).unwrap();
        assert!(app.idle_work(), "a copy to keep");
        assert!(!app.pause_step(), "kept in one step");
        assert!(app.notebook.outline_kept());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_view_with_focus_on_another_pane_is_restored_with_focus_on_an_outline() {
        // Catches a view entry written before only an outline took focus
        // restoring focus on the links, where keys reach nothing.
        for kinds in [["outline", "links"], ["links", "suggestions"]] {
            let panes: Vec<Value> = kinds
                .iter()
                .map(|k| json!({"kind": k, "cursor": 2}))
                .collect();
            let mut view = View::default();
            view.restore(&json!({"op": "pane", "panes": panes, "focus": 1}));
            let focused = view.focused().kind;
            assert_eq!(focused, PaneKind::Outline, "{kinds:?}");
        }
    }
}
