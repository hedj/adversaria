//! Keys: what each does in each mode, and leaping, which finds
//! items by their text (docs/design.md § Quasimodes).

use crate::actions::dispatch::{complete_command, in_quotes};
use crate::app::{Active, App, Event, Hold, Key, Kind, Mode, PaneKind, Press, Quasi};
use crate::outline::TOP;
use crate::reason;
use crate::session::Answer;
use format::terms::Term;
use serde_json::json;

impl App {
    pub fn handle(&mut self, ev: Event) {
        let mode = std::mem::replace(&mut self.session.mode, Mode::Normal);
        self.session.mode = match mode {
            Mode::Normal => self.normal(ev),
            Mode::Quasi(a) => self.quasi(a, ev),
            Mode::ConfirmRetract { target, dependents } => self.confirm(target, dependents, ev),
            Mode::ConfirmSuppose {
                text,
                readings,
                at,
                english,
            } => self.confirm_suppose((text, english), readings, at, ev),
            Mode::Menu(m) => self.answered(m.answer(ev)),
        };
    }

    /// Do what the menu answers: keep it, close it, or run an offer as a
    /// bound key runs its command, keeping any confirmation it asks for.
    pub(crate) fn answered(&mut self, a: Answer) -> Mode {
        match a {
            Answer::Keep(m) => Mode::Menu(m),
            Answer::Close => Mode::Normal,
            Answer::Run(line) => self.run_bound(&line),
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
        let kind = self.session.view.focused().kind;
        if kind != PaneKind::Outline {
            let switch = self.session.keys.key_for("switch");
            self.session.message = format!("this pane shows {}; {switch} to edit", kind.name());
            return Mode::Normal;
        }
        self.session.message.clear();
        self.edit_key(ev.key);
        Mode::Normal
    }

    /// Hold a quasimode's key: Suppose's opens Command with `suppose ` typed.
    fn enter_quasi(&mut self, q: Quasi) -> Mode {
        self.flush();
        let typed = if q == Quasi::Suppose { "suppose " } else { "" };
        Mode::Quasi(Active::held_on(q, Kind::of(q, self.cursor()), typed))
    }

    /// The window lost the keyboard, so the release of a quasimode's key
    /// goes elsewhere. A quasimode held with nothing else pressed (as Alt
    /// is, for Alt+Tab) is left without acting. One held while keys were
    /// pressed stays open, latched, so nothing typed is lost and nothing
    /// acts until Enter (docs/design.md § Quasimodes, not modes).
    pub fn focus_lost(&mut self) {
        let mode = std::mem::replace(&mut self.session.mode, Mode::Normal);
        self.session.mode = match mode {
            Mode::Quasi(a) => self.unheld(a),
            other => other,
        };
    }

    fn unheld(&mut self, mut a: Active) -> Mode {
        match a.hold {
            Hold::Down => self.leave(a),
            Hold::Used => {
                a.hold = Hold::Latched;
                self.session.message = "the window lost focus: this stays open until Enter".into();
                Mode::Quasi(a)
            }
            Hold::Latched => Mode::Quasi(a),
        }
    }

    /// Leave a quasimode without acting. (A Leap is left only with nothing
    /// typed, so its cursor is back.)
    fn leave(&mut self, _: Active) -> Mode {
        Mode::Normal
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
            Key::Tab if self.link_completions().is_some() => self.complete_link(),
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
        if ev.key == Key::Quasi(a.key) {
            match ev.press {
                Press::Up if a.hold == Hold::Down => {
                    a.hold = Hold::Latched;
                    return Mode::Quasi(a);
                }
                Press::Up if a.latched() => return Mode::Quasi(a),
                Press::Up => return self.finish(a),
                Press::Down if a.latched() => {
                    // A second tap ends a latched quasimode; on an empty Leap it repeats the last one.
                    return self.finish(a);
                }
                Press::Down => return Mode::Quasi(a),
            }
        }
        if ev.press == Press::Up {
            return Mode::Quasi(a);
        }
        if a.hold == Hold::Down {
            a.hold = Hold::Used;
        }
        match ev.key {
            Key::Char('?') if asks_help(&a) => a.listing = !a.listing,
            Key::Char(c) => a.input.push(c),
            Key::Backspace => {
                // Erase a character; with none left, Backspace leaves a
                // latched quasimode without acting.
                let erased = a.input.pop().is_some();
                if !erased && a.latched() {
                    self.session.message.clear();
                    return self.leave(a);
                }
            }
            Key::Tab => match a.q() {
                Quasi::LeapForward | Quasi::LeapBack => a.input = self.leap_complete(&a),
                _ if a.apply_repair() => {}
                _ => a.input = complete_command(&a.input),
            },
            Key::Escape => {
                // Esc cancels, as everywhere: nothing acts, and a Leap puts
                // the cursor back where it began.
                self.leap_return(&a);
                self.session.message.clear();
                return self.leave(a);
            }
            Key::Enter if matches!(a.q(), Quasi::LeapForward | Quasi::LeapBack) => {
                return self.leap_or_create(a);
            }
            Key::Enter => return self.finish(a),
            _ => {}
        }
        self.recheck(&mut a);
        self.leap_preview(&a);
        Mode::Quasi(a)
    }

    /// A statement with no reading, or one believed already: a world of
    /// that name, with nothing supposed in it (guide § 3.2), and say which.
    /// Suppose a text, by the Suppose key or the suppose command alike:
    /// ask which of its readings against the facts is meant; or, if it
    /// has none, open a world of that name with nothing supposed.
    pub(crate) fn suppose_text(&mut self, text: String, english: Option<String>) -> Mode {
        let facts = self.notebook.facts_now().unwrap_or_default();
        let readings = self.suppose_readings(&facts, &text);
        if readings.is_empty() {
            self.suppose_named(&text, english);
            return Mode::Normal;
        }
        Mode::ConfirmSuppose {
            text,
            readings,
            at: 0,
            english,
        }
    }

    fn suppose_named(&mut self, text: &str, english: Option<String>) {
        self.session.message = self.nothing_to_suppose(text);
        let written = self.write_here("suppose", supposed_body(text, english, None));
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

    /// Enter in Leap: leap, or, if nothing holds the pattern, make a new
    /// item holding it after the cursor's, as notational velocity does:
    /// searching and making a note are one gesture. Its text waits to be
    /// read, as typed text does. Releasing Leap on no match only says so,
    /// so a mistyped pattern makes nothing.
    fn leap_or_create(&mut self, mut a: Active) -> Mode {
        // The text searched is the text made, so "foo " finds "foo"
        // rather than make it again.
        let pattern = a.input.trim().to_string();
        a.input.clone_from(&pattern);
        if pattern.is_empty() || self.leap_first(&a).is_some() {
            return self.finish(a);
        }
        if a.near().is_none()
            && let Some(near) = self.near_match(&a, &pattern)
        {
            a.kind.warn(near);
            return Mode::Quasi(a);
        }
        self.leap_return(&a);
        self.flush_text();
        let spans = json!([[0, 0, pattern.chars().count()]]);
        match self.create_after(self.cursor(), &pattern, spans) {
            Ok(item) => {
                self.go_to(item);
                self.will_read(item, item);
                self.session.message = format!("a new item: “{pattern}”");
            }
            Err(why) => self.session.message = why,
        }
        Mode::Normal
    }

    /// End a quasimode: do what it was for.
    fn finish(&mut self, a: Active) -> Mode {
        self.leap_return(&a);
        match a.q() {
            Quasi::LeapForward | Quasi::LeapBack => {
                let forward = a.q() == Quasi::LeapForward;
                let pattern = if a.input.is_empty() {
                    match &self.session.last_leap {
                        Some((p, _)) => p.clone(),
                        None => return Mode::Normal,
                    }
                } else {
                    a.input.clone()
                };
                let found = self.leap_matches(&pattern, forward).first().copied();
                self.session.last_leap = Some((pattern.clone(), forward));
                let leap = json!({"pattern": pattern, "forward": forward, "found": found});
                match found {
                    Some(id) => {
                        self.session.message.clear();
                        self.jump(id, "leap", leap);
                    }
                    None => {
                        self.session.message = format!("no item holds “{pattern}”");
                        self.write_view("leap", leap);
                    }
                }
            }
            Quasi::Command if a.supposes() => {
                let text = a.sentence().unwrap_or("").trim().to_string();
                if text.is_empty() {
                    return Mode::Normal;
                }
                let english = self.confirmed_english(&a, &text);
                return self.suppose_text(text, english);
            }
            Quasi::Command if a.hold == Hold::Used && a.input.chars().count() == 1 => {
                return self.chord(&a.input);
            }
            Quasi::Command | Quasi::Suppose => return self.run_command(&a.input),
        }
        Mode::Normal
    }

    /// Command held for one letter is a Ctrl chord, as every desktop
    /// program reads it: Ctrl+Z undoes, Ctrl+Y and Ctrl+Shift+Z redo, and
    /// Ctrl+F finds, opening Leap; any other is refused, not taken as a
    /// prefix, so a chord of habit does not run a command by surprise.
    fn chord(&mut self, letter: &str) -> Mode {
        match letter {
            "z" => self.run_command("undo"),
            "y" | "Z" => self.run_command("redo"),
            "f" => Mode::Quasi(Active::tapped(Kind::of(Quasi::LeapForward, self.cursor()))),
            _ => {
                let said = "a command held needs two letters or more · Ctrl+Z undoes, Ctrl+Y redoes, Ctrl+F finds";
                self.session.message = said.into();
                Mode::Normal
            }
        }
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

    /// Suppose a reading, and say what was set aside.
    fn suppose_reading(&mut self, text: &str, reading: &Term, english: Option<String>) -> Mode {
        let said = self.suppose(text, reading, english);
        self.session.message = said.unwrap_or_else(|m| m);
        Mode::Normal
    }

    /// "Do you mean?" answered: of a supposition's text and the free
    /// English it was mended from, its readings, and the one shown.
    fn confirm_suppose(
        &mut self,
        (text, english): (String, Option<String>),
        readings: Vec<Term>,
        at: usize,
        ev: Event,
    ) -> Mode {
        let at = match (ev.press, ev.key) {
            (Press::Down, Key::Enter) => {
                return self.suppose_reading(&text, &readings[at], english);
            }
            (Press::Down, Key::Escape) => {
                self.session.message = "nothing supposed".into();
                return Mode::Normal;
            }
            (Press::Down, Key::Tab) => (at + 1) % readings.len(),
            _ => at,
        };
        Mode::ConfirmSuppose {
            text,
            readings,
            at,
            english,
        }
    }

    /// Leap's matches for a pattern: items whose text holds it, ignoring
    /// case, in order from the cursor (forward or back), wrapping.
    pub fn leap_matches(&self, pattern: &str, forward: bool) -> Vec<u64> {
        self.leap_matches_from(self.cursor(), pattern, forward)
    }

    /// The items that hold a pattern, in order round the outline from an
    /// item, after it.
    pub fn leap_matches_from(&self, start: u64, pattern: &str, forward: bool) -> Vec<u64> {
        if pattern.is_empty() {
            return vec![];
        }
        let outline = &self.notebook.outline;
        let round = outline.round_from(start, forward);
        outline.holding(round, pattern)
    }

    /// A Leap's matches of what is typed, counted from where it began.
    pub fn leap_found(&self, a: &Active) -> Vec<u64> {
        let round = self.leap_round(a);
        self.notebook.outline.holding(round, &a.input)
    }

    /// The items in a Leap's direction round the outline from where it
    /// began, after that item.
    pub(crate) fn leap_round(&self, a: &Active) -> Vec<u64> {
        let from = a.leap_from().unwrap_or(self.cursor());
        let forward = a.q() == Quasi::LeapForward;
        self.notebook.outline.round_from(from, forward)
    }

    /// A Leap's first match of what is typed, counted from where it began:
    /// the search stops there, as the cursor needs no more.
    fn leap_first(&self, a: &Active) -> Option<u64> {
        let round = self.leap_round(a);
        self.notebook.outline.first_holding(round, &a.input)
    }

    /// While a Leap is typed, the cursor goes to its first match, as each
    /// letter is typed (Raskin's LEAP); with nothing typed, back to where
    /// it began; with no match, it stays at the last.
    fn leap_preview(&mut self, a: &Active) {
        let Some(from) = a.leap_from() else {
            return;
        };
        let to = match a.input.is_empty() {
            true => Some(from),
            false => self.leap_first(a),
        };
        if let Some(to) = to.filter(|t| self.notebook.outline.contains(*t)) {
            self.unfold_to(to);
            self.set_cursor(to);
        }
    }

    /// Put the cursor back where a Leap began, before it acts.
    fn leap_return(&mut self, a: &Active) {
        let from = a.leap_from().filter(|f| self.notebook.outline.contains(*f));
        if let Some(from) = from {
            self.set_cursor(from);
        }
    }

    /// Recognition: the pattern, and the longest continuation every match
    /// shares, up to a space.
    fn leap_complete(&self, a: &Active) -> String {
        let pattern = &a.input;
        let matches = self.leap_found(a).into_iter();
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

/// Whether `?` asks what may come next: in Command, but not inside
/// quotes; in Suppose, only at the start or after a space, since typed
/// after a word it ends a question of ACE.
fn asks_help(a: &Active) -> bool {
    if let Some(sentence) = a.sentence() {
        return sentence.is_empty() || sentence.ends_with(' ');
    }
    match a.q() {
        Quasi::Command => !in_quotes(&a.input),
        _ => true,
    }
}

/// A supposition entry's body: its name, the free English it was mended
/// from (making a confirmed pair, crate::pairs), and the fact supposed.
pub(crate) fn supposed_body(
    name: &str,
    english: Option<String>,
    fact: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut body = json!({"name": name});
    if let Some(e) = english {
        body["english"] = json!(e);
    }
    if let Some(f) = fact {
        body["facts"] = json!([f]);
    }
    body
}
