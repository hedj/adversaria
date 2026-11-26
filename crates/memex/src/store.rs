//! The store: the facts of one world, replayed from the journal.
//!
//! A world is the path from an entry (its head) back to the root. Along the
//! path, in order, an entry's body may carry `"facts": [term, ...]`; a
//! `retract` entry's body is `{"target": seq}`, naming an earlier entry on
//! the same path whose facts it removes. A purged entry's facts are gone.
//! (docs/design.md § The journal, § The journal format.)

use crate::prolog::{self, Prolog};
use format::record::{Entry, EntryKind};
use format::terms::{self, Term};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

/// The facts of the world whose head is `head`, in the order they were added,
/// each with the seq of the entry that added it.
pub fn facts(entries: &[Entry], head: u64) -> Result<Vec<(u64, Term)>, String> {
    let mut store = Store::default();
    store.move_to(entries, head)?;
    Ok(store.facts().map(|(seq, f)| (seq, f.clone())).collect())
}

/// The entry of this number, which sits at place seq - 1.
fn entry(entries: &[Entry], seq: u64) -> Result<&Entry, String> {
    let found = entries.get((seq as usize).wrapping_sub(1));
    let found = found.filter(|e| e.seq == seq);
    found.ok_or(format!("entry {seq} does not exist"))
}

/// The facts of one world, kept so that the next world asked for is
/// reached by the entries between the two, not replayed from the root:
/// a key adds an entry or two to a journal of thousands.
#[derive(Debug, Default)]
pub struct Store {
    /// The world's entries, from the root to its head.
    path: Vec<u64>,
    on_path: HashSet<u64>,
    retracted: HashSet<u64>,
    /// The facts that stand, by the entry that added them. A parent is
    /// an earlier entry (format::chain), so this order is the path's.
    facts: BTreeMap<u64, Vec<Term>>,
    /// How many entries the journal held when the store last moved.
    read: usize,
}

impl Store {
    /// Move to the world whose head is `head`. After an error the store
    /// is empty, so the next move replays from the root.
    pub fn move_to(&mut self, entries: &[Entry], head: u64) -> Result<(), String> {
        let moved = self.try_move(entries, head);
        if moved.is_err() {
            *self = Store::default();
        }
        moved
    }

    /// The facts that stand, in the order they were added, each with the
    /// seq of the entry that added it.
    pub fn facts(&self) -> impl Iterator<Item = (u64, &Term)> {
        let by_entry = self.facts.iter();
        by_entry.flat_map(|(seq, fs)| fs.iter().map(move |f| (*seq, f)))
    }

    /// The facts that stand, grouped by the entry that added them.
    pub fn by_entry(&self) -> impl Iterator<Item = (u64, &[Term])> {
        self.facts.iter().map(|(seq, fs)| (*seq, fs.as_slice()))
    }

    fn try_move(&mut self, entries: &[Entry], head: u64) -> Result<(), String> {
        if self.purged_since(entries) {
            *self = Store::default();
        }
        self.read = entries.len();
        for e in self.branch_to(entries, head)? {
            self.read_entry(entries, e)?;
        }
        Ok(())
    }

    /// Whether the journal purged an entry since the last move, which
    /// takes a body the store may hold. The journal only grows, but for a
    /// purge; one with fewer entries is taken as purged too.
    fn purged_since(&self, entries: &[Entry]) -> bool {
        let new = entries.get(self.read..);
        new.is_none_or(|new| new.iter().any(|e| e.kind() == EntryKind::Purge))
    }

    /// The entries of the world at `head` after the last that it shares
    /// with this one, root first. The store is unwound to that entry.
    fn branch_to<'a>(&mut self, entries: &'a [Entry], head: u64) -> Result<Vec<&'a Entry>, String> {
        let mut ahead = Vec::new();
        let mut at = Some(head);
        while let Some(seq) = at.filter(|s| !self.on_path.contains(s)) {
            ahead.push(entry(entries, seq)?);
            at = ahead.last().and_then(|e| e.parent_seq());
        }
        self.unwind_to(entries, at)?;
        ahead.reverse();
        Ok(ahead)
    }

    /// Unwind the store to the world whose head is `shared`, last entry
    /// first; to no world if none.
    fn unwind_to(&mut self, entries: &[Entry], shared: Option<u64>) -> Result<(), String> {
        while self.path.last().copied() != shared {
            let Some(seq) = self.path.pop() else { break };
            self.on_path.remove(&seq);
            self.unread(entries, seq)?;
        }
        Ok(())
    }

    /// Take an entry's facts out, and give back those it retracted.
    fn unread(&mut self, entries: &[Entry], seq: u64) -> Result<(), String> {
        self.facts.remove(&seq);
        let Some(t) = retract_target(entry(entries, seq)?) else {
            return Ok(());
        };
        self.retracted.remove(&t);
        let target = entry(entries, t)?.body();
        target.map_or(Ok(()), |body| self.add(t, body))
    }

    /// Read the next entry on the path; an error names the entry.
    fn read_entry(&mut self, entries: &[Entry], e: &Entry) -> Result<(), String> {
        self.path.push(e.seq);
        self.on_path.insert(e.seq);
        let Some(body) = e.body() else { return Ok(()) };
        let done = self.apply(entries, e, body);
        done.map_err(|m| format!("entry {}: {m}", e.seq))
    }

    /// What an entry's body does to the facts: a retract removes those of
    /// its target, and any entry may add its own.
    fn apply(&mut self, entries: &[Entry], e: &Entry, body: &Value) -> Result<(), String> {
        if e.kind() == EntryKind::Retract {
            self.retract(entries, e.seq, body)?;
        }
        self.add(e.seq, body)
    }

    fn retract(&mut self, entries: &[Entry], seq: u64, body: &Value) -> Result<(), String> {
        let target = body.get("target").and_then(Value::as_u64);
        let t = target.ok_or("a retract needs a target")?;
        self.check_target(entries, seq, t)?;
        self.facts.remove(&t);
        Ok(())
    }

    /// A retract names an entry earlier in this world, which is no
    /// retraction and is not yet retracted.
    fn check_target(&mut self, entries: &[Entry], seq: u64, t: u64) -> Result<(), String> {
        if !self.on_path.contains(&t) || t == seq {
            return Err(format!(
                "retracts entry {t}, which is not earlier in this world"
            ));
        }
        if entry(entries, t)?.kind() == EntryKind::Retract {
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
        let list = list.as_array().ok_or("facts must be a list")?;
        let fs = list
            .iter()
            .map(fact)
            .collect::<Result<Vec<Term>, String>>()?;
        if !fs.is_empty() {
            self.facts.insert(seq, fs);
        }
        Ok(())
    }
}

/// The entry a retraction names, if this entry is one that names one.
fn retract_target(e: &Entry) -> Option<u64> {
    let retracts = e.kind() == EntryKind::Retract;
    retracts
        .then(|| e.body()?.get("target")?.as_u64())
        .flatten()
}

/// A fact as the journal encodes it: an atom or a compound term.
fn fact(encoded: &Value) -> Result<Term, String> {
    let t = terms::decode(encoded).map_err(|m| m.to_string())?;
    if !matches!(t, Term::Atom(_) | Term::Compound(..)) {
        return Err("a fact must be an atom or a compound".into());
    }
    Ok(boxed(t))
}

/// A term with each box as it is now stored, box(Name, Value). Until 22
/// November 2026 a box was stored as cell(Name, Value), the name the grid
/// mark has too (docs/design.md, version 1 revised; docs/concepts.md,
/// decision 7). A layer's mark is left as written: there cell is the mark.
fn boxed(t: Term) -> Term {
    match t {
        Term::Compound(f, mut a) if f == "layer" && a.len() == 2 => {
            let goal = boxed(a.pop().expect("two arguments"));
            Term::Compound(f, vec![a.pop().expect("two arguments"), goal])
        }
        Term::Compound(f, a) => {
            let a: Vec<Term> = a.into_iter().map(boxed).collect();
            let f = if f == "cell" && a.len() == 2 {
                "box".into()
            } else {
                f
            };
            Term::Compound(f, a)
        }
        t => t,
    }
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

    fn c(f: &str, args: Vec<Term>) -> Term {
        Term::Compound(f.into(), args)
    }

    #[test]
    fn a_box_an_old_journal_stored_as_cell_is_read_as_box() {
        // Catches a journal written before the rename losing its boxes, or
        // a constraint over them, or the grid mark of a layer renamed too.
        let cell = |n: &str, v: Term| c("cell", vec![Term::Atom(n.into()), v]);
        let boxed_rent = c("box", vec![Term::Atom("rent".into()), Term::Int(1200)]);
        let stored = terms::encode(&cell("rent", Term::Int(1200))).unwrap();
        assert_eq!(fact(&stored).unwrap(), boxed_rent, "as the journal is read");
        let rule = c(":-", vec![c("false", vec![]), cell("rent", Term::Var(0))]);
        let read = boxed(rule);
        assert_eq!(
            read,
            c(
                ":-",
                vec![
                    c("false", vec![]),
                    c("box", vec![Term::Atom("rent".into()), Term::Var(0)])
                ]
            )
        );
        let mark = c(
            "cell",
            vec![c("x", vec![Term::Var(0)]), c("y", vec![Term::Var(1)])],
        );
        let layer = c("layer", vec![mark.clone(), cell("rent", Term::Var(1))]);
        let goal = c("box", vec![Term::Atom("rent".into()), Term::Var(1)]);
        assert_eq!(boxed(layer), c("layer", vec![mark, goal]));
        let three = c("cell", vec![Term::Int(1), Term::Int(2), Term::Int(3)]);
        assert_eq!(boxed(three.clone()), three, "only cell/2 was a box");
    }

    fn date(e: &str, y: i64) -> Value {
        terms::encode(&Term::Compound(
            "date".into(),
            vec![Term::Atom(e.into()), Term::Int(y)],
        ))
        .unwrap()
    }

    /// Entries from (parent, kind, body), numbered from 1.
    fn journal(spec: &[(Option<u64>, EntryKind, Value)]) -> Vec<Entry> {
        let mut es: Vec<Entry> = Vec::new();
        for row in spec {
            let entry = next_entry(&es, row);
            es.push(entry);
        }
        es
    }

    /// The entry a row makes after these, numbered after the last.
    fn next_entry(es: &[Entry], row: &(Option<u64>, EntryKind, Value)) -> Entry {
        let (_, kind, body) = row;
        let parent = parent_of(es, row);
        Entry::new(es.len() as u64 + 1, parent, *kind, body.clone()).unwrap()
    }

    /// The parent a row names, of these entries, as Entry::new names it.
    fn parent_of<'a>(
        es: &'a [Entry],
        row: &(Option<u64>, EntryKind, Value),
    ) -> Option<(u64, &'a str)> {
        let e = &es[row.0? as usize - 1];
        Some((e.seq, e.hash.as_str()))
    }

    #[test]
    fn worlds_follow_the_branch() {
        // Catches facts taken from another branch, or out of order.
        let es = journal(&[
            (
                None,
                EntryKind::Edit,
                json!({"facts": [date("buran", 1988)]}),
            ),
            (
                Some(1),
                EntryKind::Edit,
                json!({"facts": [date("energia", 1987)]}),
            ),
            (
                Some(1),
                EntryKind::Suppose,
                json!({"facts": [date("talk", 1989)]}),
            ),
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
            (
                None,
                EntryKind::Edit,
                json!({"facts": [date("a", 1), date("b", 2)]}),
            ),
            (Some(1), EntryKind::Adopt, json!({"facts": [date("c", 3)]})),
            (Some(2), EntryKind::Retract, json!({"target": 1})),
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
                (None, EntryKind::Edit, json!({"facts": [date("a", 1)]})),
                (Some(1), EntryKind::Edit, json!({})),
            ]
        };
        let cases: Vec<(Option<u64>, EntryKind, Value)> = vec![
            (Some(1), EntryKind::Retract, json!({"target": 2})), // 2 is on another branch
            (Some(2), EntryKind::Retract, json!({"target": 3})), // itself
            (Some(2), EntryKind::Retract, json!({})),            // no target
        ];
        for c in cases {
            let mut spec = base();
            spec.push(c);
            assert!(facts(&journal(&spec), 3).is_err());
        }
        let mut spec = base();
        spec.push((Some(2), EntryKind::Retract, json!({"target": 1})));
        spec.push((Some(3), EntryKind::Retract, json!({"target": 3})));
        assert!(facts(&journal(&spec), 4).is_err());
        let mut spec = base();
        spec.push((Some(2), EntryKind::Retract, json!({"target": 1})));
        spec.push((Some(3), EntryKind::Retract, json!({"target": 1})));
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
                facts(&journal(&[(None, EntryKind::Edit, bad.clone())]), 1).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn purged_facts_are_gone() {
        // Catches facts of a purged entry still in the store.
        let mut es = journal(&[
            (None, EntryKind::Edit, json!({"facts": [date("a", 1)]})),
            (Some(1), EntryKind::Edit, json!({})),
        ]);
        es[0] = es[0].purged();
        assert!(facts(&es, 2).unwrap().is_empty());
    }

    /// The facts a store holds, as `facts` gives them.
    fn held(store: &Store) -> Vec<(u64, Term)> {
        store.facts().map(|(seq, f)| (seq, f.clone())).collect()
    }

    #[test]
    fn a_store_moved_between_worlds_holds_what_a_replay_from_the_root_holds() {
        // Catches a move that keeps facts of the world it left, or does not
        // give back the facts that a retraction it unwound had taken.
        let es = journal(&[
            (None, EntryKind::Edit, json!({"facts": [date("a", 1)]})),
            (
                Some(1),
                EntryKind::Edit,
                json!({"facts": [date("b", 2), date("c", 3)]}),
            ),
            (Some(2), EntryKind::Retract, json!({"target": 1})),
            (Some(3), EntryKind::Adopt, json!({"facts": [date("d", 4)]})),
            (
                Some(2),
                EntryKind::Suppose,
                json!({"facts": [date("e", 5)]}),
            ),
            (Some(5), EntryKind::Retract, json!({"target": 2})),
            (Some(1), EntryKind::Edit, json!({})),
            (
                Some(7),
                EntryKind::Retract,
                json!({"target": 1, "facts": [date("f", 6)]}),
            ),
        ]);
        let mut store = Store::default();
        for head in [4, 6, 3, 8, 1, 4, 4, 7, 2, 6, 5, 8, 3] {
            store.move_to(&es, head).unwrap();
            assert_eq!(held(&store), facts(&es, head).unwrap(), "at {head}");
        }
    }

    #[test]
    fn a_store_that_met_an_error_replays_from_the_root() {
        // Catches a store left half moved by an entry it refused: unwinding
        // the refused second retraction of entry 1 gave entry 1 back.
        let es = journal(&[
            (None, EntryKind::Edit, json!({"facts": [date("a", 1)]})),
            (Some(1), EntryKind::Retract, json!({"target": 1})),
            (Some(2), EntryKind::Retract, json!({"target": 1})),
        ]);
        let mut store = Store::default();
        assert!(store.move_to(&es, 3).is_err());
        store.move_to(&es, 2).unwrap();
        assert_eq!(held(&store), vec![]);
    }

    #[test]
    fn a_store_forgets_the_facts_of_an_entry_purged_since_it_moved() {
        // Catches a store that keeps facts it read before their entry was
        // purged.
        let mut es = journal(&[
            (None, EntryKind::Edit, json!({"facts": [date("a", 1)]})),
            (Some(1), EntryKind::Edit, json!({"facts": [date("b", 2)]})),
        ]);
        let mut store = Store::default();
        store.move_to(&es, 2).unwrap();
        es[0] = es[0].purged();
        let purge = (
            Some(2),
            EntryKind::Purge,
            json!({"target": 1, "reason": "a test"}),
        );
        es.push(next_entry(&es, &purge));
        store.move_to(&es, 3).unwrap();
        assert_eq!(held(&store), facts(&es, 3).unwrap());
        assert_eq!(held(&store).len(), 1);
    }

    #[test]
    fn an_error_names_its_entry() {
        // Catches a fault in a world reported without the entry it is in.
        let es = journal(&[
            (None, EntryKind::Edit, json!({})),
            (Some(1), EntryKind::Retract, json!({})),
        ]);
        assert_eq!(
            facts(&es, 2),
            Err("entry 2: a retract needs a target".into())
        );
    }

    #[test]
    fn an_entry_out_of_its_place_is_refused() {
        // Catches a lookup that takes whatever entry sits at a place as the
        // entry of that number: a list holding only entry 2, whose parent is
        // 1, found entry 2 again as its own parent and looped for ever.
        let es = journal(&[
            (None, EntryKind::Edit, json!({"facts": [date("a", 1)]})),
            (Some(1), EntryKind::Edit, json!({"facts": [date("b", 2)]})),
        ]);
        let only_second = &es[1..];
        let m = facts(only_second, 1).unwrap_err();
        assert_eq!(m, "entry 1 does not exist");
    }
}
