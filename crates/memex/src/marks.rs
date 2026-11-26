//! The marks under each item: the findings of the current world's rules
//! that rest on its facts, each said once, in words, with the facts it is
//! against and where they are (docs/guide.md, § 1.1). What the screen
//! draws under an item, and what the offers, the lists and the verdicts
//! read of it.

use crate::outline::Outline;
use crate::reason::{self, ConstraintEval, Finding, Prov, Solution, Sourced, WorldEval};
use format::terms::Term;
use std::collections::{BTreeSet, HashMap};

/// The fact a fact entry states: for a claim, what is claimed.
pub(crate) fn inner(t: &Term) -> &Term {
    match t {
        Term::Compound(n, a) if n == "claims" && a.len() == 2 => &a[1],
        t => t,
    }
}

/// How two facts differ: the arguments that are not equal, here and there.
pub(crate) fn difference(a: &Term, b: &Term) -> (String, String) {
    match (inner(a), inner(b)) {
        (Term::Compound(f, x), Term::Compound(g, y)) if f == g && x.len() == y.len() => {
            let d: Vec<(String, String)> = x
                .iter()
                .zip(y)
                .filter(|(p, q)| p != q)
                .map(|(p, q)| (reason::show(p), reason::show(q)))
                .collect();
            (
                d.iter().map(|p| p.0.clone()).collect::<Vec<_>>().join(" "),
                d.iter().map(|p| p.1.clone()).collect::<Vec<_>>().join(" "),
            )
        }
        (x, y) => (reason::show(x), reason::show(y)),
    }
}

/// The marks under an item: (is a violation, text) for each constraint
/// solution that rests on one of its facts.
pub fn item_marks(e: &WorldEval, outline: &Outline, item: u64) -> Vec<MarkLine> {
    let findings: Vec<Finding> = e
        .constraints
        .iter()
        .flat_map(ConstraintEval::findings)
        .collect();
    let parts = findings
        .iter()
        .flat_map(|f| finding_marks(f, outline, item));
    merged(parts.collect())
}

/// The marks of each item that has any, in the outline's order: each
/// finding gathered once, under the items whose facts it rests on, not
/// once for each item, as `item_marks` would.
pub fn marks_by_item(e: &WorldEval, outline: &Outline) -> Vec<(u64, Vec<MarkLine>)> {
    let findings: Vec<Finding> = e
        .constraints
        .iter()
        .flat_map(ConstraintEval::findings)
        .collect();
    let mut under: HashMap<u64, Vec<&Finding>> = HashMap::new();
    for f in &findings {
        for item in items_of(f.sol, outline) {
            under.entry(item).or_default().push(f);
        }
    }
    let marks = |id| Some((id, marks_under(under.remove(&id)?, outline, id)));
    outline.in_order().into_iter().filter_map(marks).collect()
}

/// Whether a finding is marked under an item: it rests on the item's
/// facts.
pub(crate) fn marked_under(f: &Finding, outline: &Outline, item: u64) -> bool {
    items_of(f.sol, outline).contains(&item)
}

/// The items whose facts a solution rests on, each once.
pub(crate) fn items_of(sol: &Solution, outline: &Outline) -> BTreeSet<u64> {
    let items = sol
        .prov
        .iter()
        .filter_map(|p| outline.fact_item.get(&p.seq()));
    items.copied().collect()
}

/// An item's marks, from the findings that rest on its facts.
fn marks_under(findings: Vec<&Finding>, outline: &Outline, item: u64) -> Vec<MarkLine> {
    let parts = findings
        .into_iter()
        .filter_map(|f| finding_marks(f, outline, item));
    merged(parts.collect())
}

/// A mark in parts: whether it is loud, what it says of the item's own
/// fact, and the other facts it is against, each with where it is.
struct Mark {
    loud: bool,
    head: String,
    rest: Vec<String>,
    constraint: u64,
}

/// A mark under an item, as drawn: whether it is loud, its text, and the
/// entry of the constraint it is a finding of, if it is one.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkLine {
    pub loud: bool,
    pub text: String,
    pub constraint: Option<u64>,
}

impl Mark {
    /// Whether it says the same of the item as another: the same kind,
    /// the same rule, the same fact here.
    fn says_same(&self, other: &Mark) -> bool {
        self.loud == other.loud && self.head == other.head
    }

    /// Name more other facts, each once.
    fn add(&mut self, rest: Vec<String>) {
        for r in rest {
            if !self.rest.contains(&r) {
                self.rest.push(r);
            }
        }
    }

    /// Its text: what it says of the item, then the other facts.
    fn text(self) -> MarkLine {
        let text = match self.rest.is_empty() {
            true => self.head,
            false => format!("{}; {}", self.head, self.rest.join("; ")),
        };
        let (loud, constraint) = (self.loud, Some(self.constraint));
        MarkLine {
            loud,
            text,
            constraint,
        }
    }
}

/// Marks that say the same of the item, one each: the facts they are
/// against, in order, each once. So a rule the item breaks with others
/// is one mark, not one for each other fact or each solution.
fn merged(marks: Vec<Mark>) -> Vec<MarkLine> {
    let mut out: Vec<Mark> = Vec::new();
    for m in marks {
        match out.iter_mut().find(|o| o.says_same(&m)) {
            Some(o) => o.add(m.rest),
            None => out.push(m),
        }
    }
    out.into_iter().map(Mark::text).collect()
}

/// A finding's mark under an item: its first fact there against each
/// other fact, or, if it rests on the item's facts alone, those facts.
fn finding_marks(f: &Finding, outline: &Outline, item: u64) -> Option<Mark> {
    let m = Marking { f, outline, item };
    m.alone().or_else(|| m.against())
}

/// A finding, marked under an item of an outline.
struct Marking<'a> {
    f: &'a Finding<'a>,
    outline: &'a Outline,
    item: u64,
}

impl<'a> Marking<'a> {
    /// The finding's facts, each with its provenance: the item's, and
    /// the rest.
    fn split(&self) -> (Vec<Sourced<'a>>, Vec<Sourced<'a>>) {
        split_by_item(self.f.sol, self.outline, self.item)
    }

    /// The mark of a finding that rests on the item's facts alone: those
    /// facts.
    fn alone(&self) -> Option<Mark> {
        let (mine, others) = self.split();
        if mine.is_empty() || !others.is_empty() {
            return None;
        }
        let facts: Vec<String> = mine.iter().map(|(_, t)| reason::show(inner(t))).collect();
        Some(self.mark(&facts.join("; "), vec![]))
    }

    /// The mark of a finding that rests on the item's facts and others:
    /// its first fact here, against each other fact.
    fn against(&self) -> Option<Mark> {
        let against = against_item(self.f, self.outline, self.item);
        let here = against.first()?.here_text();
        let theres = against.iter().map(|a| a.there_text(self.outline));
        Some(self.mark(&here, theres.collect()))
    }

    /// A mark: what the finding is, by the rule it is of, then what it
    /// says of the item's fact, and the others.
    fn mark(&self, here: &str, rest: Vec<String>) -> Mark {
        let head = format!("{}: {here} here", finding_kind(self.f, self.outline));
        Mark {
            loud: self.f.violation,
            head,
            rest,
            constraint: self.f.seq,
        }
    }
}

/// What a finding is, by the rule it is of.
fn finding_kind(f: &Finding, outline: &Outline) -> String {
    let rule = rule_named(outline, f.name);
    match f.violation {
        true => format!("breaks {rule}"),
        false => format!("sources disagree under {rule}"),
    }
}

/// A rule, by the address of the item that states it when one does (its
/// text is the rule's name), else by its name: short, as each mark under
/// each item names it.
fn rule_named(outline: &Outline, name: &str) -> String {
    stating_rule(outline, name).map_or_else(
        || format!("“{name}”"),
        |i| format!("constraint {}", outline.number(i)),
    )
}

/// The item whose text states a rule of a name, if one does.
pub(crate) fn stating_rule(outline: &Outline, name: &str) -> Option<u64> {
    let states = |i: &u64| outline.text(*i).trim_end_matches('.') == name;
    outline.in_order().into_iter().find(states)
}

/// A fact of an item against another fact of a finding that rests on both.
struct Against<'a> {
    here: &'a Term,
    there: Sourced<'a>,
}

impl Against<'_> {
    /// What the item's fact holds that the other's does not.
    fn here_text(&self) -> String {
        difference(self.here, self.there.1).0
    }

    /// What the other fact holds that the item's does not, and where it
    /// comes from.
    fn there_text(&self, outline: &Outline) -> String {
        let b = difference(self.here, self.there.1).1;
        format!("{b} in {}", where_from(outline, self.there.0))
    }
}

/// A finding against an item, if it rests on one of the item's facts:
/// the first of them against each other fact.
fn against_item<'a>(f: &'a Finding, outline: &Outline, item: u64) -> Vec<Against<'a>> {
    let (mine, others) = split_by_item(f.sol, outline, item);
    let Some((_, here)) = mine.first() else {
        return vec![];
    };
    let against = |there| Against { here, there };
    others.into_iter().map(against).collect()
}

/// A solution's facts, each with its provenance: those an item states,
/// and the rest.
fn split_by_item<'a>(
    sol: &'a Solution,
    outline: &Outline,
    item: u64,
) -> (Vec<Sourced<'a>>, Vec<Sourced<'a>>) {
    sol.sourced()
        .partition(|(p, _)| outline.states(item, p.seq()))
}

/// Where a fact comes from: its item's number; else a claim's source; else
/// its entry.
pub(crate) fn where_from(outline: &Outline, p: &Prov) -> String {
    match (outline.fact_item.get(&p.seq()), p) {
        (Some(i), _) => outline.number(*i),
        (None, Prov::Claim(src, _)) => src.clone(),
        (None, Prov::Belief(s)) => format!("entry {s}"),
    }
}
