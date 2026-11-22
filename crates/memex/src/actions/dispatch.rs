//! The command line: the table of commands, finding a command from what
//! was typed, and running it on the target (docs/guide.md, § 2.4).

use crate::app::{App, Mode};
use crate::outline::TOP;

/// What a command's slot takes: what may be typed in it, and what may be
/// pointed at to fill it (docs/design.md, Presentations).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// An item: its address, typed, or any presentation of it.
    Item,
    /// A journal entry, by its number: typed, or a presented layer,
    /// change, constraint or fact.
    Entry,
    /// A suggestion shown, by its number: typed, or the suggestion.
    Suggestion,
    /// A word.
    Word,
    /// Text.
    Text,
}

/// A command's handler, given the words after the command's name and the
/// target item. A command that asks a question sets `App::mode`.
/// A command: what it does to the memex, and its reply for the message
/// line; an empty reply leaves the message as it was.
type Handler = fn(&mut App, &[&str], u64) -> Result<String, String>;

pub struct Command {
    pub name: &'static str,
    run: Handler,
    /// It acts on the target item, so needs one.
    target: bool,
    slots: &'static [(&'static str, Slot)],
    pub help: &'static str,
    /// What it does, short enough for the list of commands that match
    /// what is typed (at most 32 characters).
    pub brief: &'static str,
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "accept",
        run: App::cmd_accept,
        target: false,
        slots: &[("suggestion", Slot::Suggestion)],
        help: "adopt the nth suggestion shown, with its evidence",
        brief: "adopt the nth suggestion shown",
    },
    Command {
        name: "adopt",
        run: App::cmd_adopt,
        target: true,
        slots: &[("suggestion", Slot::Suggestion)],
        help: "make the target's claims beliefs, keeping their source (`adopt instead`: retracting what they contradict); `adopt world`, the supposed world as the actual one; with a number, the suggestion shown with it, a claim the memex makes",
        brief: "believe claims or a suggestion",
    },
    Command {
        name: "claim",
        run: App::cmd_claim,
        target: true,
        slots: &[("that", Slot::Text)],
        help: "the target states a claim: a Prolog fact, then from and its source",
        brief: "the target states a claim",
    },
    Command {
        name: "check",
        run: App::cmd_check,
        target: true,
        slots: &[("which", Slot::Word)],
        help: "check quote (the text is in its source) or check source (the DOI, through Crossref)",
        brief: "check a quote or a source",
    },
    Command {
        name: "close",
        run: App::cmd_close,
        target: false,
        slots: &[],
        help: "close the supposition and return to the actual world; it stays in the journal",
        brief: "return to the actual world",
    },
    Command {
        name: "commit",
        run: App::cmd_commit,
        target: false,
        slots: &[],
        help: "make the supposed world the actual one",
        brief: "make the supposed world actual",
    },
    Command {
        name: "constraint",
        run: App::cmd_constraint,
        target: false,
        slots: &[("named", Slot::Text)],
        help: "a constraint: its name, a colon, then a Prolog denial false :- ...",
        brief: "a rule that must never break",
    },
    Command {
        name: "copy",
        run: App::cmd_copy,
        target: true,
        slots: &[("after", Slot::Item)],
        help: "copy the target's text to a new item after an item",
        brief: "copy the target's text",
    },
    Command {
        name: "delete",
        run: App::cmd_delete,
        target: true,
        slots: &[],
        help: "delete the target and its branch; delete word deletes the word at the text cursor",
        brief: "delete a branch, or a word",
    },
    Command {
        name: "fold",
        run: App::cmd_fold,
        target: true,
        slots: &[],
        help: "hide the target's branch, or show it if it is hidden",
        brief: "hide or show the branch",
    },
    Command {
        name: "dismiss",
        run: App::cmd_dismiss,
        target: false,
        slots: &[("suggestion", Slot::Suggestion)],
        help: "set aside what is offered: the nth suggestion shown, which returns only if its evidence changes; `dismiss world`, the supposed world, back to the actual one; `dismiss change N`, one change of a revision",
        brief: "set a suggestion or world aside",
    },
    Command {
        name: "derive",
        run: App::cmd_derive,
        target: true,
        slots: &[("text", Slot::Text)],
        help: "the target shows a computed text: a template with {Var}, then where and a Prolog goal",
        brief: "show a text computed from facts",
    },
    Command {
        name: "export",
        run: App::cmd_export,
        target: false,
        slots: &[("to", Slot::Text)],
        help: "export the outline, or one section, as a document (md, docx, odt, tex, html)",
        brief: "export as a document",
    },
    Command {
        name: "fact",
        run: App::cmd_fact,
        target: true,
        slots: &[("that", Slot::Text)],
        help: "the target states a belief: a Prolog fact",
        brief: "the target states a belief",
    },
    Command {
        name: "follow",
        run: App::cmd_follow,
        target: true,
        slots: &[("link", Slot::Word)],
        help: "go to the item at the end of the target's nth link",
        brief: "go to the end of the nth link",
    },
    Command {
        name: "next",
        run: App::cmd_next,
        target: true,
        slots: &[],
        help: "go to the next item at the target's level",
        brief: "next at this level",
    },
    Command {
        name: "previous",
        run: App::cmd_previous,
        target: true,
        slots: &[],
        help: "go to the previous item at the target's level",
        brief: "previous at this level",
    },
    Command {
        name: "find",
        run: App::cmd_find,
        target: false,
        slots: &[("what", Slot::Text)],
        help: "list beside the outline the items that hold a text, carry a #label, or are marked by violations or disagreements; find suggestions lists those; a question (What is the rest?, or Prolog in backticks) is answered once; find alone shows the links again",
        brief: "list what matches, or answer",
    },
    Command {
        name: "first",
        run: App::cmd_first,
        target: true,
        slots: &[],
        help: "go to the first item at the target's level",
        brief: "first at this level",
    },
    Command {
        name: "last",
        run: App::cmd_last,
        target: true,
        slots: &[],
        help: "go to the last item at the target's level",
        brief: "last at this level",
    },
    Command {
        name: "parent",
        run: App::cmd_parent,
        target: true,
        slots: &[],
        help: "go to the item the target is under",
        brief: "up to the item above",
    },
    Command {
        name: "child",
        run: App::cmd_child,
        target: true,
        slots: &[],
        help: "go to the first item under the target, showing it",
        brief: "down to the first below",
    },
    Command {
        name: "back",
        run: App::cmd_back,
        target: false,
        slots: &[],
        help: "go back to where the last jump (follow, leap, go) came from",
        brief: "go back along the trail",
    },
    Command {
        name: "forward",
        run: App::cmd_forward,
        target: false,
        slots: &[],
        help: "go forward again to where back came from",
        brief: "go forward along the trail",
    },
    Command {
        name: "go",
        run: App::cmd_go,
        target: false,
        slots: &[("text", Slot::Item)],
        help: "go to the item an address names, else to the next that holds the text; go link N follows the target's Nth link; go back and go forward walk the trail of jumps; go next, previous, first, last, parent or child moves by the outline's structure",
        brief: "go to an address, link or place",
    },
    Command {
        name: "help",
        run: App::cmd_help,
        target: false,
        slots: &[],
        help: "open the list of every command, as F1 does",
        brief: "list every command (F1)",
    },
    Command {
        name: "import",
        run: App::cmd_import,
        target: false,
        slots: &[("from", Slot::Text)],
        help: "import wikidata, document or results, then a file: what it says arrives as claims",
        brief: "import a file as claims",
    },
    Command {
        name: "indent",
        run: App::cmd_indent,
        target: true,
        slots: &[],
        help: "make the target a child of the item above it",
        brief: "make the target a child",
    },
    Command {
        name: "interrogate",
        run: App::cmd_interrogate,
        target: true,
        slots: &[],
        help: "ask of the target's claim: who else claims it, who disputes it, its source and record",
        brief: "question the target's claim",
    },
    Command {
        name: "layer",
        run: App::cmd_layer,
        target: true,
        slots: &[("mark", Slot::Text)],
        help: "add a layer to the target's view: a mark, then where and a goal; one mark per solution",
        brief: "add a layer to a view",
    },
    Command {
        name: "link",
        run: App::cmd_link,
        target: true,
        slots: &[("from", Slot::Item), ("to", Slot::Item), ("as", Slot::Word)],
        help: "link one item to another; the type defaults to link",
        brief: "link one item to another",
    },
    Command {
        name: "make",
        run: App::cmd_make,
        target: true,
        slots: &[],
        help: "make an item for each of the target's [[links]] that names no item, at the foot of the top level, and link it",
        brief: "make items for its [[links]]",
    },
    Command {
        name: "move",
        run: App::cmd_move,
        target: true,
        slots: &[("after", Slot::Item)],
        help: "move the target's branch to follow an item",
        brief: "move the target's branch",
    },
    Command {
        name: "name",
        run: App::cmd_name,
        target: true,
        slots: &[("as", Slot::Word)],
        help: "give the target a name that addresses it",
        brief: "give the target a name",
    },
    Command {
        name: "new",
        run: App::cmd_new,
        target: false,
        slots: &[],
        help: "a new item after the target",
        brief: "a new item after the target",
    },
    Command {
        name: "outdent",
        run: App::cmd_outdent,
        target: true,
        slots: &[],
        help: "make the target a sibling of its parent",
        brief: "make the target a sibling",
    },
    Command {
        name: "pane",
        run: App::cmd_pane,
        target: false,
        slots: &[("shows", Slot::Word)],
        help: "set the other pane to outline, links, suggestions or matches",
        brief: "set the other pane",
    },
    Command {
        name: "quit",
        run: App::cmd_quit,
        target: false,
        slots: &[],
        help: "write what is pending and leave",
        brief: "write what is pending and leave",
    },
    Command {
        name: "query",
        run: App::cmd_query,
        target: false,
        slots: &[("goal", Slot::Text)],
        help: "run a Prolog goal over this world's beliefs, through rules: each answer by its variables (at most 20), yes or no for a goal without them; it may only read facts, compare and compute",
        brief: "run a Prolog query",
    },
    Command {
        name: "redo",
        run: App::cmd_redo,
        target: false,
        slots: &[],
        help: "redo what undo undid, along the current branch",
        brief: "redo what undo undid",
    },
    Command {
        name: "rule",
        run: App::cmd_rule,
        target: false,
        slots: &[("that", Slot::Text)],
        help: "define a predicate from others: Head :- Body. Usable in derive, constraint, layer and other rules",
        brief: "define a predicate from others",
    },
    Command {
        name: "ace",
        run: App::cmd_ace,
        target: false,
        slots: &[("sentence", Slot::Text)],
        help: "what APE makes of a sentence of controlled English: its paraphrase, or where it fails and APE's advice",
        brief: "what APE makes of a sentence",
    },
    Command {
        name: "set",
        run: App::cmd_set,
        target: false,
        slots: &[("that", Slot::Text)],
        help: "state a fact and retract the beliefs it directly contradicts under a constraint; the fact goes where the replaced one was",
        brief: "state a fact in place of others",
    },
    Command {
        name: "retract",
        run: App::cmd_retract,
        target: false,
        slots: &[("entry", Slot::Entry)],
        help: "reverse one earlier entry, keeping later work; lists what depends on it first; with no entry, the target's last adoption, fact or layer",
        brief: "reverse an earlier entry (F4)",
    },
    Command {
        name: "suppose",
        run: App::cmd_suppose,
        target: false,
        slots: &[("that", Slot::Text)],
        help: "suppose a statement, as the Suppose key does: its readings against the facts are offered; with none, a world of that name opens with nothing supposed",
        brief: "suppose a statement",
    },
    Command {
        name: "switch",
        run: App::cmd_switch,
        target: false,
        slots: &[],
        help: "move to the other pane",
        brief: "move to the other pane",
    },
    Command {
        name: "split",
        run: App::cmd_split,
        target: true,
        slots: &[],
        help: "split the target's text at each sentence end: it keeps the first sentence; each other becomes a new item after it, linked back as split_from, its characters keeping their history",
        brief: "split an item into its sentences",
    },
    Command {
        name: "transclude",
        run: App::cmd_transclude,
        target: true,
        slots: &[("from", Slot::Item)],
        help: "show an item's text in place, in a new item after the target",
        brief: "show an item's text in place",
    },
    Command {
        name: "undo",
        run: App::cmd_undo,
        target: false,
        slots: &[],
        help: "undo the last change on this branch",
        brief: "undo the last change",
    },
];

impl App {
    /// Open the list of commands, as F1 does: one way to ask for help.
    pub(crate) fn cmd_help(&mut self, _: &[&str], _: u64) -> Result<String, String> {
        self.session.mode = Mode::Quasi(crate::app::Active::palette());
        Ok(String::new())
    }

    pub(crate) fn cmd_quit(&mut self, _: &[&str], _: u64) -> Result<String, String> {
        self.session.quit = true;
        Ok(String::new())
    }

    pub(crate) fn run_simple(&mut self, name: &str) {
        self.session.mode = Mode::Normal;
        let m = self.run_command(name);
        self.session.mode = m;
    }

    /// Run a command line, as typed in the Command quasimode.
    pub fn run_command(&mut self, line: &str) -> Mode {
        self.flush();
        let mode = self.command_or_message(line.trim());
        self.flush();
        self.update_frozen();
        mode
    }

    /// Run a command; if it fails, its error becomes the message. What an
    /// earlier command said is cleared first, so that a command that works
    /// and says nothing does not leave an earlier refusal on show.
    fn command_or_message(&mut self, line: &str) -> Mode {
        self.session.message.clear();
        self.command(line).unwrap_or_else(|m| {
            self.session.message = m;
            Mode::Normal
        })
    }

    /// Run a command line; the mode it leaves: Normal, unless the command
    /// asks a question.
    pub(crate) fn command(&mut self, line: &str) -> Result<Mode, String> {
        let Some((cmd, words)) = parse_line(line)? else {
            return Ok(Mode::Normal);
        };
        let reply = self.run(cmd, &words)?;
        if !reply.is_empty() {
            self.session.message = reply;
        }
        Ok(self.next_mode())
    }

    fn run(&mut self, cmd: &Command, words: &[&str]) -> Result<String, String> {
        let target = self.target_for(cmd)?;
        (cmd.run)(self, words, target)
    }

    /// The item a command acts on: the cursor's, which it must have if the
    /// command needs one.
    fn target_for(&self, cmd: &Command) -> Result<u64, String> {
        let target = self.cursor();
        if cmd.target && target == TOP {
            return Err("there is no item to act on".into());
        }
        Ok(target)
    }

    /// The mode a command set, if any, taken out.
    fn next_mode(&mut self) -> Mode {
        std::mem::replace(&mut self.session.mode, Mode::Normal)
    }

    // Addresses.

    /// An item named by its outline number ("3.2"), its name, "#seq", or the
    /// start of its text if only one item's text starts so.
    pub fn resolve(&self, addr: &str) -> Option<u64> {
        self.notebook.outline.addressed(addr.trim())
    }

    /// The one item that words address, guide words left out.
    pub(crate) fn address(&self, words: &[&str]) -> Option<u64> {
        self.resolve(&unguided(words).join(" "))
    }

    /// The two items that words address, guide words left out, and where
    /// those words divide: the first split where both parts resolve.
    pub(crate) fn two_addresses(&self, words: &[&str]) -> Option<(usize, u64, u64)> {
        let words = unguided(words);
        (1..words.len()).find_map(|k| {
            let a = self.resolve(&words[..k].join(" "))?;
            Some((k, a, self.resolve(&words[k..].join(" "))?))
        })
    }

    // The command line as shown.

    /// The command line as shown: once the command is recognised, each slot
    /// is preceded by its guide word in parentheses ("link 3.2 (to) source 9").
    pub fn guided(&self, input: &str) -> String {
        let mut words: Vec<&str> = input.split_whitespace().collect();
        if words.is_empty() {
            return String::new();
        }
        let Some(cmd) = find_command(words[0])
            .filter(|c| c.name == words[0].to_lowercase() || input.ends_with(' '))
        else {
            return input.to_string();
        };
        words.remove(0);
        let open = input.ends_with(' ');
        if cmd.name == "link" {
            return self.guided_link(&words, open);
        }
        guided_slots(cmd, &words, open)
    }

    /// The link command's line: its two ends, then its type.
    fn guided_link(&self, words: &[&str], open: bool) -> String {
        let (addr, kind) = split_kind(words);
        // A guide word typed is shown in its place, in parentheses.
        let addr = unguided(addr);
        let split = self.two_addresses(&addr).map(|(k, _, _)| k);
        let shown = self.guided_ends(&addr, split);
        match kind {
            Some(k) => format!("{shown} (as) {k}"),
            None if open && split.is_some() => format!("{shown} (as)"),
            None => shown,
        }
    }

    /// The link's two ends: both as typed, or the one typed as its to,
    /// after the from it takes from the target, in its parentheses since
    /// it is not typed ("link (from 1) (to) 2").
    fn guided_ends(&self, addr: &[&str], split: Option<usize>) -> String {
        if let Some(k) = split {
            return format!("link {} (to) {}", addr[..k].join(" "), addr[k..].join(" "));
        }
        let from = self.default_from();
        match addr.is_empty() {
            true => format!("link{from} (to)"),
            false => format!("link{from} (to) {}", addr.join(" ")),
        }
    }

    /// The from that a link with one address takes: the target's address,
    /// if the cursor is on an item.
    fn default_from(&self) -> String {
        let outline = &self.notebook.outline;
        let target = self.cursor();
        match outline.items.contains_key(&target) {
            true => format!(" (from {})", outline.number(target)),
            false => String::new(),
        }
    }
}

/// A command's words, and its first slot's guide word before them.
fn guided_slots(cmd: &Command, words: &[&str], open: bool) -> String {
    let mut out = cmd.name.to_string();
    if let Some((noise, _)) = cmd.slots.first() {
        if !words.is_empty() || open {
            out.push_str(&format!(" ({noise})"));
        }
        if !words.is_empty() {
            out.push(' ');
            out.push_str(&words.join(" "));
        }
    }
    out
}

/// Words without the guide words that may come before an address.
fn unguided<'a>(words: &[&'a str]) -> Vec<&'a str> {
    let guides = ["to", "from", "after", "as"];
    words
        .iter()
        .copied()
        .filter(|w| !guides.contains(w))
        .collect()
}

/// A link command's addresses, and the words after "as", if there are any.
pub(crate) fn split_kind<'a>(words: &'a [&'a str]) -> (&'a [&'a str], Option<String>) {
    match words.iter().position(|w| *w == "as") {
        Some(i) => (&words[..i], Some(words[i + 1..].join(" "))),
        None => (words, None),
    }
}

/// A command line's command and the words after it; None if it is empty.
fn parse_line(line: &str) -> Result<Option<(&'static Command, Vec<&str>)>, String> {
    let mut words: Vec<&str> = line.split_whitespace().collect();
    if words.is_empty() {
        return Ok(None);
    }
    let cmd =
        find_command(words[0]).ok_or_else(|| format!("no command “{}”; ? lists them", words[0]))?;
    words.remove(0);
    Ok(Some((cmd, words)))
}

/// The slot a command line waits for, if it ends in a space: the slot of
/// the guide word typed last, else its command's next slot, the words
/// typed so far filling its slots in order.
pub fn awaited(input: &str) -> Option<Slot> {
    if !input.ends_with(' ') {
        return None;
    }
    let (cmd, words) = parse_line(input).ok()??;
    let last_guide = words.last().and_then(|w| guide_slot(cmd, w));
    last_guide.or_else(|| next_slot(cmd, &words))
}

/// The slot a guide word of a command names, if the word is one.
fn guide_slot(cmd: &Command, word: &str) -> Option<Slot> {
    cmd.slots.iter().find(|(g, _)| *g == word).map(|(_, s)| *s)
}

/// A command's slot after those its words fill, guide words not counted.
fn next_slot(cmd: &Command, words: &[&str]) -> Option<Slot> {
    let filled = words
        .iter()
        .filter(|w| guide_slot(cmd, w).is_none())
        .count();
    cmd.slots.get(filled).map(|(_, s)| *s)
}

/// Whether a command line ends inside a quoted string ("…" or '…'), so a
/// ? typed now is part of the string, not a request for help.
pub(crate) fn in_quotes(input: &str) -> bool {
    let mut open: Option<char> = None;
    let mut escaped = false;
    for c in input.chars() {
        match (open, c) {
            (Some(_), '\\') if !escaped => {
                escaped = true;
                continue;
            }
            (Some(q), c) if c == q && !escaped => open = None,
            (None, '"' | '\'') => open = Some(c),
            _ => {}
        }
        escaped = false;
    }
    open.is_some()
}

/// The command a word names: exactly, or as the start of only one.
pub(crate) fn find_command(w: &str) -> Option<&'static Command> {
    let w = w.to_lowercase();
    if let Some(c) = COMMANDS.iter().find(|c| c.name == w) {
        return Some(c);
    }
    let taught = COMMANDS.iter().filter(|c| !ALIASES.contains(&c.name));
    let hits: Vec<&Command> = taught.filter(|c| c.name.starts_with(&w)).collect();
    (hits.len() == 1).then(|| hits[0])
}

/// Recognition on Tab: complete the command word if it is unambiguous.
pub(crate) fn complete_command(input: &str) -> String {
    let mut parts = input.splitn(2, ' ');
    let first = parts.next().unwrap_or("");
    let rest = parts.next();
    match find_command(first).map(|c| c.name) {
        Some(name) if rest.is_none() => format!("{name} "),
        Some(name) => format!("{name} {}", rest.unwrap()),
        None => input.to_string(),
    }
}

/// The commands in groups, in the order the guide lists them (§ 2.4):
/// each group's heading and its commands' names.
pub const GROUPS: &[(&str, &[&str])] = &[
    (
        "EDITING THE OUTLINE",
        &[
            "new",
            "delete",
            "indent",
            "outdent",
            "make",
            "move",
            "copy",
            "split",
            "transclude",
            "link",
            "fold",
            "switch",
            "help",
            "quit",
        ],
    ),
    ("FINDING AND MOVING", &["find", "go"]),
    ("FACTS AND DEFINITIONS", &["layer", "check"]),
    (
        "CLAIMS AND SUGGESTIONS",
        &["adopt", "dismiss", "import", "export"],
    ),
    (
        "WORLDS AND HISTORY",
        &["suppose", "undo", "redo", "retract"],
    ),
];

/// Commands kept by their old names, for those who learnt them and for
/// the journals and tests that use them, but no longer taught
/// (docs/concepts.md): they are in no group, so not listed, and only
/// their whole name finds them. Statements are written in items instead,
/// as sentences or in Prolog between backticks.
pub const ALIASES: &[&str] = &[
    "fact",
    "claim",
    "rule",
    "constraint",
    "ace",
    "derive",
    "accept",
    "commit",
    "close",
    "set",
    "query",
    "pane",
    "interrogate",
    "name",
    "follow",
    "back",
    "forward",
    "next",
    "previous",
    "first",
    "last",
    "parent",
    "child",
];

/// A group of commands: its heading, and those of its commands listed.
pub type Group = (&'static str, Vec<&'static Command>);

/// When a command line names its command and goes on: the command's
/// slots and what it does, for `?`.
pub fn command_usage(input: &str) -> Option<String> {
    let first = input.split_whitespace().next().unwrap_or("");
    find_command(first)
        .filter(|_| input.contains(' '))
        .map(usage)
}

/// The commands whose names start as a command line does, in their
/// groups; a group with none is left out.
pub fn command_groups(input: &str) -> Vec<Group> {
    let prefix = input.split_whitespace().next().unwrap_or("").to_lowercase();
    let named = |n: &&str| COMMANDS.iter().find(|c| c.name == *n);
    let starting = |names: &[&str]| -> Vec<&'static Command> {
        let names = names.iter().filter(|n| n.starts_with(&prefix));
        names.filter_map(named).collect()
    };
    let groups = GROUPS
        .iter()
        .map(|(heading, names)| (*heading, starting(names)));
    groups.filter(|(_, cs)| !cs.is_empty()).collect()
}

/// A command's slots, each with its guide word, and what it does.
fn usage(c: &Command) -> String {
    let slots: Vec<String> = c.slots.iter().map(|(n, _)| format!("({n}) …")).collect();
    format!("{} {} — {}", c.name, slots.join(" "), c.help)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_waits_for_the_slot_after_what_was_typed() {
        // Catches a line waiting before its command is ended by a space,
        // a slot counted wrongly past a guide word, or the slot's type lost.
        let cases = [
            ("go", None),
            ("go ", Some(Slot::Item)),
            ("go 3", None),
            ("link ", Some(Slot::Item)),
            ("link to ", Some(Slot::Item)),
            ("link 3 ", Some(Slot::Item)),
            ("link 3 to 5 as ", Some(Slot::Word)),
            ("retract ", Some(Slot::Entry)),
            ("accept ", Some(Slot::Suggestion)),
            ("query ", Some(Slot::Text)),
            ("adopt ", Some(Slot::Suggestion)),
            ("nosuch ", None),
        ];
        for (line, want) in cases {
            assert_eq!(awaited(line), want, "{line:?}");
        }
    }

    #[test]
    fn each_command_is_in_one_group() {
        // Catches a command added to the table but to no group, so the
        // list of commands would never show it, or put in two. An alias is
        // in none, on purpose.
        for c in COMMANDS {
            let groups = GROUPS.iter().filter(|(_, ns)| ns.contains(&c.name)).count();
            let want = usize::from(!ALIASES.contains(&c.name));
            assert_eq!(groups, want, "{} is in {groups} groups", c.name);
        }
        let listed: usize = GROUPS.iter().map(|(_, ns)| ns.len()).sum();
        assert_eq!(
            listed + ALIASES.len(),
            COMMANDS.len(),
            "a group names no command"
        );
    }

    #[test]
    fn an_alias_runs_by_its_whole_name_and_is_neither_listed_nor_a_prefix() {
        // Catches an old name taught again: listed beside what replaced
        // it, or found by a prefix, so that "con" was ambiguous.
        for alias in ALIASES {
            assert_eq!(find_command(alias).map(|c| c.name), Some(*alias));
            let listed = GROUPS.iter().any(|(_, ns)| ns.contains(alias));
            assert!(!listed, "{alias} is listed");
        }
        assert!(find_command("con").is_none(), "no taught command starts so");
        assert!(find_command("fac").is_none());
    }

    #[test]
    fn the_moves_have_a_group_of_their_own() {
        // Catches the moves mixed in with the edits again: a reader looking
        // for how to get somewhere finds every way under one heading.
        let groups = command_groups("");
        let heading = "FINDING AND MOVING";
        let (_, moving) = groups.iter().find(|(h, _)| *h == heading).unwrap();
        let names: Vec<&str> = moving.iter().map(|c| c.name).collect();
        let moves = ["find", "go"];
        assert_eq!(names, moves);
        let (_, editing) = &groups[0];
        assert!(editing.iter().all(|c| !moves.contains(&c.name)));
    }
}
