//! The layer command: a mark drawn once for each solution of a goal
//! (docs/guide.md, § 3.3).

use super::all_bound;
use crate::app::App;
use crate::view::channels;
use format::terms::Term;
use serde_json::json;

impl App {
    pub(crate) fn cmd_layer(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let (mark_text, goal_text) = split_where(&words.join(" "));
        let (mark, goal) = self.parse_layer(&mark_text, &goal_text)?;
        self.keep_layer(target, (&mark_text, &goal_text), mark, goal)?;
        self.screen.message = format!("layer kept: {mark_text}");
        Ok(())
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
    let Term::Compound(_, a) = t else {
        unreachable!()
    };
    let (mark, goal) = (a[0].clone(), a[1].clone());
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
