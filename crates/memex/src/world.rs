//! A world to reason over, loaded into an engine, and what its nodes find
//! (docs/design.md § Dataflow).

use crate::engine::{Engine, told};
use crate::outline::{Outline, page_key};
use crate::reason::{ConstraintEval, Nodes, WorldEval, marks};
use format::terms::Term;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

/// A world: the facts stated in it, its outline (the notebook's own, for
/// the current world, which is not copied), the Wikidata subset imported in
/// it (its entry and blob), and the journal's length.
pub struct World<'a> {
    pub facts: Vec<(u64, Term)>,
    pub outline: Cow<'a, Outline>,
    pub import: Option<(u64, PathBuf)>,
    pub at: usize,
}

impl World<'_> {
    /// What reasoning finds in the world, in the engine in `slot`, which is
    /// started only if the world has facts.
    /// A world with neither facts nor items is not read: there is nothing
    /// to reason over. One with items alone is: views, questions and
    /// suggestions read the outline's facts.
    pub fn evaluate(self, slot: &mut Option<Engine>) -> Result<WorldEval, String> {
        if self.facts.is_empty() && self.outline.top.is_empty() {
            return Ok(self.unread());
        }
        self.read_in(slot)
    }

    fn read_in(self, slot: &mut Option<Engine>) -> Result<WorldEval, String> {
        let engine = started(slot)?;
        self.load_and_read(engine)
    }

    fn unread(self) -> WorldEval {
        WorldEval {
            facts: self.facts,
            at: self.at,
            ..Default::default()
        }
    }

    fn load_and_read(self, engine: &mut Engine) -> Result<WorldEval, String> {
        self.load(engine)?;
        self.read(engine)
    }

    // Loading.

    fn load(&self, engine: &mut Engine) -> Result<(), String> {
        self.load_facts(engine)?;
        self.load_import(engine)
    }

    fn load_facts(&self, engine: &mut Engine) -> Result<(), String> {
        engine.sync(&self.with_outline()).map_err(told)
    }

    fn load_import(&self, engine: &mut Engine) -> Result<(), String> {
        let import = self.import.as_ref().map(|(s, f)| (*s, f.as_path()));
        engine.wikidata(import).map_err(told)
    }

    /// The world's facts, and the facts its outline states of its items.
    fn with_outline(&self) -> Vec<(u64, Term)> {
        let mut all = self.facts.clone();
        all.extend(outline_facts(&self.outline));
        all
    }

    // Reading.

    fn read(self, engine: &mut Engine) -> Result<WorldEval, String> {
        Ok(WorldEval {
            constraints: self.read_constraints(engine)?,
            nodes: self.read_nodes(engine),
            ..self.unread()
        })
    }

    /// Each constraint of this world's outline, read. The outline has none
    /// whose definition the world retracts: its replay leaves those out.
    fn read_constraints(&self, engine: &mut Engine) -> Result<Vec<ConstraintEval>, String> {
        let constraints = self.outline.constraints.iter();
        constraints
            .map(|(seq, c)| ConstraintEval::read(engine, *seq, c))
            .collect()
    }

    fn read_nodes(&self, engine: &mut Engine) -> Nodes {
        let derived = self.outline.derived.keys().copied().collect();
        let (derived, mut failed) = read(engine, derived);
        let (layers, layers_failed) = read(engine, self.layer_seqs());
        failed.extend(layers_failed);
        let layers = layers.into_iter().map(|(seq, v)| (seq, marks(&v)));
        Nodes {
            derived,
            failed,
            layers: layers.collect(),
        }
    }

    fn layer_seqs(&self) -> Vec<u64> {
        self.outline
            .views
            .values()
            .flatten()
            .map(|l| l.seq)
            .collect()
    }
}

/// Each node's value, and each node that cannot be read, with what went
/// wrong.
fn read(engine: &mut Engine, ids: Vec<u64>) -> (BTreeMap<u64, Term>, BTreeMap<u64, String>) {
    let (mut values, mut failed) = (BTreeMap::new(), BTreeMap::new());
    for id in ids {
        match engine.node(id) {
            Ok(v) => drop(values.insert(id, v)),
            Err(e) => drop(failed.insert(id, told(e))),
        }
    }
    (values, failed)
}

/// The engine in a slot, started if there is none, or if the one there has
/// died: a new one is loaded from the world's facts at its first sync, so
/// nothing is lost with the old.
pub fn started(slot: &mut Option<Engine>) -> Result<&mut Engine, String> {
    if slot.as_mut().is_some_and(Engine::is_dead) {
        *slot = None;
    }
    if slot.is_none() {
        *slot = Some(Engine::start().map_err(|e| format!("cannot start Prolog: {e}"))?);
    }
    Ok(slot.as_mut().unwrap())
}

/// What an outline states of its items, as facts: item_text(Id, Text) for
/// each, item_under(Id, Parent) for each that is not top-level,
/// item_tag(Id, Tag) for each #tag in its text, and item_link(From, To,
/// Type) for each link between two of its items.
pub fn outline_facts(o: &Outline) -> Vec<(u64, Term)> {
    let pages = o.page_items();
    let items = o.in_order().into_iter().flat_map(|id| item_facts(o, id));
    let links = o
        .in_order()
        .into_iter()
        .flat_map(|id| page_facts(o, &pages, id));
    items.chain(links).chain(link_facts(o)).collect()
}

/// item_link(Id, Page, label) for each label in an item's text, and
/// item_link(Id, Page, link) for each [[link]] to a page that is no item
/// (one that is has a link entry, and its fact, once the text is read):
/// the page by its item, else by its key (docs/concepts-seven.md,
/// change 4).
fn page_facts(o: &Outline, pages: &HashMap<String, u64>, id: u64) -> Vec<(u64, Term)> {
    let linked = linked_pages(&o.text(id), pages);
    linked
        .into_iter()
        .map(|(page, kind)| (id, page_link(id, page, kind)))
        .collect()
}

/// The pages a text links to that no link entry records, each with how:
/// each label's, and each [[link]]'s that is no item.
fn linked_pages(text: &str, pages: &HashMap<String, u64>) -> Vec<(Term, &'static str)> {
    let labels = crate::outline::tags(text).into_iter().map(|k| (k, "label"));
    let unmade = unmade_links(text, pages).into_iter().map(|k| (k, "link"));
    labels
        .chain(unmade)
        .map(|(k, kind)| (page_term(pages, k), kind))
        .collect()
}

/// The keys of a text's [[links]] to pages that are no items.
fn unmade_links(text: &str, pages: &HashMap<String, u64>) -> Vec<String> {
    let keys = crate::actions::bracketed(text)
        .into_iter()
        .map(|n| page_key(&n));
    keys.filter(|k| !pages.contains_key(k)).collect()
}

/// A page as a fact names it: by its item, else by its key.
fn page_term(pages: &HashMap<String, u64>, key: String) -> Term {
    pages
        .get(&key)
        .map_or(Term::Atom(key), |i| Term::Int(*i as i64))
}

/// item_link(From, To, Type) for each link whose ends are both items,
/// each by the item it is from.
fn link_facts(o: &Outline) -> impl Iterator<Item = (u64, Term)> + '_ {
    let live = o.live_links();
    live.map(|l| (l.from, item_link(l.from, l.to, &l.kind)))
}

fn item_link(from: u64, to: u64, kind: &str) -> Term {
    page_link(from, Term::Int(to as i64), kind)
}

/// item_link(From, Page, Type): a link from an item to a page, which is
/// an item's number or a name.
fn page_link(from: u64, page: Term, kind: &str) -> Term {
    let args = vec![Term::Int(from as i64), page, Term::Atom(kind.into())];
    Term::Compound("item_link".into(), args)
}

fn item_facts(o: &Outline, id: u64) -> Vec<(u64, Term)> {
    let mut out = vec![(id, item_text(o, id))];
    out.extend(item_under(o, id).map(|t| (id, t)));
    out.extend(
        crate::outline::tags(&o.text(id))
            .into_iter()
            .map(|t| (id, item_tag(id, t))),
    );
    out
}

fn item_tag(id: u64, tag: String) -> Term {
    Term::Compound(
        "item_tag".into(),
        vec![Term::Int(id as i64), Term::Atom(tag)],
    )
}

fn item_text(o: &Outline, id: u64) -> Term {
    Term::Compound(
        "item_text".into(),
        vec![Term::Int(id as i64), Term::Str(o.text(id))],
    )
}

fn item_under(o: &Outline, id: u64) -> Option<Term> {
    let p = o.parent_item(id)?;
    Some(Term::Compound(
        "item_under".into(),
        vec![Term::Int(id as i64), Term::Int(p as i64)],
    ))
}
