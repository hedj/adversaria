//! The notebook's inspector (docs/concepts-seven.md, changes 1 and 3):
//! with the cursor above the first item, what waits for the learner to
//! decide, every offer in one row each, the most pressing first; then
//! the history, the latest changes and jumps.

use super::offers::offer_row;
use super::{Presented, dim, plain};
use crate::app::App;
use crate::history::Step;
use crate::offers::LISTED;
use crate::present::Presentation;
use ratatui::text::Line;

/// How many steps of the history it lists.
const STEPS: usize = 8;

/// The notebook's inspector: what waits to be decided, then the history.
pub(super) fn notebook_lines(app: &App) -> Vec<Presented> {
    let mut lines = offer_lines(app);
    lines.extend(history_lines(app));
    lines
}

/// How many offers wait, each listed up to a number, the rest counted,
/// and how to decide one.
fn offer_lines(app: &App) -> Vec<Presented> {
    let open = app.open_offers();
    let heading = format!("THE NOTEBOOK · {} OPEN OFFERS", open.len());
    let mut lines = plain(vec![Line::styled(heading, dim()), Line::raw("")]);
    let row = |o| offer_row(o, None);
    lines.extend(open.iter().take(LISTED).flat_map(row));
    lines.extend(plain(vec![
        Line::raw(""),
        Line::styled(foot(open.len()), dim()),
    ]));
    lines
}

/// What the foot of the list says: that nothing waits, or how many more
/// do, and how to decide one.
fn foot(open: usize) -> String {
    let how = "a click goes to one, or adopts a suggestion · a right-click offers more";
    match open {
        0 => "nothing waits for you to decide".into(),
        n if n > LISTED => format!("{} more · {how}", n - LISTED),
        _ => how.into(),
    }
}

/// The latest steps of the history, newest first, each by its entry.
fn history_lines(app: &App) -> Vec<Presented> {
    let steps = app.history(STEPS);
    let heading = Line::styled("HISTORY · NEWEST FIRST", dim());
    let mut lines = plain(vec![Line::raw(""), heading, Line::raw("")]);
    lines.extend(steps.iter().map(step_row));
    if steps.is_empty() {
        lines.push((Line::styled("nothing done yet", dim()), None));
    }
    lines
}

/// A step: its entry's number and what it did; dim, and said so, if a
/// later entry retracts it.
fn step_row(s: &Step) -> Presented {
    let said = format!("entry {}  {}", s.seq, s.said);
    let line = match s.retracted {
        true => Line::styled(format!("{said}  (retracted)"), dim()),
        false => Line::raw(said),
    };
    (line, Some(Presentation::Entry(s.seq)))
}
