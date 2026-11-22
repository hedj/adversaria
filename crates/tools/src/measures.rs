//! The measures report (docs/plan.md, Milestone 7), from the journal alone.
//!
//! Every entry is one user action or one observation the memex recorded.
//! Actions by kind, after Licklider's count of his own work:
//! * writing: edit (create, text, move, delete, facts, derive, constraint), link, name;
//! * searching: view leap;
//! * following links: view follow;
//! * navigating: view move, fold, pane, scope;
//! * deciding: adopt, retract, suppose, undo, redo, purge, view dismiss and world;
//! * checking: view interrogate, and facts that are rulings;
//! * importing: import; exporting: view export.
//!
//! Clerical work is searching, following links and navigating.
//! Observations (not actions): view observed (the violations and
//! disagreements shown) and view offer (suggestions shown).
//! A suggestion is adopted by an entry that carries "suggestion"; it is
//! later retracted if a retract entry names that entry.
//! A violation is corrected if a later observation in the actual world no
//! longer shows it.

use format::record::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const KINDS: [&str; 8] = [
    "writing",
    "searching",
    "following links",
    "navigating",
    "deciding",
    "checking",
    "importing",
    "exporting",
];

#[derive(Debug, Default, PartialEq)]
pub struct Report {
    pub entries: usize,
    pub actions: BTreeMap<&'static str, usize>,
    pub observations: usize,
    /// Entries that are neither: the memex's first entry, or a view the
    /// report does not know. Counted, not hidden.
    pub unclassified: usize,
    pub offered: usize,
    pub adopted: usize,
    pub dismissed: usize,
    pub adopted_then_retracted: usize,
    pub violations_seen: usize,
    pub violations_corrected: usize,
    pub disagreements_seen: usize,
}

/// The kind of action an entry is, or None for an observation (or the first
/// entry, which the memex writes itself).
pub fn kind(e: &Entry) -> Option<&'static str> {
    let op = e.body().and_then(|b| b["op"].as_str()).unwrap_or("");
    match (e.kind(), op) {
        ("edit", "init") => None,
        ("edit", "facts")
            if e.body()
                .is_some_and(|b| b.to_string().contains("\"ruling\"")) =>
        {
            Some("checking")
        }
        ("edit", _) | ("link", _) | ("name", _)
            if e.body().is_some_and(|b| b.get("suggestion").is_some()) =>
        {
            Some("deciding")
        }
        ("edit", _) | ("link", _) | ("name", _) => Some("writing"),
        ("adopt", _)
        | ("retract", _)
        | ("suppose", _)
        | ("undo", _)
        | ("redo", _)
        | ("purge", _) => Some("deciding"),
        ("import", _) => Some("importing"),
        ("view", "leap") => Some("searching"),
        ("view", "follow") => Some("following links"),
        ("view", "move" | "fold" | "pane" | "scope") => Some("navigating"),
        ("view", "dismiss" | "world") => Some("deciding"),
        ("view", "interrogate") => Some("checking"),
        ("view", "export") => Some("exporting"),
        ("view", _) => None,
        _ => None,
    }
}

pub fn measure(entries: &[Entry]) -> Report {
    let mut r = Report::of(entries.len());
    let (mut offers, mut watch) = (Offers::default(), Watch::default());
    for e in entries {
        r.count(e);
        offers.read(e);
        watch.read(e);
    }
    offers.report(&mut r, &retracted(entries));
    watch.report(&mut r);
    r
}

impl Report {
    /// A report of a journal of n entries, every kind of action at 0.
    fn of(n: usize) -> Report {
        let actions = KINDS.iter().map(|k| (*k, 0)).collect();
        Report {
            entries: n,
            actions,
            ..Default::default()
        }
    }

    /// An entry, as an action of its kind, an observation, or neither;
    /// and a dismissal counted.
    fn count(&mut self, e: &Entry) {
        match kind(e) {
            Some(k) => *self.actions.get_mut(k).unwrap() += 1,
            None if is_view(e, &["observed", "offer"]) => self.observations += 1,
            None => self.unclassified += 1,
        }
        if is_view(e, &["dismiss"]) {
            self.dismissed += 1;
        }
    }
}

/// Whether an entry is a view of one of these ops.
fn is_view(e: &Entry, ops: &[&str]) -> bool {
    let op = e.body().and_then(|b| b["op"].as_str());
    e.kind() == "view" && op.is_some_and(|o| ops.contains(&o))
}

/// The entries that retract entries, by the entry each retracts.
fn retracted(entries: &[Entry]) -> BTreeSet<u64> {
    let retracts = entries.iter().filter(|e| e.kind() == "retract");
    retracts
        .filter_map(|e| e.body().and_then(|b| b["target"].as_u64()))
        .collect()
}

/// A list's texts.
fn strings(v: &serde_json::Value) -> Vec<String> {
    let list = v.as_array().into_iter().flatten();
    list.filter_map(|x| x.as_str().map(String::from)).collect()
}

/// The suggestions offered, by key, and the entries that adopted one.
#[derive(Default)]
struct Offers {
    offered: BTreeSet<String>,
    adopted: Vec<u64>,
}

impl Offers {
    fn read(&mut self, e: &Entry) {
        let Some(b) = e.body() else { return };
        if b.get("suggestion").is_some() {
            self.adopted.push(e.seq);
        }
        if is_view(e, &["offer"]) {
            self.offered.extend(strings(&b["keys"]));
        }
    }

    fn report(&self, r: &mut Report, retracted: &BTreeSet<u64>) {
        r.offered = self.offered.len();
        r.adopted = self.adopted.len();
        r.adopted_then_retracted = self
            .adopted
            .iter()
            .filter(|s| retracted.contains(s))
            .count();
    }
}

/// What observations showed: the violations and disagreements seen, the
/// violations later gone in the actual world, and the violations shown
/// last.
#[derive(Default)]
struct Watch {
    violations: BTreeSet<String>,
    disagreements: BTreeSet<String>,
    corrected: BTreeSet<String>,
    last: Vec<String>,
}

impl Watch {
    fn read(&mut self, e: &Entry) {
        let Some(b) = e.body() else { return };
        if is_view(e, &["observed"]) {
            self.observe(b);
        }
    }

    /// An observation: a violation that the actual world showed when last
    /// observed and does not show now is corrected. A supposition's
    /// observations are seen, but compared with nothing: they are not the
    /// actual world's.
    fn observe(&mut self, b: &serde_json::Value) {
        let now = strings(&b["violations"]);
        self.violations.extend(now.iter().cloned());
        self.disagreements.extend(strings(&b["disagreements"]));
        if b["world"] == "actual" {
            let gone = self.last.iter().filter(|k| !now.contains(k)).cloned();
            self.corrected.extend(gone);
            self.last = now;
        }
    }

    fn report(&self, r: &mut Report) {
        r.violations_seen = self.violations.len();
        r.violations_corrected = self.corrected.len();
        r.disagreements_seen = self.disagreements.len();
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let actions: usize = self.actions.values().sum();
        writeln!(
            f,
            "journal: {} entries: {actions} actions, {} observations, {} unclassified",
            self.entries, self.observations, self.unclassified
        )?;
        writeln!(f, "actions by kind:")?;
        for k in KINDS {
            writeln!(f, "  {k:<16} {}", self.actions[k])?;
        }
        let clerical = self.actions["searching"]
            + self.actions["following links"]
            + self.actions["navigating"];
        let share = if actions == 0 {
            0.0
        } else {
            100.0 * clerical as f64 / actions as f64
        };
        writeln!(
            f,
            "clerical share (searching, following links, navigating): {clerical} of {actions}, {share:.1}%"
        )?;
        writeln!(
            f,
            "suggestions: {} offered, {} adopted, {} dismissed, {} adopted and later retracted",
            self.offered, self.adopted, self.dismissed, self.adopted_then_retracted
        )?;
        writeln!(
            f,
            "constraint violations: {} seen, {} later gone in the actual world (corrected)",
            self.violations_seen, self.violations_corrected
        )?;
        write!(
            f,
            "disagreements between sources: {} seen",
            self.disagreements_seen
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn chain(bodies: &[(&str, serde_json::Value)]) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        for body in bodies {
            let entry = next(&out, body);
            out.push(entry);
        }
        out
    }

    /// The entry of a kind and a body after these, numbered after the
    /// last.
    fn next(es: &[Entry], (kind, body): &(&str, serde_json::Value)) -> Entry {
        Entry::new(es.len() as u64 + 1, after(es), kind, body.clone()).unwrap()
    }

    /// The last entry, as Entry::new names a parent.
    fn after(es: &[Entry]) -> Option<(u64, &str)> {
        es.last().map(|e| (e.seq, e.hash.as_str()))
    }

    #[test]
    fn a_violation_gone_only_in_a_supposition_is_not_corrected() {
        // Catches a supposed world counted as a correction: the actual world
        // still has the violation.
        let obs = |v: &[&str], w: &str| {
            (
                "view",
                json!({"op": "observed", "violations": v, "disagreements": [], "world": w}),
            )
        };
        let es = chain(&[
            ("edit", json!({"op": "init"})),
            obs(&["x"], "actual"),
            obs(&[], "suppose y"),
            obs(&["x"], "actual"),
        ]);
        let r = measure(&es);
        assert_eq!((r.violations_seen, r.violations_corrected), (1, 0));
        let es = chain(&[
            ("edit", json!({"op": "init"})),
            obs(&["x"], "actual"),
            obs(&[], "actual"),
        ]);
        assert_eq!(measure(&es).violations_corrected, 1);
        // A supposition's observations are not what the actual world is
        // compared with: one seen only there is not corrected, and one
        // gone from the actual world after a supposition is (the code
        // review, B1).
        let es = chain(&[
            ("edit", json!({"op": "init"})),
            obs(&["z"], "suppose y"),
            obs(&[], "actual"),
        ]);
        assert_eq!(measure(&es).violations_corrected, 0);
        let es = chain(&[
            ("edit", json!({"op": "init"})),
            obs(&["x"], "actual"),
            obs(&[], "suppose y"),
            obs(&[], "actual"),
        ]);
        assert_eq!(measure(&es).violations_corrected, 1);
    }

    #[test]
    fn a_disagreement_shown_twice_is_seen_once() {
        // Catches disagreements not counted, or counted at each showing.
        let shown = || {
            let b = json!({"op": "observed", "violations": [], "disagreements": ["d1"], "world": "actual"});
            ("view", b)
        };
        let es = chain(&[("edit", json!({"op": "init"})), shown(), shown()]);
        assert_eq!(measure(&es).disagreements_seen, 1);
    }

    #[test]
    fn an_adoption_later_retracted_is_counted() {
        // Catches a retraction of an adopted suggestion missed.
        let es = chain(&[
            ("edit", json!({"op": "init"})),
            ("edit", json!({"op": "facts", "suggestion": "k1"})),
            ("retract", json!({"target": 2})),
        ]);
        assert_eq!(measure(&es).adopted_then_retracted, 1);
    }
}
