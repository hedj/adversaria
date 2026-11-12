//! Replaying the journal into a store.
//!
//! Milestone 2 defines the store's facts. Until then the store is the list of
//! entries in journal order: (seq, parent, kind, body), body None if purged.

use crate::journal::Error;
use format::chain;
use format::record::JOURNAL;
use serde_json::Value;
use std::fs;
use std::path::Path;

pub type Fact = (u64, Option<u64>, String, Option<Value>);

/// Read and check the journal, then rebuild the store from it.
pub fn replay(dir: &Path) -> Result<Vec<Fact>, Error> {
    let text = fs::read_to_string(dir.join(JOURNAL))?;
    let entries = chain::check_journal(&text).map_err(Error::Format)?;
    Ok(entries
        .into_iter()
        .map(|e| (e.seq, e.parent, e.kind, e.body))
        .collect())
}
