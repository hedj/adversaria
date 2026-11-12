//! The terminal: checking it can do what the memex needs, reading keys, and
//! drawing (docs/design.md § The PC prototype).
//!
//! The memex needs two of kitty's protocols: the keyboard protocol, for key
//! releases (a held quasimode ends when its key is released), and the
//! graphics protocol, for images. At start it asks the terminal, and refuses
//! to start if either is missing.

#[cfg(test)]
use crate::app::Quasi;
use crate::app::{App, Event, Key, Press, Special};
use crate::config::{Config, QuasiKey};
use crate::ui;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{
    self, KeyCode, KeyEventKind, KeyboardEnhancementFlags, ModifierKeyCode,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::{execute, terminal};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

/// The probe: ask for the kitty keyboard flags (CSI ? u), query graphics
/// support (an APC that names image 31), then ask for the primary device
/// attributes (CSI c), which every terminal answers. A reply to either of
/// the first two before the attributes means the protocol is there. (After
/// the kitty documentation's advice for the graphics query.)
pub const PROBE: &[u8] = b"\x1b[?u\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Support {
    pub keyboard: bool,
    pub graphics: bool,
}

/// Read a terminal's reply to PROBE. None until the device attributes
/// reply (ESC [ ? ... c) has arrived.
pub fn parse_probe(reply: &[u8]) -> Option<Support> {
    let s = String::from_utf8_lossy(reply);
    let da = find_csi(&s, 'c')?;
    let before = &s[..da];
    let keyboard = find_csi(before, 'u').is_some();
    let graphics = before.contains("\x1b_Gi=31;");
    Some(Support { keyboard, graphics })
}

/// The position of a reply "ESC [ ? digits/semicolons <end>", if any.
fn find_csi(s: &str, end: char) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = s[from..].find("\x1b[?") {
        let start = from + i;
        let rest = &s[start + 3..];
        let n = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == ';')
            .count();
        if rest[n..].starts_with(end) {
            return Some(start);
        }
        from = start + 3;
    }
    None
}

/// Ask the terminal on /dev/tty. The reading thread stops at the device
/// attributes reply, so it takes no later keys.
pub fn probe(timeout: Duration) -> io::Result<Option<Support>> {
    let mut tty_in = std::fs::File::open("/dev/tty")?;
    let mut tty_out = std::fs::OpenOptions::new().write(true).open("/dev/tty")?;
    tty_out.write_all(PROBE)?;
    tty_out.flush()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut got = Vec::new();
        let mut b = [0u8; 1];
        while tty_in.read_exact(&mut b).is_ok() {
            got.push(b[0]);
            if let Some(s) = parse_probe(&got) {
                let _ = tx.send(s);
                return;
            }
        }
    });
    Ok(rx.recv_timeout(timeout).ok())
}

/// The keys the memex reads, from crossterm's key events. Which keys are
/// the quasimodes comes from the configuration (crate::config); a key with
/// no editing job and no configured use is ignored.
pub fn map_key(k: event::KeyEvent, config: &Config) -> Option<Event> {
    let press = match k.kind {
        KeyEventKind::Release => Press::Up,
        _ => Press::Down,
    };
    let key = match k.code {
        KeyCode::Modifier(m) => {
            Key::Quasi(config.quasi_for(QuasiKey::Modifier(modifier_name(m)?))?)
        }
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Esc => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::F(n) => Key::F(n),
        KeyCode::Insert => Key::Special(Special::Insert),
        KeyCode::PageUp => Key::Special(Special::PageUp),
        KeyCode::PageDown => Key::Special(Special::PageDown),
        KeyCode::Menu => Key::Special(Special::Menu),
        KeyCode::Pause => Key::Special(Special::Pause),
        KeyCode::ScrollLock => Key::Special(Special::ScrollLock),
        KeyCode::PrintScreen => Key::Special(Special::PrintScreen),
        KeyCode::CapsLock => Key::Special(Special::CapsLock),
        KeyCode::NumLock => Key::Special(Special::NumLock),
        _ => return None,
    };
    if let Some(q) = config.quasi_for(QuasiKey::Key(key)) {
        return Some(Event {
            key: Key::Quasi(q),
            press,
        });
    }
    // An unbound key with no editing job does nothing; dropping it here
    // keeps lock keys from reaching the app at all.
    if matches!(key, Key::Special(_)) && !config.bindings.iter().any(|(b, _)| *b == key) {
        return None;
    }
    Some(Event { key, press })
}

/// The configuration's name for a modifier key.
fn modifier_name(m: ModifierKeyCode) -> Option<&'static str> {
    use ModifierKeyCode::*;
    Some(match m {
        LeftAlt => "LeftAlt",
        RightAlt => "RightAlt",
        LeftControl => "LeftControl",
        RightControl => "RightControl",
        LeftSuper => "LeftSuper",
        RightSuper => "RightSuper",
        LeftHyper => "LeftHyper",
        RightHyper => "RightHyper",
        LeftMeta => "LeftMeta",
        RightMeta => "RightMeta",
        IsoLevel3Shift => "IsoLevel3Shift",
        IsoLevel5Shift => "IsoLevel5Shift",
        LeftShift | RightShift => return None,
    })
}

/// Why the terminal will not do, if it will not.
pub fn refusal(s: Option<Support>) -> Option<String> {
    match s {
        None => Some("the terminal did not answer a device attributes request within 2 s".into()),
        Some(Support {
            keyboard: true,
            graphics: true,
        }) => None,
        Some(s) => {
            let mut missing = Vec::new();
            if !s.keyboard {
                missing.push("the kitty keyboard protocol (needed for key releases)");
            }
            if !s.graphics {
                missing.push("the kitty graphics protocol (needed for images)");
            }
            Some(format!("this terminal lacks {}", missing.join(" and ")))
        }
    }
}

/// Run the memex on a journal directory in this terminal.
pub fn run(dir: &Path, config: Config) -> Result<(), String> {
    terminal::enable_raw_mode().map_err(|e| e.to_string())?;
    let support = probe(Duration::from_secs(2));
    if let Some(why) = refusal(support.as_ref().ok().copied().flatten()) {
        let _ = terminal::disable_raw_mode();
        return Err(why);
    }
    let mut app = App::open(dir).map_err(|e| e.to_string())?;
    app.bindings = config.bindings.clone();
    let mut out = io::stdout();
    let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS;
    execute!(
        out,
        terminal::EnterAlternateScreen,
        PushKeyboardEnhancementFlags(flags)
    )
    .map_err(|e| e.to_string())?;
    let mut term = Terminal::new(CrosstermBackend::new(out)).map_err(|e| e.to_string())?;
    let result = (|| -> io::Result<()> {
        let mut evaluated_at = None;
        while !app.quit {
            // Reason again when the journal has moved: constraints and
            // derived values are computed from it, never stored in it.
            if evaluated_at != Some((app.tip, app.journal().entries().len())) {
                if let Err(e) = app.evaluate() {
                    app.message = e;
                }
                evaluated_at = Some((app.tip, app.journal().entries().len()));
            }
            term.draw(|f| ui::draw(f, &app))?;
            if event::poll(Duration::from_secs(1))? {
                if let event::Event::Key(k) = event::read()?
                    && let Some(ev) = map_key(k, &config)
                {
                    app.handle(ev);
                }
            } else if app.pending() {
                // A second without a key: write what is pending.
                app.flush();
            }
        }
        app.flush();
        Ok(())
    })();
    let _ = execute!(
        io::stdout(),
        PopKeyboardEnhancementFlags,
        terminal::LeaveAlternateScreen
    );
    let _ = terminal::disable_raw_mode();
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_replies() {
        // Catches a terminal taken to support a protocol it did not answer for,
        // or a reply read before it is complete.
        let kitty = b"\x1b[?0u\x1b_Gi=31;OK\x1b\\\x1b[?62;c";
        assert_eq!(
            parse_probe(kitty),
            Some(Support {
                keyboard: true,
                graphics: true
            })
        );
        let plain = b"\x1b[?62;22c";
        assert_eq!(
            parse_probe(plain),
            Some(Support {
                keyboard: false,
                graphics: false
            })
        );
        let keys_only = b"\x1b[?1u\x1b[?65;1;9c";
        assert_eq!(
            parse_probe(keys_only),
            Some(Support {
                keyboard: true,
                graphics: false
            })
        );
        let graphics_error = b"\x1b_Gi=31;ENOTSUPPORTED:x\x1b\\\x1b[?62c";
        assert_eq!(
            parse_probe(graphics_error),
            Some(Support {
                keyboard: false,
                graphics: true
            })
        );
        assert_eq!(
            parse_probe(b"\x1b[?0u\x1b_Gi=31;OK"),
            None,
            "no device attributes yet"
        );
        assert_eq!(parse_probe(b"\x1b[?62;"), None);
        // A reply after the device attributes does not count.
        assert_eq!(
            parse_probe(b"\x1b[?62c\x1b[?0u"),
            Some(Support {
                keyboard: false,
                graphics: false
            })
        );
    }

    #[test]
    fn refusals_name_what_is_missing() {
        // Catches a refusal that does not say which protocol is missing.
        assert_eq!(
            refusal(Some(Support {
                keyboard: true,
                graphics: true
            })),
            None
        );
        assert!(
            refusal(Some(Support {
                keyboard: false,
                graphics: true
            }))
            .unwrap()
            .contains("keyboard")
        );
        let both = refusal(Some(Support {
            keyboard: false,
            graphics: false,
        }))
        .unwrap();
        assert!(both.contains("keyboard") && both.contains("graphics"));
        assert!(refusal(None).unwrap().contains("did not answer"));
    }

    #[test]
    fn keys_map_with_their_releases() {
        // Catches a quasimode key whose release is lost, or keys mismapped.
        use event::{KeyEvent, KeyEventState, KeyModifiers};
        let c = Config::default();
        let k = |code, kind| KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind,
            state: KeyEventState::NONE,
        };
        assert_eq!(
            map_key(
                k(
                    KeyCode::Modifier(ModifierKeyCode::RightAlt),
                    KeyEventKind::Release
                ),
                &c
            ),
            Some(Event::up(Key::Quasi(Quasi::LeapForward)))
        );
        assert_eq!(
            map_key(
                k(
                    KeyCode::Modifier(ModifierKeyCode::LeftControl),
                    KeyEventKind::Press
                ),
                &c
            ),
            Some(Event::down(Key::Quasi(Quasi::Command)))
        );
        assert_eq!(
            map_key(k(KeyCode::Char('d'), KeyEventKind::Repeat), &c),
            Some(Event::down(Key::Char('d')))
        );
        assert_eq!(map_key(k(KeyCode::CapsLock, KeyEventKind::Press), &c), None);
    }

    #[test]
    fn keys_follow_the_configuration() {
        // Catches a remapped quasimode or binding that the reader ignores.
        use event::{KeyEvent, KeyEventState, KeyModifiers};
        let k = |code, kind| KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind,
            state: KeyEventState::NONE,
        };
        let c = Config::parse(
            "[quasimodes]\nleap_forward = [\"F12\"]\ncommand = [\"RightAlt\"]\n[keys]\nInsert = \"undo\"\n",
        )
        .unwrap();
        let alt = KeyCode::Modifier(ModifierKeyCode::RightAlt);
        assert_eq!(
            map_key(k(alt, KeyEventKind::Press), &c),
            Some(Event::down(Key::Quasi(Quasi::Command)))
        );
        assert_eq!(
            map_key(k(KeyCode::F(12), KeyEventKind::Release), &c),
            Some(Event::up(Key::Quasi(Quasi::LeapForward)))
        );
        assert_eq!(
            map_key(k(KeyCode::Insert, KeyEventKind::Press), &c),
            Some(Event::down(Key::Special(Special::Insert)))
        );
        assert_eq!(
            map_key(
                k(
                    KeyCode::Modifier(ModifierKeyCode::LeftControl),
                    KeyEventKind::Press
                ),
                &c
            ),
            None,
            "LeftControl lost its job when command moved"
        );
    }
}
