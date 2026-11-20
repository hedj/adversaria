//! Presentations, after Genera's Dynamic Windows and CLIM: what is drawn
//! on the screen is recorded with the object it stands for, so that the
//! mouse can point at the object itself (docs/design.md, Presentations).
//! The screen's drawing records each presentation where it draws it
//! (crate::ui::Presentations); a gesture on one acts on its object.

use crate::actions::awaited;
use crate::app::{App, Mode};
use crate::plumb::Offer;
use crate::session::Menu;
use crate::ui::Presentations;
use serde_json::json;

/// What a drawn place stands for: an object of the model, of its type.
/// A pane is the outline pane it is drawn in, where a click on it puts
/// the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presentation {
    /// An item: its line in an outline pane, or a link to it.
    Item { pane: usize, id: u64 },
    /// A hint: the command line it stands for.
    Command(String),
    /// A fact, as a view's mark draws it: the entry that states it, and
    /// the item whose text states it.
    Fact { pane: usize, entry: u64, item: u64 },
    /// A constraint, as a mark under an item names it: the entry that
    /// made it, and the item whose text states it, if one does.
    Constraint {
        pane: usize,
        entry: u64,
        item: Option<u64>,
    },
    /// A layer of a view: its entry, and the view's item.
    Layer { entry: u64, view: u64 },
    /// A change a revision of a document proposes: its entry.
    Change(u64),
    /// A suggestion shown, by its number from 1.
    Suggestion(usize),
}

impl Presentation {
    /// Whether it is the user's own text, an item, whose words plumbing
    /// may act on; the rest (a mark, a chart, a change) is the memex's.
    pub fn is_text(&self) -> bool {
        matches!(self, Presentation::Item { .. })
    }

    /// What a click does on it, as one of the two things a click can do:
    /// put the cursor on an item, or run a command line. None: a click
    /// does nothing (a right-click offers what can be done).
    fn clicked(&self) -> Option<Presentation> {
        use Presentation as P;
        match self {
            P::Item { .. } | P::Command(_) => Some(self.clone()),
            P::Fact { pane, item, .. } => Some(P::Item {
                pane: *pane,
                id: *item,
            }),
            P::Constraint { pane, item, .. } => item.map(|id| P::Item { pane: *pane, id }),
            P::Layer { view, .. } => Some(P::Item { pane: 0, id: *view }),
            P::Change(_) => None,
            P::Suggestion(n) => Some(P::Command(format!("accept {n}"))),
        }
    }
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
        if let Some(arg) = self.argument_from(c) {
            self.session.mode.type_in(&arg);
            return;
        }
        if self.session.mode != Mode::Normal {
            return;
        }
        match c.clicked() {
            Some(Presentation::Item { pane, id }) => self.go_to_item(pane, id),
            Some(Presentation::Command(line)) => {
                self.flush();
                self.session.message.clear();
                self.run_simple(&line);
            }
            _ => {}
        }
    }

    /// Put the cursor on an item of an outline pane, as a click on it
    /// does: a view change, as an arrow key is.
    fn go_to_item(&mut self, pane: usize, id: u64) {
        if !self.notebook.outline.items.contains_key(&id) || pane > 1 {
            return;
        }
        self.flush_text();
        self.session.message.clear();
        if self.session.view.focus != pane {
            self.session.view.focus = pane;
            self.write_view("pane", json!({}));
        }
        self.set_cursor(id);
        self.session.view_dirty = true;
    }

    /// Plumb a text: open a menu of the commands its rules offer, at a
    /// cell; or, if none matches, say so.
    pub fn plumb(&mut self, text: &str, at: (u16, u16)) {
        let offers = self.plumbed(text);
        self.offer_menu(text, offers, at);
    }

    /// A right-click at a cell, on the text there (the text selected, or
    /// the word under the pointer): a menu of what the object presented
    /// there offers, then what plumbing offers for the text. With nothing
    /// selected, the cursor first goes to the item presented there, so
    /// that what the menu runs acts on it.
    pub fn right_click(
        &mut self,
        presented: &Presentations,
        at: (u16, u16),
        text: &str,
        selected: bool,
    ) {
        let p = presented.at(at.0, at.1).cloned().filter(|_| !selected);
        if let Some(item @ Presentation::Item { .. }) = &p {
            self.click(item);
        }
        let mut offers = p.as_ref().map(|p| self.translators(p)).unwrap_or_default();
        if p.as_ref().is_none_or(Presentation::is_text) {
            add_new(&mut offers, self.plumbed_words(text));
        }
        let title = p.map_or_else(|| text.to_string(), |p| self.presented_name(&p));
        self.offer_menu(&title, offers, at);
    }

    /// What plumbing offers for a text.
    fn plumbed(&self, text: &str) -> Vec<Offer> {
        let names_item = |t: &str| self.resolve(t).is_some();
        self.session.plumber.offers(text, &names_item)
    }

    /// What plumbing offers for a text that has a word in it, a letter or
    /// a digit: a view's block of a bar has none.
    fn plumbed_words(&self, text: &str) -> Vec<Offer> {
        match text.chars().any(char::is_alphanumeric) {
            true => self.plumbed(text),
            false => vec![],
        }
    }

    /// Open a menu of offers about a text at a cell; or, with none, say
    /// that nothing acts on it.
    fn offer_menu(&mut self, about: &str, offers: Vec<Offer>, at: (u16, u16)) {
        if offers.is_empty() {
            self.session.message = format!("nothing acts on “{}”", about.trim());
            return;
        }
        self.session.message.clear();
        self.session.mode = Mode::Menu(Menu::new(about, offers, at));
    }

    /// A click at a cell: on what is presented there, if anything.
    pub fn click_at(&mut self, presented: &Presentations, (x, y): (u16, u16)) {
        if let Some(p) = presented.at(x, y).cloned() {
            self.click(&p);
        }
    }
}

/// Offers added after others, each command line once.
fn add_new(offers: &mut Vec<Offer>, more: Vec<Offer>) {
    for o in more {
        if !offers.iter().any(|f| f.line == o.line) {
            offers.push(o);
        }
    }
}

impl App {
    /// What a presentation supplies to the Command line, if the line
    /// waits for a slot that takes its type: the argument, and a space,
    /// so the next slot can wait in turn.
    fn argument_from(&self, p: &Presentation) -> Option<String> {
        let slot = awaited(self.session.mode.command_line()?)?;
        Some(format!("{} ", self.supplied(p, slot)?))
    }

    /// Whether a click on a presentation does something now, so the
    /// pointer over it shows it, as Genera showed its mouse-sensitive
    /// presentations: any, with nothing open; a menu's offer, while a menu
    /// is open; one that supplies the slot a Command line waits for.
    pub fn sensitive(&self, p: &Presentation) -> bool {
        match &self.session.mode {
            Mode::Normal => true,
            Mode::Menu(_) => matches!(p, Presentation::Command(_)),
            _ => self.argument_from(p).is_some(),
        }
    }

    /// What each button does over a presentation now, for the mouse
    /// documentation line: in a menu, the offer's words; in Command, what
    /// a click types in; else what a click does and what a right-click
    /// offers.
    pub fn documentation(&self, p: &Presentation) -> String {
        if let Some(arg) = self.argument_from(p) {
            return format!("click: type “{}” into the line", arg.trim_end());
        }
        if let Mode::Menu(m) = &self.session.mode {
            return menu_documentation(m, p);
        }
        let offers = self.translators(p);
        let said = [self.click_documentation(p), offers_documentation(&offers)];
        said.into_iter().flatten().collect::<Vec<_>>().join(" · ")
    }

    /// What a click on a presentation does, if anything.
    fn click_documentation(&self, p: &Presentation) -> Option<String> {
        match p.clicked()? {
            Presentation::Item { id, .. } => {
                Some(format!("click: go to {}", self.notebook.outline.number(id)))
            }
            Presentation::Command(line) => Some(format!("click: run {line}")),
            _ => None,
        }
    }
}

/// What a right-click on a presentation offers: up to three, then how
/// many more.
fn offers_documentation(offers: &[Offer]) -> Option<String> {
    let first: Vec<&str> = offers.iter().take(3).map(|o| o.label.as_str()).collect();
    let more = offers.len().saturating_sub(3);
    let more = if more > 0 {
        format!(", {more} more")
    } else {
        String::new()
    };
    (!first.is_empty()).then(|| format!("right-click: {}{more}", first.join(", ")))
}

/// What a click on a menu's offer does: its words.
fn menu_documentation(m: &Menu, p: &Presentation) -> String {
    let offer = m.offers.iter().find(|o| offered_as(o, p));
    offer.map_or_else(String::new, |o| format!("click: {}", o.label))
}

/// Whether a presentation is an offer, as a menu draws it: its command.
fn offered_as(o: &Offer, p: &Presentation) -> bool {
    matches!(p, Presentation::Command(line) if *line == o.line)
}
