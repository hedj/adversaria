//! Statements written in Prolog, between backticks, in an item's text
//! (docs/concepts.md): what controlled English cannot say, said as Prolog,
//! and read into the same kinds as a sentence is read into. The prose
//! around them is read as a sentence would be, if it is one.
//!
//! | Written | Kind |
//! | --- | --- |
//! | `` `date(buran_flight, 1988).` `` | a fact |
//! | `` `claims("Talk-notes", date(buran_flight, 1989)).` `` | a claim of its source |
//! | `` `cost(C) :- rent(R), food(F), C is R + F.` `` | a rule |
//! | `` `false :- date(E, A), date(E, B), A \= B.` `` | a constraint |
//! | `` `?- rest(Answer).` `` | a question, answered under the item |
//! | `` The rest is {R}. `rest(R)` `` | derived text: the item shows it filled |

use crate::ace::{Kind, Said};

/// The statements a text's backticked Prolog makes, and the text outside
/// the backticks. A text with a {Var} outside them is a template, and its
/// backticked Prolog the goal that fills it.
pub(crate) fn read(text: &str) -> (Vec<Said>, String) {
    let (spans, outside) = split(text);
    if spans.is_empty() {
        return (vec![], outside);
    }
    if outside.contains('{') {
        let goal = spans.join(", ");
        let kind = Kind::Template(outside.trim().to_string());
        return (vec![Said { kind, text: goal }], String::new());
    }
    (spans.iter().map(|s| statement(s)).collect(), outside)
}

/// The text between backticks, each span trimmed and without its full
/// stop, and the text outside them. A backtick left open is text.
fn split(text: &str) -> (Vec<String>, String) {
    let (mut spans, mut outside) = (vec![], String::new());
    let mut parts = text.split('`');
    outside.push_str(parts.next().unwrap_or(""));
    while let Some(inside) = parts.next() {
        let Some(after) = parts.next() else {
            outside.push('`');
            outside.push_str(inside);
            break;
        };
        let span = inside.trim().trim_end_matches('.').trim();
        if !span.is_empty() {
            spans.push(span.to_string());
        }
        outside.push(' ');
        outside.push_str(after);
    }
    (spans, outside)
}

/// One clause of Prolog as a statement of its kind.
fn statement(clause: &str) -> Said {
    let said = |kind, text: &str| Said {
        kind,
        text: text.trim().to_string(),
    };
    if let Some(goal) = clause.strip_prefix("?-") {
        return said(Kind::Question, goal);
    }
    if clause.starts_with("false") && clause.contains(":-") {
        return said(Kind::Constraint, clause);
    }
    if clause.contains(":-") {
        return said(Kind::Rule, clause);
    }
    match claim(clause) {
        Some((source, fact)) => said(Kind::Claim(source), &fact),
        None => said(Kind::Fact, clause),
    }
}

/// The source and the fact of claims(Source, Fact), the source unquoted.
fn claim(clause: &str) -> Option<(String, String)> {
    let args = clause.strip_prefix("claims(")?.strip_suffix(')')?;
    let comma = top_level_comma(args)?;
    let source = args[..comma].trim().trim_matches(['"', '\'']).to_string();
    Some((source, args[comma + 1..].trim().to_string()))
}

/// Where the first comma outside brackets and quotes is.
fn top_level_comma(text: &str) -> Option<usize> {
    let (mut depth, mut quote) = (0i32, None);
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), _) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth -= 1,
            (None, ',') if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(Kind, String)> {
        read(text).0.into_iter().map(|s| (s.kind, s.text)).collect()
    }

    #[test]
    fn each_kind_of_clause_is_read_as_its_kind() {
        // Catches a clause of one kind taken for another, or its text lost.
        let fact = (Kind::Fact, "date(b, 1988)".to_string());
        assert_eq!(kinds("`date(b, 1988).`"), [fact]);
        let rule = "cost(C) :- rent(R), C is R";
        assert_eq!(kinds(&format!("`{rule}.`")), [(Kind::Rule, rule.into())]);
        let denial = "false :- date(E, A), date(E, B), A \\= B";
        assert_eq!(
            kinds(&format!("`{denial}`")),
            [(Kind::Constraint, denial.into())]
        );
        assert_eq!(
            kinds("`?- rest(Answer).`"),
            [(Kind::Question, "rest(Answer)".into())]
        );
        let claim = (
            Kind::Claim("Talk-notes".into()),
            "date(b, f(1, 2))".to_string(),
        );
        assert_eq!(
            kinds("`claims(\"Talk-notes\", date(b, f(1, 2))).`"),
            [claim]
        );
    }

    #[test]
    fn prose_around_clauses_is_kept_and_a_template_fills_from_its_goal() {
        // Catches the prose dropped, which may itself be a sentence, or a
        // template's goal stated as a fact.
        let (said, outside) = read("Buran flew once: `flights(b, 1).` See below.");
        assert_eq!(said.len(), 1);
        assert_eq!(outside, "Buran flew once:   See below.");
        let template = "The rest is {R}.";
        let (said, _) = read(&format!("{template} `rest(R)`"));
        assert_eq!(said[0].kind, Kind::Template(template.into()));
        assert_eq!(said[0].text, "rest(R)");
    }

    #[test]
    fn a_text_without_backticks_or_with_one_open_states_nothing() {
        assert_eq!(read("No Prolog here.").0, []);
        assert_eq!(read("").0, []);
        let (said, outside) = read("a ` b");
        assert!(said.is_empty());
        assert_eq!(outside, "a ` b");
        assert!(read("``").0.is_empty(), "an empty span");
    }
}
