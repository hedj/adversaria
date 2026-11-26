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
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// What separates the words of a text, for finding titles by them.
const SEPARATORS: &[char] = &[
    ' ', '\t', '\n', '.', ',', ';', ':', '!', '?', '(', ')', '"', '[', ']',
];

impl Outline {
    /// The unlinked mentions in the outline, as suggestions, by the item
    /// that mentions and then the item it names, as Prolog ordered them;
    /// worked out from nothing, as Mentions keeps them.
    #[cfg(test)]
    pub(crate) fn mention_suggestions(&self) -> Vec<Suggestion> {
        let mut m = Mentions::default();
        m.update(self);
        m.suggestions(self)
    }
}

/// What the unlinked mentions are worked out from, kept from one
/// evaluation to the next: each item's text as read, the titles by their
/// first word, the items whose text holds each word, and the titles each
/// text names. After a change, only the items whose text changed are read
/// again, and only they and the items holding the first word of a title
/// that came or went are looked through for titles: a keystroke changes
/// one text of 10,000.
#[derive(Default)]
pub(crate) struct Mentions {
    texts: HashMap<u64, Read>,
    titles: HashMap<String, BTreeSet<u64>>,
    holding: HashMap<String, BTreeSet<u64>>,
    named: BTreeMap<u64, BTreeSet<u64>>,
}

/// An item's text as mentions read it: as it is, in lower case, its words,
/// its labels, and, if it reads as a title, the first word of the title.
struct Read {
    text: String,
    lower: String,
    words: BTreeSet<String>,
    labels: Vec<String>,
    title: Option<String>,
}

impl Read {
    fn of(text: &str) -> Read {
        let lower = text.to_lowercase();
        let title = title_like(text).then(|| first_word(&lower)).flatten();
        let words = words(&lower).into_iter().map(String::from).collect();
        let labels = tags(text);
        let text = text.to_string();
        Read {
            text,
            lower,
            words,
            labels,
            title,
        }
    }
}

/// A text's first word.
fn first_word(text: &str) -> Option<String> {
    words(text).first().map(|w| w.to_string())
}

impl Mentions {
    /// Follow the outline: read again each item whose text changed, and
    /// look through again each text that may name a title that changed.
    pub(crate) fn update(&mut self, o: &Outline) {
        let now: HashMap<u64, &str> = o
            .in_order()
            .into_iter()
            .map(|i| (i, o.text_ref(i)))
            .collect();
        let (gone, changed) = (self.gone(&now), self.changed(&now));
        let mut retitled: Vec<String> = Vec::new();
        for id in gone.iter().chain(changed.iter().map(|(i, _)| i)) {
            retitled.extend(self.forget(*id));
        }
        for (id, text) in &changed {
            retitled.extend(self.learn(*id, text));
        }
        let read_again = changed.iter().map(|(i, _)| *i).chain(gone);
        for id in self.holders(&retitled, read_again) {
            self.look_through(id);
        }
    }

    /// The items kept whose text is gone from the outline.
    fn gone(&self, now: &HashMap<u64, &str>) -> Vec<u64> {
        let gone = self.texts.keys().filter(|i| !now.contains_key(i));
        gone.copied().collect()
    }

    /// The items whose text is new, or not what was kept.
    fn changed<'t>(&self, now: &HashMap<u64, &'t str>) -> Vec<(u64, &'t str)> {
        let differs = |(i, t): &(&u64, &&str)| self.texts.get(i).is_none_or(|r| r.text != **t);
        now.iter().filter(differs).map(|(i, t)| (*i, *t)).collect()
    }

    /// These items, and those whose text holds a word that begins a title
    /// that came or went.
    fn holders(&self, words: &[String], items: impl Iterator<Item = u64>) -> BTreeSet<u64> {
        let holding = words.iter().filter_map(|w| self.holding.get(w)).flatten();
        items.chain(holding.copied()).collect()
    }

    /// Take an item's text out of what is kept: the first word of its
    /// title, if it had one.
    fn forget(&mut self, id: u64) -> Option<String> {
        let read = self.texts.remove(&id)?;
        for w in &read.words {
            unlist(&mut self.holding, w, id);
        }
        if let Some(w) = &read.title {
            unlist(&mut self.titles, w, id);
        }
        read.title
    }

    /// Keep an item's text: the first word of its title, if it has one.
    fn learn(&mut self, id: u64, text: &str) -> Option<String> {
        let read = Read::of(text);
        for w in &read.words {
            self.holding.entry(w.clone()).or_default().insert(id);
        }
        if let Some(w) = &read.title {
            self.titles.entry(w.clone()).or_default().insert(id);
        }
        let title = read.title.clone();
        self.texts.insert(id, read);
        title
    }

    /// The titles an item's text names, as whole words, in any case: each
    /// title is found by its first word among the text's words.
    fn look_through(&mut self, id: u64) {
        let named = self.texts.get(&id).map(|r| self.titles_in(id, r));
        match named.filter(|n| !n.is_empty()) {
            Some(n) => drop(self.named.insert(id, n)),
            None => drop(self.named.remove(&id)),
        }
    }

    fn titles_in(&self, id: u64, read: &Read) -> BTreeSet<u64> {
        let candidates = read
            .words
            .iter()
            .filter_map(|w| self.titles.get(w))
            .flatten();
        let others = candidates.copied().filter(|to| *to != id);
        others.filter(|to| self.names(read, *to)).collect()
    }

    /// Whether a text names an item's title as whole words, in any case.
    fn names(&self, read: &Read, title: u64) -> bool {
        names_in(&read.lower, &self.texts[&title].lower)
    }

    /// The unlinked mentions, as suggestions, by the item that mentions
    /// and then the item it names.
    pub(crate) fn suggestions(&self, o: &Outline) -> Vec<Suggestion> {
        let linked = self.linked_items(o);
        let pairs = self
            .named
            .iter()
            .flat_map(|(from, tos)| tos.iter().map(|to| (*from, *to)));
        let unlinked = pairs.filter(|p| !linked.contains(p));
        unlinked.map(|(from, to)| self.mention(from, to)).collect()
    }

    /// The pairs of items that a link joins, or a label of one names the
    /// other's page: from, to.
    fn linked_items(&self, o: &Outline) -> HashSet<(u64, u64)> {
        let linked = o.live_links().map(|l| (l.from, l.to));
        linked.chain(self.labelled(&o.page_items())).collect()
    }

    /// The pairs of items where a label of the first names the second's
    /// page.
    fn labelled(&self, pages: &HashMap<String, u64>) -> Vec<(u64, u64)> {
        let pages_of = |r: &Read| {
            r.labels
                .iter()
                .filter_map(|l| pages.get(l).copied())
                .collect::<Vec<_>>()
        };
        let each = self.texts.iter().map(|(id, r)| (*id, pages_of(r)));
        each.flat_map(|(id, to)| to.into_iter().map(move |t| (id, t)))
            .collect()
    }

    /// The suggestion to link an item to the item whose title it names.
    fn mention(&self, from: u64, to: u64) -> Suggestion {
        let ends = [from, to].map(|i| (i, self.texts[&i].text.as_str()));
        let kind = mention_term(ends);
        let evidence = format!("its text names “{}” and does not link to it", ends[1].1);
        let facts = vec![];
        Suggestion {
            kind,
            facts,
            evidence,
            score: 3,
        }
    }
}

/// Take an item from the set kept under a word, and the word with it once
/// its set is empty.
fn unlist(index: &mut HashMap<String, BTreeSet<u64>>, word: &str, id: u64) {
    let Some(set) = index.get_mut(word) else {
        return;
    };
    set.remove(&id);
    if set.is_empty() {
        index.remove(word);
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

/// mention(From, To, Excerpt, Title), as Prolog wrote it, of two items,
/// each with its text.
fn mention_term(ends: [(u64, &str); 2]) -> Term {
    let ids = ends.map(|(i, _)| Term::Int(i as i64));
    let texts = [excerpt(ends[0].1), ends[1].1.to_string()].map(Term::Str);
    Term::Compound("mention".into(), ids.into_iter().chain(texts).collect())
}

/// The start of a text, for saying which item a suggestion is about.
fn excerpt(text: &str) -> String {
    match text.chars().count() <= 40 {
        true => text.to_string(),
        false => text.chars().take(37).chain(['…']).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::TOP;
    use format::record::EntryKind;
    use serde_json::{Value, json};

    /// The changes the test makes, each numbered by its entry.
    fn changes() -> Vec<(EntryKind, Value)> {
        let text = |item: u64, t: &str| {
            let n = t.chars().count();
            json!({"op": "text", "item": item, "add": t, "spans": [[0, 0, n]]})
        };
        let create = |t: &str| {
            let n = t.chars().count();
            json!({"op": "create", "under": TOP, "add": t, "spans": [[0, 0, n]]})
        };
        vec![
            (EntryKind::Edit, create("Buran flight")),
            (EntryKind::Edit, create("Notes on the buran flight launch")),
            (EntryKind::Edit, create("Energia")),
            (
                EntryKind::Edit,
                create("The Energia rocket carried Buran flight hardware"),
            ),
            (EntryKind::Edit, text(3, "Energia rocket")),
            (EntryKind::Edit, text(1, "Buran")),
            (EntryKind::Edit, json!({"op": "delete", "item": 3})),
            (
                EntryKind::Link,
                json!({"from": 4, "to": 1, "type": "cites"}),
            ),
            (EntryKind::Edit, text(2, "Notes #buran")),
            (EntryKind::Edit, create("A note on Buran")),
            (EntryKind::Edit, json!({"op": "delete", "item": 10})),
        ]
    }

    /// The suggestions' identities.
    fn kinds(s: Vec<Suggestion>) -> Vec<Term> {
        s.into_iter().map(|s| s.kind).collect()
    }

    /// An outline changed entry by entry, and the mentions kept as it is.
    #[derive(Default)]
    struct Followed {
        o: Outline,
        kept: Mentions,
    }

    impl Followed {
        /// Apply an entry, and check that the mentions kept are those
        /// worked out from nothing: how many there are.
        fn step(&mut self, seq: u64, (kind, body): (EntryKind, Value)) -> usize {
            assert!(self.o.apply(seq, kind, &body), "{seq}: {body}");
            self.kept.update(&self.o);
            let fresh = kinds(self.o.mention_suggestions());
            assert_eq!(
                kinds(self.kept.suggestions(&self.o)),
                fresh,
                "after entry {seq}"
            );
            fresh.len()
        }
    }

    #[test]
    fn mentions_kept_through_changes_are_those_worked_out_from_nothing() {
        // Catches a mention kept after its text, its title or its item
        // changed or went, or one missed after a title came: the kept
        // mentions follow each change, as one worked out afresh does.
        let mut f = Followed::default();
        let counts: Vec<usize> = (1..)
            .zip(changes())
            .map(|(seq, c)| f.step(seq, c))
            .collect();
        assert!(
            counts.iter().any(|n| *n >= 3),
            "too few mentions to test: {counts:?}"
        );
    }
}
