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

/// The facts of the world whose head is `head`, in the order they were added,
/// each with the seq of the entry that added it.
pub fn facts(entries: &[Entry], head: u64) -> Result<Vec<(u64, Term)>, String> {
    let get = |s: u64| {
        entries
            .get((s as usize).wrapping_sub(1))
            .ok_or(format!("entry {s} does not exist"))
    };
    let mut path = Vec::new();
    let mut at = Some(head);
    while let Some(s) = at {
        let e = get(s)?;
        path.push(e);
        at = e.parent;
    }
    path.reverse();
    let mut on_path = std::collections::HashSet::new();
    let mut out: Vec<(u64, Term)> = Vec::new();
    let mut retracted = std::collections::HashSet::new();
    for e in path {
        on_path.insert(e.seq);
        let Some(body) = &e.body else { continue };
        if e.kind == "retract" {
            let t = body
                .get("target")
                .and_then(|t| t.as_u64())
                .ok_or(format!("entry {}: a retract needs a target", e.seq))?;
            if !on_path.contains(&t) || t == e.seq {
                return Err(format!(
                    "entry {}: retracts entry {t}, which is not earlier in this world",
                    e.seq
                ));
            }
            if get(t)?.kind == "retract" {
                return Err(format!(
                    "entry {}: retracts a retraction; adopt the facts again instead",
                    e.seq
                ));
            }
            if !retracted.insert(t) {
                return Err(format!("entry {}: entry {t} is already retracted", e.seq));
            }
            out.retain(|(s, _)| *s != t);
        }
        if let Some(fs) = body.get("facts") {
            let list = fs
                .as_array()
                .ok_or(format!("entry {}: facts must be a list", e.seq))?;
            for f in list {
                let t = terms::decode(f).map_err(|m| format!("entry {}: {m}", e.seq))?;
                if !matches!(t, Term::Atom(_) | Term::Compound(..)) {
                    return Err(format!(
                        "entry {}: a fact must be an atom or a compound",
                        e.seq
                    ));
                }
                out.push((e.seq, t));
            }
        }
    }
    Ok(out)
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
        for (i, (p, k, b)) in spec.iter().enumerate() {
            let parent = p.map(|p| (p, es[p as usize - 1].hash.clone()));
            es.push(
                Entry::new(
                    i as u64 + 1,
                    parent.as_ref().map(|(p, h)| (*p, h.as_str())),
                    k,
                    b.clone(),
                )
                .unwrap(),
            );
        }
        es
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
}
