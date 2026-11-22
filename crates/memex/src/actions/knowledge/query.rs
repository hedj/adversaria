//! A Prolog query of the current world's beliefs, as typed: each answer,
//! by its variables. The goal is checked as a definition is (it may only
//! read facts, compare and compute), and proved as a derived goal is,
//! through rules. A query changes nothing, so the journal records none.

use crate::app::App;
use crate::engine::told;
use crate::reason::show;
use format::terms::Term;

/// How many answers a query shows; one more says there are more.
const SHOWN: usize = 20;

impl App {
    pub(crate) fn cmd_query(&mut self, words: &[&str], _: u64) -> Result<String, String> {
        let text = words.join(" ");
        let text = text.trim().trim_end_matches('.');
        if text.is_empty() {
            return Err("query (goal) a Prolog goal, as animal(X)".into());
        }
        let (goal, names) = self.parse_term(text, "not a Prolog goal")?;
        self.vet_definition(&goal, "not asked")?;
        self.sync_engine()?;
        let answers = self.answers(goal)?;
        Ok(answered(&names, &answers))
    }

    /// The goal's answers from the beliefs, at most one more than shown:
    /// the values of its variables, in order.
    fn answers(&mut self, goal: Term) -> Result<Vec<Vec<Term>>, String> {
        let beliefs = Term::Compound("beliefs".into(), vec![goal]);
        let most = Term::Int(SHOWN as i64 + 1);
        let limited = Term::Compound("limit".into(), vec![most, beliefs]);
        self.reason.engine(0)?.all(&limited).map_err(told)
    }
}

/// What a query found, in words: yes or no for a goal with no variables;
/// else how many answers, and each, by its variables' names and values.
fn answered(names: &[String], answers: &[Vec<Term>]) -> String {
    if answers.is_empty() {
        return "no".into();
    }
    // A variable named with a leading _ is not shown, so a goal with no
    // other has nothing to show but that it holds.
    if names.iter().all(|n| n.starts_with('_')) {
        return "yes".into();
    }
    let listed = listed(names, &answers[..answers.len().min(SHOWN)]);
    match answers.len() {
        n if n > SHOWN => format!("more than {SHOWN} answers: {listed} · …"),
        1 => format!("1 answer: {listed}"),
        n => format!("{n} answers: {listed}"),
    }
}

/// Answers, each by its variables, between dots.
fn listed(names: &[String], answers: &[Vec<Term>]) -> String {
    let each: Vec<String> = answers.iter().map(|a| answer(names, a)).collect();
    each.join(" · ")
}

/// One answer: each named variable and its value; _ variables left out.
fn answer(names: &[String], values: &[Term]) -> String {
    let pairs = names
        .iter()
        .zip(values)
        .filter(|(n, _)| !n.starts_with('_'));
    let shown: Vec<String> = pairs.map(|(n, v)| format!("{n} = {}", show(v))).collect();
    shown.join(", ")
}
