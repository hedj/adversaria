//! Reasoning over the worlds shown, after each change: what each world's
//! nodes find, the suggestions, and what follows from them. Nothing here
//! changes the journal's content; it is computed from it.

use crate::app::{App, PaneKind};
use crate::engine::{Engine, told};
use crate::outline::Outline;
use crate::reason::{Suggestion, WorldEval};
use crate::world::World;
use format::terms::Term;
use serde_json::json;
use std::path::PathBuf;

impl App {
    /// Reason over the worlds shown: sync each world's facts into its engine,
    /// and read its constraints and derived values. Where the journal is
    /// after it is noted, failed or not, since what reasoning records of
    /// its own (what it showed and offered) moves no world.
    pub fn evaluate(&mut self) -> Result<(), String> {
        let done = self.reason_over_worlds();
        self.reason.at = Some(self.journal_at());
        done
    }

    /// Reason again if the journal moved since reasoning last ran, as the
    /// window does before it draws: constraints and derived values are
    /// computed from the journal, never stored in it. A failure becomes
    /// the message. Whether it reasoned.
    pub fn reason_if_moved(&mut self) -> bool {
        if self.reason.at == Some(self.journal_at()) {
            return false;
        }
        if let Err(e) = self.evaluate() {
            self.session.message = e;
        }
        true
    }

    /// Where the journal is: its tip and its length.
    fn journal_at(&self) -> (u64, usize) {
        (self.notebook.tip, self.notebook.entry_count())
    }

    fn reason_over_worlds(&mut self) -> Result<(), String> {
        self.evaluate_worlds()?;
        self.refresh_suggestions()?;
        self.after_evaluation();
        self.refresh_preview()
    }

    fn evaluate_worlds(&mut self) -> Result<(), String> {
        self.reason.eval = [None, None];
        for (k, head) in self
            .reason
            .shown_heads(self.notebook.tip)
            .into_iter()
            .enumerate()
        {
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
        let (outline, import) = self.world_parts(k, head);
        let facts = self.notebook.facts_in(head, &outline)?;
        Ok(World {
            facts,
            outline,
            import,
            at: self.notebook.entry_count(),
        })
    }

    fn world_parts(&self, k: usize, head: u64) -> (Outline, Option<(u64, PathBuf)>) {
        (self.world_outline(k, head), self.notebook.import_on(head))
    }

    /// The outline of the world at a head: the current one's is kept.
    fn world_outline(&self, k: usize, head: u64) -> Outline {
        if k == 0 {
            self.notebook.outline.clone()
        } else {
            self.notebook.outline_at(head)
        }
    }

    // Suggestions.

    fn refresh_suggestions(&mut self) -> Result<(), String> {
        let all = self.engine_suggestions()?;
        self.reason.suggestions.set(all);
        Ok(())
    }

    /// The current world's suggestions, best evidence first: its engine's,
    /// if it has facts or items, and its outline's unlinked mentions
    /// (crate::mentions). A mention's term sorts after every other kind's,
    /// so at equal evidence it follows them, as Prolog ordered them.
    fn engine_suggestions(&mut self) -> Result<Vec<Suggestion>, String> {
        let mut all = self.prolog_suggestions()?;
        all.extend(self.notebook.outline.mention_suggestions());
        all.sort_by_key(|s| std::cmp::Reverse(s.score));
        Ok(all)
    }

    /// The current world's engine's suggestions, if it has facts or items.
    fn prolog_suggestions(&mut self) -> Result<Vec<Suggestion>, String> {
        let items = !self.notebook.outline.top.is_empty();
        let asked = self.reason.has_facts() || (items && self.reason.started());
        if !asked {
            return Ok(vec![]);
        }
        let engine = self.reason.engine(0)?;
        engine.suggestions().map_err(told)
    }

    // What follows.

    fn after_evaluation(&mut self) {
        self.reason.title = self.notebook.outline.labelled_title();
        self.update_frozen();
        let stale = self.notebook.stale_results();
        self.session.set_stale(stale);
        self.on_its_own(|app| app.record_observations());
    }

    /// While the inspector shows a claim's questions, the preview of
    /// adopting it.
    fn refresh_preview(&mut self) -> Result<(), String> {
        self.reason.preview = None;
        if let Some(item) = self.inspected_claim() {
            self.reason.preview = Some((item, self.preview_adoption(item)?));
        }
        Ok(())
    }
}

impl App {
    /// The engine of the current world, for tests that count evaluations.
    pub fn current_engine(&mut self) -> Result<&mut Engine, String> {
        self.reason.engine(0)
    }

    /// Record in the journal what reasoning showed, when it changes: the
    /// current world's violations and disagreements, and the suggestions
    /// shown for the first time. A query and its answer are journal entries
    /// (§ The journal, rule 6), so the measures report can count them.
    pub(crate) fn record_observations(&mut self) {
        self.record_shown();
        self.record_offers();
    }

    /// Record what reasoning shows of the current world, when it changes,
    /// with why each violation the actual world showed is gone. Applying
    /// the entry records it as observed, so a write that fails is tried
    /// again.
    fn record_shown(&mut self) {
        let Some(now) = self.reason.eval[0].as_ref().map(WorldEval::observed) else {
            return;
        };
        let gone = self.track_shown();
        if gone.is_empty() && !self.session.observed_changed(&now) {
            return;
        }
        let world = self.reason.world().to_string();
        let mut body = json!({"violations": now.0, "disagreements": now.1, "world": world});
        if !gone.is_empty() {
            body["gone"] = json!(gone);
        }
        self.write_view("observed", body);
    }

    /// Record the suggestions drawn for the first time: those the pane
    /// shows, while it is shown, and those the notebook's inspector
    /// lists, while it is drawn.
    fn record_offers(&mut self) {
        let new = self
            .reason
            .suggestions
            .not_offered(self.suggestions_drawn());
        if new.is_empty() {
            return;
        }
        self.write_view("offer", json!({"keys": new}));
    }

    /// The suggestions drawn now, by their identities.
    fn suggestions_drawn(&self) -> Vec<String> {
        let mut drawn = Vec::new();
        if self.session.view.shows(PaneKind::Suggestions) {
            let shown = self.reason.suggestions.shown().into_iter();
            drawn.extend(shown.map(|s| s.identity()));
        }
        if self.notebook_drawn() {
            drawn.extend(self.suggestions_listed());
        }
        drawn
    }

    /// What adopting an item's claims would raise, computed in a spare
    /// engine: a preview of adopting it, as a world.
    pub(crate) fn preview_adoption(&mut self, item: u64) -> Result<String, String> {
        let claims: Vec<Term> = self.claims_of(item).into_iter().map(|c| c.2).collect();
        if claims.is_empty() {
            return Ok("no claims to adopt".into());
        }
        let next = self.notebook.entry_count() as u64 + 1;
        let facts = self.notebook.facts_adopting(next, claims)?;
        let constraints = self.notebook.outline.constraint_names();
        let raised = self.reason.raised(&facts, &constraints, next)?;
        Ok(adoption_preview(&raised))
    }

    /// Keep the suggestions' order while they are listed.
    pub(crate) fn update_frozen(&mut self) {
        let listed = self.session.view.shows(PaneKind::Suggestions);
        self.reason.suggestions.hold_order(listed);
    }
}

/// What adopting a claim would raise, as interrogation shows it.
fn adoption_preview(raised: &[String]) -> String {
    if raised.is_empty() {
        return "adopting it raises no violation".into();
    }
    format!("adopting it would raise {}", raised.join(", "))
}
