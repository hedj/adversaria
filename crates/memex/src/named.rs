//! What names an item with no link entry (docs/concepts-seven.md, change
//! 4): the items that label it, if it is a page, and those that mention
//! its text, not linked. Each text is costly to build, so the texts are
//! built once, and only for an item that a name is given or whose text
//! reads as a name, so that a long text costs no pass over the items.

use crate::outline::{Outline, labels_page, names_in, page_key, reads_as_name, same_page};

/// What names an item with no link entry.
#[derive(Debug, Default, PartialEq)]
pub struct NamedBy {
    /// The items whose labels name it, as a page.
    pub labels: Vec<u64>,
    /// The items whose text names its text as whole words, in any case,
    /// and that do not link to it, by a link or by a label.
    pub mentions: Vec<u64>,
}

/// Every item's text, in the outline's order, built once.
struct Texts(Vec<(u64, String)>);

impl Texts {
    fn of(o: &Outline) -> Texts {
        Texts(o.in_order().into_iter().map(|i| (i, o.text(i))).collect())
    }

    /// The first item whose text is a page's name.
    fn first_titled(&self, key: &str) -> Option<u64> {
        self.0
            .iter()
            .find(|(_, t)| same_page(t, key))
            .map(|(i, _)| *i)
    }

    /// The items, but one, whose labels name any of some pages.
    fn labelling(&self, keys: &[String], but: u64) -> Vec<u64> {
        self.ids_where(but, |t| keys.iter().any(|k| labels_page(t, k)))
    }

    /// The items, but one, whose text names a name, in lower case, as
    /// whole words, in any case.
    fn mentioning(&self, name: &str, but: u64) -> Vec<u64> {
        self.ids_where(but, |t| names_in(&t.to_lowercase(), name))
    }

    /// The items, but one, whose text passes a test.
    fn ids_where(&self, but: u64, test: impl Fn(&str) -> bool) -> Vec<u64> {
        let kept = self.0.iter().filter(|(i, t)| *i != but && test(t));
        kept.map(|(i, _)| *i).collect()
    }
}

impl Outline {
    /// What names an item with no link entry: the items that label it, if
    /// it is a page, and its unlinked mentions.
    pub fn named_by(&self, id: u64) -> NamedBy {
        let text = self.text(id);
        let named = self.names.values().any(|i| *i == id);
        if !named && !reads_as_name(&text) {
            return NamedBy::default();
        }
        let texts = Texts::of(self);
        let labels = texts.labelling(&self.page_keys_of(id, &texts), id);
        let mentions = self.unlinked_mentions(id, &text, &texts, &labels);
        NamedBy { labels, mentions }
    }

    /// The keys of the pages an item is: each that it may be, and that
    /// is it, as `resolve` finds it, among the texts.
    fn page_keys_of(&self, id: u64, texts: &Texts) -> Vec<String> {
        let is_it = |k: &String| self.named_page(k).or_else(|| texts.first_titled(k)) == Some(id);
        self.possible_pages(id).into_iter().filter(is_it).collect()
    }

    /// The keys of the pages an item may be: each name given it, and its
    /// text, if that reads as a name.
    fn possible_pages(&self, id: u64) -> Vec<String> {
        let named = self
            .names
            .iter()
            .filter(|(_, i)| **i == id)
            .map(|(n, _)| page_key(n));
        let text = self.text(id);
        let titled = reads_as_name(&text).then(|| page_key(&text));
        let mut keys: Vec<String> = named.chain(titled).collect();
        keys.sort();
        keys.dedup();
        keys
    }

    /// The items whose text names an item's text, if that reads as a name,
    /// and that do not link to it, by a link or by a label.
    fn unlinked_mentions(&self, id: u64, text: &str, texts: &Texts, labels: &[u64]) -> Vec<u64> {
        let name = text.trim().to_lowercase();
        if !reads_as_name(&name) {
            return vec![];
        }
        let linking = self.linking(id, labels);
        let mentions = texts.mentioning(&name, id).into_iter();
        mentions.filter(|i| !linking.contains(i)).collect()
    }

    /// The items that link to an item: by a link, or by a label.
    fn linking(&self, id: u64, labels: &[u64]) -> Vec<u64> {
        let into = self.links_of(id).1.into_iter().map(|l| l.from);
        into.chain(labels.iter().copied()).collect()
    }
}
