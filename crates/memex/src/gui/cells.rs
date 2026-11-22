//! The screen as cells in memory: ratatui lays the memex out in cells
//! (crate::ui), and this backend keeps them for the window to paint
//! (crate::gui::paint), with the text cursor, if one is shown.

use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use std::convert::Infallible;
use unicode_width::UnicodeWidthStr;

/// The cells drawn, and where the text cursor is, if it is shown.
pub struct Cells {
    pub buffer: Buffer,
    pub cursor: Option<Position>,
}

impl Cells {
    /// Blank cells, so many columns and rows.
    pub fn new(columns: u16, rows: u16) -> Cells {
        let buffer = Buffer::empty(Rect::new(0, 0, columns, rows));
        Cells {
            buffer,
            cursor: None,
        }
    }
}

impl Backend for Cells {
    type Error = Infallible;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Infallible>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let area = self.buffer.area;
        for (x, y, cell) in content.filter(|(x, y, _)| area.contains(Position::new(*x, *y))) {
            self.buffer[(x, y)] = cell.clone();
            // ratatui sends no cell a wide character covers, so what was
            // there before would stay, painted over its right half.
            for i in 1..cell.symbol().width() as u16 {
                if let Some(covered) = self.buffer.cell_mut((x + i, y)) {
                    covered.reset();
                }
            }
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), Infallible> {
        self.cursor = None;
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Infallible> {
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Infallible> {
        Ok(self.cursor.unwrap_or_default())
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Infallible> {
        self.cursor = Some(position.into());
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Infallible> {
        self.buffer.reset();
        Ok(())
    }

    fn clear_region(&mut self, _: ClearType) -> Result<(), Infallible> {
        self.buffer.reset();
        Ok(())
    }

    fn size(&self) -> Result<Size, Infallible> {
        Ok(self.buffer.area.as_size())
    }

    fn window_size(&mut self) -> Result<WindowSize, Infallible> {
        let columns_rows = self.buffer.area.as_size();
        let pixels = Size::default();
        Ok(WindowSize {
            columns_rows,
            pixels,
        })
    }

    fn flush(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::widgets::Paragraph;

    #[test]
    fn what_is_drawn_is_kept_with_the_cursor() {
        // Catches cells drawn and not kept, cells outside the screen
        // written, or the cursor lost.
        let mut t = Terminal::new(Cells::new(10, 2)).unwrap();
        t.draw(|f| {
            f.render_widget(Paragraph::new("memex"), f.area());
            f.set_cursor_position((3, 1));
        })
        .unwrap();
        let cells = t.backend();
        let row: String = (0..10)
            .map(|x| cells.buffer[(x, 0)].symbol().to_string())
            .collect();
        assert_eq!(row, "memex     ");
        assert_eq!(cells.cursor, Some(Position::new(3, 1)));
        let mut c = Cells::new(2, 1);
        let far = Cell::new("x");
        c.draw([(5, 5, &far)].into_iter()).unwrap();
        assert_eq!(c.buffer[(0, 0)].symbol(), " ");
    }

    #[test]
    fn a_wide_character_leaves_no_old_character_under_it() {
        // Catches "ab", then "日", kept as ["日", "b"]: the cell 日 covers
        // was never sent, so the b stayed, painted over it and read by a
        // right-click (the code review, H2).
        let mut t = Terminal::new(Cells::new(4, 1)).unwrap();
        for text in ["ab", "日"] {
            t.draw(|f| f.render_widget(Paragraph::new(text), f.area()))
                .unwrap();
        }
        let row: Vec<&str> = (0..4)
            .map(|x| t.backend().buffer[(x, 0)].symbol())
            .collect();
        assert_eq!(row, ["日", " ", " ", " "]);
    }
}
