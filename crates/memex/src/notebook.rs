//! The notebook: the journal, and the outline replayed from it at the tip
//! (docs/structure.md § The code's structure). It changes only by an
//! entry, the one way state changes.

use crate::docs::Quote;
use crate::journal::{Error as JournalError, Journal};
use crate::outline::{Constraint, Outline};
use format::record::Entry;
use format::terms::Term;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Entry kinds that change the outline.
pub(crate) const CONTENT: [&str; 6] = ["edit", "link", "name", "suppose", "adopt", "import"];

/// The journal, and the state replayed from it at the tip.
pub struct Notebook {
    pub(crate) journal: Journal,
    pub outline: Outline,
    /// The entry the current state is at.
    pub tip: u64,
    /// Entries undone and not redone, most recent last.
    pub redo: Vec<u64>,
}

/// The strings of a list in an entry's body; none if it is no list.
pub(crate) fn strings(list: &Value) -> Vec<String> {
    let items = list.as_array().into_iter().flatten();
    items.filter_map(|x| x.as_str().map(String::from)).collect()
}

/// The inputs of a results import whose files no longer match the hashes
/// it recorded; none if it is no results import, or none changed.
fn changed_inputs(b: &Value) -> Option<Vec<String>> {
    if b["format"] != "memex-results" {
        return None;
    }
    let base = Path::new(b["base"].as_str().unwrap_or(""));
    let used = b["prov"]["used"].as_array().into_iter().flatten();
    let changed = used.filter(|input| input_changed(base, input));
    let paths: Vec<String> = changed
        .map(|input| input["path"].as_str().unwrap_or("").into())
        .collect();
    (!paths.is_empty()).then_some(paths)
}

/// Whether an input's file no longer matches the hash recorded for it.
/// Only a regular file is read: a pipe would block forever, and a device
/// such as /dev/zero never ends. Anything else counts as changed.
fn input_changed(base: &Path, input: &Value) -> bool {
    let path = base.join(input["path"].as_str().unwrap_or(""));
    let regular = std::fs::metadata(&path).is_ok_and(|m| m.is_file());
    let bytes = regular.then(|| std::fs::read(&path).ok()).flatten();
    let now = bytes.map(|x| format::record::sha256_hex(&x));
    now.as_deref() != input["sha256"].as_str()
}

/// What a fact is tried against: the facts of the world at the tip, the
/// outline's own and the fact, stated by the next entry, `by`; and the
/// constraints, by their entries.
pub(crate) struct Trial {
    pub(crate) facts: Vec<(u64, Term)>,
    pub(crate) constraints: Vec<(u64, Constraint)>,
    pub(crate) by: u64,
}

/// The entry a retract entry retracts.
fn target(e: &Entry) -> Option<u64> {
    e.body().and_then(|b| b["target"].as_u64())
}

/// The entries from the root to a head, oldest first, and those they
/// retract.
pub(crate) struct Branch<'a> {
    pub(crate) entries: Vec<&'a Entry>,
    retracted: HashSet<u64>,
}

impl<'a> Branch<'a> {
    fn new(entries: Vec<&'a Entry>) -> Branch<'a> {
        let retracts = entries.iter().filter(|e| e.kind() == "retract");
        let retracted = retracts.filter_map(|e| target(e)).collect();
        Branch { entries, retracted }
    }

    pub(crate) fn retracts(&self, seq: u64) -> bool {
        self.retracted.contains(&seq)
    }

    /// The edits after an entry, each with whether the branch retracts it.
    fn edits_after(&self, seq: u64) -> Vec<(&'a Entry, bool)> {
        let edits = self.entries.iter().filter(|e| e.kind() == "edit");
        let after = edits.filter(|e| e.seq > seq);
        after.map(|e| (*e, self.retracts(e.seq))).collect()
    }

    /// The content entries the branch keeps, with their bodies; `skip` is
    /// left out as if retracted.
    fn content(&self, skip: Option<u64>) -> Vec<(&'a Entry, &'a Value)> {
        let kept = |e: &&&Entry| !self.retracts(e.seq) && Some(e.seq) != skip;
        let content = self.entries.iter().filter(|e| CONTENT.contains(&e.kind()));
        content
            .filter(kept)
            .filter_map(|e| Some((*e, e.body()?)))
            .collect()
    }
}

/// An outline replayed, and the entries that no longer applied to it.
#[derive(Default)]
pub(crate) struct Replayed {
    pub(crate) outline: Outline,
    pub(crate) failed: Vec<u64>,
}

impl Replayed {
    /// Apply a content entry; one that changes nothing has failed, unless
    /// it only states facts.
    fn apply(&mut self, e: &Entry, b: &Value) {
        let applied = self.outline.apply(e.seq, e.kind(), b);
        if !applied && !matches!(e.kind(), "suppose" | "adopt" | "import") {
            self.failed.push(e.seq);
        }
    }
}

impl Notebook {
    /// Open a journal directory, writing its first entry if it is new. The
    /// state is at no entry yet: each entry is to be applied.
    pub fn open(dir: &Path) -> Result<Notebook, JournalError> {
        let mut journal = Journal::open(dir)?;
        if journal.entries().is_empty() {
            journal.append("edit", json!({"op": "init"}))?;
        }
        let outline = Outline::default();
        Ok(Notebook {
            journal,
            outline,
            tip: 0,
            redo: vec![],
        })
    }

    pub(crate) fn entry(&self, seq: u64) -> Option<&Entry> {
        self.journal.entries().get((seq as usize).wrapping_sub(1))
    }

    /// How many entries the journal has.
    pub(crate) fn entry_count(&self) -> usize {
        self.journal.entries().len()
    }

    /// Append an entry after a parent; it is not yet applied.
    pub(crate) fn append(
        &mut self,
        parent: Option<u64>,
        kind: &str,
        body: Value,
    ) -> Result<Entry, String> {
        let appended = self.journal.append_after(parent, kind, body);
        appended.map_err(|e| e.to_string())
    }

    /// The entries on the path from the tip back to the root, oldest first.
    pub(crate) fn path(&self, tip: u64) -> Vec<&Entry> {
        let mut out = Vec::new();
        let mut at = Some(tip);
        while let Some(s) = at {
            let Some(e) = self.entry(s) else { break };
            out.push(e);
            at = e.parent_seq();
        }
        out.reverse();
        out
    }

    /// The branch from the root to a head.
    pub(crate) fn branch(&self, head: u64) -> Branch<'_> {
        Branch::new(self.path(head))
    }

    /// Replay the content of a world, leaving out `skip`.
    pub(crate) fn replay(&self, tip: u64, skip: Option<u64>) -> Replayed {
        let mut out = Replayed::default();
        for (e, b) in self.branch(tip).content(skip) {
            out.apply(e, b);
        }
        out
    }

    fn rebuild(&mut self) {
        self.outline = self.outline_at(self.tip);
    }

    /// The facts stated in the world whose head is an entry.
    pub(crate) fn facts_at(&self, head: u64) -> Result<Vec<(u64, Term)>, String> {
        crate::store::facts(self.journal.entries(), head)
    }

    /// The facts at the tip, and these claims adopted as beliefs by the
    /// entry `by`.
    pub(crate) fn facts_adopting(
        &self,
        by: u64,
        claims: Vec<Term>,
    ) -> Result<Vec<(u64, Term)>, String> {
        let mut facts = self.facts_at(self.tip)?;
        facts.extend(claims.into_iter().map(|p| (by, p)));
        Ok(facts)
    }

    /// The imports of a document of this file's name, in the world at the
    /// tip.
    pub(crate) fn document_imports(&self, name: &str) -> Vec<u64> {
        let path = self.path(self.tip).into_iter();
        let of = |b: &Value| b["format"] == "document" && b["file"] == name;
        let imports = path.filter(|e| e.kind() == "import" && e.body().is_some_and(of));
        imports.map(|e| e.seq).collect()
    }

    /// The hashes of these entries, of those the journal holds.
    pub(crate) fn hashes_of(&self, entries: &std::collections::BTreeSet<u64>) -> Vec<String> {
        let found = entries.iter().filter_map(|s| self.entry(*s));
        found.map(|e| e.hash.clone()).collect()
    }

    /// The sequence number the next entry will have.
    pub(crate) fn next_seq(&self) -> u64 {
        self.entry_count() as u64 + 1
    }

    /// A fact to try against the world at the tip.
    pub(crate) fn trial(&self, fact: &Term) -> Result<Trial, String> {
        let by = self.next_seq();
        let mut facts = self.facts_at(self.tip)?;
        facts.extend(crate::world::outline_facts(&self.outline));
        facts.push((by, fact.clone()));
        let constraints = self
            .outline
            .constraints
            .iter()
            .map(|(s, c)| (*s, c.clone()))
            .collect();
        Ok(Trial {
            facts,
            constraints,
            by,
        })
    }

    /// The facts stated on an item, in the world at the tip.
    pub(crate) fn facts_of(&self, item: u64) -> Result<Vec<Term>, String> {
        let facts = self.facts_at(self.tip)?.into_iter();
        let on_item = facts.filter(|(s, _)| self.outline.fact_item.get(s) == Some(&item));
        Ok(on_item.map(|(_, f)| f).collect())
    }

    /// The outline of another world, by its head.
    pub fn outline_at(&self, head: u64) -> Outline {
        self.replay(head, None).outline
    }

    /// The Wikidata import in a world: the last import entry on its path,
    /// not retracted, and the file its blob is in.
    pub(crate) fn import_on(&self, head: u64) -> Option<(u64, PathBuf)> {
        let branch = self.branch(head);
        let imports = branch.entries.iter().rev().filter(|e| e.kind() == "import");
        let kept = imports.filter(|e| !branch.retracts(e.seq));
        kept.into_iter().find_map(|e| self.wikidata_blob(e))
    }

    /// The entry and the blob of a Wikidata import.
    fn wikidata_blob(&self, e: &Entry) -> Option<(u64, PathBuf)> {
        let b = e.body()?;
        let blob = b["blob"].as_str()?;
        (b["source"] == "Wikidata").then(|| (e.seq, self.blob(blob)))
    }

    /// Keep a file as a blob; its hash.
    pub(crate) fn store_file(&self, file: &str) -> Result<String, String> {
        let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        self.store_blob(&bytes)
    }

    /// Copy a file into blobs/, named by its SHA-256; return the hash.
    pub(crate) fn store_blob(&self, bytes: &[u8]) -> Result<String, String> {
        let hash = format::record::sha256_hex(bytes);
        let blobs = self.journal.dir().join(format::record::BLOBS);
        std::fs::create_dir_all(&blobs).map_err(|e| e.to_string())?;
        let dest = blobs.join(&hash);
        if !dest.exists() {
            std::fs::write(&dest, bytes).map_err(|e| e.to_string())?;
        }
        Ok(hash)
    }

    /// Where a blob is kept, by its hash.
    pub(crate) fn blob(&self, hash: &str) -> PathBuf {
        self.journal.dir().join(format::record::BLOBS).join(hash)
    }

    /// An item imported from a document, beside that document.
    pub(crate) fn quote(&self, item: u64) -> Result<Quote, String> {
        let origin = self.outline.items.get(&item).and_then(|i| i.block.origin);
        let origin = origin.ok_or("the target was not imported from a document")?;
        let e = self.entry(origin).ok_or("no import entry")?;
        let b = e.body().ok_or("the import was purged")?;
        let file = Path::new(b["file"].as_str().unwrap_or(""));
        let fmt = crate::docs::format_of(file).ok_or("the import is not a document")?;
        let blob = b["blob"].as_str().unwrap_or("").to_string();
        let source = crate::docs::plain_text(&self.blob(&blob), fmt)?;
        let span = self.outline.text(item);
        Ok(Quote { span, source, blob })
    }

    /// The edits of the world at the tip since the actual world it was
    /// sprouted from, at its head: each entry, what it does, and whether
    /// it is retracted.
    pub fn changes_since(&self, actual_head: u64) -> Vec<(u64, String, bool)> {
        let edits = self.branch(self.tip).edits_after(actual_head);
        let actual = self.outline_at(actual_head);
        edits
            .into_iter()
            .filter_map(|(e, gone)| Some((e.seq, self.change(e, &actual)?, gone)))
            .collect()
    }

    /// What an edit does, beside the actual outline: an item changed,
    /// added or deleted.
    fn change(&self, e: &Entry, actual: &Outline) -> Option<String> {
        let b = e.body()?;
        let item = b["item"].as_u64().unwrap_or(e.seq);
        let was = || format!("{}: “{}”", actual.number(item), actual.text(item));
        match b["op"].as_str()? {
            "text" => Some(format!(
                "changed {} → “{}”",
                was(),
                self.outline_at(e.seq).text(item)
            )),
            "create" => Some(format!("added: “{}”", b["add"].as_str().unwrap_or(""))),
            "delete" => Some(format!("deleted {}", was())),
            _ => None,
        }
    }

    /// Items whose results rest on input files that changed since: each
    /// with the files that no longer match their recorded hashes.
    pub fn stale_results(&self) -> BTreeMap<u64, Vec<String>> {
        let imports = self
            .path(self.tip)
            .into_iter()
            .filter(|e| e.kind() == "import");
        let changed = imports.filter_map(|e| Some((e.seq, changed_inputs(e.body()?)?)));
        let marked = changed.flat_map(|(seq, files)| {
            self.outline
                .items_from(seq)
                .map(move |id| (id, files.clone()))
        });
        marked.collect()
    }

    /// The order of the items before an entry is applied, if it may remove
    /// one; else nothing.
    pub(crate) fn order_before(&self, e: &Entry) -> Vec<u64> {
        let may_remove = match e.kind() {
            "view" => false,
            k if CONTENT.contains(&k) && e.parent_seq() == Some(self.tip) => {
                e.body().is_some_and(|b| b["op"] == "delete")
            }
            _ => true,
        };
        if may_remove {
            self.outline.in_order()
        } else {
            vec![]
        }
    }

    /// Apply an entry: the tip moves to it, the entries undone follow it,
    /// and the outline changes by it, or is replayed when the entry is not
    /// the next change at the tip.
    pub(crate) fn apply(&mut self, e: &Entry) {
        let follows = e.parent_seq() == Some(self.tip);
        let incremental = follows && CONTENT.contains(&e.kind());
        self.tip = e.seq;
        self.follow_undone(e);
        match e.kind() {
            "view" if follows => {}
            _ if incremental => {
                if let Some(b) = &e.body() {
                    self.outline.apply(e.seq, e.kind(), b);
                }
            }
            _ => self.rebuild(),
        }
    }

    /// The entries undone after an entry: an undo adds the entry it undoes,
    /// a redo takes the last back, and a change clears them.
    fn follow_undone(&mut self, e: &Entry) {
        match e.kind() {
            "view" | "purge" => {}
            "undo" => self
                .redo
                .extend(e.body().and_then(|b| b["undoes"].as_u64())),
            "redo" => {
                self.redo.pop();
            }
            _ => self.redo.clear(),
        }
    }
}
