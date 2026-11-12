//! Reasoning over the worlds shown, after each change: what each world's
//! nodes find, the suggestions, and what follows from them. Nothing here
//! changes the journal's content; it is computed from it.

use crate::app::App;
use crate::outline::Outline;
use crate::reason::{Suggestion, WorldEval};
use crate::world::World;
use std::path::PathBuf;

impl App {
    /// Reason over the worlds shown: sync each world's facts into its engine,
    /// and read its constraints and derived values.
    pub fn evaluate(&mut self) -> Result<(), String> {
        self.evaluate_worlds()?;
        self.refresh_suggestions()?;
        self.after_evaluation();
        self.refresh_preview()
    }

    fn evaluate_worlds(&mut self) -> Result<(), String> {
        self.eval = [None, None];
        for (k, head) in self.shown_heads().into_iter().enumerate() {
            self.eval[k] = Some(self.evaluate_world(k, head)?);
        }
        Ok(())
    }

    /// The heads of the worlds shown: the current one, and the actual one
    /// while a supposition is open.
    fn shown_heads(&self) -> Vec<u64> {
        match &self.supposition {
            Some(s) => vec![self.tip, s.actual],
            None => vec![self.tip],
        }
    }

    /// The world at a head, evaluated in engine k.
    fn evaluate_world(&mut self, k: usize, head: u64) -> Result<WorldEval, String> {
        let world = self.world(k, head)?;
        world.evaluate(&mut self.engines[k])
    }

    fn world(&self, k: usize, head: u64) -> Result<World, String> {
        let (outline, import) = self.world_parts(k, head);
        Ok(World {
            facts: self.facts_at(head)?,
            outline,
            import,
            at: self.entry_count(),
        })
    }

    fn world_parts(&self, k: usize, head: u64) -> (Outline, Option<(u64, PathBuf)>) {
        (self.world_outline(k, head), self.import_on(head))
    }

    /// The outline of the world at a head: the current one's is kept.
    fn world_outline(&self, k: usize, head: u64) -> Outline {
        if k == 0 {
            self.outline.clone()
        } else {
            self.outline_at(head)
        }
    }

    // Suggestions.

    fn refresh_suggestions(&mut self) -> Result<(), String> {
        let all = self.engine_suggestions()?;
        self.suggestions = self.undismissed(all);
        Ok(())
    }

    /// The current world's engine's suggestions, if it has facts.
    fn engine_suggestions(&mut self) -> Result<Vec<Suggestion>, String> {
        if !self.has_facts() {
            return Ok(vec![]);
        }
        self.engine(0)?.suggestions().map_err(|e| e.to_string())
    }

    fn has_facts(&self) -> bool {
        self.engines[0].is_some() && self.eval[0].as_ref().is_some_and(|e| !e.facts.is_empty())
    }

    fn undismissed(&self, all: Vec<Suggestion>) -> Vec<Suggestion> {
        all.into_iter()
            .filter(|s| !self.dismissed.contains(&s.key()))
            .collect()
    }

    // What follows.

    fn after_evaluation(&mut self) {
        self.update_frozen();
        self.stale = self.stale_results();
        self.record_observations();
    }

    /// While a claim is interrogated, the preview of adopting it.
    fn refresh_preview(&mut self) -> Result<(), String> {
        self.preview = None;
        if let Some(item) = self.view.interrogate {
            self.preview = Some(self.preview_adoption(item)?);
        }
        Ok(())
    }
}
