//! Editing by habit (docs/concepts-next.md § How far, move 2): keys that
//! every desktop program gives these jobs. Backspace in an empty item with
//! nothing under it deletes the item. Ctrl+C and Ctrl+X take the text
//! selected, or else the items selected or the cursor's; Ctrl+V puts text
//! at the text cursor, and items after the cursor's item. A cut marks its
//! items until they are pasted, as a spreadsheet's cut does, so an item
//! moved keeps its identity and its entries.
//!
//! What is cut or copied is the session's: no entry records it.

use crate::app::App;
use crate::outline::TOP;
use crate::session::Clip;
use serde_json::json;

impl App {
    /// Backspace in an empty item with nothing under it: delete it, the
    /// cursor at the end of the item shown above, or else below. False for
    /// any other item, in which Backspace erases as before.
    pub(crate) fn erase_empty(&mut self) -> bool {
        let item = self.cursor();
        let outline = &self.notebook.outline;
        let bare = outline.children(item).is_empty();
        if item == TOP || !bare || !self.text_of(item).is_empty() {
            return false;
        }
        let next = self.neighbour(item);
        self.flush_text();
        let deleted = self.write_here("edit", json!({"op": "delete", "item": item}));
        self.session.report(deleted);
        self.set_cursor(next);
        self.session.view_dirty = true;
        true
    }

    /// The item shown above one, or else the one below; the top if it is
    /// the only one.
    fn neighbour(&self, item: u64) -> u64 {
        let rows: Vec<u64> = self.visible().into_iter().map(|(id, _)| id).collect();
        let at = rows.iter().position(|r| *r == item).unwrap_or(0);
        let above = at.checked_sub(1).and_then(|i| rows.get(i));
        above.or(rows.get(at + 1)).copied().unwrap_or(TOP)
    }

    /// Ctrl+C: keep the text selected, or else the items, to paste.
    pub(crate) fn copy_clip(&mut self) {
        let clip = match self.selected_text() {
            Some(text) => Clip::Text(text),
            None => Clip::Items {
                items: self.taken(),
                cut: false,
            },
        };
        self.keep(clip);
    }

    /// Ctrl+X: erase the characters selected and keep them, or else mark
    /// the items to move when they are pasted. Text dragged is no place in
    /// an item's text, so it cannot be cut.
    pub(crate) fn cut_clip(&mut self) {
        if self.dragged_text().is_some() {
            let said = "Ctrl+X cuts text selected with Shift in one item, or items; Ctrl+C copies text dragged";
            self.session.message = said.into();
            return;
        }
        let clip = match self.selected_text() {
            Some(text) => {
                self.erase_selected();
                Clip::Text(text)
            }
            None => Clip::Items {
                items: self.taken(),
                cut: true,
            },
        };
        self.keep(clip);
    }

    /// The items Ctrl+C or Ctrl+X takes: those selected, each top-most one
    /// with its branch, or else the cursor's.
    fn taken(&self) -> Vec<u64> {
        let selected = self.selected_items();
        let outline = &self.notebook.outline;
        let heads = selected
            .iter()
            .filter(|i| !selected.contains(&outline.parent_or_top(**i)));
        let heads: Vec<u64> = heads.copied().collect();
        match heads.is_empty() && self.cursor() != TOP {
            true => vec![self.cursor()],
            false => heads,
        }
    }

    /// Keep what was cut or copied, and say what Ctrl+V will do with it.
    fn keep(&mut self, clip: Clip) {
        self.session.message = clip.said();
        self.session.clipboard = Some(clip).filter(|c| !c.is_empty());
    }

    /// Ctrl+V: the text kept, at the text cursor; or the items, after the
    /// cursor's item, moved if they were cut and copied if not. A cut is
    /// pasted once, since its items have moved.
    pub(crate) fn paste_clip(&mut self) {
        let Some(clip) = self.session.clipboard.clone() else {
            self.session.message = "nothing is cut or copied: Ctrl+X cuts, Ctrl+C copies".into();
            return;
        };
        let pasted = match clip {
            Clip::Text(text) => self.paste_text(&text),
            Clip::Items { items, cut: true } => self.move_items(&items),
            Clip::Items { items, cut: false } => self.copy_items(&items),
        };
        if pasted.is_ok() && self.session.clipboard.as_ref().is_some_and(Clip::is_cut) {
            self.session.clipboard = None;
        }
        self.session.report(pasted);
    }

    /// Text at the text cursor, in the cursor's item or a new one.
    fn paste_text(&mut self, text: &str) -> Result<(), String> {
        if self.cursor() == TOP {
            self.new_item_after(TOP)?;
        }
        let item = self.cursor();
        let shown = self.text_of(item);
        self.session.replace_before(item, shown, 0, text);
        Ok(())
    }

    /// Items cut, moved in order after the cursor's item, each with its
    /// branch; those since deleted are passed over.
    fn move_items(&mut self, items: &[u64]) -> Result<(), String> {
        let mut after = self.paste_place()?;
        if items
            .iter()
            .any(|i| self.notebook.outline.within(after, *i))
        {
            return Err("the cursor is in what was cut: move it out, then Ctrl+V".into());
        }
        for item in self.left(items) {
            self.move_after(item, after)?;
            after = item;
        }
        Ok(())
    }

    /// Items copied, each a new item of its text, in order after the
    /// cursor's item, as `copy` makes them.
    fn copy_items(&mut self, items: &[u64]) -> Result<(), String> {
        let mut after = self.paste_place()?;
        for item in self.left(items) {
            let text = self.notebook.outline.text(item);
            let n = text.chars().count();
            after = self.create_after(after, &text, json!([[0, 0, n]]))?;
        }
        Ok(())
    }

    /// The items of a list still in the outline.
    fn left(&self, items: &[u64]) -> Vec<u64> {
        let outline = &self.notebook.outline;
        items
            .iter()
            .copied()
            .filter(|i| outline.contains(*i))
            .collect()
    }

    /// The item that items are pasted after: the cursor's.
    fn paste_place(&mut self) -> Result<u64, String> {
        self.flush_text();
        let at = self.cursor();
        match at == TOP {
            true => Err("put the cursor on an item to paste after it".into()),
            false => Ok(at),
        }
    }

    /// The items cut and not yet pasted, which the outline draws dim.
    pub fn cut_items(&self) -> &[u64] {
        match &self.session.clipboard {
            Some(Clip::Items { items, cut: true }) => items,
            _ => &[],
        }
    }
}
