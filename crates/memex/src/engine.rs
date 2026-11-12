//! A world's reasoning: its facts loaded into a SWI-Prolog process, and the
//! values of its dataflow nodes (prolog/dataflow.pl).
//!
//! Facts are synced as a difference: those gone are retracted and those new
//! asserted, so SWI-Prolog's incremental tabling recomputes only the nodes
//! whose facts changed.

use crate::prolog::{Error, Prolog};
use format::terms::Term;

pub struct Engine {
    p: Prolog,
    loaded: Vec<(u64, Term)>,
    /// The import entry whose Wikidata subset is loaded.
    wikidata: Option<u64>,
}

fn c(f: &str, args: Vec<Term>) -> Term {
    Term::Compound(f.into(), args)
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
            loaded: vec![],
            wikidata: None,
        })
    }

    /// Make the loaded facts equal these, by difference.
    pub fn sync(&mut self, facts: &[(u64, Term)]) -> Result<(), Error> {
        let gone: Vec<(u64, Term)> = self
            .loaded
            .iter()
            .filter(|f| !facts.contains(f))
            .cloned()
            .collect();
        for (s, f) in &gone {
            self.p.retract_fact(f, *s)?;
        }
        for (s, f) in facts {
            if !self.loaded.contains(&(*s, f.clone())) {
                self.p.assert_fact(f, *s)?;
            }
        }
        self.loaded = facts.to_vec();
        Ok(())
    }

    /// The value of a node: the list its definition gives.
    pub fn node(&mut self, id: u64) -> Result<Term, Error> {
        let a = self
            .p
            .all(&c("node", vec![Term::Int(id as i64), Term::Var(0)]), 2)?;
        a.into_iter()
            .next()
            .map(|v| v[0].clone())
            .ok_or_else(|| Error::Server(format!("node {id} has no value")))
    }

    /// How many times a node has been evaluated.
    pub fn evals(&mut self, id: u64) -> Result<i64, Error> {
        let a = self.p.all(
            &c("node_evals", vec![Term::Int(id as i64), Term::Var(0)]),
            2,
        )?;
        match a.first().map(|v| &v[0]) {
            Some(Term::Int(n)) => Ok(*n),
            _ => Err(Error::Server("node_evals gave no count".into())),
        }
    }

    /// Read a term with Prolog's own reader. Returns the term and the names
    /// of its variables, in order of first appearance.
    pub fn parse(&mut self, text: &str) -> Result<(Term, Vec<String>), Error> {
        let goal = c(
            "term_string",
            vec![
                Term::Var(0),
                Term::Str(text.into()),
                c(
                    ".",
                    vec![
                        c("variable_names", vec![Term::Var(1)]),
                        Term::Atom("[]".into()),
                    ],
                ),
            ],
        );
        let a = self.p.all(&goal, 2)?;
        let vals = a
            .into_iter()
            .next()
            .ok_or_else(|| Error::Server("no term read".into()))?;
        let mut names = Vec::new();
        let mut list = &vals[1];
        let mut by_var = Vec::new();
        while let Term::Compound(f, args) = list {
            if f != "." {
                break;
            }
            if let Term::Compound(eq, nv) = &args[0]
                && eq == "="
                && let (Term::Atom(n), Term::Var(v)) = (&nv[0], &nv[1])
            {
                by_var.push((*v, n.clone()));
            }
            list = &args[1];
        }
        for v in variables(&vals[0]) {
            names.push(
                by_var
                    .iter()
                    .find(|(x, _)| *x == v)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_else(|| format!("_{v}")),
            );
        }
        Ok((vals[0].clone(), names))
    }

    /// Try a constraint against a planted violation.
    pub fn plant(&mut self, vars: &Term, body: &Term) -> Result<Term, Error> {
        let a = self.p.all(
            &c(
                "plant_check",
                vec![vars.clone(), body.clone(), Term::Var(1_000_000)],
            ),
            2,
        )?;
        a.into_iter()
            .next()
            .and_then(|v| v.last().cloned())
            .ok_or_else(|| Error::Server("plant_check gave nothing".into()))
    }

    /// Load the Wikidata subset of import entry `seq` from its blob, or
    /// unload it (None). Does nothing if that one is already loaded.
    pub fn wikidata(&mut self, import: Option<(u64, &std::path::Path)>) -> Result<(), Error> {
        if import.map(|i| i.0) == self.wikidata {
            return Ok(());
        }
        match import {
            Some((seq, file)) => {
                let goal = c(
                    "load_wikidata",
                    vec![
                        Term::Str(file.to_string_lossy().into_owned()),
                        Term::Int(seq as i64),
                    ],
                );
                self.p.all(&goal, 1)?;
            }
            None => {
                self.p.all(&Term::Atom("unload_wikidata".into()), 1)?;
            }
        }
        self.wikidata = import.map(|i| i.0);
        Ok(())
    }

    /// Whether a constraint's body or a derived goal calls only what the
    /// reasoner allows (prolog/dataflow.pl, vet/1). Err says what is not
    /// allowed, in words.
    pub fn check_definition(&mut self, body: &Term) -> Result<(), String> {
        match self.p.all(&c("check_definition", vec![body.clone()]), 1) {
            Ok(_) => Ok(()),
            Err(Error::Prolog(Term::Compound(e, a)))
                if e == "error"
                    && matches!(&a[0], Term::Compound(p, pa) if p == "permission_error" && pa.len() == 3 && pa[1] == Term::Atom("not_allowed".into())) =>
            {
                let Term::Compound(_, pa) = &a[0] else {
                    unreachable!()
                };
                let what = match &pa[2] {
                    Term::Compound(s, na) if s == "/" && na.len() == 2 => {
                        format!(
                            "{}/{}",
                            crate::reason::show(&na[0]),
                            crate::reason::show(&na[1])
                        )
                    }
                    Term::Atom(v) if v == "variable_goal" => "a goal held in a variable".into(),
                    other => crate::reason::show(other),
                };
                Err(format!(
                    "{what} is not allowed: a definition may only read facts, compare and compute (docs/guide.md, § 7)"
                ))
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Whether a rule may be kept (prolog/dataflow.pl, check_rule/2). Err
    /// says why not, in words.
    pub fn check_rule(&mut self, head: &Term, body: &Term) -> Result<(), String> {
        match self
            .p
            .all(&c("check_rule", vec![head.clone(), body.clone()]), 1)
        {
            Ok(_) => Ok(()),
            Err(Error::Prolog(Term::Compound(e, a))) if e == "error" => {
                let show = crate::reason::show;
                Err(match &a[0] {
                    Term::Compound(k, x) if k == "permission_error" && x.len() == 3 => {
                        match (&x[1], &x[2]) {
                            (Term::Atom(w), what) if w == "recursive_rule" => {
                                format!(
                                    "{} is recursive, directly or through other rules; rules may not be",
                                    show(what)
                                )
                            }
                            (_, Term::Compound(s, na)) if s == "/" && na.len() == 2 => format!(
                                "{}/{} is not allowed: a rule may only read facts, compare and compute",
                                show(&na[0]),
                                show(&na[1])
                            ),
                            (_, what) => format!("{} is not allowed", show(what)),
                        }
                    }
                    Term::Compound(k, x) if k == "domain_error" && x.len() == 2 => {
                        "a variable of the head is not bound by the body".to_string()
                    }
                    other => show(other),
                })
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Check a file of Wikidata facts as the loader reads it, keeping
    /// nothing: Err names the first term that is not one.
    pub fn check_wikidata(&mut self, file: &std::path::Path) -> Result<(), Error> {
        self.p
            .all(
                &c(
                    "check_wikidata",
                    vec![Term::Str(file.to_string_lossy().into_owned())],
                ),
                1,
            )
            .map(|_| ())
    }

    /// Suggestions without a model, best evidence first.
    pub fn suggestions(&mut self) -> Result<Vec<crate::reason::Suggestion>, Error> {
        let a = self.p.all(&c("suggestions", vec![Term::Var(0)]), 2)?;
        let list = a
            .into_iter()
            .next()
            .map(|v| v[0].clone())
            .unwrap_or(Term::Atom("[]".into()));
        Ok(crate::reason::list(&list)
            .iter()
            .filter_map(crate::reason::Suggestion::from_term)
            .collect())
    }

    /// Run any goal; all answers.
    pub fn all(&mut self, goal: &Term) -> Result<Vec<Vec<Term>>, Error> {
        self.p.all(goal, 100)
    }
}
