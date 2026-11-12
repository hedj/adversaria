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
    let op = e.body.as_ref().and_then(|b| b["op"].as_str()).unwrap_or("");
    match (e.kind.as_str(), op) {
        ("edit", "init") => None,
        ("edit", "facts")
            if e.body
                .as_ref()
                .is_some_and(|b| b.to_string().contains("\"ruling\"")) =>
        {
            Some("checking")
        }
        ("edit", _) | ("link", _) | ("name", _)
            if e.body
                .as_ref()
                .is_some_and(|b| b.get("suggestion").is_some()) =>
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
    let mut r = Report {
        entries: entries.len(),
        ..Default::default()
    };
    for k in KINDS {
        r.actions.insert(k, 0);
    }
    let mut offered = BTreeSet::new();
    let mut adopted = Vec::new();
    let retracted: BTreeSet<u64> = entries
        .iter()
        .filter(|e| e.kind == "retract")
        .filter_map(|e| e.body.as_ref().and_then(|b| b["target"].as_u64()))
        .collect();
    let (mut seen_v, mut seen_d, mut corrected) =
        (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let mut last: Vec<String> = Vec::new();
    for e in entries {
        let op = e.body.as_ref().and_then(|b| b["op"].as_str());
        match kind(e) {
            Some(k) => *r.actions.get_mut(k).unwrap() += 1,
            None if e.kind == "view" && matches!(op, Some("observed" | "offer")) => {
                r.observations += 1
            }
            None => r.unclassified += 1,
        }
        let Some(b) = &e.body else { continue };
        if b.get("suggestion").is_some() {
            adopted.push(e.seq);
        }
        match (e.kind.as_str(), b["op"].as_str()) {
            ("view", Some("offer")) => offered.extend(
                b["keys"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|k| k.as_str().map(String::from)),
            ),
            ("view", Some("dismiss")) => r.dismissed += 1,
            ("view", Some("observed")) => {
                let keys = |k: &str| -> Vec<String> {
                    b[k].as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                };
                let now = keys("violations");
                if b["world"] == "actual" {
                    corrected.extend(last.iter().filter(|k| !now.contains(k)).cloned());
                }
                seen_v.extend(now.iter().cloned());
                seen_d.extend(keys("disagreements"));
                last = now;
            }
            _ => {}
        }
    }
    r.offered = offered.len();
    r.adopted = adopted.len();
    r.adopted_then_retracted = adopted.iter().filter(|s| retracted.contains(s)).count();
    r.violations_seen = seen_v.len();
    r.violations_corrected = corrected.len();
    r.disagreements_seen = seen_d.len();
    r
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
        for (i, (k, b)) in bodies.iter().enumerate() {
            let parent = out.last().map(|e: &Entry| (e.seq, e.hash.clone()));
            out.push(
                Entry::new(
                    i as u64 + 1,
                    parent.as_ref().map(|(s, h)| (*s, h.as_str())),
                    k,
                    b.clone(),
                )
                .unwrap(),
            );
        }
        out
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
    }
}
