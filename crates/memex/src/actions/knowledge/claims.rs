//! Claims, with their sources: stating one, interrogating it, adopting it
//! as a belief, and taking up or dismissing suggestions.

use super::{encoded, functor_of, reserved};
use crate::app::App;
use crate::reason::Suggestion;
use format::record::Entry;
use format::terms::Term;
use serde_json::json;

impl App {
    /// State a claim: a fact, then from and its source.
    pub(crate) fn cmd_claim(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let text = words.join(" ");
        let (fact, source) = split_from(&text).ok_or("claim (that) a fact, from a source")?;
        let t = self.parse_fact(fact)?;
        self.write_fact(target, &claims(source, t), Some(source))
    }

    /// Show who else claims the target's claim, who disputes it, its
    /// source and record.
    pub(crate) fn cmd_interrogate(&mut self, _: &[&str], target: u64) -> Result<(), String> {
        if self.claims_of(target).is_empty() {
            return Err("the target states no claim".into());
        }
        self.view.show_claim(target);
        self.write_view("interrogate", json!({}));
        Ok(())
    }

    // Adopting.

    pub(crate) fn cmd_adopt(&mut self, _: &[&str], target: u64) -> Result<(), String> {
        let claims = self.claims_of(target);
        check_adoptable(&claims)?;
        self.write_adoptions(target, claims)?;
        self.message = "adopted: a belief now, with its source kept".into();
        Ok(())
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

    pub(crate) fn cmd_unadopt(&mut self, _: &[&str], target: u64) -> Result<(), String> {
        let last = self
            .last_adoption(target)
            .ok_or("the target has no adoption to retract")?;
        self.write_here("retract", json!({"target": last}))?;
        self.message = format!("adoption {last} retracted");
        Ok(())
    }

    /// An item's last adoption on this branch, if it can still be retracted.
    fn last_adoption(&self, item: u64) -> Option<u64> {
        self.adoption_of(item)
            .filter(|s| self.retractable(*s).is_ok())
    }

    fn adoption_of(&self, item: u64) -> Option<u64> {
        self.path(self.tip)
            .iter()
            .rev()
            .find(|e| adopts(e, item))
            .map(|e| e.seq)
    }

    // Suggestions.

    pub(crate) fn cmd_accept(&mut self, words: &[&str], _: u64) -> Result<(), String> {
        let s = self.suggestion(words, "accept")?;
        self.accept(&s)?;
        self.message = format!("adopted: {}", s.describe());
        Ok(())
    }

    pub(crate) fn cmd_dismiss(&mut self, words: &[&str], _: u64) -> Result<(), String> {
        let s = self.suggestion(words, "dismiss")?;
        self.dismiss_suggestion(&s);
        self.message = format!("dismissed: {}", s.describe());
        Ok(())
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
        self.shown_suggestions()
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
fn adopts(e: &Entry, item: u64) -> bool {
    e.kind() == "adopt" && e.body().and_then(|b| b["item"].as_u64()) == Some(item)
}
