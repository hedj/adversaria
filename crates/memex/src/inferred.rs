//! What version 2 of the journal records and version 1 did not, inferred
//! from version 1's entries when a journal is migrated (docs/design.md,
//! The journal format, Versions):
//!
//! * The memex's own entries, marked `"by": "memex"`: a reading's `read`
//!   record and the statements it lists; a `relink` record; `observed`
//!   and `offer`; an inline link (a `[[link]]` in a text), except one the
//!   user's `make` wrote; and a retraction of a reading's statement or of
//!   an inline link within a run of such entries that ends in a `read` or
//!   `relink` record. A retraction that Suppose wrote, followed by no
//!   such record, stays the user's.
//! * A view that only records where writing left the cursor, `written`:
//!   a `move` right after the `create` of the item it puts the cursor on,
//!   or after the `text` of that item. A move of the text cursor in the
//!   item just typed into, before the pause, is taken for writing too;
//!   a view after Backspace erased an empty item stays a move.

use format::bodies::{Body, Edit, View};
use format::record::{Entry, EntryKind};
use serde_json::{Value, json};
use std::collections::HashSet;

/// The entries version 2 would mark as the memex's own, and the views it
/// would write as `written`, by their numbers.
pub(crate) struct Inferred {
    by_memex: HashSet<u64>,
    written: HashSet<u64>,
}

impl Inferred {
    pub(crate) fn of(es: &[Entry]) -> Inferred {
        let by_memex = Runs::of(es).own(es);
        let written = es.windows(2).filter(|w| left(&w[0], &w[1]));
        let written = written.map(|w| w[1].seq).collect();
        Inferred { by_memex, written }
    }

    /// Mark an entry's body as version 2 writes it.
    pub(crate) fn mark(&self, seq: u64, body: &mut Value) {
        if self.by_memex.contains(&seq) && body["by"].is_null() {
            body["by"] = json!("memex");
        }
        if self.written.contains(&seq) {
            body["op"] = json!("written");
        }
    }
}

/// What the runs of a journal hold: the statements readings list, the
/// inline links, and the entries a `make` wrote, by their numbers.
struct Runs {
    stated: HashSet<u64>,
    inline: HashSet<u64>,
    made: HashSet<u64>,
}

impl Runs {
    fn of(es: &[Entry]) -> Runs {
        let stated = stated(es);
        let inline = inline_links(es);
        let made = made_by_make(es);
        Runs {
            stated,
            inline,
            made,
        }
    }

    /// The entries the memex wrote on its own.
    fn own(&self, es: &[Entry]) -> HashSet<u64> {
        let own = (0..es.len()).filter(|i| self.is_own(es, *i));
        own.map(|i| es[i].seq).collect()
    }

    /// Whether the entry at an index is one the memex wrote on its own.
    fn is_own(&self, es: &[Entry], i: usize) -> bool {
        let e = &es[i];
        if e.body().is_none() || self.made.contains(&e.seq) {
            return false;
        }
        match (e.kind(), op_of(e).unwrap_or("")) {
            (EntryKind::Edit, "read" | "relink")
            | (EntryKind::View, "observed" | "offer" | "start") => true,
            (EntryKind::Retract, _) => self.in_run(e) && self.run_ends_in_record(&es[i + 1..]),
            _ => self.stated.contains(&e.seq) || self.inline.contains(&e.seq),
        }
    }

    /// Whether an entry can be part of a reading's or a relinking's run:
    /// a statement, an inline link, or a retraction of either.
    fn in_run(&self, e: &Entry) -> bool {
        let target = match Body::of(e) {
            Some(Body::Retract(r)) => Some(r.target),
            _ => None,
        };
        let of_run = |s: u64| self.stated.contains(&s) || self.inline.contains(&s);
        match e.kind() {
            EntryKind::Retract => target.is_some_and(of_run),
            _ => of_run(e.seq),
        }
    }

    /// Whether the entries after a run's entry go on with the run to a
    /// `read` or `relink` record that closes it.
    fn run_ends_in_record(&self, after: &[Entry]) -> bool {
        let next = after.iter().find(|e| !self.in_run(e));
        let closes = |e: &Entry| {
            matches!(
                Body::of(e),
                Some(Body::Edit(Edit::Read(_) | Edit::Relink(_)))
            )
        };
        next.is_some_and(closes)
    }
}

/// Whether a view only records where an edit just before it left the
/// cursor: on the item it made, or in the item it typed into.
fn left(edit: &Entry, view: &Entry) -> bool {
    let after = edit.kind() == EntryKind::Edit && view.parent_seq() == Some(edit.seq);
    after && is_move(view) && stays(edit, view)
}

/// Whether an entry is a view of op `move`.
fn is_move(view: &Entry) -> bool {
    matches!(Body::of(view), Some(Body::View(View::Move(_))))
}

/// Whether a view's cursor is where an edit put it: on the item it made,
/// or in the item it typed into.
fn stays(edit: &Entry, view: &Entry) -> bool {
    let (Some(Body::Edit(e)), Some(Body::View(v))) = (Body::of(edit), Body::of(view)) else {
        return false;
    };
    let at = cursor(&v);
    match e {
        Edit::Create(_) => at == Some(edit.seq),
        Edit::Text(t) => at == Some(t.item),
        _ => false,
    }
}

/// The cursor a view records: in its pane with the focus.
fn cursor(v: &View) -> Option<u64> {
    let state = v.state()?;
    state.panes.get(state.focus as usize)?.cursor
}

/// The numbers of the statements readings list.
fn stated(es: &[Entry]) -> HashSet<u64> {
    let read = |e: &Entry| match Body::of(e) {
        Some(Body::Edit(Edit::Read(r))) => Some(r.entries),
        _ => None,
    };
    es.iter().filter_map(read).flatten().collect()
}

/// The numbers of the inline links: those a `[[link]]` in a text wrote.
fn inline_links(es: &[Entry]) -> HashSet<u64> {
    let inline = |e: &&Entry| matches!(Body::of(e), Some(Body::Link(l)) if l.inline == Some(true));
    es.iter().filter(inline).map(|e| e.seq).collect()
}

/// The numbers of the entries the user's `make` wrote: from the entry its
/// record names to the record itself.
fn made_by_make(es: &[Entry]) -> HashSet<u64> {
    let made = |e: &Entry| match Body::of(e) {
        Some(Body::Edit(Edit::Make(m))) => m.of,
        _ => None,
    };
    let records = es.iter().filter_map(|e| Some(made(e)?..=e.seq));
    records.flatten().collect()
}

/// The op of an edit or a view, if its body reads.
fn op_of(e: &Entry) -> Option<&'static str> {
    match Body::of(e)? {
        Body::Edit(edit) => Some(edit.op()),
        Body::View(view) => Some(view.op()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Entries of these kinds and bodies, each after the one before.
    fn chain(bodies: &[(EntryKind, Value)]) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        for (kind, body) in bodies {
            out.push(next(&out, *kind, body.clone()));
        }
        out
    }

    /// The entry of a kind and a body after these, numbered after the last.
    fn next(es: &[Entry], kind: EntryKind, body: Value) -> Entry {
        Entry::new(es.len() as u64 + 1, after(es), kind, body).unwrap()
    }

    /// The last entry, as Entry::new names a parent.
    fn after(es: &[Entry]) -> Option<(u64, &str)> {
        es.last().map(|e| (e.seq, e.hash.as_str()))
    }

    /// A view of op, its cursor on an item.
    /// A text edit of an item, as the memex writes one.
    fn typed(item: u64) -> Value {
        json!({"op": "text", "item": item, "add": "x", "spans": [[0, 0, 1]]})
    }

    fn view(op: &str, at: u64) -> (EntryKind, Value) {
        let panes = json!([{"kind": "outline", "cursor": at}]);
        (
            EntryKind::View,
            json!({"op": op, "focus": 0, "panes": panes}),
        )
    }

    /// Numbers, sorted.
    fn sorted(set: &HashSet<u64>) -> Vec<u64> {
        let mut v: Vec<u64> = set.iter().copied().collect();
        v.sort();
        v
    }

    #[test]
    fn runs_tell_the_memex_s_entries_from_the_user_s() {
        // Catches a re-reading's retraction, a relinking, or a reading's
        // link left as the user's; or the user's `make`, or a retraction
        // Suppose wrote, taken for the memex's.
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            (EntryKind::Edit, json!({"op": "text", "item": 9})),
            (EntryKind::Link, json!({"from": 9, "to": 7, "inline": true})),
            (EntryKind::Edit, json!({"op": "facts", "item": 9})),
            (
                EntryKind::Edit,
                json!({"op": "read", "item": 9, "entries": [4]}),
            ),
            (EntryKind::Edit, json!({"op": "text", "item": 9})),
            (EntryKind::Retract, json!({"target": 4})),
            (EntryKind::Edit, json!({"op": "facts", "item": 9})),
            (
                EntryKind::Edit,
                json!({"op": "read", "item": 9, "entries": [8]}),
            ),
            (EntryKind::Retract, json!({"target": 8, "constraints": [3]})),
            view("observed", 9),
            (EntryKind::Edit, json!({"op": "create", "under": 0})),
            (
                EntryKind::Link,
                json!({"from": 9, "to": 12, "inline": true}),
            ),
            (EntryKind::Edit, json!({"op": "make", "item": 9, "of": 12})),
            (EntryKind::Name, json!({"name": "n", "item": 7})),
            (EntryKind::Link, json!({"from": 9, "to": 7, "inline": true})),
            (
                EntryKind::Edit,
                json!({"op": "relink", "name": "n", "of": 15}),
            ),
        ]);
        let own = Runs::of(&es).own(&es);
        assert_eq!(sorted(&own), [3, 4, 5, 7, 8, 9, 11, 16, 17]);
    }

    #[test]
    fn a_move_is_written_only_where_writing_left_the_cursor() {
        // Catches every move after an edit taken for writing, though the
        // user moved the cursor elsewhere, or a move after a deletion.
        let es = chain(&[
            (EntryKind::Edit, json!({"op": "init"})),
            (EntryKind::Edit, json!({"op": "create", "under": 0})),
            view("move", 2),
            (EntryKind::Edit, typed(2)),
            view("move", 2),
            (EntryKind::Edit, typed(2)),
            view("move", 9),
            (EntryKind::Edit, json!({"op": "delete", "item": 2})),
            view("move", 9),
        ]);
        assert_eq!(sorted(&Inferred::of(&es).written), [3, 5]);
    }
}
