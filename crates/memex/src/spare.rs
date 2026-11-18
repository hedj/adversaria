//! The spare engine: what facts not yet written would do, worked out in
//! the engine kept for trials (engine 2 of crate::reasoning), so the
//! worlds' own engines are left as they are: the violations adopting an
//! item's claims would raise (interrogation's preview), and what each
//! constraint finds with a trial's facts (`set`, finding the beliefs a new
//! fact contradicts).

use crate::engine::{Engine, told};
use crate::notebook::Trial;
use crate::outline::Constraint;
use crate::reason::{ConstraintEval, raised_by, violations_of};
use crate::reasoning::Reasoning;
use format::terms::Term;

impl Reasoning {
    /// The violations that facts would raise, each resting on the entry
    /// `by`, counted by constraint: worked out in the spare engine.
    pub(crate) fn raised(
        &mut self,
        facts: &[(u64, Term)],
        constraints: &[(u64, String)],
        by: u64,
    ) -> Result<Vec<String>, String> {
        let engine = self.engine(2)?;
        engine.sync(facts).map_err(told)?;
        let raised = constraints.iter().filter_map(|c| raised_in(engine, c, by));
        Ok(raised.collect())
    }

    /// What each constraint finds, with a trial's facts, in the spare
    /// engine.
    pub(crate) fn try_out(&mut self, trial: &Trial) -> Result<Vec<ConstraintEval>, String> {
        let engine = self.engine(2)?;
        engine.sync(&trial.facts).map_err(told)?;
        let found = trial
            .constraints
            .iter()
            .filter_map(|c| evaluated_in(engine, c));
        Ok(found.collect())
    }
}

/// The violations of one constraint, by its entry and name, that the
/// entry `by` raises in an engine, if it raises any.
fn raised_in(engine: &mut Engine, constraint: &(u64, String), by: u64) -> Option<String> {
    let node = engine.node(constraint.0).ok()?;
    violations_of(raised_by(&node, by), &constraint.1)
}

/// What a constraint, by its entry, finds in an engine, if it can say.
fn evaluated_in(
    engine: &mut Engine,
    (seq, constraint): &(u64, Constraint),
) -> Option<ConstraintEval> {
    let node = engine.node(*seq).ok()?;
    Some(ConstraintEval::of(*seq, constraint.clone(), &node))
}
