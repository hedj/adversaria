//! A keyboard for the window test under Wayland (tests/memex_in_a_window.py):
//! one virtual keyboard (zwp-virtual-keyboard), kept on the seat until its
//! input ends (crate::seat says why, and what it writes). Its keymap is a
//! standard one, compiled by libxkbcommon for a US layout, so a key is
//! named by its place on the keyboard, its evdev code, as a real keyboard
//! sends it: Right Alt is 100, Left Control 29.
//!
//! Reads commands, one a line, from standard input:
//!   down CODE    press the key with that evdev code
//!   up CODE      release it
//!   key CODE     press and release it
//!   type TEXT    type the text, of the US layout's printable characters,
//!                with Shift where a character needs it
//! Unknown lines are refused.
//!
//! Usage: keyboard < commands   (WAYLAND_DISPLAY names the compositor)

use std::ffi::CStr;
use std::io::Write;
use std::os::fd::AsFd;
use std::time::Instant;
use tools::seat::{Compositor, run_as};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1 as Manager;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1 as Keyboard;
use xkbcommon_dl::{
    XkbCommon, xkb_context_flags, xkb_key_direction, xkb_keymap, xkb_keymap_compile_flags,
    xkb_keymap_format, xkb_rule_names, xkb_state, xkb_state_component,
};

/// Left Shift's evdev code.
const SHIFT: u32 = 42;

fn main() {
    run_as("keyboard", run);
}

fn run() -> Result<(), String> {
    let mut compositor = Compositor::connect()?;
    let xkb = Xkb::us()?;
    let keyboard = virtual_keyboard(&mut compositor, &xkb)?;
    let mut hands = Hands {
        keyboard,
        started: Instant::now(),
        xkb,
    };
    compositor.serve(|line| hands.send(&parse(line)?))?;
    hands.keyboard.destroy();
    compositor.settle()
}

/// A virtual keyboard on the seat, made by the compositor's manager of
/// them, with the keymap, which the compositor has once this returns.
fn virtual_keyboard(compositor: &mut Compositor, xkb: &Xkb) -> Result<Keyboard, String> {
    let manager: Manager = compositor.bind(1..=1, "virtual keyboards")?;
    let keyboard =
        manager.create_virtual_keyboard(&compositor.seat, &compositor.queue.handle(), ());
    let keymap = keymap_file(&xkb.text()?)?;
    let size = keymap.metadata().map_err(|e| e.to_string())?.len() as u32;
    let format = xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1 as u32;
    keyboard.keymap(format, keymap.as_fd(), size);
    compositor.settle()?;
    Ok(keyboard)
}

/// A US keymap, compiled by libxkbcommon, and the state of its keys and
/// modifiers, as a compositor keeps them for a real keyboard: a virtual
/// keyboard sends its modifiers itself.
struct Xkb {
    lib: &'static XkbCommon,
    keymap: *mut xkb_keymap,
    state: *mut xkb_state,
}

impl Xkb {
    fn us() -> Result<Xkb, String> {
        let lib = xkbcommon_dl::xkbcommon_option().ok_or("no libxkbcommon")?;
        let names = xkb_rule_names {
            rules: std::ptr::null(),
            model: std::ptr::null(),
            layout: c"us".as_ptr(),
            variant: std::ptr::null(),
            options: std::ptr::null(),
        };
        // Safety: the context is checked before use and released once the
        // keymap holds it; the keymap and state are checked, and released
        // when the Xkb is dropped.
        unsafe {
            let context = (lib.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_FLAGS);
            if context.is_null() {
                return Err("libxkbcommon made no context".into());
            }
            let flags = xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS;
            let keymap = (lib.xkb_keymap_new_from_names)(context, &names, flags);
            (lib.xkb_context_unref)(context);
            if keymap.is_null() {
                return Err("libxkbcommon made no US keymap".into());
            }
            let state = (lib.xkb_state_new)(keymap);
            if state.is_null() {
                (lib.xkb_keymap_unref)(keymap);
                return Err("libxkbcommon made no keyboard state".into());
            }
            Ok(Xkb { lib, keymap, state })
        }
    }

    /// The keymap as text, as the protocol sends it.
    fn text(&self) -> Result<String, String> {
        let format = xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1;
        // Safety: the keymap is live; the text is checked, then copied.
        unsafe {
            let text = (self.lib.xkb_keymap_get_as_string)(self.keymap, format);
            if text.is_null() {
                return Err("libxkbcommon wrote no keymap".into());
            }
            Ok(CStr::from_ptr(text).to_string_lossy().into_owned())
        }
    }

    /// A key pressed or released, by its evdev code; the modifiers then,
    /// if they changed.
    fn update(&mut self, code: u32, down: bool) -> Option<Modifiers> {
        let direction = match down {
            true => xkb_key_direction::XKB_KEY_DOWN,
            false => xkb_key_direction::XKB_KEY_UP,
        };
        // Safety: the state is live. xkb numbers keys from evdev's plus 8.
        let changed = unsafe { (self.lib.xkb_state_update_key)(self.state, code + 8, direction) };
        (!changed.is_empty()).then(|| self.modifiers())
    }

    /// The modifiers held, latched and locked, and the layout in effect.
    fn modifiers(&self) -> Modifiers {
        use xkb_state_component as part;
        // Safety: the state is live.
        let mods = |which| unsafe { (self.lib.xkb_state_serialize_mods)(self.state, which) };
        let layout = unsafe {
            (self.lib.xkb_state_serialize_layout)(self.state, part::XKB_STATE_LAYOUT_EFFECTIVE)
        };
        Modifiers {
            depressed: mods(part::XKB_STATE_MODS_DEPRESSED),
            latched: mods(part::XKB_STATE_MODS_LATCHED),
            locked: mods(part::XKB_STATE_MODS_LOCKED),
            layout,
        }
    }
}

/// The modifiers held, latched and locked, and the layout in effect, as
/// xkb masks and index: what a keyboard tells the compositor.
struct Modifiers {
    depressed: u32,
    latched: u32,
    locked: u32,
    layout: u32,
}

impl Modifiers {
    fn send(&self, keyboard: &Keyboard) {
        keyboard.modifiers(self.depressed, self.latched, self.locked, self.layout);
    }
}

impl Drop for Xkb {
    fn drop(&mut self) {
        // Safety: both are live, and each is released once.
        unsafe {
            (self.lib.xkb_state_unref)(self.state);
            (self.lib.xkb_keymap_unref)(self.keymap);
        }
    }
}

/// A file holding a keymap, ended by a zero byte as the protocol asks:
/// the compositor maps it by its descriptor.
fn keymap_file(text: &str) -> Result<std::fs::File, String> {
    let mut file = unlinked_file()?;
    file.write_all(text.as_bytes())
        .and_then(|_| file.write_all(&[0]))
        .map_err(|e| e.to_string())?;
    Ok(file)
}

/// A new file, open to read and write, its name already removed, so it
/// goes when it is closed.
fn unlinked_file() -> Result<std::fs::File, String> {
    let path = std::env::temp_dir().join(format!("keyboard-{}.xkb", std::process::id()));
    let opened = std::fs::File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path);
    let file = opened.map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(file)
}

/// The virtual keyboard, when it was made (its events' times count from
/// then, in milliseconds), and its keys' state.
struct Hands {
    keyboard: Keyboard,
    started: Instant,
    xkb: Xkb,
}

impl Hands {
    /// Send one command's key events.
    fn send(&mut self, command: &Command) -> Result<(), String> {
        match command {
            Command::Down(code) => self.press(*code, true),
            Command::Up(code) => self.press(*code, false),
            Command::Key(code) => self.tap(*code, false),
            Command::Type(text) => {
                for c in text.chars() {
                    let (code, shift) = typed(c).ok_or(format!("cannot type {c:?}"))?;
                    self.tap(code, shift);
                }
            }
        }
        Ok(())
    }

    /// A key pressed and released, with Shift held round it or not.
    fn tap(&mut self, code: u32, shift: bool) {
        if shift {
            self.press(SHIFT, true);
        }
        self.press(code, true);
        self.press(code, false);
        if shift {
            self.press(SHIFT, false);
        }
    }

    /// A key pressed or released, then the modifiers if it changed them.
    fn press(&mut self, code: u32, down: bool) {
        let time = self.started.elapsed().as_millis() as u32;
        self.keyboard.key(time, code, u32::from(down));
        if let Some(modifiers) = self.xkb.update(code, down) {
            modifiers.send(&self.keyboard);
        }
    }
}

/// A command: a key pressed, released or tapped, or text typed.
enum Command {
    Down(u32),
    Up(u32),
    Key(u32),
    Type(String),
}

/// A command, read from a line.
fn parse(line: &str) -> Result<Command, String> {
    let code = |w: &str| {
        w.parse::<u32>()
            .map_err(|_| format!("not a key code: {w:?}"))
    };
    match line.split_once(' ').unwrap_or((line, "")) {
        ("down", w) => Ok(Command::Down(code(w)?)),
        ("up", w) => Ok(Command::Up(code(w)?)),
        ("key", w) => Ok(Command::Key(code(w)?)),
        ("type", text) => Ok(Command::Type(text.to_string())),
        _ => Err(format!("not a command: {line:?}")),
    }
}

/// The US layout's rows of printable characters, each with its Shifted
/// row, and the evdev code of its first key.
const ROWS: [(&str, &str, u32); 4] = [
    ("1234567890-=", "!@#$%^&*()_+", 2),
    ("qwertyuiop[]", "QWERTYUIOP{}", 16),
    ("asdfghjkl;'`", "ASDFGHJKL:\"~", 30),
    ("\\zxcvbnm,./", "|ZXCVBNM<>?", 43),
];

/// The key that types a character, by its evdev code, and whether Shift
/// is held for it.
fn typed(c: char) -> Option<(u32, bool)> {
    let found = us_layout().find(|(k, _, _)| *k == c);
    found.map(|(_, code, shift)| (code, shift))
}

/// Every character the US layout types: with its key's evdev code, and
/// whether Shift is held for it.
fn us_layout() -> impl Iterator<Item = (char, u32, bool)> {
    let space = std::iter::once((' ', 57, false));
    ROWS.into_iter().flat_map(row_keys).chain(space)
}

/// A row's characters, plain and Shifted, with their keys' codes.
fn row_keys(
    (plain, shifted, first): (&'static str, &'static str, u32),
) -> impl Iterator<Item = (char, u32, bool)> {
    let keys = (first..).zip(plain.chars().zip(shifted.chars()));
    keys.flat_map(|(code, (p, s))| [(p, code, false), (s, code, true)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_are_typed_by_their_keys_on_a_us_keyboard() {
        // Catches a row counted from the wrong key, or Shift left off: the
        // codes are those of linux/input-event-codes.h.
        assert_eq!(typed('a'), Some((30, false)));
        assert_eq!(typed('H'), Some((35, true)));
        assert_eq!(typed('('), Some((10, true)));
        assert_eq!(typed('\\'), Some((43, false)));
        assert_eq!(typed('/'), Some((53, false)));
        assert_eq!(typed(' '), Some((57, false)));
        assert_eq!(typed('é'), None);
    }

    #[test]
    fn commands_are_read_and_others_refused() {
        // Catches a code misread, or a typo taken for a command.
        assert!(matches!(parse("down 100"), Ok(Command::Down(100))));
        assert!(matches!(parse("type Hello world"), Ok(Command::Type(t)) if t == "Hello world"));
        assert!(parse("down right_alt").is_err());
        assert!(parse("press 100").is_err());
    }
}
