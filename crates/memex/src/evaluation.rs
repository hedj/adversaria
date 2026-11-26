//! Reasoning over the worlds shown, after each change: what each world's
//! nodes find, the suggestions, and what follows from them. Nothing here
//! changes the journal's content; it is computed from it.

use crate::app::{App, PaneKind};
use crate::engine::{Engine, told};
use crate::notebook::Notebook;
use crate::reason::{Suggestion, WorldEval};
use crate::world::World;
use format::record::EntryKind;
use format::terms::Term;
use serde_json::{Value, json};
use std::borrow::Cow;

impl App {
    /// Reason over the worlds shown: sync each world's facts into its engine,
    /// and read its constraints and derived values. Where the journal is
    /// after it is noted, failed or not, since what reasoning records of
    /// its own (what it showed and offered) moves no world.
    pub fn evaluate(&mut self) -> Result<(), String> {
        let done = self.reason_over_worlds();
        self.reason.at = Some(self.notebook.journal_at());
        done
    }

    /// Reason again if the journal moved since reasoning last ran, as the
    /// window does before it draws: constraints and derived values are
    /// computed from the journal, never stored in it. A failure becomes
    /// the message. Whether it reasoned.
    pub fn reason_if_moved(&mut self) -> bool {
        if self.reason.at == Some(self.notebook.journal_at()) {
            return false;
        }
        if let Err(e) = self.evaluate() {
            self.session.message = e;
        }
        true
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
        let world = world_of(&self.notebook, k, head)?;
        self.reason.evaluate(k, world)
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
        self.reason.mentions.update(&self.notebook.outline);
        all.extend(self.reason.mentions.suggestions(&self.notebook.outline));
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

    /// Record in the journal what reasoning found, when it changes: the
    /// current world's violations and disagreements, and each suggestion
    /// it made for the first time, in its words. A query and its answer
    /// are journal entries (§ The journal, rule 6), so the measures report
    /// can count them. A record holds no view: it says what reasoning
    /// found, not what the screen showed (version 3).
    pub(crate) fn record_observations(&mut self) {
        self.record_shown();
        self.record_offers();
    }

    /// Write a record of what reasoning found: a view entry, of no view.
    fn write_record(&mut self, op: &str, mut body: Value) {
        body["op"] = json!(op);
        let written = self.write_here(EntryKind::View, body);
        self.session.report(written);
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
        self.write_record("observed", body);
    }

    /// Record each suggestion reasoning made for the first time: its
    /// identity, which an adoption of it names, and its words.
    fn record_offers(&mut self) {
        let made = self.reason.suggestions.list.iter();
        let ids = made.map(|s| s.identity()).collect();
        let new = self.reason.suggestions.not_offered(ids);
        if new.is_empty() {
            return;
        }
        let said: Vec<String> = new
            .iter()
            .filter_map(|k| self.said_of_suggestion(k))
            .collect();
        self.write_record("offer", json!({"keys": new, "said": said}));
    }

    /// A suggestion's words, by its identity.
    fn said_of_suggestion(&self, identity: &str) -> Option<String> {
        let mut list = self.reason.suggestions.list.iter();
        list.find(|s| s.identity() == identity)
            .map(|s| s.describe())
    }

    /// What adopting an item's claims would raise, computed in a spare
    /// engine: a preview of adopting it, as a world.
    pub(crate) fn preview_adoption(&mut self, item: u64) -> Result<String, String> {
        let claims: Vec<Term> = self
            .notebook
            .claims_of(item)
            .into_iter()
            .map(|c| c.2)
            .collect();
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

/// The world at a head, for engine k. The current world's (k = 0) outline
/// is the notebook's own, borrowed; another world's is replayed.
fn world_of(notebook: &Notebook, k: usize, head: u64) -> Result<World<'_>, String> {
    let outline = match k {
        0 => Cow::Borrowed(&notebook.outline),
        _ => Cow::Owned(notebook.outline_at(head)),
    };
    let facts = notebook.facts_in(head, &outline)?;
    let import = notebook.import_on(head);
    let at = notebook.entry_count();
    Ok(World {
        facts,
        outline,
        import,
        at,
    })
}

impl Notebook {
    /// Where the journal is: its tip and its length.
    pub(crate) fn journal_at(&self) -> (u64, usize) {
        (self.tip, self.entry_count())
    }
}
