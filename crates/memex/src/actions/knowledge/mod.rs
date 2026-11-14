//! Commands that state what the notes know: facts and the definitions
//! that reason over them, claims and their sources, and the documents and
//! data they come from (docs/guide.md, § 2.4, Facts and definitions;
//! Claims and suggestions).

mod claims;
mod definitions;
mod layers;
mod sources;

use crate::app::App;
use crate::engine::variables;
use crate::reason::{make_list, show};
use format::terms::{self, Term};
use serde_json::{Value, json};

impl App {
    /// Parse Prolog text; an error starts with `what`.
    fn parse_term(&mut self, text: &str, what: &str) -> Result<(Term, Vec<String>), String> {
        self.engine(0)?
            .parse(text)
            .map_err(|e| format!("{what}: {e}"))
    }

    /// A fact as typed: an atom or a compound term, ground, not reserved.
    fn parse_fact(&mut self, text: &str) -> Result<Term, String> {
        let (t, _) = self.parse_term(text.trim_end_matches('.'), "not a Prolog fact")?;
        check_fact(&t)?;
        Ok(t)
    }

    /// Write an edit entry stating a fact of an item, from a source if one
    /// is given.
    fn write_fact(&mut self, item: u64, fact: &Term, source: Option<&str>) -> Result<(), String> {
        let mut body = json!({"op": "facts", "item": item, "facts": [encoded(fact)?]});
        if let Some(src) = source {
            body["source"] = json!(src);
        }
        self.write_here("edit", body)?;
        Ok(())
    }

    /// Check that a goal is one a definition may use; an error starts with
    /// `what`.
    fn vet_definition(&mut self, goal: &Term, what: &str) -> Result<(), String> {
        self.engine(0)?
            .check_definition(goal)
            .map_err(|w| format!("{what}: {w}"))
    }

    /// A constraint is tried against a planted violation before it is kept.
    fn check_fires(&mut self, vars: &Term, body: &Term) -> Result<(), String> {
        let planted = self
            .engine(0)?
            .plant(vars, body)
            .map_err(|e| e.to_string())?;
        if matches!(&planted, Term::Compound(n, _) if n == "fires") {
            return Ok(());
        }
        Err(format!(
            "constraint not kept: tried against a planted violation, it did not fire ({})",
            show(&planted)
        ))
    }

    /// Write an edit entry that defines a node, numbered by the entry's own
    /// sequence number.
    fn write_node_def(&mut self, mut body: Value, defines: Term) -> Result<(), String> {
        body["facts"] = json!([self.node_def_fact(defines)?]);
        self.write_here("edit", body)?;
        Ok(())
    }

    fn node_def_fact(&self, defines: Term) -> Result<Value, String> {
        encoded(&node_def(self.next_seq(), defines))
    }

    /// The sequence number the next entry will have.
    fn next_seq(&self) -> u64 {
        self.journal.entries().len() as u64 + 1
    }
}

/// A fact is an atom or a compound term with no variables, and not one of
/// the definitions that only constraint, derive and accept make.
fn check_fact(t: &Term) -> Result<(), String> {
    if !matches!(t, Term::Atom(_) | Term::Compound(..)) || !variables(t).is_empty() {
        return Err("a fact is an atom or a compound term, with no variables".into());
    }
    if reserved(t) {
        return Err(format!(
            "{} is reserved: definitions are made by constraint, derive and accept",
            functor_of(t)
        ));
    }
    Ok(())
}

/// The definitions only commands write: node_def/2 and rule_def/3.
fn reserved(t: &Term) -> bool {
    matches!(t, Term::Compound(f, a) if (f == "node_def" && a.len() == 2) || (f == "rule_def" && a.len() == 3))
}

/// A term's name and arity, as name/arity.
fn functor_of(t: &Term) -> String {
    match t {
        Term::Compound(f, a) => format!("{f}/{}", a.len()),
        Term::Atom(a) => format!("{a}/0"),
        _ => "term".into(),
    }
}

/// A term as a journal entry carries it.
fn encoded(t: &Term) -> Result<Value, String> {
    terms::encode(t).map_err(|e| e.0)
}

/// node_def(Seq, Defines): what a node, numbered Seq, defines.
fn node_def(seq: u64, defines: Term) -> Term {
    Term::Compound("node_def".into(), vec![Term::Int(seq as i64), defines])
}

/// A term's variables, as a Prolog list.
fn var_list(t: &Term) -> Term {
    make_list(variables(t).into_iter().map(Term::Var).collect())
}

/// Every variable of `outer` must be bound by `inner`; the error names
/// those that are not, by their names as typed (`names`, in order of first
/// appearance), and the two parts as `parts`.
fn all_bound(
    outer: &Term,
    inner: &Term,
    names: &[String],
    parts: (&str, &str),
) -> Result<(), String> {
    let free = free_names(outer, inner, names);
    if free.is_empty() {
        return Ok(());
    }
    let (o, i) = parts;
    let verb = if free.len() == 1 { "is" } else { "are" };
    Err(format!(
        "{} in {o} {verb} not bound by {i}",
        free.join(", ")
    ))
}

fn free_names(outer: &Term, inner: &Term, names: &[String]) -> Vec<String> {
    let bound = variables(inner);
    variables(outer)
        .into_iter()
        .filter(|v| !bound.contains(v))
        .map(|v| {
            names
                .get(v as usize)
                .cloned()
                .unwrap_or_else(|| format!("_{v}"))
        })
        .collect()
}
