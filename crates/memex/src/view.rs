//! Views: graphics declared over Prolog goals (docs/design.md § Views).
//!
//! An item becomes a view when it has layers. A layer is a mark and a goal,
//! `layer MARK where GOAL`: each solution of the goal gives one mark, with
//! the facts it rests on. There are no chart types. A chart, a timeline, a
//! spreadsheet grid or a flowchart is marks over the user's own facts and
//! rules, after the grammar of graphics (Wilkinson; Wickham 2010), Vega-Lite
//! (Satyanarayan et al. 2017) and Observable Plot ("Plot doesn't have chart
//! types; instead, you construct charts by layering marks").
//!
//! A mark is a term: its name, then channels, each `channel(Value)`:
//!
//! | Mark | Channels (required, then optional) | Draws |
//! | --- | --- | --- |
//! | `dot` | x, y; text | a point, labelled |
//! | `bar` | x, y; text | a bar from zero: vertical if x is a category, horizontal if y is |
//! | `line` | x, y; series | points joined in order of x, one line per series |
//! | `rule` | x or y | a reference line across the plot |
//! | `text` | x, y, text | a label at a point |
//! | `tick` | x; y, text | a mark on an axis, labelled: a timeline |
//! | `span` | x1, x2; y, text | an interval, on a lane y: a Gantt chart |
//! | `cell` | x, y; text | a box in a grid: a table, a spreadsheet |
//! | `node` | id; text, shape | a box in a graph, laid out in layers; shape box, round or decision |
//! | `edge` | from, to; text | an arrow between two nodes, by id |
//!
//! Values are numbers, atoms, strings, or date(Y, M, D). Scales follow from
//! them: numbers and dates are positions on a line; anything else is a
//! category, in standard order.

use format::terms::Term;

/// A mark's channels, by name, in the order written.
pub type Channels = Vec<(String, Term)>;

/// The kinds of mark, their required and optional channels.
pub const MARKS: &[(&str, &[&str], &[&str])] = &[
    ("dot", &["x", "y"], &["text"]),
    ("bar", &["x", "y"], &["text"]),
    ("line", &["x", "y"], &["series"]),
    ("rule", &[], &["x", "y"]),
    ("text", &["x", "y", "text"], &[]),
    ("tick", &["x"], &["y", "text"]),
    ("span", &["x1", "x2"], &["y", "text"]),
    ("cell", &["x", "y"], &["text"]),
    ("node", &["id"], &["text", "shape"]),
    ("edge", &["from", "to"], &["text"]),
];

/// The channels of a mark term, by name: `bar(x(C), y(V))` gives
/// [("x", C), ("y", V)]. Err says what is wrong, in words.
pub fn channels(mark: &Term) -> Result<(&'static str, Channels), String> {
    let (name, args) = match mark {
        Term::Compound(n, a) => (n.as_str(), a.as_slice()),
        Term::Atom(n) => (n.as_str(), &[][..]),
        _ => return Err("a mark is a term such as bar(x(C), y(V))".into()),
    };
    let Some((kind, required, optional)) = MARKS.iter().find(|(k, _, _)| *k == name) else {
        let names: Vec<&str> = MARKS.iter().map(|(k, _, _)| *k).collect();
        return Err(format!(
            "no mark “{name}”; the marks are {}",
            names.join(", ")
        ));
    };
    let mut out: Vec<(String, Term)> = Vec::new();
    for a in args {
        let Term::Compound(c, v) = a else {
            return Err(format!(
                "{name}: each channel is written channel(Value), as x(C)"
            ));
        };
        if v.len() != 1 {
            return Err(format!("{name}: {c} takes one value"));
        }
        if !required.contains(&c.as_str()) && !optional.contains(&c.as_str()) {
            let all: Vec<&str> = required.iter().chain(optional.iter()).copied().collect();
            return Err(format!(
                "{name} has no channel {c}; its channels are {}",
                all.join(", ")
            ));
        }
        if out.iter().any(|(k, _)| k == c) {
            return Err(format!("{name}: {c} is given twice"));
        }
        out.push((c.clone(), v[0].clone()));
    }
    for r in *required {
        if !out.iter().any(|(k, _)| k == r) {
            return Err(format!("{name} needs {r}(…)"));
        }
    }
    if *kind == "rule" && out.len() != 1 {
        return Err("rule needs exactly one of x(…) or y(…)".into());
    }
    Ok((kind, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(f: &str, a: Vec<Term>) -> Term {
        Term::Compound(f.into(), a)
    }

    #[test]
    fn marks_are_checked() {
        // Catches a mark accepted with a missing, unknown or doubled channel.
        let c = |n: &str| t(n, vec![Term::Var(0)]);
        assert!(channels(&t("bar", vec![c("x"), c("y")])).is_ok());
        assert!(channels(&t("rule", vec![c("y")])).is_ok());
        let cases = [
            (t("pie", vec![c("x")]), "no mark “pie”"),
            (t("bar", vec![c("x")]), "bar needs y(…)"),
            (
                t("bar", vec![c("x"), c("y"), c("colour")]),
                "no channel colour",
            ),
            (t("bar", vec![c("x"), c("x"), c("y")]), "x is given twice"),
            (t("rule", vec![]), "exactly one"),
            (t("rule", vec![c("x"), c("y")]), "exactly one"),
            (
                t("bar", vec![Term::Atom("x".into()), c("y")]),
                "channel(Value)",
            ),
        ];
        for (m, why) in cases {
            let e = channels(&m).unwrap_err();
            assert!(e.contains(why), "{m:?}: {e}");
        }
    }
}
