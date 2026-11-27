//! Commands that edit the outline's items and links, and move about it
//! (docs/guide.md, § 2.4, Editing the outline).

use crate::app::{App, PaneKind, word_at};
use crate::notebook::Notebook;
use crate::outline::{Outline, page_key, reads_as_number};
use crate::session::ViewChange;
use format::bodies::{
    self, Common, Create, Delete, Edit, Jump, Leap, Link, Mark, Move, Name, View,
};
use format::terms::{self, Term};
use serde_json::Value;

impl App {
    pub(crate) fn cmd_new(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        self.new_item_after(target)?;
        Ok(String::new())
    }

    pub(crate) fn cmd_name(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let n = words.join(" ");
        if n.is_empty() {
            return Err("name (as) a word".into());
        }
        if reads_as_number(&n) || n.starts_with('#') {
            return Err(format!(
                "name not kept: “{n}” reads as an item's or an entry's number, so it would address no item"
            ));
        }
        let name = Name {
            name: n.clone(),
            item: target,
            common: Common::default(),
        };
        let named = self.write_here(name)?;
        self.on_its_own(|app| app.relink_name(&n, named.seq))?;
        Ok(String::new())
    }

    /// Delete what is selected, the word at the cursor, or the target's
    /// branch.
    pub(crate) fn cmd_delete(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        if words.is_empty()
            && let Some(done) = self.delete_selected()
        {
            return done;
        }
        match words {
            [] => {}
            ["word"] => return self.delete_word(target).map(|_| String::new()),
            _ => return Err("delete, or delete word".into()),
        }
        self.write_here(delete(target))?;
        Ok(String::new())
    }

    fn delete_word(&mut self, item: u64) -> Result<(), String> {
        let (text, at) = self.word_cut(item);
        self.set_pos(at);
        self.write_text(item, &text)
    }

    /// An item's text without the word at the cursor, and where the word was.
    fn word_cut(&self, item: u64) -> (Vec<char>, usize) {
        cut_word(self.text_of(item), self.session.view.pos)
    }

    fn set_pos(&mut self, pos: usize) {
        self.session.view.pos = pos;
        self.session.view_changed(ViewChange::Written);
    }

    // Moving items.

    pub(crate) fn cmd_indent(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        let under = self
            .notebook
            .outline
            .previous_sibling(target)
            .ok_or("the first item of a list cannot be indented")?;
        self.move_to_end(target, under)?;
        self.open_branch(under);
        Ok(String::new())
    }

    pub(crate) fn cmd_outdent(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        let p = self
            .notebook
            .outline
            .parent_item(target)
            .ok_or("a top-level item cannot be outdented")?;
        self.move_item(target, self.notebook.outline.parent_or_top(p), Some(p))?;
        Ok(String::new())
    }

    pub(crate) fn cmd_move(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let to = self
            .address(words)
            .ok_or("move (after) an item: a number, a name, or the start of its text")?;
        if self.notebook.outline.within(to, target) {
            return Err("an item cannot move into its own branch".into());
        }
        self.move_after(target, to)?;
        Ok(String::new())
    }

    /// Move an item's branch to follow another item.
    pub(crate) fn move_after(&mut self, item: u64, to: u64) -> Result<(), String> {
        self.move_item(item, self.notebook.outline.parent_or_top(to), Some(to))
    }

    /// Move an item's branch to the end of another item's children.
    fn move_to_end(&mut self, item: u64, under: u64) -> Result<(), String> {
        self.move_item(item, under, self.notebook.outline.last_child(under))
    }

    fn move_item(&mut self, item: u64, under: u64, after: Option<u64>) -> Result<(), String> {
        let common = Common::default();
        let body = Move {
            item,
            under,
            after,
            common,
        };
        self.write_here(Edit::Move(body))?;
        Ok(())
    }

    /// Show an item's children if they were folded away.
    fn open_branch(&mut self, id: u64) {
        if self.session.view.folded.remove(&id) {
            self.session.view_changed(ViewChange::Written);
        }
    }

    // Copying and transcluding.

    pub(crate) fn cmd_copy(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let to = self.address(words).ok_or("copy (after) an item")?;
        let text = self.notebook.outline.text(target);
        let n = text.chars().count();
        self.create_after(to, &text, vec![[0, 0, n as u64]])?;
        Ok(String::new())
    }

    pub(crate) fn cmd_transclude(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let from = self.address(words).ok_or("transclude (from) an item")?;
        let seq = self.transclusion_after(from, target)?;
        self.go_to(seq);
        Ok(String::new())
    }

    /// A new item after one, showing another's text in place.
    fn transclusion_after(&mut self, from: u64, after: u64) -> Result<u64, String> {
        let spans = self.notebook.outline.spans_written(from);
        self.create_after(after, "", spans)
    }

    /// A new item after one, with text and spans; its sequence number.
    pub(crate) fn create_after(
        &mut self,
        after: u64,
        add: &str,
        spans: Vec<bodies::Span>,
    ) -> Result<u64, String> {
        let under = self.notebook.outline.parent_or_top(after);
        let body = Create {
            under,
            after: Some(after),
            add: add.to_string(),
            spans,
            style: None,
            import: None,
            common: Common::default(),
        };
        Ok(self.write_here(Edit::Create(body))?.seq)
    }

    /// Go to the item words address, else to the next item that holds
    /// them, as Leap does; a plumbed text arrives here (crate::plumb).
    /// Go anywhere (docs/concepts.md, decision 5): along the trail, along
    /// a link, by the outline's structure, or to an address or a text.
    pub(crate) fn cmd_go(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        match words {
            ["back"] => self.cmd_back(&[], target),
            ["forward"] => self.cmd_forward(&[], target),
            ["link", n @ ..] => self.go_from(target, |a| a.cmd_follow(n, target)),
            [way] => match super::structure::move_named(way) {
                Some(step) => self.go_from(target, |a| step(a, &[], target)),
                None => self.go_to_text(words, target),
            },
            _ => self.go_to_text(words, target),
        }
    }

    /// A move from the target, which there must be.
    fn go_from(
        &mut self,
        target: u64,
        go: impl FnOnce(&mut Self) -> Result<String, String>,
    ) -> Result<String, String> {
        if target == crate::outline::TOP {
            return Err("there is no item to go from".into());
        }
        go(self)
    }

    /// Go to the item an address names; else show the page it names, if
    /// two or more items link to it (docs/concepts-seven.md, change 4);
    /// else go to the next item that holds the text, as a leap does.
    fn go_to_text(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let text = words.join(" ");
        if let Some(id) = self.notebook.resolve(&text) {
            self.jump(id, View::Go);
            return Ok(String::new());
        }
        let key = page_key(text.strip_prefix('#').unwrap_or(&text));
        if self.notebook.outline.page_linkers(&key).len() > 1 {
            return self.cmd_find(&[&format!("#{key}")], target);
        }
        let found = self.leap_matches(&text, true).first().copied();
        let id = found.ok_or(format!("no item is or holds “{text}”"))?;
        self.leap_to(id, &text, true);
        Ok(String::new())
    }

    /// Jump the cursor to an item, unfolding what hides it, and write the
    /// view with where it came from and where it went, so back returns.
    pub(crate) fn jump(&mut self, to: u64, op: fn(Jump) -> View) {
        let from = self.moved_to(to);
        self.write_view(|state| op(Jump::of(from, to, state)));
    }

    /// Leap to an item a pattern found, as a jump.
    pub(crate) fn leap_to(&mut self, to: u64, pattern: &str, forward: bool) {
        let from = self.moved_to(to);
        self.write_view(|state| {
            View::Leap(Leap {
                pattern: Some(pattern.to_string()),
                forward: Some(forward),
                found: Some(to),
                from: Some(from),
                to: Some(to),
                state,
                common: Common::default(),
            })
        });
    }

    /// Move the cursor to an item, unfolding what hides it; where it was.
    fn moved_to(&mut self, to: u64) -> u64 {
        let from = self.cursor();
        self.unfold_to(to);
        self.set_cursor(to);
        from
    }

    /// The item the last jump came from, if it is left and is not the
    /// target: by its number, the default of `move` and `link`.
    pub(crate) fn jumped_from(&self, target: u64) -> Option<String> {
        let from = self.notebook.last_present(&self.session.trail.back);
        let from = from.filter(|f| *f != target)?;
        Some(self.notebook.outline.number(from))
    }

    /// The target, by its number: where `copy` puts its copy by default,
    /// after itself.
    pub(crate) fn itself(&self, target: u64) -> Option<String> {
        Some(self.notebook.outline.number(target))
    }

    /// `back`, if a jump came from an item that is left: where `go` goes
    /// by default.
    pub(crate) fn back_to(&self, _: u64) -> Option<String> {
        let from = self.notebook.last_present(&self.session.trail.back);
        from.map(|_| "back".into())
    }

    /// Go back to where the last jump came from, passing over items gone.
    pub(crate) fn cmd_back(&mut self, _: &[&str], _: u64) -> Result<String, String> {
        let trail = &self.session.trail.back;
        let to = self
            .notebook
            .last_present(trail)
            .ok_or("nothing to go back to")?;
        self.jump(to, View::Back);
        Ok(String::new())
    }

    /// Go forward again to where back came from.
    pub(crate) fn cmd_forward(&mut self, _: &[&str], _: u64) -> Result<String, String> {
        let trail = &self.session.trail.forward;
        let to = self
            .notebook
            .last_present(trail)
            .ok_or("nothing to go forward to")?;
        self.jump(to, View::Forward);
        Ok(String::new())
    }

    pub(crate) fn go_to(&mut self, id: u64) {
        self.set_cursor(id);
        self.write_shown(View::Move);
    }

    // Links.

    pub(crate) fn cmd_link(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let (addr, kind) = super::dispatch::split_kind(words);
        let kind = kind
            .filter(|k| !k.is_empty())
            .unwrap_or_else(|| "link".into());
        if kind.contains(' ') {
            return Err(format!("a link's type is one word, not “{kind}”"));
        }
        let (from, to) = self
            .link_ends(addr, target)
            .ok_or("link (from) an item (to) an item, (as) a type")?;
        self.write_link(from, to, &kind)
    }

    /// The two ends words address, or the target and the one they address.
    fn link_ends(&self, words: &[&str], target: u64) -> Option<(u64, u64)> {
        match self.two_addresses(words) {
            Some((_, from, to)) => Some((from, to)),
            None => self.address(words).map(|to| (target, to)),
        }
    }

    /// A link for a [[link]] in an item's text: kept by the text, so taken
    /// back when the text no longer has it.
    pub(crate) fn write_inline_link(&mut self, from: u64, to: u64) -> Result<(), String> {
        let mut link = link(
            from,
            to,
            "link",
            Common::adding(vec![link_fact("link", from, to)?]),
        );
        link.inline = Some(true);
        self.write_here(link).map(|_| ())
    }

    /// A link from a part of a split item back to the item it was split
    /// from.
    pub(crate) fn write_split_link(&mut self, part: u64, item: u64) -> Result<(), String> {
        self.write_link(part, item, "split_from").map(|_| ())
    }

    /// A link a suggestion proposed, recording the suggestion it adopts.
    pub(crate) fn write_suggested_link(
        &mut self,
        from: u64,
        to: u64,
        mark: Mark,
    ) -> Result<(), String> {
        let common = Common::adopting(mark, vec![link_fact("link", from, to)?]);
        self.write_here(link(from, to, "link", common)).map(|_| ())
    }

    fn write_link(&mut self, from: u64, to: u64, kind: &str) -> Result<String, String> {
        let common = Common::adding(vec![link_fact(kind, from, to)?]);
        self.write_here(link(from, to, kind, common))?;
        Ok(linked(&self.notebook.outline, from, to, kind))
    }

    pub(crate) fn cmd_follow(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let k: usize = match words {
            [] => 1,
            [n] => n
                .parse()
                .map_err(|_| "follow, or follow and a link's number")?,
            _ => return Err("follow, or follow and a link's number".into()),
        };
        let ends = self.notebook.outline.neighbours(target);
        let to = *ends
            .get(k.wrapping_sub(1))
            .ok_or(format!("the target has {} links", ends.len()))?;
        self.follow_to(to);
        Ok(String::new())
    }

    fn follow_to(&mut self, to: u64) {
        self.jump(to, View::Follow);
    }

    // Folds and panes.

    /// Hide the target's branch, or show it if it is hidden: one toggle,
    /// as outliners have.
    pub(crate) fn cmd_fold(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        let folded = &mut self.session.view.folded;
        if !folded.remove(&target) {
            folded.insert(target);
        }
        self.write_shown(View::Fold);
        Ok(String::new())
    }

    pub(crate) fn cmd_pane(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let kind = PaneKind::named(words.first().copied())
            .ok_or("pane (shows) outline, links, suggestions, or matches")?;
        self.session.view.show_other(kind, target);
        self.write_shown(View::Pane);
        Ok(String::new())
    }

    pub(crate) fn cmd_switch(&mut self, _: &[&str], _: u64) -> Result<String, String> {
        if let Err(kind) = self.session.view.switch_focus() {
            return Err(format!(
                "only an outline takes keys; the other pane shows {}",
                kind.name()
            ));
        }
        self.write_shown(View::Pane);
        Ok(String::new())
    }
}

/// The edit that deletes an item, with its branch.
pub(crate) fn delete(item: u64) -> Edit {
    let common = Common::default();
    Edit::Delete(Delete { item, common })
}

/// A link of a type from one item to another.
pub(crate) fn link(from: u64, to: u64, kind: &str, common: Common) -> Link {
    Link {
        from,
        to,
        link_type: Some(kind.to_string()),
        inline: None,
        common,
    }
}

/// Text without the word around a position, and where the word began.
fn cut_word(mut text: Vec<char>, pos: usize) -> (Vec<char>, usize) {
    let (a, b) = word_at(&text, pos);
    text.drain(a..b);
    (text, a)
}

/// The fact a link states: its type, of the two items' numbers.
fn link_fact(kind: &str, from: u64, to: u64) -> Result<Value, String> {
    let fact = Term::Compound(
        kind.into(),
        vec![Term::Int(from as i64), Term::Int(to as i64)],
    );
    terms::encode(&fact).map_err(|e| e.0)
}

/// What a link command says it did.
fn linked(outline: &Outline, from: u64, to: u64, kind: &str) -> String {
    let n = |id| outline.number(id);
    format!("linked {} to {} as {kind}", n(from), n(to))
}

impl Notebook {
    /// The last item of a list still in the outline.
    fn last_present(&self, items: &[u64]) -> Option<u64> {
        let outline = &self.outline;
        items
            .iter()
            .rev()
            .copied()
            .find(|i| outline.items.contains_key(i))
    }
}
