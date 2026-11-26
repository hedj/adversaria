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
use crate::ace::{Kind, Reading, Said, Unread, Words, words_of};
use crate::app::App;
use crate::engine::told;
use crate::notebook::Notebook;
use format::record::EntryKind;
use format::terms::Term;
use serde_json::json;
use std::rc::Rc;

impl App {
    /// Start the parser of controlled English, which takes about a second
    /// to load, in a process of its own: the window starts it as it opens,
    /// so that the first pause need not wait for it. A failure is said
    /// when the parser is first needed.
    pub fn start_parser(&mut self) {
        let _started = self.reason.ace();
    }

    /// Let an item's text, written by the entry `of`, wait to be read at
    /// the next pause (App::idle). APE is started now, so that it is ready
    /// by then.
    pub(crate) fn will_read(&mut self, item: u64, of: u64) {
        self.session.wait_to_read(item, of);
        let _started = self.reason.ace();
    }

    /// A step of reading what waits: a part of the parser's lexicon, if it
    /// is not yet the current world's, else the next item's text.
    pub(crate) fn read_step(&mut self) {
        if self.lexicon_step() {
            return;
        }
        if let Some((item, of)) = self.session.next_to_read() {
            self.on_its_own(|app| app.read_item(item, of));
        }
    }

    /// Send a part of the parser's lexicon, if it is not yet the current
    /// world's: whether a step was taken. The world's words, once the
    /// journal moved, are a step of their own, so that a key pressed
    /// during the pause waits for them or for a part, not both. A failure
    /// sends none, and the reading that follows says it.
    fn lexicon_step(&mut self) -> bool {
        let Some(words) = self.kept_words() else {
            return self.words().is_ok();
        };
        let Ok(ace) = self.reason.ace() else {
            return false;
        };
        ace.lexicon_part(&words.lexicon)
            .is_ok_and(|sent| sent.is_some())
    }

    /// Read an item's text, written by the entry `of`, unless it is what
    /// was last read for it, or the item is gone (deleted before the
    /// pause; its facts went with it). Without APE nothing is read.
    fn read_item(&mut self, item: u64, of: u64) {
        if !self.notebook.outline.contains(item) {
            return;
        }
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
            self.session.set_prose(w.item, None);
            return self.unread(&w);
        }
        let (formal, prose) = super::formal::read(&text);
        if !formal.is_empty() {
            return self.state_with_prose(&w, formal, &prose);
        }
        let Ok(reading) = self.ace_read(&unbracketed(&text)) else {
            return;
        };
        match reading {
            Reading::Read(_, said) => self.state_reading(&w, &said),
            not_read => self.unread_saying_why(&w, &not_read),
        }
    }

    /// Statements written in Prolog, and the sentences of the prose around
    /// them, if it reads as controlled English; prose that does not is
    /// notes, as any is.
    fn state_with_prose(&mut self, w: &Written, mut said: Vec<Said>, prose: &str) {
        if !prose.trim().is_empty()
            && let Ok(Reading::Read(_, more)) = self.ace_read(&unbracketed(prose))
        {
            said.extend(more);
        }
        self.state_reading(w, &said);
    }

    /// A text not read, and the hint of why at the cursor, with APE's
    /// repair of it, if it has one.
    fn unread_saying_why(&mut self, w: &Written, reading: &Reading) {
        let mended = self.repair_of(w.text, reading);
        self.session
            .set_prose(w.item, Some((hint_of(reading), mended)));
        self.unread(w);
    }

    /// APE's first repair of a text that is not ACE, if the text is a
    /// sentence (it ends in a full stop, question or exclamation mark), as
    /// "Do you mean …?" mends a supposition: a heading or a name is not
    /// meant to be read.
    fn repair_of(&mut self, text: &str, reading: &Reading) -> Option<String> {
        let sentence = text.trim_end().ends_with(['.', '?', '!']);
        if !sentence || !matches!(reading, Reading::NotAce(_)) {
            return None;
        }
        self.ace_check(&unbracketed(text)).ok()?.mended()
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
    pub(crate) fn ace_read(&mut self, text: &str) -> Result<Reading, String> {
        let words = self.words()?;
        let ace = self.reason.ace()?;
        ace.read_with(&words, text).map_err(told)
    }

    /// The current world's words for the parser, kept while the journal
    /// stays where it is: gathering them from 10,000 facts took 16 ms, and
    /// each step of a pause, and each key on an ace line, asks for them.
    pub(crate) fn words(&mut self) -> Result<Rc<Words>, String> {
        if let Some(words) = self.kept_words() {
            return Ok(words);
        }
        let words = match self.reasoned_facts() {
            Some(facts) => words_of(facts),
            None => words_of(&self.notebook.facts_now()?),
        };
        let words = Rc::new(words);
        self.reason.words = Some((self.notebook.journal_at(), words.clone()));
        Ok(words)
    }

    /// The current world's facts as reasoning last gathered them, if it
    /// reasoned where the journal is: gathering them again took 6 ms on
    /// 10,000 facts.
    fn reasoned_facts(&self) -> Option<&[(u64, Term)]> {
        let current = self.reason.at == Some(self.notebook.journal_at());
        let eval = self.reason.eval[0].as_ref().filter(|_| current)?;
        Some(&eval.facts)
    }

    /// The current world's words, if they are kept for where the journal
    /// is.
    fn kept_words(&self) -> Option<Rc<Words>> {
        let (at, words) = self.reason.words.as_ref()?;
        (*at == self.notebook.journal_at()).then(|| words.clone())
    }

    /// State what an item's text was read as, in place of what it was read
    /// as before; say what the commands that state it said, as if typed
    /// (a constraint kept, a statement refused, and why).
    fn state_reading(&mut self, w: &Written, said: &[Said]) {
        self.session.set_prose(w.item, None);
        self.retract_reading(w.item);
        let (edits, told) = self.state_all(w.item, w.text, said);
        self.record_reading(w, edits, said);
        self.session.message = told.join("; ");
    }

    /// State each statement on the item: the edit entries written, and
    /// what the commands said.
    fn state_all(&mut self, item: u64, text: &str, said: &[Said]) -> (Vec<u64>, Vec<String>) {
        let first = self.next_entry();
        let told = self.outcomes(item, named(text, said));
        (self.notebook.edits_since(first), told)
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

    /// An item's text not read: what it was read as before is taken back.
    /// A reading of nothing is recorded if anything was taken back or its
    /// links changed, so that undo takes the text and all that followed
    /// from it back together.
    fn unread(&mut self, w: &Written) {
        let had = self.notebook.stated_by_reading(w.item);
        if had {
            self.retract_reading(w.item);
        }
        if had || w.linked {
            self.record_reading(w, vec![], &[]);
        }
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
            Kind::Template(shown) => self.cmd_derive(&[&format!("{shown} where {t}")], item),
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
            let written = self.write_here(EntryKind::Retract, json!({ "target": target }));
            self.session.report(written);
        }
    }

    /// Record a reading: the text read, the entries its statements are in,
    /// and each statement as the reader wrote it in Prolog, by its kind.
    fn record_reading(&mut self, w: &Written, entries: Vec<u64>, said: &[Said]) {
        let body = w.record(entries, said_json(said));
        let written = self.write_here(EntryKind::Edit, body);
        self.session.report(written);
    }
}

impl Written<'_> {
    /// The body of the record of this text's reading.
    fn record(&self, entries: Vec<u64>, said: Vec<[String; 2]>) -> serde_json::Value {
        json!({"op": "read", "item": self.item, "of": self.of, "text": self.text, "entries": entries, "said": said})
    }
}

/// Statements as a reading records them: each [kind, Prolog].
fn said_json(said: &[Said]) -> Vec<[String; 2]> {
    said.iter()
        .map(|s| [kind_word(&s.kind), s.text.clone()])
        .collect()
}

/// A statement's kind in words, as the right pane names it.
fn kind_word(kind: &Kind) -> String {
    match kind {
        Kind::Fact => "fact".into(),
        Kind::Claim(source) => format!("claim of {source}"),
        Kind::Rule => "rule".into(),
        Kind::Constraint => "constraint".into(),
        Kind::Question => "question".into(),
        Kind::Template(_) => "derived".into(),
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
    let read = unbracketed(&name_part(text));
    let base = read.trim().trim_end_matches(['.', ':']).trim();
    match said.iter().filter(|s| s.kind == Kind::Constraint).count() {
        1 => vec![base.to_string()],
        n => (1..=n).map(|k| format!("{base} ({k})")).collect(),
    }
}

/// The part of a text that names its constraints: the prose outside its
/// backticked Prolog, else, with none, the Prolog itself.
fn name_part(text: &str) -> String {
    let (_, outside) = super::formal::read(text);
    match outside.trim().is_empty() {
        true => text.replace('`', ""),
        false => outside,
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

impl Notebook {
    /// The edit entries written from entry `first` on.
    fn edits_since(&self, first: u64) -> Vec<u64> {
        let entries = self.journal.entries();
        let written = entries.get(first as usize - 1..).unwrap_or(&[]);
        let edits = written.iter().filter(|e| e.kind() == EntryKind::Edit);
        edits.map(|e| e.seq).collect()
    }

    /// Whether an item's last reading stated anything.
    fn stated_by_reading(&self, item: u64) -> bool {
        let read = self.outline.readings.get(&item);
        read.is_some_and(|r| !r.entries.is_empty())
    }

    /// The first entry of what a record closes, if the entry is one: the
    /// text edit a reading reads, or the first entry of a split. Undo takes
    /// all from there back together.
    pub(crate) fn read_from(&self, seq: u64) -> Option<u64> {
        let e = self.entry(seq)?;
        let closes = |b: &&serde_json::Value| {
            ["read", "split", "make", "relink"].contains(&b["op"].as_str().unwrap_or(""))
        };
        let b = e
            .body()
            .filter(|_| e.kind() == EntryKind::Edit)
            .filter(closes)?;
        b["of"].as_u64()
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::outline::TOP;

    #[test]
    fn the_facts_reasoning_kept_are_the_world_s_until_the_journal_moves() {
        // Catches the parser's words made from facts that reasoning
        // gathered before the journal moved, or from facts that are not
        // the world's: what it records of its own after it gathered them
        // changed them.
        let dir = std::env::temp_dir().join(format!("adversaria-reasoned-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::open(&dir).unwrap();
        app.new_item_after(TOP).unwrap();
        app.run_command("fact date(buran_flight, 1988)");
        app.evaluate().unwrap();
        let world = app.notebook.facts_now().unwrap();
        assert!(!world.is_empty());
        assert_eq!(app.reasoned_facts(), Some(world.as_slice()));
        app.run_command("fact date(buran_flight, 1989)");
        assert_eq!(app.reasoned_facts(), None, "the journal moved");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
