//! What reasoning holds between changes: the open supposition, the
//! engines, what they found in the worlds shown, and the suggestions
//! (docs/structure.md § The code's structure).

use crate::engine::Engine;
use crate::reason::{Suggestion, WorldEval};
use crate::world::World;
use std::collections::BTreeSet;

/// An open supposition: a world sprouted from the actual one.
#[derive(Debug, Clone, PartialEq)]
pub struct Supposition {
    pub name: String,
    /// The head of the actual world it was sprouted from.
    pub actual: u64,
}

/// The worlds being reasoned over, and what reasoning found in them.
#[derive(Default)]
pub struct Reasoning {
    pub supposition: Option<Supposition>,
    /// The current world's engine, the actual world's while a supposition
    /// is open, and one for previews.
    engines: [Option<Engine>; 3],
    /// What reasoning found: [0] the current world, [1] the actual world
    /// while a supposition is open.
    pub eval: [Option<WorldEval>; 2],
    pub suggestions: Suggestions,
}

impl Reasoning {
    /// The current world's name: "actual", or the open supposition's.
    pub fn world(&self) -> &str {
        self.supposition.as_ref().map_or("actual", |s| &s.name)
    }

    /// The head of the actual world, when the current one's is the tip.
    pub fn actual_head(&self, tip: u64) -> u64 {
        self.supposition.as_ref().map_or(tip, |s| s.actual)
    }

    /// The heads of the worlds shown: the current one, and the actual one
    /// while a supposition is open.
    pub(crate) fn shown_heads(&self, tip: u64) -> Vec<u64> {
        match &self.supposition {
            Some(s) => vec![tip, s.actual],
            None => vec![tip],
        }
    }

    /// Engine k, started if it is not yet.
    pub(crate) fn engine(&mut self, k: usize) -> Result<&mut Engine, String> {
        crate::world::started(&mut self.engines[k])
    }

    /// A world, evaluated in engine k.
    pub(crate) fn evaluate(&mut self, k: usize, world: World) -> Result<WorldEval, String> {
        world.evaluate(&mut self.engines[k])
    }

    /// Whether the current world has an engine and facts.
    pub(crate) fn has_facts(&self) -> bool {
        let facts = self.eval[0].as_ref().is_some_and(|e| !e.facts.is_empty());
        self.engines[0].is_some() && facts
    }
}

/// The suggestions for the current world, and what the user has done with
/// them.
#[derive(Default)]
pub struct Suggestions {
    /// Best evidence first, less those dismissed.
    pub list: Vec<Suggestion>,
    /// Keys of dismissed suggestions, from the journal.
    pub dismissed: BTreeSet<String>,
    /// Identities of those recorded as offered (shown in the pane).
    offered: BTreeSet<String>,
    /// While the suggestion pane has focus, its order is kept: a suggestion
    /// never moves under the pointer. Not journaled, like a held key.
    frozen: Option<Vec<String>>,
}

impl Suggestions {
    /// The suggestions in the order shown: while the order is held, those
    /// already shown keep their places and new ones go last.
    pub fn shown(&self) -> Vec<&Suggestion> {
        let mut out: Vec<&Suggestion> = self.held().collect();
        for s in &self.list {
            if !out.iter().any(|o| o.identity() == s.identity()) {
                out.push(s);
            }
        }
        out.truncate(5); // few in view
        out
    }

    /// The suggestions of the held order that are still listed, in it.
    fn held(&self) -> impl Iterator<Item = &Suggestion> {
        let order = self.frozen.iter().flatten();
        order.filter_map(|k| self.list.iter().find(|s| &s.identity() == k))
    }

    /// Hold the order shown while the pane has focus, and let it go after.
    pub(crate) fn hold_order(&mut self, focused: bool) {
        self.frozen = match (focused, self.frozen.take()) {
            (true, Some(f)) => Some(f),
            (true, None) => Some(self.shown().iter().map(|s| s.identity()).collect()),
            (false, _) => None,
        };
    }

    /// The engine's suggestions, less those dismissed.
    pub(crate) fn set(&mut self, all: Vec<Suggestion>) {
        let dismissed = &self.dismissed;
        self.list = all
            .into_iter()
            .filter(|s| !dismissed.contains(&s.key()))
            .collect();
    }

    pub(crate) fn dismiss(&mut self, key: String) {
        self.dismissed.insert(key);
    }

    pub(crate) fn offer(&mut self, keys: impl IntoIterator<Item = String>) {
        self.offered.extend(keys);
    }

    /// The identities of those shown and not yet offered.
    pub(crate) fn unoffered(&self) -> Vec<String> {
        let shown = self.shown().into_iter().map(|s| s.identity());
        shown.filter(|k| !self.offered.contains(k)).collect()
    }
}
