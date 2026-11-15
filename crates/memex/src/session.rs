//! How a session of the memex is set up, and what it has noticed outside
//! the journal (docs/structure.md § The code's structure).

use crate::app::Key;
use std::collections::BTreeMap;
use std::path::Path;

/// The violations and the disagreements shown, by key.
pub type Observed = (Vec<String>, Vec<String>);

/// The room's name, the keys bound to commands, whether the user has
/// quit, and what the session has noticed outside the journal.
pub struct Session {
    pub workspace: String,
    /// Keys bound to command lines, from the configuration.
    pub bindings: Vec<(Key, String)>,
    pub quit: bool,
    outside: Outside,
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
}

/// A room is named after its journal's directory.
fn room_name(dir: &Path) -> String {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned());
    name.unwrap_or_default()
}

impl Outside {
    fn new() -> Outside {
        Outside {
            crossref: "https://api.crossref.org/works/".into(),
            stale: BTreeMap::new(),
            observed: None,
        }
    }
}

impl Session {
    /// A session in a journal directory, the room named after it, with the
    /// default keys.
    pub fn new(dir: &Path) -> Session {
        let workspace = room_name(dir);
        Session {
            workspace,
            bindings: crate::config::Config::default().bindings,
            quit: false,
            outside: Outside::new(),
        }
    }

    /// How to run a command by key: its bound key, else by name.
    pub fn key_for(&self, command: &str) -> String {
        match self.bindings.iter().find(|(_, c)| c == command) {
            Some((k, _)) => crate::config::key_name(*k),
            None => format!("command {command}"),
        }
    }

    /// The command line a key is bound to, if it is bound.
    pub(crate) fn command_for(&self, key: Key) -> Option<String> {
        let bound = self.bindings.iter().find(|(k, _)| *k == key);
        bound.map(|(_, line)| line.clone())
    }

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
