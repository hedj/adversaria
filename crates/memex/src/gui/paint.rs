//! The cells painted in the window by egui: each cell at its place on a
//! grid of one monospace character's width and one line's height, so wide
//! characters and lines drawn in box characters keep the grid. Colours
//! and the modifiers the screen uses (bold, dim, italic, reversed,
//! underlined) are drawn as egui can; bold in Hack Bold, bundled
//! (crates/memex/fonts), since egui's fonts have Hack Regular alone and
//! text drawn twice a pixel apart, as bold once was, smears.

use super::boxes::{self, Brush, Grid};
use super::cells::Cells;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Painter, Rect, Shape, Stroke, Vec2};
use ratatui::buffer::Cell;
use ratatui::layout::Position;
use ratatui::style::{Color, Modifier};

/// The text and background the memex draws on, with no other colour.
pub const INK: Color32 = Color32::from_rgb(0xdc, 0xdc, 0xdc);
pub const PAPER: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1c);

/// The bold face of the cells' font: Hack Bold, of the release whose
/// Regular egui bundles, so every glyph has the same width.
static HACK_BOLD: &[u8] = include_bytes!("../../fonts/Hack-Bold.ttf");

/// egui's fonts, and Hack Bold as the family "bold", falling back to the
/// monospace family's fonts for what it lacks.
pub fn fonts() -> egui::FontDefinitions {
    use egui::{FontData, FontFamily};
    let mut defs = egui::FontDefinitions::default();
    let bold = std::sync::Arc::new(FontData::from_static(HACK_BOLD));
    defs.font_data.insert("Hack-Bold".into(), bold);
    let mono = defs
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    let family = std::iter::once("Hack-Bold".to_string())
        .chain(mono)
        .collect();
    defs.families
        .insert(FontFamily::Name("bold".into()), family);
    defs
}

/// The font bold cells are drawn in.
pub fn bold_font() -> FontId {
    FontId::new(font().size, egui::FontFamily::Name("bold".into()))
}

/// The font cells are drawn in.
pub fn font() -> FontId {
    FontId::monospace(15.0)
}

/// The size of a cell in points: one character's width, one line's height.
pub fn cell_size(ctx: &egui::Context) -> Vec2 {
    let font = font();
    ctx.fonts_mut(|f| Vec2::new(f.glyph_width(&font, 'M'), f.row_height(&font)))
}

/// The colour of the bar that marks the cursor's line, in the margin.
const ACCENT: Color32 = Color32::from_rgb(0x6c, 0x9b, 0xe0);

/// Paint every cell, a margin from the window's edge, then the bar in
/// the margin by the cursor's line, then the text cursor, if it is shown.
pub fn paint(p: &Painter, cells: &Cells, size: Vec2, margin: Vec2) {
    p.rect_filled(p.clip_rect(), 0.0, PAPER);
    let grid = Grid::new(size, 1.0 / p.pixels_per_point(), margin);
    paint_cells(p, cells, grid);
    for y in cursor_line_rows(&cells.buffer) {
        p.rect_filled(accent_bar(grid, y), 0.0, ACCENT);
    }
    if let Some(at) = cells.cursor {
        paint_cursor(p, grid.rect(at));
    }
}

/// The rows of the cursor's line: those whose first cell has the
/// cursor's ground (crate::ui::cursor), which nothing else is laid on.
fn cursor_line_rows(buffer: &ratatui::buffer::Buffer) -> Vec<u16> {
    let ground = crate::ui::cursor().bg;
    let rows = buffer.area.rows().map(|r| r.y);
    rows.filter(|y| Some(buffer[(0, *y)].bg) == ground)
        .collect()
}

/// A bar in the margin, against a row's first cell, so the bar and the
/// cursor's ground read as one shape: about three points wide, on whole
/// pixels.
fn accent_bar(grid: Grid, y: u16) -> Rect {
    let row = grid.rect(Position::new(0, y));
    let width = grid.points(3.0);
    Rect::from_x_y_ranges(row.min.x - width..=row.min.x, row.y_range())
}

/// Each cell in its place on the grid.
fn paint_cells(p: &Painter, cells: &Cells, grid: Grid) {
    for at in cells.buffer.area.positions() {
        paint_cell(p, &cells.buffer[at], brush(grid, at));
    }
}

/// What draws in a cell: its place on the grid, in ink.
fn brush(grid: Grid, at: Position) -> Brush {
    let cell = grid.rect(at);
    Brush {
        cell,
        ink: INK,
        grid,
    }
}

/// The text cursor: a bar at the left of its cell.
fn paint_cursor(p: &Painter, cell: Rect) {
    let bar = Rect::from_min_size(cell.min, Vec2::new(2.0, cell.height()));
    p.rect_filled(bar, 0.0, INK);
}

/// One cell: its background if it has one, then its text.
fn paint_cell(p: &Painter, cell: &Cell, mut brush: Brush) {
    let (fg, bg) = colours(cell);
    if bg != PAPER {
        p.rect_filled(brush.cell, 0.0, bg);
    }
    brush.ink = fg;
    paint_text(p, cell, &brush);
}

/// A cell's text: a box-drawing or block character as shapes that fill
/// the cell (crate::gui::boxes), anything else in the font. A blank
/// under a line (underlined, struck through) is laid out like a letter,
/// so the line runs through it unbroken.
fn paint_text(p: &Painter, cell: &Cell, brush: &Brush) {
    if let Some(shapes) = drawn(cell.symbol(), brush).filter(|_| !lined(cell)) {
        return p.extend(shapes);
    }
    let job = LayoutJob::single_section(cell.symbol().to_string(), format(cell, brush.ink));
    p.galley(brush.cell.min, p.layout_job(job), brush.ink);
}

/// Whether a cell is a blank that a line runs through: underlined or
/// struck through.
fn lined(cell: &Cell) -> bool {
    let lines = Modifier::UNDERLINED | Modifier::CROSSED_OUT;
    cell.symbol().trim().is_empty() && cell.modifier.intersects(lines)
}

/// A cell's symbol as shapes: no shapes for a blank, the shapes of a
/// box-drawing or block character; none if the font draws it.
fn drawn(symbol: &str, brush: &Brush) -> Option<Vec<Shape>> {
    let mut chars = symbol.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_whitespace() => Some(Vec::new()),
        (Some(c), None) => boxes::shapes(c, brush),
        (None, _) => Some(Vec::new()),
        _ => None,
    }
}

/// How a cell's text is drawn: its colour, italics, underline.
fn format(cell: &Cell, fg: Color32) -> TextFormat {
    let m = cell.modifier;
    let line = |k| match m.contains(k) {
        true => Stroke::new(1.0, fg),
        false => Stroke::NONE,
    };
    let font_id = if m.contains(Modifier::BOLD) {
        bold_font()
    } else {
        font()
    };
    TextFormat {
        font_id,
        color: fg,
        italics: m.contains(Modifier::ITALIC),
        underline: line(Modifier::UNDERLINED),
        strikethrough: line(Modifier::CROSSED_OUT),
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
    fn bold_is_drawn_in_hack_bold_as_wide_as_the_regular() {
        // Catches bold drawn in the regular face (or twice, smeared), or a
        // bold face whose width breaks the cell grid.
        assert_eq!(
            format(&cell(Modifier::BOLD, Color::Reset, Color::Reset), INK).font_id,
            bold_font()
        );
        assert_eq!(
            format(&cell(Modifier::empty(), Color::Reset, Color::Reset), INK).font_id,
            font()
        );
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts());
        let mut out = ctx.run_ui(Default::default(), |_| {});
        out.textures_delta.clear();
        let width = |f: &FontId| ctx.fonts_mut(|x| x.glyph_width(f, 'M'));
        assert_eq!(width(&bold_font()), width(&font()));
        assert!(
            ctx.fonts_mut(|x| x.has_glyph(&bold_font(), '‼')),
            "falls back to the regular"
        );
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
    fn a_blank_under_a_line_is_laid_out_so_the_line_runs_through() {
        // Catches an underline or a strike broken at every space: a blank
        // cell was painted as no shapes, its line with it.
        let mut cell = Cell::new(" ");
        assert!(!lined(&cell), "a plain blank is skipped");
        cell.modifier = Modifier::UNDERLINED;
        assert!(lined(&cell));
        cell.modifier = Modifier::CROSSED_OUT;
        assert!(lined(&cell));
        cell.set_symbol("a");
        assert!(!lined(&cell), "a letter is laid out anyway");
    }

    #[test]
    fn the_bar_marks_the_cursors_rows_in_the_margin() {
        // Catches the bar on a row without the cursor's ground, drawn over
        // the first cell rather than in the margin, or apart from the
        // cursor's ground, so the two read as two shapes.
        let mut buffer = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 10, 5));
        for y in [2, 3] {
            buffer.set_style(ratatui::layout::Rect::new(0, y, 10, 1), crate::ui::cursor());
        }
        assert_eq!(cursor_line_rows(&buffer), [2, 3]);
        let grid = Grid::new(Vec2::new(9.0, 18.0), 0.8, Vec2::splat(9.0));
        let (bar, cell) = (accent_bar(grid, 2), grid.rect(Position::new(0, 2)));
        assert!(bar.min.x > 0.0, "{bar:?} in the margin");
        assert_eq!(bar.max.x, cell.min.x, "against the first cell, no gap");
        assert_eq!(bar.y_range(), cell.y_range());
    }

    #[test]
    fn a_line_character_is_drawn_as_shapes_and_a_letter_by_the_font() {
        // Catches the divider left to the font's glyph, which stops short.
        let grid = Grid::new(Vec2::new(9.0, 18.0), 1.0, Vec2::ZERO);
        let brush = brush(grid, Position::new(0, 0));
        assert!(drawn("│", &brush).is_some_and(|s| !s.is_empty()));
        assert!(drawn("a", &brush).is_none());
        assert!(drawn(" ", &brush).is_some_and(|s| s.is_empty()));
    }
}

#[cfg(test)]
mod glyph_tests {
    use super::boxes::{Brush, Grid, shapes};
    use super::*;

    /// The characters beyond ASCII that the screen's code writes: those on
    /// its lines that are not comments.
    fn written() -> Vec<char> {
        let code = [
            include_str!("../ui.rs"),
            include_str!("../ui/wrap.rs"),
            include_str!("../view.rs"),
        ];
        let before_tests =
            |c: &&'static str| -> &'static str { c.split("#[cfg(test)]").next().unwrap_or("") };
        let lines = code.iter().map(before_tests).flat_map(|c| c.lines());
        let mut found: Vec<char> = lines
            .filter(|l| !l.trim_start().starts_with("//"))
            .flat_map(|l| l.chars().filter(|c| !c.is_ascii()).collect::<Vec<_>>())
            .collect();
        found.sort();
        found.dedup();
        found
    }

    /// A context with its fonts loaded, as after a frame.
    fn loaded() -> egui::Context {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |_| {});
        out.textures_delta.clear();
        ctx
    }

    /// A brush for one cell of the usual size.
    fn brush() -> Brush {
        let size = Vec2::new(9.0, 18.0);
        Brush {
            cell: Rect::from_min_size(egui::Pos2::ZERO, size),
            ink: Color32::WHITE,
            grid: Grid::new(size, 1.0, Vec2::ZERO),
        }
    }

    /// Whether a character is drawn: as shapes, or by a glyph of the font.
    fn drawn(ctx: &egui::Context, c: char) -> bool {
        shapes(c, &brush()).is_some() || ctx.fonts_mut(|f| f.has_glyph(&font(), c))
    }

    #[test]
    fn every_symbol_the_screen_writes_is_in_the_font() {
        // Catches a symbol drawn as a font's empty box, as the conflict
        // flag U+2691 was: neither drawn as shapes nor in egui's fonts.
        let ctx = loaded();
        let missing: Vec<String> = written()
            .into_iter()
            .filter(|c| !drawn(&ctx, *c))
            .map(|c| format!("{c} U+{:04X}", c as u32))
            .collect();
        assert!(missing.is_empty(), "not in the font: {missing:?}");
    }
}
