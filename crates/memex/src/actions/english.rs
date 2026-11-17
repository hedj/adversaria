//! Controlled English: what APE makes of a text, read with the words the
//! current world gives (docs/translator.md).

use crate::ace::{Check, Next, Parse, hint, lexicon_of, listing, said};
use crate::app::{Active, App, English, Hint, Quasi};
use crate::engine::told;
use crate::pairs::{pairs_of, proposal};

impl App {
    /// `ace`: what APE makes of a text, on the message line.
    pub(crate) fn cmd_ace(&mut self, words: &[&str], _: u64) -> Result<String, String> {
        let text = words.join(" ");
        if text.is_empty() {
            return Err("ace (sentence) a sentence of controlled English".into());
        }
        Ok(said(&self.ace_parse(&text)?))
    }

    /// Check again what is typed in Suppose, the one quasimode whose text
    /// is English, and list what may come next while `?` lists.
    pub(crate) fn recheck(&mut self, a: &mut Active) {
        if a.q != Quasi::Suppose {
            return;
        }
        let typed = !a.input.trim().is_empty();
        let english = a.english.get_or_insert_with(English::default);
        let mut hint = typed.then(|| self.hint_for(&a.input, &mut english.free));
        if a.listing {
            let listed = self.ace_listing(&a.input);
            hint.get_or_insert_with(Hint::none).listing = listed;
        }
        english.hint = hint;
    }

    /// What APE makes of what is typed in Suppose, and, if it is not ACE,
    /// a sentence of ACE recalled from an earlier pair like it, which Esc
    /// puts in place. Keeps the fullest text that was not ACE.
    fn hint_for(&mut self, input: &str, free_english: &mut Option<String>) -> Hint {
        let checked = self.ace_check(input);
        if let Ok(Check::NotAce(_)) = &checked {
            keep_fullest(free_english, input);
        }
        let free = matches!(checked, Ok(Check::Open(_) | Check::NotAce(_)));
        let recalled = if free { self.recalled(input) } else { None };
        let hint = hint_of(checked);
        match recalled {
            Some(ace) => as_before(hint, ace),
            None => hint,
        }
    }

    /// The ACE of the latest confirmed pair like a text, adapted to it, if
    /// it is ACE (crate::pairs).
    fn recalled(&mut self, text: &str) -> Option<String> {
        let pairs = pairs_of(self.journal().entries());
        let (ace, _) = proposal(&pairs, text)?;
        matches!(self.ace_check(&ace), Ok(Check::Ace(_))).then_some(ace)
    }

    /// The free English a supposition's text was mended from, if the text
    /// is a sentence of ACE: the pair it confirms. (The free English was
    /// not ACE, so it is not the text.)
    pub(crate) fn confirmed_english(&mut self, a: &Active, text: &str) -> Option<String> {
        let english = a.english.as_ref()?.free.clone()?;
        let ace = matches!(self.ace_check(text), Ok(Check::Ace(_)));
        ace.then_some(english)
    }

    /// What may come next in a text, listed with help; or why not.
    fn ace_listing(&mut self, text: &str) -> Vec<String> {
        match self.ace_next(text) {
            Ok(next) => listing(&next),
            Err(e) => vec![e],
        }
    }

    fn ace_next(&mut self, text: &str) -> Result<Vec<Next>, String> {
        let facts = self.notebook.facts_now()?;
        let ace = self.reason.ace()?;
        ace.next_with(lexicon_of(&facts), text).map_err(told)
    }

    /// What a text being typed is, with the current world's words.
    fn ace_check(&mut self, text: &str) -> Result<Check, String> {
        let facts = self.notebook.facts_now()?;
        let ace = self.reason.ace()?;
        ace.check_with(lexicon_of(&facts), text).map_err(told)
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

/// A check as a hint: in words, with the text its first repair mends;
/// or why there is no check.
fn hint_of(checked: Result<Check, String>) -> Hint {
    let (said, mended) = match checked {
        Ok(c) => (hint(&c), c.mended()),
        Err(said) => (said, None),
    };
    Hint {
        said: Some(said),
        mended,
        ..Hint::none()
    }
}

impl Hint {
    /// Nothing said, nothing to put in place, nothing listed.
    fn none() -> Hint {
        Hint {
            said: None,
            mended: None,
            listing: vec![],
        }
    }
}

/// Keep a text as the free English if it is at least as long as the one
/// kept: a word half typed or half deleted does not replace it.
fn keep_fullest(english: &mut Option<String>, text: &str) {
    let text = text.trim();
    if english.as_ref().is_none_or(|e| text.len() >= e.len()) {
        *english = Some(text.to_string());
    }
}

/// A hint with a sentence recalled from an earlier pair first, which Esc
/// puts in place.
fn as_before(h: Hint, ace: String) -> Hint {
    let said = h.said.map(|s| format!("as before: {ace} · Esc · {s}"));
    Hint {
        said,
        mended: Some(ace),
        ..h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fullest_free_english_is_kept() {
        // Catches a word half deleted, or a shorter edit, replacing the
        // free English typed before it.
        let mut english = None;
        keep_fullest(&mut english, "the flight was in 1989 ");
        keep_fullest(&mut english, "the flight  in 1989");
        assert_eq!(english.as_deref(), Some("the flight was in 1989"));
        keep_fullest(&mut english, "the flight was in 1989 and");
        assert_eq!(english.as_deref(), Some("the flight was in 1989 and"));
    }
}
