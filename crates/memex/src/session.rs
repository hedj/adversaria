//! What a session of the memex holds and the journal does not: what the
//! user sees and is in the middle of, how the session is set up, and what
//! it has noticed outside the journal (docs/structure.md § The code's structure).

use crate::app::{Active, Key, Mode, PendingText, View};
use crate::notebook::strings;
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
    pub quit: bool,
    outside: Outside,
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
}

impl Default for Outside {
    fn default() -> Outside {
        Outside {
            crossref: "https://api.crossref.org/works/".into(),
            stale: BTreeMap::new(),
            observed: None,
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
