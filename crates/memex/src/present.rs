//! Presentations, after Genera's Dynamic Windows and CLIM: what is drawn
//! on the screen is recorded with the object it stands for, so that the
//! mouse can point at the object itself (docs/design.md, Presentations).
//! The screen's drawing records each presentation where it draws it
//! (crate::ui::Presentations); a gesture on one acts on its object.

use crate::actions::awaited;
use crate::app::{App, Mode};
use crate::plumb::Offer;
use crate::session::Menu;
use crate::session::ViewChange;
use crate::ui::Presentations;
use format::bodies::View;

/// What a drawn place stands for: an object of the model, of its type.
/// A pane is the outline pane it is drawn in, where a click on it puts
/// the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presentation {
    /// An item: its line in an outline pane.
    Item { pane: usize, id: u64 },
    /// A link's line in the links pane: the item at its other end, which a
    /// click jumps to, as follow does.
    Link { to: u64 },
    /// A hint: the command line it stands for.
    Command(String),
    /// A fact, as a view's mark draws it: the entry that states it, and
    /// the item whose text states it.
    Fact { pane: usize, entry: u64, item: u64 },
    /// A constraint, as a mark under an item names it: the entry that
    /// made it, the item whose text states it, if one does, and the item
    /// the mark is under, if a mark presents it.
    Constraint {
        pane: usize,
        entry: u64,
        item: Option<u64>,
        under: Option<u64>,
    },
    /// A layer of a view: its entry, and the view's item.
    Layer { entry: u64, view: u64 },
    /// A change a revision of a document proposes: its entry.
    Change(u64),
    /// A suggestion shown, by its number from 1.
    Suggestion(usize),
    /// An entry of the history, by its number: a right-click retracts a
    /// change, or goes back to where a jump went.
    Entry(u64),
    /// The notebook itself, as an outline pane's title presents it: a
    /// click puts the cursor above the first item, where the inspector
    /// lists what waits to be decided.
    Notebook { pane: usize },
}

impl Presentation {
    /// Where a right-click on it puts the cursor first, so the item's
    /// offers act on it: on an item, or the item a mark is under.
    fn right_clicked(&self) -> Option<Presentation> {
        use Presentation as P;
        match self {
            P::Item { .. } => Some(self.clone()),
            P::Link { to } => Some(P::Item { pane: 0, id: *to }),
            P::Constraint {
                pane,
                under: Some(id),
                ..
            } => Some(P::Item {
                pane: *pane,
                id: *id,
            }),
            _ => None,
        }
    }

    /// Whether it is the user's own text, an item, whose words plumbing
    /// may act on; the rest (a mark, a chart, a change) is the memex's.
    pub fn is_text(&self) -> bool {
        matches!(self, Presentation::Item { .. } | Presentation::Link { .. })
    }

    /// What a click does on it, as one of the two things a click can do:
    /// put the cursor on an item, or run a command line. None: a click
    /// does nothing (a right-click offers what can be done).
    fn clicked(&self) -> Option<Presentation> {
        use Presentation as P;
        match self {
            P::Item { .. } | P::Command(_) | P::Link { .. } => Some(self.clone()),
            P::Fact { pane, item, .. } => Some(P::Item {
                pane: *pane,
                id: *item,
            }),
            P::Constraint { pane, item, .. } => item.map(|id| P::Item { pane: *pane, id }),
            P::Layer { view, .. } => Some(P::Item { pane: 0, id: *view }),
            P::Change(_) | P::Entry(_) => None,
            P::Suggestion(n) => Some(P::Command(format!("adopt suggestion {n}"))),
            P::Notebook { pane } => Some(P::Item {
                pane: *pane,
                id: crate::outline::TOP,
            }),
        }
    }
}

impl App {
    /// A command a mouse button runs, as a browser's back and forward
    /// buttons do: only when no quasimode, menu or question is open.
    pub fn mouse_command(&mut self, line: &str) {
        if matches!(self.session.mode, Mode::Normal) {
            self.session.mode = self.run_command(line);
        }
    }

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
            Some(Presentation::Link { to }) => self.follow_link(to),
            Some(Presentation::Command(line)) => {
                self.flush();
                self.session.message.clear();
                self.run_simple(&line);
            }
            _ => {}
        }
    }

    /// Jump along a link to the item at its other end, as follow does, so
    /// back returns.
    fn follow_link(&mut self, to: u64) {
        if !self.notebook.outline.items.contains_key(&to) {
            return;
        }
        self.flush_text();
        self.session.message.clear();
        self.session.selection = None;
        self.jump(to, View::Follow);
    }

    /// Put the cursor on an item of an outline pane, as a click on it
    /// does: a view change, as an arrow key is.
    fn go_to_item(&mut self, pane: usize, id: u64) {
        let there = id == crate::outline::TOP || self.notebook.outline.items.contains_key(&id);
        if !there || pane > 1 {
            return;
        }
        self.flush_text();
        self.session.message.clear();
        self.session.selection = None;
        if self.session.view.focus != pane {
            self.session.view.focus = pane;
            self.write_shown(View::Pane);
        }
        self.set_cursor(id);
        self.session.view_changed(ViewChange::Moved);
    }

    /// Plumb a text: open a menu of the commands its rules offer, at a
    /// cell; or, if none matches, say so.
    pub fn plumb(&mut self, text: &str, at: (u16, u16)) {
        let offers = self.plumbed(text);
        self.offer_menu(text, offers, Some(at));
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
        if let Some(item) = p.as_ref().and_then(Presentation::right_clicked) {
            self.click(&item);
        }
        let offers = self.menu_offers(p.as_ref(), text);
        let title = p.map_or_else(|| text.to_string(), |p| self.presented_name(&p));
        self.offer_menu(&title, offers, Some(at));
    }

    /// The Menu key, the keyboard's right-click: the menu a right-click
    /// on the cursor's item opens, at that item, with plumbing's offers
    /// for the text selected or the word at the text cursor; the mode it
    /// leaves.
    pub(crate) fn menu_at_cursor(&mut self) -> Mode {
        let id = self.cursor();
        if id == crate::outline::TOP {
            self.session.message = "there is no item to act on".into();
            return Mode::Normal;
        }
        let p = Presentation::Item {
            pane: self.session.view.focus,
            id,
        };
        let offers = self.menu_offers(Some(&p), &self.word_here());
        self.offer_menu(&self.presented_name(&p), offers, None);
        std::mem::replace(&mut self.session.mode, Mode::Normal)
    }

    /// The text selected, or else the word at the text cursor.
    fn word_here(&self) -> String {
        let text = self.text_of(self.cursor());
        let (a, b) = crate::app::word_at(&text, self.session.view.pos);
        let word = || text[a..b].iter().collect::<String>().trim().to_string();
        self.selected_text().unwrap_or_else(word)
    }

    /// What a right-click's menu offers, in order: what the object
    /// presented offers; for a text, what plumbing offers; and for an
    /// item's text, the cursor on it, the verbs that apply to the item
    /// (crate::verbs). Each command line is offered once.
    fn menu_offers(&self, p: Option<&Presentation>, text: &str) -> Vec<Offer> {
        let mut offers = p.map(|p| self.translators(p)).unwrap_or_default();
        if p.is_none_or(Presentation::is_text) {
            add_new(&mut offers, self.plumbed_words(text));
        }
        if p.is_some_and(Presentation::is_text) {
            add_new(&mut offers, self.verb_offers(self.cursor()));
        }
        offers
    }

    /// What plumbing offers for a text. For a rule's `item` test, the
    /// text names an item only if it is not the cursor's: going to the
    /// item the cursor is on, linking it to itself or showing it in its
    /// own place does nothing.
    fn plumbed(&self, text: &str) -> Vec<Offer> {
        let names_item = |t: &str| self.notebook.resolve(t).is_some_and(|i| i != self.cursor());
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
    fn offer_menu(&mut self, about: &str, offers: Vec<Offer>, at: Option<(u16, u16)>) {
        if offers.is_empty() {
            self.session.message = format!("nothing acts on “{}”", about.trim());
            return;
        }
        let offers = offers.into_iter().map(|o| self.session.keys.labelled(o));
        let offers = offers.collect();
        self.session.message.clear();
        self.session.mode = Mode::Menu(Menu::new(about, offers, at));
    }

    /// The item the inspector shows: the claim it interrogates, or the
    /// one under the cursor.
    pub fn inspector_subject(&self) -> u64 {
        let selected = self.session.view.selected();
        self.inspected_claim().unwrap_or(selected)
    }

    /// How far the inspector is scrolled, for the item it shows now.
    pub fn inspector_rows(&self) -> u16 {
        self.session.inspector.rows_for(self.inspector_subject())
    }

    /// Note what the frame just drawn showed of the inspector, so that
    /// scrolling stops at its end and pages by what it shows.
    pub fn fit_inspector(&mut self, drawn: &Presentations) {
        let Some(fit) = drawn.1 else { return };
        let scroll = &mut self.session.inspector;
        (scroll.page, scroll.max) = (fit.page, fit.max);
    }

    /// Scroll the inspector by so many rows, down if more than none, up if
    /// fewer, within what the screen last drew of it.
    pub fn scroll_inspector(&mut self, by: i32) {
        let of = self.inspector_subject();
        let scroll = &mut self.session.inspector;
        let from = i32::from(scroll.rows_for(of));
        scroll.rows = (from + by).clamp(0, i32::from(scroll.max)) as u16;
        scroll.of = of;
    }

    /// The wheel turned by so many rows, down if more than none, over a
    /// cell: the inspector scrolls, if the cell is in it.
    pub fn wheel(&mut self, drawn: &Presentations, cell: (u16, u16), rows: i32) {
        if drawn.1.is_some_and(|fit| fit.holds(cell)) {
            self.scroll_inspector(rows);
        }
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
        let verbs = p.is_text();
        let said = [
            self.click_documentation(p),
            offers_documentation(&offers, verbs),
        ];
        said.into_iter().flatten().collect::<Vec<_>>().join(" · ")
    }

    /// What a click on a presentation does, if anything.
    fn click_documentation(&self, p: &Presentation) -> Option<String> {
        match p.clicked()? {
            Presentation::Item { id, .. } if id == crate::outline::TOP => {
                Some("click: go to what waits for you to decide".into())
            }
            Presentation::Item { id, .. } => {
                Some(format!("click: go to {}", self.notebook.outline.number(id)))
            }
            Presentation::Link { to } => Some(format!(
                "click: follow to {}",
                self.notebook.outline.number(to)
            )),
            Presentation::Command(line) => Some(format!("click: run {line}")),
            _ => None,
        }
    }
}

/// What a right-click on a presentation offers, as the choices of a
/// menu: up to three, then how many more, apart by semicolons, since an
/// offer's words may hold a comma ("the rule, 1.3"); and, on an item's
/// text, the verbs, which are named, not counted, since finding which
/// apply reads the journal, too slow for each move of the pointer.
fn offers_documentation(offers: &[Offer], verbs: bool) -> Option<String> {
    let mut first: Vec<&str> = offers.iter().take(3).map(|o| o.label.as_str()).collect();
    let more = offers.len().saturating_sub(3);
    let more = (more > 0).then(|| format!("{more} more"));
    first.extend(more.as_deref());
    first.extend(verbs.then_some("the verbs that apply to it"));
    (!first.is_empty()).then(|| format!("right-click for: {}", first.join("; ")))
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
