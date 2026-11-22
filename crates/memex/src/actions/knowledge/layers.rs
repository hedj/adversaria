//! Views (docs/guide.md, § 3.3): `show` makes an item a chart, a table
//! or a graph of the world's facts, from the guide's recipes; behind it,
//! the view grammar, a layer: a mark drawn once for each solution of a
//! goal (docs/concepts.md, decision 6).

use super::all_bound;
use crate::app::App;
use crate::view::channels;
use format::terms::Term;
use serde_json::json;

/// The layers each kind of view `show` makes, as `layer` takes them: the
/// guide's recipes for a bar chart of the boxes, a table of them, and the
/// graph of the notes' links.
const SHOWN_AS: &[(&str, &[&str])] = &[
    ("chart", &["bar(x(C), y(V)) where box(C, V)"]),
    (
        "table",
        &[
            "cell(x(a), y(C), text(C)) where box(C, _)",
            "cell(x(b), y(C), text(V)) where box(C, V)",
        ],
    ),
    (
        "graph",
        &[
            "node(id(I), text(T)) where item_text(I, T), (item_link(I, _, _) ; item_link(_, I, _))",
            "edge(from(F), to(T)) where item_link(F, T, _)",
        ],
    ),
];

impl App {
    /// Show an item as a chart, a table or a graph (`show 2 as chart`, or
    /// the target with no address); else add the layer the words write.
    pub(crate) fn cmd_show(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let at = words.iter().position(|w| *w == "as").unwrap_or(0);
        let (address, kind) = match words.get(at..) {
            Some(["as", kind]) => (&words[..at], *kind),
            _ => return self.cmd_layer(words, target),
        };
        let layers = SHOWN_AS.iter().find(|(k, _)| *k == kind);
        let (_, layers) =
            layers.ok_or("show (as) chart, table or graph, or a mark where a goal")?;
        let item = match address {
            [] => target,
            a => self
                .address(a)
                .ok_or(format!("no item is “{}”", a.join(" ")))?,
        };
        for layer in *layers {
            self.cmd_layer(&[layer], item)?;
        }
        Ok(format!("shown as a {kind}"))
    }

    pub(crate) fn cmd_layer(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let (mark_text, goal_text) = split_where(&words.join(" "));
        let (mark, goal) = self.parse_layer(&mark_text, &goal_text)?;
        self.keep_layer(target, (&mark_text, &goal_text), mark, goal)?;
        Ok(format!("layer added: {mark_text}"))
    }

    /// A mark and a goal, parsed as one term so they share their variables.
    fn parse_layer(&mut self, mark: &str, goal: &str) -> Result<(Term, Term), String> {
        let (t, names) = self.parse_term(
            &format!("layer(({mark}), ({goal}))"),
            "not a mark and a goal",
        )?;
        let (mark, goal) = layer_parts(&t, &names)?;
        self.vet_definition(&goal, "layer not kept")?;
        Ok((mark, goal))
    }

    fn keep_layer(
        &mut self,
        item: u64,
        (m, g): (&str, &str),
        mark: Term,
        goal: Term,
    ) -> Result<(), String> {
        let defines = Term::Compound("layer".into(), vec![mark, goal]);
        self.write_node_def(
            json!({"op": "layer", "item": item, "mark": m, "goal": g}),
            defines,
        )
    }
}

/// A layer's mark and goal: the mark one the views can draw, and each of
/// its variables bound by the goal.
fn layer_parts(t: &Term, names: &[String]) -> Result<(Term, Term), String> {
    // Text can close the brackets the mark and goal are read in, or end
    // them with a comment, so the term read need not be layer/2.
    let (mark, goal) = match t {
        Term::Compound(f, a) if f == "layer" && a.len() == 2 => (a[0].clone(), a[1].clone()),
        _ => return Err("layer not kept: that is no mark and goal".into()),
    };
    channels(&mark).map_err(|e| format!("layer not kept: {e}"))?;
    all_bound(&mark, &goal, names, ("the mark", "the goal"))
        .map_err(|v| format!("layer not kept: {v}"))?;
    Ok((mark, goal))
}

/// The mark and the goal of "MARK where GOAL"; the goal is true if there
/// is no where.
fn split_where(text: &str) -> (String, String) {
    match text.split_once(" where ") {
        Some((m, g)) => (
            m.trim().to_string(),
            g.trim().trim_end_matches('.').to_string(),
        ),
        None => (
            text.trim().trim_end_matches('.').to_string(),
            "true".to_string(),
        ),
    }
}
