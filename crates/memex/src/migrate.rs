//! Migrating a journal of an earlier version to this one (docs/design.md,
//! The journal format, Versions). Each entry is written again, its body
//! converted where this version says it otherwise, so every hash changes;
//! parents stay, by number. Entry 1 names the version and the hash of the
//! last entry of the journal it came from, which that journal's seals,
//! kept beside it, still seal. What version 2 records and version 1 did
//! not is inferred from version 1's entries (crate::inferred). An offer of
//! version 1 or 2 is marked as drawn: it was recorded once drawn in an
//! open pane, and those never drawn were not, where version 3 records
//! each suggestion made.

use crate::inferred::Inferred;
use format::bodies::{Body, View};
use format::record::Entry;
use serde_json::{Value, json};
use std::collections::HashMap;

/// A journal's entries of an earlier version, as this one writes them.
pub(crate) fn migrated(entries: &[Entry], from: u64) -> Result<Vec<Entry>, String> {
    if entries.first().is_some_and(|e| e.body().is_none()) {
        return Err("entry 1 is purged, so it cannot name the journal's version".into());
    }
    let mut rewrite = Rewrite::of(entries, from);
    entries.iter().map(|e| rewrite.entry(e)).collect()
}

/// A journal being written again: the version it was of and the hash of
/// its last entry; for version 1, what version 2 records of its entries
/// that version 1 did not; and each entry's new hash, by its number.
struct Rewrite {
    from: (u64, Option<String>),
    inferred: Option<Inferred>,
    hashes: HashMap<u64, String>,
}

impl Rewrite {
    /// The rewriting of these entries, of a version, none written yet.
    fn of(entries: &[Entry], version: u64) -> Rewrite {
        let from = (version, entries.last().map(|e| e.hash.clone()));
        let inferred = (version == 1).then(|| Inferred::of(entries));
        let hashes = HashMap::new();
        Rewrite {
            from,
            inferred,
            hashes,
        }
    }

    /// An entry written again, its new hash kept for its children.
    fn entry(&mut self, e: &Entry) -> Result<Entry, String> {
        let new = self.rewritten(e)?;
        self.hashes.insert(e.seq, new.hash.clone());
        Ok(new)
    }

    /// An entry after its parent's new hash: its body converted, if
    /// version 2 says it otherwise, else its content as it was.
    fn rewritten(&self, e: &Entry) -> Result<Entry, String> {
        let parent = e.parent_seq().map(|p| (p, self.hashes[&p].as_str()));
        match self.converted(e) {
            Some(b) => Entry::new(e.seq, parent, e.kind(), b),
            None => e.with_parent(parent),
        }
    }
}

impl Rewrite {
    /// An entry's body as this version writes it, if that differs: the
    /// first entry names the journal it came from, by its version and
    /// last hash; from version 1, what version 2 converts; an offer,
    /// recorded only once drawn, is marked so. A purged entry has no body
    /// to convert, and stays purged.
    fn converted(&self, e: &Entry) -> Option<Value> {
        let mut body = e.body()?.clone();
        if e.seq == 1 {
            let (version, last) = &self.from;
            body["migrated_from"] = json!({"version": version, "last": last});
            body["version"] = json!(format::record::JOURNAL_VERSION);
        }
        if let Some(inferred) = &self.inferred {
            to_v2(e, inferred, &mut body);
        }
        if matches!(Body::read(e.kind(), &body), Ok(Body::View(View::Offer(_)))) {
            body["drawn"] = json!(true);
        }
        (Some(&body) != e.body()).then_some(body)
    }
}

/// An entry's body of version 1 as version 2 writes it: a session's start
/// drops the schema, which the header now declares; what the memex wrote
/// on its own is marked so; a view that only records where writing left
/// the cursor is `written`.
fn to_v2(e: &Entry, inferred: &Inferred, body: &mut Value) {
    if matches!(Body::read(e.kind(), body), Ok(Body::View(View::Start(_)))) {
        body.as_object_mut().map(|b| b.remove("schema"));
    }
    inferred.mark(e.seq, body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use format::record::EntryKind;

    /// Entries of these kinds and bodies, each after the one before.
    fn chain(bodies: &[(EntryKind, Value)]) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        for (kind, body) in bodies {
            out.push(next(&out, *kind, body.clone()));
        }
        out
    }

    /// The entry of a kind and a body after these, numbered after the last.
    fn next(es: &[Entry], kind: EntryKind, body: Value) -> Entry {
        Entry::new(es.len() as u64 + 1, after(es), kind, body).unwrap()
    }

    /// The last entry, as Entry::new names a parent.
    fn after(es: &[Entry]) -> Option<(u64, &str)> {
        es.last().map(|e| (e.seq, e.hash.as_str()))
    }

    #[test]
    fn a_start_loses_its_schema_and_a_purged_entry_stays_purged() {
        // Catches a schema left in a start entry, which the header now
        // declares; a purged entry refused, or given a body; or another
        // entry's body changed.
        let mut old = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            (
                EntryKind::View,
                json!({"op": "start", "schema": 2, "build": "abc", "by": "memex"}),
            ),
            (
                EntryKind::Edit,
                json!({"op": "create", "under": 0, "after": null, "add": "x", "spans": []}),
            ),
        ]);
        old[2] = old[2].purged();
        let new = migrated(&old, 1).unwrap();
        let start = new[1].body().unwrap();
        assert_eq!(
            start,
            &json!({"op": "start", "build": "abc", "by": "memex"})
        );
        assert_eq!(new[2].body(), None, "still purged");
        assert_eq!(new[2].content.hash, old[2].content.hash, "the same content");
        assert_ne!(new[2].hash, old[2].hash, "after a parent hashed anew");
    }

    #[test]
    fn a_journal_whose_entry_1_is_purged_is_not_migrated() {
        // Catches a journal migrated with entry 1 purged: it could not name
        // its version, so version 3 would refuse it, and it would not open.
        let mut old = chain(&[(EntryKind::Edit, json!({"op": "init"}))]);
        old[0] = old[0].purged();
        let refused = migrated(&old, 2).unwrap_err();
        assert!(refused.contains("entry 1 is purged"), "{refused}");
    }
}
