//! The verbs a right-click offers for an item (docs/concepts-next.md
//! § How far, move 1): each listed verb that applies to the item as it is
//! now, after what its presentation offers and what plumbing offers. Each
//! runs as the verb typed alone runs, so a noun with a default takes it,
//! and the offer's words name the default; a noun with none is asked for,
//! and the offer's words end in an ellipsis. A verb with a key of habit
//! names it, so the menu teaches the key.

use crate::app::App;
use crate::outline::tags;
use crate::plumb::Offer;
use crate::translators::offer;

impl App {
    /// The verbs that apply to an item, the cursor on it: the most used
    /// first, and `delete` last, as desktop menus put it.
    pub(crate) fn verb_offers(&self, id: u64) -> Vec<Offer> {
        let mut offers = vec![
            self.towards("link it to", "link", ""),
            self.towards("move it after", "move", "Ctrl+X, Ctrl+V"),
            habit("copy it after itself", "copy", "Ctrl+C, Ctrl+V"),
            habit("a new item after it", "new", "Enter"),
        ];
        offers.extend(self.show_offer(id));
        offers.push(offer("show another item's text after it…", "transclude"));
        offers.extend(self.made_offer(id));
        offers.extend(self.check_offers(id));
        offers.extend(self.suppose_offer(id));
        offers.extend(label_offers(&self.notebook.outline.text(id)));
        offers.extend(self.retract_offer(id));
        offers.push(self.delete_offer(id));
        offers
    }

    /// A verb whose noun defaults to the item last jumped from: its words
    /// name that item, or, with none, end in an ellipsis, since it asks.
    fn towards(&self, words: &str, line: &str, keys: &str) -> Offer {
        let said = match self.jumped_from(self.cursor()) {
            Some(n) => format!("{words} {n}, the item you came from"),
            None => format!("{words} an item…"),
        };
        habit(&said, line, keys)
    }

    /// `show`, as a chart, if the item is no view yet.
    fn show_offer(&self, id: u64) -> Option<Offer> {
        let viewed = self.notebook.outline.views.contains_key(&id);
        (!viewed).then(|| offer("show it as a chart", "show"))
    }

    /// `adopt fix`, if the item's [[links]] name items that are not there.
    fn made_offer(&self, id: u64) -> Option<Offer> {
        let names = self.notebook.outline.unresolved(id);
        let said = format!("make the items its [[links]] name: {}", names.join(", "));
        (!names.is_empty()).then(|| offer(&said, "adopt fix"))
    }

    /// `check`, for each kind the item has: a quotation imported from a
    /// document, a source it states.
    fn check_offers(&self, id: u64) -> Vec<Offer> {
        let quote = self.notebook.quote(id).is_ok();
        let source = self.states_a_source(id);
        let quote = quote.then(|| offer("check its quotation", "check quote"));
        let source = source.then(|| offer("check its source", "check source"));
        quote.into_iter().chain(source).collect()
    }

    /// `suppose`, if the memex reads the item as statements.
    fn suppose_offer(&self, id: u64) -> Option<Offer> {
        let reads = self.statement_of(id).is_some();
        reads.then(|| offer("suppose what it states, in a world of its own", "suppose"))
    }

    /// `retract`, if the item has an entry of its own to take back, and
    /// its presentation does not offer that entry already, as an
    /// adoption taken back.
    fn retract_offer(&self, id: u64) -> Option<Offer> {
        let last = self.last_retractable(id)?;
        let adopted = self.last_adoption(id).map(|seq| seq.to_string());
        let said = format!("retract its last entry, {last}");
        (adopted.as_deref() != Some(&last)).then(|| offer(&said, "retract"))
    }

    /// `delete`, named for what it deletes.
    fn delete_offer(&self, id: u64) -> Offer {
        match self.notebook.outline.children(id).is_empty() {
            true => offer("delete it", "delete"),
            false => offer("delete it and its branch", "delete"),
        }
    }
}

/// `find`, for each label the item's text carries.
fn label_offers(text: &str) -> Vec<Offer> {
    let found = |t: String| {
        offer(
            &format!("find the items labelled #{t}"),
            &format!("find #{t}"),
        )
    };
    tags(text).into_iter().map(found).collect()
}

/// An offer, with the keys of habit that do the same, if any.
fn habit(said: &str, line: &str, keys: &str) -> Offer {
    match keys.is_empty() {
        true => offer(said, line),
        false => offer(&format!("{said}  ({keys})"), line),
    }
}
