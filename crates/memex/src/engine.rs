//! A world's reasoning: its facts loaded into a SWI-Prolog process, and the
//! values of its dataflow nodes (prolog/dataflow.pl).
//!
//! Facts are synced as a difference: those gone are retracted and those new
//! asserted, so SWI-Prolog's incremental tabling recomputes only the nodes
//! whose facts changed.

use crate::prolog::{Error, Prolog};
use crate::reason::{Suggestion, list, show};
use format::terms::Term;
use std::collections::BTreeMap;

pub struct Engine {
    p: Prolog,
    /// The facts loaded, by the number of the entry or item each is of.
    loaded: BTreeMap<u64, Vec<Term>>,
    /// The import entry whose Wikidata subset is loaded.
    wikidata: Option<u64>,
}

fn c(f: &str, args: Vec<Term>) -> Term {
    Term::Compound(f.into(), args)
}

/// Facts by the number of the entry or item each is of.
type Groups<'a> = BTreeMap<u64, Vec<&'a Term>>;

/// Facts grouped by their numbers, in the order given within each.
fn grouped(facts: &[(u64, Term)]) -> Groups<'_> {
    let mut groups: Groups = BTreeMap::new();
    for (seq, term) in facts {
        groups.entry(*seq).or_default().push(term);
    }
    groups
}

/// Whether a loaded group is the wanted one, term for term.
fn same(held: &[Term], wanted: Option<&Vec<&Term>>) -> bool {
    let wanted = wanted.map_or(&[][..], Vec::as_slice);
    held.len() == wanted.len() && held.iter().zip(wanted).all(|(h, w)| h == *w)
}

/// The variables of a term, each once, in order of first appearance.
pub fn variables(t: &Term) -> Vec<u64> {
    fn walk(t: &Term, out: &mut Vec<u64>) {
        match t {
            Term::Var(v) if !out.contains(v) => out.push(*v),
            Term::Compound(_, args) => args.iter().for_each(|a| walk(a, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(t, &mut out);
    out
}

impl Engine {
    pub fn start() -> Result<Engine, Error> {
        Ok(Engine {
            p: Prolog::start()?,
            loaded: BTreeMap::new(),
            wikidata: None,
        })
    }

    /// Whether its server is dead (Prolog::is_dead).
    pub fn is_dead(&mut self) -> bool {
        self.p.is_dead()
    }

    /// Make the loaded facts equal these, by difference. Each change is
    /// recorded as it is made, so a fact the server refuses leaves the
    /// record true of what the server holds, and a retry repeats nothing.
    pub fn sync(&mut self, facts: &[(u64, Term)]) -> Result<(), Error> {
        let wanted = grouped(facts);
        self.retract_unwanted(&wanted)?;
        self.assert_missing(&wanted)
    }

    /// Retract each loaded fact that is no longer wanted. Groups that are
    /// as wanted, nearly all of them, are passed over by comparing them.
    fn retract_unwanted(&mut self, wanted: &Groups) -> Result<(), Error> {
        let differ = |(seq, held): (&u64, &Vec<Term>)| !same(held, wanted.get(seq));
        let changed: Vec<u64> = self
            .loaded
            .iter()
            .filter(|g| differ(*g))
            .map(|(s, _)| *s)
            .collect();
        for seq in changed {
            let keep = wanted.get(&seq).map_or(&[][..], Vec::as_slice);
            let gone = self.loaded[&seq].iter().filter(|t| !keep.contains(t));
            for term in gone.cloned().collect::<Vec<_>>() {
                self.p.retract_fact(&term, seq)?;
                self.unload(seq, &term);
            }
        }
        Ok(())
    }

    /// Assert each wanted fact that is not loaded, once.
    fn assert_missing(&mut self, wanted: &Groups) -> Result<(), Error> {
        for (seq, terms) in wanted {
            for term in terms.iter().copied() {
                if !self.loaded.get(seq).is_some_and(|held| held.contains(term)) {
                    self.p.assert_fact(term, *seq)?;
                    self.loaded.entry(*seq).or_default().push(term.clone());
                }
            }
        }
        Ok(())
    }

    /// Record a fact retracted.
    fn unload(&mut self, seq: u64, term: &Term) {
        let Some(held) = self.loaded.get_mut(&seq) else {
            return;
        };
        held.retain(|t| t != term);
        if held.is_empty() {
            self.loaded.remove(&seq);
        }
    }

    /// The first answer to a goal, if it has one: a value for each of its
    /// variables.
    fn first(&mut self, goal: &Term) -> Result<Option<Vec<Term>>, Error> {
        Ok(self.p.all(goal, 2)?.into_iter().next())
    }

    /// The value of a node: the list its definition gives.
    pub fn node(&mut self, id: u64) -> Result<Term, Error> {
        let goal = c("node", vec![Term::Int(id as i64), Term::Var(0)]);
        let value = self.first(&goal)?.map(|v| v[0].clone());
        value.ok_or_else(|| Error::Server(format!("node {id} has no value")))
    }

    /// How many times a node has been evaluated.
    pub fn evals(&mut self, id: u64) -> Result<i64, Error> {
        let goal = c("node_evals", vec![Term::Int(id as i64), Term::Var(0)]);
        match self.first(&goal)?.as_deref() {
            Some([Term::Int(n)]) => Ok(*n),
            _ => Err(Error::Server("node_evals gave no count".into())),
        }
    }

    /// Read a term with Prolog's own reader. Returns the term and the names
    /// of its variables, in order of first appearance.
    pub fn parse(&mut self, text: &str) -> Result<(Term, Vec<String>), Error> {
        let answer = self.first(&read_goal(text))?;
        let vals = answer.ok_or_else(|| Error::Server("no term read".into()))?;
        let names = names_of(&vals[0], &vals[1]);
        Ok((vals[0].clone(), names))
    }

    /// Try a constraint against a planted violation.
    pub fn plant(&mut self, vars: &Term, body: &Term) -> Result<Term, Error> {
        let result = Term::Var(1_000_000);
        let goal = c("plant_check", vec![vars.clone(), body.clone(), result]);
        let found = self.first(&goal)?.and_then(|v| v.last().cloned());
        found.ok_or_else(|| Error::Server("plant_check gave nothing".into()))
    }

    /// Load the Wikidata subset of import entry `seq` from its blob, or
    /// unload it (None). Does nothing if that one is already loaded.
    pub fn wikidata(&mut self, import: Option<(u64, &std::path::Path)>) -> Result<(), Error> {
        let seq = import.map(|i| i.0);
        if seq == self.wikidata {
            return Ok(());
        }
        self.p.all(&wikidata_goal(import), 1)?;
        self.wikidata = seq;
        Ok(())
    }

    /// Whether a constraint's body or a derived goal calls only what the
    /// reasoner allows (prolog/dataflow.pl, vet/1). Err says what is not
    /// allowed, in words.
    pub fn check_definition(&mut self, body: &Term) -> Result<(), String> {
        let goal = c("check_definition", vec![body.clone()]);
        let checked = self.p.all(&goal, 1);
        checked.map(|_| ()).map_err(definition_refusal)
    }

    /// Whether a rule may be kept (prolog/dataflow.pl, check_rule/2). Err
    /// says why not, in words.
    pub fn check_rule(&mut self, head: &Term, body: &Term) -> Result<(), String> {
        let goal = c("check_rule", vec![head.clone(), body.clone()]);
        let checked = self.p.all(&goal, 1);
        checked.map(|_| ()).map_err(rule_refusal)
    }

    /// Check a file of Wikidata facts as the loader reads it, keeping
    /// nothing: Err names the first term that is not one.
    pub fn check_wikidata(&mut self, file: &std::path::Path) -> Result<(), Error> {
        let file = Term::Str(file.to_string_lossy().into_owned());
        self.p.all(&c("check_wikidata", vec![file]), 1).map(|_| ())
    }

    /// Suggestions without a model, best evidence first.
    pub fn suggestions(&mut self) -> Result<Vec<Suggestion>, Error> {
        let answer = self.first(&c("suggestions", vec![Term::Var(0)]))?;
        let found = answer.map(|v| list(&v[0])).unwrap_or_default();
        Ok(found.iter().filter_map(Suggestion::from_term).collect())
    }

    /// Run any goal; all answers.
    pub fn all(&mut self, goal: &Term) -> Result<Vec<Vec<Term>>, Error> {
        self.p.all(goal, 100)
    }
}

// Reading a term.

/// The goal that reads a text as a term, with the names of its variables.
fn read_goal(text: &str) -> Term {
    let names = c("variable_names", vec![Term::Var(1)]);
    let options = c(".", vec![names, Term::Atom("[]".into())]);
    c(
        "term_string",
        vec![Term::Var(0), Term::Str(text.into()), options],
    )
}

/// The names of a term's variables, in order of first appearance, from
/// the reader's list of Name = Variable; a variable it did not name is _N.
fn names_of(term: &Term, bindings: &Term) -> Vec<String> {
    let named: Vec<(u64, String)> = list(bindings).iter().filter_map(binding).collect();
    let all = variables(term).into_iter();
    all.map(|v| name_of(&named, v)).collect()
}

/// A variable's name, if the reader named it; else _N.
fn name_of(named: &[(u64, String)], v: u64) -> String {
    let name = named.iter().find(|(x, _)| *x == v).map(|(_, n)| n.clone());
    name.unwrap_or_else(|| format!("_{v}"))
}

/// A variable and its name, from Name = Variable.
fn binding(pair: &Term) -> Option<(u64, String)> {
    match pair {
        Term::Compound(eq, nv) if eq == "=" => match (&nv[0], &nv[1]) {
            (Term::Atom(n), Term::Var(v)) => Some((*v, n.clone())),
            _ => None,
        },
        _ => None,
    }
}

/// The goal that loads an import's Wikidata subset, or unloads it.
fn wikidata_goal(import: Option<(u64, &std::path::Path)>) -> Term {
    match import {
        Some((seq, file)) => {
            let file = Term::Str(file.to_string_lossy().into_owned());
            c("load_wikidata", vec![file, Term::Int(seq as i64)])
        }
        None => Term::Atom("unload_wikidata".into()),
    }
}

// Refusals, in words.

/// What Prolog's error(E, _) holds, if the error is one.
/// What an error from the engine says, as the user should read it: of a
/// Prolog error, its formal part, such as type_error(evaluable, foo/0);
/// else the error's own words.
pub fn told(e: Error) -> String {
    match raised(&e) {
        Some(formal) => show(formal),
        None => e.to_string(),
    }
}

fn raised(e: &Error) -> Option<&Term> {
    match e {
        Error::Prolog(Term::Compound(f, a)) if f == "error" => a.first(),
        _ => None,
    }
}

/// What a permission error forbids, and how: permission_error(_, How, What).
fn forbidden(e: &Error) -> Option<(&Term, &Term)> {
    match raised(e)? {
        Term::Compound(k, x) if k == "permission_error" && x.len() == 3 => Some((&x[1], &x[2])),
        _ => None,
    }
}

/// A predicate as Name/Arity, if the term is one.
fn indicator(t: &Term) -> Option<String> {
    match t {
        Term::Compound(s, na) if s == "/" && na.len() == 2 => {
            Some(format!("{}/{}", show(&na[0]), show(&na[1])))
        }
        _ => None,
    }
}

fn definition_refusal(e: Error) -> String {
    match forbidden(&e) {
        Some((Term::Atom(how), what)) if how == "not_allowed" => format!(
            "{} is not allowed: a definition may only read facts, compare and compute (docs/guide.md, § 3.1)",
            not_allowed(what)
        ),
        _ => told(e),
    }
}

/// What a definition may not call, as the user should read it.
fn not_allowed(what: &Term) -> String {
    match (indicator(what), what) {
        (Some(predicate), _) => predicate,
        (None, Term::Atom(v)) if v == "variable_goal" => "a goal held in a variable".into(),
        (None, other) => show(other),
    }
}

fn rule_refusal(e: Error) -> String {
    match (forbidden(&e), raised(&e)) {
        (Some((how, what)), _) => rule_forbidden(how, what),
        (None, Some(Term::Compound(k, x))) if k == "domain_error" && x.len() == 2 => {
            "a variable of the head is not bound by the body".to_string()
        }
        (None, Some(other)) => show(other),
        (None, None) => told(e),
    }
}

fn rule_forbidden(how: &Term, what: &Term) -> String {
    match (how, indicator(what)) {
        (Term::Atom(w), _) if w == "recursive_rule" => format!(
            "{} is recursive, directly or through other rules; rules may not be",
            show(what)
        ),
        (_, Some(predicate)) => {
            format!("{predicate} is not allowed: a rule may only read facts, compare and compute")
        }
        (_, None) => format!("{} is not allowed", show(what)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sync_leaves_the_engine_holding_these_facts_each_once() {
        // Catches a fact left asserted after the world drops it, one not
        // asserted, or one asserted twice when the world lists it twice.
        let mut engine = Engine::start().unwrap();
        let fact = |seq: u64, a: &str| (seq, c("p", vec![Term::Atom(a.into())]));
        engine.sync(&[fact(1, "a"), fact(2, "b")]).unwrap();
        engine
            .sync(&[fact(2, "b"), fact(3, "c"), fact(3, "c")])
            .unwrap();
        let answers = engine.all(&c("p", vec![Term::Var(0)])).unwrap();
        let mut held: Vec<String> = answers.iter().map(|a| show(&a[0])).collect();
        held.sort();
        assert_eq!(held, ["b", "c"]);
    }

    /// What the engine says of a definition or a rule, read from text.
    fn refusal(check: impl Fn(&mut Engine, Term) -> Result<(), String>, text: &str) -> String {
        let mut engine = Engine::start().unwrap();
        let (term, _) = engine.parse(text).unwrap();
        check(&mut engine, term).unwrap_err()
    }

    #[test]
    fn a_refused_fact_leaves_the_facts_before_it_known() {
        // Catches sync recording nothing when one fact is refused: the
        // facts asserted before it were then asserted again on each retry,
        // and taking them away later left them on the server (the code
        // review, C-B2). append/3 is a library's, so it is refused.
        let mut engine = Engine::start().unwrap();
        let atom = |a: &str| Term::Atom(a.into());
        let foo = Term::Compound("foo".into(), vec![atom("a")]);
        let reserved = Term::Compound("append".into(), vec![atom("x"), atom("y"), atom("z")]);
        let both = [(1, foo.clone()), (2, reserved)];
        assert!(engine.sync(&both).is_err());
        assert!(engine.sync(&both).is_err(), "a retry is refused again");
        engine.sync(&[]).unwrap();
        let foo_x = Term::Compound("foo".into(), vec![Term::Var(0)]);
        assert_eq!(engine.p.all(&foo_x, 2).unwrap(), Vec::<Vec<Term>>::new());
    }

    #[test]
    fn a_variable_typed_without_a_name_is_named_by_its_number() {
        // Catches an unnamed variable given another variable's name, or
        // none: in f(X, _), the second is _1.
        let (_, names) = Engine::start().unwrap().parse("f(X, _)").unwrap();
        assert_eq!(names, ["X", "_1"]);
    }

    #[test]
    fn a_goal_held_in_a_variable_is_refused_in_words() {
        // Catches the refusal shown as Prolog's own term, variable_goal.
        let said = refusal(|e, body| e.check_definition(&body), "p(X), X");
        assert!(
            said.starts_with("a goal held in a variable is not allowed"),
            "{said}"
        );
    }

    #[test]
    fn a_head_variable_the_body_leaves_free_is_refused_in_words() {
        // Catches the refusal lost, or shown as Prolog's domain_error.
        let rule = |e: &mut Engine, t: Term| match t {
            Term::Compound(_, a) => e.check_rule(&a[0], &a[1]),
            _ => Ok(()),
        };
        let said = refusal(rule, "q(X, Y) :- p(X)");
        assert_eq!(said, "a variable of the head is not bound by the body");
    }
}
