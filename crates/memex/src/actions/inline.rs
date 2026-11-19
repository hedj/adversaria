//! Links typed in an item's text, as `[[Buran flight]]`: each is a link
//! to the item named so, else to the item whose text it is. A link to no
//! item waits: the cursor's line says so, and `make` makes the item, or
//! the link is made when an item of that name or text is read. The text
//! keeps its links: when it changes, the links it no longer has are taken
//! back and those it gains are written, at the pause, when it is read.

use crate::app::App;
use crate::outline::TOP;
use serde_json::json;

impl App {
    /// Make an item's links what its text's [[links]] say; whether any
    /// entry was written.
    pub(crate) fn link_inline(&mut self, item: u64) -> bool {
        let before = self.notebook.tip;
        let text = self.notebook.outline.text(item);
        let wanted: Vec<u64> = bracketed(&text)
            .iter()
            .filter_map(|x| self.inline_target(item, x))
            .collect();
        self.retract_inline(item, &wanted);
        self.add_inline(item, &wanted);
        self.notebook.tip != before
    }

    /// Take back the item's inline links to items its text no longer
    /// links.
    fn retract_inline(&mut self, item: u64, wanted: &[u64]) {
        let links = self.notebook.outline.inline_links_of(item);
        let gone = links.iter().filter(|l| !wanted.contains(&l.to));
        let seqs: Vec<u64> = gone.map(|l| l.seq).collect();
        for target in seqs {
            let written = self.write_here("retract", json!({ "target": target }));
            self.session.report(written);
        }
    }

    /// Write the item's inline links to items it does not link yet.
    fn add_inline(&mut self, item: u64, wanted: &[u64]) {
        for to in wanted {
            let links = self.notebook.outline.inline_links_of(item);
            if links.iter().any(|l| l.to == *to) {
                continue;
            }
            let written = self.write_inline_link(item, *to);
            self.session.report(written);
        }
    }

    /// The item a [[link]] goes to, if one is named so or holds its text;
    /// never the linking item itself. A link to no item is left for the
    /// user: `make` makes it.
    fn inline_target(&self, from: u64, name: &str) -> Option<u64> {
        self.notebook.outline.resolve(name).filter(|t| *t != from)
    }

    /// Link the items whose [[links]] name an item and do not link it yet:
    /// it was just made, or its text just came to match. Whether any entry
    /// was written.
    pub(crate) fn link_to(&mut self, item: u64) -> bool {
        let outline = &self.notebook.outline;
        let waiting = |o: &u64| *o != item && outline.unresolved_to(*o, item);
        let others: Vec<u64> = outline.in_order().into_iter().filter(waiting).collect();
        let mut linked = false;
        for other in others {
            linked |= self.link_inline(other);
        }
        linked
    }

    /// Link again each item whose [[links]] use a name, after the name is
    /// given (or moves to another item). If any link changed, a closing
    /// record names the name's entry, `first`, so one undo takes the name
    /// and its links back together.
    pub(crate) fn relink_name(&mut self, name: &str, first: u64) -> Result<(), String> {
        let items = self.using_name(name);
        let mut changed = false;
        for item in &items {
            changed |= self.link_inline(*item);
        }
        if changed {
            let body = json!({"op": "relink", "name": name, "of": first, "items": items});
            self.write_here("edit", body)?;
        }
        Ok(())
    }

    /// The items whose [[links]] use a name, in order.
    fn using_name(&self, name: &str) -> Vec<u64> {
        let outline = &self.notebook.outline;
        let order = outline.in_order().into_iter();
        order.filter(|i| outline.links_by_name(*i, name)).collect()
    }

    /// Make an item for each of the target's [[links]] that names no item,
    /// at the foot of the top level, and link them; one undo takes it all
    /// back.
    pub(crate) fn cmd_make(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        self.flush_text();
        let names = self.notebook.outline.unresolved(target);
        if names.is_empty() {
            return Err("nothing to make: each of the target's [[links]] names an item".into());
        }
        let first = self.notebook.next_seq();
        let made = self.make_items(&names, first)?;
        self.link_inline(target);
        let body = json!({"op": "make", "item": target, "of": first, "made": made});
        self.write_here("edit", body)?;
        Ok(format!("made {}: {}", made.len(), names.join(", ")))
    }

    /// An item for each name, at the foot of the top level, each waiting
    /// to be read as part of what starts at the entry `first`.
    fn make_items(&mut self, names: &[String], first: u64) -> Result<Vec<u64>, String> {
        names
            .iter()
            .map(|name| self.make_item(name, first))
            .collect()
    }

    /// An item holding a name, at the foot of the top level, waiting to be
    /// read as part of what starts at the entry `first`.
    fn make_item(&mut self, name: &str, first: u64) -> Result<u64, String> {
        let spans = json!([[0, 0, name.chars().count()]]);
        let item = self.create_after(self.last_top_item(), name, spans)?;
        self.will_read(item, first);
        Ok(item)
    }

    /// The last item at the top level, or none (TOP) in an empty outline.
    fn last_top_item(&self) -> u64 {
        let outline = &self.notebook.outline;
        let mut order = outline.in_order().into_iter().rev();
        order
            .find(|i| outline.parent_or_top(*i) == TOP)
            .unwrap_or(TOP)
    }
}

/// The texts of a text's [[links]], each trimmed, in order, without the
/// empty.
pub(crate) fn bracketed(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find("[[") {
        let after = &rest[open + 2..];
        let Some(close) = after.find("]]") else {
            break;
        };
        let inner = after[..close].trim();
        if !inner.is_empty() {
            found.push(inner.to_string());
        }
        rest = &after[close + 2..];
    }
    found
}

/// A text without its links' brackets: what the reader reads.
pub(crate) fn unbracketed(text: &str) -> String {
    text.replace("[[", "").replace("]]", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_found_between_double_brackets() {
        // Catches a link missed, one kept with its spaces, an empty one
        // kept, or an unclosed one taken for a link.
        let text = "See [[Buran flight]] and [[ Energia ]], not [[]] or [[open";
        assert_eq!(bracketed(text), ["Buran flight", "Energia"]);
        assert!(bracketed("no links").is_empty());
    }

    #[test]
    fn the_reader_reads_a_text_without_its_brackets() {
        let text = "The date of [[Buran-flight]] is 1988.";
        assert_eq!(unbracketed(text), "The date of Buran-flight is 1988.");
    }
}
