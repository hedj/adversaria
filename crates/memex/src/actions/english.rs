//! Controlled English: what APE makes of a text, read with the words the
//! current world gives (docs/translator.md).

use crate::ace::{Parse, lexicon_of, said};
use crate::app::App;
use crate::engine::told;

impl App {
    /// `ace`: what APE makes of a text, on the message line.
    pub(crate) fn cmd_ace(&mut self, words: &[&str], _: u64) -> Result<String, String> {
        let text = words.join(" ");
        if text.is_empty() {
            return Err("ace (sentence) a sentence of controlled English".into());
        }
        Ok(said(&self.ace_parse(&text)?))
    }

    /// What APE makes of a text, with the current world's words in its
    /// lexicon. APE's warnings that a definite noun phrase has no
    /// antecedent are left out: one sentence about a fact ("the date of
    /// buran_flight is 1988") has none to have, and the warning would
    /// come with each.
    pub(crate) fn ace_parse(&mut self, text: &str) -> Result<Parse, String> {
        let facts = self.notebook.facts_now()?;
        let ace = self.reason.ace()?;
        let mut parse = ace.parse_with(lexicon_of(&facts), text).map_err(told)?;
        parse.messages.retain(|m| m.error || m.kind != "anaphor");
        Ok(parse)
    }
}
