//! A command's nouns (docs/concepts-next.md § The grammar): a command is
//! a verb and nouns. The first is the target, or the selection. A verb
//! typed alone takes each noun's default; for a noun with none, the memex
//! prompts, keeping the Command quasimode open until it is given.

use super::dispatch::{Command, Slot};
use crate::app::App;

/// A noun of a command: the guide word shown before it, the kind of thing
/// it takes, what it is if left out, and the words it may be, each with
/// what it means, so `?` lists them and Tab completes them.
#[derive(Clone, Copy)]
pub struct Noun {
    pub guide: &'static str,
    pub kind: Slot,
    fill: Fill,
    among: &'static [(&'static str, &'static str)],
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
        among: &[],
    }
}

impl Noun {
    /// The noun, which may be these words, each with what it means.
    pub const fn among(self, words: &'static [(&'static str, &'static str)]) -> Noun {
        Noun {
            among: words,
            ..self
        }
    }

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

/// Whether a command's first noun takes text, so that the text selected
/// fills it: a command with no target whose first noun is text, or an
/// item it reads from an address or a text (`go`).
pub(crate) fn takes_text(cmd: &Command) -> bool {
    let first = cmd.slots.first().map(|n| n.kind);
    !cmd.target && matches!(first, Some(Slot::Text | Slot::Item))
}

/// The words a line's first noun may be that start as its last word does,
/// once the line names its command: each with what it means. None before
/// the command is named, or after a second word; the noun's guide word
/// typed is not one.
pub fn candidates(input: &str) -> Vec<(&'static str, &'static str)> {
    let Some((_, noun, start)) = first_noun(input) else {
        return vec![];
    };
    let among = noun.among.iter().copied();
    among.filter(|(w, _)| w.starts_with(start)).collect()
}

/// A line's command, its first noun, and the start of that noun's word
/// typed, if the line has gone no further.
fn first_noun(input: &str) -> Option<(&'static Command, Noun, &str)> {
    let (verb, rest) = input.split_once(' ')?;
    let cmd = super::dispatch::find_command(verb)?;
    let noun = *cmd.slots.first()?;
    Some((cmd, noun, typing(rest, noun.guide)?))
}

/// The start of the one word being typed in what follows a line's
/// command, its guide word left out, or nothing yet if it ends in a
/// space; none after more words.
fn typing<'a>(rest: &'a str, guide: &str) -> Option<&'a str> {
    let typed: Vec<&str> = rest.split_whitespace().filter(|w| *w != guide).collect();
    match (typed.as_slice(), rest.is_empty() || rest.ends_with(' ')) {
        ([], true) => Some(""),
        ([w], false) => Some(w),
        _ => None,
    }
}

/// Recognition on Tab after a command: the first noun's word, after its
/// guide word if the command reads it, if one word alone starts as the
/// last word does ("adopt wo" becomes "adopt world ").
pub(crate) fn complete_noun(input: &str) -> Option<String> {
    let (cmd, noun, _) = first_noun(input)?;
    let [(word, _)] = candidates(input)[..] else {
        return None;
    };
    let word = word.split_whitespace().next()?;
    Some(format!("{} {} ", cmd.name, noun.words(word)))
}
