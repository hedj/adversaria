//! The measures report (docs/plan.md, Milestone 7), from the journal alone.
//!
//! Every entry is one user action, one observation the memex recorded, or
//! an entry the memex wrote on its own (from version 2, marked
//! `"by": "memex"`: a session's start, a reading of a text and what it
//! states, relinking), which is not the user's action.
//! Actions by kind, after Licklider's count of his own work:
//! * writing: edit (create, text, move, delete, facts, derive, constraint), link, name;
//! * searching: view leap and find;
//! * following links: view follow;
//! * navigating: view move, fold, pane, scope; view go (to an address),
//!   back and forward;
//! * deciding: adopt, retract, suppose, undo, redo, purge, view dismiss and world;
//! * checking: view interrogate; view move that ends with the inspector
//!   showing a claim's questions (`inspects`); facts that are rulings;
//!   and view verdict, the user's word on whether a finding is right;
//! * importing: import; exporting: view export.
//!
//! A view written (from version 2) records only where writing left the
//! cursor: in the text typed, on an item made, off one deleted. It is
//! part of the writing before it, not an action, and not navigating.
//!
//! A verdict (version 2) is the user's word on findings: a real error, the
//! rule is wrong, the text was misread, or known already. A finding
//! judged more than once counts by its last verdict.
//!
//! Clerical work is searching, following links and navigating.
//! Observations (not actions): view observed (the violations and
//! disagreements reasoning found) and view offer (each suggestion the
//! memex made, in its words, from version 3; an offer of version 1 or 2,
//! recorded only once drawn on the screen, is marked `drawn` when
//! migrated, and those counted apart; so is each offer of a journal of
//! version 1 or 2 not migrated, whose entry 1 names no version).
//! A suggestion is adopted by an entry that carries "suggestion"; it is
//! later retracted if a retract entry names that entry.
//! A violation is gone if a later observation in the actual world no
//! longer shows it. Why it went is what that observation records under
//! `gone` (version 2): corrected, its rule changed or deleted, an item
//! under it deleted, or such an item's text no longer read; without a
//! record, as in version 1, the reason is unrecorded.

use format::record::{Entry, EntryKind};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The verdicts on findings, as verdict entries name them.
pub const VERDICTS: [&str; 4] = ["error", "rule", "misread", "known"];

/// Why a violation went, as observed entries record it, and when they
/// record nothing.
pub const WHY: [&str; 5] = ["corrected", "rule", "deleted", "unread", "unrecorded"];

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
    /// Entries the memex wrote on its own, other than observations.
    pub by_memex: usize,
    /// View entries that record only where writing left the cursor.
    pub left_by_writing: usize,
    /// Entries that are none of these: the memex's first entry, or a view
    /// the report does not know. Counted, not hidden.
    pub unclassified: usize,
    pub offered: usize,
    /// Of those offered, the ones recorded by version 1 or 2, which
    /// recorded a suggestion only once it was drawn.
    pub offered_drawn: usize,
    pub adopted: usize,
    pub dismissed: usize,
    pub adopted_then_retracted: usize,
    pub violations_seen: usize,
    /// Violations later gone in the actual world.
    pub violations_gone: usize,
    /// Of those, the ones gone because their facts were corrected.
    pub violations_corrected: usize,
    /// Of those gone, how many went for each reason, as WHY names them.
    pub gone_why: BTreeMap<&'static str, usize>,
    pub disagreements_seen: usize,
    /// The findings given a verdict, by their last verdict, as VERDICTS
    /// names them.
    pub verdicts: BTreeMap<&'static str, usize>,
}

/// Whether a facts edit states a ruling: a fact ruling(...), not any
/// fact that holds the word.
fn states_a_ruling(body: &serde_json::Value) -> bool {
    let facts = body["facts"].as_array().into_iter().flatten();
    facts.into_iter().any(|f| f["f"] == "ruling")
}

/// The kind of action an entry is, or None for an observation (or the first
/// entry, which the memex writes itself).
pub fn kind(e: &Entry) -> Option<&'static str> {
    let op = e.body().and_then(|b| b["op"].as_str()).unwrap_or("");
    match (e.kind(), op) {
        (EntryKind::Edit, "init") => None,
        (EntryKind::Edit, "facts") if e.body().is_some_and(states_a_ruling) => Some("checking"),
        (EntryKind::Edit, _) | (EntryKind::Link, _) | (EntryKind::Name, _)
            if e.body().is_some_and(|b| b.get("suggestion").is_some()) =>
        {
            Some("deciding")
        }
        (EntryKind::Edit, _) | (EntryKind::Link, _) | (EntryKind::Name, _) => Some("writing"),
        (EntryKind::Adopt, _)
        | (EntryKind::Retract, _)
        | (EntryKind::Suppose, _)
        | (EntryKind::Undo, _)
        | (EntryKind::Redo, _)
        | (EntryKind::Purge, _) => Some("deciding"),
        (EntryKind::Import, _) => Some("importing"),
        (EntryKind::View, "leap" | "find") => Some("searching"),
        (EntryKind::View, "follow") => Some("following links"),
        (EntryKind::View, "move") if inspects(e) => Some("checking"),
        (EntryKind::View, "move" | "fold" | "pane" | "scope" | "go" | "back" | "forward") => {
            Some("navigating")
        }
        (EntryKind::View, "dismiss" | "world") => Some("deciding"),
        (EntryKind::View, "interrogate" | "verdict") => Some("checking"),
        (EntryKind::View, "export") => Some("exporting"),
        (EntryKind::View, _) => None,
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
    r.verdicts = verdicts(entries);
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

    /// An entry, as an observation, the memex's own, where writing left
    /// the cursor, an action of its kind, or none of these; and a
    /// dismissal counted.
    fn count(&mut self, e: &Entry) {
        match kind(e) {
            _ if is_view(e, &["observed", "offer"]) => self.observations += 1,
            _ if by_memex(e) => self.by_memex += 1,
            _ if is_view(e, &["written"]) => self.left_by_writing += 1,
            Some(k) => *self.actions.get_mut(k).unwrap() += 1,
            None => self.unclassified += 1,
        }
        if is_view(e, &["dismiss"]) {
            self.dismissed += 1;
        }
    }
}

/// Whether a view entry shows a claim's questions in the inspector: since
/// the inspector shows them at the selected claim item, without a command
/// (docs/concepts.md, decision 4), a move to one is a check.
fn inspects(e: &Entry) -> bool {
    e.body().is_some_and(|b| b["inspects"].is_u64())
}

/// Whether the memex wrote an entry on its own, not at the user's command
/// or key.
fn by_memex(e: &Entry) -> bool {
    e.body().is_some_and(|b| b["by"] == "memex")
}

/// Whether an entry is a view of one of these ops.
fn is_view(e: &Entry, ops: &[&str]) -> bool {
    let op = e.body().and_then(|b| b["op"].as_str());
    e.kind() == EntryKind::View && op.is_some_and(|o| ops.contains(&o))
}

/// The entries that retract entries, by the entry each retracts.
fn retracted(entries: &[Entry]) -> BTreeSet<u64> {
    let retracts = entries.iter().filter(|e| e.kind() == EntryKind::Retract);
    retracts
        .filter_map(|e| e.body().and_then(|b| b["target"].as_u64()))
        .collect()
}

/// How many findings have each verdict last.
fn verdicts(entries: &[Entry]) -> BTreeMap<&'static str, usize> {
    let mut judged = Judged::default();
    entries.iter().for_each(|e| judged.read(e));
    judged.counts()
}

/// Each finding's last verdict, by its name.
#[derive(Default)]
struct Judged(BTreeMap<String, &'static str>);

impl Judged {
    /// A verdict entry: its word, for each finding it names.
    fn read(&mut self, e: &Entry) {
        let Some(b) = e.body().filter(|_| is_view(e, &["verdict"])) else {
            return;
        };
        let Some(word) = VERDICTS.iter().find(|v| b["verdict"] == **v) else {
            return;
        };
        let named = strings(&b["findings"]).into_iter();
        self.0.extend(named.map(|f| (f, *word)));
    }

    /// How many findings have each verdict last.
    fn counts(&self) -> BTreeMap<&'static str, usize> {
        let count = |v: &&str| self.0.values().filter(|w| *w == v).count();
        VERDICTS.iter().map(|v| (*v, count(v))).collect()
    }
}

/// Why each violation an observation records as gone went, by its name;
/// a reason the report does not know is unrecorded.
fn reasons(b: &serde_json::Value) -> BTreeMap<String, &'static str> {
    let each = b["gone"].as_array().into_iter().flatten();
    each.filter_map(|g| Some((g["finding"].as_str()?.to_string(), reason(&g["why"]))))
        .collect()
}

/// A reason, as WHY names it.
fn reason(why: &serde_json::Value) -> &'static str {
    let known = WHY.iter().find(|w| why == **w);
    known.copied().unwrap_or("unrecorded")
}

/// A list's texts.
fn strings(v: &serde_json::Value) -> Vec<String> {
    let list = v.as_array().into_iter().flatten();
    list.filter_map(|x| x.as_str().map(String::from)).collect()
}

/// The offers recorded by version 1 or 2, if any were: they recorded a
/// suggestion only once drawn, so they count fewer than were made.
fn drawn_said(drawn: usize) -> String {
    match drawn {
        0 => String::new(),
        n => format!(
            "; {n} of those offered recorded by version 1 or 2, which recorded only those drawn"
        ),
    }
}

/// The suggestions offered, by key, and the entries that adopted one.
#[derive(Default)]
struct Offers {
    offered: BTreeSet<String>,
    drawn: BTreeSet<String>,
    adopted: Vec<u64>,
    /// Entry 1 names no version of 3 or later: the journal was written
    /// by version 1 or 2 and not migrated, so each of its offers was
    /// recorded only once drawn.
    before_3: bool,
}

impl Offers {
    fn read(&mut self, e: &Entry) {
        let Some(b) = e.body() else { return };
        if e.seq == 1 {
            self.before_3 = b["version"].as_u64().is_none_or(|v| v < 3);
        }
        if b.get("suggestion").is_some() {
            self.adopted.push(e.seq);
        }
        if is_view(e, &["offer"]) {
            let keys = strings(&b["keys"]);
            if self.before_3 || b["drawn"] == true {
                self.drawn.extend(keys.iter().cloned());
            }
            self.offered.extend(keys);
        }
    }

    fn report(&self, r: &mut Report, retracted: &BTreeSet<u64>) {
        r.offered = self.offered.len();
        r.offered_drawn = self.drawn.len();
        r.adopted = self.adopted.len();
        r.adopted_then_retracted = self
            .adopted
            .iter()
            .filter(|s| retracted.contains(s))
            .count();
    }
}

/// What observations showed: the violations and disagreements seen, the
/// violations later gone in the actual world with why each went, and the
/// violations shown last.
#[derive(Default)]
struct Watch {
    violations: BTreeSet<String>,
    disagreements: BTreeSet<String>,
    gone: BTreeMap<String, &'static str>,
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
    /// observed and does not show now is gone, for the reason recorded. A
    /// supposition's observations are seen, but compared with nothing:
    /// they are not the actual world's.
    fn observe(&mut self, b: &serde_json::Value) {
        let now = strings(&b["violations"]);
        self.violations.extend(now.iter().cloned());
        self.disagreements.extend(strings(&b["disagreements"]));
        if b["world"] == "actual" {
            self.went(&now, &reasons(b));
            self.last = now;
        }
    }

    /// Each violation shown last and not now, gone for its reason.
    fn went(&mut self, now: &[String], why: &BTreeMap<String, &'static str>) {
        let gone = self.last.iter().filter(|k| !now.contains(k));
        for k in gone {
            let reason = why.get(k).copied().unwrap_or("unrecorded");
            self.gone.insert(k.clone(), reason);
        }
    }

    fn report(&self, r: &mut Report) {
        r.violations_seen = self.violations.len();
        r.violations_gone = self.gone.len();
        r.gone_why = WHY.iter().map(|w| (*w, self.count_gone(w))).collect();
        r.violations_corrected = r.gone_why["corrected"];
        r.disagreements_seen = self.disagreements.len();
    }

    /// How many violations went for a reason.
    fn count_gone(&self, why: &str) -> usize {
        self.gone.values().filter(|w| **w == why).count()
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let actions: usize = self.actions.values().sum();
        writeln!(
            f,
            "journal: {} entries: {actions} actions, {} observations",
            self.entries, self.observations
        )?;
        writeln!(
            f,
            "other entries: {} views where writing left the cursor, {} written by the memex on its own, {} unclassified",
            self.left_by_writing, self.by_memex, self.unclassified
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
            "suggestions: {} offered, {} adopted, {} dismissed, {} adopted and later retracted{}",
            self.offered,
            self.adopted,
            self.dismissed,
            self.adopted_then_retracted,
            drawn_said(self.offered_drawn)
        )?;
        writeln!(
            f,
            "constraint violations: {} seen, {} later gone in the actual world",
            self.violations_seen, self.violations_gone
        )?;
        let why = |w: &str| self.gone_why.get(w).copied().unwrap_or(0);
        writeln!(
            f,
            "  gone because: {} corrected, {} their rule changed or deleted, {} an item under them deleted, {} its text no longer read, {} not recorded",
            why("corrected"),
            why("rule"),
            why("deleted"),
            why("unread"),
            why("unrecorded")
        )?;
        writeln!(
            f,
            "disagreements between sources: {} seen",
            self.disagreements_seen
        )?;
        let said = |v: &str| self.verdicts.get(v).copied().unwrap_or(0);
        write!(
            f,
            "verdicts on findings: {} a real error, {} the rule is wrong, {} the text was misread, {} known already",
            said("error"),
            said("rule"),
            said("misread"),
            said("known")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn chain(bodies: &[(EntryKind, serde_json::Value)]) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        for body in bodies {
            let entry = next(&out, body);
            out.push(entry);
        }
        out
    }

    /// The entry of a kind and a body after these, numbered after the
    /// last.
    fn next(es: &[Entry], (kind, body): &(EntryKind, serde_json::Value)) -> Entry {
        Entry::new(es.len() as u64 + 1, after(es), *kind, body.clone()).unwrap()
    }

    /// The last entry, as Entry::new names a parent.
    fn after(es: &[Entry]) -> Option<(u64, &str)> {
        es.last().map(|e| (e.seq, e.hash.as_str()))
    }

    #[test]
    fn a_violation_gone_only_in_a_supposition_is_not_gone() {
        // Catches a supposed world counted as a correction: the actual world
        // still has the violation.
        let obs = |v: &[&str], w: &str| {
            (
                EntryKind::View,
                json!({"op": "observed", "violations": v, "disagreements": [], "world": w}),
            )
        };
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            obs(&["x"], "actual"),
            obs(&[], "suppose y"),
            obs(&["x"], "actual"),
        ]);
        let r = measure(&es);
        assert_eq!((r.violations_seen, r.violations_gone), (1, 0));
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            obs(&["x"], "actual"),
            obs(&[], "actual"),
        ]);
        assert_eq!(measure(&es).violations_gone, 1);
        // A supposition's observations are not what the actual world is
        // compared with: one seen only there is not corrected, and one
        // gone from the actual world after a supposition is (the code
        // review, B1).
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            obs(&["z"], "suppose y"),
            obs(&[], "actual"),
        ]);
        assert_eq!(measure(&es).violations_gone, 0);
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            obs(&["x"], "actual"),
            obs(&[], "suppose y"),
            obs(&[], "actual"),
        ]);
        assert_eq!(measure(&es).violations_gone, 1);
    }

    #[test]
    fn what_the_memex_writes_on_its_own_is_no_action_of_the_user() {
        // Catches a reading's facts counted as the user's writing, which
        // raised the actions and lowered the clerical share.
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            (
                EntryKind::View,
                json!({"op": "start", "build": "abc", "by": "memex"}),
            ),
            (
                EntryKind::Edit,
                json!({"op": "facts", "item": 2, "by": "memex"}),
            ),
            (EntryKind::Edit, json!({"op": "facts", "item": 2})),
            (
                EntryKind::View,
                json!({"op": "offer", "keys": ["k"], "by": "memex"}),
            ),
        ]);
        let r = measure(&es);
        assert_eq!((r.by_memex, r.actions["writing"]), (2, 1));
        assert_eq!((r.observations, r.unclassified), (1, 1));
    }

    #[test]
    fn an_offer_recorded_only_once_drawn_is_counted_apart() {
        // Catches an offer of version 1 or 2, which recorded only the
        // suggestions drawn, counted as if it recorded each one made; or
        // a suggestion offered twice counted twice.
        let offer = |keys: &[&str], drawn: bool| {
            let mut b = json!({"op": "offer", "keys": keys, "by": "memex"});
            if drawn {
                b["drawn"] = json!(true);
            }
            (EntryKind::View, b)
        };
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init", "version": 3})),
            offer(&["a", "b"], true),
            offer(&["b", "c"], false),
        ]);
        let r = measure(&es);
        assert_eq!((r.offered, r.offered_drawn), (3, 2));
        assert!(
            r.to_string()
                .contains("2 of those offered recorded by version 1 or 2"),
            "{r}"
        );
    }

    #[test]
    fn each_offer_of_a_journal_not_migrated_from_version_2_is_counted_drawn() {
        // Catches a version 2 journal read as it was written, its offers
        // not marked, counted as if it recorded each suggestion made.
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            (
                EntryKind::View,
                json!({"op": "offer", "keys": ["a"], "by": "memex"}),
            ),
        ]);
        assert_eq!(measure(&es).offered_drawn, 1);
    }

    #[test]
    fn a_violation_gone_is_counted_by_the_reason_recorded() {
        // Catches every violation that went counted as corrected, though
        // its rule was deleted; or a reason lost when none is recorded.
        let obs = |v: &[&str], gone: serde_json::Value| {
            let b = json!({"op": "observed", "violations": v, "disagreements": [], "world": "actual", "gone": gone});
            (EntryKind::View, b)
        };
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            obs(&["x", "y", "z"], json!([])),
            obs(
                &["z"],
                json!([{"finding": "x", "why": "rule"}, {"finding": "y", "why": "corrected"}]),
            ),
            obs(&[], json!(null)),
        ]);
        let r = measure(&es);
        assert_eq!(r.violations_gone, 3);
        let why = |w| r.gone_why[w];
        assert_eq!(
            (why("rule"), why("corrected"), why("unrecorded")),
            (1, 1, 1)
        );
        assert_eq!(r.violations_corrected, 1);
    }

    #[test]
    fn a_finding_counts_by_its_last_verdict() {
        // Catches verdicts not counted, or a finding judged twice counted
        // twice.
        let verdict = |word: &str, findings: &[&str]| {
            let b = json!({"op": "verdict", "verdict": word, "rule": 3, "under": 2, "findings": findings});
            (EntryKind::View, b)
        };
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            verdict("known", &["a"]),
            verdict("rule", &["b", "c"]),
            verdict("error", &["a"]),
        ]);
        let r = measure(&es);
        let said = |v| r.verdicts[v];
        assert_eq!(
            (said("error"), said("rule"), said("misread"), said("known")),
            (1, 2, 0, 0)
        );
    }

    #[test]
    fn a_move_to_a_claims_questions_is_checking() {
        // Catches a check made by selecting a claim item counted as
        // navigating: checking would fall with no less of it done.
        let moved = |inspects: serde_json::Value| {
            (EntryKind::View, json!({"op": "move", "inspects": inspects}))
        };
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            moved(json!(3)),
            moved(json!(null)),
        ]);
        assert_eq!(
            (kind(&es[1]), kind(&es[2])),
            (Some("checking"), Some("navigating"))
        );
    }

    #[test]
    fn where_writing_left_the_cursor_is_not_navigating() {
        // Catches the view entry after each burst of typing counted as a
        // move of the user's, as all 14 moves of the version-1 fixture were.
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            (EntryKind::Edit, json!({"op": "text", "item": 2})),
            (EntryKind::View, json!({"op": "written"})),
            (EntryKind::View, json!({"op": "move"})),
        ]);
        let r = measure(&es);
        assert_eq!(r.left_by_writing, 1);
        assert_eq!((r.actions["writing"], r.actions["navigating"]), (1, 1));
        assert_eq!(r.unclassified, 1, "the first entry alone");
    }

    #[test]
    fn going_to_an_address_back_or_forward_is_navigating() {
        // Catches the jumps of go, back and forward left unclassified, as
        // eight of the version-1 fixture's nine unclassified entries were,
        // and a verdict on a finding counted as no action.
        let view = |op: &str| (EntryKind::View, json!({"op": op}));
        let ops = ["go", "back", "forward", "verdict"];
        let mut bodies = vec![(EntryKind::Edit, json!({"op": "init"}))];
        bodies.extend(ops.map(view));
        let kinds: Vec<_> = chain(&bodies)[1..].iter().map(kind).collect();
        let navigating = Some("navigating");
        assert_eq!(
            kinds,
            [navigating, navigating, navigating, Some("checking")]
        );
    }

    #[test]
    fn a_find_is_searching_as_a_leap_is() {
        // Catches the find command's view entry counted as no action, so
        // searching by it was missing from the clerical share.
        let view = |op: &str| (EntryKind::View, json!({"op": op}));
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            view("find"),
            view("leap"),
        ]);
        assert_eq!(
            (kind(&es[1]), kind(&es[2])),
            (Some("searching"), Some("searching"))
        );
    }

    #[test]
    fn a_fact_is_checking_only_if_it_is_a_ruling() {
        // Catches a fact that holds the word ruling, topic(x, ruling),
        // taken for a check (the code review, B2).
        let facts = |f: serde_json::Value| {
            (
                EntryKind::Edit,
                json!({"op": "facts", "item": 2, "facts": [f]}),
            )
        };
        let ruling = json!({"f": "ruling", "args": [2, "doi", "found", "h"]});
        let topic = json!({"f": "topic", "args": [{"a": "x"}, {"a": "ruling"}]});
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            facts(ruling),
            facts(topic),
        ]);
        assert_eq!(kind(&es[1]), Some("checking"));
        assert_eq!(kind(&es[2]), Some("writing"));
    }

    #[test]
    fn a_disagreement_shown_twice_is_seen_once() {
        // Catches disagreements not counted, or counted at each showing.
        let shown = || {
            let b = json!({"op": "observed", "violations": [], "disagreements": ["d1"], "world": "actual"});
            (EntryKind::View, b)
        };
        let es = chain(&[(EntryKind::Edit, json!({"op": "init"})), shown(), shown()]);
        assert_eq!(measure(&es).disagreements_seen, 1);
    }

    #[test]
    fn an_adoption_later_retracted_is_counted() {
        // Catches a retraction of an adopted suggestion missed.
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            (EntryKind::Edit, json!({"op": "facts", "suggestion": "k1"})),
            (EntryKind::Retract, json!({"target": 2})),
        ]);
        assert_eq!(measure(&es).adopted_then_retracted, 1);
    }
}
