//! Reading what the user writes (docs/design.md § Reading controlled
//! English): an item's text, when it is ACE, is read as the statements it
//! makes, as written, so that nothing is written twice, in English and
//! again in Prolog. Each statement goes through the command that states
//! it, with that command's checks. A reading is recorded by a `read` edit
//! entry, which names the text edit it reads and the entries its
//! statements are in; when the text changes, those are retracted and the
//! text is read again. Text waits to be read until the user pauses
//! (App::idle), as APE is slower than a keystroke. Only text typed is read: an imported document is a
//! source's claims, not the user's beliefs.

use super::inline::unbracketed;
use crate::ace::{Kind, Reading, Said, Unread, words_of};
use crate::app::App;
use crate::engine::told;
use serde_json::json;

impl App {
    /// Let an item's text, written by the entry `of`, wait to be read at
    /// the next pause (App::idle). APE is started now, so that it is ready
    /// by then.
    pub(crate) fn will_read(&mut self, item: u64, of: u64) {
        self.session.wait_to_read(item, of);
        let _started = self.reason.ace();
    }

    /// Read each item's text that waits to be read.
    pub(crate) fn read_waiting(&mut self) {
        for (item, of) in self.session.take_to_read() {
            self.read_item(item, of);
        }
    }

    /// Read an item's text, written by the entry `of`, unless it is what
    /// was last read for it. Without APE nothing is read.
    fn read_item(&mut self, item: u64, of: u64) {
        let text = self.notebook.outline.text(item);
        let read = self.notebook.outline.readings.get(&item);
        if read.is_some_and(|r| r.text == text) {
            return;
        }
        let linked = self.link_inline(item) | self.link_to(item);
        let w = Written {
            item,
            of,
            text: &text,
            linked,
        };
        if text.trim().is_empty() {
            self.session.set_prose(None);
            return self.unread(&w);
        }
        let Ok(reading) = self.ace_read(&unbracketed(&text)) else {
            return;
        };
        match reading {
            Reading::Read(_, said) => self.state_reading(&w, &said),
            not_read => self.unread_saying_why(&w, &not_read),
        }
    }

    /// A text not read, and the hint of why at the cursor.
    fn unread_saying_why(&mut self, w: &Written, reading: &Reading) {
        self.session.set_prose(Some((w.item, hint_of(reading))));
        self.unread(w);
    }

    /// Let each item whose text is not what was last read for it wait to
    /// be read again: after an undo or a redo, which change text without
    /// typing.
    pub(crate) fn read_changed(&mut self) {
        for item in self.notebook.outline.changed_since_read() {
            self.will_read(item, self.notebook.tip);
        }
    }

    /// What the notebook reads in a text, with the current world's words.
    fn ace_read(&mut self, text: &str) -> Result<Reading, String> {
        let facts = self.notebook.facts_now()?;
        let ace = self.reason.ace()?;
        ace.read_with(words_of(&facts), text).map_err(told)
    }

    /// State what an item's text was read as, in place of what it was read
    /// as before; say what the commands that state it said, as if typed
    /// (a constraint kept, a statement refused, and why).
    fn state_reading(&mut self, w: &Written, said: &[Said]) {
        self.session.set_prose(None);
        self.retract_reading(w.item);
        let (edits, told) = self.state_all(w.item, w.text, said);
        self.record_reading(w, edits);
        self.session.message = told.join("; ");
    }

    /// State each statement on the item: the edit entries written, and
    /// what the commands said.
    fn state_all(&mut self, item: u64, text: &str, said: &[Said]) -> (Vec<u64>, Vec<String>) {
        let first = self.notebook.next_seq();
        let told = self.outcomes(item, named(text, said));
        (self.edits_since(first), told)
    }

    /// State each named statement on the item: what each command said,
    /// when it said anything.
    fn outcomes(&mut self, item: u64, named: Vec<(Said, String)>) -> Vec<String> {
        let told = named
            .into_iter()
            .map(|(s, name)| self.state(item, &s, &name));
        let told = told.map(|r| r.unwrap_or_else(|why| why));
        told.filter(|t| !t.is_empty()).collect()
    }

    /// The edit entries written from entry `first` on.
    fn edits_since(&self, first: u64) -> Vec<u64> {
        let entries = self.notebook.journal.entries();
        let written = entries.get(first as usize - 1..).unwrap_or(&[]);
        let edits = written.iter().filter(|e| e.kind() == "edit");
        edits.map(|e| e.seq).collect()
    }

    /// An item's text not read: what it was read as before is taken back.
    /// A reading of nothing is recorded if anything was taken back or its
    /// links changed, so that undo takes the text and all that followed
    /// from it back together.
    fn unread(&mut self, w: &Written) {
        let had = self.stated_by_reading(w.item);
        if had {
            self.retract_reading(w.item);
        }
        if had || w.linked {
            self.record_reading(w, vec![]);
        }
    }

    /// Whether an item's last reading stated anything.
    fn stated_by_reading(&self, item: u64) -> bool {
        let read = self.notebook.outline.readings.get(&item);
        read.is_some_and(|r| !r.entries.is_empty())
    }

    /// One statement, through the command that states it, on the item.
    fn state(&mut self, item: u64, said: &Said, name: &str) -> Result<String, String> {
        let t = said.text.as_str();
        match &said.kind {
            Kind::Fact => self.cmd_fact(&[t], item),
            Kind::Claim(source) => self.cmd_claim(&[&format!("{t} from {source}")], item),
            Kind::Rule => self.cmd_rule(&[t], item),
            Kind::Constraint => self.cmd_constraint(&[&format!("{name}: {t}")], item),
            Kind::Question => self.ask(t, item),
        }
    }

    /// Take back the entries an item's text was last read as, those the
    /// branch has not taken back already.
    fn retract_reading(&mut self, item: u64) {
        let Some(read) = self.notebook.outline.readings.get(&item) else {
            return;
        };
        let branch = self.notebook.branch(self.notebook.tip);
        let live: Vec<u64> = read
            .entries
            .iter()
            .copied()
            .filter(|s| !branch.retracts(*s))
            .collect();
        for target in live {
            let written = self.write_here("retract", json!({ "target": target }));
            self.session.report(written);
        }
    }

    fn record_reading(&mut self, w: &Written, entries: Vec<u64>) {
        let (item, of, text) = (w.item, w.of, w.text);
        let body = json!({"op": "read", "item": item, "of": of, "text": text, "entries": entries});
        let written = self.write_here("edit", body);
        self.session.report(written);
    }

    /// The first entry of what a record closes, if the entry is one: the
    /// text edit a reading reads, or the first entry of a split. Undo takes
    /// all from there back together.
    pub(crate) fn read_from(&self, seq: u64) -> Option<u64> {
        let e = self.notebook.entry(seq)?;
        let closes = |b: &&serde_json::Value| {
            ["read", "split", "make"].contains(&b["op"].as_str().unwrap_or(""))
        };
        let b = e.body().filter(|_| e.kind() == "edit").filter(closes)?;
        b["of"].as_u64()
    }
}

/// An item's text as written: the item, the entry that wrote the text,
/// the text, and whether reading its [[links]] wrote anything.
struct Written<'a> {
    item: u64,
    of: u64,
    text: &'a str,
    linked: bool,
}

/// Each statement with the name a constraint takes (other statements take
/// none).
fn named(text: &str, said: &[Said]) -> Vec<(Said, String)> {
    let mut names = constraint_names(text, said).into_iter();
    let mut name = |s: &Said| match s.kind {
        Kind::Constraint => names.next().unwrap_or_default(),
        _ => String::new(),
    };
    said.iter().map(|s| (s.clone(), name(s))).collect()
}

/// The names of a text's constraints: the text, and each one's place among
/// them when there are more than one.
fn constraint_names(text: &str, said: &[Said]) -> Vec<String> {
    let read = unbracketed(text);
    let base = read.trim().trim_end_matches('.');
    match said.iter().filter(|s| s.kind == Kind::Constraint).count() {
        1 => vec![base.to_string()],
        n => (1..=n).map(|k| format!("{base} ({k})")).collect(),
    }
}

/// Why a text was not read, in words.
fn hint_of(reading: &Reading) -> String {
    match reading {
        Reading::Unread(paraphrase, Unread::Unnamed) => format!(
            "not read: ACE takes this as \"{paraphrase}\", which is about something with no name; write \"every\" to mean all of them, or give it a name"
        ),
        Reading::Unread(paraphrase, Unread::Unstated) => format!(
            "not read: ACE takes this as \"{paraphrase}\", but the notebook has no statement for all of it"
        ),
        Reading::NotAce(messages) => {
            let stopped = messages.iter().find(|m| m.kind == "sentence");
            match stopped {
                Some(m) => format!("prose: not ACE, it stops at {}", m.note.subject),
                None => "prose: not ACE".into(),
            }
        }
        Reading::Read(..) => String::new(),
    }
}
