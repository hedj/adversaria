//! What a find lists (docs/concepts.md, concept 10): the items that hold
//! a text, that carry a label, or that a kind of finding marks. Leap
//! finds a text too, so the list after a leap is its matches.

use crate::marks::item_marks;
use crate::outline::{Outline, tags};
use crate::reason::WorldEval;

#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    /// The items whose text holds it, in any case.
    Text(String),
    /// The items that link to a page, by a label or a [[link]]: the page,
    /// by its key (docs/concepts-seven.md, change 4).
    Label(String),
    /// The items a violation marks if loud, else a disagreement.
    Marked { loud: bool },
}

impl Found {
    /// What words after `find` list: a kind of finding by its name, a
    /// label by `#`, else a text.
    pub fn of(words: &str) -> Found {
        match words {
            "violations" => Found::Marked { loud: true },
            "disagreements" => Found::Marked { loud: false },
            _ => match tags(words).as_slice() {
                [label] if !words.contains(' ') => Found::Label(label.clone()),
                _ => Found::Text(words.to_string()),
            },
        }
    }

    /// The words that find it again, as the journal records them.
    pub fn words(&self) -> String {
        match self {
            Found::Text(t) => t.clone(),
            Found::Label(l) => format!("#{l}"),
            Found::Marked { loud: true } => "violations".into(),
            Found::Marked { loud: false } => "disagreements".into(),
        }
    }

    /// The list's heading: what it lists.
    pub fn heading(&self) -> String {
        match self {
            Found::Text(t) => format!("MATCHES OF “{t}”"),
            Found::Label(l) => format!("LINKED TO #{l}"),
            Found::Marked { loud: true } => "MARKED BY A VIOLATION".into(),
            Found::Marked { loud: false } => "MARKED BY A DISAGREEMENT".into(),
        }
    }

    /// The items it lists, in the outline's order; for a kind of finding,
    /// none until the world is evaluated.
    pub fn items(&self, outline: &Outline, eval: Option<&WorldEval>) -> Vec<u64> {
        let order = outline.in_order();
        match (self, eval) {
            (Found::Text(t), _) => outline.holding(order, t),
            (Found::Label(l), _) => outline.page_linkers(l),
            (Found::Marked { loud }, Some(e)) => marked(outline, order, e, *loud),
            (Found::Marked { .. }, None) => vec![],
        }
    }
}

/// The items, of those given, that a violation marks if loud, else a
/// disagreement, in a world's evaluation.
fn marked(outline: &Outline, items: Vec<u64>, e: &WorldEval, loud: bool) -> Vec<u64> {
    items
        .into_iter()
        .filter(|id| mark_kinds(e, outline, *id).contains(&loud))
        .collect()
}

/// The kind of each mark under an item: loud for a violation, quiet for a
/// disagreement.
fn mark_kinds(e: &WorldEval, outline: &Outline, item: u64) -> Vec<bool> {
    item_marks(e, outline, item)
        .iter()
        .map(|m| m.loud)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_name_a_kind_of_finding_a_label_or_a_text_and_find_it_again() {
        // Catches a label taken for a text (it would match #drafts), a
        // text taken for a label, or words that do not find the same list
        // again after a replay.
        for (words, found) in [
            ("violations", Found::Marked { loud: true }),
            ("disagreements", Found::Marked { loud: false }),
            ("#draft", Found::Label("draft".into())),
            ("#draft more", Found::Text("#draft more".into())),
            ("#Draft", Found::Label("draft".into())),
            ("#1", Found::Text("#1".into())),
            ("buran", Found::Text("buran".into())),
        ] {
            assert_eq!(Found::of(words), found, "{words}");
            assert_eq!(Found::of(&found.words()), found, "{words}");
        }
    }
}
