//! What a session of the memex holds and the journal does not: what the
//! user sees and is in the middle of, how the session is set up, and what
//! it has noticed outside the journal (docs/structure.md § The code's structure).

use crate::app::{Active, Event, Key, Mode, PendingText, Press, Quasi, View};
use crate::found::Found;
use crate::notebook::strings;
use crate::plumb::Offer;
use crate::present::Presentation;
use format::record::Entry;
use serde_json::Value;
use std::collections::BTreeMap;

/// The violations and the disagreements shown, by key.
pub type Observed = (Vec<String>, Vec<String>);

/// What the user sees and is in the middle of, the keys bound to
/// commands, whether the user has quit, and what the session has noticed
/// outside the journal: all that it holds and the journal does not.
#[derive(Default)]
pub struct Session {
    pub view: View,
    pub mode: Mode,
    pub message: String,
    /// Text typed into an item and not yet written.
    pub(crate) pending_text: Option<PendingText>,
    /// The view moved and is not yet written.
    pub(crate) view_dirty: bool,
    /// The last leap's pattern and direction, for leaping again.
    pub(crate) last_leap: Option<(String, bool)>,
    /// What the list beside the outline lists: the last find's, or the
    /// last leap's matches.
    pub found: Option<Found>,
    /// Where jumps came from, for back and forward.
    pub trail: Trail,
    /// What is selected, if anything.
    pub selection: Option<Selection>,
    /// What Ctrl+C or Ctrl+X took, for Ctrl+V (crate::habits).
    pub clipboard: Option<Clip>,
    pub keys: Keys,
    /// The plumbing rules a right-click asks (crate::plumb).
    pub plumber: crate::plumb::Plumber,
    pub quit: bool,
    /// The cell under the mouse pointer, if it is over the window.
    pub pointer: Option<(u16, u16)>,
    outside: Outside,
}

/// What is selected (crate::selection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// From an item and a place in its text, in characters, to the
    /// cursor, so a move with Shift extends it.
    From { item: u64, pos: usize },
    /// Text a drag of the mouse covers, as drawn.
    Drawn(String),
}

/// What Ctrl+C or Ctrl+X took: text, or items, marked to move if cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clip {
    Text(String),
    Items { items: Vec<u64>, cut: bool },
}

impl Clip {
    /// What was taken, and what Ctrl+V will do with it.
    pub(crate) fn said(&self) -> String {
        match self {
            Clip::Text(t) if t.is_empty() => "nothing is selected to take".into(),
            Clip::Text(t) => format!("took “{t}”: Ctrl+V puts it at the text cursor"),
            Clip::Items { items, .. } if items.is_empty() => "there is no item to take".into(),
            Clip::Items { items, cut: true } => match items.len() {
                1 => "cut 1 item: Ctrl+V moves it after the cursor's".into(),
                n => format!("cut {n} items: Ctrl+V moves them after the cursor's"),
            },
            Clip::Items { items, cut: false } => match items.len() {
                1 => "copied 1 item: Ctrl+V puts a copy after the cursor's".into(),
                n => format!("copied {n} items: Ctrl+V puts copies after the cursor's"),
            },
        }
    }

    /// Whether it took nothing.
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Clip::Text(t) => t.is_empty(),
            Clip::Items { items, .. } => items.is_empty(),
        }
    }

    /// Whether it is items cut, which a paste moves once.
    pub(crate) fn is_cut(&self) -> bool {
        matches!(self, Clip::Items { cut: true, .. })
    }
}

/// What a right-click offers (crate::plumb): the offers, the one chosen,
/// and the cell, column and row, where the menu opens; with none, it
/// opens at the cursor's item, as the Menu key opens it.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    /// The text the menu acts on.
    pub text: String,
    pub offers: Vec<Offer>,
    pub chosen: usize,
    pub at: Option<(u16, u16)>,
}

/// What the menu does with a key or a click.
pub enum Answer {
    Keep(Menu),
    Close,
    Run(String),
}

impl Menu {
    /// A menu of offers for a text, the first chosen, at a cell or at the
    /// cursor's item.
    pub fn new(text: &str, offers: Vec<Offer>, at: Option<(u16, u16)>) -> Menu {
        Menu {
            text: text.trim().to_string(),
            offers,
            chosen: 0,
            at,
        }
    }

    /// A key: Up and Down choose, Enter or a number runs, Esc closes; any
    /// other key, or a release, keeps the menu as it is.
    pub fn answer(self, ev: Event) -> Answer {
        if ev.press == Press::Up {
            return Answer::Keep(self);
        }
        let n = self.offers.len();
        match ev.key {
            Key::Up => Answer::Keep(self.moved(n - 1)),
            Key::Down => Answer::Keep(self.moved(1)),
            Key::Escape => Answer::Close,
            Key::Enter => {
                let i = self.chosen;
                self.run(i)
            }
            Key::Char(c) => self.numbered(c),
            _ => Answer::Keep(self),
        }
    }

    /// A click: on an offer, it runs; anywhere else, the menu closes.
    pub fn clicked(&self, c: &Presentation) -> Answer {
        let on = |o: &&Offer| Presentation::Command(o.line.clone()) == *c;
        self.offers
            .iter()
            .find(on)
            .map_or(Answer::Close, |o| Answer::Run(o.line.clone()))
    }

    /// Each offer's words, by its number (the first nine), and whether it
    /// is the one chosen, with its command line.
    pub fn entries(&self) -> Vec<(String, bool, String)> {
        let number = |i: usize| {
            if i < 9 {
                (i + 1).to_string()
            } else {
                " ".into()
            }
        };
        let entry = |(i, o): (usize, &Offer)| {
            (
                format!("{} {}", number(i), o.label),
                i == self.chosen,
                o.line.clone(),
            )
        };
        self.offers.iter().enumerate().map(entry).collect()
    }

    /// The menu with the choice moved on, round at the end.
    fn moved(self, by: usize) -> Menu {
        let chosen = (self.chosen + by) % self.offers.len();
        Menu { chosen, ..self }
    }

    /// Run an offer by its number, 1 to 9; another key keeps the menu.
    fn numbered(self, c: char) -> Answer {
        match c.to_digit(10).and_then(|d| (d as usize).checked_sub(1)) {
            Some(i) => self.run(i),
            None => Answer::Keep(self),
        }
    }

    /// Run the offer at a place, if there is one.
    fn run(self, i: usize) -> Answer {
        match self.offers.get(i) {
            Some(o) => Answer::Run(o.line.clone()),
            None => Answer::Keep(self),
        }
    }
}

/// Where the cursor jumped from, so that back returns there: the items
/// left by each jump (follow, leap, go), the last on top, and those back
/// left, for forward. Each jump's view entry records where it came from,
/// so the trail is the journal's, and a reopened notebook has it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Trail {
    pub back: Vec<u64>,
    pub forward: Vec<u64>,
}

impl Trail {
    /// Follow a view entry: a jump puts where it came from on the back
    /// list and empties the forward one; back and forward move between.
    fn apply(&mut self, b: &Value) {
        let (Some(from), to) = (b["from"].as_u64(), b["to"].as_u64()) else {
            return;
        };
        match b["op"].as_str() {
            Some("back") => step(&mut self.back, &mut self.forward, from, to),
            Some("forward") => step(&mut self.forward, &mut self.back, from, to),
            _ if to != Some(from) => {
                self.back.push(from);
                self.forward.clear();
            }
            _ => {}
        }
    }
}

/// A step along the trail from one item to another: the list stepped
/// along loses the items up to the one reached (those passed over were
/// gone), and the other gains the one left.
fn step(along: &mut Vec<u64>, other: &mut Vec<u64>, from: u64, to: Option<u64>) {
    while let Some(i) = along.pop() {
        if Some(i) == to {
            break;
        }
    }
    other.push(from);
}

/// The pattern and direction of a leap entry.
fn leap_of(b: &Value) -> Option<(String, bool)> {
    let pattern = b["pattern"].as_str()?;
    Some((pattern.to_string(), b["forward"] != false))
}

/// What a find entry lists, if it names a list.
fn found_of(b: &Value) -> Option<Found> {
    let filter = b["filter"].as_str().filter(|f| !f.is_empty())?;
    Some(Found::of(filter))
}

/// The violations and the disagreements an observed entry records.
fn observed_of(b: &Value) -> Observed {
    (strings(&b["violations"]), strings(&b["disagreements"]))
}

/// The keys of the configuration, as the screen names them: those bound
/// to command lines, and each quasimode's key, by the name its hints use.
pub struct Keys {
    bindings: Vec<(Key, String)>,
    quasimodes: Vec<(Quasi, String)>,
}

impl Keys {
    /// Keys bound to command lines, and each quasimode's key by name.
    pub fn new(bindings: Vec<(Key, String)>, quasimodes: Vec<(Quasi, String)>) -> Keys {
        Keys {
            bindings,
            quasimodes,
        }
    }

    /// The name of a quasimode's key, as its hints say it.
    pub fn quasi_key(&self, q: Quasi) -> String {
        let named = self.quasimodes.iter().find(|(x, _)| *x == q);
        named.map_or_else(|| q.label().to_string(), |(_, name)| name.clone())
    }

    /// An offer, its label naming the key bound to its line, if one is,
    /// so a menu teaches the key ("adopt its claims  (F3)").
    pub(crate) fn labelled(&self, o: Offer) -> Offer {
        match self.bound_to(&o.line) {
            Some(key) => Offer {
                label: format!("{}  ({key})", o.label),
                ..o
            },
            None => o,
        }
    }

    /// The name of the first key bound to a command line, if one is.
    pub fn bound_to(&self, line: &str) -> Option<String> {
        let bound = self.bindings.iter().find(|(_, l)| l == line);
        bound.map(|(k, _)| crate::config::key_name(*k))
    }
}

/// What the session knows that the journal does not hold.
struct Outside {
    /// Where the source check asks Crossref: its works API, unless a test
    /// points it elsewhere.
    crossref: String,
    /// Results whose input files changed since: item -> those files.
    stale: BTreeMap<u64, Vec<String>>,
    /// The last violations and disagreements recorded in the journal.
    observed: Option<Observed>,
    /// Why the text of an item was not read, last time it was written,
    /// and APE's repair of it, if it has one.
    prose: Option<(u64, String, Option<String>)>,
    /// The items whose text is written and waits to be read, each with the
    /// entry that wrote it.
    to_read: BTreeMap<u64, u64>,
}

impl Default for Outside {
    fn default() -> Outside {
        Outside {
            crossref: "https://api.crossref.org/works/".into(),
            stale: BTreeMap::new(),
            observed: None,
            prose: None,
            to_read: BTreeMap::new(),
        }
    }
}

impl Default for Keys {
    fn default() -> Keys {
        crate::config::Config::default().keys()
    }
}

impl Keys {
    /// How to run a command by key: its bound key, else by name.
    pub fn key_for(&self, command: &str) -> String {
        self.bound_to(command)
            .unwrap_or_else(|| format!("command {command}"))
    }

    /// The command line a key is bound to, if it is bound.
    pub(crate) fn command_for(&self, key: Key) -> Option<String> {
        let bound = self.bindings.iter().find(|(k, _)| *k == key);
        bound.map(|(_, line)| line.clone())
    }
}

impl Session {
    /// A session that starts by saying this.
    pub(crate) fn saying(message: String) -> Session {
        let session = Session::default();
        Session { message, ..session }
    }

    /// Why an item's text was not read as statements, if it was written
    /// in this session and was not.
    pub fn prose_hint(&self, item: u64) -> Option<&str> {
        let hint = self.outside.prose.as_ref().filter(|(i, ..)| *i == item);
        hint.map(|(_, why, _)| why.as_str())
    }

    /// The text APE would mend an item's text to, if its text was written
    /// in this session, was not read, and has a repair.
    pub fn prose_repair(&self, item: u64) -> Option<&str> {
        let hint = self.outside.prose.as_ref().filter(|(i, ..)| *i == item);
        hint.and_then(|(.., mended)| mended.as_deref())
    }

    /// Note why an item's text was not read, with its repair, if any; or
    /// that it was read.
    pub(crate) fn set_prose(&mut self, hint: Option<(u64, String, Option<String>)>) {
        self.outside.prose = hint;
    }

    /// Let an item's text, written by the entry `of`, wait to be read.
    pub(crate) fn wait_to_read(&mut self, item: u64, of: u64) {
        self.outside.to_read.insert(item, of);
    }

    /// The items that wait to be read, which then wait no more.
    pub(crate) fn take_to_read(&mut self) -> BTreeMap<u64, u64> {
        std::mem::take(&mut self.outside.to_read)
    }

    /// Whether any item's text waits to be read.
    pub fn waits_to_read(&self) -> bool {
        !self.outside.to_read.is_empty()
    }

    /// The quasimode in progress, if it lists over the outline.
    pub(crate) fn listed(&self) -> Option<&Active> {
        match &self.mode {
            Mode::Quasi(a) if a.lists() => Some(a),
            _ => None,
        }
    }

    /// Apply a view entry: what it shows, and what it records of a leap or
    /// of what reasoning showed.
    pub(crate) fn apply(&mut self, e: &Entry) {
        let Some(b) = e.body().filter(|_| e.kind() == "view") else {
            return;
        };
        self.view.restore(b);
        self.trail.apply(b);
        match b["op"].as_str() {
            Some("leap") => {
                if let Some((pattern, forward)) = leap_of(b) {
                    self.leapt(&pattern, forward);
                }
            }
            Some("find") => self.found = found_of(b).or(self.found.take()),
            Some("observed") => self.set_observed(observed_of(b)),
            _ => {}
        }
    }

    /// A leap for a pattern: leaping again finds it, and the list beside
    /// the outline lists its matches.
    pub(crate) fn leapt(&mut self, pattern: &str, forward: bool) {
        self.last_leap = Some((pattern.to_string(), forward));
        self.found = Some(Found::Text(pattern.to_string()));
    }

    /// Show what went wrong, if anything did.
    pub(crate) fn report<T>(&mut self, result: Result<T, String>) {
        if let Err(m) = result {
            self.message = m;
        }
    }

    /// Type a character at the text cursor into an item whose text, as
    /// shown, is this.
    pub(crate) fn type_into(&mut self, item: u64, mut text: Vec<char>, c: char) {
        let pos = self.view.pos.min(text.len());
        text.insert(pos, c);
        self.view.pos = pos + 1;
        self.hold_text(item, text);
    }

    /// Replace the characters typed before the text cursor, in an item
    /// whose text, as shown, is this, with others, the cursor after them.
    pub(crate) fn replace_before(&mut self, item: u64, mut text: Vec<char>, n: usize, with: &str) {
        let pos = self.view.pos.min(text.len());
        let start = pos.saturating_sub(n);
        text.splice(start..pos, with.chars());
        self.view.pos = start + with.chars().count();
        self.hold_text(item, text);
    }

    /// Erase the character before the text cursor, or the one after it, in
    /// an item whose text, as shown, is this; at the end, nothing.
    pub(crate) fn erase_in(&mut self, item: u64, mut text: Vec<char>, before: bool) {
        let pos = self.view.pos.min(text.len());
        if before && pos > 0 {
            text.remove(pos - 1);
            self.view.pos = pos - 1;
        } else if !before && pos < text.len() {
            text.remove(pos);
        } else {
            return;
        }
        self.hold_text(item, text);
    }

    /// Keep an item's new text until it is written.
    pub(crate) fn hold_text(&mut self, item: u64, text: Vec<char>) {
        self.pending_text = Some(PendingText { item, text });
        self.view_dirty = true;
    }

    /// The text typed into an item and not yet written, if any.
    pub(crate) fn typed(&self, item: u64) -> Option<&[char]> {
        let p = self.pending_text.as_ref().filter(|p| p.item == item);
        p.map(|p| p.text.as_slice())
    }

    /// The text typed and not yet written, to write it.
    pub(crate) fn take_text(&mut self) -> Option<PendingText> {
        self.pending_text.take()
    }

    /// Whether anything is typed or moved and not yet written.
    pub fn pending(&self) -> bool {
        self.pending_text.is_some() || self.view_dirty
    }

    /// Point the source check at another Crossref-like service (for tests).
    pub fn set_crossref(&mut self, base: &str) {
        self.outside.crossref = base.to_string();
    }

    /// Where the source check asks for a DOI's record.
    pub(crate) fn source_url(&self, doi: &str) -> String {
        format!(
            "{}{}",
            self.outside.crossref,
            crate::checks::encode_doi(doi)
        )
    }

    /// Results whose input files changed since: item -> those files.
    pub fn stale(&self) -> &BTreeMap<u64, Vec<String>> {
        &self.outside.stale
    }

    pub(crate) fn set_stale(&mut self, stale: BTreeMap<u64, Vec<String>>) {
        self.outside.stale = stale;
    }

    /// Whether what is shown now differs from what was last recorded: at
    /// the start, whether anything is shown at all.
    pub(crate) fn observed_changed(&self, now: &Observed) -> bool {
        match &self.outside.observed {
            Some(o) => o != now,
            None => !now.0.is_empty() || !now.1.is_empty(),
        }
    }

    /// What was last recorded as shown.
    pub(crate) fn set_observed(&mut self, now: Observed) {
        self.outside.observed = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_source_check_asks_crossref_unless_pointed_elsewhere() {
        let mut s = Session::default();
        let doi = "10.1000/a b";
        assert_eq!(
            s.source_url(doi),
            "https://api.crossref.org/works/10.1000/a%20b"
        );
        s.set_crossref("http://localhost:1/");
        assert_eq!(s.source_url(doi), "http://localhost:1/10.1000/a%20b");
    }
}
