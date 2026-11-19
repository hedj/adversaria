//! Keys and clicks from the window to the memex. A key comes from winit
//! as its place on the keyboard (physical) and what the layout makes of
//! it (logical), with its text. A modifier is named by its place, as the
//! configuration names quasimode keys (Right Alt, Left Control, ...); a
//! character is the key's text, or, where the text is a control character
//! (while Ctrl is held, as for the Command quasimode), the layout's
//! character for the key, which Ctrl does not change.

use crate::app::{Click, Event, Key, Press, Quasi, Special};
use crate::config::{Config, QuasiKey};
use crate::ui::Hits;
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
pub fn clicked(hits: &Hits, (column, row): (u16, u16)) -> Option<Click> {
    hits.at(column, row).cloned()
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
        // Right Ctrl is Scope.
        let left = named_key(KeyCode::ControlLeft, NamedKey::Control);
        let right = named_key(KeyCode::ControlRight, NamedKey::Control);
        assert_eq!(
            pressed(&left, &Config::default()),
            Some(Key::Quasi(Quasi::Command))
        );
        assert_eq!(
            pressed(&right, &Config::default()),
            Some(Key::Quasi(Quasi::Scope))
        );
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
