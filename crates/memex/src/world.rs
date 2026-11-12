//! A world to reason over, loaded into an engine, and what its nodes find
//! (docs/design.md § Dataflow).

use crate::engine::Engine;
use crate::outline::{Constraint, Outline};
use crate::reason::{ConstraintEval, Nodes, WorldEval, marks};
use format::terms::Term;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A world: the facts stated in it, its outline, the Wikidata subset
/// imported in it (its entry and blob), and the journal's length.
pub struct World {
    pub facts: Vec<(u64, Term)>,
    pub outline: Outline,
    pub import: Option<(u64, PathBuf)>,
    pub at: usize,
}

impl World {
    /// What reasoning finds in the world, in the engine in `slot`, which is
    /// started only if the world has facts.
    pub fn evaluate(self, slot: &mut Option<Engine>) -> Result<WorldEval, String> {
        if self.facts.is_empty() {
            return Ok(self.unread());
        }
        self.read_in(slot)
    }

    fn read_in(self, slot: &mut Option<Engine>) -> Result<WorldEval, String> {
        let engine = started(slot)?;
        self.load_and_read(engine)
    }

    fn unread(self) -> WorldEval {
        WorldEval {
            facts: self.facts,
            at: self.at,
            ..Default::default()
        }
    }

    fn load_and_read(self, engine: &mut Engine) -> Result<WorldEval, String> {
        self.load(engine)?;
        self.read(engine)
    }

    // Loading.

    fn load(&self, engine: &mut Engine) -> Result<(), String> {
        self.load_facts(engine)?;
        self.load_import(engine)
    }

    fn load_facts(&self, engine: &mut Engine) -> Result<(), String> {
        engine.sync(&self.with_outline()).map_err(|e| e.to_string())
    }

    fn load_import(&self, engine: &mut Engine) -> Result<(), String> {
        let import = self.import.as_ref().map(|(s, f)| (*s, f.as_path()));
        engine.wikidata(import).map_err(|e| e.to_string())
    }

    /// The world's facts, and the facts its outline states of its items.
    fn with_outline(&self) -> Vec<(u64, Term)> {
        let mut all = self.facts.clone();
        all.extend(outline_facts(&self.outline));
        all
    }

    // Reading.

    fn read(self, engine: &mut Engine) -> Result<WorldEval, String> {
        Ok(WorldEval {
            constraints: self.read_constraints(engine)?,
            nodes: self.read_nodes(engine),
            facts: self.facts,
            at: self.at,
        })
    }

    /// Each constraint still stated in this world, read.
    fn read_constraints(&self, engine: &mut Engine) -> Result<Vec<ConstraintEval>, String> {
        self.stated_constraints()
            .into_iter()
            .map(|(seq, c)| ConstraintEval::read(engine, seq, c))
            .collect()
    }

    /// The outline's constraints whose definitions are facts of this
    /// world: one retracted in it is not.
    fn stated_constraints(&self) -> Vec<(u64, &Constraint)> {
        self.outline
            .constraints
            .iter()
            .filter(|(seq, _)| self.states(**seq))
            .map(|(seq, c)| (*seq, c))
            .collect()
    }

    fn states(&self, seq: u64) -> bool {
        self.facts.iter().any(|(s, _)| *s == seq)
    }

    fn read_nodes(&self, engine: &mut Engine) -> Nodes {
        Nodes {
            derived: self.read_derived(engine),
            layers: self.read_layers(engine),
        }
    }

    /// Each derived item's answers, where its node can be read.
    fn read_derived(&self, engine: &mut Engine) -> BTreeMap<u64, Term> {
        self.outline
            .derived
            .keys()
            .filter_map(|item| engine.node(*item).ok().map(|v| (*item, v)))
            .collect()
    }

    /// Each layer's marks, where its node can be read.
    fn read_layers(&self, engine: &mut Engine) -> BTreeMap<u64, Vec<(Term, Vec<u64>)>> {
        self.layer_seqs()
            .into_iter()
            .filter_map(|seq| layer_marks(engine, seq).map(|m| (seq, m)))
            .collect()
    }

    fn layer_seqs(&self) -> Vec<u64> {
        self.outline
            .views
            .values()
            .flatten()
            .map(|l| l.seq)
            .collect()
    }
}

fn layer_marks(engine: &mut Engine, seq: u64) -> Option<Vec<(Term, Vec<u64>)>> {
    engine.node(seq).ok().map(|v| marks(&v))
}

/// The engine in a slot, started if there is none.
pub fn started(slot: &mut Option<Engine>) -> Result<&mut Engine, String> {
    if slot.is_none() {
        *slot = Some(Engine::start().map_err(|e| format!("cannot start Prolog: {e}"))?);
    }
    Ok(slot.as_mut().unwrap())
}

/// What an outline states of its items, as facts: item_text(Id, Text) for
/// each, and item_under(Id, Parent) for each that is not top-level.
pub fn outline_facts(o: &Outline) -> Vec<(u64, Term)> {
    o.in_order()
        .into_iter()
        .flat_map(|id| item_facts(o, id))
        .collect()
}

fn item_facts(o: &Outline, id: u64) -> Vec<(u64, Term)> {
    let mut out = vec![(id, item_text(o, id))];
    out.extend(item_under(o, id).map(|t| (id, t)));
    out
}

fn item_text(o: &Outline, id: u64) -> Term {
    Term::Compound(
        "item_text".into(),
        vec![Term::Int(id as i64), Term::Str(o.text(id))],
    )
}

fn item_under(o: &Outline, id: u64) -> Option<Term> {
    let p = o.parent_item(id)?;
    Some(Term::Compound(
        "item_under".into(),
        vec![Term::Int(id as i64), Term::Int(p as i64)],
    ))
}
