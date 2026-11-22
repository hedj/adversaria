//! The user's configuration: which keys are the quasimodes, and which keys
//! run which commands (docs/guide.md, Configuration).
//!
//! A TOML file, by default `$XDG_CONFIG_HOME/adversaria/config.toml`, else
//! `~/.config/adversaria/config.toml`; `--config FILE` or
//! `$ADVERSARIA_CONFIG` names another. A missing file means the defaults.
//! A file that does not parse, or names a key or command the memex does not
//! know, or gives one key two jobs, is refused with the reason: the memex
//! does not start on a configuration it would have to guess at.

use crate::actions::COMMANDS;
use crate::app::{Key, Quasi, Special};
use crate::plumb::{Rule, Test};
use crate::session::Keys;
use std::path::{Path, PathBuf};

/// The defaults, as a file: what `memex --print-config` writes, and
/// docs/config.example.toml (a test keeps them the same).
pub const DEFAULT_TOML: &str = r#"# Adversaria: key configuration. Copy to ~/.config/adversaria/config.toml
# and edit. Every key here is named as below; see docs/guide.md.
#
# Quasimode keys: LeftAlt RightAlt LeftControl RightControl LeftSuper
#   RightSuper LeftHyper RightHyper LeftMeta RightMeta IsoLevel3Shift
#   IsoLevel5Shift, F1 to F35, Insert PageUp PageDown Menu Pause ScrollLock
#   PrintScreen CapsLock NumLock.
# Bound keys: F1 to F35, Insert PageUp PageDown Menu Pause ScrollLock
#   PrintScreen CapsLock NumLock, and ShiftUp ShiftDown ShiftLeft ShiftRight
#   ShiftHome ShiftEnd (an arrow, Home or End with Shift held).
# Reserved, for editing, and refused: Enter Tab Backspace Delete Esc, the
#   arrows, Home End, Shift, and any printable character.

# Each quasimode is held (or tapped, to latch it) on any of its keys. A
# quasimode named here replaces its default keys; the others keep theirs.
[quasimodes]
leap_forward = ["RightAlt", "IsoLevel3Shift"]
leap_back = ["LeftAlt"]
command = ["LeftControl"]
suppose = ["LeftSuper", "RightSuper"]

# A key runs a command line, as typed in the Command quasimode; "palette"
# opens the command palette; "" unbinds the key. A key named here replaces
# its default; the others keep theirs.
[keys]
F1 = "palette"
F3 = "adopt"
F4 = "retract"
F6 = "switch"
F7 = "go back"
F8 = "go forward"
F10 = "quit"
ShiftUp = "go previous"
ShiftDown = "go next"
ShiftLeft = "go parent"
ShiftRight = "go child"
ShiftHome = "go first"
ShiftEnd = "go last"

# The mouse. A click on an item moves the cursor there, a click on a hint
# such as "F3 adopts" runs its command, and a click on a suggestion accepts
# it; a drag selects text, and a right-click offers the commands that can
# act on it (plumbing, below). `memex --no-mouse` or `memex --mouse`
# overrides this for one run.
[mouse]
enabled = true

# Plumbing: a right-click offers the commands whose rules match the text
# selected (or, with none, the word under the pointer). Your rules come
# before the built-in ones (docs/guide.md § 2.3). Each has a name, the
# menu's words; a pattern, a regular expression the whole text must match;
# a command line, as typed in Command, with $0 for the text and $1 to $9
# for the pattern's groups; and, if it needs one, a test: "file" (a file
# that exists) or "item" (the text names an item). For example:
#
# [[plumb]]
# name = "show Wikidata's statements about $0"
# match = 'Q\d+'
# run = "query wd('$0', P, V)"
"#;

/// A key a quasimode may be held on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuasiKey {
    /// A modifier key, by its name (crate::gui::input names each by its place).
    Modifier(&'static str),
    /// Any key a binding may also use.
    Key(Key),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub quasimodes: Vec<(Quasi, Vec<QuasiKey>)>,
    pub bindings: Vec<(Key, String)>,
    /// Whether the memex takes the mouse ([mouse] enabled).
    pub mouse: bool,
    /// The user's plumbing rules ([[plumb]]), in order.
    pub plumb: Vec<Rule>,
}

impl Default for Config {
    fn default() -> Self {
        let nothing = Config::under(vec![], false);
        Config::read(DEFAULT_TOML, nothing).expect("the default configuration parses")
    }
}

const MODIFIERS: [&str; 12] = [
    "LeftAlt",
    "RightAlt",
    "LeftControl",
    "RightControl",
    "LeftSuper",
    "RightSuper",
    "LeftHyper",
    "RightHyper",
    "LeftMeta",
    "RightMeta",
    "IsoLevel3Shift",
    "IsoLevel5Shift",
];

const SPECIALS: [(&str, Special); 15] = [
    ("Insert", Special::Insert),
    ("PageUp", Special::PageUp),
    ("PageDown", Special::PageDown),
    ("Menu", Special::Menu),
    ("Pause", Special::Pause),
    ("ScrollLock", Special::ScrollLock),
    ("PrintScreen", Special::PrintScreen),
    ("CapsLock", Special::CapsLock),
    ("NumLock", Special::NumLock),
    ("ShiftUp", Special::ShiftUp),
    ("ShiftDown", Special::ShiftDown),
    ("ShiftLeft", Special::ShiftLeft),
    ("ShiftRight", Special::ShiftRight),
    ("ShiftHome", Special::ShiftHome),
    ("ShiftEnd", Special::ShiftEnd),
];

const RESERVED: [&str; 14] = [
    "Enter",
    "Tab",
    "BackTab",
    "Backspace",
    "Delete",
    "Esc",
    "Up",
    "Down",
    "Left",
    "Right",
    "Home",
    "End",
    "LeftShift",
    "RightShift",
];

/// A key that a binding may use, by name.
fn bound_key(name: &str) -> Result<Key, String> {
    if let Some(n) = name
        .strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|n| (1..=35).contains(n) && name == format!("F{n}"))
    {
        return Ok(Key::F(n));
    }
    if let Some((_, s)) = SPECIALS.iter().find(|(n, _)| *n == name) {
        return Ok(Key::Special(*s));
    }
    if RESERVED.contains(&name) || name.chars().count() == 1 {
        return Err(format!("{name} is reserved for editing"));
    }
    Err(format!("no key is called {name}"))
}

/// A quasimode's keys as a hint names them: its first key, as it is
/// printed on a keyboard ("Left Ctrl", "AltGr"); a modifier listed for
/// both hands, by its name alone ("Super").
fn spoken(keys: &[QuasiKey]) -> String {
    let names: Vec<String> = keys.iter().map(quasi_key_name).collect();
    let first = names.first().cloned().unwrap_or_default();
    let family = first
        .trim_start_matches("Left ")
        .trim_start_matches("Right ");
    let both = ["Left ", "Right "].map(|side| names.contains(&format!("{side}{family}")));
    if both == [true, true] {
        family.to_string()
    } else {
        first
    }
}

/// A quasimode's key, as it is printed on a keyboard.
fn quasi_key_name(k: &QuasiKey) -> String {
    match k {
        QuasiKey::Modifier(m) => printed(m),
        QuasiKey::Key(k) => key_name(*k),
    }
}

/// A modifier's name in the configuration, as a keyboard prints it.
fn printed(modifier: &str) -> String {
    let (side, family) = match modifier.strip_prefix("Left") {
        Some(rest) => ("Left ", rest),
        None => modifier
            .strip_prefix("Right")
            .map_or(("", modifier), |r| ("Right ", r)),
    };
    let family = match family {
        "Control" => "Ctrl",
        "IsoLevel3Shift" => "AltGr",
        "IsoLevel5Shift" => "Level 5 Shift",
        other => other,
    };
    format!("{side}{family}")
}

/// A key's name, as the configuration writes it.
pub fn key_name(k: Key) -> String {
    match k {
        Key::F(n) => format!("F{n}"),
        Key::Special(s) => SPECIALS
            .iter()
            .find(|(_, x)| *x == s)
            .map(|(n, _)| n.to_string())
            .unwrap_or_default(),
        other => format!("{other:?}"),
    }
}

fn quasi_key(name: &str) -> Result<QuasiKey, String> {
    if let Some(m) = MODIFIERS.iter().find(|m| **m == name) {
        return Ok(QuasiKey::Modifier(m));
    }
    // Shift may be let go before the arrow, whose release would then be
    // the plain arrow's, and the quasimode would stay held.
    if name.starts_with("Shift") {
        return Err(format!("{name} is two keys, and a quasimode holds one"));
    }
    bound_key(name).map(QuasiKey::Key)
}

/// A quasimode's keys, as written: one name or a list of names.
fn key_list(keys: &toml::Value) -> Result<Vec<QuasiKey>, String> {
    let named: Result<Vec<QuasiKey>, String> =
        key_names(keys)?.into_iter().map(quasi_key).collect();
    let keys = named?;
    if keys.is_empty() {
        return Err("a quasimode needs at least one key".into());
    }
    Ok(keys)
}

fn key_names(keys: &toml::Value) -> Result<Vec<&str>, String> {
    match keys {
        toml::Value::String(s) => Ok(vec![s.as_str()]),
        toml::Value::Array(a) => a.iter().map(key_text).collect(),
        _ => Err("a key name, or a list of them".into()),
    }
}

fn key_text(v: &toml::Value) -> Result<&str, String> {
    v.as_str().ok_or("each key is a string".into())
}

/// A bound command line must start with a command the memex has, or be
/// "palette".
fn check_command(place: &str, cmd: &str) -> Result<(), String> {
    let first = cmd.split_whitespace().next().unwrap_or("");
    if first != "palette" && !COMMANDS.iter().any(|c| c.name == first) {
        return Err(format!(
            "{place}: no command “{first}”; the commands are listed in docs/guide.md"
        ));
    }
    Ok(())
}

/// The plumbing rules of a text of [[plumb]] tables alone, as the
/// defaults are written (crate::plumb).
pub(crate) fn rules_of(text: &str) -> Result<Vec<Rule>, String> {
    let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    table.get("plumb").map_or(Ok(vec![]), plumb_rules)
}

/// The rules of a [[plumb]] list, in order, or what is wrong with one.
fn plumb_rules(value: &toml::Value) -> Result<Vec<Rule>, String> {
    let rules = value
        .as_array()
        .ok_or("plumb must be a list of tables, each as [[plumb]]")?;
    let rule = |(n, r): (usize, &toml::Value)| {
        let t = r.as_table().ok_or("each rule is a table")?;
        plumb_rule(t).map_err(|e| format!("[[plumb]] rule {}: {e}", n + 1))
    };
    rules.iter().enumerate().map(rule).collect()
}

/// A plumbing rule, from its table: name, match and run, and if, if
/// given; nothing else.
fn plumb_rule(t: &toml::Table) -> Result<Rule, String> {
    if let Some(k) = t
        .keys()
        .find(|k| !["name", "match", "run", "if"].contains(&k.as_str()))
    {
        return Err(format!("no setting {k}: only name, match, run and if"));
    }
    let text = |k: &str| match t.get(k) {
        Some(toml::Value::String(s)) if !s.is_empty() => Ok(s.clone()),
        _ => Err(format!("{k}: text, in quotes")),
    };
    let test = match t.get("if") {
        None => None,
        Some(v) => Some(
            v.as_str()
                .and_then(Test::named)
                .ok_or("if: \"file\" or \"item\"")?,
        ),
    };
    let rule = Rule {
        name: text("name")?,
        pattern: text("match")?,
        test,
        run: text("run")?,
    };
    crate::plumb::whole(&rule.pattern).map_err(|e| format!("match: {e}"))?;
    check_command("run", &rule.run)?;
    Ok(rule)
}

/// Why a name in [quasimodes] is refused: Scope's, that it was removed
/// and what does its work now.
fn no_quasimode(name: &str) -> String {
    match name {
        "scope" => "[quasimodes] has no scope: it was removed; `delete word` deletes the word at the cursor, and Right Ctrl is free".into(),
        _ => format!("[quasimodes] has no {name}: the quasimodes are leap_forward, leap_back, command, suppose"),
    }
}

fn quasi_named(name: &str) -> Option<Quasi> {
    match name {
        "leap_forward" => Some(Quasi::LeapForward),
        "leap_back" => Some(Quasi::LeapBack),
        "command" => Some(Quasi::Command),
        "suppose" => Some(Quasi::Suppose),
        _ => None,
    }
}

impl Config {
    /// The plumber of its rules, before the defaults (crate::plumb).
    pub fn plumber(&self) -> Result<crate::plumb::Plumber, String> {
        crate::plumb::Plumber::new(&self.plumb)
    }

    /// The keys it binds, and each quasimode's key by name, for a
    /// session.
    pub fn keys(&self) -> Keys {
        let spoken = self.quasimodes.iter().map(|(q, ks)| (*q, spoken(ks)));
        Keys::new(self.bindings.clone(), spoken.collect())
    }

    /// Read a configuration over the defaults: a quasimode or key named
    /// replaces its default, and the rest keep theirs. Every error names what
    /// is wrong.
    pub fn parse(text: &str) -> Result<Config, String> {
        let d = Config::default();
        Config::read(text, Config::under(d.bindings, d.mouse))
    }

    /// Read a file's text over a configuration's bindings and mouse:
    /// each section, then what every configuration must have.
    fn read(text: &str, under: Config) -> Result<Config, String> {
        let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut config = under;
        for (section, value) in &table {
            config.read_section(section, value)?;
        }
        config.complete()?;
        Ok(config)
    }

    /// What a file is read over: these bindings and mouse, and no
    /// quasimodes or rules yet.
    fn under(bindings: Vec<(Key, String)>, mouse: bool) -> Config {
        Config {
            quasimodes: vec![],
            bindings,
            mouse,
            plumb: vec![],
        }
    }

    fn read_section(&mut self, section: &str, value: &toml::Value) -> Result<(), String> {
        if section == "plumb" {
            return self.read_plumb(value);
        }
        let t = value
            .as_table()
            .ok_or_else(|| format!("{section} must be a table, as [{section}]"))?;
        match section {
            "quasimodes" => self.read_quasimodes(t),
            "keys" => self.read_keys(t),
            "mouse" => self.read_mouse(t),
            other => Err(format!(
                "no section [{other}]: the sections are [quasimodes], [keys], [mouse] and [[plumb]]"
            )),
        }
    }

    /// [quasimodes]: each named quasimode's keys replace its defaults.
    fn read_quasimodes(&mut self, t: &toml::Table) -> Result<(), String> {
        for (name, keys) in t {
            let q = quasi_named(name).ok_or_else(|| no_quasimode(name))?;
            let keys = key_list(keys).map_err(|e| format!("[quasimodes] {name}: {e}"))?;
            self.quasimodes.push((q, keys));
        }
        Ok(())
    }

    /// [keys]: each named key's command line replaces its default; ""
    /// unbinds it.
    fn read_keys(&mut self, t: &toml::Table) -> Result<(), String> {
        for (name, cmd) in t {
            let k = bound_key(name).map_err(|e| format!("[keys] {e}"))?;
            let cmd = cmd
                .as_str()
                .ok_or_else(|| format!("[keys] {name}: a command line, in quotes"))?;
            self.bindings.retain(|(b, _)| *b != k);
            if !cmd.is_empty() {
                check_command(&format!("[keys] {name}"), cmd)?;
                self.bindings.push((k, cmd.to_string()));
            }
        }
        Ok(())
    }

    /// [mouse]: enabled, true or false.
    fn read_mouse(&mut self, t: &toml::Table) -> Result<(), String> {
        for (name, v) in t {
            match (name.as_str(), v) {
                ("enabled", toml::Value::Boolean(b)) => self.mouse = *b,
                ("enabled", _) => return Err("[mouse] enabled: true or false".into()),
                _ => return Err(format!("[mouse] has no {name}: only enabled")),
            }
        }
        Ok(())
    }

    /// [[plumb]]: rules, each a table, in order.
    fn read_plumb(&mut self, value: &toml::Value) -> Result<(), String> {
        self.plumb.extend(plumb_rules(value)?);
        Ok(())
    }

    /// What every configuration must have: a key for each quasimode, one
    /// order (so equal settings compare equal), one job per key.
    fn complete(&mut self) -> Result<(), String> {
        self.fill_quasimodes();
        self.order_settings();
        self.check_one_job()
    }

    /// Each quasimode not named takes its default keys, or it could be
    /// reached only by the palette.
    fn fill_quasimodes(&mut self) {
        for q in Quasi::ALL {
            if !self.quasimodes.iter().any(|(x, _)| *x == q) {
                self.quasimodes.push((q, Config::defaults_for(q)));
            }
        }
    }

    fn order_settings(&mut self) {
        self.quasimodes.sort_by_key(|(q, _)| *q as u8);
        self.bindings.sort_by_key(|(k, _)| format!("{k:?}"));
    }

    fn check_one_job(&self) -> Result<(), String> {
        let held = self.quasimodes.iter().flat_map(|(_, ks)| ks).copied();
        let bound = self.bindings.iter().map(|(k, _)| QuasiKey::Key(*k));
        let mut used: Vec<QuasiKey> = Vec::new();
        for k in held.chain(bound) {
            if used.contains(&k) {
                return Err(format!("{k:?} is given two jobs"));
            }
            used.push(k);
        }
        Ok(())
    }

    fn defaults_for(q: Quasi) -> Vec<QuasiKey> {
        let d: toml::Table = toml::from_str(DEFAULT_TOML).unwrap();
        let name = match q {
            Quasi::LeapForward => "leap_forward",
            Quasi::LeapBack => "leap_back",
            Quasi::Command => "command",
            Quasi::Suppose => "suppose",
        };
        d["quasimodes"][name]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| quasi_key(v.as_str().unwrap()).unwrap())
            .collect()
    }

    /// The quasimode a key is held for, if any.
    pub fn quasi_for(&self, k: QuasiKey) -> Option<Quasi> {
        self.quasimodes
            .iter()
            .find(|(_, ks)| ks.contains(&k))
            .map(|(q, _)| *q)
    }

    /// Where the configuration is read from, in order: the given path,
    /// $ADVERSARIA_CONFIG, $XDG_CONFIG_HOME/adversaria/config.toml,
    /// ~/.config/adversaria/config.toml.
    pub fn path(given: Option<&Path>) -> Option<PathBuf> {
        if let Some(p) = given {
            return Some(p.to_path_buf());
        }
        if let Some(p) = std::env::var_os("ADVERSARIA_CONFIG") {
            return Some(PathBuf::from(p));
        }
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("adversaria").join("config.toml"))
    }

    /// Load the configuration: the defaults if the file is missing; an error,
    /// naming the file, if it is there but wrong.
    pub fn load(given: Option<&Path>) -> Result<Config, String> {
        let Some(path) = Config::path(given) else {
            return Ok(Config::default());
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Config::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && given.is_none() => {
                Ok(Config::default())
            }
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quasimodes_key_is_named_as_a_keyboard_prints_it() {
        // Catches a hint naming a key as the file writes it
        // ("LeftControl"), or "Left Super" for a key that either Super
        // works for.
        let defaults = Config::default().keys();
        assert_eq!(defaults.quasi_key(Quasi::Command), "Left Ctrl");
        assert_eq!(defaults.quasi_key(Quasi::LeapForward), "Right Alt");
        assert_eq!(defaults.quasi_key(Quasi::Suppose), "Super");
        let toml = "[quasimodes]\nsuppose = [\"RightAlt\", \"IsoLevel3Shift\"]\nleap_forward = [\"RightControl\"]\nleap_back = [\"Menu\"]\n";
        let moved = Config::parse(toml).unwrap().keys();
        assert_eq!(moved.quasi_key(Quasi::Suppose), "Right Alt");
        assert_eq!(moved.quasi_key(Quasi::LeapForward), "Right Ctrl");
        assert_eq!(moved.quasi_key(Quasi::LeapBack), "Menu");
        let altgr = Config::parse(
            "[quasimodes]\nleap_forward = [\"IsoLevel3Shift\"]\nsuppose = [\"RightSuper\"]\n",
        );
        let altgr = altgr.unwrap().keys();
        assert_eq!(altgr.quasi_key(Quasi::LeapForward), "AltGr");
        assert_eq!(altgr.quasi_key(Quasi::Suppose), "Right Super");
    }

    /// The quasimode a key is held for, by the key's name.
    fn quasi(c: &Config, key: &str) -> Option<Quasi> {
        c.quasi_for(quasi_key(key).unwrap())
    }

    /// The command line a key is bound to, by the key's name.
    fn bound<'a>(c: &'a Config, key: &str) -> Option<&'a str> {
        let k = bound_key(key).unwrap();
        c.bindings
            .iter()
            .find(|(b, _)| *b == k)
            .map(|(_, s)| s.as_str())
    }

    #[test]
    fn defaults() {
        // Catches defaults that differ from what the guide and the tests use.
        let c = Config::default();
        assert_eq!(quasi(&c, "RightAlt"), Some(Quasi::LeapForward));
        assert_eq!(quasi(&c, "LeftControl"), Some(Quasi::Command));
        assert_eq!(bound(&c, "F10"), Some("quit"));
    }

    #[test]
    fn thirteen_default_bindings() {
        // Catches a default binding added or lost unnoticed.
        assert_eq!(Config::default().bindings.len(), 13);
    }

    #[test]
    fn the_example_file_is_the_defaults() {
        // Catches docs/config.example.toml drifting from the defaults.
        let example = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/config.example.toml"
        ))
        .unwrap();
        assert_eq!(example, DEFAULT_TOML);
    }

    #[test]
    fn a_function_key_is_named_one_way() {
        // Catches "F+1" and "F01" read as F1, a name the file would then
        // write back otherwise (the code review, D-B7).
        for name in ["F+1", "F01", "F0", "F36"] {
            assert!(bound_key(name).is_err(), "{name}");
        }
    }

    #[test]
    fn key_names_read_back() {
        // Catches a key shown to the user under a name the file would refuse.
        for name in ["F1", "F35", "Insert", "NumLock", "ShiftUp", "ShiftEnd"] {
            let k = bound_key(name).unwrap();
            assert_eq!(key_name(k), name);
        }
    }

    #[test]
    fn a_remap_replaces_only_what_it_names() {
        // Catches a configured key ignored, a default kept beside its
        // replacement, or the others losing their defaults.
        let c = Config::parse(
            "[quasimodes]\nleap_forward = \"F12\"\n[keys]\nInsert = \"pane suggestions\"\n",
        )
        .unwrap();
        assert_eq!(quasi(&c, "F12"), Some(Quasi::LeapForward));
        assert_eq!(quasi(&c, "RightAlt"), None, "replaced, not added");
        assert_eq!(
            quasi(&c, "LeftAlt"),
            Some(Quasi::LeapBack),
            "the others keep their defaults"
        );
        assert_eq!(bound(&c, "Insert"), Some("pane suggestions"));
    }

    #[test]
    fn a_shifted_key_is_bound_but_holds_no_quasimode() {
        // Catches a quasimode held on Shift+Up: Shift let go first, the
        // release would be plain Up's, and the quasimode would stay held.
        let err = Config::parse("[quasimodes]\nleap_back = \"ShiftUp\"\n").unwrap_err();
        assert!(err.contains("ShiftUp is two keys"), "{err}");
        let c = Config::parse("[keys]\nShiftUp = \"back\"\n").unwrap();
        assert_eq!(bound(&c, "ShiftUp"), Some("back"));
    }

    #[test]
    fn a_binding_replaces_or_unbinds_its_default() {
        // Catches "" not unbinding, an unbound key not free for a
        // quasimode, or a binding added beside its default.
        let c = Config::parse("[keys]\nF1 = \"\"\n[quasimodes]\nleap_back = \"F1\"\n").unwrap();
        assert_eq!(bound(&c, "F1"), None);
        assert_eq!(quasi(&c, "F1"), Some(Quasi::LeapBack));
        let c = Config::parse("[keys]\nF10 = \"export out.md\"\n").unwrap();
        assert_eq!(bound(&c, "F10"), Some("export out.md"), "replaced");
    }

    #[test]
    fn plumbing_rules_are_read_in_order() {
        // Catches the example in the defaults' comments not a rule that
        // parses, the order lost, or "if" not read.
        let example: String = DEFAULT_TOML
            .lines()
            .skip_while(|l| !l.starts_with("# [[plumb]]"))
            .map(|l| l.trim_start_matches("# "))
            .collect::<Vec<_>>()
            .join("\n");
        let two = format!(
            "{example}\n[[plumb]]\nname = \"open $0\"\nmatch = '\\S+\\.md'\nrun = \"import document $0\"\nif = \"file\"\n"
        );
        let c = Config::parse(&two).unwrap();
        let names: Vec<&str> = c.plumb.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["show Wikidata's statements about $0", "open $0"]);
        assert_eq!(c.plumb[1].test, Some(Test::File));
        assert!(Config::default().plumb.is_empty());
    }

    #[test]
    fn a_plumbing_rule_it_would_guess_at_is_refused() {
        // Catches a pattern that is no regular expression, a command that
        // does not exist, an unknown test or setting, or a missing part,
        // taken without a word.
        let rule = |body: &str| Config::parse(&format!("[[plumb]]\n{body}\n")).unwrap_err();
        let ok = "name = \"n\"\nmatch = 'x'\nrun = \"go $0\"";
        assert!(rule(&ok.replace("'x'", "'('")).contains("match"));
        assert!(rule(&ok.replace("go $0", "fly $0")).contains("no command “fly”"));
        assert!(rule(&format!("{ok}\nif = \"dir\"")).contains("if"));
        assert!(rule(&format!("{ok}\nwhen = 1")).contains("no setting when"));
        assert!(rule("name = \"n\"\nmatch = 'x'").contains("run"));
        assert!(
            Config::parse("plumb = 1")
                .unwrap_err()
                .contains("list of tables")
        );
    }

    #[test]
    fn an_empty_file_is_the_defaults() {
        // Catches an empty file read as no settings at all.
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn the_mouse_is_on_unless_turned_off() {
        // Catches the mouse off by default, off for a file that does not
        // name it, or not turned off by the file.
        assert!(Config::default().mouse);
        assert!(Config::parse("[keys]\nF1 = \"help\"\n").unwrap().mouse);
        assert!(!Config::parse("[mouse]\nenabled = false\n").unwrap().mouse);
    }

    #[test]
    fn refusals_say_why() {
        // Catches a configuration accepted that the memex would have to guess at.
        let cases = [
            ("[quasimodes]\nleap = \"F1\"\n", "no leap"),
            ("[quasimodes]\ncommand = \"Enter\"\n", "reserved"),
            ("[quasimodes]\ncommand = \"a\"\n", "reserved"),
            ("[quasimodes]\ncommand = \"LeftShift\"\n", "reserved"),
            (
                "[quasimodes]\ncommand = \"Hyperspace\"\n",
                "no key is called Hyperspace",
            ),
            ("[keys]\nF2 = \"frobnicate\"\n", "no command “frobnicate”"),
            ("[keys]\nF2 = 3\n", "a command line"),
            ("[quasimodes]\nleap_back = \"F1\"\n", "two jobs"),
            ("[quasimodes]\nsuppose = \"LeftAlt\"\n", "two jobs"),
            (
                "[quasimodes]\nscope = \"Menu\"\n",
                "`delete word` deletes the word",
            ),
            ("[colours]\nx = 1\n", "no section [colours]"),
            ("[keys\n", "TOML parse error"),
            ("[quasimodes]\nleap_back = []\n", "at least one key"),
            ("[mouse]\nenabled = \"yes\"\n", "true or false"),
            ("[mouse]\nwheel = true\n", "has no wheel"),
        ];
        for (text, why) in cases {
            let e = Config::parse(text).unwrap_err();
            assert!(e.contains(why), "{text:?} gave {e:?}");
        }
    }

    #[test]
    fn a_fault_in_a_quasimodes_keys_names_the_quasimode() {
        // Catches a fault reported without the quasimode it is in, or a key
        // that is not a string accepted.
        let said = Config::parse("[quasimodes]\ncommand = [3]\n").unwrap_err();
        assert_eq!(said, "[quasimodes] command: each key is a string");
    }
}
