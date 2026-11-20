//! What a session of the memex holds and the journal does not: what the
//! user sees and is in the middle of, how the session is set up, and what
//! it has noticed outside the journal (docs/structure.md § The code's structure).

use crate::app::{Active, Click, Event, Key, Mode, PendingText, Press, View};
use crate::notebook::strings;
use crate::plumb::Offer;
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
    pub keys: Keys,
    /// The plumbing rules a right-click asks (crate::plumb).
    pub plumber: crate::plumb::Plumber,
    pub quit: bool,
    outside: Outside,
}

/// What a right-click offers (crate::plumb): the offers, the one chosen,
/// and the cell, column and row, where the menu opens.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    /// The text the menu acts on.
    pub text: String,
    pub offers: Vec<Offer>,
    pub chosen: usize,
    pub at: (u16, u16),
}

/// What the menu does with a key or a click.
pub enum Answer {
    Keep(Menu),
    Close,
    Run(String),
}

impl Menu {
    /// A menu of offers for a text, the first chosen, at a cell.
    pub fn new(text: &str, offers: Vec<Offer>, at: (u16, u16)) -> Menu {
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
    pub fn clicked(&self, c: &Click) -> Answer {
        let on = |o: &&Offer| Click::Command(o.line.clone()) == *c;
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

/// The pattern and direction of a leap entry.
fn leap_of(b: &Value) -> Option<(String, bool)> {
    let pattern = b["pattern"].as_str()?;
    Some((pattern.to_string(), b["forward"] != false))
}

/// The violations and the disagreements an observed entry records.
fn observed_of(b: &Value) -> Observed {
    (strings(&b["violations"]), strings(&b["disagreements"]))
}

/// Keys bound to command lines, from the configuration.
pub struct Keys(pub Vec<(Key, String)>);

/// What the session knows that the journal does not hold.
struct Outside {
    /// Where the source check asks Crossref: its works API, unless a test
    /// points it elsewhere.
    crossref: String,
    /// Results whose input files changed since: item -> those files.
    stale: BTreeMap<u64, Vec<String>>,
    /// The last violations and disagreements recorded in the journal.
    observed: Option<Observed>,
    /// Why the text of an item was not read, last time it was written.
    prose: Option<(u64, String)>,
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
        match self.0.iter().find(|(_, c)| c == command) {
            Some((k, _)) => crate::config::key_name(*k),
            None => format!("command {command}"),
        }
    }

    /// The command line a key is bound to, if it is bound.
    pub(crate) fn command_for(&self, key: Key) -> Option<String> {
        let bound = self.0.iter().find(|(k, _)| *k == key);
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
        let hint = self.outside.prose.as_ref().filter(|(i, _)| *i == item);
        hint.map(|(_, why)| why.as_str())
    }

    /// Note why an item's text was not read, or that it was.
    pub(crate) fn set_prose(&mut self, hint: Option<(u64, String)>) {
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
        match b["op"].as_str() {
            Some("leap") => self.last_leap = leap_of(b),
            Some("observed") => self.set_observed(observed_of(b)),
            _ => {}
        }
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
    fn hold_text(&mut self, item: u64, text: Vec<char>) {
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
