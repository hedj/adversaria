//! The store: the facts of one world, replayed from the journal.
//!
//! A world is the path from an entry (its head) back to the root. Along the
//! path, in order, an entry's body may carry `"facts": [term, ...]`; a
//! `retract` entry's body is `{"target": seq}`, naming an earlier entry on
//! the same path whose facts it removes. A purged entry's facts are gone.
//! (docs/design.md § The journal, § The journal format.)

use crate::prolog::{self, Prolog};
use format::record::Entry;
use format::terms::{self, Term};
use serde_json::Value;
use std::collections::HashSet;

/// The facts of the world whose head is `head`, in the order they were added,
/// each with the seq of the entry that added it.
pub fn facts(entries: &[Entry], head: u64) -> Result<Vec<(u64, Term)>, String> {
    let mut world = Replay::new(entries);
    for e in path(entries, head)? {
        world.read(e)?;
    }
    Ok(world.facts)
}

fn entry(entries: &[Entry], seq: u64) -> Result<&Entry, String> {
    let found = entries.get((seq as usize).wrapping_sub(1));
    found.ok_or(format!("entry {seq} does not exist"))
}

/// The entries of a world, from the root to its head.
fn path(entries: &[Entry], head: u64) -> Result<Vec<&Entry>, String> {
    let mut path = Vec::new();
    let mut at = Some(head);
    while let Some(seq) = at {
        path.push(entry(entries, seq)?);
        at = path.last().and_then(|e| e.parent_seq());
    }
    path.reverse();
    Ok(path)
}

/// A world being replayed: the entries read so far on its path, those
/// retracted, and the facts that stand.
struct Replay<'a> {
    entries: &'a [Entry],
    on_path: HashSet<u64>,
    retracted: HashSet<u64>,
    facts: Vec<(u64, Term)>,
}

impl<'a> Replay<'a> {
    fn new(entries: &'a [Entry]) -> Self {
        let (on_path, retracted, facts) = (HashSet::new(), HashSet::new(), Vec::new());
        Replay {
            entries,
            on_path,
            retracted,
            facts,
        }
    }

    /// Read the next entry on the path; an error names the entry.
    fn read(&mut self, e: &Entry) -> Result<(), String> {
        self.on_path.insert(e.seq);
        let Some(body) = e.body() else { return Ok(()) };
        let done = self.apply(e, body);
        done.map_err(|m| format!("entry {}: {m}", e.seq))
    }

    /// What an entry's body does to the facts: a retract removes those of
    /// its target, and any entry may add its own.
    fn apply(&mut self, e: &Entry, body: &Value) -> Result<(), String> {
        if e.kind() == "retract" {
            self.retract(e.seq, body)?;
        }
        self.add(e.seq, body)
    }

    fn retract(&mut self, seq: u64, body: &Value) -> Result<(), String> {
        let target = body.get("target").and_then(Value::as_u64);
        let t = target.ok_or("a retract needs a target")?;
        self.check_target(seq, t)?;
        self.facts.retain(|(s, _)| *s != t);
        Ok(())
    }

    /// A retract names an entry earlier in this world, which is no
    /// retraction and is not yet retracted.
    fn check_target(&mut self, seq: u64, t: u64) -> Result<(), String> {
        if !self.on_path.contains(&t) || t == seq {
            return Err(format!(
                "retracts entry {t}, which is not earlier in this world"
            ));
        }
        if entry(self.entries, t)?.kind() == "retract" {
            return Err("retracts a retraction; adopt the facts again instead".into());
        }
        if !self.retracted.insert(t) {
            return Err(format!("entry {t} is already retracted"));
        }
        Ok(())
    }

    /// Add the facts a body carries, if it carries any.
    fn add(&mut self, seq: u64, body: &Value) -> Result<(), String> {
        let Some(list) = body.get("facts") else {
            return Ok(());
        };
        for f in list.as_array().ok_or("facts must be a list")? {
            self.facts.push((seq, fact(f)?));
        }
        Ok(())
    }
}

/// A fact as the journal encodes it: an atom or a compound term.
fn fact(encoded: &Value) -> Result<Term, String> {
    let t = terms::decode(encoded).map_err(|m| m.to_string())?;
    if !matches!(t, Term::Atom(_) | Term::Compound(..)) {
        return Err("a fact must be an atom or a compound".into());
    }
    Ok(t)
}

/// Load a world's facts into Prolog, in order.
pub fn load(p: &mut Prolog, facts: &[(u64, Term)]) -> Result<(), prolog::Error> {
    for (seq, f) in facts {
        p.assert_fact(f, *seq)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn date(e: &str, y: i64) -> Value {
        terms::encode(&Term::Compound(
            "date".into(),
            vec![Term::Atom(e.into()), Term::Int(y)],
        ))
        .unwrap()
    }

    /// Entries from (parent, kind, body), numbered from 1.
    fn journal(spec: &[(Option<u64>, &str, Value)]) -> Vec<Entry> {
        let mut es: Vec<Entry> = Vec::new();
        for (row, seq) in spec.iter().zip(1..) {
            let parent = parent_of(&es, row);
            let (_, kind, body) = row;
            es.push(Entry::new(seq, parent, kind, body.clone()).unwrap());
        }
        es
    }

    /// The parent a row names, of these entries, as Entry::new names it.
    fn parent_of<'a>(es: &'a [Entry], row: &(Option<u64>, &str, Value)) -> Option<(u64, &'a str)> {
        let e = &es[row.0? as usize - 1];
        Some((e.seq, e.hash.as_str()))
    }

    #[test]
    fn worlds_follow_the_branch() {
        // Catches facts taken from another branch, or out of order.
        let es = journal(&[
            (None, "edit", json!({"facts": [date("buran", 1988)]})),
            (Some(1), "edit", json!({"facts": [date("energia", 1987)]})),
            (Some(1), "suppose", json!({"facts": [date("talk", 1989)]})),
        ]);
        let seqs = |h| {
            facts(&es, h)
                .unwrap()
                .into_iter()
                .map(|(s, _)| s)
                .collect::<Vec<_>>()
        };
        assert_eq!(seqs(2), vec![1, 2]);
        assert_eq!(seqs(3), vec![1, 3]);
        assert_eq!(seqs(1), vec![1]);
    }

    #[test]
    fn retraction_removes_only_its_target() {
        // Catches a retraction that removes the wrong facts, or none.
        let es = journal(&[
            (None, "edit", json!({"facts": [date("a", 1), date("b", 2)]})),
            (Some(1), "adopt", json!({"facts": [date("c", 3)]})),
            (Some(2), "retract", json!({"target": 1})),
        ]);
        let f = facts(&es, 3).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].0, 2);
        assert_eq!(facts(&es, 2).unwrap().len(), 3);
    }

    #[test]
    fn bad_retractions_refused() {
        // Catches a retraction of another branch, of itself, of a retraction,
        // or a second one of the same entry, accepted.
        let base = || {
            vec![
                (None, "edit", json!({"facts": [date("a", 1)]})),
                (Some(1), "edit", json!({})),
            ]
        };
        let cases: Vec<(Option<u64>, &str, Value)> = vec![
            (Some(1), "retract", json!({"target": 2})), // 2 is on another branch
            (Some(2), "retract", json!({"target": 3})), // itself
            (Some(2), "retract", json!({})),            // no target
        ];
        for c in cases {
            let mut spec = base();
            spec.push(c);
            assert!(facts(&journal(&spec), 3).is_err());
        }
        let mut spec = base();
        spec.push((Some(2), "retract", json!({"target": 1})));
        spec.push((Some(3), "retract", json!({"target": 3})));
        assert!(facts(&journal(&spec), 4).is_err());
        let mut spec = base();
        spec.push((Some(2), "retract", json!({"target": 1})));
        spec.push((Some(3), "retract", json!({"target": 1})));
        assert!(facts(&journal(&spec), 4).is_err());
    }

    #[test]
    fn malformed_facts_refused() {
        // Catches a fact that is not a term, or not callable, accepted.
        for bad in [
            json!({"facts": 1}),
            json!({"facts": [1.5]}),
            json!({"facts": [7]}),
            json!({"facts": ["text"]}),
        ] {
            assert!(
                facts(&journal(&[(None, "edit", bad.clone())]), 1).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn purged_facts_are_gone() {
        // Catches facts of a purged entry still in the store.
        let mut es = journal(&[
            (None, "edit", json!({"facts": [date("a", 1)]})),
            (Some(1), "edit", json!({})),
        ]);
        es[0] = es[0].purged();
        assert!(facts(&es, 2).unwrap().is_empty());
    }

    #[test]
    fn an_error_names_its_entry() {
        // Catches a fault in a world reported without the entry it is in.
        let es = journal(&[(None, "edit", json!({})), (Some(1), "retract", json!({}))]);
        assert_eq!(
            facts(&es, 2),
            Err("entry 2: a retract needs a target".into())
        );
    }
}
