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
//! * edit `{"op":"delete","item":i}`: the item and its branch leave the outline,
//!   and with them the facts of every entry that names one of them
//!   (Notebook::facts_in).
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

use format::bodies;
use format::body::Body;
use format::record::EntryKind;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// The top of the outline: the parent of the top-level items.
pub const TOP: u64 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub seq: u64,
    pub start: usize,
    pub len: usize,
}

/// What an export holds: the blocks of the items exported, from the depth
/// of each root, and the entries their texts came from.
#[derive(Default)]
pub struct Exported {
    pub blocks: Vec<crate::docs::Block>,
    pub entries: BTreeSet<u64>,
}

impl Exported {
    /// Add an item, its depth counted from a root's.
    fn add(&mut self, o: &Outline, id: u64, base: usize) {
        self.blocks.push(o.block_at(id, base));
        self.entries.extend(o.entries_of(id));
    }
}

/// A revision of a document in items: the item each new block keeps, if
/// any; the blocks whose text changes; the items dropped; and where the
/// document is.
pub struct Revised {
    pub kept: Vec<Option<u64>>,
    pub changed: Vec<usize>,
    pub dropped: Vec<u64>,
    pub home: u64,
    /// The item before the document where it is, if any: a block new at
    /// its start goes after it, not above the outline.
    pub before: Option<u64>,
}

impl Revised {
    /// The alignment of a document's blocks with its old items, in items;
    /// where it is, the outline says (Outline::revised).
    fn of(r: crate::docs::Revision, old: &[u64], home: u64) -> Revised {
        let kept = r.kept.iter().map(|k| k.map(|o| old[o])).collect();
        let dropped = r.dropped.iter().map(|o| old[*o]).collect();
        Revised {
            kept,
            changed: r.changed,
            dropped,
            home,
            before: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Item {
    pub id: u64,
    pub parent: u64,
    pub children: Vec<u64>,
    pub spans: Vec<Span>,
    said: Said,
    pub block: Block,
}

/// What an item's spans say: the characters they name, joined, and the
/// key of the page that text would name (page_key). Set with the spans,
/// since what an entry added never changes.
#[derive(Debug, Clone, PartialEq, Default)]
struct Said {
    text: String,
    key: String,
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
    /// Shown under the item's own text (a question's answer), not in its
    /// place.
    pub under: bool,
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

/// What an item's text was read as (crate::actions::reading): the text
/// read, and the entries that state what it was read as.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub text: String,
    pub entries: Vec<u64>,
    /// Each statement as the reader wrote it in Prolog, with its kind in
    /// words ("fact", "claim of Talk-notes", "rule", ...).
    pub said: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outline {
    /// The item each fact-bearing entry belongs to, by the entry's seq.
    pub fact_item: BTreeMap<u64, u64>,
    /// The link entries that a [[link]] in their item's text keeps.
    pub inline_links: BTreeSet<u64>,
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
    /// What each item's text was last read as, by the item.
    pub readings: HashMap<u64, Reading>,
    /// The item whose text's reading wrote an entry, by the entry: a rule
    /// or a fact read from an item's text holds while the item does
    /// (Notebook::facts_in), though its entry may not name the item.
    pub read_by: HashMap<u64, u64>,
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
        (from < to).then(|| Span {
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

/// How one text becomes another: the characters it keeps at its start
/// and at its end, and those added between.
pub struct TextChange {
    pub start: usize,
    pub added: String,
    pub end: usize,
}

impl TextChange {
    /// The change from one text to another; none if they are the same.
    /// The end kept never overlaps the start kept.
    pub fn between(old: &[char], new: &[char]) -> Option<TextChange> {
        if old == new {
            return None;
        }
        let start = shared_len(old.iter(), new.iter());
        let room = old.len().min(new.len()) - start;
        let end = shared_len(old.iter().rev(), new.iter().rev()).min(room);
        let added = new[start..new.len() - end].iter().collect();
        Some(TextChange { start, added, end })
    }

    /// The span of what the change adds, of no entry yet; none if it adds
    /// nothing.
    fn added_span(&self) -> Vec<Span> {
        let len = self.added.chars().count();
        let span = Span {
            seq: 0,
            start: 0,
            len,
        };
        if len > 0 { vec![span] } else { vec![] }
    }
}

/// How many characters two texts share before they differ.
fn shared_len<'a>(a: impl Iterator<Item = &'a char>, b: impl Iterator<Item = &'a char>) -> usize {
    a.zip(b).take_while(|(x, y)| x == y).count()
}

/// Spans as an entry writes them: [seq, start, len] each.
pub fn written_spans(spans: &[Span]) -> Vec<bodies::Span> {
    let row = |s: &Span| [s.seq, s.start as u64, s.len as u64];
    spans.iter().map(row).collect()
}

fn parse_spans(list: &[Value], seq: u64) -> Option<Vec<Span>> {
    list.iter()
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

/// The block an item's entry makes: its style, and the import it came
/// from.
fn block_of(b: &Body) -> Block {
    let style = b.style().unwrap_or("").to_string();
    Block {
        style,
        origin: b.import(),
    }
}

/// The spans a body gives for an item's text, by the entry it is.
fn spans_of(b: &Body, seq: u64) -> Option<Vec<Span>> {
    parse_spans(b.spans()?, seq)
}

impl Item {
    /// A new item under another, with the style and origin its entry gives.
    /// It has no text until its spans are set (Outline::set_spans).
    fn new(id: u64, parent: u64, block: Block) -> Item {
        let item = Item::default();
        Item {
            id,
            parent,
            block,
            ..item
        }
    }
}

impl Outline {
    /// Whether an item states the fact of an entry.
    pub fn states(&self, item: u64, seq: u64) -> bool {
        self.fact_item.get(&seq) == Some(&item)
    }

    /// Whether an item states an entry's fact and its text was not read
    /// as it: a fact given it by `fact` or `claim`.
    pub fn states_own(&self, item: u64, seq: u64) -> bool {
        let read = self
            .readings
            .get(&item)
            .is_some_and(|r| r.entries.contains(&seq));
        self.states(item, seq) && !read
    }

    /// Whether an entry made a layer of an item's view.
    pub fn is_layer_of(&self, item: u64, seq: u64) -> bool {
        let layers = self.views.get(&item);
        layers.is_some_and(|ls| ls.iter().any(|l| l.seq == seq))
    }

    pub fn contains(&self, id: u64) -> bool {
        id == TOP || self.items.contains_key(&id)
    }

    /// The links whose ends are both in the outline.
    pub(crate) fn live_links(&self) -> impl Iterator<Item = &Link> {
        let live = |l: &&Link| self.contains(l.from) && self.contains(l.to);
        self.links.iter().filter(live)
    }

    /// The items at an item's level under its parent, it among them.
    pub fn siblings(&self, id: u64) -> &[u64] {
        self.children(self.parent_or_top(id))
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
        self.text_ref(id).to_string()
    }

    /// The text of an item, borrowed: none for no item.
    pub fn text_ref(&self, id: u64) -> &str {
        self.items.get(&id).map_or("", |i| &i.said.text)
    }

    /// What some spans say: the characters they name, joined, and the
    /// key of the page that text would name.
    fn said_by(&self, spans: &[Span]) -> Said {
        let text: String = spans.iter().map(|s| self.text_of(s)).collect();
        let key = page_key(&text);
        Said { text, key }
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
        self.remove_rules_read(id);
    }

    /// The rules read from an item removed: their facts no longer hold
    /// (Notebook::facts_in), so the world evaluates them no more.
    fn remove_rules_read(&mut self, id: u64) {
        let read_by = &self.read_by;
        self.constraints
            .retain(|seq, _| read_by.get(seq) != Some(&id));
    }

    fn spans_valid(&self, spans: &[Span]) -> bool {
        spans.iter().all(|s| {
            self.added.get(&s.seq).is_some_and(|cs| {
                s.start
                    .checked_add(s.len)
                    .is_some_and(|end| end <= cs.len())
            })
        })
    }

    /// Apply an entry to the outline; false if it does not apply, as when
    /// it names an item that is not there. Each kind of entry, and each
    /// kind of edit, is one method below.
    pub fn apply(&mut self, seq: u64, kind: EntryKind, body: &Value) -> bool {
        let b = Body::of(body);
        let done = match kind {
            EntryKind::Edit => self.edit(seq, &b),
            EntryKind::Adopt => self.adopt(seq, &b),
            EntryKind::Link => self.link(seq, &b),
            EntryKind::Name => self.name(&b),
            _ => None,
        };
        done.is_some()
    }

    /// An edit of the items: their text and their places.
    fn edit(&mut self, seq: u64, b: &Body) -> Option<()> {
        match b.op().unwrap_or("") {
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
        match b.op().unwrap_or("") {
            "facts" => self.state_facts(seq, b),
            "constraint" => self.keep_constraint(seq, b),
            "layer" => self.add_layer(seq, b),
            "derive" => self.set_derived(b),
            "read" => self.record_reading(b),
            "split" | "make" | "relink" => Some(()),
            _ => None,
        }
    }

    /// Record what an item's text was read as.
    fn record_reading(&mut self, b: &Body) -> Option<()> {
        let item = self.item_in(b)?;
        let reading = reading_of(b)?;
        self.read_by
            .extend(reading.entries.iter().map(|e| (*e, item)));
        self.readings.insert(item, reading);
        Some(())
    }

    /// The items whose text is not what was last read for them.
    pub fn changed_since_read(&self) -> Vec<u64> {
        let changed = self
            .readings
            .iter()
            .filter(|(i, r)| r.text != self.text(**i));
        changed.map(|(i, _)| *i).collect()
    }

    /// Whether an item's text, as it is now, was read as statements: a
    /// formal meaning to the memex.
    pub fn has_meaning(&self, item: u64) -> bool {
        let read = self.readings.get(&item);
        read.is_some_and(|r| !r.entries.is_empty() && r.text == self.text(item))
    }

    /// The item a body names, if it is in the outline.
    fn item_in(&self, b: &Body) -> Option<u64> {
        b.item().filter(|i| self.items.contains_key(i))
    }

    // Items: their text and their places.

    fn create(&mut self, seq: u64, b: &Body) -> Option<()> {
        let under = b.under().filter(|u| self.contains(*u))?;
        self.record_text(seq, seq, b);
        let spans = spans_of(b, seq).unwrap_or_default();
        if !self.spans_valid(&spans) {
            return None;
        }
        self.items.insert(seq, Item::new(seq, under, block_of(b)));
        self.set_spans(seq, spans)?;
        self.place(seq, under, b.after());
        Some(())
    }

    fn retext(&mut self, seq: u64, b: &Body) -> Option<()> {
        let id = self.item_in(b)?;
        self.record_text(seq, id, b);
        let spans = spans_of(b, seq).filter(|s| self.spans_valid(s))?;
        self.set_spans(id, spans)
    }

    /// Set an item's spans, and its text with them: the one place either
    /// changes.
    fn set_spans(&mut self, id: u64, spans: Vec<Span>) -> Option<()> {
        let said = self.said_by(&spans);
        let item = self.items.get_mut(&id)?;
        (item.spans, item.said) = (spans, said);
        Some(())
    }

    /// Keep the characters an entry adds, written for an item.
    fn record_text(&mut self, seq: u64, item: u64, b: &Body) {
        self.added
            .insert(seq, b.add().unwrap_or("").chars().collect());
        self.owner.insert(seq, item);
    }

    fn move_item(&mut self, b: &Body) -> Option<()> {
        let (id, under, after) = (self.item_in(b)?, b.under()?, b.after());
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
        if let Some(src) = b.source() {
            self.source.insert(id, src.to_string());
        }
        Some(())
    }

    fn keep_constraint(&mut self, seq: u64, b: &Body) -> Option<()> {
        let (name, rule) = (b.name().unwrap_or("").into(), b.rule().unwrap_or("").into());
        self.constraints.insert(seq, Constraint { name, rule });
        Some(())
    }

    fn add_layer(&mut self, seq: u64, b: &Body) -> Option<()> {
        let id = self.item_in(b)?;
        let (mark, goal) = (b.mark().unwrap_or("").into(), b.goal().unwrap_or("").into());
        self.views
            .entry(id)
            .or_default()
            .push(Layer { seq, mark, goal });
        Some(())
    }

    fn set_derived(&mut self, b: &Body) -> Option<()> {
        let id = self.item_in(b)?;
        let (template, names) = (b.template().unwrap_or("").into(), b.names());
        let under = b.shown_under();
        self.derived.insert(
            id,
            Derive {
                template,
                names,
                under,
            },
        );
        Some(())
    }

    // The other kinds of entry.

    /// An adoption: of a suggested constraint, or of facts, which belong to
    /// an item if the entry names one.
    fn adopt(&mut self, seq: u64, b: &Body) -> Option<()> {
        if b.op() == Some("constraint") {
            return self.keep_constraint(seq, b);
        }
        if b.item().is_some() {
            let id = self.item_in(b)?;
            self.fact_item.insert(seq, id);
        }
        Some(())
    }

    fn link(&mut self, seq: u64, b: &Body) -> Option<()> {
        let link = link_of(seq, b)?;
        if !self.items.contains_key(&link.from) || !self.items.contains_key(&link.to) {
            return None;
        }
        if b.inline() {
            self.inline_links.insert(link.seq);
        }
        self.links.push(link);
        Some(())
    }

    fn name(&mut self, b: &Body) -> Option<()> {
        let name = b.name()?;
        let id = self.item_in(b)?;
        self.names.insert(name.to_string(), id);
        Some(())
    }

    /// The item a [[link]] of this name goes to: its page's item, if the
    /// page is one (docs/concepts-seven.md, change 4). A label of the name
    /// links to the same page, not to the item that carries it.
    pub fn resolve(&self, name: &str) -> Option<u64> {
        let key = page_key(name);
        let titled = || {
            self.in_order()
                .into_iter()
                .find(|id| same_page(&self.text(*id), &key))
        };
        self.named_page(&key).or_else(titled)
    }

    /// The item named for a page by `name`, if one is: of two names of one
    /// page ("Draft", "draft"), the first in the names' sorted order, so
    /// that it is the same each time.
    pub(crate) fn named_page(&self, key: &str) -> Option<u64> {
        let live = self
            .names
            .iter()
            .filter(|(_, id)| self.items.contains_key(id));
        let named = live.filter(|(name, _)| page_key(name) == key);
        named.map(|(_, id)| *id).next()
    }

    /// Each page that is an item, by its key, as `resolve` finds it: the
    /// item named so, else the first, in the outline's order, whose text
    /// is the page's name. All at once, for the facts of every label.
    pub fn page_items(&self) -> HashMap<String, u64> {
        let titled = self.in_order().into_iter().rev();
        let keyed = titled.filter_map(|id| Some((self.items.get(&id)?.said.key.clone(), id)));
        let mut pages: HashMap<String, u64> = keyed.collect();
        let keys: BTreeSet<String> = self.names.keys().map(|n| page_key(n)).collect();
        let named = keys
            .into_iter()
            .filter_map(|k| Some((self.named_page(&k)?, k)));
        pages.extend(named.map(|(id, k)| (k, id)));
        pages
    }

    /// The items that link to a page, by a label or a [[link]], in the
    /// outline's order.
    pub fn page_linkers(&self, key: &str) -> Vec<u64> {
        let order = self.in_order().into_iter();
        order
            .filter(|id| links_to_page(&self.text(*id), key))
            .collect()
    }

    /// The item that alone carries the label `title`, which heads the
    /// outline. It reads every item's text, so the memex finds it once
    /// for each evaluation, not at each frame.
    pub fn labelled_title(&self) -> Option<u64> {
        self.by_label("title")
    }

    /// Whether one of an item's [[links]] uses this name.
    pub fn links_by_name(&self, id: u64, name: &str) -> bool {
        crate::actions::bracketed(&self.text(id))
            .iter()
            .any(|n| n == name)
    }

    /// Whether an item's [[links]] name another item it does not link yet.
    pub fn unresolved_to(&self, id: u64, to: u64) -> bool {
        let names = crate::actions::bracketed(&self.text(id));
        let names_it = names.iter().any(|n| self.resolve(n) == Some(to));
        names_it && !self.inline_links_of(id).iter().any(|l| l.to == to)
    }

    /// An item's [[links]] that name no item (but itself), in order.
    pub fn unresolved(&self, id: u64) -> Vec<String> {
        let names = crate::actions::bracketed(&self.text(id));
        let unmatched = |n: &String| self.resolve(n).is_none_or(|t| t == id);
        names.into_iter().filter(unmatched).collect()
    }

    /// An item's links that a [[link]] in its text keeps.
    pub fn inline_links_of(&self, id: u64) -> Vec<&Link> {
        let (out, _) = self.links_of(id);
        out.into_iter()
            .filter(|l| self.inline_links.contains(&l.seq))
            .collect()
    }

    /// Links that leave or reach an item and its target or source still
    /// exist, each grouped by kind in the order the kinds first appear:
    /// the order the links pane lists them and `follow` numbers them.
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
        (by_kind(out), by_kind(into))
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

    /// An item as a change names it: its number and its text.
    pub fn labelled(&self, id: u64) -> String {
        format!("{}: “{}”", self.number(id), self.text(id))
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
    pub fn spans_written(&self, id: u64) -> Vec<bodies::Span> {
        written_spans(&self.items[&id].spans)
    }

    /// The item an address names: "#seq", an outline number ("3.2"), a
    /// name, a label that one item alone carries (`draft` or `#draft`), or
    /// the start of only one item's text.
    pub fn addressed(&self, a: &str) -> Option<u64> {
        if a.is_empty() {
            return None;
        }
        self.by_seq(a)
            .or_else(|| self.by_number(a))
            .or_else(|| self.by_name(a))
            .or_else(|| self.by_page(a))
            .or_else(|| self.by_prefix(a))
    }

    /// The item a page's name addresses, written with its # or without:
    /// the page's item, if it is one, else the one item that links to it,
    /// by a label or a [[link]] (docs/concepts.md, decision 5: a label
    /// addresses as a name did; concepts-seven.md, change 4). A page two
    /// items link to addresses neither: `go` shows the page.
    fn by_page(&self, a: &str) -> Option<u64> {
        let key = page_key(a.strip_prefix('#').unwrap_or(a));
        if key.is_empty() {
            return None;
        }
        self.named_page(&key)
            .or_else(|| self.titled_or_linked(&key))
    }

    /// In one pass over the items, as each text is costly to build: the
    /// first whose text is a page's name, else the one that links to it.
    fn titled_or_linked(&self, key: &str) -> Option<u64> {
        let mut linking = Vec::new();
        for id in self.in_order() {
            let text = self.text(id);
            if same_page(&text, key) {
                return Some(id);
            }
            if links_to_page(&text, key) {
                linking.push(id);
            }
        }
        match linking.as_slice() {
            [one] => Some(*one),
            _ => None,
        }
    }

    /// The one item that carries a label, written with its # or without.
    fn by_label(&self, a: &str) -> Option<u64> {
        let word = a.strip_prefix('#').unwrap_or(a);
        let label = tags(&format!("#{word}")).pop()?;
        if label != word.to_lowercase().replace('-', "_") {
            return None;
        }
        let carries = |id: &&u64| tags(&self.text(**id)).contains(&label);
        let mut carrying = self.items.keys().filter(carries);
        let first = *carrying.next()?;
        carrying.next().is_none().then_some(first)
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
    /// The items shown, in order, with their depth: the children of a
    /// folded item are not shown.
    pub fn visible(&self, folded: &BTreeSet<u64>) -> Vec<(u64, usize)> {
        let mut out = Vec::new();
        let mut stack: Vec<(u64, usize)> = self.top.iter().rev().map(|i| (*i, 0)).collect();
        while let Some((id, d)) = stack.pop() {
            out.push((id, d));
            if !folded.contains(&id) {
                stack.extend(self.children(id).iter().rev().map(|c| (*c, d + 1)));
            }
        }
        out
    }

    /// Each constraint, by its entry, and its name.
    pub fn constraint_names(&self) -> Vec<(u64, String)> {
        self.constraints
            .iter()
            .map(|(s, c)| (*s, c.name.clone()))
            .collect()
    }

    /// What exporting a section, or the whole outline, holds.
    pub fn exported(&self, section: Option<u64>) -> Exported {
        let roots = section.map_or(self.top.clone(), |s| vec![s]);
        let mut out = Exported::default();
        for r in roots {
            let base = self.depth_of(r);
            for id in self.branch(r) {
                out.add(self, id, base);
            }
        }
        out
    }

    /// An item and the items under it, in order.
    fn branch(&self, root: u64) -> Vec<u64> {
        self.walk(vec![root])
    }

    /// The depth of an item in the outline: a top-level item is 0.
    pub fn depth_of(&self, id: u64) -> usize {
        let mut d = 0;
        let mut at = self.parent(id);
        while let Some(p) = at.filter(|p| *p != TOP) {
            d += 1;
            at = self.parent(p);
        }
        d
    }

    /// An item as a block, its depth counted from a root's.
    fn block_at(&self, id: u64, base: usize) -> crate::docs::Block {
        crate::docs::Block::plain(
            self.depth_of(id) - base,
            self.export_style(id),
            self.text(id),
        )
    }

    /// How an item exports: by its style, or the user's own as a heading
    /// if it has children, else a paragraph.
    fn export_style(&self, id: u64) -> &str {
        let item = &self.items[&id];
        match (item.block.style.as_str(), item.children.is_empty()) {
            ("", true) => "para",
            ("", false) => "heading",
            (style, _) => style,
        }
    }

    /// The entries an item's text came from, and its own.
    fn entries_of(&self, id: u64) -> Vec<u64> {
        let spans = self.items[&id].spans.iter().map(|s| s.seq);
        spans.chain([id]).collect()
    }

    /// The items any of these imports brought in, in order.
    pub fn items_from_any(&self, imports: &[u64]) -> Vec<u64> {
        let from = |i: &u64| self.origin(*i).is_some_and(|o| imports.contains(&o));
        self.in_order().into_iter().filter(from).collect()
    }

    /// The import entry an item came from, if any.
    pub fn origin(&self, id: u64) -> Option<u64> {
        self.items.get(&id).and_then(|i| i.block.origin)
    }

    /// How a new version of a document, by its blocks' styles and texts,
    /// meets the items its earlier imports brought in.
    pub fn revised(&self, earlier: &[u64], new: &[(String, String)]) -> Revised {
        let old = self.items_from_any(earlier);
        let r = crate::docs::Revision::of(&self.keys_of(&old), new);
        let home = self
            .parent(old.first().copied().unwrap_or(TOP))
            .unwrap_or(TOP);
        let before = old.first().and_then(|first| self.before(*first));
        Revised {
            before,
            ..Revised::of(r, &old, home)
        }
    }

    /// The item before one among its siblings, if it is not the first.
    fn before(&self, id: u64) -> Option<u64> {
        let siblings = self.siblings(id);
        let at = siblings.iter().position(|s| *s == id)?;
        at.checked_sub(1).map(|i| siblings[i])
    }

    /// Each item's style and text.
    pub fn keys_of(&self, items: &[u64]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|i| (self.items[i].block.style.clone(), self.text(*i)))
            .collect()
    }

    /// Whether every item in the branch under an item is one of these.
    pub fn branch_within(&self, id: u64, these: &HashSet<u64>) -> bool {
        let mut stack = self.children(id).to_vec();
        while let Some(x) = stack.pop() {
            if !these.contains(&x) {
                return false;
            }
            stack.extend(self.children(x));
        }
        true
    }

    /// The items an import entry brought in.
    pub fn items_from(&self, import: u64) -> impl Iterator<Item = u64> + '_ {
        let from = self
            .items
            .iter()
            .filter(move |(_, i)| i.block.origin == Some(import));
        from.map(|(id, _)| *id)
    }

    /// The items, of these, whose text holds a pattern, ignoring case.
    /// The first of these items that holds a pattern, ignoring case.
    pub fn first_holding(&self, items: Vec<u64>, pattern: &str) -> Option<u64> {
        let p = pattern.to_lowercase();
        items
            .into_iter()
            .find(|id| self.text(*id).to_lowercase().contains(&p))
    }

    pub fn holding(&self, items: Vec<u64>, pattern: &str) -> Vec<u64> {
        let p = pattern.to_lowercase();
        let holds = |id: &u64| self.text(*id).to_lowercase().contains(&p);
        items.into_iter().filter(holds).collect()
    }

    /// The items in order after one, forward or back, wrapping round to
    /// it last.
    pub fn round_from(&self, at: u64, forward: bool) -> Vec<u64> {
        let order = self.in_order();
        let next = order.iter().position(|x| *x == at).map_or(0, |i| i + 1);
        let mut round = [&order[next..], &order[..next]].concat();
        if !forward {
            // Back: the items before it, nearest first; it comes last, if it
            // is in the outline at all.
            round.reverse();
            if next > 0 {
                round.rotate_left(1);
            }
        }
        round
    }

    /// The spans of an item's text after a change: those of the start and
    /// the end it keeps, and one, of no entry yet, for what it adds.
    pub fn spans_after(&self, item: u64, change: &TextChange) -> Vec<Span> {
        let len = self.text(item).chars().count();
        let start = self.slice(item, 0, change.start);
        let end = self.slice(item, len - change.end, len);
        [start, change.added_span(), end].concat()
    }

    pub fn in_order(&self) -> Vec<u64> {
        self.walk(self.top.iter().rev().copied().collect())
    }

    /// The items from a stack of them, each before the items under it, in
    /// order; the last of the stack first.
    fn walk(&self, mut stack: Vec<u64>) -> Vec<u64> {
        let mut out = Vec::with_capacity(self.items.len());
        while let Some(id) = stack.pop() {
            out.push(id);
            stack.extend(self.children(id).iter().rev());
        }
        out
    }
}

/// Links grouped by kind, in the order the kinds first appear; within a
/// kind, in their own order.
fn by_kind(mut links: Vec<&Link>) -> Vec<&Link> {
    let kinds: Vec<String> = links.iter().map(|l| l.kind.clone()).collect();
    links.sort_by_key(|l| kinds.iter().position(|k| *k == l.kind));
    links
}

/// Whether an address is written as an item's number: parts of digits,
/// split by dots, as `2` or `1.4`.
pub fn reads_as_number(a: &str) -> bool {
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    a.split('.').all(digits)
}

/// A page's key: its name in lower case, a run of spaces or a hyphen as an
/// underscore, as a label is read (`tags`), so that `#buran_flight`,
/// `[[Buran flight]]` and `[[buran-flight]]` name one page.
pub fn page_key(name: &str) -> String {
    page_chars(name).collect()
}

/// A name's characters as its page's key has them.
fn page_chars(name: &str) -> impl Iterator<Item = char> + '_ {
    let words = name.split_whitespace().enumerate();
    let joined = words.flat_map(|(i, w)| (i > 0).then_some('_').into_iter().chain(w.chars()));
    joined
        .flat_map(char::to_lowercase)
        .map(|c| if c == '-' { '_' } else { c })
}

/// Whether a text is a page's name, compared a character at a time, so
/// that most texts are told apart at their first.
pub(crate) fn same_page(text: &str, key: &str) -> bool {
    page_chars(text).eq(key.chars())
}

/// Whether a text links to a page: by a label, or by a [[link]].
pub fn links_to_page(text: &str, key: &str) -> bool {
    labels_page(text, key) || brackets_page(text, key)
}

/// Whether a text carries a page's label; one with no `#` is passed
/// over unread.
pub(crate) fn labels_page(text: &str, key: &str) -> bool {
    text.contains('#') && tags(text).iter().any(|t| t == key)
}

/// Whether a text's [[links]] name a page; one with no `[[` is passed
/// over unread.
fn brackets_page(text: &str, key: &str) -> bool {
    let named = |n: &String| page_key(n) == key;
    text.contains("[[") && crate::actions::bracketed(text).iter().any(named)
}

/// A text's #tags, each once, in order: a # at the start or after a space,
/// then a letter, then letters, digits, hyphens and underscores; in lower
/// case, hyphens made underscores, as the reader writes names. Not C#,
/// #1, a heading's "# ", or a web address's /#anchor.
pub fn tags(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        let tag = word.strip_prefix('#').and_then(tag_of);
        if let Some(t) = tag.filter(|t| !found.contains(t)) {
            found.push(t);
        }
    }
    found
}

/// The tag a word after its # starts with, if it starts with a letter.
fn tag_of(rest: &str) -> Option<String> {
    let tag: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let starts = tag.chars().next().is_some_and(char::is_alphabetic);
    starts.then(|| page_key(&tag))
}

/// A read record's reading: the text, its entries and its statements.
fn reading_of(b: &Body) -> Option<Reading> {
    let entries = b.entries()?;
    let text = b.read_text().unwrap_or("").to_string();
    let said = said_of(b.said());
    Some(Reading {
        text,
        entries,
        said,
    })
}

/// A reading's statements, each [kind, text]; none for a record written
/// before readings kept them.
fn said_of(list: Option<&[Value]>) -> Vec<(String, String)> {
    let pairs = list.into_iter().flatten();
    let pair = |p: &Value| {
        Some((
            p.get(0)?.as_str()?.to_string(),
            p.get(1)?.as_str()?.to_string(),
        ))
    };
    pairs.filter_map(pair).collect()
}

/// A link entry's link: its ends, and its type (link, if none).
fn link_of(seq: u64, b: &Body) -> Option<Link> {
    let (from, to) = (b.from()?, b.to()?);
    let kind = b.link_type().unwrap_or("link").to_string();
    Some(Link {
        seq,
        from,
        to,
        kind,
    })
}

/// Whether a text reads as a name: 3 to 60 characters, six words at
/// most, not ending in a sentence's end (guide § 3.4).
pub fn reads_as_name(text: &str) -> bool {
    let text = text.trim();
    let n = text.chars().count();
    let words = text.split_whitespace().count();
    (3..=60).contains(&n) && words <= 6 && !text.ends_with(['.', '?', '!'])
}

/// Whether a text holds a name as whole words: not inside a longer word.
pub(crate) fn names_in(text: &str, name: &str) -> bool {
    let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    text.match_indices(name).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + name.len()..].chars().next();
        !word(before) && !word(after)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tags_are_words_after_a_hash() {
        // Catches a tag missed, kept with its punctuation or case, kept
        // twice, or found where # is not a tag: C#, #1, "# heading",
        // example.com/#anchor.
        let text = "Buran #reusable, #Soviet-space! #reusable C# #1 # heading example.com/#anchor";
        assert_eq!(tags(text), ["reusable", "soviet_space"]);
        assert!(tags("").is_empty());
    }

    fn create(o: &mut Outline, seq: u64, under: u64, after: Option<u64>, text: &str) {
        let n = text.chars().count();
        let b = json!({"op":"create","under":under,"after":after,"add":text,"spans":[[0,0,n]]});
        assert!(o.apply(seq, EntryKind::Edit, &b), "create {seq}");
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
    fn a_span_too_long_for_any_text_does_not_apply() {
        // Catches a span from the journal whose start and length overflow
        // when added: a panic in a debug build, a wrapped sum accepted in
        // a release build (the code review, E6).
        let mut o = branch();
        let b = json!({"op":"text","item":2,"add":"","spans":[[2,1,u64::MAX]]});
        assert!(!o.apply(4, EntryKind::Edit, &b));
        assert_eq!(o.text(2), "b");
    }

    #[test]
    fn a_round_back_from_outside_the_outline_holds_every_item() {
        // Catches a backward round from TOP, or a deleted item, that
        // dropped the last item, so a backward Leap from there never found
        // a match in it (the code review, E3). Order: 1, 2, 3.
        let o = branch();
        assert_eq!(o.round_from(TOP, false), [3, 2, 1]);
        assert_eq!(o.round_from(TOP, true), [1, 2, 3]);
        assert_eq!(o.round_from(2, false), [1, 3, 2], "from an item, it last");
        assert_eq!(o.round_from(2, true), [3, 1, 2]);
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
    fn a_slice_past_a_span_leaves_it_out() {
        // Catches the part of a span that the slice does not reach
        // computed anyway: its length underflows, which panics in a debug
        // build (the end of an item of two spans, kept by an edit).
        let mut o = Outline::default();
        create(&mut o, 2, TOP, None, "The food is 400.");
        let mut spans = o.slice(2, 0, 12);
        spans.push(Span {
            seq: 0,
            start: 0,
            len: 4,
        });
        let b = json!({"op":"text","item":2,"add":"900.","spans":written_spans(&spans)});
        assert!(o.apply(3, EntryKind::Edit, &b));
        let end = o.slice(2, 14, 16);
        assert_eq!(
            end,
            [Span {
                seq: 3,
                start: 2,
                len: 2
            }]
        );
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
            EntryKind::Edit,
            &json!({"op":"text","item":2,"add":"1988","spans":written_spans(&spans)})
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
        let b = json!({"op":"text","item":2,"add":"1988","spans":written_spans(&spans)});
        assert!(o.apply(3, EntryKind::Edit, &b));
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
        let flew = written_spans(&o.slice(2, 6, 10));
        let b = json!({"op":"create","under":0,"after":2,"add":"","spans":flew});
        assert!(o.apply(3, EntryKind::Edit, &b));
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
            EntryKind::Edit,
            &json!({"op":"create","under":0,"after":2,"add":"Timeline: ","spans":
            written_spans(&[Span { seq: 0, start: 0, len: 10 }, spans[0]])})
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
                EntryKind::Edit,
                &json!({"op":"move","item":2,"under":3,"after":null})
            ),
            "into own branch"
        );
        assert!(o.apply(
            5,
            EntryKind::Edit,
            &json!({"op":"move","item":3,"under":0,"after":2})
        ));
        assert_eq!(o.in_order(), vec![2, 3]);
        assert!(o.apply(6, EntryKind::Edit, &json!({"op":"delete","item":2})));
        assert!(!o.apply(
            7,
            EntryKind::Edit,
            &json!({"op":"text","item":2,"add":"x","spans":[[0,0,1]]})
        ));
        assert!(!o.apply(
            8,
            EntryKind::Link,
            &json!({"from":2,"to":3,"type":"supports"})
        ));
        assert!(!o.apply(
            9,
            EntryKind::Edit,
            &json!({"op":"create","under":2,"after":null,"add":"","spans":[]})
        ));
        assert!(
            !o.apply(
                10,
                EntryKind::Edit,
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
        assert!(o.apply(4, EntryKind::Link, &json!({"from":2,"to":3,"type":"cites"})));
        assert!(o.apply(5, EntryKind::Name, &json!({"name":"src","item":3})));
        let (out, into) = o.links_of(3);
        assert!(out.is_empty());
        assert_eq!(into[0].from, 2);
        assert_eq!(o.names["src"], 3);
        assert!(o.apply(6, EntryKind::Edit, &json!({"op":"delete","item":2})));
        assert!(o.links_of(3).1.is_empty());
    }
}
