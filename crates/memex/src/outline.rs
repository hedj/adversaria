//! The outline: items in a tree, their text, links and names, as replayed
//! from the journal (docs/design.md § The journal, § The outline model).
//!
//! An item is known by the seq of the entry that created it, so its address
//! never changes. Text is appended once: an `edit` entry's `add` holds the
//! new characters, and an item's text is a list of spans, each a run of
//! characters added by some entry. An edit writes a new list, so surviving
//! characters keep their addresses, and a span that points into another
//! item's text is a transclusion, shown in place.
//!
//! Bodies (format version 1):
//! * edit `{"op":"init"}`: entry 1 of every journal.
//! * edit `{"op":"create","under":p,"after":s|null,"add":text,"spans":[[seq,start,len],...]}`:
//!   a new item under p (0 is the top), after sibling s, or first.
//! * edit `{"op":"text","item":i,"add":text,"spans":[...]}`: new text for item i.
//! * edit `{"op":"move","item":i,"under":p,"after":s|null}`.
//! * edit `{"op":"delete","item":i}`: the item and its branch leave the outline.
//! * link `{"from":a,"to":b,"type":t,"facts":[t(a,b)]}`.
//! * name `{"name":n,"item":i}`.
//!
//! * edit `{"op":"facts","item":i,"facts":[...],"source":s}`: facts stated by item
//!   i; with a source, they are claims, `claims(Source, P)`.
//! * edit `{"op":"constraint","name":n,"rule":text,"facts":[node_def(seq, constraint(...))]}`.
//! * edit `{"op":"layer","item":i,"mark":m,"goal":g,"facts":[node_def(seq, layer(Mark, Goal))]}`:
//!   a layer of the view at item i (crate::view).
//! * edit `{"op":"derive","item":i,"template":t,"names":[...],"facts":[node_def(i, query(...))]}`:
//!   item i shows a value computed from the beliefs.
//! * adopt `{"claim":seq,"item":i,"source":s,"facts":[P]}`: a claim made a belief.
//!
//! In spans, seq 0 means the entry itself. Positions count Unicode scalar
//! values. An op that names an item not in the outline (after a retraction,
//! say) is skipped.

use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

/// The top of the outline: the parent of the top-level items.
pub const TOP: u64 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub seq: u64,
    pub start: usize,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: u64,
    pub parent: u64,
    pub children: Vec<u64>,
    pub spans: Vec<Span>,
    pub block: Block,
}

/// What an item is as a block of a document.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Block {
    /// What kind of block it is: heading, para, bullet, ... (empty for
    /// the user's own items).
    pub style: String,
    /// The import entry that made it, if any.
    pub origin: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub seq: u64,
    pub from: u64,
    pub to: u64,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Derive {
    pub template: String,
    pub names: Vec<String>,
}

/// A layer of a view: a mark drawn for each solution of a goal.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    /// The entry that made it; also its dataflow node.
    pub seq: u64,
    pub mark: String,
    pub goal: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub name: String,
    pub rule: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outline {
    /// The item each fact-bearing entry belongs to, by the entry's seq.
    pub fact_item: BTreeMap<u64, u64>,
    /// The source an item's claims came from.
    pub source: HashMap<u64, String>,
    pub derived: HashMap<u64, Derive>,
    /// Views: each item's layers, in the order they were made.
    pub views: BTreeMap<u64, Vec<Layer>>,
    /// Constraints, by the seq of the entry that made each.
    pub constraints: BTreeMap<u64, Constraint>,
    pub items: HashMap<u64, Item>,
    pub top: Vec<u64>,
    /// The characters each entry added, by seq.
    pub added: HashMap<u64, Vec<char>>,
    /// The item each entry's added text was written for.
    pub owner: HashMap<u64, u64>,
    pub links: Vec<Link>,
    pub names: BTreeMap<String, u64>,
}

/// A run of an item's text, and the item it was written for (its origin).
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub text: String,
    pub origin: u64,
}

impl Run {
    /// Take in a run of the same origin; else give it back.
    fn absorb(&mut self, next: Run) -> Option<Run> {
        if self.origin != next.origin {
            return Some(next);
        }
        self.text.push_str(&next.text);
        None
    }
}

impl Span {
    /// The part of a span, which begins at `at` in its item's text, from
    /// `from` to `to` in that text; none if they do not overlap.
    fn part(&self, at: usize, from: usize, to: usize) -> Option<Span> {
        let from = from.saturating_sub(at);
        let to = to.saturating_sub(at).min(self.len);
        let start = self.start + from;
        (from < to).then_some(Span {
            seq: self.seq,
            start,
            len: to - from,
        })
    }
}

/// Each span with the place in the text where it begins.
fn with_offsets(spans: &[Span]) -> Vec<(usize, &Span)> {
    let mut at = 0;
    let placed = spans.iter().map(|s| {
        let here = at;
        at += s.len;
        (here, s)
    });
    placed.collect()
}

pub fn span_json(spans: &[Span]) -> Value {
    json!(
        spans
            .iter()
            .map(|s| [s.seq, s.start as u64, s.len as u64])
            .collect::<Vec<_>>()
    )
}

fn parse_spans(v: &Value, seq: u64) -> Option<Vec<Span>> {
    v.as_array()?
        .iter()
        .map(|s| {
            let a = s.as_array()?;
            let n = |i: usize| a.get(i).and_then(|x| x.as_u64());
            let sq = n(0)?;
            Some(Span {
                seq: if sq == 0 { seq } else { sq },
                start: n(1)? as usize,
                len: n(2)? as usize,
            })
        })
        .collect()
}

/// An entry's body, read by key. A value that is missing, or of another
/// type, reads as none; as text it reads as empty.
struct Body<'a>(&'a Value);

impl<'a> Body<'a> {
    fn num(&self, key: &str) -> Option<u64> {
        self.0.get(key).and_then(Value::as_u64)
    }

    fn opt(&self, key: &str) -> Option<&'a str> {
        self.0.get(key).and_then(Value::as_str)
    }

    fn text(&self, key: &str) -> &'a str {
        self.opt(key).unwrap_or("")
    }

    /// A list's texts.
    fn texts(&self, key: &str) -> Vec<String> {
        let list = self.0.get(key).and_then(Value::as_array);
        list.into_iter()
            .flatten()
            .filter_map(|n| n.as_str().map(String::from))
            .collect()
    }

    /// The block an item's entry makes: its style, and the import it came
    /// from.
    fn block(&self) -> Block {
        let style = self.text("style").to_string();
        Block {
            style,
            origin: self.num("import"),
        }
    }

    /// The spans the body gives for an item's text.
    fn spans(&self, seq: u64) -> Option<Vec<Span>> {
        parse_spans(&self.0["spans"], seq)
    }
}

impl Item {
    /// A new item under another, with the style and origin its entry gives.
    fn new(id: u64, parent: u64, spans: Vec<Span>, block: Block) -> Item {
        Item {
            id,
            parent,
            children: vec![],
            spans,
            block,
        }
    }
}

impl Outline {
    pub fn contains(&self, id: u64) -> bool {
        id == TOP || self.items.contains_key(&id)
    }

    pub fn children(&self, id: u64) -> &[u64] {
        if id == TOP {
            &self.top
        } else {
            self.items.get(&id).map_or(&[], |i| &i.children)
        }
    }

    fn children_mut(&mut self, id: u64) -> &mut Vec<u64> {
        if id == TOP {
            &mut self.top
        } else {
            &mut self.items.get_mut(&id).unwrap().children
        }
    }

    pub fn parent(&self, id: u64) -> Option<u64> {
        self.items.get(&id).map(|i| i.parent)
    }

    /// The runs of an item's text, with where each came from.
    pub fn runs(&self, id: u64) -> Vec<Run> {
        let mut out: Vec<Run> = Vec::new();
        for s in self.spans_of(id) {
            let run = self.run_of(s, id);
            let rest = match out.last_mut() {
                Some(last) => last.absorb(run),
                None => Some(run),
            };
            out.extend(rest);
        }
        out
    }

    /// A span of item id's text, as a run of the item it was written for.
    fn run_of(&self, s: &Span, id: u64) -> Run {
        let text = self.text_of(s);
        let origin = self.written_for(s.seq).unwrap_or(id);
        Run { text, origin }
    }

    /// The characters of a span.
    fn text_of(&self, s: &Span) -> String {
        let added = self.added.get(&s.seq);
        added
            .map(|cs| cs.iter().skip(s.start).take(s.len).collect())
            .unwrap_or_default()
    }

    /// An item's spans; none if it is not there.
    fn spans_of(&self, id: u64) -> &[Span] {
        self.items.get(&id).map_or(&[], |i| &i.spans)
    }

    pub fn text(&self, id: u64) -> String {
        self.runs(id).into_iter().map(|r| r.text).collect()
    }

    /// Spans covering characters [from, to) of an item's text.
    pub fn slice(&self, id: u64, from: usize, to: usize) -> Vec<Span> {
        let placed = with_offsets(self.spans_of(id));
        placed
            .into_iter()
            .filter_map(|(at, s)| s.part(at, from, to))
            .collect()
    }

    fn is_within(&self, id: u64, ancestor: u64) -> bool {
        let mut at = Some(id);
        while let Some(x) = at {
            if x == ancestor {
                return true;
            }
            at = self.parent(x);
        }
        ancestor == TOP
    }

    /// Put an item under a parent, after a sibling. If that sibling is gone
    /// (retracted, say), the item goes first: its place depended on the
    /// sibling, but the item does not.
    /// Put an item among another's children: after the one named, or
    /// first if none is named or the one named is not among them.
    fn place(&mut self, id: u64, under: u64, after: Option<u64>) {
        let pos = self.index_after(under, after);
        self.children_mut(under).insert(pos, id);
    }

    /// The index under a parent just after one of its children: the
    /// first, if there is none or it is not there.
    fn index_after(&self, under: u64, after: Option<u64>) -> usize {
        let at = after.and_then(|s| self.children(under).iter().position(|c| *c == s));
        at.map_or(0, |p| p + 1)
    }

    fn unplace(&mut self, id: u64) {
        let p = self.items[&id].parent;
        self.children_mut(p).retain(|c| *c != id);
    }

    fn remove_branch(&mut self, id: u64) {
        let kids = self
            .items
            .get(&id)
            .map(|i| i.children.clone())
            .unwrap_or_default();
        for k in kids {
            self.remove_branch(k);
        }
        self.items.remove(&id);
    }

    fn spans_valid(&self, spans: &[Span]) -> bool {
        spans.iter().all(|s| {
            self.added
                .get(&s.seq)
                .is_some_and(|cs| s.start + s.len <= cs.len())
        })
    }

    /// Apply an entry to the outline; false if it does not apply, as when
    /// it names an item that is not there. Each kind of entry, and each
    /// kind of edit, is one method below.
    pub fn apply(&mut self, seq: u64, kind: &str, body: &Value) -> bool {
        let b = Body(body);
        let done = match kind {
            "edit" => self.edit(seq, &b),
            "adopt" => self.adopt(seq, &b),
            "link" => self.link(seq, &b),
            "name" => self.name(&b),
            _ => None,
        };
        done.is_some()
    }

    /// An edit of the items: their text and their places.
    fn edit(&mut self, seq: u64, b: &Body) -> Option<()> {
        match b.text("op") {
            "init" => Some(()),
            "create" => self.create(seq, b),
            "text" => self.retext(seq, b),
            "move" => self.move_item(b),
            "delete" => self.delete(b),
            _ => self.annotate(seq, b),
        }
    }

    /// An edit of what an item states or shows.
    fn annotate(&mut self, seq: u64, b: &Body) -> Option<()> {
        match b.text("op") {
            "facts" => self.state_facts(seq, b),
            "constraint" => self.keep_constraint(seq, b),
            "layer" => self.add_layer(seq, b),
            "derive" => self.set_derived(b),
            _ => None,
        }
    }

    /// The item a body names, if it is in the outline.
    fn item_in(&self, b: &Body) -> Option<u64> {
        b.num("item").filter(|i| self.items.contains_key(i))
    }

    // Items: their text and their places.

    fn create(&mut self, seq: u64, b: &Body) -> Option<()> {
        let under = b.num("under").filter(|u| self.contains(*u))?;
        self.record_text(seq, seq, b);
        let spans = b.spans(seq).unwrap_or_default();
        if !self.spans_valid(&spans) {
            return None;
        }
        self.items
            .insert(seq, Item::new(seq, under, spans, b.block()));
        self.place(seq, under, b.num("after"));
        Some(())
    }

    fn retext(&mut self, seq: u64, b: &Body) -> Option<()> {
        let id = self.item_in(b)?;
        self.record_text(seq, id, b);
        let spans = b.spans(seq).filter(|s| self.spans_valid(s))?;
        self.items.get_mut(&id)?.spans = spans;
        Some(())
    }

    /// Keep the characters an entry adds, written for an item.
    fn record_text(&mut self, seq: u64, item: u64, b: &Body) {
        self.added.insert(seq, b.text("add").chars().collect());
        self.owner.insert(seq, item);
    }

    fn move_item(&mut self, b: &Body) -> Option<()> {
        let (id, under, after) = (self.item_in(b)?, b.num("under")?, b.num("after"));
        if !self.can_move(id, under, after) {
            return None;
        }
        self.unplace(id);
        self.place(id, under, after);
        self.items.get_mut(&id)?.parent = under;
        Some(())
    }

    /// An item can move under another that exists and is not in its own
    /// branch, and not to follow itself.
    fn can_move(&self, id: u64, under: u64, after: Option<u64>) -> bool {
        let into_itself = self.is_within(under, id) && under != TOP;
        self.contains(under) && !into_itself && after != Some(id)
    }

    fn delete(&mut self, b: &Body) -> Option<()> {
        let id = self.item_in(b)?;
        self.unplace(id);
        self.remove_branch(id);
        Some(())
    }

    // What an item states or shows.

    fn state_facts(&mut self, seq: u64, b: &Body) -> Option<()> {
        let id = self.item_in(b)?;
        self.fact_item.insert(seq, id);
        if let Some(src) = b.opt("source") {
            self.source.insert(id, src.to_string());
        }
        Some(())
    }

    fn keep_constraint(&mut self, seq: u64, b: &Body) -> Option<()> {
        let (name, rule) = (b.text("name").into(), b.text("rule").into());
        self.constraints.insert(seq, Constraint { name, rule });
        Some(())
    }

    fn add_layer(&mut self, seq: u64, b: &Body) -> Option<()> {
        let id = self.item_in(b)?;
        let (mark, goal) = (b.text("mark").into(), b.text("goal").into());
        self.views
            .entry(id)
            .or_default()
            .push(Layer { seq, mark, goal });
        Some(())
    }

    fn set_derived(&mut self, b: &Body) -> Option<()> {
        let id = self.item_in(b)?;
        let (template, names) = (b.text("template").into(), b.texts("names"));
        self.derived.insert(id, Derive { template, names });
        Some(())
    }

    // The other kinds of entry.

    /// An adoption: of a suggested constraint, or of facts, which belong to
    /// an item if the entry names one.
    fn adopt(&mut self, seq: u64, b: &Body) -> Option<()> {
        if b.text("op") == "constraint" {
            return self.keep_constraint(seq, b);
        }
        if b.num("item").is_some() {
            let id = self.item_in(b)?;
            self.fact_item.insert(seq, id);
        }
        Some(())
    }

    fn link(&mut self, seq: u64, b: &Body) -> Option<()> {
        let (from, to) = (b.num("from")?, b.num("to")?);
        if !self.items.contains_key(&from) || !self.items.contains_key(&to) {
            return None;
        }
        let kind = b.opt("type").unwrap_or("link").to_string();
        self.links.push(Link {
            seq,
            from,
            to,
            kind,
        });
        Some(())
    }

    fn name(&mut self, b: &Body) -> Option<()> {
        let name = b.opt("name")?;
        let id = self.item_in(b)?;
        self.names.insert(name.to_string(), id);
        Some(())
    }

    /// Links that leave or reach an item and its target or source still exist.
    pub fn links_of(&self, id: u64) -> (Vec<&Link>, Vec<&Link>) {
        let live = |l: &&Link| self.items.contains_key(&l.from) && self.items.contains_key(&l.to);
        let out = self
            .links
            .iter()
            .filter(|l| l.from == id)
            .filter(live)
            .collect();
        let into = self
            .links
            .iter()
            .filter(|l| l.to == id)
            .filter(live)
            .collect();
        (out, into)
    }

    /// Items whose text shows some of this item's text in place.
    pub fn transcluded_in(&self, id: u64) -> Vec<u64> {
        let others = self.items.values().filter(|i| i.id != id);
        let mut out: Vec<u64> = others
            .filter(|i| self.quotes(i, id))
            .map(|i| i.id)
            .collect();
        out.sort();
        out
    }

    /// Whether an item shows text written for another.
    fn quotes(&self, item: &Item, id: u64) -> bool {
        item.spans
            .iter()
            .any(|s| self.written_for(s.seq) == Some(id))
    }

    /// The item an entry's added text was written for.
    fn written_for(&self, seq: u64) -> Option<u64> {
        self.owner.get(&seq).copied()
    }

    /// Count of items below one.
    pub fn descendants(&self, id: u64) -> usize {
        self.children(id)
            .iter()
            .map(|c| 1 + self.descendants(*c))
            .sum()
    }

    /// The outline number of an item: "3.2" is the second child of the third
    /// top-level item.
    pub fn number(&self, id: u64) -> String {
        let mut parts = Vec::new();
        let mut at = id;
        while let Some(p) = self.parent(at) {
            let i = self.children(p).iter().position(|c| *c == at).unwrap_or(0);
            parts.push((i + 1).to_string());
            at = p;
        }
        parts.reverse();
        parts.join(".")
    }

    /// The item an outline number names.
    pub fn by_number(&self, n: &str) -> Option<u64> {
        let mut at = TOP;
        for part in n.split('.') {
            let i: usize = part.parse().ok()?;
            at = *self.children(at).get(i.checked_sub(1)?)?;
        }
        (at != TOP).then_some(at)
    }

    /// An item's parent: TOP for a top-level item or one not in the outline.
    pub fn parent_or_top(&self, id: u64) -> u64 {
        self.parent(id).unwrap_or(TOP)
    }

    /// An item's parent, if the item is not top-level.
    pub fn parent_item(&self, id: u64) -> Option<u64> {
        self.parent(id).filter(|p| *p != TOP)
    }

    /// The item before this one among its siblings.
    pub fn previous_sibling(&self, id: u64) -> Option<u64> {
        let siblings = self.children(self.parent_or_top(id));
        let i = siblings.iter().position(|x| *x == id)?;
        i.checked_sub(1).map(|j| siblings[j])
    }

    pub fn last_child(&self, id: u64) -> Option<u64> {
        self.children(id).last().copied()
    }

    /// Whether an item is in the branch under top, top included.
    pub fn in_branch(&self, id: u64, top: u64) -> bool {
        let mut at = Some(id);
        while let Some(x) = at {
            if x == top {
                return true;
            }
            at = self.parent(x);
        }
        false
    }

    /// Whether an item is top itself, or is in the outline and under top.
    pub fn within(&self, id: u64, top: u64) -> bool {
        id == top || self.in_order().contains(&id) && self.in_branch(id, top)
    }

    /// The items at the other ends of an item's links: those it links to,
    /// then those that link to it.
    pub fn neighbours(&self, id: u64) -> Vec<u64> {
        let (out, into) = self.links_of(id);
        out.iter()
            .map(|l| l.to)
            .chain(into.iter().map(|l| l.from))
            .collect()
    }

    /// An item's spans, as an edit entry gives them, to show its text in
    /// place elsewhere.
    pub fn spans_json(&self, id: u64) -> Value {
        span_json(&self.items[&id].spans)
    }

    /// The item an address names: "#seq", an outline number ("3.2"), a
    /// name, or the start of only one item's text.
    pub fn addressed(&self, a: &str) -> Option<u64> {
        if a.is_empty() {
            return None;
        }
        self.by_seq(a)
            .or_else(|| self.by_number(a))
            .or_else(|| self.by_name(a))
            .or_else(|| self.by_prefix(a))
    }

    /// "#seq": an item by its sequence number.
    fn by_seq(&self, a: &str) -> Option<u64> {
        a.strip_prefix('#')
            .and_then(|s| s.parse().ok())
            .filter(|i| self.items.contains_key(i))
    }

    fn by_name(&self, a: &str) -> Option<u64> {
        self.names
            .get(a)
            .copied()
            .filter(|i| self.items.contains_key(i))
    }

    /// The one item whose text starts so, ignoring case.
    fn by_prefix(&self, a: &str) -> Option<u64> {
        let la = a.to_lowercase();
        let hits: Vec<u64> = self
            .in_order()
            .into_iter()
            .filter(|id| self.text(*id).to_lowercase().starts_with(&la))
            .collect();
        (hits.len() == 1).then(|| hits[0])
    }

    /// All items in outline order, depth first.
    pub fn in_order(&self) -> Vec<u64> {
        let mut out = Vec::with_capacity(self.items.len());
        let mut stack: Vec<u64> = self.top.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            out.push(id);
            stack.extend(self.children(id).iter().rev());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create(o: &mut Outline, seq: u64, under: u64, after: Option<u64>, text: &str) {
        let n = text.chars().count();
        let b = json!({"op":"create","under":under,"after":after,"add":text,"spans":[[0,0,n]]});
        assert!(o.apply(seq, "edit", &b), "create {seq}");
    }

    /// Two top-level items, the first with a child: 1 { 2 }, 3.
    fn branch() -> Outline {
        let mut o = Outline::default();
        create(&mut o, 1, TOP, None, "a");
        create(&mut o, 2, 1, None, "b");
        create(&mut o, 3, TOP, Some(1), "c");
        o
    }

    #[test]
    fn a_child_is_within_its_parents_branch() {
        assert!(branch().within(2, 1));
    }

    #[test]
    fn an_item_is_within_its_own_branch() {
        assert!(branch().within(1, 1));
    }

    #[test]
    fn a_sibling_and_a_parent_are_not_within_a_branch() {
        let o = branch();
        assert_eq!((o.within(3, 1), o.within(1, 2)), (false, false));
    }

    #[test]
    fn tree_numbers_and_order() {
        // Catches children out of order, or numbers that do not name items.
        let mut o = Outline::default();
        create(&mut o, 2, TOP, None, "The programme");
        create(&mut o, 3, TOP, Some(2), "DRAKON");
        create(&mut o, 4, 2, None, "Buran flew once");
        create(&mut o, 5, 2, Some(4), "Energia flew first");
        assert_eq!(o.in_order(), vec![2, 4, 5, 3]);
        assert_eq!(o.number(5), "1.2");
        assert_eq!(o.by_number("1.2"), Some(5));
        assert_eq!(o.by_number("2"), Some(3));
        assert_eq!(o.by_number("3"), None);
        assert_eq!(o.descendants(2), 2);
    }

    #[test]
    fn edits_keep_surviving_addresses() {
        // Catches an edit that rewrites old characters instead of pointing at them.
        let mut o = Outline::default();
        create(&mut o, 2, TOP, None, "Buran flew in 1989");
        let keep = o.slice(2, 0, 14); // "Buran flew in "
        let mut spans = keep.clone();
        spans.push(Span {
            seq: 0,
            start: 0,
            len: 4,
        });
        assert!(o.apply(
            3,
            "edit",
            &json!({"op":"text","item":2,"add":"1988","spans":span_json(&spans)})
        ));
        assert_eq!(o.text(2), "Buran flew in 1988");
        assert_eq!(
            o.items[&2].spans[0],
            Span {
                seq: 2,
                start: 0,
                len: 14
            }
        );
        assert_eq!(o.runs(2).len(), 1, "both runs belong to item 2");
    }

    #[test]
    fn a_slice_across_two_spans_cuts_each_at_its_end() {
        // Catches a span's part running past its own end into the next.
        let mut o = Outline::default();
        create(&mut o, 2, TOP, None, "Buran flew in 1989");
        let mut spans = o.slice(2, 0, 14);
        spans.push(Span {
            seq: 0,
            start: 0,
            len: 4,
        });
        let b = json!({"op":"text","item":2,"add":"1988","spans":span_json(&spans)});
        assert!(o.apply(3, "edit", &b));
        let want = [
            Span {
                seq: 2,
                start: 10,
                len: 4,
            },
            Span {
                seq: 3,
                start: 0,
                len: 2,
            },
        ];
        assert_eq!(o.slice(2, 10, 16), want);
    }

    #[test]
    fn a_quote_from_inside_an_item_shows_its_own_characters() {
        // Catches a quoted span read from the start of its entry's text.
        let mut o = Outline::default();
        create(&mut o, 2, TOP, None, "Buran flew once.");
        let flew = span_json(&o.slice(2, 6, 10));
        let b = json!({"op":"create","under":0,"after":2,"add":"","spans":flew});
        assert!(o.apply(3, "edit", &b));
        assert_eq!(o.text(3), "flew");
    }

    #[test]
    fn transclusion_shows_origin() {
        // Catches a transcluded run shown without its origin, or not found.
        let mut o = Outline::default();
        create(&mut o, 2, TOP, None, "Buran flew once.");
        let spans = o.slice(2, 0, 5);
        assert!(o.apply(
            3,
            "edit",
            &json!({"op":"create","under":0,"after":2,"add":"Timeline: ","spans":
            span_json(&[Span { seq: 0, start: 0, len: 10 }, spans[0]])})
        ));
        let runs = o.runs(3);
        assert_eq!(
            runs.iter()
                .map(|r| (r.text.as_str(), r.origin))
                .collect::<Vec<_>>(),
            vec![("Timeline: ", 3), ("Buran", 2)]
        );
        assert_eq!(o.transcluded_in(2), vec![3]);
    }

    #[test]
    fn moves_deletes_and_bad_ops() {
        // Catches a move into its own branch, or an op on a missing item, applied.
        let mut o = Outline::default();
        create(&mut o, 2, TOP, None, "a");
        create(&mut o, 3, 2, None, "b");
        assert!(
            !o.apply(
                4,
                "edit",
                &json!({"op":"move","item":2,"under":3,"after":null})
            ),
            "into own branch"
        );
        assert!(o.apply(
            5,
            "edit",
            &json!({"op":"move","item":3,"under":0,"after":2})
        ));
        assert_eq!(o.in_order(), vec![2, 3]);
        assert!(o.apply(6, "edit", &json!({"op":"delete","item":2})));
        assert!(!o.apply(
            7,
            "edit",
            &json!({"op":"text","item":2,"add":"x","spans":[[0,0,1]]})
        ));
        assert!(!o.apply(8, "link", &json!({"from":2,"to":3,"type":"supports"})));
        assert!(!o.apply(
            9,
            "edit",
            &json!({"op":"create","under":2,"after":null,"add":"","spans":[]})
        ));
        assert!(
            !o.apply(
                10,
                "edit",
                &json!({"op":"text","item":3,"add":"","spans":[[2,0,99]]})
            ),
            "span beyond its text"
        );
        assert_eq!(o.in_order(), vec![3]);
    }

    #[test]
    fn links_and_names() {
        // Catches links shown in the wrong direction, or to deleted items.
        let mut o = Outline::default();
        create(&mut o, 2, TOP, None, "claim");
        create(&mut o, 3, TOP, Some(2), "source");
        assert!(o.apply(4, "link", &json!({"from":2,"to":3,"type":"cites"})));
        assert!(o.apply(5, "name", &json!({"name":"src","item":3})));
        let (out, into) = o.links_of(3);
        assert!(out.is_empty());
        assert_eq!(into[0].from, 2);
        assert_eq!(o.names["src"], 3);
        assert!(o.apply(6, "edit", &json!({"op":"delete","item":2})));
        assert!(o.links_of(3).1.is_empty());
    }
}
