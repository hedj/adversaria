//! Unlinked mentions, offered as suggestions (docs/design.md,
//! Suggestions): an item whose text names another item's title, as whole
//! words, in any case, and does not link to it, by a link or a label:
//! link them? (Obsidian's unlinked mentions.) Worked out here, where the
//! texts are, by the rule prolog/dataflow.pl had: a title is a text of 3
//! to 60 characters, of at most six words between spaces, that ends in no
//! full stop, question or exclamation mark; titles are found by the first
//! of their words, so each text is looked up by its own words.

use crate::outline::{Outline, names_in, tags};
use crate::reason::Suggestion;
use format::terms::Term;
use std::collections::{BTreeSet, HashMap, HashSet};

/// What separates the words of a text, for finding titles by them.
const SEPARATORS: &[char] = &[
    ' ', '\t', '\n', '.', ',', ';', ':', '!', '?', '(', ')', '"', '[', ']',
];

impl Outline {
    /// The unlinked mentions in the outline, as suggestions, by the item
    /// that mentions and then the item it names, as Prolog ordered them.
    pub(crate) fn mention_suggestions(&self) -> Vec<Suggestion> {
        let texts: Texts = self
            .in_order()
            .into_iter()
            .map(|i| (i, self.text(i)))
            .collect();
        let pairs = self.unlinked_mentions_of(&texts).into_iter();
        pairs.map(|(from, to)| mention(&texts, from, to)).collect()
    }

    /// The pairs of items, from and to, where the first's text names the
    /// second's title and does not link to it.
    fn unlinked_mentions_of(&self, texts: &Texts) -> BTreeSet<(u64, u64)> {
        let linked = self.linked_items(texts);
        let named = Titles::of(texts).named_in_each(texts).into_iter();
        named.filter(|p| !linked.contains(p)).collect()
    }

    /// The pairs of items that a link joins, or a label of one names the
    /// other's page: from, to.
    fn linked_items(&self, texts: &Texts) -> HashSet<(u64, u64)> {
        let linked = self.live_links().map(|l| (l.from, l.to));
        linked.chain(labelled(&self.page_items(), texts)).collect()
    }
}

/// Every item's text, by the item.
type Texts = HashMap<u64, String>;

/// The pairs of items where a label of the first names the second's page.
fn labelled(pages: &HashMap<String, u64>, texts: &Texts) -> Vec<(u64, u64)> {
    let pages_of = |text: &String| {
        tags(text)
            .into_iter()
            .filter_map(|k| pages.get(&k).copied())
    };
    let each = texts
        .iter()
        .map(|(id, text)| (*id, pages_of(text).collect::<Vec<_>>()));
    each.flat_map(|(id, named)| named.into_iter().map(move |page| (id, page)))
        .collect()
}

/// The texts that read as titles, by the first of their words in lower
/// case, each with its item.
struct Titles<'a>(HashMap<String, Vec<(u64, &'a str)>>);

impl<'a> Titles<'a> {
    fn of(texts: &'a Texts) -> Titles<'a> {
        let mut index: HashMap<String, Vec<(u64, &str)>> = HashMap::new();
        for (id, text) in texts.iter().filter(|(_, t)| title_like(t)) {
            if let Some(first) = words(&text.to_lowercase()).first() {
                index
                    .entry(first.to_string())
                    .or_default()
                    .push((*id, text));
            }
        }
        Titles(index)
    }

    /// The pairs of items where the first's text names the second's
    /// title.
    fn named_in_each(&self, texts: &Texts) -> Vec<(u64, u64)> {
        let each = texts.iter();
        each.flat_map(|(from, text)| self.named_in(*from, text))
            .collect()
    }

    /// The items, other than one, whose titles a text names, each with it.
    fn named_in(&self, from: u64, text: &str) -> Vec<(u64, u64)> {
        let lower = text.to_lowercase();
        let names = |title: &str| names_in(&lower, &title.to_lowercase());
        let found = self
            .candidates(&lower)
            .filter(|(to, title)| *to != from && names(title));
        found.map(|(to, _)| (from, to)).collect()
    }

    /// The titles that start with one of a text's words, each with its item.
    fn candidates(&self, lower: &str) -> impl Iterator<Item = (u64, &'a str)> {
        let found = words(lower)
            .into_iter()
            .filter_map(|w| self.0.get(w))
            .flatten();
        found.copied().collect::<Vec<_>>().into_iter()
    }
}

/// Whether a text reads as a title.
fn title_like(text: &str) -> bool {
    let n = text.chars().count();
    let short = (3..=60).contains(&n) && text.split(' ').count() <= 6;
    short && !text.ends_with(['.', '?', '!'])
}

/// A text's words, split at its separators.
fn words(text: &str) -> Vec<&str> {
    text.split(SEPARATORS).filter(|w| !w.is_empty()).collect()
}

/// The suggestion to link an item to the item whose title it names.
fn mention(texts: &Texts, from: u64, to: u64) -> Suggestion {
    let kind = mention_term(texts, from, to);
    let evidence = format!("its text names “{}” and does not link to it", texts[&to]);
    let facts = vec![];
    Suggestion {
        kind,
        facts,
        evidence,
        score: 3,
    }
}

/// mention(From, To, Excerpt, Title), as Prolog wrote it.
fn mention_term(texts: &Texts, from: u64, to: u64) -> Term {
    let words = [excerpt(&texts[&from]), texts[&to].clone()].map(Term::Str);
    let ends = [from, to].map(|i| Term::Int(i as i64));
    Term::Compound("mention".into(), ends.into_iter().chain(words).collect())
}

/// The start of a text, for saying which item a suggestion is about.
fn excerpt(text: &str) -> String {
    match text.chars().count() <= 40 {
        true => text.to_string(),
        false => text.chars().take(37).chain(['…']).collect(),
    }
}
