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
    let mut starts = s.match_indices("\x1b[?").map(|(i, _)| i);
    starts.find(|i| ends_reply(&s[i + 3..], end))
}

/// Whether text starts with digits and semicolons, then the end of a reply.
fn ends_reply(rest: &str, end: char) -> bool {
    let n = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ';')
        .count();
    rest[n..].starts_with(end)
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
    let key = key_of(k.code, config)?;
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

/// The memex's key for a crossterm key code: a modifier only if it is a
/// quasimode's.
fn key_of(code: KeyCode, config: &Config) -> Option<Key> {
    Some(match code {
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
    })
}

/// What a mouse event clicks, if anything: a press of the left button on
/// something drawn to be clicked. Other buttons, releases, drags and the
/// wheel do nothing.
pub fn clicked(m: event::MouseEvent, hits: &ui::Hits) -> Option<crate::app::Click> {
    match m.kind {
        event::MouseEventKind::Down(event::MouseButton::Left) => hits.at(m.column, m.row).cloned(),
        _ => None,
    }
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
    check_terminal()?;
    let app = open(dir, &config)?;
    let mut term = screen(config.mouse)?;
    let result = Session::new(app, &config).run(&mut term);
    leave(config.mouse);
    result.map_err(|e| e.to_string())
}

/// The memex on a journal directory, with the configuration's keys.
fn open(dir: &Path, config: &Config) -> Result<App, String> {
    let mut app = App::open(dir).map_err(|e| e.to_string())?;
    app.bindings = config.bindings.clone();
    Ok(app)
}

/// The terminal to draw on, once its screen is taken.
fn screen(mouse: bool) -> Result<Terminal<CrosstermBackend<io::Stdout>>, String> {
    let mut out = io::stdout();
    enter(&mut out, mouse).map_err(|e| e.to_string())?;
    Terminal::new(CrosstermBackend::new(out)).map_err(|e| e.to_string())
}

/// Put the terminal in raw mode and ask what it can do; if it will not
/// do, say why and leave raw mode.
fn check_terminal() -> Result<(), String> {
    terminal::enable_raw_mode().map_err(|e| e.to_string())?;
    let support = probe(Duration::from_secs(2)).ok().flatten();
    let Some(why) = refusal(support) else {
        return Ok(());
    };
    let _ = terminal::disable_raw_mode();
    Err(why)
}

/// Take the alternate screen, with key releases reported, and the mouse
/// if the configuration asks for it.
fn enter(out: &mut io::Stdout, mouse: bool) -> io::Result<()> {
    let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS;
    execute!(
        out,
        terminal::EnterAlternateScreen,
        PushKeyboardEnhancementFlags(flags)
    )?;
    if mouse {
        execute!(out, event::EnableMouseCapture)?;
    }
    Ok(())
}

/// Give the terminal back as it was. A failure here has nowhere to be
/// shown, so it is ignored.
fn leave(mouse: bool) {
    if mouse {
        let _ = execute!(io::stdout(), event::DisableMouseCapture);
    }
    let _ = execute!(
        io::stdout(),
        PopKeyboardEnhancementFlags,
        terminal::LeaveAlternateScreen
    );
    let _ = terminal::disable_raw_mode();
}

/// The memex at work in a terminal: what it reads keys with, where the
/// last screen put what can be clicked, and where in the journal it last
/// reasoned.
struct Session<'a> {
    app: App,
    config: &'a Config,
    hits: ui::Hits,
    reasoned_at: Option<(u64, usize)>,
}

impl<'a> Session<'a> {
    fn new(app: App, config: &'a Config) -> Session<'a> {
        let hits = ui::Hits::default();
        Session {
            app,
            config,
            hits,
            reasoned_at: None,
        }
    }

    /// Reason, draw and wait for an event, until the user quits; then
    /// write what is pending.
    fn run(&mut self, term: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        while !self.app.quit {
            self.reason_if_moved();
            term.draw(|f| self.hits = ui::draw_hits(f, &self.app))?;
            self.wait()?;
        }
        self.app.flush();
        Ok(())
    }

    /// Reason again when the journal has moved: constraints and derived
    /// values are computed from it, never stored in it.
    fn reason_if_moved(&mut self) {
        if self.reasoned_at == Some(self.place()) {
            return;
        }
        if let Err(e) = self.app.evaluate() {
            self.app.message = e;
        }
        self.reasoned_at = Some(self.place());
    }

    /// Where the journal is: the tip, and how many entries it has.
    fn place(&self) -> (u64, usize) {
        (self.app.tip, self.app.journal().entries().len())
    }

    /// Take the next event, or after a second without one, write what is
    /// pending.
    fn wait(&mut self) -> io::Result<()> {
        if event::poll(Duration::from_secs(1))? {
            self.take(event::read()?);
        } else if self.app.pending() {
            self.app.flush();
        }
        Ok(())
    }

    /// A key to the app, or a click on what was drawn to be clicked.
    fn take(&mut self, e: event::Event) {
        match e {
            event::Event::Key(k) => {
                if let Some(ev) = map_key(k, self.config) {
                    self.app.handle(ev);
                }
            }
            event::Event::Mouse(m) => {
                if let Some(c) = clicked(m, &self.hits) {
                    self.app.click(&c);
                }
            }
            _ => {}
        }
    }
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

    /// What a key event maps to, under a configuration.
    fn mapped(code: KeyCode, kind: KeyEventKind, c: &Config) -> Option<Event> {
        let k = event::KeyEvent::new_with_kind(code, event::KeyModifiers::NONE, kind);
        map_key(k, c)
    }

    /// A configuration that moves leap forward to F12 and command to
    /// RightAlt, and binds Insert.
    fn remapped() -> Config {
        let text = "[quasimodes]\nleap_forward = [\"F12\"]\ncommand = [\"RightAlt\"]\n[keys]\nInsert = \"undo\"\n";
        Config::parse(text).unwrap()
    }

    // Each of these catches a quasimode key whose release is lost, a key
    // mismapped, or a remapping that the reader ignores.

    #[test]
    fn right_alt_released_ends_leap_forward() {
        let alt = KeyCode::Modifier(ModifierKeyCode::RightAlt);
        let got = mapped(alt, KeyEventKind::Release, &Config::default());
        assert_eq!(got, Some(Event::up(Key::Quasi(Quasi::LeapForward))));
    }

    #[test]
    fn left_control_pressed_starts_command() {
        let control = KeyCode::Modifier(ModifierKeyCode::LeftControl);
        let got = mapped(control, KeyEventKind::Press, &Config::default());
        assert_eq!(got, Some(Event::down(Key::Quasi(Quasi::Command))));
    }

    #[test]
    fn a_letter_repeated_is_a_letter_pressed() {
        let got = mapped(KeyCode::Char('d'), KeyEventKind::Repeat, &Config::default());
        assert_eq!(got, Some(Event::down(Key::Char('d'))));
    }

    #[test]
    fn caps_lock_with_no_job_is_dropped() {
        let got = mapped(KeyCode::CapsLock, KeyEventKind::Press, &Config::default());
        assert_eq!(got, None);
    }

    #[test]
    fn a_quasimode_moved_to_a_modifier_follows_the_configuration() {
        let alt = KeyCode::Modifier(ModifierKeyCode::RightAlt);
        let got = mapped(alt, KeyEventKind::Press, &remapped());
        assert_eq!(got, Some(Event::down(Key::Quasi(Quasi::Command))));
    }

    #[test]
    fn a_quasimode_moved_to_a_function_key_reports_its_release() {
        let got = mapped(KeyCode::F(12), KeyEventKind::Release, &remapped());
        assert_eq!(got, Some(Event::up(Key::Quasi(Quasi::LeapForward))));
    }

    #[test]
    fn a_special_key_with_a_binding_reaches_the_app() {
        let got = mapped(KeyCode::Insert, KeyEventKind::Press, &remapped());
        assert_eq!(got, Some(Event::down(Key::Special(Special::Insert))));
    }

    #[test]
    fn a_modifier_whose_quasimode_moved_away_is_dropped() {
        let control = KeyCode::Modifier(ModifierKeyCode::LeftControl);
        let got = mapped(control, KeyEventKind::Press, &remapped());
        assert_eq!(got, None, "LeftControl lost its job when command moved");
    }
}
