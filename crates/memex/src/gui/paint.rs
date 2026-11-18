//! The cells painted in the window by egui: each cell at its place on a
//! grid of one monospace character's width and one line's height, so wide
//! characters and lines drawn in box characters keep the grid. Colours
//! and the modifiers the screen uses (bold, dim, italic, reversed,
//! underlined) are drawn as egui can: bold as the text drawn twice, a
//! pixel apart, since egui's default fonts have no bold face.

use super::cells::Cells;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use ratatui::buffer::Cell;
use ratatui::layout::Position;
use ratatui::style::{Color, Modifier};

/// The text and background the memex draws on, with no other colour.
pub const INK: Color32 = Color32::from_rgb(0xdc, 0xdc, 0xdc);
pub const PAPER: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1c);

/// The font cells are drawn in.
pub fn font() -> FontId {
    FontId::monospace(15.0)
}

/// The size of a cell in points: one character's width, one line's height.
pub fn cell_size(ctx: &egui::Context) -> Vec2 {
    let font = font();
    ctx.fonts_mut(|f| Vec2::new(f.glyph_width(&font, 'M'), f.row_height(&font)))
}

/// Paint every cell, then the text cursor, if it is shown.
pub fn paint(p: &Painter, cells: &Cells, size: Vec2) {
    p.rect_filled(p.clip_rect(), 0.0, PAPER);
    paint_cells(p, cells, size);
    if let Some(at) = cells.cursor {
        paint_cursor(p, cell_rect(size, at));
    }
}

/// Each cell in its place on the grid.
fn paint_cells(p: &Painter, cells: &Cells, size: Vec2) {
    for at in cells.buffer.area.positions() {
        paint_cell(p, &cells.buffer[at], cell_rect(size, at));
    }
}

/// The text cursor: a bar at the left of its cell.
fn paint_cursor(p: &Painter, cell: Rect) {
    let bar = Rect::from_min_size(cell.min, Vec2::new(2.0, cell.height()));
    p.rect_filled(bar, 0.0, INK);
}

/// Where a cell is, by its column and row.
fn cell_rect(size: Vec2, cell: Position) -> Rect {
    let at = Pos2::new(f32::from(cell.x) * size.x, f32::from(cell.y) * size.y);
    Rect::from_min_size(at, size)
}

/// One cell: its background if it has one, then its text.
fn paint_cell(p: &Painter, cell: &Cell, rect: Rect) {
    let (fg, bg) = colours(cell);
    if bg != PAPER {
        p.rect_filled(rect, 0.0, bg);
    }
    if !cell.symbol().trim().is_empty() {
        paint_text(p, cell, rect.min, fg);
    }
}

/// A cell's text in its colour; bold as the text drawn twice, a pixel
/// apart.
fn paint_text(p: &Painter, cell: &Cell, at: Pos2, fg: Color32) {
    let job = LayoutJob::single_section(cell.symbol().to_string(), format(cell, fg));
    let galley = p.layout_job(job);
    if cell.modifier.contains(Modifier::BOLD) {
        p.galley(at + Vec2::new(1.0, 0.0), galley.clone(), fg);
    }
    p.galley(at, galley, fg);
}

/// How a cell's text is drawn: its colour, italics, underline.
fn format(cell: &Cell, fg: Color32) -> TextFormat {
    let m = cell.modifier;
    let underline = match m.contains(Modifier::UNDERLINED) {
        true => Stroke::new(1.0, fg),
        false => Stroke::NONE,
    };
    TextFormat {
        font_id: font(),
        color: fg,
        italics: m.contains(Modifier::ITALIC),
        underline,
        ..Default::default()
    }
}

/// A cell's text and background colours, after reversing and dimming.
fn colours(cell: &Cell) -> (Color32, Color32) {
    let (fg, bg) = (colour(cell.fg, INK), colour(cell.bg, PAPER));
    let (fg, bg) = match cell.modifier.contains(Modifier::REVERSED) {
        true => (bg, fg),
        false => (fg, bg),
    };
    let dim = cell.modifier.contains(Modifier::DIM);
    (if dim { fg.gamma_multiply(0.55) } else { fg }, bg)
}

/// A terminal colour as egui's; Reset is the default given.
fn colour(c: Color, default: Color32) -> Color32 {
    match c {
        Color::Reset => default,
        Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
        Color::Indexed(i) => named(usize::from(i % 16)),
        other => named(ansi(other)),
    }
}

/// The ANSI number of a named colour.
fn ansi(c: Color) -> usize {
    const ORDER: [Color; 16] = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
        Color::DarkGray,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
        Color::White,
    ];
    ORDER.iter().position(|o| *o == c).unwrap_or(7)
}

/// The sixteen ANSI colours, as a terminal commonly shows them.
fn named(i: usize) -> Color32 {
    const RGB: [(u8, u8, u8); 16] = [
        (0x1c, 0x1c, 0x1c),
        (0xcd, 0x31, 0x31),
        (0x0d, 0xbc, 0x79),
        (0xe5, 0xe5, 0x10),
        (0x24, 0x72, 0xc8),
        (0xbc, 0x3f, 0xbc),
        (0x11, 0xa8, 0xcd),
        (0xdc, 0xdc, 0xdc),
        (0x66, 0x66, 0x66),
        (0xf1, 0x4c, 0x4c),
        (0x23, 0xd1, 0x8b),
        (0xf5, 0xf5, 0x43),
        (0x3b, 0x8e, 0xea),
        (0xd6, 0x70, 0xd6),
        (0x29, 0xb8, 0xdb),
        (0xff, 0xff, 0xff),
    ];
    let (r, g, b) = RGB[i];
    Color32::from_rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(style: Modifier, fg: Color, bg: Color) -> Cell {
        let mut c = Cell::new("x");
        c.modifier = style;
        c.fg = fg;
        c.bg = bg;
        c
    }

    #[test]
    fn reversed_swaps_the_colours_and_dim_fades_the_text() {
        // Catches reversed ignored (the selected item would not show), or
        // dim drawn as plain text.
        let plain = colours(&cell(Modifier::empty(), Color::Reset, Color::Reset));
        assert_eq!(plain, (INK, PAPER));
        let rev = colours(&cell(Modifier::REVERSED, Color::Reset, Color::Reset));
        assert_eq!(rev, (PAPER, INK));
        let (dim, _) = colours(&cell(Modifier::DIM, Color::Reset, Color::Reset));
        assert_ne!(dim, INK);
    }

    #[test]
    fn named_and_indexed_colours_are_the_ansi_ones() {
        // Catches a named colour or an indexed one drawn as another.
        assert_eq!(colour(Color::Red, INK), Color32::from_rgb(0xcd, 0x31, 0x31));
        assert_eq!(
            colour(Color::Indexed(9), INK),
            Color32::from_rgb(0xf1, 0x4c, 0x4c)
        );
        assert_eq!(colour(Color::Rgb(1, 2, 3), INK), Color32::from_rgb(1, 2, 3));
    }

    #[test]
    fn a_cell_is_placed_by_its_column_and_row() {
        // Catches the grid's columns or rows swapped or not scaled.
        let at = cell_rect(Vec2::new(9.0, 18.0), Position::new(3, 2));
        assert_eq!(at.min, Pos2::new(27.0, 36.0));
        assert_eq!(at.size(), Vec2::new(9.0, 18.0));
    }
}
