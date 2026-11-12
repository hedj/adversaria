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
type Handler = fn(&mut App, &[&str], u64) -> Result<(), String>;

pub struct Command {
    pub name: &'static str,
    run: Handler,
    /// It acts on the target item, so needs one.
    target: bool,
    slots: &'static [(&'static str, Slot)],
    pub help: &'static str,
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "accept",
        run: App::cmd_accept,
        target: false,
        slots: &[("suggestion", Slot::Word)],
        help: "adopt the nth suggestion shown, with its evidence",
    },
    Command {
        name: "adopt",
        run: App::cmd_adopt,
        target: true,
        slots: &[],
        help: "make the target's claims beliefs, keeping their source",
    },
    Command {
        name: "claim",
        run: App::cmd_claim,
        target: true,
        slots: &[("that", Slot::Text)],
        help: "the target states a claim: a Prolog fact, then from and its source",
    },
    Command {
        name: "check",
        run: App::cmd_check,
        target: true,
        slots: &[("which", Slot::Word)],
        help: "check quote (the text is in its source) or check source (the DOI, through Crossref)",
    },
    Command {
        name: "close",
        run: App::cmd_close,
        target: false,
        slots: &[],
        help: "close the supposition and return to the actual world; it stays in the journal",
    },
    Command {
        name: "commit",
        run: App::cmd_commit,
        target: false,
        slots: &[],
        help: "make the supposed world the actual one",
    },
    Command {
        name: "constraint",
        run: App::cmd_constraint,
        target: false,
        slots: &[("named", Slot::Text)],
        help: "a constraint: its name, a colon, then a Prolog denial false :- ...",
    },
    Command {
        name: "copy",
        run: App::cmd_copy,
        target: true,
        slots: &[("after", Slot::Addr)],
        help: "copy the target's text to a new item after an item",
    },
    Command {
        name: "delete",
        run: App::cmd_delete,
        target: true,
        slots: &[],
        help: "delete the target: a word, or an item and its branch",
    },
    Command {
        name: "fold",
        run: App::cmd_fold,
        target: true,
        slots: &[],
        help: "hide the target's branch",
    },
    Command {
        name: "dismiss",
        run: App::cmd_dismiss,
        target: false,
        slots: &[("suggestion", Slot::Word)],
        help: "dismiss the nth suggestion shown; it returns only if its evidence changes",
    },
    Command {
        name: "derive",
        run: App::cmd_derive,
        target: true,
        slots: &[("text", Slot::Text)],
        help: "the target shows a computed text: a template with {Var}, then where and a Prolog goal",
    },
    Command {
        name: "export",
        run: App::cmd_export,
        target: false,
        slots: &[("to", Slot::Text)],
        help: "export the outline, or one section, as a document (md, docx, odt, tex, html)",
    },
    Command {
        name: "fact",
        run: App::cmd_fact,
        target: true,
        slots: &[("that", Slot::Text)],
        help: "the target states a belief: a Prolog fact",
    },
    Command {
        name: "follow",
        run: App::cmd_follow,
        target: true,
        slots: &[("link", Slot::Word)],
        help: "go to the item at the end of the target's nth link",
    },
    Command {
        name: "help",
        run: App::cmd_help,
        target: false,
        slots: &[],
        help: "list every command",
    },
    Command {
        name: "import",
        run: App::cmd_import,
        target: false,
        slots: &[("from", Slot::Text)],
        help: "import wikidata, document or results, then a file: what it says arrives as claims",
    },
    Command {
        name: "indent",
        run: App::cmd_indent,
        target: true,
        slots: &[],
        help: "make the target a child of the item above it",
    },
    Command {
        name: "interrogate",
        run: App::cmd_interrogate,
        target: true,
        slots: &[],
        help: "ask of the target's claim: who else claims it, who disputes it, its source and record",
    },
    Command {
        name: "layer",
        run: App::cmd_layer,
        target: true,
        slots: &[("mark", Slot::Text)],
        help: "add a layer to the target's view: a mark, then where and a goal; one mark per solution",
    },
    Command {
        name: "link",
        run: App::cmd_link,
        target: true,
        slots: &[("from", Slot::Addr), ("to", Slot::Addr), ("as", Slot::Word)],
        help: "link one item to another; the type defaults to link",
    },
    Command {
        name: "move",
        run: App::cmd_move,
        target: true,
        slots: &[("after", Slot::Addr)],
        help: "move the target's branch to follow an item",
    },
    Command {
        name: "name",
        run: App::cmd_name,
        target: true,
        slots: &[("as", Slot::Word)],
        help: "give the target a name that addresses it",
    },
    Command {
        name: "new",
        run: App::cmd_new,
        target: false,
        slots: &[],
        help: "a new item after the target",
    },
    Command {
        name: "outdent",
        run: App::cmd_outdent,
        target: true,
        slots: &[],
        help: "make the target a sibling of its parent",
    },
    Command {
        name: "pane",
        run: App::cmd_pane,
        target: false,
        slots: &[("shows", Slot::Word)],
        help: "set the other pane to outline or links",
    },
    Command {
        name: "quit",
        run: App::cmd_quit,
        target: false,
        slots: &[],
        help: "write what is pending and leave",
    },
    Command {
        name: "redo",
        run: App::cmd_redo,
        target: false,
        slots: &[],
        help: "redo what undo undid, along the current branch",
    },
    Command {
        name: "rule",
        run: App::cmd_rule,
        target: false,
        slots: &[("that", Slot::Text)],
        help: "define a predicate from others: Head :- Body. Usable in derive, constraint, layer and other rules",
    },
    Command {
        name: "set",
        run: App::cmd_set,
        target: false,
        slots: &[("that", Slot::Text)],
        help: "state a fact and retract the beliefs it directly contradicts under a constraint; the fact goes where the replaced one was",
    },
    Command {
        name: "retract",
        run: App::cmd_retract,
        target: false,
        slots: &[("entry", Slot::Word)],
        help: "reverse one earlier entry, keeping later work; lists what depends on it first",
    },
    Command {
        name: "suppose",
        run: App::cmd_suppose,
        target: false,
        slots: &[("that", Slot::Text)],
        help: "start a named world with nothing supposed; hold Suppose to suppose a statement",
    },
    Command {
        name: "switch",
        run: App::cmd_switch,
        target: false,
        slots: &[],
        help: "move to the other pane",
    },
    Command {
        name: "transclude",
        run: App::cmd_transclude,
        target: true,
        slots: &[("from", Slot::Addr)],
        help: "show an item's text in place, in a new item after the target",
    },
    Command {
        name: "unadopt",
        run: App::cmd_unadopt,
        target: true,
        slots: &[],
        help: "retract the target's last adoption",
    },
    Command {
        name: "undo",
        run: App::cmd_undo,
        target: false,
        slots: &[],
        help: "undo the last change on this branch",
    },
    Command {
        name: "unfold",
        run: App::cmd_unfold,
        target: true,
        slots: &[],
        help: "show the target's branch",
    },
];

impl App {
    pub(crate) fn run_simple(&mut self, name: &str) {
        self.mode = Mode::Normal;
        let m = self.run_command(name);
        self.mode = m;
    }

    /// Run a command line, as typed in the Command quasimode.
    pub fn run_command(&mut self, line: &str) -> Mode {
        self.flush();
        let mode = match self.command(line.trim()) {
            Ok(mode) => mode,
            Err(m) => {
                self.message = m;
                Mode::Normal
            }
        };
        self.flush();
        self.update_frozen();
        mode
    }

    /// An item named by its outline number ("3.2"), its name, "#seq", or the
    /// start of its text if only one item's text starts so.
    pub fn resolve(&self, addr: &str) -> Option<u64> {
        let a = addr.trim();
        if a.is_empty() {
            return None;
        }
        if let Some(id) = a
            .strip_prefix('#')
            .and_then(|s| s.parse().ok())
            .filter(|i| self.outline.items.contains_key(i))
        {
            return Some(id);
        }
        if let Some(id) = self.outline.by_number(a) {
            return Some(id);
        }
        if let Some(id) = self
            .outline
            .names
            .get(a)
            .filter(|i| self.outline.items.contains_key(i))
        {
            return Some(*id);
        }
        let la = a.to_lowercase();
        let hits: Vec<u64> = self
            .outline
            .in_order()
            .into_iter()
            .filter(|id| self.outline.text(*id).to_lowercase().starts_with(&la))
            .collect();
        (hits.len() == 1).then(|| hits[0])
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

    pub(crate) fn command(&mut self, line: &str) -> Result<Mode, String> {
        let mut words: Vec<&str> = line.split_whitespace().collect();
        if words.is_empty() {
            return Ok(Mode::Normal);
        }
        let name = find_command(words[0])
            .ok_or_else(|| format!("no command “{}”; ? lists them", words[0]))?;
        words.remove(0);
        let cmd = COMMANDS.iter().find(|c| c.name == name).unwrap();
        let target = self.cursor();
        if cmd.target && target == TOP {
            return Err("there is no item to act on".into());
        }
        (cmd.run)(self, &words, target)?;
        if self.mode == Mode::Normal && self.view.scope != ScopeLevel::Statement {
            self.view.scope = ScopeLevel::Statement;
            self.view_dirty = true;
        }
        Ok(std::mem::replace(&mut self.mode, Mode::Normal))
    }

    /// The command line as shown: once the command is recognised, each slot
    /// is preceded by its guide word in parentheses ("link 3.2 (to) source 9").
    pub fn guided(&self, input: &str) -> String {
        let mut words: Vec<&str> = input.split_whitespace().collect();
        if words.is_empty() {
            return String::new();
        }
        let Some(name) = find_command(words[0])
            .filter(|n| *n == words[0].to_lowercase() || input.ends_with(' '))
        else {
            return input.to_string();
        };
        let cmd = COMMANDS.iter().find(|c| c.name == name).unwrap();
        words.remove(0);
        let open = input.ends_with(' ');
        if name == "link" {
            let (addr, kind) = match words.iter().position(|w| *w == "as") {
                Some(i) => (&words[..i], Some(words[i + 1..].join(" "))),
                None => (&words[..], None),
            };
            let shown = match self.addresses(addr, 2) {
                Some(_) => {
                    let k = (1..addr.len())
                        .find(|k| {
                            self.resolve(&addr[..*k].join(" ")).is_some()
                                && self.resolve(&addr[*k..].join(" ")).is_some()
                        })
                        .unwrap();
                    format!("link {} (to) {}", addr[..k].join(" "), addr[k..].join(" "))
                }
                None if addr.is_empty() => "link (to)".to_string(),
                None => format!("link (to) {}", addr.join(" ")),
            };
            return match kind {
                Some(k) => format!("{shown} (as) {k}"),
                None if open && self.addresses(addr, 2).is_some() => format!("{shown} (as)"),
                None => shown,
            };
        }
        let mut out = name.to_string();
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

pub(crate) fn find_command(w: &str) -> Option<&'static str> {
    let w = w.to_lowercase();
    if let Some(c) = COMMANDS.iter().find(|c| c.name == w) {
        return Some(c.name);
    }
    let hits: Vec<&Command> = COMMANDS.iter().filter(|c| c.name.starts_with(&w)).collect();
    (hits.len() == 1).then(|| hits[0].name)
}

/// Recognition on Escape: complete the command word if it is unambiguous.
pub(crate) fn complete_command(input: &str) -> String {
    let mut parts = input.splitn(2, ' ');
    let first = parts.next().unwrap_or("");
    let rest = parts.next();
    match find_command(first) {
        Some(name) if rest.is_none() => format!("{name} "),
        Some(name) => format!("{name} {}", rest.unwrap()),
        None => input.to_string(),
    }
}

/// What may come next in a command line, for `?`.
pub fn command_help(input: &str) -> Vec<String> {
    let first = input.split_whitespace().next().unwrap_or("");
    match find_command(first).filter(|_| input.contains(' ')) {
        Some(name) => {
            let c = COMMANDS.iter().find(|c| c.name == name).unwrap();
            let slots: Vec<String> = c.slots.iter().map(|(n, _)| format!("({n}) …")).collect();
            vec![format!("{name} {} — {}", slots.join(" "), c.help)]
        }
        None => COMMANDS
            .iter()
            .filter(|c| c.name.starts_with(&first.to_lowercase()))
            .map(|c| format!("{} — {}", c.name, c.help))
            .collect(),
    }
}
