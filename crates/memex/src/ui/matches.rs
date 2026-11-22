//! The list beside the outline: every item that a find lists (the last
//! Leap's matches, a label's items, those a kind of finding marks), in the
//! outline's order, numbered, each a click away, with where the cursor is
//! among them, so a list can be worked through.

use super::{Presentations, Presented, dim, draw_presented, inset, plain};
use crate::app::App;
use crate::found::Found;
use crate::present::Presentation;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;

/// The list, drawn in its area, each item a click away.
pub(super) fn draw(f: &mut Frame, app: &App, area: Rect) -> Presentations {
    draw_presented(f, match_list(app), inset(area))
}

/// The list's lines: its heading, then each item, the one the cursor is
/// on marked; or how to fill it.
fn match_list(app: &App) -> Vec<Presented> {
    let Some(found) = &app.session.found else {
        let empty = "(find, or Leap, and what it finds is listed here)";
        return plain(vec![Line::styled(empty, dim())]);
    };
    listed(app, found)
}

/// The heading of what a find lists, then each item.
fn listed(app: &App, found: &Found) -> Vec<Presented> {
    let world = app.reason.eval[0].as_ref();
    let items = found.items(&app.notebook.outline, world);
    let mut lines = plain(heading(app, found, &items));
    lines.extend(match_lines(app, &items));
    lines
}

/// Each match's line, numbered from 1.
fn match_lines(app: &App, found: &[u64]) -> Vec<Presented> {
    let numbered = found.iter().enumerate();
    numbered.map(|(k, id)| match_line(app, k, *id)).collect()
}

/// The heading: what is listed, how many, and which of them the cursor
/// is on, if it is on one; then a blank line.
fn heading(app: &App, found: &Found, items: &[u64]) -> Vec<Line<'static>> {
    let at = items.iter().position(|i| *i == app.cursor());
    let at = at.map_or(String::new(), |k| {
        format!(" · at {} of {}", k + 1, items.len())
    });
    let said = format!("{} · {}{at}", found.heading(), items.len());
    vec![Line::styled(said, dim()), Line::raw("")]
}

/// A match: its number in the list, its address and text, marked where
/// the cursor is; a click jumps to it, as follow does.
fn match_line(app: &App, k: usize, id: u64) -> Presented {
    let text = Line::raw(match_text(app, k, id));
    (text, Some(Presentation::Link { to: id }))
}

/// A match's words: where the cursor is, its number, address and text.
fn match_text(app: &App, k: usize, id: u64) -> String {
    let outline = &app.notebook.outline;
    let mark = if id == app.cursor() { "▸" } else { " " };
    format!(
        "{mark} {}. {}. {}",
        k + 1,
        outline.number(id),
        outline.text(id)
    )
}
