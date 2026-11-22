//! A command's nouns (docs/concepts-next.md § The grammar): a command is
//! a verb and nouns. The first is the target, or the selection. A verb
//! typed alone takes each noun's default; for a noun with none, the memex
//! prompts, keeping the Command quasimode open until it is given.

use super::dispatch::{Command, Slot};
use crate::app::App;

/// A noun of a command: the guide word shown before it, the kind of thing
/// it takes, and what it is if left out.
#[derive(Clone, Copy)]
pub struct Noun {
    pub guide: &'static str,
    pub kind: Slot,
    fill: Fill,
}

/// What a noun left out is.
#[derive(Clone, Copy)]
enum Fill {
    /// Nothing: the command does what it does without it.
    Optional,
    /// Asked for: the memex prompts for it.
    Asked,
    /// A word it always is, if it is left out.
    Fixed(&'static str),
    /// Its default now, for a target; with none now, it is asked for, and
    /// the prompt says why there is none.
    Default(fn(&App, u64) -> Option<String>, &'static str),
}

/// A noun the command does without, if it is left out.
pub const fn noun(guide: &'static str, kind: Slot) -> Noun {
    Noun {
        guide,
        kind,
        fill: Fill::Optional,
    }
}

impl Noun {
    /// The noun, asked for if it is left out.
    pub const fn asked(self) -> Noun {
        Noun {
            fill: Fill::Asked,
            ..self
        }
    }

    /// The noun, a word it always is if it is left out.
    pub const fn fixed(self, word: &'static str) -> Noun {
        Noun {
            fill: Fill::Fixed(word),
            ..self
        }
    }

    /// The noun, with a default if it is left out, and why there may be
    /// none.
    pub const fn defaults_to(
        self,
        default: fn(&App, u64) -> Option<String>,
        none: &'static str,
    ) -> Noun {
        Noun {
            fill: Fill::Default(default, none),
            ..self
        }
    }

    /// Whether it is asked for or has a default: whether the prompt
    /// shows it.
    fn prompted(&self) -> bool {
        !matches!(self.fill, Fill::Optional)
    }

    /// Its default now, for a target, if it has one.
    fn default_now(&self, app: &App, target: u64) -> Option<String> {
        match self.fill {
            Fill::Fixed(word) => Some(word.into()),
            Fill::Default(f, _) => f(app, target),
            _ => None,
        }
    }

    /// The words that give it: its value, after its guide word if the
    /// command reads the guide word.
    fn words(&self, value: &str) -> String {
        match super::dispatch::GUIDES.contains(&self.guide) {
            true => format!("{} {value}", self.guide),
            false => value.to_string(),
        }
    }
}

/// What a command typed alone is: its line, with each noun's default, or
/// the first noun it must ask for.
pub(crate) enum Alone {
    Line(String),
    Ask(Noun),
}

/// A command typed alone, for a target: its line with the defaults, or the
/// noun to ask for.
pub(crate) fn alone(app: &App, cmd: &Command, target: u64) -> Alone {
    let mut line = cmd.name.to_string();
    for n in cmd.slots.iter().filter(|n| n.prompted()) {
        let Some(value) = n.default_now(app, target) else {
            return Alone::Ask(*n);
        };
        line.push(' ');
        line.push_str(&n.words(&value));
    }
    Alone::Line(line)
}

/// The prompt for a command typed alone: each noun it prompts for, its
/// guide word, its kind, and its default now ("move (after) ‹item› [1.3]").
pub(crate) fn prompts(app: &App, cmd: &Command, target: u64) -> String {
    let shown = cmd.slots.iter().filter(|n| n.prompted());
    shown
        .map(|n| prompt(n, n.default_now(app, target)))
        .collect()
}

/// A noun as the prompt shows it, with its default, if it has one now.
fn prompt(n: &Noun, default: Option<String>) -> String {
    let kind = format!(" ({}) ‹{}›", n.guide, n.kind.name());
    match default {
        Some(d) => format!("{kind} [{d}]"),
        None => kind,
    }
}

/// What the message says while a noun is asked for: why it has no
/// default, if it may have one, and how to give it.
pub(crate) fn asking(cmd: &Command, n: &Noun) -> String {
    let noun = format!("{} ({}) ‹{}›", cmd.name, n.guide, n.kind.name());
    let how = "type it or point at it · Enter runs it, Esc leaves it";
    match n.fill {
        Fill::Default(_, none) => format!("{none}: {noun} · {how}"),
        _ => format!("{noun}: {how}"),
    }
}
