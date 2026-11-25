//! The inspector, scrolled: of lines taller than their area, the rows that
//! fit after those scrolled past, a row above saying how many are hidden
//! there and a row below saying how many are hidden there, and where each
//! row a click lands on is drawn. PageUp, PageDown and the wheel scroll
//! it (crate::present).

use super::wrap::hung;
use super::{Column, Presentations, Presented, dim};
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

/// What the screen drew of the inspector: its area, the rows a page
/// shows, and the most rows it can be scrolled.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Fit {
    pub area: Rect,
    pub page: u16,
    pub max: u16,
}

impl Fit {
    /// Whether a cell is in the inspector.
    pub fn holds(&self, (x, y): (u16, u16)) -> bool {
        self.area.contains(Position { x, y })
    }
}

/// The rows of lines that an area shows: so many rows in all, from which
/// row, how many, whether a note above and one below say what is hidden,
/// and the most it can be scrolled. Lines that fit show from their first
/// row, with no note.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Window {
    total: u16,
    from: u16,
    shown: u16,
    above: bool,
    below: bool,
    max: u16,
}

impl Window {
    /// The window on so many rows that an area of a height shows, scrolled
    /// by so many: never past the last row, which then shows under a note
    /// alone. An area of fewer than three rows has no room for notes.
    fn on(total: u16, height: u16, by: u16) -> Window {
        let max = match total > height && height >= 3 {
            true => total - (height - 1),
            false => 0,
        };
        let from = by.min(max);
        let above = from > 0;
        let room = height - u16::from(above);
        let below = from + room < total && max > 0;
        let shown = (room - u16::from(below)).min(total - from);
        Window {
            total,
            from,
            shown,
            above,
            below,
            max,
        }
    }

    /// The row on the screen the window's first row is drawn in.
    fn top(&self, area: Rect) -> u16 {
        area.y + u16::from(self.above)
    }

    /// Where a block of rows, laid out from an area's first row, lands on
    /// the screen, if any of it is shown.
    fn place(&self, area: Rect, r: Rect) -> Option<Rect> {
        let start = r.y - area.y;
        let first = start.max(self.from);
        let last = (start + r.height).min(self.from + self.shown);
        if first >= last {
            return None;
        }
        let y = self.top(area) + first - self.from;
        Some(Rect {
            y,
            height: last - first,
            ..r
        })
    }

    /// The note above the rows shown, if any are hidden there.
    fn note_above(&self) -> Option<Line<'static>> {
        let said = format!("↑ {} rows above · PageUp", self.from);
        self.above.then(|| Line::styled(said, dim()))
    }

    /// The note below the rows shown, if any are hidden there.
    fn note_below(&self) -> Option<Line<'static>> {
        let hidden = self.total - self.from - self.shown;
        let said = format!("↓ {hidden} rows below · PageDown");
        self.below.then(|| Line::styled(said, dim()))
    }

    /// What the screen drew of an area through the window.
    fn fit(&self, area: Rect) -> Fit {
        let (page, max) = (self.shown, self.max);
        Fit { area, page, max }
    }
}

/// Lines drawn hung in an area, scrolled by so many rows if they are taller
/// than it, each that presents an object recorded where it is drawn, and
/// what was drawn of the area.
pub(super) fn draw_scrolled(
    f: &mut Frame,
    lines: Vec<Presented>,
    area: Rect,
    by: u16,
) -> Presentations {
    let column = laid_out(lines, area);
    let rows = hung(&column.lines, area.width as usize);
    let window = Window::on(rows.len() as u16, area.height, by);
    f.render_widget(Paragraph::new(shown(rows, &window)), area);
    let moved = column.hits.0.into_iter();
    let placed = moved.filter_map(|(r, p)| Some((window.place(area, r)?, p)));
    Presentations(placed.collect(), Some(window.fit(area)))
}

/// Lines laid out in a column as tall as they are, with where each that
/// presents an object is, as though the area had room for them all.
fn laid_out(lines: Vec<Presented>, area: Rect) -> Column {
    let tall = Rect {
        height: u16::MAX - area.y,
        ..area
    };
    let mut column = Column::new(tall);
    for (line, presented) in lines {
        match presented {
            Some(p) => column.clickable(vec![line], p),
            None => column.lines.push(line),
        }
    }
    column
}

/// The rows a window shows, with its notes of what is hidden above and
/// below.
fn shown(rows: Vec<Line<'static>>, w: &Window) -> Vec<Line<'static>> {
    let window = rows.into_iter().skip(w.from.into()).take(w.shown.into());
    let with_above = w.note_above().into_iter().chain(window);
    with_above.chain(w.note_below()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window's place, what it shows, and its notes.
    fn seen(w: Window) -> (u16, u16, bool, bool) {
        (w.from, w.shown, w.above, w.below)
    }

    #[test]
    fn a_window_never_scrolls_past_the_last_row_and_notes_what_it_hides() {
        // Catches a scroll past the end, a note that hides a row it could
        // show, rows hidden with no note, or lines that fit given a scroll.
        assert_eq!(seen(Window::on(30, 10, 0)), (0, 9, false, true));
        assert_eq!(seen(Window::on(30, 10, 5)), (5, 8, true, true));
        let end = Window::on(30, 10, 100);
        assert_eq!(seen(end), (21, 9, true, false));
        assert_eq!((end.from + end.shown, end.max), (30, 21), "the last row");
        let fits = Window::on(10, 10, 4);
        assert_eq!((seen(fits), fits.max), ((0, 10, false, false), 0));
        let short = Window::on(5, 10, 0);
        assert_eq!((seen(short), short.max), ((0, 5, false, false), 0));
    }

    #[test]
    fn a_click_lands_where_its_rows_are_drawn_once_scrolled() {
        // Catches a click left at the row it would have had unscrolled, or
        // kept on a row scrolled out of sight. The area starts at row 2;
        // rows 5 to 12 of the lines show, under the note on row 2.
        let area = Rect::new(0, 2, 20, 10);
        let w = Window::on(30, 10, 5);
        let place = |y, height| w.place(area, Rect::new(0, y, 20, height));
        assert_eq!(place(7, 1), Some(Rect::new(0, 3, 20, 1)));
        assert_eq!(place(6, 2), Some(Rect::new(0, 3, 20, 1)));
        assert_eq!(place(4, 2), None, "scrolled past");
        assert_eq!(place(15, 1), None, "below the window");
    }
}
