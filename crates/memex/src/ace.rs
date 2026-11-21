//! Attempto Controlled English: its parser, APE, in a Prolog server of its
//! own (prolog/ace_server.pl), and the user lexicon a notebook gives it
//! (docs/translator.md).
//!
//! APE knows English through Clex; the notebook adds its own words: each
//! entity a proper name, each one-place fact's name an adjective
//! ("Buran is reusable"), each two-place fact's name a noun ("the date
//! of buran_flight is 1988"), and each label of one word a name for its
//! entity.

use crate::prolog::{Error, Prolog, prolog_file};
use crate::reason::{list, make_list, parts};
use format::terms::Term;
use std::collections::{BTreeMap, BTreeSet};

/// The parser: its server, and the user lexicon it holds.
pub struct Ace {
    p: Prolog,
    lexicon: Option<Vec<Term>>,
}

/// What APE made of a text: its paraphrase and logical form (APE's DRS)
/// when it is ACE, and APE's messages.
#[derive(Debug, Clone, PartialEq)]
pub struct Parse {
    pub ace: Option<(String, Term)>,
    pub messages: Vec<Message>,
}

/// What the notebook reads in a text (prolog/ace_read.pl): the statements
/// a text of ACE makes, or why it makes none.
#[derive(Debug, Clone, PartialEq)]
pub enum Reading {
    /// ACE, read whole: its paraphrase, and its statements.
    Read(String, Vec<Said>),
    /// ACE, but not read: its paraphrase, and why.
    Unread(String, Unread),
    /// Not ACE: APE's messages.
    NotAce(Vec<Message>),
}

/// Why a text of ACE is not read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unread {
    /// It states something of a thing with no name ("A fish is an
    /// animal": one fish, unnamed).
    Unnamed,
    /// Some of it is nothing the notebook states.
    Unstated,
}

/// A statement read: its kind, and the text of the command that states it.
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    pub kind: Kind,
    pub text: String,
}

/// What a statement is: a fact, a claim and its source, a rule ("Head :-
/// Body"), a constraint ("false :- Body") or a question.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Fact,
    Claim(String),
    Rule,
    Constraint,
    /// A question: a goal that binds Answer to its answer.
    Question,
}

/// A message of APE's: an error or a warning, its kind (word, sentence,
/// anaphor, lexicon, ...), and its note.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub error: bool,
    pub kind: String,
    pub note: Note,
}

/// Where a message is (the sentence and the token, from 1; 0 for none),
/// what it is about (a word, or the sentence with <> where parsing
/// stopped), and APE's advice.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub place: (u64, u64),
    pub subject: String,
    pub repair: String,
}

/// What a text being typed is: ACE, with its paraphrase, or not, and why.
#[derive(Debug, Clone, PartialEq)]
pub enum Check {
    Ace(String),
    Open(Opening),
    NotAce(Failure),
}

/// A text that is not ACE yet but can go on as ACE: what may come next
/// (those the word being typed begins), and the text with that word
/// completed, if one word alone fits.
#[derive(Debug, Clone, PartialEq)]
pub struct Opening {
    pub next: Vec<Next>,
    pub completed: Option<String>,
}

/// What may come next: a word of the notebook, a word of ACE (a function
/// word), or a kind of word (noun, name, verb, ...).
#[derive(Debug, Clone, PartialEq)]
pub enum Next {
    Own(String),
    Word(String),
    Kind(String),
}

/// Why a text is not ACE: the sentence marked where parsing stopped (<>),
/// APE's other messages, and up to three repairs.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub at: String,
    pub notes: Vec<Message>,
    pub repairs: Vec<Repair>,
}

/// An edit of one word that makes a text ACE: the word, the word for its
/// place, and the text so mended.
#[derive(Debug, Clone, PartialEq)]
pub struct Repair {
    pub from: String,
    pub to: String,
    pub mended: String,
}

impl Check {
    /// The text mended by the first repair, if there is one.
    pub fn mended(&self) -> Option<String> {
        match self {
            Check::Ace(_) => None,
            Check::Open(o) => o.completed.clone(),
            Check::NotAce(f) => f.repairs.first().map(|r| r.mended.clone()),
        }
    }
}

impl Note {
    /// The note in words: its subject, then APE's advice.
    fn said(&self) -> String {
        format!("{}: {}", self.subject, self.repair)
    }
}

impl Ace {
    /// Start the parser. APE is found through APE_HOME, which the Nix shell
    /// sets; without it there is no parser, and the error says so.
    pub fn start() -> Result<Ace, Error> {
        if std::env::var_os("APE_HOME").is_none() {
            return Err(Error::Server(NO_APE.into()));
        }
        let p = Prolog::start_with(&prolog_file("ace_server.pl"))?;
        Ok(Ace { p, lexicon: None })
    }

    /// Make the user lexicon these entries, unless it is already; the
    /// messages APE gave about them (a word defined twice, say).
    pub fn set_lexicon(&mut self, entries: Vec<Term>) -> Result<Vec<Message>, Error> {
        if self.lexicon.as_ref() == Some(&entries) {
            return Ok(vec![]);
        }
        let goal = call(
            "ace_lexicon",
            vec![make_list(entries.clone()), Term::Var(0)],
        );
        let told = self.only_answer(&goal)?;
        self.lexicon = Some(entries);
        messages(&told)
    }

    /// Whether its server is dead (Prolog::is_dead).
    pub fn is_dead(&mut self) -> bool {
        self.p.is_dead()
    }

    /// What APE makes of a text, with this user lexicon; APE's messages
    /// about the lexicon, if it changed, come first.
    pub fn parse_with(&mut self, lexicon: Vec<Term>, text: &str) -> Result<Parse, Error> {
        let told = self.set_lexicon(lexicon)?;
        let mut parse = self.parse(text)?;
        parse.messages.splice(0..0, told);
        Ok(parse)
    }

    /// What a text being typed is, with this user lexicon (ace_check/2):
    /// ACE, or where it fails, and repairs.
    pub fn check_with(&mut self, lexicon: Vec<Term>, text: &str) -> Result<Check, Error> {
        self.set_lexicon(lexicon)?;
        let goal = call("ace_check", vec![Term::Str(text.into()), Term::Var(0)]);
        check_of(&self.only_answer(&goal)?)
    }

    /// What may come next in a text, with this user lexicon (ace_next/2).
    pub fn next_with(&mut self, lexicon: Vec<Term>, text: &str) -> Result<Vec<Next>, Error> {
        self.set_lexicon(lexicon)?;
        let goal = call("ace_next", vec![Term::Str(text.into()), Term::Var(0)]);
        list(&self.only_answer(&goal)?)
            .iter()
            .map(next_of)
            .collect()
    }

    /// What the notebook reads in a text, with this user lexicon
    /// (ace_read_text/2).
    pub fn read_with(&mut self, words: Words, text: &str) -> Result<Reading, Error> {
        self.set_lexicon(words.lexicon)?;
        let names = |n: Vec<String>| make_list(n.into_iter().map(Term::Atom).collect());
        let words = call("words", vec![names(words.rules), names(words.boxes)]);
        let goal = call(
            "ace_read_text",
            vec![Term::Str(text.into()), words, Term::Var(0)],
        );
        reading_of(&self.only_answer(&goal)?)
    }

    /// What APE makes of a text.
    pub fn parse(&mut self, text: &str) -> Result<Parse, Error> {
        let goal = call("ace_parse", vec![Term::Str(text.into()), Term::Var(0)]);
        parse_of(&self.only_answer(&goal)?)
    }

    /// The value of a goal's one variable, which it must give.
    fn only_answer(&mut self, goal: &Term) -> Result<Term, Error> {
        let answers = self.p.all(goal, 1)?;
        let value = answers
            .into_iter()
            .next()
            .and_then(|v| v.into_iter().next());
        value.ok_or_else(|| Error::Server(format!("APE gave no answer to {goal:?}")))
    }
}

const NO_APE: &str = "APE is not installed: APE_HOME is not set (the Nix shell sets it)";

fn call(f: &str, args: Vec<Term>) -> Term {
    Term::Compound(f.into(), args)
}

fn bad(what: &str, t: &Term) -> Error {
    Error::Server(format!("APE gave a {what} not understood: {t:?}"))
}

/// A parse, from parsed(Paraphrase, Drs, Messages) or not_ace(Messages).
fn parse_of(t: &Term) -> Result<Parse, Error> {
    if let Some(a) = parts(t, "parsed", 3) {
        return parsed(a);
    }
    let a = parts(t, "not_ace", 1).ok_or_else(|| bad("parse", t))?;
    let messages = messages(&a[0])?;
    Ok(Parse {
        ace: None,
        messages,
    })
}

/// A reading, from read(Paraphrase, Statements, Messages),
/// unread(Paraphrase, Why, Messages) or not_ace(Messages).
fn reading_of(t: &Term) -> Result<Reading, Error> {
    if let Some(a) = parts(t, "read", 3) {
        let said = list(&a[1]).iter().map(said_of).collect::<Result<_, _>>()?;
        return Ok(Reading::Read(text(&a[0])?, said));
    }
    if let Some(a) = parts(t, "unread", 3) {
        return Ok(Reading::Unread(text(&a[0])?, why_of(&a[1])?));
    }
    let a = parts(t, "not_ace", 1).ok_or_else(|| bad("reading", t))?;
    Ok(Reading::NotAce(messages(&a[0])?))
}

/// Why a text is not read: unnamed or unstated.
fn why_of(t: &Term) -> Result<Unread, Error> {
    match t {
        Term::Atom(w) if w == "unnamed" => Ok(Unread::Unnamed),
        Term::Atom(w) if w == "unstated" => Ok(Unread::Unstated),
        _ => Err(bad("reason", t)),
    }
}

/// A statement, from said(Kind, Text).
fn said_of(t: &Term) -> Result<Said, Error> {
    let a = parts(t, "said", 2).ok_or_else(|| bad("statement", t))?;
    let (kind, text) = (kind_of(&a[0])?, text(&a[1])?);
    Ok(Said { kind, text })
}

/// A statement's kind: fact, claim(Source), rule or constraint.
fn kind_of(t: &Term) -> Result<Kind, Error> {
    if let Some(a) = parts(t, "claim", 1) {
        return Ok(Kind::Claim(text(&a[0])?));
    }
    match t {
        Term::Atom(k) if k == "fact" => Ok(Kind::Fact),
        Term::Atom(k) if k == "rule" => Ok(Kind::Rule),
        Term::Atom(k) if k == "constraint" => Ok(Kind::Constraint),
        Term::Atom(k) if k == "question" => Ok(Kind::Question),
        _ => Err(bad("kind of statement", t)),
    }
}

/// A parse of ACE, from the arguments of parsed/3.
fn parsed(a: &[Term]) -> Result<Parse, Error> {
    let ace = Some((text(&a[0])?, a[1].clone()));
    let messages = messages(&a[2])?;
    Ok(Parse { ace, messages })
}

/// A check, from ace(Paraphrase) or not_ace(Failure, Notes, Repairs).
fn check_of(t: &Term) -> Result<Check, Error> {
    if let Some(a) = parts(t, "ace", 1) {
        return Ok(Check::Ace(text(&a[0])?));
    }
    if let Some(a) = parts(t, "open", 2) {
        return opening(a).map(Check::Open);
    }
    let a = parts(t, "not_ace", 3).ok_or_else(|| bad("check", t))?;
    let (at, notes) = (text(&a[0])?, messages(&a[1])?);
    let repairs = list(&a[2]).iter().map(repair).collect::<Result<_, _>>()?;
    Ok(Check::NotAce(Failure { at, notes, repairs }))
}

/// An opening, from the arguments of open(Next, Completed); "" is no
/// completion.
fn opening(a: &[Term]) -> Result<Opening, Error> {
    let next = list(&a[0]).iter().map(next_of).collect::<Result<_, _>>()?;
    let completed = Some(text(&a[1])?).filter(|c| !c.is_empty());
    Ok(Opening { next, completed })
}

/// What may come next, from own(Form), word(Form) or kind(Name).
fn next_of(t: &Term) -> Result<Next, Error> {
    if let Some(a) = parts(t, "own", 1) {
        return Ok(Next::Own(text(&a[0])?));
    }
    if let Some(a) = parts(t, "word", 1) {
        return Ok(Next::Word(text(&a[0])?));
    }
    let a = parts(t, "kind", 1).ok_or_else(|| bad("next word", t))?;
    Ok(Next::Kind(text(&a[0])?))
}

/// A repair, from repair(From, To, Mended).
fn repair(t: &Term) -> Result<Repair, Error> {
    let a = parts(t, "repair", 3).ok_or_else(|| bad("repair", t))?;
    let (from, to) = (text(&a[0])?, text(&a[1])?);
    Ok(Repair {
        from,
        to,
        mended: text(&a[2])?,
    })
}

fn messages(t: &Term) -> Result<Vec<Message>, Error> {
    list(t).iter().map(message).collect()
}

/// A message, from message(Importance, Type, Sentence, Token, Subject,
/// Repair).
fn message(t: &Term) -> Result<Message, Error> {
    let a = parts(t, "message", 6).ok_or_else(|| bad("message", t))?;
    let note = note(&a[2..])?;
    let error = a[0] == atom("error");
    Ok(Message {
        error,
        kind: text(&a[1])?,
        note,
    })
}

/// A message's note, from its last four arguments: Sentence, Token,
/// Subject, Repair.
fn note(a: &[Term]) -> Result<Note, Error> {
    let place = (number(&a[0])?, number(&a[1])?);
    let (subject, repair) = (text(&a[2])?, text(&a[3])?);
    Ok(Note {
        place,
        subject,
        repair,
    })
}

/// The text of a string or an atom.
fn text(t: &Term) -> Result<String, Error> {
    match t {
        Term::Str(s) | Term::Atom(s) => Ok(s.clone()),
        _ => Err(bad("text", t)),
    }
}

/// A count of APE's, from 0.
fn number(t: &Term) -> Result<u64, Error> {
    match t {
        Term::Int(n) if *n >= 0 => Ok(*n as u64),
        _ => Err(bad("number", t)),
    }
}

/// A parse in words: its paraphrase, or "not ACE" and where it failed;
/// then each other message, its subject and APE's advice.
pub fn said(parse: &Parse) -> String {
    let (head, rest): (String, Vec<&Message>) = match &parse.ace {
        Some((paraphrase, _)) => (
            format!("ACE: {paraphrase}"),
            parse.messages.iter().collect(),
        ),
        None => failed(&parse.messages),
    };
    let told = rest.iter().map(|m| m.note.said());
    std::iter::once(head)
        .chain(told)
        .collect::<Vec<_>>()
        .join(" · ")
}

/// "not ACE" and the sentence marked where parsing stopped, if APE said
/// where; and the other messages.
fn failed(messages: &[Message]) -> (String, Vec<&Message>) {
    let at = |m: &&Message| m.error && m.kind == "sentence";
    let place = messages.iter().find(at);
    let head = match place {
        Some(m) => format!("not ACE: {}", m.note.subject),
        None => "not ACE".to_string(),
    };
    (head, messages.iter().filter(|m| !at(m)).collect())
}

/// A check as a hint while typing: ACE and its paraphrase; or "not ACE",
/// where, and the first repair, which Tab applies, with the others; or,
/// with no repair, why: each word APE does not know.
pub fn hint(check: &Check) -> String {
    match check {
        Check::Ace(paraphrase) => format!("ACE: {paraphrase}"),
        Check::Open(o) => going_on(o),
        Check::NotAce(f) => format!("not ACE: {} · {}", f.at, mend_or_why(f)),
    }
}

/// What may come next, the first twelve, and the word Tab completes.
fn going_on(o: &Opening) -> String {
    let shown: Vec<String> = o.next.iter().take(12).map(Next::said).collect();
    let more = if o.next.len() > 12 { ", …" } else { "" };
    let next = format!("next: {}{more}", shown.join(", "));
    match o
        .completed
        .as_deref()
        .and_then(|c| c.split_whitespace().last())
    {
        Some(word) => format!("{next} · Tab: {word}"),
        None => next,
    }
}

impl Next {
    /// A word as it is; a kind with its article: "a noun", "an adverb".
    fn said(&self) -> String {
        match self {
            Next::Own(w) | Next::Word(w) => w.clone(),
            Next::Kind(k) if k.starts_with(['a', 'e', 'i', 'o', 'u']) => format!("an {k}"),
            Next::Kind(k) => format!("a {k}"),
        }
    }
}

/// What may come next, listed with a line of help (`?` in Suppose): the
/// notebook's words, each kind of word with what it is, and the words of
/// ACE, ten to a line.
pub fn listing(next: &[Next]) -> Vec<String> {
    if next.is_empty() {
        return vec!["nothing may come next: the text cannot go on as ACE".into()];
    }
    let mut lines = vec![format!("WHAT MAY COME NEXT · {}", next.len())];
    lines.extend(words_line("from your notebook", next, |n| {
        matches!(n, Next::Own(_))
    }));
    lines.extend(next.iter().filter_map(kind_line));
    let words: Vec<String> = next
        .iter()
        .filter(|n| matches!(n, Next::Word(_)))
        .map(Next::said)
        .collect();
    lines.extend(
        words
            .chunks(10)
            .map(|c| format!("words of ACE: {}", c.join(", "))),
    );
    lines
}

/// The words of one sort, on one line after a label, if there are any.
fn words_line(label: &str, next: &[Next], sort: fn(&Next) -> bool) -> Option<String> {
    let words: Vec<String> = next.iter().filter(|n| sort(n)).map(Next::said).collect();
    (!words.is_empty()).then(|| format!("{label}: {}", words.join(", ")))
}

/// A kind of word, with a line of help: what it is, and examples.
fn kind_line(n: &Next) -> Option<String> {
    let Next::Kind(k) = n else { return None };
    let help = match k.as_str() {
        "noun" => "a thing of a kind: a flight, the date of ...",
        "unit" => "a unit after a number: 3 dollars",
        "name" => "one thing by its name: John, buran_flight",
        "verb" => "what something does, in the present: waits, owns",
        "participle" => "after is: is bought",
        "adjective" => "a quality: is green, a green park",
        "adverb" => "how something is done: quickly",
        "preposition" => "how two things relate: in, of, near",
        "number" => "a number: 3, 1989, 2.5",
        "variable" => "a thing named to be named again: X, Y1",
        _ => "",
    };
    Some(format!("{} — {help}", n.said()))
}

/// The repairs, the first with the key that applies it; or, with none,
/// the words APE does not know.
fn mend_or_why(f: &Failure) -> String {
    let Some((first, rest)) = f.repairs.split_first() else {
        return unknown_words(&f.notes);
    };
    let others = rest.iter().map(|r| format!("{} → {}", r.from, r.to));
    let others = others.collect::<Vec<_>>().join(", ");
    let also = if others.is_empty() {
        String::new()
    } else {
        format!(" (or {others})")
    };
    format!("Tab: {} → {}{also}", first.from, first.to)
}

/// The words APE does not know, said so; else that no one-word edit
/// makes it ACE.
fn unknown_words(notes: &[Message]) -> String {
    let unknown = notes.iter().filter(|m| m.error && m.kind == "word");
    let words: Vec<String> = unknown.map(|m| m.note.subject.clone()).collect();
    match words.as_slice() {
        [] => "no edit of one word makes it ACE".into(),
        _ => format!("words ACE does not know: {}", words.join(", ")),
    }
}

/// What a world gives the reader: its lexicon, the names of its rules of
/// one argument, whose values the reader reads in place of boxes, and the
/// names of its boxes, which a question asks the value of.
pub struct Words {
    pub lexicon: Vec<Term>,
    pub rules: Vec<String>,
    pub boxes: Vec<String>,
}

/// The words a world's facts give the reader.
pub fn words_of(facts: &[(u64, Term)]) -> Words {
    let names = |pick: fn(&Term) -> Option<String>| {
        let names: BTreeSet<String> = facts.iter().filter_map(|(_, f)| pick(f)).collect();
        names.into_iter().collect()
    };
    Words {
        lexicon: lexicon_of(facts),
        rules: names(one_place_rule),
        boxes: names(box_name),
    }
}

/// The name of a box a fact gives a value: cell(Name, Value).
fn box_name(fact: &Term) -> Option<String> {
    match fact {
        Term::Compound(f, a) if f == "cell" && a.len() == 2 => match &a[0] {
            Term::Atom(name) => Some(name.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// The name of a rule that defines a value of one argument.
fn one_place_rule(fact: &Term) -> Option<String> {
    let Term::Compound(f, a) = fact else {
        return None;
    };
    match (f.as_str(), a.get(1)) {
        ("rule_def", Some(Term::Compound(head, args))) if args.len() == 1 => Some(head.clone()),
        _ => None,
    }
}

/// The user lexicon a world's facts give, each entry once, in order.
pub fn lexicon_of(facts: &[(u64, Term)]) -> Vec<Term> {
    let mut entries = BTreeMap::new();
    for (_, f) in facts {
        for e in entries_of(f) {
            entries.insert(format!("{e:?}"), e);
        }
    }
    entries.into_values().collect()
}

/// The entries one fact gives: a claim's as its claimed fact's.
fn entries_of(fact: &Term) -> Vec<Term> {
    match fact {
        Term::Compound(f, a) if f == "claims" && a.len() == 2 => entries_of(&a[1]),
        Term::Compound(f, a) if f == "label" && a.len() == 2 => {
            label_entry(&a[0], &a[1]).into_iter().collect()
        }
        Term::Compound(f, _) if f == "node_def" || f == "rule_def" => vec![],
        Term::Compound(f, a) => fact_entries(f, a),
        _ => vec![],
    }
}

/// A fact's name as a word of its kind, and each of its entities as a
/// proper name.
fn fact_entries(name: &str, args: &[Term]) -> Vec<Term> {
    let kind = match args.len() {
        1 => Some(call("adj_itr", vec![atom(name), atom(name)])),
        2 => Some(call("noun_sg", vec![atom(name), atom(name), atom("neutr")])),
        _ => None,
    };
    let names = args.iter().filter_map(entity_name);
    kind.filter(|_| is_word(name))
        .into_iter()
        .chain(names)
        .collect()
}

/// An entity as a proper name for itself, if its name is one word.
fn entity_name(t: &Term) -> Option<Term> {
    let Term::Atom(a) = t else { return None };
    is_word(a).then(|| proper_name(a, a))
}

/// A label as a proper name for its entity, its spaces made hyphens, if
/// that is one word: "Energia rocket" names energia as Energia-rocket.
fn label_entry(of: &Term, text: &Term) -> Option<Term> {
    let (Term::Atom(e), Term::Str(s)) = (of, text) else {
        return None;
    };
    let word = s.split_whitespace().collect::<Vec<_>>().join("-");
    is_word(&word).then(|| proper_name(&word, e))
}

fn proper_name(word: &str, entity: &str) -> Term {
    call("pn_sg", vec![atom(word), atom(entity), atom("neutr")])
}

fn atom(s: &str) -> Term {
    Term::Atom(s.into())
}

/// Whether a name can be one word of ACE: a letter, then letters, digits,
/// underscores and hyphens, in ASCII.
fn is_word(s: &str) -> bool {
    let mut chars = s.chars();
    let first = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    first && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// said(Kind, Text), as ace_read_text/2 gives a statement.
    fn said_term(kind: Term, text: &str) -> Term {
        c("said", vec![kind, Term::Str(text.into())])
    }

    /// A reading of these statements.
    fn read_term(statements: Vec<Term>) -> Term {
        c(
            "read",
            vec![Term::Str("p".into()), make_list(statements), atom("[]")],
        )
    }

    #[test]
    fn each_kind_of_statement_is_read_as_its_kind_with_its_text() {
        // Catches a claim's source lost, one kind of statement taken for
        // another, or a statement's text lost.
        let source = c("claim", vec![Term::Str("Talk-notes".into())]);
        let kinds = [atom("fact"), source, atom("rule"), atom("constraint")];
        let read = read_term(kinds.into_iter().map(|k| said_term(k, "t")).collect());
        let Ok(Reading::Read(_, said)) = reading_of(&read) else {
            panic!("not read: {read:?}");
        };
        let claim = Kind::Claim("Talk-notes".into());
        let made = |kind| Said {
            kind,
            text: "t".into(),
        };
        let expected = [Kind::Fact, claim, Kind::Rule, Kind::Constraint].map(made);
        assert_eq!(said, expected);
    }

    #[test]
    fn a_text_left_unread_and_a_statement_of_no_kind_are_told_apart() {
        // Catches an unread text taken for one read, or a statement of an
        // unknown kind let through.
        let unread = |why| {
            let paraphrase = Term::Str("There is a flight.".into());
            c("unread", vec![paraphrase, atom(why), atom("[]")])
        };
        let flight = |why| Reading::Unread("There is a flight.".into(), why);
        assert_eq!(
            reading_of(&unread("unnamed")).unwrap(),
            flight(Unread::Unnamed)
        );
        assert_eq!(
            reading_of(&unread("unstated")).unwrap(),
            flight(Unread::Unstated)
        );
        assert!(reading_of(&unread("odd")).is_err());
        assert!(reading_of(&read_term(vec![said_term(atom("odd"), "x")])).is_err());
    }

    fn c(f: &str, args: Vec<Term>) -> Term {
        call(f, args)
    }

    fn shown(facts: &[Term]) -> Vec<String> {
        let numbered: Vec<(u64, Term)> = facts.iter().cloned().map(|f| (1, f)).collect();
        lexicon_of(&numbered)
            .iter()
            .map(crate::reason::show)
            .collect()
    }

    #[test]
    fn a_fact_names_its_entities_and_its_relation() {
        // Catches a world's entities left out of the lexicon, or a
        // two-place fact's name not made a noun.
        let date = c("date", vec![atom("buran_flight"), Term::Int(1988)]);
        assert_eq!(
            shown(&[date]),
            [
                "noun_sg(date, date, neutr)",
                "pn_sg(buran_flight, buran_flight, neutr)"
            ]
        );
    }

    #[test]
    fn a_one_place_fact_names_an_adjective() {
        // Catches "Buran is reusable" left without its adjective.
        let reusable = c("reusable", vec![atom("buran")]);
        assert_eq!(
            shown(&[reusable]),
            ["adj_itr(reusable, reusable)", "pn_sg(buran, buran, neutr)"]
        );
    }

    #[test]
    fn a_claim_gives_the_words_of_what_it_claims() {
        // Catches a claim's fact left out, or its source taken as an entity.
        let p = c("reusable", vec![atom("buran")]);
        let claim = c("claims", vec![Term::Str("my notes".into()), p]);
        assert_eq!(
            shown(&[claim]),
            ["adj_itr(reusable, reusable)", "pn_sg(buran, buran, neutr)"]
        );
    }

    #[test]
    fn a_label_of_one_word_names_its_entity() {
        // Catches a label not used, or a label of words that cannot be one
        // word of ACE (an apostrophe) taken as one.
        let label = |s: &str| c("label", vec![atom("energia"), Term::Str(s.into())]);
        assert_eq!(
            shown(&[label("Energia rocket")]),
            ["pn_sg(Energia-rocket, energia, neutr)"]
        );
        assert_eq!(shown(&[label("Buran's flight")]), Vec::<String>::new());
    }

    #[test]
    fn only_names_that_can_be_words_enter() {
        // Catches a name ACE cannot read as one word: one with a space, one
        // starting with a digit, the empty name.
        let odd = c("p", vec![atom("two words"), atom("1st")]);
        let empty = c("q", vec![atom("")]);
        assert_eq!(
            shown(&[odd, empty]),
            ["adj_itr(q, q)", "noun_sg(p, p, neutr)"]
        );
    }

    #[test]
    fn facts_of_three_places_and_definitions_give_no_word_of_their_own() {
        // Catches a three-place fact's name or a definition taken as a word.
        let three = c("task", vec![atom("design"), Term::Int(1), Term::Int(4)]);
        let def = c("node_def", vec![Term::Int(3), atom("q")]);
        assert_eq!(shown(&[three, def]), ["pn_sg(design, design, neutr)"]);
    }

    #[test]
    fn each_entry_is_given_once() {
        // Catches an entry repeated for each fact that gives it, which APE
        // reports as a word defined twice.
        let a = c("date", vec![atom("buran_flight"), Term::Int(1988)]);
        let b = c("date", vec![atom("buran_flight"), Term::Int(1989)]);
        assert_eq!(shown(&[a, b]).len(), 2);
    }

    /// A message as ace.pl gives it, of its importance and kind, and its
    /// subject and advice: of sentence 1, at no token.
    fn told_term(head: [&str; 2], note: [&str; 2]) -> Term {
        let place = [Term::Int(1), Term::Int(0)];
        let note = note.map(|s| Term::Str(s.into()));
        c("message", [head.map(atom), place, note].concat())
    }

    fn told(head: [&str; 2], note: [&str; 2]) -> Message {
        message(&told_term(head, note)).unwrap()
    }

    #[test]
    fn a_sentence_not_ace_is_said_with_where_it_failed_and_the_advice() {
        // Catches the place of failure or a word's advice left unsaid, or
        // the sentence's own message said twice.
        let parse = Parse {
            ace: None,
            messages: vec![
                told(["error", "word"], ["was", "as"]),
                told(
                    ["error", "sentence"],
                    ["The flight <> was in 1989.", "This is the first ..."],
                ),
            ],
        };
        assert_eq!(
            said(&parse),
            "not ACE: The flight <> was in 1989. · was: as"
        );
    }

    #[test]
    fn a_sentence_of_ace_is_said_by_its_paraphrase_and_warnings() {
        // Catches a warning left unsaid beside the paraphrase.
        let ace = Some(("Buran waits.".to_string(), atom("drs")));
        let warned = told(["warning", "word"], ["Buran", "Undefined word."]);
        let parse = Parse {
            ace,
            messages: vec![warned],
        };
        assert_eq!(said(&parse), "ACE: Buran waits. · Buran: Undefined word.");
        let bare = Parse {
            ace: None,
            messages: vec![],
        };
        assert_eq!(said(&bare), "not ACE");
    }

    fn failure(repairs: Vec<Repair>, notes: Vec<Message>) -> Check {
        let at = "The flight <> was in 1989.".to_string();
        Check::NotAce(Failure { at, notes, repairs })
    }

    fn mend(from: &str, to: &str) -> Repair {
        let mended = format!("The flight {to} in 1989");
        Repair {
            from: from.into(),
            to: to.into(),
            mended,
        }
    }

    #[test]
    fn a_hint_gives_the_first_repair_with_its_key_and_the_others() {
        // Catches the key that applies a repair left unsaid, or the other
        // repairs dropped.
        let one = failure(vec![mend("was", "is")], vec![]);
        assert_eq!(
            hint(&one),
            "not ACE: The flight <> was in 1989. · Tab: was → is"
        );
        let two = failure(vec![mend("was", "is"), mend("was", "has")], vec![]);
        assert!(hint(&two).ends_with("Tab: was → is (or was → has)"));
        assert_eq!(hint(&Check::Ace("John waits.".into())), "ACE: John waits.");
    }

    #[test]
    fn a_hint_without_a_repair_says_why() {
        // Catches a failure with no repair said with no reason.
        let unknown = told(
            ["error", "word"],
            ["Qzx", "Use the prefix n:, v:, a: or p:."],
        );
        let shown = hint(&failure(vec![], vec![unknown]));
        assert!(shown.ends_with("words ACE does not know: Qzx"), "{shown}");
        let bare = hint(&failure(vec![], vec![]));
        assert!(bare.ends_with("no edit of one word makes it ACE"), "{bare}");
    }

    fn opened(next: Vec<Next>, completed: Option<&str>) -> Check {
        let completed = completed.map(String::from);
        Check::Open(Opening { next, completed })
    }

    #[test]
    fn an_opening_shows_what_may_come_next_and_what_esc_completes() {
        // Catches a kind shown without its article, more than twelve shown
        // without saying so, or the completion unsaid or not applied.
        let words = vec![
            Next::Word("buran_flight".into()),
            Next::Kind("adverb".into()),
        ];
        let o = opened(words, Some("The date of buran_flight "));
        assert_eq!(
            hint(&o),
            "next: buran_flight, an adverb · Tab: buran_flight"
        );
        assert_eq!(o.mended().as_deref(), Some("The date of buran_flight "));
        let many = (0..13).map(|i| Next::Word(format!("w{i}"))).collect();
        assert!(hint(&opened(many, None)).ends_with("w11, …"));
        assert_eq!(opened(vec![Next::Kind("noun".into())], None).mended(), None);
    }

    #[test]
    fn a_listing_gives_each_sort_its_line_and_each_kind_its_help() {
        // Catches the notebook's words mixed with ACE's, a kind without its
        // help, or a long run of words on one line.
        let mut next = vec![Next::Own("buran_flight".into()), Next::Kind("name".into())];
        next.extend((0..12).map(|i| Next::Word(format!("w{i}"))));
        let lines = listing(&next);
        assert_eq!(lines[0], "WHAT MAY COME NEXT · 14");
        assert_eq!(lines[1], "from your notebook: buran_flight");
        assert_eq!(
            lines[2],
            "a name — one thing by its name: John, buran_flight"
        );
        assert!(lines[3].starts_with("words of ACE: w0,") && lines[3].ends_with("w9"));
        assert_eq!(lines[4], "words of ACE: w10, w11");
        assert_eq!(
            listing(&[]),
            ["nothing may come next: the text cannot go on as ACE"]
        );
    }

    #[test]
    fn an_opening_is_read() {
        // Catches the next words or kinds misread, or an empty completion
        // taken for one.
        let next = make_list(vec![
            c("own", vec![atom("date")]),
            c("word", vec![atom("of")]),
            c("kind", vec![atom("noun")]),
        ]);
        let t = c("open", vec![next, Term::Str(String::new())]);
        let want = vec![
            Next::Own("date".into()),
            Next::Word("of".into()),
            Next::Kind("noun".into()),
        ];
        assert_eq!(check_of(&t).unwrap(), opened(want, None));
        let odd = c(
            "open",
            vec![make_list(vec![atom("of")]), Term::Str(String::new())],
        );
        assert!(check_of(&odd).is_err());
    }

    #[test]
    fn a_check_and_its_repairs_are_read() {
        // Catches a repair's words or its mended text misread, or a term
        // of another shape taken for a check.
        let r = c(
            "repair",
            ["was", "is", "The flight is in 1989"]
                .map(|s| Term::Str(s.into()))
                .to_vec(),
        );
        let t = c(
            "not_ace",
            vec![Term::Str("at".into()), atom("[]"), make_list(vec![r])],
        );
        assert_eq!(
            check_of(&t).unwrap(),
            failure_at("at", vec![mend("was", "is")])
        );
        assert!(check_of(&atom("ace")).is_err());
    }

    fn failure_at(at: &str, repairs: Vec<Repair>) -> Check {
        Check::NotAce(Failure {
            at: at.into(),
            notes: vec![],
            repairs,
        })
    }

    #[test]
    fn a_parse_and_its_messages_are_read() {
        // Catches a paraphrase, a place or a message's texts misread, or
        // a term of another shape taken for a parse.
        let t = c(
            "not_ace",
            vec![make_list(vec![told_term(["error", "word"], ["was", "as"])])],
        );
        let parse = parse_of(&t).unwrap();
        assert_eq!(parse.ace, None);
        let m = &parse.messages[0];
        assert_eq!(
            (m.error, m.kind.as_str(), m.note.place),
            (true, "word", (1, 0))
        );
        assert_eq!(
            (m.note.subject.as_str(), m.note.repair.as_str()),
            ("was", "as")
        );
        assert!(parse_of(&atom("parsed")).is_err());
    }

    #[test]
    fn a_parse_of_ace_keeps_its_paraphrase_and_logical_form() {
        // Catches the paraphrase or the DRS lost, or a paraphrase that is
        // not text accepted.
        let drs = c("drs", vec![atom("[]"), atom("[]")]);
        let ok = c(
            "parsed",
            vec![Term::Str("John waits.".into()), drs.clone(), atom("[]")],
        );
        assert_eq!(
            parse_of(&ok).unwrap().ace,
            Some(("John waits.".into(), drs.clone()))
        );
        let bad = c("parsed", vec![Term::Int(1), drs, atom("[]")]);
        assert!(parse_of(&bad).is_err());
    }
}
