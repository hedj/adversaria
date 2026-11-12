//! Commands that move along the journal's history and between worlds
//! (docs/guide.md, § 2.4, Worlds and history).

use crate::app::{App, Mode};
use serde_json::{Value, json};

impl App {
    pub(crate) fn cmd_undo(&mut self, _: &[&str], _: u64) -> Result<(), String> {
        let t = self.undoable().ok_or("nothing to undo on this branch")?;
        self.write(self.parent_of(t), "undo", json!({"undoes": t}))?;
        self.message = format!("undid entry {t}");
        Ok(())
    }

    /// The entry undo would undo: the last on this branch that changed
    /// content, unless it is the journal's first.
    fn undoable(&self) -> Option<u64> {
        self.content_tip(self.tip).filter(|t| *t > 1)
    }

    fn parent_of(&self, seq: u64) -> Option<u64> {
        self.entry(seq).and_then(|e| e.parent)
    }

    pub(crate) fn cmd_redo(&mut self, _: &[&str], _: u64) -> Result<(), String> {
        let Some(&t) = self.redo.last() else {
            return Err("nothing to redo".into());
        };
        self.write(Some(t), "redo", json!({"redoes": t}))?;
        self.message = format!("redid entry {t}");
        Ok(())
    }

    /// Ask before retracting an entry, listing what depends on it.
    pub(crate) fn cmd_retract(&mut self, words: &[&str], _: u64) -> Result<(), String> {
        let t = entry_number(words).ok_or("retract (entry) seq")?;
        let dependents = self.retractable(t)?;
        self.ask_retract(t, dependents);
        Ok(())
    }

    fn ask_retract(&mut self, target: u64, dependents: Vec<u64>) {
        self.message.clear();
        self.mode = Mode::ConfirmRetract { target, dependents };
    }

    pub(crate) fn cmd_suppose(&mut self, words: &[&str], _: u64) -> Result<(), String> {
        let n = words.join(" ");
        if n.is_empty() {
            return Err("suppose (that) a name for the world".into());
        }
        self.write_here("suppose", json!({"name": n}))?;
        Ok(())
    }

    /// Return to the actual world as it was; the supposition stays in the
    /// journal.
    pub(crate) fn cmd_close(&mut self, _: &[&str], _: u64) -> Result<(), String> {
        let (name, actual) = self.supposed()?;
        self.leave_world(actual, "close")?;
        self.message = format!("closed “{name}”; it stays in the journal");
        Ok(())
    }

    /// Make the supposed world the actual one.
    pub(crate) fn cmd_commit(&mut self, _: &[&str], _: u64) -> Result<(), String> {
        let (name, _) = self.supposed()?;
        self.leave_world(self.tip, "commit")?;
        self.message = format!("“{name}” is now the actual world");
        Ok(())
    }

    /// The open supposition's name, and the actual world's head it left.
    fn supposed(&self) -> Result<(String, u64), String> {
        self.supposition
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
        self.view.interrogate = None;
    }

    fn world_body(&self, op: &str) -> Value {
        self.view.to_json("world", json!({op: true}))
    }
}

/// An entry's sequence number, as written after retract ("12" or "#12").
fn entry_number(words: &[&str]) -> Option<u64> {
    words.first()?.trim_start_matches('#').parse().ok()
}
