//! Claims, with their sources: stating one, interrogating it, adopting it
//! as a belief, and taking up or dismissing suggestions.

use super::{encoded, functor_of, node_def, reserved};
use crate::app::App;
use crate::reason::{Suggestion, show};
use format::record::Entry;
use format::terms::Term;
use serde_json::{Value, json};

impl App {
    /// An item's claims: (entry seq, source, claimed fact).
    pub fn claims_of(&self, item: u64) -> Vec<(u64, String, Term)> {
        let facts = self.notebook.facts_now().unwrap_or_default();
        facts
            .into_iter()
            .filter(|(s, _)| self.notebook.outline.fact_item.get(s) == Some(&item))
            .filter_map(|(s, f)| match f {
                Term::Compound(n, a) if n == "claims" && a.len() == 2 => {
                    Some((s, name_of(&a[0]), a[1].clone()))
                }
                _ => None,
            })
            .collect()
    }

    /// State a claim: a fact, then from and its source.
    pub(crate) fn cmd_claim(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let text = words.join(" ");
        let (fact, source) = split_from(&text).ok_or("claim (that) a fact, from a source")?;
        let t = self.parse_fact(fact)?;
        self.write_fact(target, &claims(source, t), Some(source))?;
        Ok(String::new())
    }

    /// Show who else claims the target's claim, who disputes it, its
    /// source and record.
    pub(crate) fn cmd_interrogate(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        if self.claims_of(target).is_empty() {
            return Err("the target states no claim".into());
        }
        self.session.view.show_claim(target);
        self.write_view("interrogate", json!({}));
        Ok(String::new())
    }

    // Adopting.

    pub(crate) fn cmd_adopt(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        let claims = self.claims_of(target);
        check_adoptable(&claims)?;
        let new = self.not_adopted(claims);
        if new.is_empty() {
            return Err("the target's claims are beliefs already".into());
        }
        self.write_adoptions(target, new)?;
        Ok("adopted: a belief now, with its source kept".into())
    }

    /// The claims with no adoption in effect on this branch.
    fn not_adopted(&self, claims: Vec<(u64, String, Term)>) -> Vec<(u64, String, Term)> {
        let branch = self.notebook.branch(self.notebook.tip);
        let adopted: Vec<u64> = branch.live().filter_map(adopted_claim).collect();
        claims
            .into_iter()
            .filter(|(seq, ..)| !adopted.contains(seq))
            .collect()
    }

    /// One adopt entry for each claim, keeping its source.
    fn write_adoptions(
        &mut self,
        item: u64,
        claims: Vec<(u64, String, Term)>,
    ) -> Result<(), String> {
        for (seq, src, p) in claims {
            let f = encoded(&p)?;
            self.write_here(
                "adopt",
                json!({"claim": seq, "item": item, "source": src, "facts": [f]}),
            )?;
        }
        Ok(())
    }

    pub(crate) fn cmd_unadopt(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        let last = self
            .last_adoption(target)
            .ok_or("the target has no adoption to retract")?;
        self.write_here("retract", json!({"target": last}))?;
        Ok(format!("adoption {last} retracted"))
    }

    /// An item's last adoption on this branch that can still be retracted:
    /// one already retracted is passed over, for the one before it.
    fn last_adoption(&self, item: u64) -> Option<u64> {
        let path = self.notebook.path(self.notebook.tip);
        let adoptions = path.iter().rev().filter(|e| adopts(e, item));
        adoptions
            .map(|e| e.seq)
            .find(|s| self.retractable(*s).is_ok())
    }

    // Suggestions.

    pub(crate) fn cmd_accept(&mut self, words: &[&str], _: u64) -> Result<String, String> {
        let s = self.suggestion(words, "accept")?;
        self.accept(&s)?;
        Ok(format!("adopted: {}", s.describe()))
    }

    pub(crate) fn cmd_dismiss(&mut self, words: &[&str], _: u64) -> Result<String, String> {
        let s = self.suggestion(words, "dismiss")?;
        self.dismiss_suggestion(&s);
        Ok(format!("dismissed: {}", s.describe()))
    }

    fn dismiss_suggestion(&mut self, s: &Suggestion) {
        self.write_view("dismiss", json!({"key": s.key()}));
    }

    /// The suggestion shown with the number a command's words give.
    fn suggestion(&self, words: &[&str], command: &str) -> Result<Suggestion, String> {
        let n: usize = words
            .first()
            .and_then(|w| w.parse().ok())
            .ok_or(format!("{command} (suggestion) a number"))?;
        self.reason
            .suggestions
            .shown()
            .get(n.wrapping_sub(1))
            .map(|s| (*s).clone())
            .ok_or(format!("there is no suggestion {n}"))
    }
}

/// The fact and the source of "FACT from SOURCE", split at the last from.
fn split_from(text: &str) -> Option<(&str, &str)> {
    let i = text.rfind(" from ")?;
    Some((&text[..i], text[i + 6..].trim()))
}

/// claims(Source, Fact): a fact as a source states it.
fn claims(source: &str, fact: Term) -> Term {
    Term::Compound("claims".into(), vec![Term::Str(source.into()), fact])
}

/// Claims can be adopted unless they are reserved definitions.
fn check_adoptable(claims: &[(u64, String, Term)]) -> Result<(), String> {
    if claims.is_empty() {
        return Err("the target states no claim to adopt".into());
    }
    match claims.iter().find(|(_, _, p)| reserved(p)) {
        Some((_, _, p)) => Err(format!(
            "not adopted: the claim is a {}, which is reserved for constraint, derive and accept",
            functor_of(p)
        )),
        None => Ok(()),
    }
}

/// Whether an entry adopts a claim of an item.
/// The claim an adopt entry adopted, if it is one.
fn adopted_claim(e: &Entry) -> Option<u64> {
    let body = e.body().filter(|_| e.kind() == "adopt")?;
    body["claim"].as_u64()
}

fn adopts(e: &Entry, item: u64) -> bool {
    e.kind() == "adopt" && e.body().and_then(|b| b["item"].as_u64()) == Some(item)
}

impl App {
    /// Adopt a suggestion: one entry, marked as an adopted suggestion and
    /// carrying its evidence, so it can be found and retracted later. A
    /// constraint is tried against a planted violation first.
    pub(crate) fn accept(&mut self, s: &Suggestion) -> Result<(), String> {
        let mark = s.mark();
        match (s.kind_name(), &s.kind) {
            ("constraint", Term::Compound(_, a)) if a.len() == 3 => self.adopt_constraint(a, mark),
            ("rule", Term::Compound(_, a)) if a.len() == 2 => self.adopt_rule(a, mark),
            ("reference", Term::Compound(_, a)) if a.len() == 2 => self.adopt_reference(a, mark),
            ("mention", Term::Compound(_, a)) if a.len() == 4 => self.adopt_mention(a, mark),
            _ => self.adopt_facts(&s.facts, mark),
        }
    }

    /// A suggested link between an item whose text names another and that
    /// item: a link, which records the suggestion it adopts.
    fn adopt_mention(&mut self, a: &[Term], mark: Value) -> Result<(), String> {
        let (Term::Int(from), Term::Int(to)) = (&a[0], &a[1]) else {
            return Err("not adopted: the suggestion names no items".into());
        };
        self.write_suggested_link(*from as u64, *to as u64, mark)
    }

    /// A suggested constraint, its name, variables and body, once it fires
    /// on a planted violation.
    fn adopt_constraint(&mut self, a: &[Term], mark: Value) -> Result<(), String> {
        if let Some(found) = self.misfire(&a[1], &a[2])? {
            return Err(format!(
                "not adopted: tried against a planted violation, it did not fire ({found})"
            ));
        }
        let name = name_of(&a[0]);
        let defines = Term::Compound(
            "constraint".into(),
            vec![Term::Str(name.clone()), a[1].clone(), a[2].clone()],
        );
        let rule = format!("false :- {}", show(&a[2]));
        let fact = encoded(&node_def(self.next_seq(), defines))?;
        let body = json!({"suggestion": mark, "op": "constraint", "name": name, "rule": rule, "facts": [fact]});
        self.write_here("adopt", body)?;
        Ok(())
    }

    /// A suggested rule, its head and body, numbered by its own entry.
    fn adopt_rule(&mut self, a: &[Term], mark: Value) -> Result<(), String> {
        let seq = Term::Int(self.next_seq() as i64);
        let def = Term::Compound("rule_def".into(), vec![seq, a[0].clone(), a[1].clone()]);
        self.write_adopted(mark, vec![encoded(&def)?])
    }

    /// A suggested reference, an entity and a work: the item about the
    /// entity cites the item that is the work.
    fn adopt_reference(&mut self, a: &[Term], mark: Value) -> Result<(), String> {
        let from = *self
            .items_about(&a[0])
            .first()
            .ok_or("no item states facts about that entity")?;
        let to = *self
            .items_about(&a[1])
            .first()
            .ok_or("no item is that work")?;
        let cites = Term::Compound(
            "cites".into(),
            vec![Term::Int(from as i64), Term::Int(to as i64)],
        );
        let body = json!({"from": from, "to": to, "type": "cites", "facts": [encoded(&cites)?], "suggestion": mark});
        self.write_here("link", body)?;
        Ok(())
    }

    /// A suggestion's own facts.
    fn adopt_facts(&mut self, facts: &[Term], mark: Value) -> Result<(), String> {
        let facts = facts.iter().map(encoded).collect::<Result<_, _>>()?;
        self.write_adopted(mark, facts)
    }

    fn write_adopted(&mut self, mark: Value, facts: Vec<Value>) -> Result<(), String> {
        self.write_here("adopt", json!({"suggestion": mark, "facts": facts}))?;
        Ok(())
    }
}

/// A name, as a term gives it: a string's text, or any other term shown,
/// as an atom is by its own text.
fn name_of(t: &Term) -> String {
    match t {
        Term::Str(x) => x.clone(),
        t => show(t),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::TOP;

    /// A memex in a directory of this test's own, with an item stating a
    /// fact about an entity x.
    fn about_x(name: &str) -> (std::path::PathBuf, App) {
        let dir =
            std::env::temp_dir().join(format!("adversaria-app-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::open(&dir).unwrap();
        app.new_item_after(TOP).unwrap();
        app.write_fact(
            app.cursor(),
            &Term::Compound("p".into(), vec![Term::Atom("x".into())]),
            None,
        )
        .unwrap();
        (dir, app)
    }

    /// Accept a suggestion of a kind, with nothing else.
    fn accepting(app: &mut App, kind: &str, args: Vec<Term>) -> Result<(), String> {
        let kind = Term::Compound(kind.into(), args);
        let (facts, evidence) = (vec![], String::new());
        app.accept(&Suggestion {
            kind,
            facts,
            evidence,
            score: 0,
        })
    }

    #[test]
    fn a_suggested_constraint_that_does_not_fire_is_not_adopted() {
        let (dir, mut app) = about_x("misfire");
        let x = Term::Var(0);
        let never = Term::Compound("<".into(), vec![x.clone(), x.clone()]);
        let body = Term::Compound(",".into(), vec![Term::Compound("p".into(), vec![x]), never]);
        let args = vec![Term::Str("never".into()), Term::Atom("[]".into()), body];
        let refused = accepting(&mut app, "constraint", args);
        assert!(
            matches!(&refused, Err(e) if e.starts_with("not adopted: tried against a planted")),
            "{refused:?}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_reference_to_a_work_with_no_item_is_not_adopted() {
        let (dir, mut app) = about_x("no-work");
        let args = vec![Term::Atom("x".into()), Term::Atom("y".into())];
        let refused = accepting(&mut app, "reference", args);
        assert_eq!(refused, Err("no item is that work".into()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
