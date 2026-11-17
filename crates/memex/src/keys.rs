//! Keys and clicks: what each does in each mode, and leaping, which finds
//! items by their text (docs/design.md § Quasimodes).

use crate::actions::dispatch::{complete_command, in_quotes};
use crate::app::{Active, App, Click, Event, Key, Mode, PaneKind, Press, Quasi, ScopeLevel};
use crate::outline::TOP;
use crate::reason;
use format::terms::Term;
use serde_json::json;

impl App {
    pub fn handle(&mut self, ev: Event) {
        let mode = std::mem::replace(&mut self.session.mode, Mode::Normal);
        self.session.mode = match mode {
            Mode::Normal => self.normal(ev),
            Mode::Quasi(a) => self.quasi(a, ev),
            Mode::ConfirmRetract { target, dependents } => self.confirm(target, dependents, ev),
            Mode::ConfirmSuppose { text, readings, at } => {
                self.confirm_suppose(text, readings, at, ev)
            }
        };
    }

    /// A click, from the mouse. It acts only when no quasimode or question
    /// is open, and does what the keyboard would: moving the cursor is a
    /// view change like an arrow key; a command is run as a bound key runs
    /// it, keeping any confirmation it asks for.
    pub fn click(&mut self, c: &Click) {
        if self.session.mode != Mode::Normal {
            return;
        }
        match c {
            Click::Item { pane, id } => {
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
            Click::Command(line) => {
                self.flush();
                self.session.message.clear();
                self.run_simple(line);
            }
        }
    }

    fn normal(&mut self, ev: Event) -> Mode {
        if ev.press == Press::Up {
            return Mode::Normal;
        }
        if let Key::Quasi(q) = ev.key {
            return self.enter_quasi(q);
        }
        if let Some(line) = self.session.keys.command_for(ev.key) {
            return self.run_bound(&line);
        }
        if self.session.view.focused().kind == PaneKind::Links {
            let switch = self.session.keys.key_for("switch");
            self.session.message = format!("this pane shows links; {switch} to edit");
            return Mode::Normal;
        }
        self.session.message.clear();
        self.edit_key(ev.key);
        Mode::Normal
    }

    /// Hold a quasimode's key: the Scope quasimode starts at a word.
    fn enter_quasi(&mut self, q: Quasi) -> Mode {
        self.flush();
        if q == Quasi::Scope {
            self.session.view.scope = ScopeLevel::Word;
        }
        Mode::Quasi(Active::held(q))
    }

    /// Run the command line a key is bound to, keeping what it asks for,
    /// such as a confirmation; "palette" opens the command palette.
    fn run_bound(&mut self, line: &str) -> Mode {
        self.flush();
        if line == "palette" {
            return Mode::Quasi(Active::palette());
        }
        self.run_simple(line);
        std::mem::replace(&mut self.session.mode, Mode::Normal)
    }

    /// A key that edits the outline or moves within it.
    fn edit_key(&mut self, key: Key) {
        match key {
            Key::Char(c) => self.type_char(c),
            Key::Backspace => self.erase(true),
            Key::Delete => self.erase(false),
            Key::Enter => self.enter_item(),
            Key::Tab => self.run_simple("indent"),
            Key::BackTab => self.run_simple("outdent"),
            Key::Up | Key::Down => self.move_line(key == Key::Up),
            Key::Left | Key::Right | Key::Home | Key::End => self.move_in_text(key),
            _ => {}
        }
    }

    /// Enter: a new item after the cursor's.
    fn enter_item(&mut self) {
        self.flush();
        let made = self.new_item_after(self.cursor());
        self.session.report(made);
    }

    /// Up or Down: the cursor to the item shown above or below, or again
    /// to the first or last, its text cursor at the end; from no item, to
    /// the first.
    fn move_line(&mut self, up: bool) {
        self.flush_text();
        let rows = self.visible();
        let at = rows.iter().position(|(id, _)| *id == self.cursor());
        let to = match at {
            Some(i) if up => i.saturating_sub(1),
            Some(i) => (i + 1).min(rows.len().saturating_sub(1)),
            None => 0,
        };
        if let Some((id, _)) = rows.get(to) {
            self.set_cursor(*id);
            self.session.view_dirty = true;
        }
    }

    /// Left, Right, Home or End: the text cursor within its item.
    fn move_in_text(&mut self, key: Key) {
        let len = self.text_of(self.cursor()).len();
        let view = &mut self.session.view;
        view.pos = text_pos(view.pos, key, len);
        self.session.view_dirty = true;
    }

    fn type_char(&mut self, c: char) {
        if self.cursor() == TOP
            && let Err(m) = self.new_item_after(TOP)
        {
            self.session.message = m;
            return;
        }
        let item = self.cursor();
        let text = self.text_of(item);
        self.session.type_into(item, text, c);
    }

    fn erase(&mut self, before: bool) {
        let item = self.cursor();
        if item == TOP {
            return;
        }
        let text = self.text_of(item);
        self.session.erase_in(item, text, before);
    }

    pub(crate) fn new_item_after(&mut self, id: u64) -> Result<(), String> {
        let (under, after) = if id == TOP {
            (TOP, None)
        } else {
            (self.notebook.outline.parent(id).unwrap_or(TOP), Some(id))
        };
        let e = self.write_here(
            "edit",
            json!({"op": "create", "under": under, "after": after, "add": "", "spans": []}),
        )?;
        self.set_cursor(e.seq);
        self.write_view("move", json!({}));
        Ok(())
    }

    fn quasi(&mut self, mut a: Active, ev: Event) -> Mode {
        // The quasimode's own key: release ends a held quasimode; a tap latches.
        if ev.key == Key::Quasi(a.q) {
            match ev.press {
                Press::Up if !a.used && !a.latched => {
                    a.latched = true;
                    return Mode::Quasi(a);
                }
                Press::Up if a.latched => return Mode::Quasi(a),
                Press::Up => return self.finish(a),
                Press::Down if a.latched => {
                    // A second tap ends a latched quasimode; on an empty Leap it repeats the last one.
                    return self.finish(a);
                }
                Press::Down => return Mode::Quasi(a),
            }
        }
        if ev.press == Press::Up {
            return Mode::Quasi(a);
        }
        a.used = true;
        match ev.key {
            Key::Char('?')
                if a.q != Quasi::Suppose && !(a.q == Quasi::Command && in_quotes(&a.input)) =>
            {
                a.listing = !a.listing
            }
            Key::Char(' ') if a.q == Quasi::Scope => {
                self.session.view.scope = match self.session.view.scope {
                    ScopeLevel::Word => ScopeLevel::Statement,
                    _ => ScopeLevel::Branch,
                };
            }
            Key::Char(c) if a.q != Quasi::Scope => a.input.push(c),
            Key::Backspace => {
                // Erase a character; with none left, Backspace leaves a
                // latched quasimode without acting.
                let erased = a.input.pop().is_some();
                if !erased && a.latched {
                    self.session.message.clear();
                    return Mode::Normal;
                }
            }
            Key::Escape => match a.q {
                Quasi::LeapForward | Quasi::LeapBack => {
                    a.input = self.leap_complete(&a.input, a.q == Quasi::LeapForward)
                }
                Quasi::Command => a.input = complete_command(&a.input),
                _ => {}
            },
            Key::Enter => return self.finish(a),
            _ => {}
        }
        Mode::Quasi(a)
    }

    /// A statement with no reading, or one believed already: a world of
    /// that name, with nothing supposed in it (guide § 3.2), and say which.
    fn suppose_named(&mut self, text: &str) {
        self.session.message = self.nothing_to_suppose(text);
        let written = self.write_here("suppose", json!({"name": text}));
        self.session.report(written);
    }

    /// What a supposition with no reading says: that it is believed
    /// already, or that it has no reading.
    fn nothing_to_suppose(&self, text: &str) -> String {
        let facts = self.notebook.facts_now().unwrap_or_default();
        match reason::believed_already(&facts, text) {
            true => {
                format!("“{text}” is believed already: a world of that name, with nothing changed")
            }
            false => format!("no reading of “{text}”: a world of that name, with nothing supposed"),
        }
    }

    /// End a quasimode: do what it was for.
    fn finish(&mut self, a: Active) -> Mode {
        match a.q {
            Quasi::LeapForward | Quasi::LeapBack => {
                let forward = a.q == Quasi::LeapForward;
                let pattern = if a.input.is_empty() {
                    match &self.session.last_leap {
                        Some((p, _)) => p.clone(),
                        None => return Mode::Normal,
                    }
                } else {
                    a.input.clone()
                };
                let found = self.leap_matches(&pattern, forward).first().copied();
                match found {
                    Some(id) => {
                        self.unfold_to(id);
                        self.set_cursor(id);
                        self.session.message.clear();
                    }
                    None => self.session.message = format!("no item holds “{pattern}”"),
                }
                self.session.last_leap = Some((pattern.clone(), forward));
                self.write_view(
                    "leap",
                    json!({"pattern": pattern, "forward": forward, "found": found}),
                );
            }
            Quasi::Scope => self.write_view("scope", json!({})),
            Quasi::Suppose => {
                let text = a.input.trim().to_string();
                if text.is_empty() {
                    return Mode::Normal;
                }
                let facts = self.notebook.facts_now().unwrap_or_default();
                let readings = reason::readings(&facts, &text);
                if readings.is_empty() {
                    self.suppose_named(&text);
                } else {
                    return Mode::ConfirmSuppose {
                        text,
                        readings,
                        at: 0,
                    };
                }
            }
            Quasi::Command => return self.run_command(&a.input),
        }
        Mode::Normal
    }

    fn confirm(&mut self, target: u64, dependents: Vec<u64>, ev: Event) -> Mode {
        if ev.press == Press::Up {
            return Mode::ConfirmRetract { target, dependents };
        }
        match ev.key {
            Key::Enter => {
                match self.write_here("retract", json!({"target": target})) {
                    Ok(e) => {
                        self.session.message =
                            format!("entry {target} retracted by entry {}", e.seq)
                    }
                    Err(m) => self.session.message = m,
                }
                Mode::Normal
            }
            Key::Escape => {
                self.session.message = format!("entry {target} kept");
                Mode::Normal
            }
            _ => Mode::ConfirmRetract { target, dependents },
        }
    }

    fn confirm_suppose(&mut self, text: String, readings: Vec<Term>, at: usize, ev: Event) -> Mode {
        if ev.press == Press::Up {
            return Mode::ConfirmSuppose { text, readings, at };
        }
        match ev.key {
            Key::Enter => {
                let fact = readings[at].clone();
                let said = self.suppose(&text, &fact).unwrap_or_else(|m| m);
                self.session.message = said;
                Mode::Normal
            }
            Key::Tab => {
                let at = (at + 1) % readings.len();
                Mode::ConfirmSuppose { text, readings, at }
            }
            Key::Escape => {
                self.session.message = "nothing supposed".into();
                Mode::Normal
            }
            _ => Mode::ConfirmSuppose { text, readings, at },
        }
    }

    /// Leap's matches for a pattern: items whose text holds it, ignoring
    /// case, in order from the cursor (forward or back), wrapping.
    pub fn leap_matches(&self, pattern: &str, forward: bool) -> Vec<u64> {
        if pattern.is_empty() {
            return vec![];
        }
        let outline = &self.notebook.outline;
        let round = outline.round_from(self.cursor(), forward);
        outline.holding(round, pattern)
    }

    /// Recognition: the pattern, and the longest continuation every match
    /// shares, up to a space.
    fn leap_complete(&self, pattern: &str, forward: bool) -> String {
        let matches = self.leap_matches(pattern, forward).into_iter();
        let texts: Vec<String> = matches.map(|id| self.notebook.outline.text(id)).collect();
        recognised(pattern, &texts)
    }

    pub(crate) fn unfold_to(&mut self, id: u64) {
        let mut at = self.notebook.outline.parent(id);
        while let Some(p) = at {
            self.session.view.folded.remove(&p);
            at = self.notebook.outline.parent(p);
        }
    }
}

/// Where a key moves the text cursor, in a text of a length.
fn text_pos(pos: usize, key: Key, len: usize) -> usize {
    match key {
        Key::Left => pos.saturating_sub(1),
        Key::Right => (pos + 1).min(len),
        Key::Home => 0,
        _ => len,
    }
}

/// A pattern, and the longest continuation after it that every text
/// shares, up to a space; each text holds the pattern.
fn recognised(pattern: &str, texts: &[String]) -> String {
    let p = pattern.to_lowercase();
    let conts: Vec<String> = texts.iter().map(|t| continuation(t, &p)).collect();
    format!("{pattern}{}", common_prefix(&conts))
}

/// The longest start that every word shares; none for no words.
fn common_prefix(words: &[String]) -> String {
    let Some(first) = words.first() else {
        return String::new();
    };
    let rest = words.iter().skip(1);
    rest.fold(first.clone(), |prefix, w| shared_start(&prefix, w))
}

/// The longest start that two words share.
fn shared_start(a: &str, b: &str) -> String {
    let pairs = a.chars().zip(b.chars());
    pairs.take_while(|(x, y)| x == y).map(|(x, _)| x).collect()
}

/// What follows a lower-case pattern in a text, ignoring case, up to a
/// space.
fn continuation(text: &str, p: &str) -> String {
    let t = text.to_lowercase();
    let after = &t[t.find(p).unwrap() + p.len()..];
    after.chars().take_while(|c| !c.is_whitespace()).collect()
}
