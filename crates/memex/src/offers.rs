//! What waits for the learner to decide (docs/concepts-seven.md, change
//! 1): every offer put to them and not yet decided, each with its kind,
//! what it offers, its source, and the object a click or a right-click on
//! it acts on. The notebook's inspector lists them, with the cursor above
//! the first item. A revision's changes are not among them: while a
//! revision is open its changes fill the screen, the notebook hidden.

use crate::app::App;
use crate::marks::MarkLine;
use crate::notebook::Notebook;
use crate::outline::Outline;
use crate::present::Presentation;
use crate::reason::{ConstraintEval, Suggestion, WorldEval};
use format::terms::Term;
use std::collections::HashMap;

/// An offer not yet decided.
#[derive(Debug, Clone, PartialEq)]
pub struct Open {
    /// What kind of offer it is, with the number that addresses it, if
    /// one does: "finding", "claim", "mend", "suggestion 1".
    pub kind: String,
    pub what: String,
    /// Where it came from: a rule, a revision, a source, the memex.
    pub source: String,
    pub presents: Presentation,
}

/// How many offers the notebook's inspector lists; the rest it counts.
pub(crate) const LISTED: usize = 12;

impl App {
    /// Whether the notebook's inspector is drawn: an inspector is shown
    /// with the cursor above the first item, no claim is questioned in
    /// it, and no supposition's worlds fill the screen in place of the
    /// panes (crate::ui::notebook).
    pub(crate) fn notebook_drawn(&self) -> bool {
        let view = &self.session.view;
        let inspector = view.shows(crate::app::PaneKind::Links);
        let at_top = view.selected() == crate::outline::TOP;
        let plain = self.inspected_claim().is_none() && self.reason.supposition.is_none();
        inspector && at_top && plain
    }

    /// The suggestions the notebook's inspector lists, among its first
    /// offers, by their identities.
    pub(crate) fn suggestions_listed(&self) -> Vec<String> {
        let open = self.open_offers();
        let listed = open
            .into_iter()
            .take(LISTED)
            .filter_map(|o| suggestion_number(&o.presents));
        let shown = self.reason.suggestions.shown();
        listed
            .filter_map(|n| shown.get(n - 1).map(|s| s.identity()))
            .collect()
    }

    /// Every offer not yet decided, the most pressing first: findings,
    /// claims not adopted, a text to mend, then the memex's suggestions,
    /// as ranked.
    pub fn open_offers(&self) -> Vec<Open> {
        let mut open = self.findings_open();
        open.extend(self.claims_open());
        open.extend(self.mends_open());
        open.extend(self.suggestions_open());
        open
    }

    /// Each finding under each item, in the outline's order, from the rule
    /// it is a finding of.
    fn findings_open(&self) -> Vec<Open> {
        let Some(e) = self.reason.eval[0].as_ref() else {
            return vec![];
        };
        let rules = self.notebook.rules(e);
        let marked = crate::marks::marks_by_item(e, &self.notebook.outline).into_iter();
        let marks = marked.flat_map(|(id, marks)| marks.into_iter().map(move |m| (id, m)));
        marks
            .filter_map(|(id, m)| self.finding(id, m, &rules))
            .collect()
    }

    /// A finding marked under an item, from the rule it is a finding of.
    fn finding(&self, id: u64, m: MarkLine, rules: &HashMap<u64, Rule>) -> Option<Open> {
        let rule = rules.get(&m.constraint?)?;
        let what = self.about(id, &m.text);
        let presents = rule.under(id);
        let source = rule.source.clone();
        Some(Open {
            kind: "finding".into(),
            what,
            source,
            presents,
        })
    }

    /// The claims of each item not yet adopted, from their sources.
    fn claims_open(&self) -> Vec<Open> {
        let claims = self.claims_open_by_item().into_iter();
        let claim = |(id, (_, source, fact))| self.claim_open(id, source, &fact);
        claims.map(claim).collect()
    }

    /// A claim an item states, not yet adopted, from its source.
    pub(crate) fn claim_open(&self, id: u64, source: String, fact: &Term) -> Open {
        let what = self.about(id, &self.restated(fact));
        let presents = Presentation::Item { pane: 0, id };
        Open {
            kind: "claim".into(),
            what,
            source,
            presents,
        }
    }

    /// The texts written in this session that did not read, each that
    /// the memex can mend and is still in the outline.
    fn mends_open(&self) -> Vec<Open> {
        let present = |(id, _): &(u64, &str)| self.notebook.outline.contains(*id);
        let each = self.session.repairs().filter(present);
        each.map(|(id, mended)| self.mend_open(id, mended))
            .collect()
    }

    /// The mend of an item's text the memex offers.
    fn mend_open(&self, id: u64, mended: &str) -> Open {
        let what = self.about(id, &format!("“{mended}”"));
        let presents = Presentation::Item { pane: 0, id };
        let source = "the memex's reading".into();
        Open {
            kind: "mend".into(),
            what,
            source,
            presents,
        }
    }

    /// The memex's suggestions shown, by their numbers.
    pub(crate) fn suggestions_open(&self) -> Vec<Open> {
        let shown = self.reason.suggestions.shown().into_iter();
        shown.zip(1..).map(|(s, n)| suggestion_open(s, n)).collect()
    }

    /// What an offer says of an item, after its number.
    fn about(&self, id: u64, what: &str) -> String {
        format!("{}: {what}", self.notebook.outline.number(id))
    }

    /// A fact in words, by the current world's labels.
    fn restated(&self, fact: &Term) -> String {
        let facts = self.reason.eval[0]
            .as_ref()
            .map(|e| e.facts.as_slice())
            .unwrap_or_default();
        crate::reason::restate(facts, crate::reason::claimed(fact))
    }
}

/// A suggestion of the memex's, by its number.
fn suggestion_open(s: &Suggestion, n: usize) -> Open {
    let kind = format!("suggestion {n}");
    let source = format!("the memex: {}", s.evidence);
    let presents = Presentation::Suggestion(n);
    Open {
        kind,
        what: s.describe(),
        source,
        presents,
    }
}

/// A change of a revision, from the world it is in.
pub(crate) fn change_open(seq: u64, what: String, world: &str) -> Open {
    let kind = format!("change {seq}");
    let source = format!("the {world}");
    let presents = Presentation::Change(seq);
    Open {
        kind,
        what,
        source,
        presents,
    }
}

/// A rule, as the findings of it name it: the entry that made it, where
/// they come from, and the item that states it, if one does.
struct Rule {
    entry: u64,
    source: String,
    item: Option<u64>,
}

impl Rule {
    /// A rule, as evaluated, in an outline.
    fn of(c: &ConstraintEval, outline: &Outline) -> Rule {
        let name = &c.constraint.name;
        let item = crate::marks::stating_rule(outline, name);
        let source = format!("your rule “{name}”");
        Rule {
            entry: c.seq,
            source,
            item,
        }
    }

    /// The rule, as presented under an item where a finding of it is
    /// marked.
    fn under(&self, id: u64) -> Presentation {
        let (entry, item) = (self.entry, self.item);
        Presentation::Constraint {
            pane: 0,
            entry,
            item,
            under: Some(id),
        }
    }
}

/// The number of the suggestion an offer presents, if it presents one.
fn suggestion_number(p: &Presentation) -> Option<usize> {
    match p {
        Presentation::Suggestion(n) => Some(*n),
        _ => None,
    }
}

impl Notebook {
    /// Each rule, by the entry that made it. Each is found once, not once
    /// for each finding, since finding the item that states a rule reads
    /// the text of every item.
    fn rules(&self, e: &WorldEval) -> HashMap<u64, Rule> {
        let rule = |c| Rule::of(c, &self.outline);
        e.constraints.iter().map(|c| (c.seq, rule(c))).collect()
    }
}
