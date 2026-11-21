//! Presentation translators, after CLIM: what a right-click on a
//! presented object offers, by its type, and only what applies to the
//! object as it is now; and what it supplies to a command's slot that
//! takes its type, when a command line waits for one (docs/design.md,
//! Presentations). Each offer is a command line, run as a bound key runs
//! it.

use crate::actions::{Slot, sentences};
use crate::app::App;
use crate::plumb::Offer;
use crate::present::Presentation;

impl App {
    /// What a right-click on a presentation offers for its object.
    pub(crate) fn translators(&self, p: &Presentation) -> Vec<Offer> {
        use Presentation as P;
        match p {
            P::Item { id, .. } | P::Link { to: id } => self.item_offers(*id),
            P::Fact { item, .. } => vec![self.go_offer(*item, "the item that states it")],
            P::Constraint {
                entry, item, under, ..
            } => self.mark_offers(*entry, *item, *under),
            P::Layer { entry, view } => self.layer_offers(*entry, *view),
            P::Change(entry) => change_offers(*entry),
            P::Suggestion(n) => suggestion_offers(*n),
            P::Command(line) => vec![offer(&format!("run {line}"), line)],
        }
    }

    /// What a presentation supplies to a slot, if the slot takes its
    /// type: an item's address, an entry's number, a suggestion's number.
    pub(crate) fn supplied(&self, p: &Presentation, slot: Slot) -> Option<String> {
        match slot {
            Slot::Item => self.address_of(p),
            Slot::Entry => entry_of(p).map(|e| e.to_string()),
            Slot::Suggestion => suggestion_of(p).map(|n| n.to_string()),
            Slot::Word | Slot::Text => None,
        }
    }

    /// The address of the item a presentation stands for or rests on.
    fn address_of(&self, p: &Presentation) -> Option<String> {
        Some(self.notebook.outline.number(self.item_of(p)?))
    }

    /// The item a presentation stands for or rests on, if one.
    fn item_of(&self, p: &Presentation) -> Option<u64> {
        use Presentation as P;
        match p {
            P::Item { id, .. } | P::Link { to: id } => Some(*id),
            P::Fact { item, .. } => Some(*item),
            P::Constraint { item, .. } => *item,
            P::Layer { view, .. } => Some(*view),
            _ => None,
        }
    }

    /// What an item offers, the cursor on it: its claim questioned or
    /// adopted, its adoption taken back, its sentences split, its branch
    /// folded or shown.
    fn item_offers(&self, id: u64) -> Vec<Offer> {
        let mut offers = self.claim_offers(id);
        let text: Vec<char> = self.notebook.outline.text(id).chars().collect();
        if sentences(&text).len() > 1 {
            offers.push(offer("split it into its sentences", "split"));
        }
        offers.extend(self.fold_offer(id));
        offers
    }

    /// What an item's claims offer: questioned or adopted, if it states
    /// any; its adoption taken back, if it has one.
    fn claim_offers(&self, id: u64) -> Vec<Offer> {
        let mut offers = Vec::new();
        if !self.claims_of(id).is_empty() {
            offers.push(offer("question its claim", "interrogate"));
            offers.push(offer("adopt its claims", "adopt"));
        }
        if self.last_adoption(id).is_some() {
            offers.push(offer("take back its adoption", "unadopt"));
        }
        offers
    }

    /// Its branch folded or shown, if it has items under it.
    fn fold_offer(&self, id: u64) -> Option<Offer> {
        if self.notebook.outline.children(id).is_empty() {
            return None;
        }
        Some(match self.session.view.folded.contains(&id) {
            true => offer("show its branch", "unfold"),
            false => offer("hide its branch", "fold"),
        })
    }

    /// What a constraint offers: under a mark, first what the item it is
    /// under offers, since the mark is a finding about that item; then
    /// the rule's.
    fn mark_offers(&self, entry: u64, item: Option<u64>, under: Option<u64>) -> Vec<Offer> {
        let mut offers = under.map(|u| self.item_offers(u)).unwrap_or_default();
        offers.extend(self.constraint_offers(entry, item));
        offers
    }

    /// What a constraint offers: its rule, if an item states it, and
    /// taking it back.
    fn constraint_offers(&self, entry: u64, item: Option<u64>) -> Vec<Offer> {
        let go = item.map(|i| self.go_offer(i, "the item that states the rule"));
        let retract = offer("retract the constraint", &format!("retract {entry}"));
        go.into_iter().chain([retract]).collect()
    }

    /// What a layer offers: its view, and taking it back.
    fn layer_offers(&self, entry: u64, view: u64) -> Vec<Offer> {
        let retract = offer("retract this layer", &format!("retract {entry}"));
        vec![retract, self.go_offer(view, "its view")]
    }

    /// Going to an item, by its number, named as what it is.
    fn go_offer(&self, item: u64, what: &str) -> Offer {
        let at = self.notebook.outline.number(item);
        offer(&format!("go to {what}, {at}"), &format!("go {at}"))
    }

    /// What a presented object is called in a menu's title: its text,
    /// name or number.
    pub(crate) fn presented_name(&self, p: &Presentation) -> String {
        use Presentation as P;
        let outline = &self.notebook.outline;
        match p {
            P::Item { id, .. } | P::Link { to: id } => outline.text(*id),
            P::Fact { entry, .. } => self.fact_shown(*entry),
            P::Constraint { entry, .. } => self.constraint_name(*entry),
            P::Layer { entry, .. } => format!("layer of entry {entry}"),
            P::Change(entry) => format!("change {entry}"),
            P::Suggestion(n) => format!("suggestion {n}"),
            P::Command(line) => line.clone(),
        }
    }

    /// A fact, by the entry that states it, as Prolog.
    fn fact_shown(&self, entry: u64) -> String {
        let facts = self.notebook.facts_now().unwrap_or_default();
        let fact = facts.iter().find(|(s, _)| *s == entry);
        fact.map_or_else(
            || format!("fact of entry {entry}"),
            |(_, t)| crate::reason::show(t),
        )
    }

    /// A constraint's name, by its entry.
    fn constraint_name(&self, entry: u64) -> String {
        let eval = self.reason.eval[0].as_ref();
        let c = eval.and_then(|e| e.constraint_of(entry));
        c.map_or_else(
            || format!("constraint {entry}"),
            |c| c.constraint.name.clone(),
        )
    }
}

/// The journal entry a presentation stands for, if one: a layer, a
/// change, a constraint or a fact, each by the entry that made it.
fn entry_of(p: &Presentation) -> Option<u64> {
    use Presentation as P;
    match p {
        P::Layer { entry, .. } | P::Change(entry) => Some(*entry),
        P::Constraint { entry, .. } | P::Fact { entry, .. } => Some(*entry),
        _ => None,
    }
}

/// The number of the suggestion a presentation stands for, if one.
fn suggestion_of(p: &Presentation) -> Option<usize> {
    match p {
        Presentation::Suggestion(n) => Some(*n),
        _ => None,
    }
}

/// What a revision's change offers: rejecting it, or keeping the rest.
fn change_offers(entry: u64) -> Vec<Offer> {
    let reject = offer("reject this change", &format!("retract {entry}"));
    vec![reject, offer("keep the changes not rejected", "commit")]
}

/// What a suggestion offers: accepting or dismissing it.
fn suggestion_offers(n: usize) -> Vec<Offer> {
    let accept = offer("accept it", &format!("accept {n}"));
    vec![accept, offer("dismiss it", &format!("dismiss {n}"))]
}

/// An offer: the menu's words, and the command line.
fn offer(label: &str, line: &str) -> Offer {
    Offer {
        label: label.to_string(),
        line: line.to_string(),
    }
}
