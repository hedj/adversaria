//! Facts, and the definitions that reason over them: rules, constraints
//! and derived text. Each definition is checked before it is kept.

use super::{all_bound, encoded, functor_of, node_def, reserved, var_list};
use crate::app::App;
use crate::outline::TOP;
use format::terms::Term;
use serde_json::{Value, json};

impl App {
    pub(crate) fn cmd_fact(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let t = self.parse_fact(&words.join(" "))?;
        self.write_fact(target, &t, None)
    }

    /// State a fact, retracting the beliefs it directly contradicts.
    pub(crate) fn cmd_set(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let t = self.parse_fact(&words.join(" "))?;
        self.set_fact(&t, target)
    }

    // Rules.

    pub(crate) fn cmd_rule(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let text = words.join(" ");
        let pi = self.add_rule(text.trim(), target)?;
        self.screen.message = format!("rule {pi} kept");
        Ok(())
    }

    /// Parse, check and keep a rule; its name and arity.
    fn add_rule(&mut self, text: &str, target: u64) -> Result<String, String> {
        let (head, body) = self.parse_rule(text)?;
        self.vet_rule(&head, &body)?;
        let pi = functor_of(&head);
        self.keep_rule(text, head, body, target)?;
        Ok(pi)
    }

    /// A rule as typed: its head and body, the head not reserved and every
    /// head variable bound by the body.
    fn parse_rule(&mut self, text: &str) -> Result<(Term, Term), String> {
        let (t, names) = self.parse_term(text.trim_end_matches('.'), "not a Prolog rule")?;
        let (head, body) = rule_parts(&t)?;
        all_bound(&head, &body, &names, ("the head", "the body"))
            .map_err(|v| format!("rule not kept: {v}"))?;
        Ok((head, body))
    }

    /// The check reads the world's rules: the engine is brought up to date
    /// first, in case it has not been evaluated since.
    fn vet_rule(&mut self, head: &Term, body: &Term) -> Result<(), String> {
        self.sync_engine()?;
        self.engine(0)?
            .check_rule(head, body)
            .map_err(|w| format!("rule not kept: {w}"))
    }

    fn sync_engine(&mut self) -> Result<(), String> {
        let facts = self.facts_at(self.record.tip)?;
        self.engine(0)?.sync(&facts).map_err(|e| e.to_string())
    }

    fn keep_rule(&mut self, text: &str, head: Term, body: Term, target: u64) -> Result<(), String> {
        let entry = rule_entry(text, self.next_seq(), head, body, target)?;
        self.write_here("edit", entry)?;
        Ok(())
    }

    // Constraints.

    pub(crate) fn cmd_constraint(&mut self, words: &[&str], _: u64) -> Result<(), String> {
        let text = words.join(" ");
        let (name, rule) = text
            .split_once(": ")
            .ok_or("constraint (named) a name: false :- body.")?;
        let (name, rule) = (name.trim(), rule.trim());
        let body = self.parse_denial(rule)?;
        let vars = self.vet_constraint(&body)?;
        self.keep_constraint(name, rule, vars, body)?;
        self.screen.message = format!("constraint {name} kept: it fires on a planted violation");
        Ok(())
    }

    /// The body of a denial, false :- Body.
    fn parse_denial(&mut self, rule: &str) -> Result<Term, String> {
        let (t, _) = self.parse_term(rule.trim_end_matches('.'), "not a Prolog rule")?;
        denial_body(&t).ok_or("a constraint is a denial: false :- body.".into())
    }

    /// Check a constraint's body, and that it fires on a planted violation;
    /// its variables.
    fn vet_constraint(&mut self, body: &Term) -> Result<Term, String> {
        self.vet_definition(body, "constraint not kept")?;
        let vars = var_list(body);
        self.check_fires(&vars, body)?;
        Ok(vars)
    }

    fn keep_constraint(
        &mut self,
        name: &str,
        rule: &str,
        vars: Term,
        body: Term,
    ) -> Result<(), String> {
        let defines = Term::Compound(
            "constraint".into(),
            vec![Term::Str(name.into()), vars, body],
        );
        self.write_node_def(
            json!({"op": "constraint", "name": name, "rule": rule}),
            defines,
        )
    }

    // Derived text.

    pub(crate) fn cmd_derive(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let text = words.join(" ");
        let (template, goal) = text
            .split_once(" where ")
            .ok_or("derive (text) a template with {Var}, where a goal")?;
        let (g, names) = self.parse_term(goal.trim().trim_end_matches('.'), "not a Prolog goal")?;
        self.vet_definition(&g, "not derived")?;
        self.write_derive(target, template.trim(), &names, g)
    }

    fn write_derive(
        &mut self,
        item: u64,
        template: &str,
        names: &[String],
        goal: Term,
    ) -> Result<(), String> {
        let fact = encoded(&query_def(item, goal))?;
        self.write_here(
            "edit",
            json!({"op": "derive", "item": item, "template": template, "names": names, "facts": [fact]}),
        )?;
        Ok(())
    }
}

/// A rule's head and body; a head may not be one of the reserved
/// definitions, nor claims/2.
fn rule_parts(t: &Term) -> Result<(Term, Term), String> {
    let (head, body) = match t {
        Term::Compound(n, a) if n == ":-" && a.len() == 2 => (a[0].clone(), a[1].clone()),
        _ => return Err("rule (that) Head :- Body.".into()),
    };
    if reserved(&head)
        || matches!(&head, Term::Compound(f, _) | Term::Atom(f) if f == "rule_def" || f == "node_def" || f == "claims")
    {
        return Err(format!("rule not kept: {} is reserved", functor_of(&head)));
    }
    Ok((head, body))
}

/// The edit entry keeping a rule: rule_def(Seq, Head, Body), and the item
/// it was typed in, if any.
fn rule_entry(text: &str, seq: u64, head: Term, body: Term, target: u64) -> Result<Value, String> {
    let def = Term::Compound("rule_def".into(), vec![Term::Int(seq as i64), head, body]);
    let mut b = json!({"op": "facts", "rule": text, "facts": [encoded(&def)?]});
    if target != TOP {
        b["item"] = json!(target);
    }
    Ok(b)
}

fn denial_body(t: &Term) -> Option<Term> {
    match t {
        Term::Compound(n, a) if n == ":-" && a.len() == 2 && a[0] == Term::Atom("false".into()) => {
            Some(a[1].clone())
        }
        _ => None,
    }
}

/// A derived text's definition: node_def(Item, query(Vars, Goal)).
fn query_def(item: u64, goal: Term) -> Term {
    node_def(
        item,
        Term::Compound("query".into(), vec![var_list(&goal), goal]),
    )
}
