//! The memory of confirmed pairs (docs/translator.md, "Proposing a
//! formulation"): free English a user typed in Suppose, and the sentence
//! of ACE they mended it into and supposed. Each supposition entry written
//! from a sentence of ACE that began as free English holds the pair: its
//! `name`, the ACE, and its `english`. A new text like an earlier free
//! English, word for word but for a name or a number or two, is offered
//! the earlier ACE with the new words swapped in.

use format::bodies::Body;
use format::record::Entry;

/// Free English, and the sentence of ACE it was mended into.
#[derive(Debug, Clone, PartialEq)]
pub struct Pair {
    pub english: String,
    pub ace: String,
}

/// At most this many words may differ between a text and an earlier free
/// English for the earlier ACE to be adapted to it.
const MOST_CHANGED: usize = 2;

/// The pairs the journal holds, the latest first.
pub fn pairs_of(entries: &[Entry]) -> Vec<Pair> {
    let mut pairs: Vec<Pair> = entries.iter().filter_map(pair_of).collect();
    pairs.reverse();
    pairs
}

/// The pair a supposition entry holds, if it holds one.
fn pair_of(e: &Entry) -> Option<Pair> {
    let Body::Suppose(s) = Body::of(e)? else {
        return None;
    };
    let (english, ace) = (s.english?, s.name?);
    Some(Pair { english, ace })
}

/// The earlier ACE adapted to a text: the latest pair whose free English
/// the text matches but for a few words, each of which the ACE holds;
/// the ACE with the text's words in their place, and the pair.
pub fn proposal<'a>(pairs: &'a [Pair], text: &str) -> Option<(String, &'a Pair)> {
    pairs
        .iter()
        .find_map(|p| adapted(p, text).map(|ace| (ace, p)))
}

/// A pair's ACE with each word in which a text differs from its free
/// English put in its place, if the two have as many words, differ in a
/// few and in fewer than half, and the ACE holds each word changed.
pub fn adapted(pair: &Pair, text: &str) -> Option<String> {
    let changed = changes(&pair.english, text)?;
    changed
        .iter()
        .try_fold(pair.ace.clone(), |ace, (from, to)| swapped(&ace, from, to))
}

/// The words in which a text differs from an earlier one, place by place
/// (ignoring case and end marks), each once, if they may be swapped: a
/// word changed the same way in two places is one change, and one
/// changed two ways, or changed in one place and kept in another, cannot
/// be swapped: the swap puts it as the other word everywhere.
fn changes(earlier: &str, text: &str) -> Option<Vec<(String, String)>> {
    let (old, new) = (words(earlier), words(text));
    if old.len() != new.len() || old.is_empty() {
        return None;
    }
    let mut changed: Vec<(String, String)> = Vec::new();
    for (o, n) in old
        .iter()
        .zip(&new)
        .filter(|(o, n)| !o.eq_ignore_ascii_case(n))
    {
        once(&mut changed, o, n)?;
    }
    if kept_elsewhere(&changed, &old, &new) {
        return None;
    }
    let few = changed.len() <= MOST_CHANGED && 2 * changed.len() < old.len();
    few.then_some(changed)
}

/// Whether a word changed in one place is kept in another.
fn kept_elsewhere(changed: &[(String, String)], old: &[String], new: &[String]) -> bool {
    let was_changed = |w: &String| changed.iter().any(|(f, _)| f.eq_ignore_ascii_case(w));
    let kept = |(o, n): (&String, &String)| o.eq_ignore_ascii_case(n) && was_changed(o);
    old.iter().zip(new).any(kept)
}

/// Note a change once; none if the word was already changed another way.
fn once(changed: &mut Vec<(String, String)>, from: &str, to: &str) -> Option<()> {
    let same = |(f, _): &&(String, String)| f.eq_ignore_ascii_case(from);
    match changed.iter().find(same) {
        Some((_, t)) => t.eq_ignore_ascii_case(to).then_some(()),
        None => {
            changed.push((from.to_string(), to.to_string()));
            Some(())
        }
    }
}

/// A text's words, without the marks that end them.
fn words(text: &str) -> Vec<String> {
    let bare = |w: &str| w.trim_end_matches(['.', '?', '!', ',']).to_string();
    text.split_whitespace()
        .map(bare)
        .filter(|w| !w.is_empty())
        .collect()
}

/// A sentence with each word equal to one (ignoring case) put as another,
/// its end mark kept; none if it holds no such word.
fn swapped(sentence: &str, from: &str, to: &str) -> Option<String> {
    let tokens: Vec<&str> = sentence.split(' ').collect();
    if !tokens.iter().any(|w| core(w).eq_ignore_ascii_case(from)) {
        return None;
    }
    let swap = |w: &&str| match core(w).eq_ignore_ascii_case(from) {
        true => format!("{to}{}", &w[core(w).len()..]),
        false => w.to_string(),
    };
    Some(tokens.iter().map(swap).collect::<Vec<_>>().join(" "))
}

/// A word without the mark that ends it.
fn core(w: &str) -> &str {
    w.trim_end_matches(['.', '?', '!', ','])
}

#[cfg(test)]
mod tests {
    use super::*;
    use format::record::EntryKind;

    fn pair(english: &str, ace: &str) -> Pair {
        Pair {
            english: english.into(),
            ace: ace.into(),
        }
    }

    #[test]
    fn a_number_changed_is_swapped_into_the_ace() {
        // Catches the earlier ACE offered unchanged, or its end mark lost.
        let p = pair(
            "the flight was in 1989",
            "The date of buran_flight is 1989.",
        );
        let got = adapted(&p, "the flight was in 1990");
        assert_eq!(got.as_deref(), Some("The date of buran_flight is 1990."));
    }

    #[test]
    fn a_name_changed_is_swapped_wherever_the_ace_holds_it() {
        // Catches a name swapped once where the ACE holds it twice, or
        // case taken for a difference.
        let p = pair("Buran waits and Buran rests", "Buran waits. Buran rests.");
        let got = adapted(&p, "energia waits and Energia rests");
        assert_eq!(got.as_deref(), Some("energia waits. energia rests."));
        let two_ways = adapted(&p, "energia waits and Polyus rests");
        assert_eq!(two_ways, None);
        let same = adapted(&pair("John waits", "John waits."), "john waits.");
        assert_eq!(same.as_deref(), Some("John waits."));
        // Changed in one place and kept in another: swapping it everywhere
        // says what was not typed (the code review, E4).
        let p = pair("Buran waits for Buran", "Buran waits for Buran.");
        assert_eq!(adapted(&p, "Energia waits for Buran"), None);
    }

    #[test]
    fn a_word_the_ace_does_not_hold_cannot_be_swapped() {
        // Catches a pair adapted though its ACE does not hold the word that
        // changed: flight is not in it, so launch has nowhere to go.
        let p = pair(
            "the flight was in 1989",
            "The date of buran_flight is 1989.",
        );
        assert_eq!(adapted(&p, "the launch was in 1989"), None);
    }

    #[test]
    fn texts_too_unlike_are_not_adapted() {
        // Catches a pair adapted to a text of other length, to one differing
        // in three words (though fewer than half), or in half its words, or
        // to an empty text.
        let p = pair(
            "the flight was in 1989",
            "The date of buran_flight is 1989.",
        );
        assert_eq!(adapted(&p, "the flight was in 1989 too"), None);
        assert_eq!(adapted(&p, "a flight is at 1990"), None);
        let long = pair(
            "the flight of the rocket was in 1989",
            "The date of the flight of the rocket is 1989.",
        );
        assert!(adapted(&long, "the flight of the probe was in 1990").is_some());
        assert_eq!(adapted(&long, "the launch of the probe was in 1990"), None);
        let short = pair("John waits", "John waits.");
        assert_eq!(adapted(&short, "Mary waits"), None);
        assert_eq!(adapted(&pair("", "John waits."), ""), None);
    }

    /// Supposition entries with these bodies, numbered from 1, each after
    /// the last.
    fn entries(bodies: &[serde_json::Value]) -> Vec<Entry> {
        let mut es: Vec<Entry> = Vec::new();
        for b in bodies {
            let e = next_entry(&es, b);
            es.push(e);
        }
        es
    }

    /// A supposition entry of a body after these.
    fn next_entry(es: &[Entry], body: &serde_json::Value) -> Entry {
        let parent = es.last().map(|e| (e.seq, e.hash.as_str()));
        Entry::new(
            es.len() as u64 + 1,
            parent,
            EntryKind::Suppose,
            body.clone(),
        )
        .unwrap()
    }

    #[test]
    fn the_journals_pairs_are_read_latest_first() {
        // Catches a supposition with no free English taken for a pair, or
        // the pairs read oldest first, so an old pair outranks a new one.
        let es = entries(&[
            serde_json::json!({"name": "John waits.", "english": "John waited"}),
            serde_json::json!({"name": "a world"}),
            serde_json::json!({"name": "Mary waits.", "english": "Mary waited"}),
        ]);
        let english: Vec<String> = pairs_of(&es).into_iter().map(|p| p.english).collect();
        assert_eq!(english, ["Mary waited", "John waited"]);
    }

    #[test]
    fn the_latest_pair_that_fits_is_proposed() {
        // Catches an earlier pair preferred to a later one, or a pair that
        // does not fit proposed.
        let pairs = [
            pair(
                "the flight was in 1989",
                "The date of buran_flight is 1989.",
            ),
            pair(
                "the flight was in 1988",
                "The year of buran_flight is 1988.",
            ),
            pair("John waited", "John waits."),
        ];
        let (ace, from) = proposal(&pairs, "the flight was in 2001").unwrap();
        assert_eq!(ace, "The date of buran_flight is 2001.");
        assert_eq!(from, &pairs[0]);
        assert!(proposal(&pairs, "something else entirely").is_none());
    }
}
