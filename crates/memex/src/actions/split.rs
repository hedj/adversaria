//! Splitting an item into its sentences, as a fleeting note is made into
//! atomic ones: the item keeps its first sentence, with its links and its
//! address; each other sentence becomes a new item after it, linked back
//! as split_from. The new items are made of the original's spans, so
//! their characters keep their addresses and their history. A closing
//! record names the split's first entry, so one undo takes it all back.

use crate::app::App;
use crate::outline::{Span, span_json};
use serde_json::json;
use std::ops::Range;

impl App {
    pub(crate) fn cmd_split(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        self.flush_text();
        let text: Vec<char> = self.notebook.outline.text(target).chars().collect();
        let parts = sentences(&text);
        if parts.len() < 2 {
            return Err("nothing to split: the target's text is one sentence".into());
        }
        let split = self.split_of(target);
        let slices = self.later_parts(target, &parts);
        self.keep_first(&split, &text[parts[0].clone()])?;
        let made = self.make_parts(&split, slices)?;
        self.close_split(&split, made)?;
        Ok(format!("split into {} items", parts.len()))
    }

    /// The record that closes a split: the item, the split's first entry
    /// and the new items, so that one undo takes it all back.
    fn close_split(&mut self, split: &Split, made: Vec<u64>) -> Result<(), String> {
        let body = json!({"op": "split", "item": split.item, "of": split.first, "parts": made});
        self.write_here("edit", body).map(|_| ())
    }

    /// A split of an item that starts at the next entry.
    fn split_of(&self, item: u64) -> Split {
        let first = self.notebook.next_seq();
        Split { item, first }
    }

    /// The spans of an item's parts after the first.
    fn later_parts(&self, item: u64, parts: &[Range<usize>]) -> Vec<Vec<Span>> {
        let outline = &self.notebook.outline;
        let later = parts[1..].iter();
        later
            .map(|Range { start, end }| outline.slice(item, *start, *end))
            .collect()
    }

    /// The item cut to its first part, which waits to be read.
    fn keep_first(&mut self, split: &Split, first: &[char]) -> Result<(), String> {
        self.write_text(split.item, first)?;
        self.will_read(split.item, split.first);
        Ok(())
    }

    /// An item for each later part, each after the last, made of its spans;
    /// the new items.
    fn make_parts(&mut self, split: &Split, slices: Vec<Vec<Span>>) -> Result<Vec<u64>, String> {
        let mut after = split.item;
        let mut made = Vec::new();
        for spans in slices {
            after = self.make_part(split, after, &spans)?;
            made.push(after);
        }
        Ok(made)
    }

    /// A part after an item, linked back to the item split, waiting to be
    /// read as part of the split.
    fn make_part(&mut self, split: &Split, after: u64, spans: &[Span]) -> Result<u64, String> {
        let part = self.create_after(after, "", span_json(spans))?;
        self.write_split_link(part, split.item)?;
        self.will_read(part, split.first);
        Ok(part)
    }
}

/// A split being made: the item split, and the split's first entry.
struct Split {
    item: u64,
    first: u64,
}

/// A text's sentences: each ends after a full stop, question mark or
/// exclamation mark that ends a sentence (sentence_ends), or at the
/// text's end; the spaces between are left out, and so is a part of
/// spaces alone.
pub(crate) fn sentences(text: &[char]) -> Vec<Range<usize>> {
    let mut parts = Vec::new();
    let mut start = 0;
    for i in 0..text.len() {
        if sentence_ends(text, i) {
            parts.push(start..i + 1);
            start = i + 1;
        }
    }
    parts.push(start..text.len());
    let parts = parts.into_iter().map(|r| trimmed(text, r));
    parts.filter(|r| !r.is_empty()).collect()
}

/// Whether the mark at i ends a sentence: a full stop, question mark or
/// exclamation mark, then a space, then a word that starts one (a capital,
/// a digit, a quote or a bracket); and, for a full stop, the word before
/// it is not an abbreviation ("e.g.", "Dr.") or an initial ("J.").
fn sentence_ends(text: &[char], i: usize) -> bool {
    let mark = matches!(text[i], '.' | '?' | '!');
    let spaced = text.get(i + 1).is_some_and(|c| c.is_whitespace());
    mark && spaced && starts_sentence(&text[i + 1..]) && !(text[i] == '.' && abbreviated(text, i))
}

/// Whether the first word after the spaces starts a sentence: a capital,
/// a digit, a quote or a bracket.
fn starts_sentence(rest: &[char]) -> bool {
    let next = rest.iter().find(|c| !c.is_whitespace());
    next.is_some_and(|c| c.is_uppercase() || c.is_ascii_digit() || "\"'“‘([".contains(*c))
}

/// Whether the word before a full stop at i is an abbreviation or an
/// initial.
fn abbreviated(text: &[char], i: usize) -> bool {
    let start = text[..i]
        .iter()
        .rposition(|c| c.is_whitespace())
        .map_or(0, |s| s + 1);
    let word: String = text[start..i].iter().collect::<String>().to_lowercase();
    let initial = word.chars().count() == 1 && word.chars().all(char::is_alphabetic);
    initial || ABBREVIATIONS.contains(&word.trim_start_matches(['(', '"', '\'']))
}

/// Words a full stop shortens, not ends: "e.g.", "Dr." and the like.
const ABBREVIATIONS: [&str; 26] = [
    "e.g", "i.e", "etc", "cf", "vs", "viz", "approx", "no", "nos", "fig", "figs", "p", "pp", "vol",
    "ed", "eds", "mr", "mrs", "ms", "dr", "prof", "st", "jr", "sr", "al", "ca",
];

/// A range without the spaces at its ends.
fn trimmed(text: &[char], r: Range<usize>) -> Range<usize> {
    let start = (r.start..r.end)
        .find(|i| !text[*i].is_whitespace())
        .unwrap_or(r.end);
    let end = (start..r.end)
        .rev()
        .find(|i| !text[*i].is_whitespace())
        .map_or(start, |i| i + 1);
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(s: &str) -> Vec<String> {
        let text: Vec<char> = s.chars().collect();
        sentences(&text)
            .into_iter()
            .map(|r| text[r].iter().collect())
            .collect()
    }

    #[test]
    fn a_text_splits_at_each_sentence_end() {
        // Catches a sentence end missed, the spaces between kept, a
        // decimal point taken for an end, or an empty part kept.
        assert_eq!(
            parts("Buran flew in 1988.  It flew once! Why only once?  "),
            ["Buran flew in 1988.", "It flew once!", "Why only once?"]
        );
        assert_eq!(parts("Pi is 3.14 and no more"), ["Pi is 3.14 and no more"]);
        // Catches an abbreviation, an initial or a lower-case word after a
        // full stop taken for a sentence end.
        assert_eq!(
            parts("Shuttles, e.g. Buran, flew. Dr. Smith and J. Jones wrote it. see p. 4 too."),
            [
                "Shuttles, e.g. Buran, flew.",
                "Dr. Smith and J. Jones wrote it. see p. 4 too."
            ]
        );
        assert_eq!(
            parts("It flew. \"Once,\" they said."),
            ["It flew.", "\"Once,\" they said."]
        );
        assert!(parts("   ").is_empty());
    }
}
