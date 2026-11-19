//! Links typed in an item's text, as `[[Buran flight]]`: each is a link
//! to the item named so, else to the item whose text it is, else to a new
//! item holding it. The text keeps its links: when it changes, the links
//! it no longer has are taken back and those it gains are written, at the
//! pause, when the text is read.

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

    /// The item a [[link]] goes to: the one named so, else the first whose
    /// text it is (ignoring case), else a new item holding it, at the foot
    /// of the top level, whose text waits to be read. Never the linking
    /// item itself.
    fn inline_target(&mut self, from: u64, name: &str) -> Option<u64> {
        let found = self.named_or_titled(name).filter(|t| *t != from);
        found.or_else(|| self.new_target(name))
    }

    /// The item named so, else the first whose text it is.
    fn named_or_titled(&self, name: &str) -> Option<u64> {
        let named = self.notebook.outline.names.get(name).copied();
        named.or_else(|| self.titled(name))
    }

    /// The first item whose text is this, ignoring case.
    fn titled(&self, name: &str) -> Option<u64> {
        let outline = &self.notebook.outline;
        let mut order = outline.in_order().into_iter();
        order.find(|i| outline.text(*i).trim().eq_ignore_ascii_case(name))
    }

    /// A new item holding a [[link]]'s text, at the foot of the top level.
    fn new_target(&mut self, name: &str) -> Option<u64> {
        let last = self.last_top_item();
        let spans = json!([[0, 0, name.chars().count()]]);
        let made = self.create_after(last, name, spans);
        let item = made
            .inspect_err(|why| self.session.message = why.clone())
            .ok()?;
        self.will_read(item, item);
        Some(item)
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
