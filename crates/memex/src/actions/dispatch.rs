//! The command line: the table of commands, finding a command from what
//! was typed, and running it on the target (docs/guide.md, § 2.4).

use crate::app::{App, Mode, ScopeLevel};
use crate::outline::TOP;

#[derive(Debug, Clone, Copy)]
enum Slot {
    Addr,
    Word,
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
        slots: &[("suggestion", Slot::Word)],
        help: "adopt the nth suggestion shown, with its evidence",
        brief: "adopt the nth suggestion shown",
    },
    Command {
        name: "adopt",
        run: App::cmd_adopt,
        target: true,
        slots: &[],
        help: "make the target's claims beliefs, keeping their source",
        brief: "make the target's claims beliefs",
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
        slots: &[("after", Slot::Addr)],
        help: "copy the target's text to a new item after an item",
        brief: "copy the target's text",
    },
    Command {
        name: "delete",
        run: App::cmd_delete,
        target: true,
        slots: &[],
        help: "delete the target: a word, or an item and its branch",
        brief: "delete a word, or a branch",
    },
    Command {
        name: "fold",
        run: App::cmd_fold,
        target: true,
        slots: &[],
        help: "hide the target's branch",
        brief: "hide the target's branch",
    },
    Command {
        name: "dismiss",
        run: App::cmd_dismiss,
        target: false,
        slots: &[("suggestion", Slot::Word)],
        help: "dismiss the nth suggestion shown; it returns only if its evidence changes",
        brief: "dismiss the nth suggestion",
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
        name: "help",
        run: App::cmd_help,
        target: false,
        slots: &[],
        help: "list every command",
        brief: "list every command",
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
        slots: &[("from", Slot::Addr), ("to", Slot::Addr), ("as", Slot::Word)],
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
        slots: &[("after", Slot::Addr)],
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
        help: "set the other pane to outline or links",
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
        slots: &[("entry", Slot::Word)],
        help: "reverse one earlier entry, keeping later work; lists what depends on it first",
        brief: "reverse one earlier entry",
    },
    Command {
        name: "suppose",
        run: App::cmd_suppose,
        target: false,
        slots: &[("that", Slot::Text)],
        help: "start a named world with nothing supposed; hold Suppose to suppose a statement",
        brief: "start a named world",
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
        slots: &[("from", Slot::Addr)],
        help: "show an item's text in place, in a new item after the target",
        brief: "show an item's text in place",
    },
    Command {
        name: "unadopt",
        run: App::cmd_unadopt,
        target: true,
        slots: &[],
        help: "retract the target's last adoption",
        brief: "retract the last adoption",
    },
    Command {
        name: "undo",
        run: App::cmd_undo,
        target: false,
        slots: &[],
        help: "undo the last change on this branch",
        brief: "undo the last change",
    },
    Command {
        name: "unfold",
        run: App::cmd_unfold,
        target: true,
        slots: &[],
        help: "show the target's branch",
        brief: "show the target's branch",
    },
];

impl App {
    pub(crate) fn cmd_help(&mut self, _: &[&str], _: u64) -> Result<String, String> {
        self.session.message = COMMANDS
            .iter()
            .map(|c| c.name)
            .collect::<Vec<_>>()
            .join(" · ");
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

    /// The mode a command set, if any, taken out; scope returns to the
    /// statement unless a question is open.
    fn next_mode(&mut self) -> Mode {
        if self.session.mode == Mode::Normal {
            self.statement_scope();
        }
        std::mem::replace(&mut self.session.mode, Mode::Normal)
    }

    fn statement_scope(&mut self) {
        if self.session.view.scope != ScopeLevel::Statement {
            self.session.view.scope = ScopeLevel::Statement;
            self.session.view_dirty = true;
        }
    }

    // Addresses.

    /// An item named by its outline number ("3.2"), its name, "#seq", or the
    /// start of its text if only one item's text starts so.
    pub fn resolve(&self, addr: &str) -> Option<u64> {
        self.notebook.outline.addressed(addr.trim())
    }

    /// The one item that words address.
    pub(crate) fn address(&self, words: &[&str]) -> Option<u64> {
        self.addresses(words, 1).map(|a| a[0])
    }

    /// Split words into addresses: the first split where every part resolves.
    pub(crate) fn addresses(&self, words: &[&str], n: usize) -> Option<Vec<u64>> {
        let words: Vec<&str> = words
            .iter()
            .copied()
            .filter(|w| !["to", "from", "after", "as"].contains(w))
            .collect();
        if n == 1 {
            return self.resolve(&words.join(" ")).map(|a| vec![a]);
        }
        (1..words.len()).find_map(|k| {
            let a = self.resolve(&words[..k].join(" "))?;
            let b = self.resolve(&words[k..].join(" "))?;
            Some(vec![a, b])
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
        let two = self.addresses(addr, 2).is_some();
        let shown = self.guided_ends(addr, two);
        match kind {
            Some(k) => format!("{shown} (as) {k}"),
            None if open && two => format!("{shown} (as)"),
            None => shown,
        }
    }

    fn guided_ends(&self, addr: &[&str], two: bool) -> String {
        if two {
            let k = self.split_point(addr);
            format!("link {} (to) {}", addr[..k].join(" "), addr[k..].join(" "))
        } else if addr.is_empty() {
            "link (to)".to_string()
        } else {
            format!("link (to) {}", addr.join(" "))
        }
    }

    /// Where words that address two items divide.
    fn split_point(&self, addr: &[&str]) -> usize {
        (1..addr.len())
            .find(|k| {
                self.resolve(&addr[..*k].join(" ")).is_some()
                    && self.resolve(&addr[*k..].join(" ")).is_some()
            })
            .unwrap()
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

/// A link command's addresses, and the words after "as", if there are any.
fn split_kind<'a>(words: &'a [&'a str]) -> (&'a [&'a str], Option<String>) {
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
    let hits: Vec<&Command> = COMMANDS.iter().filter(|c| c.name.starts_with(&w)).collect();
    (hits.len() == 1).then(|| hits[0])
}

/// Recognition on Escape: complete the command word if it is unambiguous.
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

/// What may come next in a command line, for `?`.
pub fn command_help(input: &str) -> Vec<String> {
    let first = input.split_whitespace().next().unwrap_or("");
    match find_command(first).filter(|_| input.contains(' ')) {
        Some(c) => vec![usage(c)],
        None => commands_starting(&first.to_lowercase()),
    }
}

/// A command's slots, each with its guide word, and what it does.
fn usage(c: &Command) -> String {
    let slots: Vec<String> = c.slots.iter().map(|(n, _)| format!("({n}) …")).collect();
    format!("{} {} — {}", c.name, slots.join(" "), c.help)
}

/// The commands whose names start so, each with what it does, briefly.
fn commands_starting(prefix: &str) -> Vec<String> {
    COMMANDS
        .iter()
        .filter(|c| c.name.starts_with(prefix))
        .map(|c| format!("{} — {}", c.name, c.brief))
        .collect()
}
