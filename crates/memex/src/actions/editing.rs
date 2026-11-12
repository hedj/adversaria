//! Commands that edit the outline's items and links, and move about it
//! (docs/guide.md, § 2.4, Editing the outline).

use crate::app::{App, PaneKind, word_at};
use format::terms::{self, Term};
use serde_json::{Value, json};

impl App {
    pub(crate) fn cmd_new(&mut self, _: &[&str], target: u64) -> Result<(), String> {
        self.new_item_after(target)
    }

    pub(crate) fn cmd_name(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let n = words.join(" ");
        if n.is_empty() {
            return Err("name (as) a word".into());
        }
        self.write_here("name", json!({"name": n, "item": target}))?;
        Ok(())
    }

    /// Delete the word at the cursor, or the target's branch.
    pub(crate) fn cmd_delete(&mut self, _: &[&str], target: u64) -> Result<(), String> {
        if self.view.in_word() {
            return self.delete_word(target);
        }
        self.write_here("edit", json!({"op": "delete", "item": target}))?;
        Ok(())
    }

    fn delete_word(&mut self, item: u64) -> Result<(), String> {
        let (text, at) = self.word_cut(item);
        self.set_pos(at);
        self.write_text(item, &text)
    }

    /// An item's text without the word at the cursor, and where the word was.
    fn word_cut(&self, item: u64) -> (Vec<char>, usize) {
        cut_word(self.text_of(item), self.view.pos)
    }

    fn set_pos(&mut self, pos: usize) {
        self.view.pos = pos;
        self.view_dirty = true;
    }

    // Moving items.

    pub(crate) fn cmd_indent(&mut self, _: &[&str], target: u64) -> Result<(), String> {
        let under = self
            .outline
            .previous_sibling(target)
            .ok_or("the first item of a list cannot be indented")?;
        self.move_to_end(target, under)?;
        self.open_branch(under);
        Ok(())
    }

    pub(crate) fn cmd_outdent(&mut self, _: &[&str], target: u64) -> Result<(), String> {
        let p = self
            .outline
            .parent_item(target)
            .ok_or("a top-level item cannot be outdented")?;
        self.move_item(target, self.outline.parent_or_top(p), Some(p))
    }

    pub(crate) fn cmd_move(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let to = self
            .address(words)
            .ok_or("move (after) an item: a number, a name, or the start of its text")?;
        if self.outline.within(to, target) {
            return Err("an item cannot move into its own branch".into());
        }
        self.move_after(target, to)
    }

    /// Move an item's branch to follow another item.
    fn move_after(&mut self, item: u64, to: u64) -> Result<(), String> {
        self.move_item(item, self.outline.parent_or_top(to), Some(to))
    }

    /// Move an item's branch to the end of another item's children.
    fn move_to_end(&mut self, item: u64, under: u64) -> Result<(), String> {
        self.move_item(item, under, self.outline.last_child(under))
    }

    fn move_item(&mut self, item: u64, under: u64, after: Option<u64>) -> Result<(), String> {
        let body = json!({"op": "move", "item": item, "under": under, "after": after});
        self.write_here("edit", body)?;
        Ok(())
    }

    /// Show an item's children if they were folded away.
    fn open_branch(&mut self, id: u64) {
        if self.view.folded.remove(&id) {
            self.view_dirty = true;
        }
    }

    // Copying and transcluding.

    pub(crate) fn cmd_copy(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let to = self.address(words).ok_or("copy (after) an item")?;
        let text = self.outline.text(target);
        let n = text.chars().count();
        self.create_after(to, &text, json!([[0, 0, n]]))?;
        Ok(())
    }

    pub(crate) fn cmd_transclude(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let from = self.address(words).ok_or("transclude (from) an item")?;
        let seq = self.transclusion_after(from, target)?;
        self.go_to(seq);
        Ok(())
    }

    /// A new item after one, showing another's text in place.
    fn transclusion_after(&mut self, from: u64, after: u64) -> Result<u64, String> {
        let spans = self.outline.spans_json(from);
        self.create_after(after, "", spans)
    }

    /// A new item after one, with text and spans; its sequence number.
    fn create_after(&mut self, after: u64, add: &str, spans: Value) -> Result<u64, String> {
        let under = self.outline.parent_or_top(after);
        let body =
            json!({"op": "create", "under": under, "after": after, "add": add, "spans": spans});
        Ok(self.write_here("edit", body)?.seq)
    }

    fn go_to(&mut self, id: u64) {
        self.set_cursor(id);
        self.write_view("move", json!({}));
    }

    // Links.

    pub(crate) fn cmd_link(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let (addr, kind) = split_as(words);
        let (from, to) = self
            .link_ends(addr, target)
            .ok_or("link (from) an item (to) an item, (as) a type")?;
        self.write_link(from, to, kind)
    }

    /// The two ends words address, or the target and the one they address.
    fn link_ends(&self, words: &[&str], target: u64) -> Option<(u64, u64)> {
        match self.addresses(words, 2) {
            Some(e) => Some((e[0], e[1])),
            None => self.addresses(words, 1).map(|t| (target, t[0])),
        }
    }

    fn write_link(&mut self, from: u64, to: u64, kind: &str) -> Result<(), String> {
        let fact = link_fact(kind, from, to)?;
        self.write_here(
            "link",
            json!({"from": from, "to": to, "type": kind, "facts": [fact]}),
        )?;
        self.message = self.linked(from, to, kind);
        Ok(())
    }

    fn linked(&self, from: u64, to: u64, kind: &str) -> String {
        let n = |id| self.outline.number(id);
        format!("linked {} to {} as {kind}", n(from), n(to))
    }

    pub(crate) fn cmd_follow(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let k: usize = words.first().and_then(|w| w.parse().ok()).unwrap_or(1);
        let ends = self.outline.neighbours(target);
        let to = *ends
            .get(k.wrapping_sub(1))
            .ok_or(format!("the target has {} links", ends.len()))?;
        self.follow_to(to);
        Ok(())
    }

    fn follow_to(&mut self, to: u64) {
        self.unfold_to(to);
        self.set_cursor(to);
        self.write_view("follow", json!({"to": to}));
    }

    // Folds and panes.

    pub(crate) fn cmd_fold(&mut self, _: &[&str], target: u64) -> Result<(), String> {
        self.view.folded.insert(target);
        self.write_view("fold", json!({}));
        Ok(())
    }

    pub(crate) fn cmd_unfold(&mut self, _: &[&str], target: u64) -> Result<(), String> {
        self.view.folded.remove(&target);
        self.write_view("fold", json!({}));
        Ok(())
    }

    pub(crate) fn cmd_pane(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let kind = PaneKind::named(words.first().copied())
            .ok_or("pane (shows) outline, links, or suggestions")?;
        self.view.show_other(kind, target);
        self.write_view("pane", json!({}));
        Ok(())
    }

    pub(crate) fn cmd_switch(&mut self, _: &[&str], _: u64) -> Result<(), String> {
        self.view.switch_focus();
        self.write_view("pane", json!({}));
        Ok(())
    }
}

/// Text without the word around a position, and where the word began.
fn cut_word(mut text: Vec<char>, pos: usize) -> (Vec<char>, usize) {
    let (a, b) = word_at(&text, pos);
    text.drain(a..b);
    (text, a)
}

/// The addresses in a link command's words, and its type: the word after
/// "as", or "link".
fn split_as<'a>(words: &'a [&'a str]) -> (&'a [&'a str], &'a str) {
    match words.iter().position(|w| *w == "as") {
        Some(i) => (&words[..i], words.get(i + 1).copied().unwrap_or("link")),
        None => (words, "link"),
    }
}

/// The fact a link states: its type, of the two items' numbers.
fn link_fact(kind: &str, from: u64, to: u64) -> Result<Value, String> {
    let fact = Term::Compound(
        kind.into(),
        vec![Term::Int(from as i64), Term::Int(to as i64)],
    );
    terms::encode(&fact).map_err(|e| e.0)
}
