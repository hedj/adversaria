//! How a session of the memex is set up, and what it has noticed outside
//! the journal (docs/structure.md § The code's structure).

use crate::app::Key;
use std::collections::BTreeMap;

/// The violations and the disagreements shown, by key.
pub type Observed = (Vec<String>, Vec<String>);

/// The keys bound to commands, whether the user has quit, and what the
/// session has noticed outside the journal.
#[derive(Default)]
pub struct Session {
    pub keys: Keys,
    pub quit: bool,
    outside: Outside,
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
    /// Point the source check at another Crossref-like service (for tests).
    pub fn set_crossref(&mut self, base: &str) {
        self.outside.crossref = base.to_string();
    }

    /// Where the source check asks for a DOI's record.
    pub(crate) fn source_url(&self, doi: &str) -> String {
        format!("{}{}", self.outside.crossref, crate::app::encode_doi(doi))
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
