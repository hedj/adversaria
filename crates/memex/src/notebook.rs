//! The notebook: the journal, the outline replayed from it at the tip, the
//! outlines it had, kept to start a replay from, and the store of the
//! facts of the world last asked for (docs/structure.md § The code's
//! structure). It changes only by an entry, the one way state changes.

use crate::docs::Quote;
use crate::journal::{Error as JournalError, Journal};
use crate::outline::{Constraint, Outline, TOP};
use crate::store::Store;
use format::body::Body;
use format::record::{Entry, EntryKind};
use format::terms::Term;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Entry kinds that change the outline.
pub(crate) const CONTENT: [EntryKind; 6] = [
    EntryKind::Edit,
    EntryKind::Link,
    EntryKind::Name,
    EntryKind::Suppose,
    EntryKind::Adopt,
    EntryKind::Import,
];

/// How many outlines the notebook keeps to start a replay from. Each is a
/// whole outline: about 7 MB at 10,000 items, measured as the memory one
/// copy added.
const KEPT: usize = 8;

/// The build this memex is: its git commit, from build.rs.
pub const BUILD: &str = env!("MEMEX_BUILD");

/// The journal, and the state replayed from it at the tip.
pub struct Notebook {
    pub(crate) journal: Journal,
    pub outline: Outline,
    /// The entry the current state is at.
    pub tip: u64,
    /// Entries undone and not redone, most recent last.
    pub redo: Vec<u64>,
    /// The facts of the world last asked for, kept to reach the next.
    store: RefCell<Store>,
    /// Outlines the notebook has had, each by its content tip, the least
    /// lately used first: a replay starts from a copy of the nearest on the
    /// new tip's path, so that an undo need not replay from the root. They
    /// never change.
    kept: Vec<(u64, Outline)>,
}

/// The inputs of a results import whose files no longer match the hashes
/// it recorded; none if it is no results import, or none changed.
fn changed_inputs(b: Body) -> Option<Vec<String>> {
    if b.format() != Some("memex-results") {
        return None;
    }
    let base = Path::new(b.base().unwrap_or(""));
    let used = b.prov().and_then(|p| p["used"].as_array());
    let used = used.into_iter().flatten();
    let changed = used.filter(|input| input_changed(base, input));
    let paths: Vec<String> = changed
        .map(|input| input["path"].as_str().unwrap_or("").into())
        .collect();
    (!paths.is_empty()).then_some(paths)
}

/// Where a move puts an item: under another, or at the top.
fn placed(outline: &Outline, under: Option<u64>) -> String {
    match under.filter(|u| *u != TOP) {
        Some(u) => format!("under “{}”", outline.text(u)),
        None => "to the top".into(),
    }
}

/// Whether an input's file no longer matches the hash recorded for it.
/// One that cannot be read (read_input) counts as changed.
fn input_changed(base: &Path, input: &Value) -> bool {
    let path = base.join(input["path"].as_str().unwrap_or(""));
    let bytes = read_input(&path).ok();
    let now = bytes.map(|x| format::record::sha256_hex(&x));
    now.as_deref() != input["sha256"].as_str()
}

/// Write a file in a directory whole or not at all, and durably, before
/// the journal names it: through a temporary file beside the directory,
/// since every file in blobs/ must be named by its content, and a rename.
/// A file already there is written again, so one torn by a crash is mended.
fn write_whole(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.with_extension("tmp");
    let mut file = std::fs::File::create(&tmp)?;
    std::io::Write::write_all(&mut file, bytes)?;
    file.sync_all()?;
    std::fs::rename(&tmp, dir.join(name))?;
    std::fs::File::open(dir)?.sync_all()
}

/// The bytes of a file the memex is given to read. Only a regular file is
/// read: a pipe would block forever, and a device such as /dev/zero never
/// ends.
pub(crate) fn read_input(path: &Path) -> Result<Vec<u8>, String> {
    let shown = path.display();
    if !std::fs::metadata(path).is_ok_and(|m| m.is_file()) {
        return Err(format!("{shown}: not a regular file"));
    }
    std::fs::read(path).map_err(|e| format!("{shown}: {e}"))
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
    e.body().map(Body::of).and_then(Body::target)
}

/// The entries from the root to a head, oldest first, and those they
/// retract.
pub(crate) struct Branch<'a> {
    pub(crate) entries: Vec<&'a Entry>,
    retracted: HashSet<u64>,
}

impl<'a> Branch<'a> {
    fn new(entries: Vec<&'a Entry>) -> Branch<'a> {
        let retracts = entries.iter().filter(|e| e.kind() == EntryKind::Retract);
        let retracted = retracts.filter_map(|e| target(e)).collect();
        Branch { entries, retracted }
    }

    /// Its entries that it does not retract.
    pub(crate) fn live(&self) -> impl Iterator<Item = &'a Entry> + '_ {
        self.entries
            .iter()
            .copied()
            .filter(|e| !self.retracts(e.seq))
    }

    pub(crate) fn retracts(&self, seq: u64) -> bool {
        self.retracted.contains(&seq)
    }

    /// The edits after an entry, each with whether the branch retracts it.
    fn edits_after(&self, seq: u64) -> Vec<(&'a Entry, bool)> {
        let edits = self.entries.iter().filter(|e| e.kind() == EntryKind::Edit);
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
        if !applied
            && !matches!(
                e.kind(),
                EntryKind::Suppose | EntryKind::Adopt | EntryKind::Import
            )
        {
            self.failed.push(e.seq);
        }
    }
}

impl Notebook {
    /// Open a notebook's journal, and say what opening it set aside, if
    /// anything (Journal::opened). The state is at no entry yet: each
    /// entry is to be applied.
    pub fn open(dir: &Path) -> Result<(Notebook, Option<String>), JournalError> {
        let (journal, note) = Journal::opened(dir)?;
        Ok((Notebook::at_no_entry(journal), note))
    }

    /// A notebook of a journal, whose state is at no entry.
    fn at_no_entry(journal: Journal) -> Notebook {
        Notebook {
            journal,
            outline: Outline::default(),
            tip: 0,
            redo: vec![],
            store: RefCell::default(),
            kept: vec![],
        }
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
        kind: EntryKind,
        body: Value,
    ) -> Result<Entry, String> {
        let appended = self.journal.append_after(parent, kind, body);
        appended.map_err(|e| e.to_string())
    }

    /// The entries on the path from the tip back to the root, oldest first.
    pub(crate) fn path(&self, tip: u64) -> Vec<&Entry> {
        let mut out: Vec<&Entry> = self.back_from(tip).collect();
        out.reverse();
        out
    }

    /// The entries from a tip back to the root, newest first, each read
    /// only when it is asked for.
    pub(crate) fn back_from(&self, tip: u64) -> impl Iterator<Item = &Entry> + '_ {
        let parent = |e: &&Entry| e.parent_seq().and_then(|s| self.entry(s));
        std::iter::successors(self.entry(tip), parent)
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

    /// The outline at the tip, from the nearest outline kept on its path,
    /// else replayed from the root; the outline it replaces is kept.
    fn rebuild(&mut self, was: u64) {
        let outline = match self.nearest_kept(self.tip) {
            Some((i, ahead)) => self.kept_copy(i, &ahead),
            None => self.outline_at(self.tip),
        };
        let replaced = std::mem::replace(&mut self.outline, outline);
        self.keep(self.content_tip(was), replaced);
    }

    /// The outline kept nearest a tip on its path, by its place among those
    /// kept, and the content entries after it to the tip, oldest first.
    /// None is near if a retract comes first: it may take back an entry
    /// the outline kept holds.
    fn nearest_kept(&self, tip: u64) -> Option<(usize, Vec<u64>)> {
        let mut ahead = vec![];
        for e in self.back_from(tip) {
            if let Some(i) = self.kept.iter().position(|(at, _)| *at == e.seq) {
                ahead.reverse();
                return Some((i, ahead));
            }
            if e.kind() == EntryKind::Retract {
                return None;
            }
            if CONTENT.contains(&e.kind()) {
                ahead.push(e.seq);
            }
        }
        None
    }

    /// A copy of an outline kept, by its place among them, with content
    /// entries applied; the one kept is now the last lately used.
    fn kept_copy(&mut self, i: usize, ahead: &[u64]) -> Outline {
        let kept = self.kept.remove(i);
        let outline = self.applied(kept.1.clone(), ahead);
        self.kept.push(kept);
        outline
    }

    /// An outline with these content entries applied, in order.
    fn applied(&self, mut outline: Outline, entries: &[u64]) -> Outline {
        let each = entries.iter().filter_map(|s| self.entry(*s));
        for e in each {
            if let Some(b) = e.body() {
                outline.apply(e.seq, e.kind(), b);
            }
        }
        outline
    }

    /// Keep an outline, by its content tip, unless one is kept there; the
    /// least lately used goes beyond KEPT.
    fn keep(&mut self, at: Option<u64>, outline: Outline) {
        let Some(at) = at.filter(|at| !self.kept.iter().any(|(k, _)| k == at)) else {
            return;
        };
        self.kept.push((at, outline));
        if self.kept.len() > KEPT {
            self.kept.remove(0);
        }
    }

    /// Whether an outline is kept for the tip, or none is needed.
    pub(crate) fn outline_kept(&self) -> bool {
        let at = self.content_tip(self.tip);
        at.is_none_or(|at| self.kept.iter().any(|(k, _)| *k == at))
    }

    /// Keep a copy of the outline at the tip, which a later change may be
    /// undone back to: what a pause does last (App::pause_step).
    pub(crate) fn keep_outline(&mut self) {
        if !self.outline_kept() {
            let copy = self.outline.clone();
            self.keep(self.content_tip(self.tip), copy);
        }
    }

    /// The last entry on the path that changed content: what undo undoes.
    pub(crate) fn content_tip(&self, from: u64) -> Option<u64> {
        let changed = |e: &&Entry| CONTENT.contains(&e.kind()) || e.kind() == EntryKind::Retract;
        self.back_from(from).find(changed).map(|e| e.seq)
    }

    /// The facts stated in the world whose head is an entry, by that
    /// world's outline: an entry's facts hold only while each item it
    /// names (`item`, or a link's `from` and `to`), and the item whose
    /// text's reading wrote it, is in the outline, so
    /// deleting an item takes its statements out, and undoing the
    /// deletion puts them back.
    pub(crate) fn facts_in(
        &self,
        head: u64,
        outline: &Outline,
    ) -> Result<Vec<(u64, Term)>, String> {
        let mut store = self.store.borrow_mut();
        store.move_to(self.journal.entries(), head)?;
        let standing = store
            .by_entry()
            .filter(|(seq, _)| self.items_stand(*seq, outline));
        let facts = standing.flat_map(|(seq, fs)| fs.iter().map(move |f| (seq, f.clone())));
        Ok(facts.collect())
    }

    /// Whether each item an entry names, and the item whose reading
    /// wrote it, if one did, is in an outline.
    fn items_stand(&self, seq: u64, outline: &Outline) -> bool {
        let Some(b) = self.entry(seq).and_then(|e| e.body()) else {
            return true;
        };
        let named = ["item", "from", "to"].iter().filter_map(|k| b[*k].as_u64());
        let mut tied = named.chain(outline.read_by.get(&seq).copied());
        tied.all(|i| outline.contains(i))
    }

    /// The facts of the world at the tip.
    pub(crate) fn facts_now(&self) -> Result<Vec<(u64, Term)>, String> {
        self.facts_in(self.tip, &self.outline)
    }

    /// The facts at the tip, and these claims adopted as beliefs by the
    /// entry `by`.
    pub(crate) fn facts_adopting(
        &self,
        by: u64,
        claims: Vec<Term>,
    ) -> Result<Vec<(u64, Term)>, String> {
        let mut facts = self.facts_now()?;
        facts.extend(claims.into_iter().map(|p| (by, p)));
        Ok(facts)
    }

    /// The imports of a document of this file's name, in the world at the
    /// tip.
    pub(crate) fn document_imports(&self, name: &str) -> Vec<u64> {
        let path = self.path(self.tip).into_iter();
        let of = |b: Body| b.format() == Some("document") && b.file() == Some(name);
        let import =
            |e: &&Entry| e.kind() == EntryKind::Import && e.body().map(Body::of).is_some_and(of);
        let imports = path.filter(import);
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
        let mut facts = self.facts_now()?;
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
        let facts = self.facts_now()?.into_iter();
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
        let imports = branch
            .entries
            .iter()
            .rev()
            .filter(|e| e.kind() == EntryKind::Import);
        let kept = imports.filter(|e| !branch.retracts(e.seq));
        kept.into_iter().find_map(|e| self.wikidata_blob(e))
    }

    /// The entry and the blob of a Wikidata import.
    fn wikidata_blob(&self, e: &Entry) -> Option<(u64, PathBuf)> {
        let b = Body::of(e.body()?);
        let blob = b.blob()?;
        (b.source() == Some("Wikidata")).then(|| (e.seq, self.blob(blob)))
    }

    /// Keep a file the memex is given (read_input) as a blob; its hash.
    pub(crate) fn store_file(&self, file: &str) -> Result<String, String> {
        self.store_blob(&read_input(Path::new(file))?)
    }

    /// Keep bytes in blobs/, named by their SHA-256; return the hash.
    pub(crate) fn store_blob(&self, bytes: &[u8]) -> Result<String, String> {
        let hash = format::record::sha256_hex(bytes);
        let blobs = self.journal.dir().join(format::record::BLOBS);
        write_whole(&blobs, &hash, bytes).map_err(|e| e.to_string())?;
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
        let b = Body::of(e.body().ok_or("the import was purged")?);
        let file = Path::new(b.file().unwrap_or(""));
        let fmt = crate::docs::format_of(file).ok_or("the import is not a document")?;
        let blob = b.blob().unwrap_or("").to_string();
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
    /// added, deleted or moved.
    fn change(&self, e: &Entry, actual: &Outline) -> Option<String> {
        let b = Body::of(e.body()?);
        let item = b.item().unwrap_or(e.seq);
        let was = || actual.labelled(item);
        match b.op()? {
            "text" => Some(format!(
                "changed {} → “{}”",
                was(),
                self.outline_at(e.seq).text(item)
            )),
            "create" => Some(format!("added: “{}”", b.add().unwrap_or(""))),
            "delete" => Some(format!("deleted {}", was())),
            "move" => Some(format!("moved {} {}", was(), placed(actual, b.under()))),
            _ => None,
        }
    }

    /// Items whose results rest on input files that changed since: each
    /// with the files that no longer match their recorded hashes.
    pub fn stale_results(&self) -> BTreeMap<u64, Vec<String>> {
        let imports = self
            .path(self.tip)
            .into_iter()
            .filter(|e| e.kind() == EntryKind::Import);
        let changed = imports.filter_map(|e| Some((e.seq, changed_inputs(Body::of(e.body()?))?)));
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
            EntryKind::View => false,
            k if CONTENT.contains(&k) && e.parent_seq() == Some(self.tip) => {
                e.body().is_some_and(|b| Body::of(b).op() == Some("delete"))
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
        let was = std::mem::replace(&mut self.tip, e.seq);
        self.follow_undone(e);
        match e.kind() {
            EntryKind::View if follows => {}
            _ if incremental => {
                if let Some(b) = &e.body() {
                    self.outline.apply(e.seq, e.kind(), b);
                }
            }
            EntryKind::Purge => self.purged(),
            _ => self.rebuild(was),
        }
    }

    /// A purge took a body that the outlines kept may hold: none is kept,
    /// and the outline is replayed from the root.
    fn purged(&mut self) {
        self.kept.clear();
        self.outline = self.outline_at(self.tip);
    }

    /// The entries undone after an entry: an undo adds the entry it undoes,
    /// a redo takes the last back, and a change clears them. Leaving a
    /// world is a change: what was undone there is not redone in another.
    fn follow_undone(&mut self, e: &Entry) {
        let body = e.body().map(Body::of);
        let leaves = body.is_some_and(|b| b.op() == Some("world"));
        match e.kind() {
            EntryKind::View if leaves => self.redo.clear(),
            EntryKind::View | EntryKind::Purge => {}
            EntryKind::Undo => self.redo.extend(body.and_then(Body::undoes)),
            EntryKind::Redo => {
                self.redo.pop();
            }
            _ => self.redo.clear(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A notebook of a journal of its own, its first entry applied.
    fn opened(name: &str) -> Notebook {
        let dir =
            std::env::temp_dir().join(format!("adversaria-kept-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (mut n, _) = Notebook::open(&dir).unwrap();
        n.journal.durable = false;
        let first = n.entry(1).unwrap().clone();
        n.apply(&first);
        n
    }

    /// An entry written after another and applied; its number.
    fn written(n: &mut Notebook, after: u64, kind: EntryKind, body: Value) -> u64 {
        let e = n.append(Some(after), kind, body).unwrap();
        n.apply(&e);
        e.seq
    }

    /// An item of a text made at the top, after the tip; its number.
    fn made(n: &mut Notebook, text: &str) -> u64 {
        let spans = [[0, 0, text.chars().count()]];
        let body = json!({"op": "create", "under": TOP, "add": text, "spans": spans});
        written(n, n.tip, EntryKind::Edit, body)
    }

    /// The texts of the items, sorted: each is made first at the top.
    fn texts(n: &Notebook) -> Vec<String> {
        let mut texts: Vec<String> = n
            .outline
            .in_order()
            .iter()
            .map(|i| n.outline.text(*i))
            .collect();
        texts.sort();
        texts
    }

    #[test]
    fn an_undo_starts_from_the_outline_kept_and_applies_what_follows_it() {
        // Catches an undo that replays from the root though an outline is
        // kept on its path, or applies the entries after it out of order.
        let mut n = opened("undo");
        made(&mut n, "Alder");
        n.keep_outline();
        let birch = made(&mut n, "Birch");
        let spans = [[birch, 0, 5], [0, 0, 5]];
        let text = json!({"op": "text", "item": birch, "add": " bark", "spans": spans});
        let bark = written(&mut n, birch, EntryKind::Edit, text);
        let cedar = made(&mut n, "Cedar");
        let undo = n
            .append(Some(bark), EntryKind::Undo, json!({"undoes": cedar}))
            .unwrap();
        assert_eq!(n.nearest_kept(undo.seq), Some((0, vec![birch, bark])));
        n.apply(&undo);
        assert_eq!(texts(&n), ["Alder", "Birch bark"]);
        assert_eq!(n.outline, n.outline_at(n.tip));
    }

    /// The texts after an undo of an entry, back to another.
    fn undone(n: &mut Notebook, to: u64, undoes: u64) -> Vec<String> {
        let undo = n
            .append(Some(to), EntryKind::Undo, json!({"undoes": undoes}))
            .unwrap();
        n.apply(&undo);
        texts(n)
    }

    #[test]
    fn an_undo_shows_the_outline_kept_not_one_replayed_from_the_root() {
        // Catches the outlines kept never started from, or one changed by
        // an undo that starts from it: one marked with an item that no
        // entry made shows through an undo past it and one back to it.
        let mut n = opened("used");
        let alder = made(&mut n, "Alder");
        n.keep_outline();
        let marker = json!({"op": "create", "under": TOP, "add": "marker", "spans": [[0, 0, 6]]});
        n.kept[0].1.apply(1_000, EntryKind::Edit, &marker);
        let birch = made(&mut n, "Birch");
        let cedar = made(&mut n, "Cedar");
        assert_eq!(undone(&mut n, birch, cedar), ["Alder", "Birch", "marker"]);
        assert_eq!(undone(&mut n, alder, birch), ["Alder", "marker"]);
    }

    #[test]
    fn a_retraction_after_the_outline_kept_is_replayed_from_the_root() {
        // Catches an outline kept that holds what a later retraction takes
        // back used to start from.
        let mut n = opened("retract");
        let alder = made(&mut n, "Alder");
        made(&mut n, "Birch");
        n.keep_outline();
        let retract = n
            .append(Some(n.tip), EntryKind::Retract, json!({"target": alder}))
            .unwrap();
        assert_eq!(n.nearest_kept(retract.seq), None);
        n.apply(&retract);
        assert_eq!(texts(&n), ["Birch"]);
    }

    #[test]
    fn a_purge_drops_the_outlines_kept_that_hold_what_it_took() {
        // Catches an outline kept from before a purge started from after
        // it: it holds the text the purge took.
        let mut n = opened("purge");
        made(&mut n, "Alder");
        let secret = made(&mut n, "secret");
        n.keep_outline();
        let purge = n.journal.purge(secret, "privacy").unwrap();
        n.apply(&purge);
        let after = written(
            &mut n,
            purge.seq,
            EntryKind::Undo,
            json!({"undoes": secret}),
        );
        assert_eq!(n.tip, after);
        assert_eq!(texts(&n), ["Alder"]);
    }
}
