//! The matches pane: every item that holds the last Leap's pattern, in
//! the outline's order, numbered, each a click away, with where the
//! cursor is among them, so a list of matches can be worked through.

use super::{Presentations, Presented, dim, draw_presented, inset, plain};
use crate::app::App;
use crate::outline::TOP;
use crate::present::Presentation;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;

/// The matches pane, drawn in its area, each match a click away.
pub(super) fn draw(f: &mut Frame, app: &App, area: Rect) -> Presentations {
    draw_presented(f, match_list(app), inset(area))
}

/// The matches pane's lines: its heading, then each match, the one the
/// cursor is on marked; or how to fill it.
fn match_list(app: &App) -> Vec<Presented> {
    let Some((pattern, _)) = &app.session.last_leap else {
        let empty = "(Leap, and its matches are listed here)";
        return plain(vec![Line::styled(empty, dim())]);
    };
    matches_of(app, pattern)
}

/// The heading of a pattern's matches, then each of them.
fn matches_of(app: &App, pattern: &str) -> Vec<Presented> {
    let found = holding(app, pattern);
    let mut lines = plain(heading(app, pattern, &found));
    lines.extend(match_lines(app, &found));
    lines
}

/// Each match's line, numbered from 1.
fn match_lines(app: &App, found: &[u64]) -> Vec<Presented> {
    let numbered = found.iter().enumerate();
    numbered.map(|(k, id)| match_line(app, k, *id)).collect()
}

/// The items that hold a pattern, from the top of the outline.
fn holding(app: &App, pattern: &str) -> Vec<u64> {
    let outline = &app.notebook.outline;
    outline.holding(outline.round_from(TOP, true), pattern)
}

/// The heading: the pattern, how many hold it, and which of them the
/// cursor is on, if it is on one; then a blank line.
fn heading(app: &App, pattern: &str, found: &[u64]) -> Vec<Line<'static>> {
    let at = found.iter().position(|i| *i == app.cursor());
    let at = at.map_or(String::new(), |k| {
        format!(" · at {} of {}", k + 1, found.len())
    });
    let said = format!("MATCHES OF “{pattern}” · {}{at}", found.len());
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
