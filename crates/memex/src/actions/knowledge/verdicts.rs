//! Verdicts on findings (docs/design.md § Measures): what the user says a
//! finding of a rule, marked under an item, is worth. A verdict changes
//! nothing shown; it is a view entry, so the measures report can tell the
//! findings that caught real errors from those that did not.

use crate::app::App;
use crate::marks::marked_under;
use serde_json::json;

/// How a verdict is given.
const USAGE: &str = "verdict error, rule, misread or known, then the rule's entry";

/// Why a verdict typed without a rule's entry has none.
pub(crate) const NO_RULE: &str =
    "no rule's finding is marked under the target, or more than one rule's is";

/// The verdicts, each with what it says of the finding.
pub(crate) const VERDICTS: &[(&str, &str)] = &[
    ("error", "a real error"),
    ("rule", "the rule is wrong"),
    ("misread", "the text was misread"),
    ("known", "known already"),
];

impl App {
    /// Record a verdict on the findings of a rule, by its entry, marked
    /// under the target; without an entry, the one rule marked there.
    pub(crate) fn cmd_verdict(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let (which, rest) = words.split_first().ok_or(USAGE)?;
        let (word, says) = verdict_named(which)?;
        let rule = self.rule_of(rest, target)?;
        let findings = self.findings_marked(rule, target)?;
        let body = json!({"verdict": word, "rule": rule, "under": target, "findings": findings});
        self.write_view("verdict", body);
        Ok(format!("recorded: {says}"))
    }

    /// The rule a verdict names, by its entry; with none named, the one
    /// rule whose findings are marked under the target.
    fn rule_of(&self, words: &[&str], target: u64) -> Result<u64, String> {
        match words {
            [] => self.one_rule_under(target).ok_or(NO_RULE.into()),
            _ => crate::actions::entry_number(words).ok_or(USAGE.into()),
        }
    }

    /// The findings of a rule marked under the target, or why a verdict
    /// on them cannot be given: there are none.
    fn findings_marked(&self, rule: u64, target: u64) -> Result<Vec<String>, String> {
        let findings = self.findings_under(rule, target);
        match findings.is_empty() {
            true => Err(format!(
                "no finding of entry {rule} is marked under the target"
            )),
            false => Ok(findings),
        }
    }

    /// The findings of a rule, by its entry, marked under an item, each
    /// named as an `observed` entry names it.
    fn findings_under(&self, rule: u64, item: u64) -> Vec<String> {
        let eval = self.reason.eval[0].as_ref();
        let Some(c) = eval.and_then(|e| e.constraint_of(rule)) else {
            return vec![];
        };
        let outline = &self.notebook.outline;
        c.named_where(|f| marked_under(f, outline, item))
    }

    /// The rule a verdict on the target is about by default, by its
    /// entry, as the prompt shows it.
    pub(crate) fn rule_under(&self, target: u64) -> Option<String> {
        self.one_rule_under(target).map(|r| r.to_string())
    }

    /// The one rule whose findings are marked under an item, by its
    /// entry; none if no rule's are, or more than one rule's.
    fn one_rule_under(&self, item: u64) -> Option<u64> {
        let eval = self.reason.eval[0].as_ref()?;
        let rules = eval.constraints.iter().map(|c| c.seq);
        let mut marked = rules.filter(|r| !self.findings_under(*r, item).is_empty());
        let (one, none) = (marked.next()?, marked.next().is_none());
        none.then_some(one)
    }
}

/// A verdict, by its word: the word, and what it says of the finding.
fn verdict_named(word: &str) -> Result<&'static (&'static str, &'static str), String> {
    VERDICTS
        .iter()
        .find(|(w, _)| *w == word)
        .ok_or(USAGE.into())
}
