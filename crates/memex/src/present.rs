//! Presentations, after Genera's Dynamic Windows and CLIM: what is drawn
//! on the screen is recorded with the object it stands for, so that the
//! mouse can point at the object itself (docs/design.md, Presentations).
//! The screen's drawing records each presentation where it draws it
//! (crate::ui::Presentations); a gesture on one acts on its object.

use crate::app::{App, Mode};
use crate::session::Menu;
use crate::ui::Presentations;
use serde_json::json;

/// What a drawn place stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presentation {
    /// An item's line in an outline pane: the cursor goes there.
    Item { pane: usize, id: u64 },
    /// A hint or a suggestion: the command line it stands for runs.
    Command(String),
}

impl App {
    /// A click, from the mouse. It acts only when no quasimode or question
    /// is open, and does what the keyboard would: moving the cursor is a
    /// view change like an arrow key; a command is run as a bound key runs
    /// it, keeping any confirmation it asks for.
    pub fn click(&mut self, c: &Presentation) {
        if let Mode::Menu(m) = &self.session.mode {
            let answer = m.clicked(c);
            self.session.mode = self.answered(answer);
            return;
        }
        if self.session.mode != Mode::Normal {
            return;
        }
        match c {
            Presentation::Item { pane, id } => {
                if !self.notebook.outline.items.contains_key(id) || *pane > 1 {
                    return;
                }
                self.flush_text();
                self.session.message.clear();
                if self.session.view.focus != *pane {
                    self.session.view.focus = *pane;
                    self.write_view("pane", json!({}));
                }
                self.set_cursor(*id);
                self.session.view_dirty = true;
            }
            Presentation::Command(line) => {
                self.flush();
                self.session.message.clear();
                self.run_simple(line);
            }
        }
    }

    /// Plumb a text: open a menu of the commands its rules offer, at a
    /// cell; or, if none matches, say so.
    pub fn plumb(&mut self, text: &str, at: (u16, u16)) {
        let names_item = |t: &str| self.resolve(t).is_some();
        let offers = self.session.plumber.offers(text, &names_item);
        if offers.is_empty() {
            self.session.message = format!("nothing acts on “{}”", text.trim());
            return;
        }
        self.session.message.clear();
        self.session.mode = Mode::Menu(Menu::new(text, offers, at));
    }

    /// A click at a cell: on what is presented there, if anything.
    pub fn click_at(&mut self, presented: &Presentations, (x, y): (u16, u16)) {
        if let Some(p) = presented.at(x, y).cloned() {
            self.click(&p);
        }
    }

    /// A click at a cell, if an item is presented there: the cursor goes
    /// to it, as a right-click does before it plumbs.
    pub fn click_item_at(&mut self, presented: &Presentations, (x, y): (u16, u16)) {
        if let Some(p @ Presentation::Item { .. }) = presented.at(x, y).cloned() {
            self.click(&p);
        }
    }
}
