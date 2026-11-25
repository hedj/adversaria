//! Near matches: where a text holds something a few edits from a
//! pattern, so that Leap can warn before it makes an item for a pattern
//! that is likely a typo of one that exists ("Korolov" for "Korolev").
//! Sellers' algorithm: the fewest edits (insertions, deletions,
//! substitutions) between the pattern and any stretch of the text, in
//! time proportional to their lengths multiplied.

use crate::app::{Active, App, Near};
use crate::outline::Outline;

/// The edits a pattern may be from a stretch of text and still be near:
/// none for fewer than four characters, one up to seven, else two.
pub fn edits_allowed(pattern: &str) -> usize {
    match pattern.chars().count() {
        0..=3 => 0,
        4..=7 => 1,
        _ => 2,
    }
}

/// Whether two words are one edit apart, ignoring case: a letter put in,
/// taken out or changed, or two side by side swapped. Equal words are
/// not.
pub fn one_edit_apart(a: &str, b: &str) -> bool {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    let same = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let (a, b) = (&a[same..], &b[same..]);
    match (a.len(), b.len()) {
        (0, 0) => false,
        (m, n) if m == n => a[1..] == b[1..] || swapped(a, b),
        (m, n) if m + 1 == n => a == &b[1..],
        (m, n) if n + 1 == m => &a[1..] == b,
        _ => false,
    }
}

/// Whether two equally long runs of letters, which differ at their first,
/// are the same but for their first two swapped.
fn swapped(a: &[char], b: &[char]) -> bool {
    a.len() >= 2 && a[0] == b[1] && a[1] == b[0] && a[2..] == b[2..]
}

/// The stretch of a text nearest a pattern, ignoring case, and how many
/// edits apart they are, if it is near: within the edits allowed, and
/// at least one (an exact match is not near, it is a match).
pub fn near_in(text: &str, pattern: &str) -> Option<(usize, String)> {
    let (edits, found) = nearest_stretch(text, pattern)?;
    let near = (1..=edits_allowed(pattern)).contains(&edits);
    near.then_some((edits, found))
}

/// The stretch of a text fewest edits from a pattern, ignoring case, and
/// how many.
fn nearest_stretch(text: &str, pattern: &str) -> Option<(usize, String)> {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (edits, from, to) = nearest(&p, &t)?;
    Some((edits, t[from..to].iter().collect()))
}

impl App {
    /// What a Leap with no match warns of: the item nearest its pattern,
    /// counted from where it began, the fewest edits, the first of those.
    pub(crate) fn near_match(&self, a: &Active, pattern: &str) -> Option<Near> {
        let outline = &self.notebook.outline;
        let (found, item) = nearest_item(outline, self.leap_round(a), pattern)?;
        let place = outline.number(item);
        Some(Near {
            pattern: pattern.into(),
            place,
            found,
        })
    }
}

/// Of some items, in order, the one nearest a pattern, and what of its
/// text is near: the fewest edits, the first of those.
fn nearest_item(outline: &Outline, items: Vec<u64>, pattern: &str) -> Option<(String, u64)> {
    let all = items
        .into_iter()
        .filter_map(|i| near_item(outline, i, pattern));
    let ((_, found), item) = all.min_by_key(|((edits, _), _)| *edits)?;
    Some((found, item))
}

/// What of an item's text is near a pattern, if any is, with the item.
fn near_item(outline: &Outline, i: u64, pattern: &str) -> Option<((usize, String), u64)> {
    Some((near_in(&outline.text(i), pattern)?, i))
}

/// The fewest edits between a pattern and a stretch of a text, and where
/// that stretch starts and ends: the first such stretch.
fn nearest(p: &[char], t: &[char]) -> Option<(usize, usize, usize)> {
    let mut column: Vec<(usize, usize)> = (0..=p.len()).map(|i| (i, 0)).collect();
    let mut best = (column[p.len()].0, 0, 0);
    for (j, c) in t.iter().enumerate() {
        column = next_column(&column, p, *c, j);
        if column[p.len()].0 < best.0 {
            best = (column[p.len()].0, column[p.len()].1, j + 1);
        }
    }
    (!p.is_empty()).then_some(best)
}

/// The next column of edit counts, each with where its stretch starts,
/// after a character of the text at a place: the pattern may start
/// anywhere, so the top row is free.
fn next_column(before: &[(usize, usize)], p: &[char], c: char, j: usize) -> Vec<(usize, usize)> {
    let mut next = vec![(0, j + 1)];
    let lower: String = c.to_lowercase().collect();
    for i in 1..before.len() {
        let same = lower.chars().eq(std::iter::once(p[i - 1]));
        let diagonal = (before[i - 1].0 + usize::from(!same), before[i - 1].1);
        let up = (next[i - 1].0 + 1, next[i - 1].1);
        let left = (before[i].0 + 1, before[i].1);
        next.push(*[diagonal, up, left].iter().min_by_key(|(e, _)| *e).unwrap());
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_one_edit_apart_differ_by_one_letter_or_one_swap() {
        // Catches an edit missed (put in, taken out, changed, swapped), two
        // edits taken for one, case taken for an edit, or a word taken as
        // an edit from itself.
        let one = [
            ("fnd", "find"),
            ("deleete", "delete"),
            ("lonk", "link"),
            ("fnid", "find"),
            ("FND", "find"),
            ("köln", "koln"),
        ];
        for (a, b) in one {
            assert!(one_edit_apart(a, b), "{a} {b}");
        }
        for (a, b) in [
            ("find", "find"),
            ("fold", "find"),
            ("fn", "find"),
            ("dlte", "delete"),
        ] {
            assert!(!one_edit_apart(a, b), "{a} {b}");
        }
    }

    #[test]
    fn a_typo_is_near_and_names_what_it_is_near() {
        assert_eq!(
            near_in("notes on Korolev's design", "korolov"),
            Some((1, "Korolev".into()))
        );
    }

    #[test]
    fn an_exact_match_or_one_too_far_is_not_near() {
        assert_eq!(near_in("Korolev", "korolev"), None, "a match");
        assert_eq!(near_in("Korolev", "kerolov"), None, "two edits in seven");
        assert_eq!(near_in("Kor", "kar"), None, "too short to guess");
    }

    #[test]
    fn a_longer_pattern_may_be_two_edits_away() {
        assert_eq!(
            near_in("the Energia launcher", "energya launchr"),
            Some((2, "Energia launch".into())),
            "the first stretch two edits away"
        );
    }

    #[test]
    fn an_empty_text_or_pattern_is_near_nothing() {
        assert_eq!(near_in("", "korolev"), None);
        assert_eq!(near_in("Korolev", ""), None);
    }
}
