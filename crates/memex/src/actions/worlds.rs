//! Commands that move along the journal's history and between worlds
//! (docs/guide.md, § 2.4, Worlds and history).

use crate::app::{App, Mode};
use crate::reason::{Contradicted, direct_conflicts};
use format::terms::{self, Term};
use serde_json::{Value, json};

impl App {
    pub(crate) fn cmd_undo(&mut self, _: &[&str], _: u64) -> Result<String, String> {
        let t = self.undoable().ok_or("nothing to undo on this branch")?;
        self.write(self.parent_of(t), "undo", json!({"undoes": t}))?;
        self.read_changed();
        Ok(format!("undid entry {t}"))
    }

    /// The entry undo would undo: the last on this branch that changed
    /// content, unless it is the journal's first.
    /// A text edit and its reading are one change: undo takes back both.
    fn undoable(&self) -> Option<u64> {
        let t = self.content_tip(self.notebook.tip).filter(|t| *t > 1)?;
        Some(self.read_from(t).unwrap_or(t))
    }

    fn parent_of(&self, seq: u64) -> Option<u64> {
        self.notebook.entry(seq).and_then(|e| e.parent_seq())
    }

    pub(crate) fn cmd_redo(&mut self, _: &[&str], _: u64) -> Result<String, String> {
        let Some(&t) = self.notebook.redo.last() else {
            return Err("nothing to redo".into());
        };
        self.write(Some(t), "redo", json!({"redoes": t}))?;
        self.read_changed();
        Ok(format!("redid entry {t}"))
    }

    /// Ask before retracting an entry, listing what depends on it: the
    /// entry named, or, with none named, the target's own last entry that
    /// can still be retracted (an adoption, a fact or a layer of it).
    pub(crate) fn cmd_retract(&mut self, words: &[&str], _: u64) -> Result<String, String> {
        let t = match words.is_empty() {
            true => self.targets_last().ok_or(
                "the target has nothing to retract: no adoption, fact or layer of its own",
            )?,
            false => entry_number(words)
                .ok_or("retract (entry) seq, or retract alone for the target's last")?,
        };
        let dependents = self.retractable(t)?;
        self.ask_retract(t, dependents);
        Ok(String::new())
    }

    /// The target's own last entry on this branch that can still be
    /// retracted: an adoption of its claims, a fact stated on it (not one
    /// its text was read as, which its text keeps), or a layer of its view.
    fn targets_last(&self) -> Option<u64> {
        let item = self.cursor();
        let path = self.notebook.path(self.notebook.tip);
        let own = path.iter().rev().filter(|e| self.owns(item, e));
        own.map(|e| e.seq).find(|s| self.retractable(*s).is_ok())
    }

    /// Whether an entry is an item's own: an adoption of its claims, a
    /// fact stated on it and not read from its text, or a layer of it.
    fn owns(&self, item: u64, e: &format::record::Entry) -> bool {
        let outline = &self.notebook.outline;
        let own = outline.states_own(item, e.seq) || outline.is_layer_of(item, e.seq);
        own || crate::actions::adopts(e, item)
    }

    fn ask_retract(&mut self, target: u64, dependents: Vec<u64>) {
        self.session.message.clear();
        self.session.mode = Mode::ConfirmRetract { target, dependents };
    }

    /// Suppose a statement as the Suppose key does (App::suppose_text).
    pub(crate) fn cmd_suppose(&mut self, words: &[&str], _: u64) -> Result<String, String> {
        let text = words.join(" ");
        if text.is_empty() {
            return Err("suppose (that) a statement, or a name for the world".into());
        }
        self.session.mode = self.suppose_text(text, None);
        Ok(String::new())
    }

    /// Return to the actual world as it was; the supposition stays in the
    /// journal.
    pub(crate) fn cmd_close(&mut self, _: &[&str], _: u64) -> Result<String, String> {
        let (name, actual) = self.supposed()?;
        self.leave_world(actual, "close")?;
        Ok(format!("closed “{name}”; it stays in the journal"))
    }

    /// Make the supposed world the actual one.
    pub(crate) fn cmd_commit(&mut self, _: &[&str], _: u64) -> Result<String, String> {
        let (name, _) = self.supposed()?;
        self.leave_world(self.notebook.tip, "commit")?;
        Ok(format!("“{name}” is now the actual world"))
    }

    /// The open supposition's name, and the actual world's head it left.
    fn supposed(&self) -> Result<(String, u64), String> {
        self.reason
            .supposition
            .as_ref()
            .map(|s| (s.name.clone(), s.actual))
            .ok_or("no supposition is open".into())
    }

    /// Write the view entry that ends a supposition, after an entry.
    fn leave_world(&mut self, parent: u64, op: &str) -> Result<(), String> {
        self.stop_interrogating();
        self.flush_text();
        let body = self.world_body(op);
        self.write(Some(parent), "view", body)?;
        Ok(())
    }

    fn stop_interrogating(&mut self) {
        self.session.view.interrogate = None;
    }

    fn world_body(&self, op: &str) -> Value {
        self.view_body("world", json!({op: true}))
    }
}

/// An entry's sequence number, as written after retract ("12" or "#12").
fn entry_number(words: &[&str]) -> Option<u64> {
    words.first()?.trim_start_matches('#').parse().ok()
}

impl App {
    /// Sprout a world in which `fact` holds; then set aside, in that world
    /// only, each belief it directly contradicts (the supposition is
    /// entrenched over them). See `direct_conflicts` for what "directly"
    /// means; a violation it leaves is shown, for the user to resolve.
    pub(crate) fn suppose(
        &mut self,
        name: &str,
        fact: &Term,
        english: Option<String>,
    ) -> Result<String, String> {
        let f = terms::encode(fact).map_err(|e| e.0)?;
        let body = crate::keys::supposed_body(name, english, Some(f));
        let s = self.write_here("suppose", body)?;
        let (set_aside, left) = self.set_aside_conflicts(s.seq)?;
        self.evaluate()?;
        let places: Vec<String> = set_aside.iter().map(|t| self.fact_place(*t)).collect();
        Ok(with_left(supposed(&places), &left))
    }

    /// Retract, round by round, five at most, each belief that the fact of
    /// an entry directly contradicts: those set aside, and the violations
    /// left.
    fn set_aside_conflicts(&mut self, seq: u64) -> Result<(Vec<u64>, Vec<String>), String> {
        let mut set_aside = Vec::new();
        let mut left = Vec::new();
        for _ in 0..5 {
            let targets;
            (targets, left) = self.retractable_conflicts(seq)?;
            if targets.is_empty() {
                break;
            }
            self.retract_all(&targets)?;
            set_aside.extend(targets.into_iter().map(|(t, _)| t));
        }
        Ok((set_aside, left))
    }

    /// After reasoning again, the beliefs that the fact of an entry directly
    /// contradicts and that can be retracted, and the violations left.
    fn retractable_conflicts(
        &mut self,
        seq: u64,
    ) -> Result<(Vec<Contradicted>, Vec<String>), String> {
        self.evaluate()?;
        let eval = self.reason.eval[0].as_ref().ok_or("no evaluation")?;
        let (mut targets, left) = direct_conflicts(&eval.constraints, seq);
        targets.retain(|(t, _)| self.retractable(*t).is_ok());
        Ok((targets, left))
    }

    /// The readings of a typed supposition. A fact written as Prolog is
    /// exact: it is its only reading, or it has none when the world holds
    /// it already. Other text is read through the world's own facts
    /// (reason::readings).
    pub(crate) fn suppose_readings(&mut self, facts: &[(u64, Term)], text: &str) -> Vec<Term> {
        let Some(typed) = self.typed_fact(text) else {
            return crate::reason::readings(facts, text);
        };
        let held = facts.iter().any(|(_, f)| *f == typed);
        if held { vec![] } else { vec![typed] }
    }

    /// The compound fact a text is, if it is written as one: a name, then
    /// its arguments in brackets. English such as "food is 900" is Prolog
    /// too (is(food, 900)), so the form is checked first.
    fn typed_fact(&mut self, text: &str) -> Option<Term> {
        let text = text.trim().trim_end_matches('.');
        let (name, _) = text.split_once('(')?;
        let named = name.starts_with(|c: char| c.is_ascii_lowercase())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !named || !text.ends_with(')') {
            return None;
        }
        self.parse_fact(text).ok()
    }

    /// Retract each belief, naming the constraints it was contradicted
    /// under, so that a world can say which constraint each answered.
    pub(crate) fn retract_all(&mut self, targets: &[Contradicted]) -> Result<(), String> {
        for (t, constraints) in targets {
            let body = json!({"target": t, "constraints": constraints});
            self.write_here("retract", body)?;
        }
        Ok(())
    }
}

/// What a supposition says it set aside, at these places.
fn supposed(places: &[String]) -> String {
    if places.is_empty() {
        return "supposed; nothing set aside".into();
    }
    format!("supposed; set aside in this world: {}", places.join(", "))
}

/// A reply, and the violations left for the user, if any are.
pub(crate) fn with_left(said: String, left: &[String]) -> String {
    if left.is_empty() {
        return said;
    }
    format!("{said}; left for you: {}", left.join(", "))
}
