//! One row for an offer (docs/concepts-seven.md, change 2): what it is
//! and what it offers, then, dim, where it came from. The notebook's
//! inspector, the suggestions pane, a claim's questions and a revision's
//! changes each draw their offers so.

use super::{Presented, dim};
use crate::offers::{Open, change_open};
use ratatui::text::Line;

/// An offer in two rows, both acting on what it presents; one already
/// decided is dim, and says how it was decided.
pub(super) fn offer_row(o: &Open, decided: Option<&str>) -> Vec<Presented> {
    let said = format!("{}  {}", o.kind, o.what);
    let what = match decided {
        Some(how) => Line::styled(format!("{said}  ({how})"), dim()),
        None => Line::raw(said),
    };
    let from = Line::styled(format!("   from {}", o.source), dim());
    let on = || Some(o.presents.clone());
    vec![(what, on()), (from, on())]
}

/// One change of a revision, from it: dim, and said so, if retracted.
pub(super) fn change_rows(
    world: &str,
    (seq, what, retracted): &(u64, String, bool),
) -> Vec<Presented> {
    let change = change_open(*seq, what.clone(), world);
    offer_row(&change, retracted.then_some("retracted"))
}
