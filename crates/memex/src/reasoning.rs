//! What reasoning holds between changes: the open supposition, the
//! engines, what they found in the worlds shown, and the suggestions
//! (docs/structure.md § The code's structure).

use crate::ace::Ace;
use crate::engine::Engine;
use crate::notebook::strings;
use crate::reason::{Suggestion, WorldEval};
use crate::world::World;
use format::record::Entry;
use serde_json::Value;
use std::collections::BTreeSet;

/// An open supposition: a world sprouted from the actual one.
#[derive(Debug, Clone, PartialEq)]
pub struct Supposition {
    pub name: String,
    /// The head of the actual world it was sprouted from.
    pub actual: u64,
    /// The suppose entry that opened it.
    opened: u64,
}

impl Supposition {
    /// The supposition a suppose entry opens, from the world at its parent.
    fn of(e: &Entry, b: &Value) -> Supposition {
        let name = b["name"].as_str().unwrap_or("supposed").to_string();
        let actual = e.parent_seq().unwrap_or(1);
        Supposition {
            name,
            actual,
            opened: e.seq,
        }
    }
}

/// The worlds being reasoned over, and what reasoning found in them.
#[derive(Default)]
pub struct Reasoning {
    pub supposition: Option<Supposition>,
    /// The supposition the last undo left, which a redo of its suppose
    /// entry opens again.
    undone_supposition: Option<Supposition>,
    /// The current world's engine, the actual world's while a supposition
    /// is open, and one for previews.
    engines: [Option<Engine>; 3],
    /// What reasoning found: [0] the current world, [1] the actual world
    /// while a supposition is open.
    pub eval: [Option<WorldEval>; 2],
    /// The item labelled `title`, as of the last evaluation.
    pub title: Option<u64>,
    /// The preview of adopting a claim item's claims, and the item: the
    /// violations adopting them would raise.
    pub preview: Option<(u64, String)>,
    pub suggestions: Suggestions,
    /// The parser of controlled English, started when first needed.
    ace: Option<Ace>,
}

impl Reasoning {
    /// Apply an entry: a supposition opens or closes, and a suggestion is
    /// dismissed or offered.
    pub(crate) fn apply(&mut self, e: &Entry) {
        let Some(b) = e.body() else { return };
        let opened = |s: &Option<Supposition>, key: &str| {
            s.as_ref()
                .is_some_and(|s| b[key].as_u64() == Some(s.opened))
        };
        match (e.kind(), b["op"].as_str()) {
            // Undoing the suppose leaves its world; redoing it returns.
            ("undo", _) if opened(&self.supposition, "undoes") => {
                self.undone_supposition = self.supposition.take();
            }
            ("redo", _) if opened(&self.undone_supposition, "redoes") => {
                self.supposition = self.undone_supposition.take();
            }
            ("view", Some("world")) => self.supposition = None,
            ("view", Some("dismiss")) => self.suggestions.dismiss(b["key"].as_str()),
            ("view", Some("offer")) => self.suggestions.offer(strings(&b["keys"])),
            ("suppose", _) => self.supposition = Some(Supposition::of(e, b)),
            _ => {}
        }
    }

    /// The current world's name: "actual", or the open supposition's.
    pub fn world(&self) -> &str {
        self.supposition.as_ref().map_or("actual", |s| &s.name)
    }

    /// Whether the open supposition is a document's revision, whose
    /// changes fill the screen in place of the outline.
    pub fn in_revision(&self) -> bool {
        let sup = self.supposition.as_ref();
        sup.is_some_and(|s| s.name.starts_with("revision of "))
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
    /// The parser of controlled English, started if it is not running.
    pub(crate) fn ace(&mut self) -> Result<&mut Ace, String> {
        if self.ace.as_mut().is_some_and(Ace::is_dead) {
            self.ace = None;
        }
        if self.ace.is_none() {
            let started = Ace::start().map_err(|e| format!("cannot start the parser: {e}"))?;
            self.ace = Some(started);
        }
        Ok(self.ace.as_mut().unwrap())
    }

    pub(crate) fn engine(&mut self, k: usize) -> Result<&mut Engine, String> {
        crate::world::started(&mut self.engines[k])
    }

    /// A world, evaluated in engine k.
    pub(crate) fn evaluate(&mut self, k: usize, world: World) -> Result<WorldEval, String> {
        world.evaluate(&mut self.engines[k])
    }

    /// How many marks a layer, by its seq, gives in the current world.
    pub(crate) fn mark_count(&self, layer: u64) -> usize {
        self.eval[0]
            .as_ref()
            .map_or(0, |e| e.nodes.marks(layer).len())
    }

    /// Whether the current world has an engine and facts.
    /// Whether the current world's engine is running.
    pub(crate) fn started(&self) -> bool {
        self.engines[0].is_some()
    }

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
    /// Identities of those recorded as offered (drawn on the screen).
    offered: BTreeSet<String>,
    /// While the suggestions are listed, their order is kept: a suggestion
    /// never moves under the pointer, and the number that adopts it stays
    /// its number. Not journaled, like a held key.
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

    /// Hold the order shown while the suggestions are listed, and let it
    /// go after.
    pub(crate) fn hold_order(&mut self, listed: bool) {
        self.frozen = match (listed, self.frozen.take()) {
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

    pub(crate) fn dismiss(&mut self, key: Option<&str>) {
        self.dismissed.extend(key.map(String::from));
    }

    pub(crate) fn offer(&mut self, keys: impl IntoIterator<Item = String>) {
        self.offered.extend(keys);
    }

    /// Of these identities, those not yet recorded as offered, each once
    /// in its first place.
    pub(crate) fn not_offered(&self, keys: Vec<String>) -> Vec<String> {
        let mut seen = self.offered.clone();
        keys.into_iter()
            .filter(|k| seen.insert(k.clone()))
            .collect()
    }
}
