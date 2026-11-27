//! Why a violation the actual world showed is gone (docs/design.md, The
//! journal format, `observed`): its rule no longer stands; an item whose
//! facts it rested on was deleted; such an item's text no longer reads as
//! anything; or else its facts were corrected. The memex knows which at
//! the evaluation that finds it gone, so the observed entry records it,
//! and the measures report need not replay the journal to work it out.

use crate::app::App;
use crate::marks::items_of;
use crate::outline::Outline;
use crate::reason::{ConstraintEval, Shown, Solution, WorldEval};
use format::bodies::Gone;

/// The violations a world shows, each with what it rests on.
fn shown(e: &WorldEval, outline: &Outline) -> Vec<Shown> {
    let each = e.constraints.iter();
    each.flat_map(|c| of_rule(c, outline)).collect()
}

/// A rule's violations, each with what it rests on.
fn of_rule(c: &ConstraintEval, outline: &Outline) -> Vec<Shown> {
    let named = c.violations_named();
    named.map(|v| shown_of(c.seq, v, outline)).collect()
}

/// A violation of a rule, by the rule's entry, named, with the items
/// whose facts its solution rests on.
fn shown_of(rule: u64, (name, sol): (String, &Solution), outline: &Outline) -> Shown {
    let items = items_of(sol, outline);
    Shown { name, rule, items }
}

impl App {
    /// In the actual world, why each violation last shown there is gone
    /// now, what it shows now being kept for next time; in a supposition,
    /// nothing.
    pub(crate) fn track_shown(&mut self) -> Vec<Gone> {
        let actual = self.reason.supposition.is_none();
        let Some(e) = self.reason.eval[0].as_ref().filter(|_| actual) else {
            return vec![];
        };
        let now = shown(e, &self.notebook.outline);
        let gone = self.gone_since(&now);
        self.reason.last_shown = now;
        gone
    }

    /// Why each violation last shown is not among those shown now.
    fn gone_since(&self, now: &[Shown]) -> Vec<Gone> {
        let kept = |s: &&Shown| now.iter().any(|n| n.name == s.name);
        let gone = self.reason.last_shown.iter().filter(|s| !kept(s));
        let why = |s: &Shown| Some(self.why_gone(s).to_string());
        let each = |s: &Shown| Gone {
            finding: s.name.clone(),
            why: why(s),
        };
        gone.map(each).collect()
    }

    /// Why a violation is gone: its rule, an item deleted, an item's text
    /// unread, or else its facts corrected.
    fn why_gone(&self, s: &Shown) -> &'static str {
        let outline = &self.notebook.outline;
        let items = || s.items.iter().copied();
        if !self.rule_stands(s.rule) {
            return "rule";
        }
        if items().any(|i| !outline.contains(i)) {
            return "deleted";
        }
        if items().any(|i| unread(outline, i)) {
            return "unread";
        }
        "corrected"
    }

    /// Whether a rule, by its entry, is still among the world's.
    fn rule_stands(&self, rule: u64) -> bool {
        let eval = self.reason.eval[0].as_ref();
        eval.is_some_and(|e| e.constraint_of(rule).is_some())
    }
}

/// Whether an item's text, once read, now reads as nothing.
fn unread(outline: &Outline, item: u64) -> bool {
    let reading = outline.readings.get(&item);
    reading.is_some_and(|r| r.entries.is_empty())
}
