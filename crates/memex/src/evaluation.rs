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
        self.reason.eval = [None, None];
        for (k, head) in self.reason.shown_heads(self.tip).into_iter().enumerate() {
            self.reason.eval[k] = Some(self.evaluate_world(k, head)?);
        }
        Ok(())
    }

    /// The world at a head, evaluated in engine k.
    fn evaluate_world(&mut self, k: usize, head: u64) -> Result<WorldEval, String> {
        let world = self.world(k, head)?;
        self.reason.evaluate(k, world)
    }

    fn world(&self, k: usize, head: u64) -> Result<World, String> {
        let facts = self.facts_at(head)?;
        let (outline, import) = self.world_parts(k, head);
        Ok(World {
            facts,
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
        self.reason.suggestions.set(all);
        Ok(())
    }

    /// The current world's engine's suggestions, if it has facts.
    fn engine_suggestions(&mut self) -> Result<Vec<Suggestion>, String> {
        if !self.reason.has_facts() {
            return Ok(vec![]);
        }
        let engine = self.reason.engine(0)?;
        engine.suggestions().map_err(|e| e.to_string())
    }

    // What follows.

    fn after_evaluation(&mut self) {
        self.update_frozen();
        let stale = self.stale_results();
        self.session.set_stale(stale);
        self.record_observations();
    }

    /// While a claim is interrogated, the preview of adopting it.
    fn refresh_preview(&mut self) -> Result<(), String> {
        self.reason.preview = None;
        if let Some(item) = self.session.view.interrogate {
            self.reason.preview = Some(self.preview_adoption(item)?);
        }
        Ok(())
    }
}
