//! A styled line wrapped to a width, at spaces, with a hanging indent:
//! each row after the first starts under the line's text, not its head.
//! The rows remember where in the line each starts, so a column of the
//! line (the text cursor's, a click's place) can be found on its row.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthChar;

/// A line wrapped: its rows, the column of the line at which each row
/// starts, the indent of every row after the first, and the width.
pub struct Wrapped {
    pub rows: Vec<Line<'static>>,
    starts: Vec<usize>,
    hang: usize,
    width: usize,
}

/// A character of a line: the character, its style, its cells, and the
/// column of the line at which it starts.
#[derive(Clone)]
struct Cell {
    c: char,
    style: Style,
    cells: usize,
    col: usize,
}

/// Wrap a line to a width; rows after the first are set in by the hang.
/// A hang that would leave fewer than 8 columns is dropped.
pub fn wrap(line: &Line<'static>, width: usize, hang: usize) -> Wrapped {
    let width = width.max(1);
    let hang = if hang + 8 <= width { hang } else { 0 };
    let cut = rows(cells(line), (width, hang));
    let rows = drawn(&cut, hang, line);
    Wrapped {
        rows,
        starts: starts(&cut),
        hang,
        width,
    }
}

/// Lines wrapped to a width, so a wrapped line reads as one. A line of
/// content (code, values, claims) has its later rows set in four cells
/// past its own indent, clear of the lines set in two under it as its
/// parts. A heading or a note, drawn wholly dim, wraps flush to its own
/// indent, as prose does, so it does not look like a list.
pub fn hung(lines: &[Line<'static>], width: usize) -> Vec<Line<'static>> {
    lines.iter().flat_map(|l| hung_line(l, width)).collect()
}

/// A line's rows, its later rows set in past its indent (above).
fn hung_line(line: &Line<'static>, width: usize) -> Vec<Line<'static>> {
    let past = if is_note(line) { 0 } else { 4 };
    wrap(line, width, indent_of(line) + past).rows
}

/// Whether a line is a heading or a note: all its text drawn dim.
fn is_note(line: &Line<'static>) -> bool {
    let dim = |s: &Span| {
        line.style
            .patch(s.style)
            .add_modifier
            .contains(Modifier::DIM)
    };
    let words = line.spans.iter().filter(|s| !s.content.trim().is_empty());
    words.clone().count() > 0 && words.into_iter().all(dim)
}

/// The spaces a line starts with.
fn indent_of(line: &Line<'static>) -> usize {
    let text = line.spans.iter().map(|s| s.content.as_ref());
    text.collect::<String>()
        .chars()
        .take_while(|c| *c == ' ')
        .count()
}

/// Lines drawn hung (above) to the width of the area they are drawn in.
pub struct Hung(pub Vec<Line<'static>>);

impl Widget for Hung {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(hung(&self.0, area.width as usize)).render(area, buf);
    }
}

/// The column of the line at which each row starts.
fn starts(rows: &[Vec<Cell>]) -> Vec<usize> {
    rows.iter()
        .map(|r| r.first().map_or(0, |c| c.col))
        .collect()
}

/// The rows as lines, those after the first set in by the hang.
fn drawn(rows: &[Vec<Cell>], hang: usize, line: &Line<'static>) -> Vec<Line<'static>> {
    let indent = |k| if k == 0 { 0 } else { hang };
    rows.iter()
        .enumerate()
        .map(|(k, r)| row_line(r, indent(k), line))
        .collect()
}

/// The characters of a line, each with its style and its column.
fn cells(line: &Line<'static>) -> Vec<Cell> {
    let chars = line.spans.iter().flat_map(span_chars);
    chars
        .scan(0, |col, (c, style)| Some(cell(c, style, col)))
        .collect()
}

/// A span's characters, each with the span's style.
fn span_chars<'a>(span: &'a Span<'static>) -> impl Iterator<Item = (char, Style)> + 'a {
    span.content.chars().map(move |c| (c, span.style))
}

/// A character at a column, which then moves past its cells.
fn cell(c: char, style: Style, col: &mut usize) -> Cell {
    let cells = c.width().unwrap_or(0);
    let at = std::mem::replace(col, *col + cells);
    Cell {
        c,
        style,
        cells,
        col: at,
    }
}

/// The characters in rows, for a width and a hang: the first row as wide
/// as the width, the others less the hang; each row ends after its last
/// space that fits, if there is one past the hang, else where it is full.
fn rows(cells: Vec<Cell>, shape: (usize, usize)) -> Vec<Vec<Cell>> {
    let mut rows = vec![Vec::new()];
    for cell in cells {
        // A word carried to the new row can leave too little room for a
        // wide character after it, so break again; an empty row takes any.
        while !fits(&rows, &cell, shape) {
            break_row(&mut rows, shape.1);
        }
        rows.last_mut().expect("one row at least").push(cell);
    }
    rows
}

/// Whether a character fits on the last row: it is empty, or has room.
fn fits(rows: &[Vec<Cell>], cell: &Cell, (width, hang): (usize, usize)) -> bool {
    let room = if rows.len() == 1 { width } else { width - hang };
    let row = rows.last().map_or(&[][..], |r| r.as_slice());
    row.is_empty() || used(row) + cell.cells <= room
}

/// How many cells a row's characters take.
fn used(row: &[Cell]) -> usize {
    row.iter().map(|c| c.cells).sum()
}

/// Start a row, with what the last hands on.
fn break_row(rows: &mut Vec<Vec<Cell>>, hang: usize) {
    let carried = carry(rows.last_mut().expect("one row at least"), hang);
    rows.push(carried);
}

/// What a full row hands to the next: the characters after its last
/// space past the hang, or nothing, if it has none.
fn carry(row: &mut Vec<Cell>, hang: usize) -> Vec<Cell> {
    let space = row.iter().rposition(|c| c.c == ' ' && c.col >= hang);
    match space {
        Some(at) => row.split_off(at + 1),
        None => Vec::new(),
    }
}

/// A row as a line: its indent, then runs of characters of one style; the
/// line's own style (the cursor's, say) over the whole row.
fn row_line(row: &[Cell], indent: usize, line: &Line<'static>) -> Line<'static> {
    let indent = Span::raw(" ".repeat(indent));
    let spans = std::iter::once(indent).chain(runs(row)).collect();
    Line {
        spans,
        style: line.style,
        alignment: line.alignment,
    }
}

/// A row's runs of characters of one style, each a span.
fn runs(row: &[Cell]) -> impl Iterator<Item = Span<'static>> + '_ {
    row.chunk_by(|a, b| a.style == b.style).map(run)
}

/// A run of characters of one style as a span.
fn run(cells: &[Cell]) -> Span<'static> {
    let text: String = cells.iter().map(|c| c.c).collect();
    Span::styled(text, cells[0].style)
}

impl Wrapped {
    /// The row and the column on it where a column of the line is drawn.
    pub fn place(&self, col: usize) -> (usize, usize) {
        let k = self.starts.iter().rposition(|s| *s <= col).unwrap_or(0);
        let x = self.indent(k) + col - self.starts[k];
        (k, x.min(self.width.saturating_sub(1)))
    }

    /// Where a reach of the line's columns, [from, to), is drawn: on each
    /// row it reaches, its first column and width. A reach that runs to a
    /// row's end takes the rest of that row.
    pub fn spans(&self, reach: (usize, usize)) -> Vec<(usize, usize, usize)> {
        let rows = 0..self.starts.len();
        rows.filter_map(|k| self.span_on(k, reach)).collect()
    }

    /// Where a reach is drawn on row k, if it reaches it.
    fn span_on(&self, k: usize, reach: (usize, usize)) -> Option<(usize, usize, usize)> {
        let (a, b) = self.clip(k, reach)?;
        self.drawn_on(k, a, b)
    }

    /// The part of a reach on row k, if any: [a, b), b unbounded when the
    /// reach runs to the row's end.
    fn clip(&self, k: usize, reach: (usize, usize)) -> Option<(usize, usize)> {
        let row = (self.starts[k], self.end(k));
        let (a, b) = overlap(reach, row)?;
        Some((a, if b == row.1 { usize::MAX } else { b }))
    }

    /// Where columns [a, b) of the line, all on row k, are drawn: the row,
    /// its first column and its width, cut at the row's end.
    fn drawn_on(&self, k: usize, a: usize, b: usize) -> Option<(usize, usize, usize)> {
        let x = self.indent(k) + a - self.starts[k];
        let room = self.width.checked_sub(x).filter(|r| *r > 0)?;
        Some((k, x, (b - a).min(room)))
    }

    /// The column of the line at which row k ends: where the next starts,
    /// or none, for the last.
    fn end(&self, k: usize) -> usize {
        self.starts.get(k + 1).copied().unwrap_or(usize::MAX)
    }

    fn indent(&self, k: usize) -> usize {
        if k == 0 { 0 } else { self.hang }
    }
}

/// The columns two reaches share, [a, b), if any.
fn overlap((a1, b1): (usize, usize), (a2, b2): (usize, usize)) -> Option<(usize, usize)> {
    let (a, b) = (a1.max(a2), b1.min(b2));
    (a < b).then_some((a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(w: &Wrapped) -> Vec<String> {
        let row = |l: &Line| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        };
        w.rows.iter().map(row).collect()
    }

    #[test]
    fn a_line_that_fits_is_one_row() {
        let w = wrap(&Line::raw("• 1. short"), 20, 5);
        assert_eq!(text(&w), ["• 1. short"]);
    }

    #[test]
    fn a_long_line_breaks_at_spaces_under_its_text() {
        // Catches a row cut mid-word, or a second row set back to the
        // pane's edge rather than under the text.
        let w = wrap(
            &Line::raw("• 1. Buran flew once, unmanned, in 1988."),
            20,
            5,
        );
        assert_eq!(
            text(&w),
            [
                "• 1. Buran flew ",
                "     once, ",
                "     unmanned, in ",
                "     1988."
            ]
        );
    }

    #[test]
    fn a_word_longer_than_a_row_is_cut_where_the_row_ends() {
        let w = wrap(&Line::raw("ab abcdefghijklmnop"), 10, 0);
        assert_eq!(text(&w), ["ab ", "abcdefghij", "klmnop"]);
    }

    #[test]
    fn a_wide_character_after_a_carried_word_goes_to_a_row_of_its_own() {
        // Catches a row one cell too wide, the wide character clipped at
        // the pane's edge and not drawn (the code review, G-B1).
        let w = wrap(&Line::raw("aaaaaaaaaa bbbbbbbbb日 x"), 10, 0);
        assert!(w.rows.iter().all(|r| r.width() <= 10), "{:?}", text(&w));
        assert_eq!(text(&w).concat(), "aaaaaaaaaa bbbbbbbbb日 x");
    }

    #[test]
    fn the_head_is_never_a_place_to_break() {
        // Catches the first row ending inside the head ("• 1. "), which
        // leaves the head alone on its row.
        let w = wrap(&Line::raw("• 1. Supercalifragilistic"), 15, 5);
        assert_eq!(text(&w), ["• 1. Supercalif", "     ragilistic"]);
    }

    #[test]
    fn a_column_is_found_on_its_row() {
        // Catches the text cursor or a click put on the wrong row, or
        // without the hang.
        let w = wrap(
            &Line::raw("• 1. Buran flew once, unmanned, in 1988."),
            20,
            5,
        );
        assert_eq!(w.place(2), (0, 2));
        assert_eq!(w.place(16), (1, 5));
        assert_eq!(w.place(40), (3, 10));
    }

    #[test]
    fn a_span_is_found_on_each_row_it_reaches() {
        // Catches a click on a wrapped item's second row missed.
        let w = wrap(
            &Line::raw("• 1. Buran flew once, unmanned, in 1988."),
            20,
            5,
        );
        assert_eq!(
            w.spans((0, usize::MAX)),
            [(0, 0, 20), (1, 5, 15), (2, 5, 15), (3, 5, 15)]
        );
        assert_eq!(w.spans((5, 10)), [(0, 5, 5)]);
        // A reach that starts where a row starts takes nothing of the row
        // before: an empty overlap at its end is not "the rest of it".
        assert_eq!(w.spans((22, 30)), [(2, 5, 8)]);
    }

    #[test]
    fn styles_and_the_lines_own_style_are_kept() {
        let bold = Style::default().add_modifier(ratatui::style::Modifier::BOLD);
        let line = Line::from(vec![Span::raw("aaa "), Span::styled("bbb ccc", bold)]).style(bold);
        let w = wrap(&line, 8, 0);
        assert_eq!(text(&w), ["aaa bbb ", "ccc"]);
        assert_eq!(w.rows[1].style, bold);
        assert_eq!(w.rows[1].spans.last().unwrap().style, bold);
    }

    #[test]
    fn an_empty_line_is_one_empty_row() {
        let w = wrap(&Line::raw(""), 10, 2);
        assert_eq!(text(&w), [""]);
        assert_eq!(w.place(0), (0, 0));
    }
}
