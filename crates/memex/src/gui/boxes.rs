//! Box-drawing and block characters drawn as shapes that fill their cell,
//! as terminals draw them. A font's glyph for a line stops short of the
//! cell's height, so where a cell is a fractional number of pixels high
//! (a scale of 1.25, say) a seam shows between rows. Lines are a whole
//! number of pixels wide, placed on the cell's pixels, so a line meets its
//! neighbour in the next cell exactly. Characters drawn so: the light and
//! heavy lines, corners, tees and crosses; the rounded corners; the
//! diagonals; and the full, half and eighth blocks. Others (dashed lines,
//! double lines, shades) are left to the font.

use egui::{Color32, Pos2, Rect, Shape, Stroke, Vec2};
use ratatui::layout::Position;
use std::f32::consts::FRAC_PI_2;

/// The grid the cells lie on: a cell's size, a pixel, and where its
/// first cell's corner is, in points.
#[derive(Debug, Clone, Copy)]
pub struct Grid {
    size: Vec2,
    pixel: f32,
    origin: Vec2,
}

impl Grid {
    /// A grid of cells of a size, a pixel, and its first cell's corner.
    pub fn new(size: Vec2, pixel: f32, origin: Vec2) -> Grid {
        Grid {
            size,
            pixel,
            origin,
        }
    }

    /// Where a cell is, by its column and row. Its edges are on pixels, the
    /// same for neighbouring cells, so they meet with no seam between.
    pub fn rect(&self, at: Position) -> Rect {
        Rect::from_min_max(self.corner(at.x, at.y), self.corner(at.x + 1, at.y + 1))
    }

    /// The top left corner of the cell at a column and row, on a pixel.
    fn corner(&self, x: u16, y: u16) -> Pos2 {
        let points = Vec2::new(f32::from(x), f32::from(y)) * self.size + self.origin;
        ((points / self.pixel).round() * self.pixel).to_pos2()
    }

    /// About so many points, in whole pixels, at least one.
    pub fn points(&self, n: f32) -> f32 {
        (n / self.pixel).round().max(1.0) * self.pixel
    }

    /// A light line's width: about a point.
    fn line(&self) -> f32 {
        self.points(1.0)
    }
}

/// Where and how to draw: a cell, with its edges on pixels, the ink, and
/// the grid it lies on.
#[derive(Debug, Clone, Copy)]
pub struct Brush {
    pub cell: Rect,
    pub ink: Color32,
    pub grid: Grid,
}

/// The shapes that draw a character, if it is one drawn as shapes.
pub fn shapes(c: char, at: &Brush) -> Option<Vec<Shape>> {
    lines(c, at)
        .or_else(|| block(c, at))
        .or_else(|| rounded(c, at))
        .or_else(|| diagonals(c, at))
}

/// The sides of a cell a line can leave by.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Side {
    Left,
    Up,
    Right,
    Down,
}

const SIDES: [Side; 4] = [Side::Left, Side::Up, Side::Right, Side::Down];

impl Side {
    /// A rectangle stretched to this side of a cell.
    fn stretch(self, mut r: Rect, cell: Rect) -> Rect {
        match self {
            Side::Left => r.min.x = cell.min.x,
            Side::Up => r.min.y = cell.min.y,
            Side::Right => r.max.x = cell.max.x,
            Side::Down => r.max.y = cell.max.y,
        }
        r
    }

    /// Where a line through a point leaves a cell by this side.
    fn edge(self, through: Pos2, cell: Rect) -> Pos2 {
        let r = self.stretch(Rect::from_min_max(through, through), cell);
        if matches!(self, Side::Left | Side::Up) {
            r.min
        } else {
            r.max
        }
    }

    /// The way toward this side, a unit long.
    fn toward(self) -> Vec2 {
        match self {
            Side::Left => Vec2::new(-1.0, 0.0),
            Side::Up => Vec2::new(0.0, -1.0),
            Side::Right => Vec2::new(1.0, 0.0),
            Side::Down => Vec2::new(0.0, 1.0),
        }
    }
}

/// A line, corner, tee or cross: an arm to each side it reaches.
fn lines(c: char, at: &Brush) -> Option<Vec<Shape>> {
    let weights = arms(c)?;
    let reached = SIDES.iter().zip(weights).filter(|(_, w)| *w > 0);
    Some(reached.map(|(side, w)| at.arm(*side, w)).collect())
}

/// The weight of a line's arm to the left, up, right and down: 0 none, 1
/// light, 2 heavy. Only lines of one weight.
fn arms(c: char) -> Option<[u8; 4]> {
    let light = |[l, u, r, d]: [u8; 4]| Some([l, u, r, d]);
    let heavy = |a: [u8; 4]| Some(a.map(|w| w * 2));
    match c {
        '─' => light([1, 0, 1, 0]),
        '│' => light([0, 1, 0, 1]),
        '┌' => light([0, 0, 1, 1]),
        '┐' => light([1, 0, 0, 1]),
        '└' => light([0, 1, 1, 0]),
        '┘' => light([1, 1, 0, 0]),
        '├' => light([0, 1, 1, 1]),
        '┤' => light([1, 1, 0, 1]),
        '┬' => light([1, 0, 1, 1]),
        '┴' => light([1, 1, 1, 0]),
        '┼' => light([1, 1, 1, 1]),
        '━' => heavy([1, 0, 1, 0]),
        '┃' => heavy([0, 1, 0, 1]),
        '┏' => heavy([0, 0, 1, 1]),
        '┓' => heavy([1, 0, 0, 1]),
        '┗' => heavy([0, 1, 1, 0]),
        '┛' => heavy([1, 1, 0, 0]),
        '┣' => heavy([0, 1, 1, 1]),
        '┫' => heavy([1, 1, 0, 1]),
        '┳' => heavy([1, 0, 1, 1]),
        '┻' => heavy([1, 1, 1, 0]),
        '╋' => heavy([1, 1, 1, 1]),
        _ => None,
    }
}

/// A block: the part of the cell it fills.
fn block(c: char, at: &Brush) -> Option<Vec<Shape>> {
    let (min, max) = filled(c)?;
    let part = Rect::from_min_max(at.point(min), at.point(max));
    Some(vec![Shape::rect_filled(part, 0.0, at.ink)])
}

/// The corners of the part of a cell a block fills, as fractions of the
/// cell from its top left corner.
fn filled(c: char) -> Option<(Vec2, Vec2)> {
    let eighths = |n: u32| n as f32 / 8.0;
    let code = c as u32;
    Some(match code {
        0x2580 => (Vec2::ZERO, Vec2::new(1.0, 0.5)),
        0x2581..=0x2588 => (
            Vec2::new(0.0, 1.0 - eighths(code - 0x2580)),
            Vec2::splat(1.0),
        ),
        0x2589..=0x258F => (Vec2::ZERO, Vec2::new(eighths(0x2590 - code), 1.0)),
        0x2590 => (Vec2::new(0.5, 0.0), Vec2::splat(1.0)),
        _ => return None,
    })
}

/// A rounded corner, by the two sides it joins: straight from the first
/// side, a quarter circle, straight on to the second.
fn rounded(c: char, at: &Brush) -> Option<Vec<Shape>> {
    let (from, to) = match c {
        '╭' => (Side::Down, Side::Right),
        '╮' => (Side::Down, Side::Left),
        '╯' => (Side::Up, Side::Left),
        '╰' => (Side::Up, Side::Right),
        _ => return None,
    };
    Some(vec![at.straight(from), at.arc(from, to), at.straight(to)])
}

/// A diagonal, rising (from the bottom left) or falling, or both.
fn diagonals(c: char, at: &Brush) -> Option<Vec<Shape>> {
    let rising: &[bool] = match c {
        '╱' => &[true],
        '╲' => &[false],
        '╳' => &[true, false],
        _ => return None,
    };
    Some(rising.iter().map(|r| at.diagonal(*r)).collect())
}

impl Brush {
    /// A point in the cell, by fractions of its width and height.
    fn point(&self, at: Vec2) -> Pos2 {
        self.cell.min + self.cell.size() * at
    }

    fn stroke(&self) -> Stroke {
        Stroke::new(self.grid.line(), self.ink)
    }

    /// Where lines cross: a square of whole pixels, a line's width on each
    /// side, at the middle of the cell.
    fn knot(&self, width: f32) -> Rect {
        let pixel = self.grid.pixel;
        let half = ((self.cell.size() - Vec2::splat(width)) / 2.0 / pixel).floor();
        Rect::from_min_size(self.cell.min + half * pixel, Vec2::splat(width))
    }

    /// The middle of a light line's knot.
    fn middle(&self) -> Pos2 {
        self.knot(self.grid.line()).center()
    }

    /// The arm of a line to one side: the knot stretched to that side, so
    /// arms meet in a corner without a notch.
    fn arm(&self, side: Side, weight: u8) -> Shape {
        let knot = self.knot(self.grid.line() * f32::from(weight));
        Shape::rect_filled(side.stretch(knot, self.cell), 0.0, self.ink)
    }

    /// A rounded corner's radius: half the cell's width or height.
    fn radius(&self) -> f32 {
        self.cell.width().min(self.cell.height()) / 2.0
    }

    /// A rounded corner's straight part: from a side to the arc.
    fn straight(&self, side: Side) -> Shape {
        let start = self.middle() + side.toward() * self.radius();
        Shape::line_segment([start, side.edge(self.middle(), self.cell)], self.stroke())
    }

    /// A rounded corner's quarter circle, from its way toward one side to
    /// its way toward the other.
    fn arc(&self, from: Side, to: Side) -> Shape {
        let (a, b) = (from.toward() * self.radius(), to.toward() * self.radius());
        let centre = self.middle() + a + b;
        let points = (0..=8).map(|step| centre - turned(b, a, step));
        Shape::line(points.collect(), self.stroke())
    }

    /// A line from corner to corner, rising from the bottom left or
    /// falling from the top left.
    fn diagonal(&self, rising: bool) -> Shape {
        let (start, end) = match rising {
            true => (Vec2::new(0.0, 1.0), Vec2::new(1.0, 0.0)),
            false => (Vec2::ZERO, Vec2::splat(1.0)),
        };
        Shape::line_segment([self.point(start), self.point(end)], self.stroke())
    }
}

/// A vector turned a number of eighths of a right angle from one way
/// toward another at right angles to it, keeping its length.
fn turned(from: Vec2, toward: Vec2, eighths: u8) -> Vec2 {
    let angle = f32::from(eighths) / 8.0 * FRAC_PI_2;
    from * angle.cos() + toward * angle.sin()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cell 9.6 by 21.6 points, at a scale of 1.25: a pixel is 0.8.
    fn brush() -> Brush {
        let cell = Rect::from_min_size(Pos2::new(8.0, 16.0), Vec2::new(9.6, 21.6));
        let grid = Grid {
            size: cell.size(),
            pixel: 0.8,
            origin: Vec2::ZERO,
        };
        Brush {
            cell,
            ink: Color32::WHITE,
            grid,
        }
    }

    /// The smallest rectangle round the shapes.
    fn bounds(shapes: &[Shape]) -> Rect {
        shapes
            .iter()
            .map(Shape::visual_bounding_rect)
            .fold(Rect::NOTHING, |a, b| a.union(b))
    }

    #[test]
    fn a_vertical_line_runs_the_cells_full_height() {
        // Catches the gap between rows: the line must reach the cell's top
        // and bottom edges.
        let b = brush();
        let drawn = bounds(&shapes('│', &b).unwrap());
        assert_eq!(
            (drawn.top(), drawn.bottom()),
            (b.cell.top(), b.cell.bottom())
        );
        let across = bounds(&shapes('─', &b).unwrap());
        assert_eq!(
            (across.left(), across.right()),
            (b.cell.left(), b.cell.right())
        );
    }

    #[test]
    fn lines_are_whole_pixels_and_heavy_ones_wider() {
        // Catches a line blurred across pixels, or heavy drawn as light.
        let b = brush();
        let width = |c| bounds(&shapes(c, &b).unwrap()).width();
        let pixels = width('│') / b.grid.pixel;
        assert!((pixels - pixels.round()).abs() < 1e-4, "{pixels}");
        assert!(width('┃') > width('│'));
    }

    #[test]
    fn a_corner_reaches_the_two_sides_it_joins() {
        // Catches an arm to the wrong side: ┌ goes right and down only.
        let b = brush();
        let drawn = bounds(&shapes('┌', &b).unwrap());
        assert_eq!(
            (drawn.right(), drawn.bottom()),
            (b.cell.right(), b.cell.bottom())
        );
        assert!(drawn.left() > b.cell.left() && drawn.top() > b.cell.top());
        let round = bounds(&shapes('╭', &b).unwrap());
        assert!((round.bottom() - b.cell.bottom()).abs() < 1.0 && round.top() > b.cell.top());
    }

    #[test]
    fn blocks_fill_their_part_of_the_cell() {
        // Catches eighths counted from the wrong end.
        let b = brush();
        assert_eq!(bounds(&shapes('█', &b).unwrap()), b.cell);
        let low = bounds(&shapes('▂', &b).unwrap());
        assert_eq!(low.bottom(), b.cell.bottom());
        assert!((low.height() - b.cell.height() / 4.0).abs() < 1e-4);
        let left = bounds(&shapes('▎', &b).unwrap());
        assert!((left.width() - b.cell.width() / 4.0).abs() < 1e-4);
    }

    #[test]
    fn other_characters_are_left_to_the_font() {
        // Catches a letter or a shade drawn as a shape.
        assert!(shapes('a', &brush()).is_none());
        assert!(shapes('░', &brush()).is_none());
        assert!(shapes('═', &brush()).is_none());
    }

    #[test]
    fn a_cell_is_placed_by_its_column_and_row() {
        // Catches the grid's columns or rows swapped or not scaled.
        let grid = Grid {
            size: Vec2::new(9.0, 18.0),
            pixel: 1.0,
            origin: Vec2::ZERO,
        };
        let at = grid.rect(Position::new(3, 2));
        assert_eq!(at.min, Pos2::new(27.0, 36.0));
        assert_eq!(at.size(), Vec2::new(9.0, 18.0));
    }

    #[test]
    fn neighbouring_cells_meet_on_a_pixel_at_a_fractional_scale() {
        // Catches the seam between rows: at a scale of 1.25 a cell 17.5
        // points high is 21.875 pixels, and its edges must be whole
        // pixels that the next row starts on.
        let grid = Grid {
            size: Vec2::new(9.6, 17.5),
            pixel: 0.8,
            origin: Vec2::ZERO,
        };
        for y in 0..40 {
            let (this, next) = (
                grid.rect(Position::new(0, y)),
                grid.rect(Position::new(0, y + 1)),
            );
            assert_eq!(this.bottom(), next.top());
            let pixels = this.bottom() / grid.pixel;
            assert!((pixels - pixels.round()).abs() < 1e-3, "row {y}: {pixels}");
        }
    }
}
