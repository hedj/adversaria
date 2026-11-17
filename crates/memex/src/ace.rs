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
use std::collections::BTreeMap;

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

/// A parse of ACE, from the arguments of parsed/3.
fn parsed(a: &[Term]) -> Result<Parse, Error> {
    let ace = Some((text(&a[0])?, a[1].clone()));
    let messages = messages(&a[2])?;
    Ok(Parse { ace, messages })
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
