//! The user's configuration: which keys are the quasimodes, and which keys
//! run which commands (docs/guide.md, Configuration).
//!
//! A TOML file, by default `$XDG_CONFIG_HOME/adversaria/config.toml`, else
//! `~/.config/adversaria/config.toml`; `--config FILE` or
//! `$ADVERSARIA_CONFIG` names another. A missing file means the defaults.
//! A file that does not parse, or names a key or command the memex does not
//! know, or gives one key two jobs, is refused with the reason: the memex
//! does not start on a configuration it would have to guess at.

use crate::app::{COMMANDS, Key, Quasi, Special};
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
#   PrintScreen CapsLock NumLock.
# Reserved, for editing, and refused: Enter Tab Backspace Delete Esc, the
#   arrows, Home End, Shift, and any printable character.

# Each quasimode is held (or tapped, to latch it) on any of its keys. A
# quasimode named here replaces its default keys; the others keep theirs.
[quasimodes]
leap_forward = ["RightAlt", "IsoLevel3Shift"]
leap_back = ["LeftAlt"]
command = ["LeftControl"]
scope = ["RightControl"]
suppose = ["LeftSuper", "RightSuper"]

# A key runs a command line, as typed in the Command quasimode; "palette"
# opens the command palette; "" unbinds the key. A key named here replaces
# its default; the others keep theirs.
[keys]
F1 = "palette"
F2 = "interrogate"
F3 = "adopt"
F4 = "unadopt"
F6 = "switch"
F10 = "quit"

# The mouse. When enabled, a click on an item moves the cursor there, a click
# on a hint such as "F3 adopts" runs its command, and a click on a suggestion
# accepts it. Off by default: while the memex has the mouse, the terminal's
# own text selection usually needs Shift held. `memex --mouse` or
# `memex --no-mouse` overrides this for one run.
[mouse]
enabled = false
"#;

/// A key a quasimode may be held on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuasiKey {
    /// A modifier key, by crossterm's name for it.
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
}

impl Default for Config {
    fn default() -> Self {
        Config::read(DEFAULT_TOML, vec![]).expect("the default configuration parses")
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

const SPECIALS: [(&str, Special); 9] = [
    ("Insert", Special::Insert),
    ("PageUp", Special::PageUp),
    ("PageDown", Special::PageDown),
    ("Menu", Special::Menu),
    ("Pause", Special::Pause),
    ("ScrollLock", Special::ScrollLock),
    ("PrintScreen", Special::PrintScreen),
    ("CapsLock", Special::CapsLock),
    ("NumLock", Special::NumLock),
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
        .filter(|n| (1..=35).contains(n))
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
    bound_key(name).map(QuasiKey::Key)
}

/// A quasimode's keys, as written: one name or a list of names.
fn key_list(name: &str, keys: &toml::Value) -> Result<Vec<QuasiKey>, String> {
    let ks = key_names(name, keys)?
        .iter()
        .map(|n| quasi_key(n).map_err(|e| format!("[quasimodes] {name}: {e}")))
        .collect::<Result<Vec<QuasiKey>, _>>()?;
    if ks.is_empty() {
        return Err(format!(
            "[quasimodes] {name}: a quasimode needs at least one key"
        ));
    }
    Ok(ks)
}

fn key_names<'a>(name: &str, keys: &'a toml::Value) -> Result<Vec<&'a str>, String> {
    match keys {
        toml::Value::String(s) => Ok(vec![s.as_str()]),
        toml::Value::Array(a) => a
            .iter()
            .map(|v| {
                v.as_str()
                    .ok_or(format!("[quasimodes] {name}: each key is a string"))
            })
            .collect(),
        _ => Err(format!(
            "[quasimodes] {name}: a key name, or a list of them"
        )),
    }
}

/// A bound command line must start with a command the memex has, or be
/// "palette".
fn check_command(name: &str, cmd: &str) -> Result<(), String> {
    let first = cmd.split_whitespace().next().unwrap_or("");
    if first != "palette" && !COMMANDS.iter().any(|c| c.name == first) {
        return Err(format!(
            "[keys] {name}: no command “{first}”; the commands are listed in docs/guide.md"
        ));
    }
    Ok(())
}

fn quasi_named(name: &str) -> Option<Quasi> {
    match name {
        "leap_forward" => Some(Quasi::LeapForward),
        "leap_back" => Some(Quasi::LeapBack),
        "command" => Some(Quasi::Command),
        "scope" => Some(Quasi::Scope),
        "suppose" => Some(Quasi::Suppose),
        _ => None,
    }
}

impl Config {
    /// Read a configuration over the defaults: a quasimode or key named
    /// replaces its default, and the rest keep theirs. Every error names what
    /// is wrong.
    pub fn parse(text: &str) -> Result<Config, String> {
        Config::read(text, Config::default().bindings)
    }

    /// Read a file's text over the given bindings: each section, then
    /// what every configuration must have.
    fn read(text: &str, bindings: Vec<(Key, String)>) -> Result<Config, String> {
        let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut config = Config::empty(bindings);
        for (section, value) in &table {
            config.read_section(section, value)?;
        }
        config.complete()?;
        Ok(config)
    }

    fn empty(bindings: Vec<(Key, String)>) -> Config {
        Config {
            quasimodes: vec![],
            bindings,
            mouse: false,
        }
    }

    fn read_section(&mut self, section: &str, value: &toml::Value) -> Result<(), String> {
        let t = value
            .as_table()
            .ok_or_else(|| format!("{section} must be a table, as [{section}]"))?;
        match section {
            "quasimodes" => self.read_quasimodes(t),
            "keys" => self.read_keys(t),
            "mouse" => self.read_mouse(t),
            other => Err(format!(
                "no section [{other}]: the sections are [quasimodes], [keys] and [mouse]"
            )),
        }
    }

    /// [quasimodes]: each named quasimode's keys replace its defaults.
    fn read_quasimodes(&mut self, t: &toml::Table) -> Result<(), String> {
        for (name, keys) in t {
            let q = quasi_named(name).ok_or_else(|| {
                format!("[quasimodes] has no {name}: the quasimodes are leap_forward, leap_back, command, scope, suppose")
            })?;
            self.quasimodes.push((q, key_list(name, keys)?));
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
                check_command(name, cmd)?;
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
            Quasi::Scope => "scope",
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
    fn six_default_bindings() {
        // Catches a default binding added or lost unnoticed.
        assert_eq!(Config::default().bindings.len(), 6);
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
    fn key_names_read_back() {
        // Catches a key shown to the user under a name the file would refuse.
        for name in ["F1", "F35", "Insert", "NumLock"] {
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
    fn a_binding_replaces_or_unbinds_its_default() {
        // Catches "" not unbinding, an unbound key not free for a
        // quasimode, or a binding added beside its default.
        let c = Config::parse("[keys]\nF1 = \"\"\n[quasimodes]\nscope = \"F1\"\n").unwrap();
        assert_eq!(bound(&c, "F1"), None);
        assert_eq!(quasi(&c, "F1"), Some(Quasi::Scope));
        let c = Config::parse("[keys]\nF10 = \"export out.md\"\n").unwrap();
        assert_eq!(bound(&c, "F10"), Some("export out.md"), "replaced");
    }

    #[test]
    fn an_empty_file_is_the_defaults() {
        // Catches an empty file read as no settings at all.
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn the_mouse_is_off_unless_asked_for() {
        // Catches the mouse on by default, or not turned on by the file.
        assert!(!Config::default().mouse);
        assert!(Config::parse("[mouse]\nenabled = true\n").unwrap().mouse);
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
            ("[quasimodes]\nscope = \"F1\"\n", "two jobs"),
            ("[quasimodes]\nscope = \"LeftAlt\"\n", "two jobs"),
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
}
