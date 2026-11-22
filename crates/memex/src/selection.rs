//! The selection: what a command takes for its first noun
//! (docs/concepts-next.md § The grammar). Shift with an arrow, Home or End
//! selects from where the cursor was to where it goes; the other Leap key,
//! pressed while one is held, selects the items the Leap went over; a drag
//! of the mouse selects the text it covers. A plain move ends the
//! selection; a command takes it and ends it.
//!
//! It is the session's, not the view's: no entry records it.

use crate::app::{Active, App, Key, Mode, Special};
use crate::session::Selection;
use serde_json::json;
use std::ops::Range;

/// The arrow, Home or End that a key with Shift held is.
pub(crate) fn unshifted(s: Special) -> Option<Key> {
    Some(match s {
        Special::ShiftUp => Key::Up,
        Special::ShiftDown => Key::Down,
        Special::ShiftLeft => Key::Left,
        Special::ShiftRight => Key::Right,
        Special::ShiftHome => Key::Home,
        Special::ShiftEnd => Key::End,
        _ => return None,
    })
}

impl App {
    /// An arrow, Home or End with Shift: move the cursor as the key does
    /// without Shift, selecting from where it was, or from where the
    /// selection began. False for any other key.
    pub(crate) fn select_by(&mut self, s: Special) -> bool {
        let Some(key) = unshifted(s) else {
            return false;
        };
        let (item, pos) = self.begins().unwrap_or(self.place());
        self.move_by(key);
        self.session.selection = Some(Selection::From { item, pos });
        true
    }

    /// Where the cursor is: its item, and the text cursor in it.
    fn place(&self) -> (u64, usize) {
        (self.cursor(), self.session.view.pos)
    }

    /// The other Leap key, pressed while one is held: leap, and select
    /// from the item where the Leap began to the item it reached. So a
    /// span is selected by leaping to one end, then to the other.
    pub(crate) fn leap_select(&mut self, a: Active) -> Mode {
        let from = a.leap_from();
        let mode = self.finish(a);
        if let Some(item) = from.filter(|f| *f != self.cursor()) {
            self.session.selection = Some(Selection::From { item, pos: 0 });
        }
        mode
    }

    /// Text a drag of the mouse selected. With none, a drag's selection
    /// ends, as a click ends it; one made with keys stays.
    pub fn select_text(&mut self, text: &str) {
        let drawn = matches!(self.session.selection, Some(Selection::Drawn(_)));
        match text.trim() {
            "" if drawn => self.session.selection = None,
            "" => {}
            t => self.session.selection = Some(Selection::Drawn(t.to_string())),
        }
    }

    /// The text a drag of the mouse selected, if it is the selection.
    pub fn dragged_text(&self) -> Option<&str> {
        match &self.session.selection {
            Some(Selection::Drawn(text)) => Some(text),
            _ => None,
        }
    }

    /// Where a selection made with keys or a Leap begins: an item, and a
    /// place in its text.
    fn begins(&self) -> Option<(u64, usize)> {
        match self.session.selection {
            Some(Selection::From { item, pos }) => Some((item, pos)),
            _ => None,
        }
    }

    /// The characters selected within the cursor's item, if the selection
    /// begins in it; none if it holds no character.
    pub fn selected_range(&self) -> Option<Range<usize>> {
        let ((item, pos), (here, at)) = (self.begins()?, self.place());
        let len = self.text_of(item).len();
        between(pos, at, len).filter(|_| item == here)
    }

    /// The items selected, as shown, from the one the selection begins in
    /// to the cursor's; none if it begins in the cursor's item, or in one
    /// not shown.
    pub fn selected_items(&self) -> Vec<u64> {
        let Some((item, _)) = self.begins() else {
            return vec![];
        };
        let rows: Vec<u64> = self.visible().into_iter().map(|(id, _)| id).collect();
        let at = |id| rows.iter().position(|r| *r == id);
        match (at(item), at(self.cursor())) {
            (Some(a), Some(b)) if a != b => rows[a.min(b)..=a.max(b)].to_vec(),
            _ => vec![],
        }
    }

    /// A key that acts on a selection: Backspace or Delete erases the
    /// characters selected, and a character typed replaces them. With
    /// items selected, or text dragged, Backspace and Delete refuse, so a
    /// key that erases a character neither deletes a branch nor erases
    /// beside what is shown selected. True if the key is done.
    pub(crate) fn edit_selected(&mut self, key: Key) -> bool {
        let other = self.dragged_text().is_some() || !self.selected_items().is_empty();
        match key {
            Key::Backspace | Key::Delete if self.erase_selected() => true,
            Key::Backspace | Key::Delete if other => {
                let said = "Backspace and Delete erase text selected with Shift in one item; the command delete deletes items selected";
                self.session.message = said.into();
                true
            }
            Key::Char(_) => {
                self.erase_selected();
                false
            }
            _ => false,
        }
    }

    /// The text selected: text dragged, or the characters selected in one
    /// item. What a command whose first noun is text takes, typed alone.
    pub fn selected_text(&self) -> Option<String> {
        if let Some(text) = self.dragged_text() {
            return Some(text.into());
        }
        let range = self.selected_range()?;
        Some(self.text_of(self.cursor())[range].iter().collect())
    }

    /// Delete what is selected, as `delete` alone does: the items, each
    /// with its branch, or the characters selected in one item. None if
    /// nothing is selected; a refusal for text dragged, which is no place
    /// in an item's text.
    pub(crate) fn delete_selected(&mut self) -> Option<Result<String, String>> {
        let items = self.selected_items();
        if !items.is_empty() {
            return Some(self.delete_items(&items));
        }
        if self.erase_selected() {
            return Some(Ok(String::new()));
        }
        let refused = "delete takes items, or text selected with Shift in one item";
        self.dragged_text().map(|_| Err(refused.into()))
    }

    /// Delete items shown in a run, each with its branch. One whose parent
    /// is among them goes with its parent's branch: in a run of rows, an
    /// item's selected ancestor has its parent selected too.
    fn delete_items(&mut self, items: &[u64]) -> Result<String, String> {
        let outline = &self.notebook.outline;
        let heads = items
            .iter()
            .filter(|i| !items.contains(&outline.parent_or_top(**i)));
        let heads: Vec<u64> = heads.copied().collect();
        for item in &heads {
            self.write_here("edit", json!({"op": "delete", "item": item}))?;
        }
        let n = heads.len();
        Ok(format!(
            "deleted {n} items with their branches; undo takes back one at a time"
        ))
    }

    /// Erase the characters selected within an item, the text cursor
    /// where they were. False if none are selected.
    pub(crate) fn erase_selected(&mut self) -> bool {
        let Some(range) = self.selected_range() else {
            return false;
        };
        let item = self.cursor();
        let mut text = self.text_of(item);
        text.drain(range.clone());
        self.session.view.pos = range.start;
        self.session.hold_text(item, text);
        self.session.selection = None;
        true
    }
}

/// The characters between two places in a text of a length; none if they
/// are one place.
fn between(a: usize, b: usize, len: usize) -> Option<Range<usize>> {
    let (a, b) = (a.min(len), b.min(len));
    Some(a.min(b)..a.max(b)).filter(|r| !r.is_empty())
}
