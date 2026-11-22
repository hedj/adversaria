//! The selection: what a command takes for its first noun
//! (docs/concepts-next.md § The grammar). Shift with an arrow, Home or End
//! selects from where the cursor was to where it goes. A plain move ends
//! the selection; a command takes it and ends it.
//!
//! It is the session's, not the view's: no entry records it.

use crate::app::{App, Key, Special};
use crate::session::Selection;
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
        let begun = self.session.selection.take().unwrap_or(self.place());
        self.move_by(key);
        self.session.selection = Some(begun);
        self.session.view_dirty = true;
        true
    }

    /// Where the cursor is: its item, and the text cursor in it.
    fn place(&self) -> Selection {
        Selection {
            item: self.cursor(),
            pos: self.session.view.pos,
        }
    }

    /// The characters selected within the cursor's item, if the selection
    /// begins in it; none if it holds no character.
    pub fn selected_range(&self) -> Option<Range<usize>> {
        let s = self.selection_here()?;
        let len = self.text_of(s.item).len();
        between(s.pos, self.place().pos, len)
    }

    /// The items selected, as shown, from the one the selection begins in
    /// to the cursor's; none if it begins in the cursor's item, or in one
    /// not shown.
    pub fn selected_items(&self) -> Vec<u64> {
        let Some(s) = self.session.selection else {
            return vec![];
        };
        let rows: Vec<u64> = self.visible().into_iter().map(|(id, _)| id).collect();
        let at = |id| rows.iter().position(|r| *r == id);
        match (at(s.item), at(self.cursor())) {
            (Some(a), Some(b)) if a != b => rows[a.min(b)..=a.max(b)].to_vec(),
            _ => vec![],
        }
    }

    /// The selection, if it begins in the cursor's item.
    fn selection_here(&self) -> Option<Selection> {
        self.session.selection.filter(|s| s.item == self.cursor())
    }

    /// A key that acts on a selection: Backspace or Delete deletes the
    /// characters selected, and a character typed replaces them. With
    /// items selected, Backspace and Delete refuse, so a key that deletes
    /// a character does not delete a branch. True if the key is done.
    pub(crate) fn edit_selected(&mut self, key: Key) -> bool {
        match key {
            Key::Backspace | Key::Delete if self.erase_selected() => true,
            Key::Backspace | Key::Delete if !self.selected_items().is_empty() => {
                let said = "Backspace and Delete delete text selected in one item";
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

    /// Delete the characters selected within an item, the text cursor
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
