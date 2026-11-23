//! The notebook's inspector (docs/concepts-seven.md, change 1): with the
//! cursor above the first item, what waits for the learner to decide,
//! every offer in one row each, the most pressing first.

use super::{Presented, dim, plain};
use crate::app::App;
use crate::offers::Open;
use ratatui::text::Line;

/// How many offers the inspector lists; the rest it counts.
const LISTED: usize = 12;

/// The notebook's inspector: how many offers wait, each listed up to a
/// number, the rest counted, and how to decide one.
pub(super) fn notebook_lines(app: &App) -> Vec<Presented> {
    let open = app.open_offers();
    let heading = format!("THE NOTEBOOK · {} OPEN OFFERS", open.len());
    let mut lines = plain(vec![Line::styled(heading, dim()), Line::raw("")]);
    lines.extend(open.iter().take(LISTED).flat_map(offer_row));
    lines.extend(plain(vec![
        Line::raw(""),
        Line::styled(foot(open.len()), dim()),
    ]));
    lines
}

/// An offer in one row: its kind and what it offers, then, dim, its
/// source; both act on what it presents.
pub(super) fn offer_row(o: &Open) -> Vec<Presented> {
    let what = Line::raw(format!("{}  {}", o.kind, o.what));
    let from = Line::styled(format!("   from {}", o.source), dim());
    let on = || Some(o.presents.clone());
    vec![(what, on()), (from, on())]
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
