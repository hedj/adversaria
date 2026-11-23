//! History (docs/concepts-seven.md, change 3): the latest changes and
//! jumps of this branch, newest first, each with its entry's number and
//! what it did, said by the outline as it is now. The notebook's
//! inspector ends with it, so an entry's number is seen, not recalled.

use crate::app::App;
use crate::outline::Outline;
use format::record::Entry;
use serde_json::Value;
use std::collections::BTreeSet;

/// One step of the history: its entry, what it did, and whether a later
/// entry of the branch retracts it.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub seq: u64,
    pub said: String,
    pub retracted: bool,
}

impl App {
    /// The latest steps of this branch, newest first, up to a number:
    /// each change, retraction, undo and redo, and each jump. Walked back
    /// from the tip, so only as far as the steps asked for.
    pub fn history(&self, most: usize) -> Vec<Step> {
        let mut met = Met::default();
        let outline = &self.notebook.outline;
        let path = self.notebook.back_from(self.notebook.tip);
        path.filter_map(|e| met.step(e, outline))
            .take(most)
            .collect()
    }

    /// The item a jump went to, if an entry is one: a view entry that
    /// moved the cursor from one item to another, as the trail follows
    /// it. One that stayed where it was is no step of the history, so its
    /// row is never right-clicked.
    pub(crate) fn jumped_to(&self, seq: u64) -> Option<u64> {
        let e = self.notebook.entry(seq)?;
        let b = e.body().filter(|_| e.kind() == "view")?;
        b["from"].as_u64().and(b["to"].as_u64())
    }
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
    let b = e.body().filter(|_| e.kind() == "retract")?;
    b["target"].as_u64()
}

/// The entries a reading of a text wrote, if an entry is one.
fn read_entries(e: &Entry) -> Vec<u64> {
    let b = e.body().filter(|b| e.kind() == "edit" && b["op"] == "read");
    let entries = b
        .and_then(|b| b["entries"].as_array())
        .into_iter()
        .flatten();
    entries.filter_map(Value::as_u64).collect()
}

/// What an entry did, if it is a step of the history; a reading of a
/// text, which follows its edit, and the rest of the view, are not.
fn said(e: &Entry, o: &Outline) -> Option<String> {
    let b = e.body()?;
    match e.kind() {
        "edit" => edit_said(e.seq, b, o),
        "link" => Some(format!(
            "linked {} to {}",
            at(o, &b["from"]),
            at(o, &b["to"])
        )),
        "name" => Some(format!(
            "named {} “{}”",
            at(o, &b["item"]),
            b["name"].as_str()?
        )),
        "suppose" => Some(format!("supposed “{}”", b["name"].as_str()?)),
        "adopt" => Some(format!("adopted, in {}", at(o, &b["item"]))),
        "import" => Some(format!(
            "imported {}",
            b["file"].as_str().or(b["source"].as_str())?
        )),
        "retract" => Some(format!("retracted entry {}", b["target"].as_u64()?)),
        "undo" => Some(format!("undid entry {}", b["undoes"].as_u64()?)),
        "redo" => Some(format!("redid entry {}", b["redoes"].as_u64()?)),
        "view" => jump_said(b, o),
        _ => None,
    }
}

/// What an edit did to the outline: to its item, which an item's
/// creation is, by its entry's number, if it names none.
fn edit_said(seq: u64, b: &Value, o: &Outline) -> Option<String> {
    let item = at(o, &b["item"].as_u64().unwrap_or(seq).into());
    match b["op"].as_str()? {
        "text" => Some(format!("edited {item}")),
        "create" => Some(format!("added {item}")),
        "delete" => Some("deleted an item".into()),
        "move" => Some(format!("moved {item}")),
        "facts" => Some(format!("stated facts in {item}")),
        "constraint" => Some(format!("stated a rule in {item}")),
        "layer" => Some(format!("added a layer to {item}")),
        "read" | "init" => None,
        op => Some(format!("{op}: {item}")),
    }
}

/// What a jump did: how it went, to which item, and from which.
fn jump_said(b: &Value, o: &Outline) -> Option<String> {
    let (from, to) = (b["from"].as_u64()?, b["to"].as_u64()?);
    if from == to {
        return None;
    }
    let how = match b["op"].as_str()? {
        "back" => "went back to",
        "forward" => "went forward to",
        "follow" => "followed a link to",
        "leap" => "leapt to",
        _ => "went to",
    };
    Some(format!(
        "{how} {} from {}",
        at(o, &b["to"]),
        number(o, from)
    ))
}

/// An item, as the outline addresses it now: its number and text, or
/// that it is gone.
fn at(o: &Outline, id: &Value) -> String {
    match id.as_u64().filter(|i| o.contains(*i)) {
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
