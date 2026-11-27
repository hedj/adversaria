//! History (docs/concepts-seven.md, change 3): the latest changes and
//! jumps of this branch, newest first, each with its entry's number and
//! what it did, said by the outline as it is now. The notebook's
//! inspector ends with it, so an entry's number is seen, not recalled.

use crate::notebook::Notebook;
use crate::outline::Outline;
use format::bodies::{Body, Edit, View};
use format::record::Entry;
use std::collections::BTreeSet;

/// One step of the history: its entry, what it did, and whether a later
/// entry of the branch retracts it.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub seq: u64,
    pub said: String,
    pub retracted: bool,
}

/// What a walk back along a branch has met: the entries retracted, and
/// those a reading of a text wrote. A retraction and a reading come after
/// what they name, so each is met first.
#[derive(Default)]
struct Met {
    retracted: BTreeSet<u64>,
    read: BTreeSet<u64>,
}

impl Met {
    /// An entry met: what it names, then its step, if it is one. A
    /// reading, and the facts it wrote, follow the text's edit, and are no
    /// steps of their own.
    fn step(&mut self, e: &Entry, o: &Outline) -> Option<Step> {
        self.retracted.extend(retract_target(e));
        self.read.extend(read_entries(e));
        let said = said(e, o).filter(|_| !self.read.contains(&e.seq))?;
        let retracted = self.retracted.contains(&e.seq);
        Some(Step {
            seq: e.seq,
            said,
            retracted,
        })
    }
}

/// The entry a retract entry takes back, if it is one.
fn retract_target(e: &Entry) -> Option<u64> {
    match Body::of(e)? {
        Body::Retract(r) => Some(r.target),
        _ => None,
    }
}

/// The entries a reading of a text wrote, if an entry is one.
fn read_entries(e: &Entry) -> Vec<u64> {
    match Body::of(e) {
        Some(Body::Edit(Edit::Read(r))) => r.entries,
        _ => vec![],
    }
}

/// What an entry did, if it is a step of the history; a reading of a
/// text, which follows its edit, and the rest of the view, are not.
fn said(e: &Entry, o: &Outline) -> Option<String> {
    match Body::of(e)? {
        Body::Edit(edit) => edit_said(e.seq, &edit, o),
        Body::Link(l) => Some(format!(
            "linked {} to {}",
            at(o, Some(l.from)),
            at(o, Some(l.to))
        )),
        Body::Name(n) => Some(format!("named {} “{}”", at(o, Some(n.item)), n.name)),
        Body::Suppose(s) => Some(format!("supposed “{}”", s.name?)),
        Body::Adopt(a) => Some(format!("adopted, in {}", at(o, a.item))),
        Body::Import(i) => Some(format!("imported {}", i.file.or(i.source)?)),
        Body::Retract(r) => Some(format!("retracted entry {}", r.target)),
        Body::Undo(u) => Some(format!("undid entry {}", u.undoes)),
        Body::Redo(r) => Some(format!("redid entry {}", r.redoes)),
        Body::View(v) => jump_said(&v, o),
        Body::Purge(_) => None,
    }
}

/// What an edit did to the outline: to its item, which an item's
/// creation is, by its entry's number, if it names none.
fn edit_said(seq: u64, edit: &Edit, o: &Outline) -> Option<String> {
    let item = at(o, Some(edit.item().unwrap_or(seq)));
    match edit {
        Edit::Text(_) => Some(format!("edited {item}")),
        Edit::Create(_) => Some(format!("added {item}")),
        Edit::Delete(_) => Some("deleted an item".into()),
        Edit::Move(_) => Some(format!("moved {item}")),
        Edit::Facts(_) => Some(format!("stated facts in {item}")),
        Edit::Constraint(_) => Some(format!("stated a rule in {item}")),
        Edit::Layer(_) => Some(format!("added a layer to {item}")),
        Edit::Read(_) | Edit::Init(_) => None,
        other => Some(format!("{}: {item}", other.op())),
    }
}

/// What a jump did: how it went, to which item, and from which.
fn jump_said(v: &View, o: &Outline) -> Option<String> {
    let (Some(from), Some(to)) = v.jump()? else {
        return None;
    };
    if from == to {
        return None;
    }
    let how = match v {
        View::Back(_) => "went back to",
        View::Forward(_) => "went forward to",
        View::Follow(_) => "followed a link to",
        View::Leap(_) => "leapt to",
        _ => "went to",
    };
    Some(format!(
        "{how} {} from {}",
        at(o, Some(to)),
        number(o, from)
    ))
}

/// An item, as the outline addresses it now: its number and text, or
/// that it is gone.
fn at(o: &Outline, id: Option<u64>) -> String {
    match id.filter(|i| o.contains(*i)) {
        Some(i) => o.labelled(i),
        None => "an item since deleted".into(),
    }
}

/// An item's number now, or that it is gone.
fn number(o: &Outline, id: u64) -> String {
    match o.contains(id) {
        true => o.number(id),
        false => "an item since deleted".into(),
    }
}

impl Notebook {
    /// What an entry did, in the words of the history, said by the outline
    /// as it is now; nothing for an entry that is no step of it.
    pub(crate) fn said_of(&self, seq: u64) -> String {
        let e = self.entry(seq);
        e.and_then(|e| said(e, &self.outline)).unwrap_or_default()
    }

    /// The latest steps of this branch, newest first, up to a number:
    /// each change, retraction, undo and redo, and each jump. Walked back
    /// from the tip, so only as far as the steps asked for.
    pub fn history(&self, most: usize) -> Vec<Step> {
        let mut met = Met::default();
        let outline = &self.outline;
        let path = self.back_from(self.tip);
        path.filter_map(|e| met.step(e, outline))
            .take(most)
            .collect()
    }

    /// The item a jump went to, if an entry is one: a view entry that
    /// moved the cursor from one item to another, as the trail follows
    /// it. One that stayed where it was is no step of the history, so its
    /// row is never right-clicked.
    pub(crate) fn jumped_to(&self, seq: u64) -> Option<u64> {
        let Body::View(v) = Body::of(self.entry(seq)?)? else {
            return None;
        };
        let (from, to) = v.jump()?;
        from.and(to)
    }
}
