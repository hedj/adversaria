//! Keys and clicks from the window to the memex. A key comes from winit
//! as its place on the keyboard (physical) and what the layout makes of
//! it (logical), with its text. A modifier is named by its place, as the
//! configuration names quasimode keys (Right Alt, Left Control, ...); a
//! character is the key's text, or, where the text is a control character
//! (while Ctrl is held, as for the Command quasimode), the layout's
//! character for the key, which Ctrl does not change.

use crate::app::{Event, Key, Press, Quasi, Special};
use crate::config::{Config, QuasiKey};
use crate::present::Presentation;
use crate::ui::Presentations;
use ratatui::buffer::Buffer;
use ratatui::style::Modifier;
use winit::keyboard::{Key as Logical, KeyCode, NamedKey, PhysicalKey};

/// A key as winit gives it: its place, what the layout makes of it, its
/// text, if it has any, and whether it went down or up.
#[derive(Debug, Clone)]
pub struct KeyPress {
    pub physical: PhysicalKey,
    pub logical: Logical,
    pub text: Option<String>,
    pub pressed: bool,
}

/// The memex's event for a key pressed or released, with Shift held or
/// not: a quasimode's key, a key with an editing job, or one bound by the
/// configuration. A modifier with no quasimode, and a key with no job and
/// no binding, give none.
pub fn map_key(k: &KeyPress, shift: bool, config: &Config) -> Option<Event> {
    let press = if k.pressed { Press::Down } else { Press::Up };
    let key = memex_key(k, shift, config)?;
    Some(Event { key, press })
}

/// The memex's key for a key, with Shift held or not.
fn memex_key(k: &KeyPress, shift: bool, config: &Config) -> Option<Key> {
    match modifier_names(k) {
        Some(names) => Some(Key::Quasi(quasi_held_on(names, config)?)),
        None => configured(key_of(k, shift)?, config),
    }
}

/// The quasimode the configuration holds on a modifier, by any of its
/// names.
fn quasi_held_on(names: &[&'static str], config: &Config) -> Option<Quasi> {
    names
        .iter()
        .find_map(|n| config.quasi_for(QuasiKey::Modifier(n)))
}

/// What a key is under the configuration: a quasimode's key, or itself;
/// none if it has no job and no binding.
fn configured(key: Key, config: &Config) -> Option<Key> {
    if let Some(q) = config.quasi_for(QuasiKey::Key(key)) {
        return Some(Key::Quasi(q));
    }
    let bound = config.bindings.iter().any(|(b, _)| *b == key);
    (bound || !matches!(key, Key::Special(_))).then_some(key)
}

impl From<&winit::event::KeyEvent> for KeyPress {
    fn from(e: &winit::event::KeyEvent) -> KeyPress {
        let text = e.text.as_ref().map(|t| t.to_string());
        KeyPress {
            physical: e.physical_key,
            logical: e.logical_key.clone(),
            text,
            pressed: e.state == winit::event::ElementState::Pressed,
        }
    }
}

/// The configuration's names for a modifier key, by its place; Right Alt
/// is also IsoLevel3Shift where the layout makes it AltGr. Shift has none:
/// it is no quasimode's. None if the key is no modifier.
fn modifier_names(k: &KeyPress) -> Option<&'static [&'static str]> {
    let PhysicalKey::Code(code) = k.physical else {
        return None;
    };
    let alt_graph = k.logical == Logical::Named(NamedKey::AltGraph);
    Some(match code {
        KeyCode::AltLeft => &["LeftAlt"],
        KeyCode::AltRight if alt_graph => &["RightAlt", "IsoLevel3Shift"],
        KeyCode::AltRight => &["RightAlt"],
        KeyCode::ControlLeft => &["LeftControl"],
        KeyCode::ControlRight => &["RightControl"],
        KeyCode::SuperLeft => &["LeftSuper"],
        KeyCode::SuperRight => &["RightSuper"],
        KeyCode::Hyper => &["LeftHyper", "RightHyper"],
        KeyCode::Meta => &["LeftMeta", "RightMeta"],
        KeyCode::ShiftLeft | KeyCode::ShiftRight => &[],
        _ => return None,
    })
}

/// The memex's key for a key that is not a modifier.
fn key_of(k: &KeyPress, shift: bool) -> Option<Key> {
    match &k.logical {
        Logical::Named(n) => named(*n, shift),
        Logical::Character(c) => character(k.text.as_deref(), c),
        _ => None,
    }
}

/// A character: the key's text if it is one printable character, else the
/// layout's character for the key.
fn character(text: Option<&str>, layout: &str) -> Option<Key> {
    let one = |s: &str| {
        let mut cs = s.chars();
        cs.next().filter(|c| !c.is_control() && cs.next().is_none())
    };
    text.and_then(one).or_else(|| one(layout)).map(Key::Char)
}

/// The memex's key for a named key; Tab with Shift is BackTab.
fn named(n: NamedKey, shift: bool) -> Option<Key> {
    Some(match n {
        NamedKey::Tab if shift => Key::BackTab,
        NamedKey::Space => Key::Char(' '),
        NamedKey::Insert => Key::Special(Special::Insert),
        NamedKey::PageUp => Key::Special(Special::PageUp),
        NamedKey::PageDown => Key::Special(Special::PageDown),
        NamedKey::ContextMenu => Key::Special(Special::Menu),
        NamedKey::Pause => Key::Special(Special::Pause),
        NamedKey::ScrollLock => Key::Special(Special::ScrollLock),
        NamedKey::PrintScreen => Key::Special(Special::PrintScreen),
        NamedKey::CapsLock => Key::Special(Special::CapsLock),
        NamedKey::NumLock => Key::Special(Special::NumLock),
        _ => return editing(n).or_else(|| function(n)),
    })
}

/// A key with an editing job.
fn editing(n: NamedKey) -> Option<Key> {
    Some(match n {
        NamedKey::Enter => Key::Enter,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Delete => Key::Delete,
        NamedKey::Escape => Key::Escape,
        NamedKey::Tab => Key::Tab,
        NamedKey::ArrowUp => Key::Up,
        NamedKey::ArrowDown => Key::Down,
        NamedKey::ArrowLeft => Key::Left,
        NamedKey::ArrowRight => Key::Right,
        NamedKey::Home => Key::Home,
        NamedKey::End => Key::End,
        _ => return None,
    })
}

/// A function key, F1 to F35, as the configuration names them.
fn function(n: NamedKey) -> Option<Key> {
    const KEYS: [NamedKey; 35] = [
        NamedKey::F1,
        NamedKey::F2,
        NamedKey::F3,
        NamedKey::F4,
        NamedKey::F5,
        NamedKey::F6,
        NamedKey::F7,
        NamedKey::F8,
        NamedKey::F9,
        NamedKey::F10,
        NamedKey::F11,
        NamedKey::F12,
        NamedKey::F13,
        NamedKey::F14,
        NamedKey::F15,
        NamedKey::F16,
        NamedKey::F17,
        NamedKey::F18,
        NamedKey::F19,
        NamedKey::F20,
        NamedKey::F21,
        NamedKey::F22,
        NamedKey::F23,
        NamedKey::F24,
        NamedKey::F25,
        NamedKey::F26,
        NamedKey::F27,
        NamedKey::F28,
        NamedKey::F29,
        NamedKey::F30,
        NamedKey::F31,
        NamedKey::F32,
        NamedKey::F33,
        NamedKey::F34,
        NamedKey::F35,
    ];
    let at = KEYS.iter().position(|k| *k == n)?;
    Some(Key::F(at as u8 + 1))
}

/// What a click on a cell acts on, if anything.
pub fn clicked(hits: &Presentations, (column, row): (u16, u16)) -> Option<Presentation> {
    hits.at(column, row).cloned()
}

// Text selected with the mouse, as the window shows it: the cells from
// where the left button went down to where it is, in reading order,
// within the pane where it began (the columns between the `│` that part
// the panes). A right-click plumbs the text selected, or, with none, the
// word under the pointer (crate::plumb).

/// The mouse's selection: the text selected, if any, and whether the
/// left button is down, selecting.
#[derive(Debug, Default, Clone, Copy)]
pub struct Drag {
    pub selection: Option<Selection>,
    pub dragging: bool,
}

impl Drag {
    /// The left button down at a cell: a selection starts there; with no
    /// cell (the mouse off, or a menu open), none does.
    pub fn press(&mut self, at: Option<(u16, u16)>) {
        self.selection = at.map(|cell| Selection {
            from: cell,
            to: cell,
        });
        self.dragging = at.is_some();
    }

    /// The pointer moved to a cell: a selection being made ends there.
    pub fn moved(&mut self, to: (u16, u16)) {
        if let Some(s) = self.selection.as_mut().filter(|_| self.dragging) {
            s.to = to;
        }
    }

    /// The text a right-click at a cell plumbs: the text selected, or,
    /// with none, the word under the cell.
    pub fn text_at(&self, buf: &Buffer, cell: (u16, u16)) -> String {
        match self.selection {
            Some(s) => s.text(buf),
            None => word_at(buf, cell),
        }
    }

    /// The left button up: a selection of one cell was a click alone.
    pub fn release(&mut self) {
        self.dragging = false;
        if self.selection.is_some_and(|s| s.is_click()) {
            self.selection = None;
        }
    }
}

/// A selection, by the cell where it began and the cell where it ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub from: (u16, u16),
    pub to: (u16, u16),
}

/// The pane a selection lies in: its first column and the one after its
/// last.
type Columns = (u16, u16);

impl Selection {
    /// Whether it holds only the cell it began at: a click, not a drag.
    pub fn is_click(&self) -> bool {
        self.from == self.to
    }

    /// Its cells, row by row: each row and the columns it takes.
    fn rows(&self, buf: &Buffer) -> Vec<(u16, Columns)> {
        let pane = pane_of(buf, self.from);
        let ends = ordered(self.from, self.to);
        let rows = ends.0.1..=ends.1.1;
        rows.map(|y| (y, columns(y, ends, pane))).collect()
    }

    /// The text it holds: each row's characters, trimmed, joined by a
    /// space, as a wrapped line reads.
    pub fn text(&self, buf: &Buffer) -> String {
        let row = |(y, cols): (u16, Columns)| symbols(buf, y, cols).trim().to_string();
        let rows: Vec<String> = self.rows(buf).into_iter().map(row).collect();
        rows.into_iter()
            .filter(|r| !r.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Draw it reversed, as the outline draws its cursor.
    pub fn mark(&self, buf: &mut Buffer) {
        for (y, (first, last)) in self.rows(buf) {
            for x in first..last {
                let cell = &mut buf[(x, y)];
                cell.modifier.toggle(Modifier::REVERSED);
            }
        }
    }
}

/// The columns a selection takes in a row: from its start on its first
/// row, else from the pane's edge; to its end on its last row, else to
/// the pane's other edge.
fn columns(y: u16, (start, end): ((u16, u16), (u16, u16)), pane: Columns) -> Columns {
    let first = if y == start.1 {
        start.0.max(pane.0)
    } else {
        pane.0
    };
    let last = if y == end.1 {
        (end.0 + 1).min(pane.1)
    } else {
        pane.1
    };
    (first, last.max(first))
}

/// Two cells in reading order: by row, then by column.
fn ordered(a: (u16, u16), b: (u16, u16)) -> ((u16, u16), (u16, u16)) {
    if (a.1, a.0) <= (b.1, b.0) {
        (a, b)
    } else {
        (b, a)
    }
}

/// The columns of the pane that holds a cell: from after the `│` before
/// it, if any, to the `│` after it, or the edge.
fn pane_of(buf: &Buffer, (x, y): (u16, u16)) -> Columns {
    let width = buf.area.width;
    let bar = |c: &u16| buf[(*c, y)].symbol() == "│";
    let first = (0..x).rev().find(bar).map_or(0, |c| c + 1);
    let last = (x..width).find(bar).unwrap_or(width);
    (first, last)
}

/// The characters of a row's columns.
fn symbols(buf: &Buffer, y: u16, (first, last): Columns) -> String {
    (first..last).map(|x| buf[(x, y)].symbol()).collect()
}

/// The word under a cell: the run of characters around it that are not
/// spaces or a pane's `│`, without the punctuation around it, and without
/// a full stop, question or exclamation mark that ends it.
pub fn word_at(buf: &Buffer, cell: (u16, u16)) -> String {
    let (row, at) = row_around(buf, cell);
    let (start, end) = word_span(&row, at);
    let word: String = row[start..end].iter().collect();
    let around: &[char] = &[
        ',', ';', ':', '(', ')', '[', ']', '"', '“', '”', '‘', '’', '\'',
    ];
    word.trim_matches(around)
        .trim_end_matches(['.', '?', '!'])
        .to_string()
}

/// Where the run of characters that are not spaces around a place begins
/// and ends.
fn word_span(row: &[char], at: usize) -> (usize, usize) {
    let apart = |c: &char| c.is_whitespace();
    let start = row[..at].iter().rposition(apart).map_or(0, |i| i + 1);
    let end = row[at..]
        .iter()
        .position(apart)
        .map_or(row.len(), |i| at + i);
    (start, end)
}

/// The characters of the pane's row that holds a cell, and where in them
/// the cell is.
fn row_around(buf: &Buffer, (x, y): (u16, u16)) -> (Vec<char>, usize) {
    let pane = pane_of(buf, (x, y));
    let row: Vec<char> = symbols(buf, y, pane).chars().collect();
    let at = usize::from(x.saturating_sub(pane.0)).min(row.len());
    (row, at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode, logical: Logical, text: Option<&str>) -> KeyPress {
        let text = text.map(String::from);
        KeyPress {
            physical: PhysicalKey::Code(code),
            logical,
            text,
            pressed: true,
        }
    }

    /// The key let go.
    fn released(k: &KeyPress) -> KeyPress {
        KeyPress {
            pressed: false,
            ..k.clone()
        }
    }

    fn named_key(code: KeyCode, n: NamedKey) -> KeyPress {
        press(code, Logical::Named(n), None)
    }

    fn letter(code: KeyCode, c: &str, text: Option<&str>) -> KeyPress {
        press(code, Logical::Character(c.into()), text)
    }

    /// The key a press gives, without Shift.
    fn pressed(k: &KeyPress, c: &Config) -> Option<Key> {
        map_key(k, false, c).map(|e| e.key)
    }

    fn remapped() -> Config {
        let toml = "[quasimodes]\nleap_forward = \"F12\"\n[keys]\nInsert = \"quit\"\n";
        Config::parse(toml).unwrap()
    }

    #[test]
    fn right_alt_released_ends_leap_forward() {
        // Catches a release read as a press, or Right Alt not Leap's.
        let k = named_key(KeyCode::AltRight, NamedKey::Alt);
        let e = map_key(&released(&k), false, &Config::default()).unwrap();
        assert_eq!(
            e,
            Event {
                key: Key::Quasi(Quasi::LeapForward),
                press: Press::Up
            }
        );
    }

    #[test]
    fn right_alt_made_alt_graph_by_the_layout_is_still_leap_forward() {
        // Catches AltGr (IsoLevel3Shift) missed where the layout has one.
        let k = named_key(KeyCode::AltRight, NamedKey::AltGraph);
        let e = map_key(&k, false, &Config::default()).unwrap();
        assert_eq!(e.key, Key::Quasi(Quasi::LeapForward));
    }

    #[test]
    fn left_control_pressed_starts_command() {
        // Catches the modifiers' places confused: Left Ctrl is Command,
        // Right Ctrl has no job (Scope, which it held, was removed).
        let left = named_key(KeyCode::ControlLeft, NamedKey::Control);
        let right = named_key(KeyCode::ControlRight, NamedKey::Control);
        assert_eq!(
            pressed(&left, &Config::default()),
            Some(Key::Quasi(Quasi::Command))
        );
        assert_eq!(pressed(&right, &Config::default()), None);
    }

    #[test]
    fn a_character_typed_with_control_held_is_the_layouts() {
        // Catches commands lost while Ctrl is held: the key's text is then
        // a control character, and "(" must come from the layout.
        let paren = letter(KeyCode::Digit9, "(", Some("\u{1b}"));
        let e = map_key(&paren, true, &Config::default()).unwrap();
        assert_eq!(e.key, Key::Char('('));
        let typed = letter(KeyCode::KeyE, "e", Some("é"));
        assert_eq!(
            map_key(&typed, false, &Config::default()).unwrap().key,
            Key::Char('é')
        );
    }

    #[test]
    fn shift_tab_is_back_tab_and_space_a_character() {
        // Catches Shift ignored on Tab, or Space dropped as a named key.
        let tab = named_key(KeyCode::Tab, NamedKey::Tab);
        let space = named_key(KeyCode::Space, NamedKey::Space);
        let c = Config::default();
        assert_eq!(map_key(&tab, true, &c).unwrap().key, Key::BackTab);
        assert_eq!(map_key(&tab, false, &c).unwrap().key, Key::Tab);
        assert_eq!(map_key(&space, false, &c).unwrap().key, Key::Char(' '));
    }

    #[test]
    fn a_key_with_no_job_and_no_binding_is_dropped() {
        // Catches Caps Lock or Shift reaching the app, and a bound key
        // dropped.
        let caps = named_key(KeyCode::CapsLock, NamedKey::CapsLock);
        let shift = named_key(KeyCode::ShiftLeft, NamedKey::Shift);
        let insert = named_key(KeyCode::Insert, NamedKey::Insert);
        assert_eq!(map_key(&caps, false, &Config::default()), None);
        assert_eq!(map_key(&shift, false, &Config::default()), None);
        let bound = map_key(&insert, false, &remapped()).unwrap();
        assert_eq!(bound.key, Key::Special(Special::Insert));
    }

    #[test]
    fn a_quasimode_moved_to_a_function_key_reports_its_release() {
        // Catches F12 not Leap's when the configuration moves Leap there.
        let f12 = released(&named_key(KeyCode::F12, NamedKey::F12));
        let leap_up = Event::up(Key::Quasi(Quasi::LeapForward));
        assert_eq!(map_key(&f12, false, &remapped()), Some(leap_up));
    }

    #[test]
    fn a_quasimode_moved_away_leaves_its_old_key() {
        // Catches Right Alt still Leap's when Leap is moved to F12.
        let alt = named_key(KeyCode::AltRight, NamedKey::Alt);
        assert_eq!(pressed(&alt, &remapped()), None);
    }

    #[test]
    fn a_function_key_with_no_quasimode_is_itself() {
        // Catches a function key dropped as unbound, or misnumbered.
        let f10 = named_key(KeyCode::F10, NamedKey::F10);
        assert_eq!(pressed(&f10, &Config::default()), Some(Key::F(10)));
    }

    #[test]
    fn every_function_key_the_configuration_names_arrives() {
        // Catches F13 to F35, which the configuration takes, dropped by
        // the window, so a key bound to one did nothing.
        let f13 = named_key(KeyCode::F13, NamedKey::F13);
        assert_eq!(pressed(&f13, &Config::default()), Some(Key::F(13)));
        let f35 = named_key(KeyCode::F35, NamedKey::F35);
        assert_eq!(pressed(&f35, &Config::default()), Some(Key::F(35)));
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use ratatui::layout::Rect;

    /// A buffer of these rows, each as wide as the first.
    fn screen(rows: &[&str]) -> Buffer {
        let width = rows[0].chars().count() as u16;
        let mut buf = Buffer::empty(Rect::new(0, 0, width, rows.len() as u16));
        for (y, r) in rows.iter().enumerate() {
            buf.set_string(0, y as u16, r, ratatui::style::Style::default());
        }
        buf
    }

    const PANES: [&str; 3] = [
        "• Buran flew in 1988.   │ LINKS OF IT   ",
        "  It landed by itself,  │ cites 1.3     ",
        "  see 1.3.              │ #12 here      ",
    ];

    #[test]
    fn a_selection_reads_in_order_within_its_pane() {
        // Catches the other pane's text taken in, rows out of order, a
        // drag up read backwards, or a row's indent kept.
        let buf = screen(&PANES);
        let down = Selection {
            from: (8, 0),
            to: (8, 1),
        };
        assert_eq!(down.text(&buf), "flew in 1988. It land");
        let up = Selection {
            from: (8, 1),
            to: (8, 0),
        };
        assert_eq!(up.text(&buf), down.text(&buf));
        let right = Selection {
            from: (26, 1),
            to: (34, 1),
        };
        assert_eq!(right.text(&buf), "cites 1.3");
        assert!(
            Selection {
                from: (3, 2),
                to: (3, 2)
            }
            .is_click()
        );
    }

    #[test]
    fn a_click_takes_the_word_under_it_without_its_punctuation() {
        // Catches a word cut at the click, punctuation around it kept, an
        // address's own full stop lost, or the pane's bar taken in.
        let buf = screen(&PANES);
        assert_eq!(word_at(&buf, (7, 2)), "1.3");
        assert_eq!(word_at(&buf, (17, 0)), "1988");
        assert_eq!(word_at(&buf, (20, 1)), "itself");
        assert_eq!(word_at(&buf, (27, 2)), "#12");
        assert_eq!(word_at(&buf, (21, 2)), "");
    }

    #[test]
    fn marking_reverses_the_cells_selected_alone() {
        // Catches a cell outside the selection marked, or one inside it
        // left as it was.
        let mut buf = screen(&PANES);
        Selection {
            from: (2, 0),
            to: (6, 0),
        }
        .mark(&mut buf);
        let reversed = |x: u16| buf[(x, 0)].modifier.contains(Modifier::REVERSED);
        assert!((2..=6).all(reversed));
        assert!(!reversed(1) && !reversed(7));
    }
}
