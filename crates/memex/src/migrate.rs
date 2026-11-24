//! Migrating a journal of version 1 to version 2 (docs/design.md, The
//! journal format, Versions). Each entry is written again, its body
//! converted where version 2 says it otherwise, so every hash changes;
//! parents stay, by number. Entry 1 names the hash of the last entry of
//! the journal it came from, which that journal's seals, kept beside it,
//! still seal. What version 2 records and version 1 did not is inferred
//! from version 1's entries (crate::inferred).

use crate::inferred::Inferred;
use format::record::Entry;
use serde_json::{Value, json};
use std::collections::HashMap;

/// A journal's entries of version 1, as version 2 writes them.
pub(crate) fn from_v1(entries: &[Entry]) -> Result<Vec<Entry>, String> {
    let mut rewrite = Rewrite::of(entries);
    entries.iter().map(|e| rewrite.entry(e)).collect()
}

/// A journal being written again: the hash of the old journal's last
/// entry, what version 2 records of its entries that version 1 did not,
/// and each entry's new hash, by its number.
struct Rewrite {
    last: Option<String>,
    inferred: Inferred,
    hashes: HashMap<u64, String>,
}

impl Rewrite {
    /// The rewriting of these entries, none written yet.
    fn of(entries: &[Entry]) -> Rewrite {
        let last = entries.last().map(|e| e.hash.clone());
        let inferred = Inferred::of(entries);
        let hashes = HashMap::new();
        Rewrite {
            last,
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
    /// An entry's body as version 2 writes it, if that differs: the first
    /// entry names the journal it came from, by its last hash; a
    /// session's start drops the schema, which the header now declares;
    /// what the memex wrote on its own is marked so; a view that only
    /// records where writing left the cursor is `written`. A purged entry
    /// has no body to convert, and stays purged.
    fn converted(&self, e: &Entry) -> Option<Value> {
        let mut body = e.body()?.clone();
        if e.seq == 1 {
            body["migrated_from"] = json!({"version": 1, "last": self.last});
        }
        if e.kind() == "view" && body["op"] == "start" {
            body.as_object_mut().map(|b| b.remove("schema"));
        }
        self.inferred.mark(e.seq, &mut body);
        (Some(&body) != e.body()).then_some(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Entries of these kinds and bodies, each after the one before.
    fn chain(bodies: &[(&str, Value)]) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        for (kind, body) in bodies {
            out.push(next(&out, kind, body.clone()));
        }
        out
    }

    /// The entry of a kind and a body after these, numbered after the last.
    fn next(es: &[Entry], kind: &str, body: Value) -> Entry {
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
            ("edit", json!({"op": "init"})),
            (
                "view",
                json!({"op": "start", "schema": 2, "build": "abc", "by": "memex"}),
            ),
            (
                "edit",
                json!({"op": "create", "under": 0, "after": null, "add": "x", "spans": []}),
            ),
        ]);
        old[2] = old[2].purged();
        let new = from_v1(&old).unwrap();
        let start = new[1].body().unwrap();
        assert_eq!(
            start,
            &json!({"op": "start", "build": "abc", "by": "memex"})
        );
        assert_eq!(new[2].body(), None, "still purged");
        assert_eq!(new[2].content.hash, old[2].content.hash, "the same content");
        assert_ne!(new[2].hash, old[2].hash, "after a parent hashed anew");
    }
}
