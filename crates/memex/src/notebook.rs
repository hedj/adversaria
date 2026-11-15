//! The notebook: the journal, and the outline replayed from it at the tip
//! (docs/structure.md § The code's structure). It changes only by an
//! entry, the one way state changes.

use crate::docs::Quote;
use crate::journal::{Error as JournalError, Journal};
use crate::outline::Outline;
use format::record::Entry;
use format::terms::Term;
use serde_json::{Value, json};
use std::collections::HashSet;
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

/// An outline replayed, the name of its world, and the entries that no
/// longer applied.
pub(crate) type Replayed = (Outline, String, Vec<u64>);

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

    /// Replay the content of a world, leaving out `skip`.
    pub(crate) fn replay(&self, tip: u64, skip: Option<u64>) -> Replayed {
        let path = self.path(tip);
        let mut retracted: HashSet<u64> = path
            .iter()
            .filter(|e| e.kind() == "retract")
            .filter_map(|e| e.body().and_then(|b| b["target"].as_u64()))
            .collect();
        retracted.extend(skip);
        let mut o = Outline::default();
        let mut world = "actual".to_string();
        let mut failed = Vec::new();
        for e in path {
            if !CONTENT.contains(&e.kind()) || retracted.contains(&e.seq) {
                continue;
            }
            let Some(b) = &e.body() else { continue };
            if e.kind() == "suppose" {
                world = b["name"].as_str().unwrap_or("supposed").to_string();
            }
            if !o.apply(e.seq, e.kind(), b) && !matches!(e.kind(), "suppose" | "adopt" | "import") {
                failed.push(e.seq);
            }
        }
        (o, world, failed)
    }

    fn rebuild(&mut self) {
        let (o, _, _) = self.replay(self.tip, None);
        self.outline = o;
    }

    /// The facts stated in the world whose head is an entry.
    pub(crate) fn facts_at(&self, head: u64) -> Result<Vec<(u64, Term)>, String> {
        crate::store::facts(self.journal.entries(), head)
    }

    /// The outline of another world, by its head.
    pub fn outline_at(&self, head: u64) -> Outline {
        self.replay(head, None).0
    }

    /// The Wikidata import in a world: the last import entry on its path,
    /// not retracted, and the file its blob is in.
    pub(crate) fn import_on(&self, head: u64) -> Option<(u64, std::path::PathBuf)> {
        let path = self.path(head);
        let retracted: HashSet<u64> = path
            .iter()
            .filter(|e| e.kind() == "retract")
            .filter_map(|e| e.body().and_then(|b| b["target"].as_u64()))
            .collect();
        path.iter()
            .rev()
            .filter(|e| e.kind() == "import" && !retracted.contains(&e.seq))
            .find_map(|e| {
                let b = e.body()?;
                let blob = b["blob"].as_str()?;
                (b["source"] == "Wikidata").then(|| (e.seq, self.blob(blob)))
            })
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
