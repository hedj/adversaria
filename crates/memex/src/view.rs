//! Views: graphics declared over Prolog goals (docs/design.md § Views).
//!
//! An item becomes a view when it has layers. A layer is a mark and a goal,
//! `layer MARK where GOAL`: each solution of the goal gives one mark, with
//! the facts it rests on. There are no chart types. A chart, a timeline, a
//! spreadsheet grid or a flowchart is marks over the user's own facts and
//! rules, after the grammar of graphics (Wilkinson; Wickham 2010), Vega-Lite
//! (Satyanarayan et al. 2017) and Observable Plot ("Plot doesn't have chart
//! types; instead, you construct charts by layering marks").
//!
//! A mark is a term: its name, then channels, each `channel(Value)`:
//!
//! | Mark | Channels (required, then optional) | Draws |
//! | --- | --- | --- |
//! | `dot` | x, y; text | a point, labelled |
//! | `bar` | x, y; text | a bar from zero: vertical if x is a category, horizontal if y is |
//! | `line` | x, y; series | points joined in order of x, one line per series |
//! | `rule` | x or y | a reference line across the plot |
//! | `text` | x, y, text | a label at a point |
//! | `tick` | x; y, text | a mark on an axis, labelled: a timeline |
//! | `span` | x1, x2; y, text | an interval, on a lane y: a Gantt chart |
//! | `cell` | x, y; text | a box in a grid: a table, a spreadsheet |
//! | `node` | id; text, shape | a box in a graph, laid out in layers; shape box, round or decision |
//! | `edge` | from, to; text | an arrow between two nodes, by id |
//!
//! Values are numbers, atoms, strings, or date(Y, M, D). Scales follow from
//! them: numbers and dates are positions on a line; anything else is a
//! category, in standard order.

use format::terms::Term;

/// A mark's channels, by name, in the order written.
pub type Channels = Vec<(String, Term)>;

/// The kinds of mark, their required and optional channels.
pub const MARKS: &[(&str, &[&str], &[&str])] = &[
    ("dot", &["x", "y"], &["text"]),
    ("bar", &["x", "y"], &["text"]),
    ("line", &["x", "y"], &["series"]),
    ("rule", &[], &["x", "y"]),
    ("text", &["x", "y", "text"], &[]),
    ("tick", &["x"], &["y", "text"]),
    ("span", &["x1", "x2"], &["y", "text"]),
    ("cell", &["x", "y"], &["text"]),
    ("node", &["id"], &["text", "shape"]),
    ("edge", &["from", "to"], &["text"]),
];

/// The channels of a mark term, by name: `bar(x(C), y(V))` gives
/// [("x", C), ("y", V)]. Err says what is wrong, in words.
pub fn channels(mark: &Term) -> Result<(&'static str, Channels), String> {
    let (name, args) = match mark {
        Term::Compound(n, a) => (n.as_str(), a.as_slice()),
        Term::Atom(n) => (n.as_str(), &[][..]),
        _ => return Err("a mark is a term such as bar(x(C), y(V))".into()),
    };
    let kind = Kind::named(name)?;
    let mut out = Channels::new();
    for a in args {
        let channel = kind.channel(a)?;
        if out.iter().any(|(k, _)| *k == channel.0) {
            return Err(format!("{name}: {} is given twice", channel.0));
        }
        out.push(channel);
    }
    kind.check_complete(&out)?;
    Ok((kind.name, out))
}

/// A kind of mark: its name, and the channels it must and may have.
struct Kind {
    name: &'static str,
    required: &'static [&'static str],
    optional: &'static [&'static str],
}

impl Kind {
    fn named(name: &str) -> Result<Kind, String> {
        let found = MARKS.iter().find(|(k, _, _)| *k == name);
        let Some(&(name, required, optional)) = found else {
            let names: Vec<&str> = MARKS.iter().map(|(k, _, _)| *k).collect();
            return Err(format!(
                "no mark “{name}”; the marks are {}",
                names.join(", ")
            ));
        };
        Ok(Kind {
            name,
            required,
            optional,
        })
    }

    /// Every channel the kind has, the required first.
    fn all(&self) -> Vec<&'static str> {
        let both = self.required.iter().chain(self.optional);
        both.copied().collect()
    }

    /// One channel of a mark, written channel(Value).
    fn channel(&self, arg: &Term) -> Result<(String, Term), String> {
        let name = self.name;
        let Term::Compound(c, v) = arg else {
            return Err(format!(
                "{name}: each channel is written channel(Value), as x(C)"
            ));
        };
        if v.len() != 1 {
            return Err(format!("{name}: {c} takes one value"));
        }
        if !self.all().contains(&c.as_str()) {
            let all = self.all().join(", ");
            return Err(format!("{name} has no channel {c}; its channels are {all}"));
        }
        Ok((c.clone(), v[0].clone()))
    }

    /// A mark has every channel its kind requires; a rule has one only.
    fn check_complete(&self, given: &Channels) -> Result<(), String> {
        let has = |r: &&&str| given.iter().any(|(k, _)| k == **r);
        if let Some(missing) = self.required.iter().find(|r| !has(r)) {
            return Err(format!("{} needs {missing}(…)", self.name));
        }
        if self.name == "rule" && given.len() != 1 {
            return Err("rule needs exactly one of x(…) or y(…)".into());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------
// Drawing. A view's marks become a figure of characters: a plot (dot, bar,
// line, rule, text, tick, span), a grid (cell) or a graph (node, edge).
// The marks choose: any node or edge makes a graph; cells alone, a grid;
// anything else, a plot.

/// A mark to draw: the term, the seqs of the facts it rests on, and
/// whether any of them is in a violation (drawn loudly).
#[derive(Debug, Clone)]
pub struct Input {
    pub mark: Term,
    pub prov: Vec<u64>,
    pub loud: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ink {
    Plain,
    Dim,
    Loud,
}

/// A rectangle of the figure and the facts drawn there.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub prov: Vec<u64>,
}

/// What a view draws: rows of characters, where each mark landed, and
/// notes (legends, and what could not be drawn).
#[derive(Debug, Clone, Default)]
pub struct Figure {
    pub rows: Vec<Vec<(char, Ink)>>,
    pub hits: Vec<Hit>,
    pub notes: Vec<String>,
}

impl Figure {
    /// The rows as plain text, trailing spaces removed.
    pub fn text(&self) -> Vec<String> {
        self.rows
            .iter()
            .map(|r| {
                r.iter()
                    .map(|(c, _)| *c)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }
}

/// A value on a channel.
#[derive(Debug, Clone, PartialEq)]
enum V {
    Num(f64),
    /// A date, as days since 1970-01-01, and its text.
    Time(f64, String),
    Cat(String),
}

impl V {
    fn of(t: &Term) -> V {
        match t {
            Term::Int(n) => V::Num(*n as f64),
            Term::Float(x) => V::Num(*x),
            Term::Compound(f, a) if f == "date" && a.len() == 3 => match (&a[0], &a[1], &a[2]) {
                (Term::Int(y), Term::Int(m), Term::Int(d)) => {
                    V::Time(days(*y, *m, *d) as f64, format!("{y}-{m:02}-{d:02}"))
                }
                _ => V::Cat(label(t)),
            },
            _ => V::Cat(label(t)),
        }
    }
    fn num(&self) -> Option<f64> {
        match self {
            V::Num(x) | V::Time(x, _) => Some(*x),
            V::Cat(_) => None,
        }
    }
    fn text(&self) -> String {
        match self {
            V::Num(x) => number(*x),
            V::Time(_, s) | V::Cat(s) => s.clone(),
        }
    }
}

/// A term as a label: atoms and strings as they are, others as written.
fn label(t: &Term) -> String {
    match t {
        Term::Atom(a) => a.clone(),
        Term::Str(s) => s.clone(),
        other => crate::reason::show(other),
    }
}

fn number(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        let s = format!("{x:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// days_from_civil).
fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// A parsed mark.
struct M {
    kind: &'static str,
    ch: Vec<(String, V)>,
    prov: Vec<u64>,
    loud: bool,
}

impl M {
    fn read(input: &Input) -> Result<M, String> {
        let (kind, ch) = channels(&input.mark)?;
        let ch = ch.into_iter().map(|(k, t)| (k, V::of(&t))).collect();
        let (prov, loud) = (input.prov.clone(), input.loud);
        Ok(M {
            kind,
            ch,
            prov,
            loud,
        })
    }
    fn get(&self, c: &str) -> Option<&V> {
        self.ch.iter().find(|(k, _)| k == c).map(|(_, v)| v)
    }
    fn ink(&self) -> Ink {
        if self.loud { Ink::Loud } else { Ink::Plain }
    }
    /// A node or an edge: a mark of a graph.
    fn in_graph(&self) -> bool {
        matches!(self.kind, "node" | "edge")
    }
    fn is_cell(&self) -> bool {
        self.kind == "cell"
    }
}

/// The marks that can be read, each once, and what is wrong with the rest.
fn read_marks(inputs: &[Input]) -> (Vec<M>, Vec<String>) {
    let (mut marks, mut notes) = (Vec::new(), Vec::new());
    for input in inputs {
        match M::read(input) {
            Ok(m) => add_once(&mut marks, m),
            Err(e) => notes.push(e),
        }
    }
    (marks, notes)
}

/// Identical marks (a constant layer over many solutions) are drawn once,
/// loudly if any of them is loud.
fn add_once(marks: &mut Vec<M>, m: M) {
    match marks.iter_mut().find(|x| x.kind == m.kind && x.ch == m.ch) {
        Some(same) => same.loud |= m.loud,
        None => marks.push(m),
    }
}

/// A note of how many marks a frame leaves out, if it leaves any.
fn not_drawn(ms: &[M], left_out: fn(&M) -> bool, what: &str) -> Option<String> {
    let n = ms.iter().filter(|m| left_out(m)).count();
    (n > 0).then(|| format!("{n} {what}"))
}

/// Draw a view's marks in a space `width` wide and, for a plot, `height`
/// high. A grid or graph takes the height it needs.
pub fn render(inputs: &[Input], width: usize, height: usize) -> Figure {
    let (ms, mut notes) = read_marks(inputs);
    let width = width.max(8);
    let mut fig = if ms.iter().any(M::in_graph) {
        let others = "marks not drawn: a graph draws only node and edge";
        notes.extend(not_drawn(&ms, |m| !m.in_graph(), others));
        graph(&ms)
    } else if !ms.is_empty() && ms.iter().all(M::is_cell) {
        grid(&ms, width)
    } else {
        let cells = "cells not drawn: cells make a grid of their own";
        notes.extend(not_drawn(&ms, M::is_cell, cells));
        plot(&ms, width, height.max(4))
    };
    fig.notes.splice(0..0, notes);
    fig
}

/// A drawing surface with line junctions merged into box-drawing
/// characters.
struct Canvas {
    w: usize,
    h: usize,
    cells: Vec<Vec<(char, Ink)>>,
    /// Line directions at each cell: up 1, down 2, left 4, right 8.
    lines: Vec<Vec<(u8, Ink)>>,
    hits: Vec<Hit>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Canvas {
        Canvas {
            w,
            h,
            cells: vec![vec![(' ', Ink::Plain); w]; h],
            lines: vec![vec![(0, Ink::Dim); w]; h],
            hits: vec![],
        }
    }
    fn put(&mut self, x: usize, y: usize, c: char, ink: Ink) {
        if x < self.w && y < self.h {
            self.cells[y][x] = (c, ink);
        }
    }
    fn free(&self, x: usize, y: usize, n: usize) -> bool {
        y < self.h
            && x + n <= self.w
            && (x..x + n).all(|i| self.cells[y][i].0 == ' ' && self.lines[y][i].0 == 0)
    }
    /// A box three rows high and `w` wide, of its rows of characters (as
    /// frame_of gives them): each its left end, what fills it, and its
    /// right end.
    fn frame(&mut self, x: usize, y: usize, w: usize, rows: [[char; 3]; 3], ink: Ink) {
        for (i, [left, fill, right]) in rows.into_iter().enumerate() {
            let middle = fill.to_string().repeat(w - 2);
            self.text(x, y + i, &format!("{left}{middle}{right}"), ink);
        }
    }

    /// A line from a point to a column on its right, up or down that
    /// column, and back left to another point, with an arrow there.
    fn round_right(&mut self, from: (usize, usize), to: (usize, usize), column: usize, ink: Ink) {
        self.hline(from.0, column, from.1, ink);
        self.vline(column, to.1, from.1, ink);
        self.hline(to.0, column, to.1, ink);
        self.put(to.0, to.1, '◀', ink);
    }

    /// A line down from a point to a row, across, and down to another
    /// point.
    fn elbow(&mut self, from: (usize, usize), row: usize, to: (usize, usize), ink: Ink) {
        self.vline(from.0, from.1, row, ink);
        self.hline(from.0, to.0, row, ink);
        self.vline(to.0, row, to.1, ink);
    }

    /// A rectangle of one character.
    fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, c: char, ink: Ink) {
        for row in y..y + h {
            for col in x..x + w {
                self.put(col, row, c, ink);
            }
        }
    }
    fn text(&mut self, x: usize, y: usize, s: &str, ink: Ink) {
        for (i, c) in s.chars().enumerate() {
            self.put(x + i, y, c, ink);
        }
    }
    fn line(&mut self, x: usize, y: usize, bits: u8, ink: Ink) {
        if x < self.w && y < self.h {
            let (b, i) = self.lines[y][x];
            self.lines[y][x] = (b | bits, if i == Ink::Loud { i } else { ink });
        }
    }
    /// A horizontal line from x0 to x1 on row y.
    fn hline(&mut self, x0: usize, x1: usize, y: usize, ink: Ink) {
        let (a, b) = (x0.min(x1), x0.max(x1));
        for x in a..=b {
            let mut bits = 0;
            if x > a {
                bits |= 4;
            }
            if x < b {
                bits |= 8;
            }
            self.line(x, y, bits, ink);
        }
    }
    /// A vertical line from y0 to y1 at column x.
    fn vline(&mut self, x: usize, y0: usize, y1: usize, ink: Ink) {
        let (a, b) = (y0.min(y1), y0.max(y1));
        for y in a..=b {
            let mut bits = 0;
            if y > a {
                bits |= 1;
            }
            if y < b {
                bits |= 2;
            }
            self.line(x, y, bits, ink);
        }
    }
    fn hit(&mut self, x: usize, y: usize, w: usize, h: usize, prov: &[u64]) {
        if !prov.is_empty() && w > 0 && h > 0 {
            self.hits.push(Hit {
                x,
                y,
                w,
                h,
                prov: prov.to_vec(),
            });
        }
    }
    fn finish(self, notes: Vec<String>) -> Figure {
        let mut rows = self.cells;
        for (y, row) in self.lines.iter().enumerate() {
            for (x, (bits, ink)) in row.iter().enumerate() {
                if *bits != 0 && rows[y][x].0 == ' ' {
                    rows[y][x] = (junction(*bits), *ink);
                }
            }
        }
        Figure {
            rows,
            hits: self.hits,
            notes,
        }
    }
}

fn junction(bits: u8) -> char {
    match bits {
        1..=3 => '│',
        4 | 8 | 12 => '─',
        10 => '┌',
        6 => '┐',
        9 => '└',
        5 => '┘',
        11 => '├',
        7 => '┤',
        14 => '┬',
        13 => '┴',
        _ => '┼',
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else if n == 0 {
        String::new()
    } else {
        let mut t: String = s.chars().take(n - 1).collect();
        t.push('…');
        t
    }
}

// ---- plots ----------------------------------------------------------

/// A scale: numbers (and dates) onto a line, or categories onto bands.
#[derive(Debug, Clone)]
enum Scale {
    Line { lo: f64, hi: f64, time: bool },
    Band(Vec<String>),
    None,
}

impl Scale {
    fn train(values: &[&V], zero: bool) -> Scale {
        if values.is_empty() {
            return Scale::None;
        }
        if values.iter().all(|v| v.num().is_some()) {
            let xs: Vec<f64> = values.iter().filter_map(|v| v.num()).collect();
            let (mut lo, mut hi) = xs
                .iter()
                .fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
            if zero {
                lo = lo.min(0.0);
                hi = hi.max(0.0);
            }
            if lo == hi {
                lo -= 1.0;
                hi += 1.0;
            }
            let time = values.iter().all(|v| matches!(v, V::Time(..)));
            Scale::Line { lo, hi, time }
        } else {
            let mut cats: Vec<&V> = values.to_vec();
            cats.sort_by(|a, b| match (a.num(), b.num()) {
                (Some(x), Some(y)) => x.total_cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => a.text().cmp(&b.text()),
            });
            let mut out: Vec<String> = Vec::new();
            for c in cats {
                let t = c.text();
                if !out.contains(&t) {
                    out.push(t);
                }
            }
            Scale::Band(out)
        }
    }
    /// A value's position in 0..n, as (start, end) of its place: a single
    /// cell for a line scale, the band for a band scale.
    fn place(&self, v: &V, n: usize) -> Option<(usize, usize)> {
        let n = n.max(1);
        match self {
            Scale::Line { lo, hi, .. } => {
                let x = v.num()?;
                let p = ((x - lo) / (hi - lo) * (n - 1) as f64)
                    .round()
                    .clamp(0.0, (n - 1) as f64) as usize;
                Some((p, p))
            }
            Scale::Band(cats) => {
                let i = cats.iter().position(|c| *c == v.text())?;
                let a = i * n / cats.len();
                let b = ((i + 1) * n / cats.len()).max(a + 1) - 1;
                Some((a, b))
            }
            Scale::None => Some((n / 2, n / 2)),
        }
    }
    fn mid(&self, v: &V, n: usize) -> Option<usize> {
        self.place(v, n).map(|(a, b)| (a + b) / 2)
    }
    fn is_band(&self) -> bool {
        matches!(self, Scale::Band(_))
    }
    fn tick_text(&self, x: f64) -> String {
        match self {
            Scale::Line { time: true, .. } => {
                // Back from days to a year, near enough for an axis label.
                let y = 1970.0 + x / 365.2425;
                format!("{}", y.floor() as i64)
            }
            _ => number(x),
        }
    }
}

fn plot(ms: &[M], width: usize, height: usize) -> Figure {
    let frame = Frame::train(ms, width, height);
    let mut p = Plot {
        cv: Canvas::new(width, height),
        frame,
        notes: Vec::new(),
    };
    p.axes();
    p.y_labels();
    p.x_labels();
    // Rules first, under the other marks; then each kind in turn.
    p.rules(ms);
    p.bars(ms);
    p.lines(ms);
    p.spans(ms);
    p.points(ms);
    p.ticks(ms);
    p.cv.finish(p.notes)
}

/// Whether a plot's bars lie along the x axis: all have a category for y.
fn horizontal_bars(ms: &[M]) -> bool {
    let mut bars = ms.iter().filter(|m| m.kind == "bar").peekable();
    let categorical = |m: &M| m.get("y").is_some_and(|v| v.num().is_none());
    bars.peek().is_some() && bars.all(categorical)
}

/// Where a plot draws: its scales, the width of the margin that holds the
/// y labels, and the size of the plotting area to its right.
struct Frame {
    xs: Scale,
    ys: Scale,
    left: usize,
    /// The plotting area's width and height.
    size: (usize, usize),
    /// Bars lie along the x axis.
    horizontal: bool,
}

impl Frame {
    /// The scales that hold the marks' values, and the area left for them.
    fn train(ms: &[M], width: usize, height: usize) -> Frame {
        let horizontal = horizontal_bars(ms);
        let any_bars = ms.iter().any(|m| m.kind == "bar");
        let xv: Vec<&V> = ms
            .iter()
            .flat_map(|m| ["x", "x1", "x2"].map(|c| m.get(c)))
            .flatten()
            .collect();
        let yv: Vec<&V> = ms.iter().filter_map(|m| m.get("y")).collect();
        // Bars start at zero, on whichever axis holds their numbers.
        let xs = Scale::train(&xv, horizontal);
        let ys = Scale::train(&yv, any_bars && !horizontal);
        let left = margin(&ys);
        let size = (
            width.saturating_sub(left + 1).max(2),
            height.saturating_sub(2).max(2),
        );
        Frame {
            xs,
            ys,
            left,
            size,
            horizontal,
        }
    }

    /// The first column of the plotting area.
    fn x0(&self) -> usize {
        self.left + 1
    }

    /// The rows a value takes. Numbers rise up the page; categories read
    /// down it, first at the top.
    fn row(&self, v: &V) -> Option<(usize, usize)> {
        let ph = self.size.1;
        let (a, b) = self.ys.place(v, ph)?;
        Some(if self.ys.is_band() {
            (a, b)
        } else {
            (ph - 1 - b, ph - 1 - a)
        })
    }

    /// The middle row of a value's rows.
    fn mid_row(&self, v: &V) -> Option<usize> {
        self.row(v).map(|(c, d)| (c + d) / 2)
    }

    /// The columns a value takes.
    fn col(&self, v: &V) -> Option<(usize, usize)> {
        let x0 = self.x0();
        self.xs.place(v, self.size.0).map(|(a, b)| (x0 + a, x0 + b))
    }

    /// The middle column of a value's columns.
    fn mid_col(&self, v: &V) -> Option<usize> {
        self.xs.mid(v, self.size.0).map(|p| self.x0() + p)
    }
}

/// The width of the margin for a scale's labels on the y axis, with a
/// space after the longest.
fn margin(ys: &Scale) -> usize {
    let labels: Vec<String> = match ys {
        Scale::Band(c) => c.iter().map(|s| truncate(s, 14)).collect(),
        Scale::Line { lo, hi, .. } => vec![ys.tick_text(*lo), ys.tick_text(*hi)],
        Scale::None => vec![],
    };
    let longest = labels.iter().map(|s| s.chars().count()).max();
    longest.map_or(0, |n| n + 1)
}

/// The legend's points, one for each series of lines.
const POINTS: [char; 6] = ['●', '○', '■', '□', '◆', '◇'];

/// A plot being drawn: its canvas, its frame, and its notes so far.
struct Plot {
    cv: Canvas,
    frame: Frame,
    notes: Vec<String>,
}

impl Plot {
    /// The axes, dim: the y axis only if there is a y scale.
    fn axes(&mut self) {
        let (left, (pw, ph)) = (self.frame.left, self.frame.size);
        if !matches!(self.frame.ys, Scale::None) {
            self.cv.vline(left, 0, ph, Ink::Dim);
        }
        self.cv.hline(left, self.frame.x0() + pw - 1, ph, Ink::Dim);
    }

    /// The y labels, right-aligned in the margin.
    fn y_labels(&mut self) {
        for (text, y) in self.y_label_places() {
            let x = self.frame.left.saturating_sub(text.chars().count() + 1);
            self.cv.text(x, y, &text, Ink::Dim);
        }
    }

    /// Each y label and its row: every category at its middle, or a line
    /// scale's ends at the top and the bottom.
    fn y_label_places(&self) -> Vec<(String, usize)> {
        let f = &self.frame;
        match &f.ys {
            Scale::Band(c) => {
                let place =
                    |s: &String| f.mid_row(&V::Cat(s.clone())).map(|y| (truncate(s, 14), y));
                c.iter().filter_map(place).collect()
            }
            Scale::Line { lo, hi, .. } => {
                vec![
                    (f.ys.tick_text(*hi), 0),
                    (f.ys.tick_text(*lo), f.size.1 - 1),
                ]
            }
            Scale::None => vec![],
        }
    }

    /// The x labels, on the last row: each category centred in its band,
    /// or a line scale's ends at the two ends of the axis.
    fn x_labels(&mut self) {
        let f = &self.frame;
        let (x0, (pw, ly)) = (f.x0(), (f.size.0, f.size.1 + 1));
        let labels: Vec<(usize, String)> = match &f.xs {
            Scale::Band(c) => c.iter().filter_map(|s| band_label(f, s)).collect(),
            Scale::Line { lo, hi, .. } => {
                let r = f.xs.tick_text(*hi);
                let right = (x0 + pw).saturating_sub(r.chars().count());
                vec![(x0, f.xs.tick_text(*lo)), (right, r)]
            }
            Scale::None => vec![],
        };
        for (x, text) in labels {
            self.cv.text(x, ly, &text, Ink::Dim);
        }
    }

    /// Reference lines across the plot, at a y or at an x.
    fn rules(&mut self, ms: &[M]) {
        for m in ms.iter().filter(|m| m.kind == "rule") {
            let ink = if m.loud { Ink::Loud } else { Ink::Dim };
            if let Some(v) = m.get("y") {
                self.rule_across(v, ink);
            } else if let Some(v) = m.get("x") {
                self.rule_down(v, ink);
            }
        }
    }

    fn rule_across(&mut self, v: &V, ink: Ink) {
        if let Some(y) = self.frame.mid_row(v) {
            let x0 = self.frame.x0();
            self.cv.hline(x0, x0 + self.frame.size.0 - 1, y, ink);
        }
    }

    fn rule_down(&mut self, v: &V, ink: Ink) {
        if let Some((a, b)) = self.frame.col(v) {
            self.cv.vline((a + b) / 2, 0, self.frame.size.1 - 1, ink);
        }
    }

    /// Bars from zero, along whichever axis holds their numbers, each
    /// labelled with its text or its number.
    fn bars(&mut self, ms: &[M]) {
        for m in ms.iter().filter(|m| m.kind == "bar") {
            let (Some(x), Some(y)) = (m.get("x"), m.get("y")) else {
                continue;
            };
            if self.frame.horizontal {
                self.bar_across(m, x, y);
            } else {
                self.bar_up(m, x, y);
            }
        }
    }

    /// A bar along a row, from zero to its x.
    fn bar_across(&mut self, m: &M, x: &V, y: &V) {
        let f = &self.frame;
        let (Some(r), Some((c0, _)), Some((c1, _))) = (f.mid_row(y), f.col(&V::Num(0.0)), f.col(x))
        else {
            return;
        };
        let (a, b) = (c0.min(c1), c0.max(c1));
        for x in a..=b {
            self.cv.put(x, r, '█', m.ink());
        }
        let label = bar_label(m, x);
        self.cv.text(b + 2, r, &label, m.ink());
        self.cv.hit(a, r, b - a + 1, 1, &m.prov);
    }

    /// A bar up a column, from zero to its y, its label above it if there
    /// is room.
    fn bar_up(&mut self, m: &M, x: &V, y: &V) {
        let f = &self.frame;
        let (Some((ca, cb)), Some((r0, _)), Some((r1, _))) =
            (f.col(x), f.row(&V::Num(0.0)), f.row(y))
        else {
            return;
        };
        let w = (cb - ca + 1).saturating_sub(2).clamp(1, 8);
        let cx = (ca + cb).div_ceil(2) - w / 2;
        let (a, b) = (r0.min(r1), r0.max(r1));
        self.cv.fill(cx, a, w, b - a + 1, '█', m.ink());
        if a > 0 {
            let label = bar_label(m, y);
            let n = label.chars().count();
            self.cv
                .text((cx + w / 2).saturating_sub(n / 2), a - 1, &label, m.ink());
        }
        self.cv.hit(cx, a, w, b - a + 1, &m.prov);
    }

    /// Lines, one for each series, each joining its points in order of x.
    fn lines(&mut self, ms: &[M]) {
        let named = series_names(ms);
        for (k, s) in named.iter().enumerate() {
            let p = POINTS[k % POINTS.len()];
            let pts = self.series_points(ms, s);
            self.join(&pts);
            self.points_of(&pts, p);
            if named.len() > 1 || !s.is_empty() {
                self.notes.push(format!("{p} {s}"));
            }
        }
    }

    /// Where a series' marks land, in order of x.
    fn series_points<'a>(&self, ms: &'a [M], s: &str) -> Vec<(usize, usize, &'a M)> {
        let f = &self.frame;
        let at = |m: &'a M| {
            let (a, b) = f.col(m.get("x")?)?;
            Some(((a + b) / 2, f.mid_row(m.get("y")?)?, m))
        };
        let of_series = ms.iter().filter(|m| m.kind == "line" && series_of(m) == s);
        let mut pts: Vec<(usize, usize, &M)> = of_series.filter_map(at).collect();
        pts.sort_by_key(|(x, _, _)| *x);
        pts
    }

    /// Dotted lines between points in turn, where nothing is drawn yet.
    fn join(&mut self, pts: &[(usize, usize, &M)]) {
        for w in pts.windows(2) {
            let ((xa, ya, _), (xb, yb, _)) = (w[0], w[1]);
            for (x, y) in between((xa, ya), (xb, yb)) {
                if self.cv.cells[y][x].0 == ' ' {
                    self.cv.put(x, y, '·', Ink::Plain);
                }
            }
        }
    }

    fn points_of(&mut self, pts: &[(usize, usize, &M)], p: char) {
        for (x, y, m) in pts {
            self.cv.put(*x, *y, p, m.ink());
            self.cv.hit(*x, *y, 1, 1, &m.prov);
        }
    }

    /// Intervals on lanes: from x1 to x2, on the row of y or across the
    /// middle, labelled inside if the label fits, else after.
    fn spans(&mut self, ms: &[M]) {
        for m in ms.iter().filter(|m| m.kind == "span") {
            let f = &self.frame;
            let (Some(a), Some(b)) = (
                m.get("x1").and_then(|v| f.col(v)),
                m.get("x2").and_then(|v| f.col(v)),
            ) else {
                continue;
            };
            let r = match m.get("y") {
                Some(v) => f.mid_row(v),
                None => Some(f.size.1 / 2),
            };
            if let Some(r) = r {
                self.span(m, (a.0.min(b.0), a.0.max(b.0)), r);
            }
        }
    }

    fn span(&mut self, m: &M, (a, b): (usize, usize), r: usize) {
        for x in a..=b {
            self.cv.put(x, r, '━', m.ink());
        }
        if let Some(t) = m.get("text").map(V::text) {
            let inside = t.chars().count() + 2 <= b - a + 1;
            self.cv
                .text(if inside { a + 1 } else { b + 2 }, r, &t, m.ink());
        }
        self.cv.hit(a, r, b - a + 1, 1, &m.prov);
    }

    /// Dots and texts, each at its point.
    fn points(&mut self, ms: &[M]) {
        for m in ms.iter().filter(|m| matches!(m.kind, "dot" | "text")) {
            let f = &self.frame;
            let (Some(xv), Some(yv)) = (m.get("x"), m.get("y")) else {
                continue;
            };
            let (Some(x), Some(y)) = (f.mid_col(xv), f.mid_row(yv)) else {
                continue;
            };
            if m.kind == "dot" {
                self.dot(m, x, y);
            } else {
                self.text_mark(m, x, y);
            }
        }
    }

    /// A dot, labelled with its text if it has one.
    fn dot(&mut self, m: &M, x: usize, y: usize) {
        self.cv.put(x, y, '●', m.ink());
        if let Some(t) = m.get("text").map(V::text) {
            self.cv.text(x + 2, y, &t, m.ink());
        }
        self.cv.hit(x, y, 1, 1, &m.prov);
    }

    fn text_mark(&mut self, m: &M, x: usize, y: usize) {
        let t = m.get("text").map(V::text).unwrap_or_default();
        self.cv.text(x, y, &t, m.ink());
        self.cv.hit(x, y, t.chars().count(), 1, &m.prov);
    }

    /// Ticks, in order of x: on a lane if they have y; else on the x axis,
    /// labels stacked above it, each on the lowest row where it fits.
    fn ticks(&mut self, ms: &[M]) {
        for m in ticks_in_order(ms) {
            let Some(x) = m.get("x").and_then(|v| self.frame.mid_col(v)) else {
                continue;
            };
            let t = m
                .get("text")
                .or(m.get("x"))
                .map(V::text)
                .unwrap_or_default();
            match m.get("y") {
                Some(yv) => self.tick_on_lane(m, x, yv, &t),
                None => self.tick_on_axis(m, x, &t),
            }
        }
    }

    fn tick_on_lane(&mut self, m: &M, x: usize, yv: &V, t: &str) {
        let Some(y) = self.frame.mid_row(yv) else {
            return;
        };
        self.cv.put(x, y, '◆', m.ink());
        self.cv.text(x + 2, y, t, m.ink());
        self.cv.hit(x, y, 1, 1, &m.prov);
    }

    /// A tick on the axis, and its label on the lowest free row above,
    /// with a stem down to the axis; a note if no row has room.
    fn tick_on_axis(&mut self, m: &M, x: usize, t: &str) {
        let axis = self.frame.size.1;
        self.cv.put(x, axis, '┴', m.ink());
        let n = t.chars().count() + 1;
        let start = x.min(self.cv.w.saturating_sub(n));
        let Some(y) = (0..axis).rev().find(|y| self.cv.free(start, *y, n)) else {
            self.notes.push(format!("no room for the label {t}"));
            return;
        };
        self.cv.text(start, y, t, m.ink());
        self.stem(x, y + 1, axis);
        self.cv.hit(start, y, n - 1, axis - y + 1, &m.prov);
    }

    /// A dim line down column x, from row `from` to the row above `to`,
    /// where nothing is drawn yet.
    fn stem(&mut self, x: usize, from: usize, to: usize) {
        for r in from..to {
            if self.cv.cells[r][x].0 == ' ' {
                self.cv.put(x, r, '│', Ink::Dim);
            }
        }
    }
}

/// A category's label, centred in its band and cut to fit it.
fn band_label(f: &Frame, s: &str) -> Option<(usize, String)> {
    let (a, b) = f.col(&V::Cat(s.to_string()))?;
    let t = truncate(s, (b - a).max(1));
    let x = (a + b + 1).saturating_sub(t.chars().count()) / 2;
    Some((x, t))
}

/// A bar's label: its text, or the number it shows.
fn bar_label(m: &M, number: &V) -> String {
    m.get("text").map(V::text).unwrap_or_else(|| number.text())
}

fn series_of(m: &M) -> String {
    m.get("series").map(V::text).unwrap_or_default()
}

/// The series of a plot's lines, each once, in order of first appearance.
fn series_names(ms: &[M]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in ms.iter().filter(|m| m.kind == "line").map(series_of) {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// The cells strictly between two points, along a straight line.
fn between((xa, ya): (usize, usize), (xb, yb): (usize, usize)) -> Vec<(usize, usize)> {
    let n = xb.abs_diff(xa).max(ya.abs_diff(yb)).max(1);
    let step = |a: usize, b: usize, i: usize| {
        (a as f64 + (b as f64 - a as f64) * i as f64 / n as f64).round() as usize
    };
    (1..n).map(|i| (step(xa, xb, i), step(ya, yb, i))).collect()
}

/// A plot's ticks, in order of x; a tick without a number for x first.
fn ticks_in_order(ms: &[M]) -> Vec<&M> {
    let mut ticks: Vec<&M> = ms.iter().filter(|m| m.kind == "tick").collect();
    let x = |m: &M| m.get("x").and_then(V::num).unwrap_or(0.0);
    ticks.sort_by(|a, b| x(a).total_cmp(&x(b)));
    ticks
}

// ---- grids ----------------------------------------------------------

fn grid(ms: &[M], width: usize) -> Figure {
    let table = Table::of(ms);
    let columns = Columns::fit(&table, width);
    table.draw(&columns)
}

/// The names along one axis of a grid: a band scale's categories, else
/// the distinct values in order, so a numeric axis still lists them:
/// rows 1, 2, 3.
fn axis_names(ms: &[M], channel: &str) -> Vec<String> {
    let values: Vec<&V> = ms.iter().filter_map(|m| m.get(channel)).collect();
    match Scale::train(&values, false) {
        Scale::Band(c) => c,
        Scale::Line { .. } | Scale::None => distinct(values),
    }
}

/// Values' texts, each once, in order of number and then of text.
fn distinct(mut values: Vec<&V>) -> Vec<String> {
    let num = |v: &V| v.num().unwrap_or(0.0);
    values.sort_by(|a, b| num(a).total_cmp(&num(b)).then(a.text().cmp(&b.text())));
    let mut out: Vec<String> = Vec::new();
    for t in values.iter().map(|v| v.text()) {
        if !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// One cell of a grid: the texts of its marks, whether any is loud, the
/// facts under them, and whether the last is a number.
#[derive(Clone, Default)]
struct Cell {
    text: String,
    loud: bool,
    prov: Vec<u64>,
    number: bool,
}

impl Cell {
    /// Add a mark's text, or ■ if it has none.
    fn add(&mut self, m: &M) {
        if !self.text.is_empty() {
            self.text.push_str(", ");
        }
        self.text
            .push_str(&m.get("text").map(V::text).unwrap_or_else(|| "■".into()));
        self.loud |= m.loud;
        self.prov.extend(&m.prov);
        self.number = m.get("text").is_some_and(|v| v.num().is_some());
    }

    /// The width the cell's text takes, with its mark if it is loud.
    fn width(&self) -> usize {
        self.text.chars().count() + usize::from(self.loud)
    }

    /// The cell as shown in a column of a width: its text, cut to fit and
    /// marked ! if loud.
    fn shown(&self, width: usize) -> String {
        let mut t = truncate(&self.text, width - usize::from(self.loud));
        if self.loud {
            t.push('!');
        }
        t
    }

    fn ink(&self) -> Ink {
        if self.loud { Ink::Loud } else { Ink::Plain }
    }
}

/// A grid's columns and rows, by name, and its cells, row by row.
struct Table {
    cols: Vec<String>,
    rows: Vec<String>,
    cells: Vec<Vec<Cell>>,
}

impl Table {
    fn of(ms: &[M]) -> Table {
        let (cols, rows) = (axis_names(ms, "x"), axis_names(ms, "y"));
        let cells = vec![vec![Cell::default(); cols.len()]; rows.len()];
        let mut t = Table { cols, rows, cells };
        for m in ms {
            t.place(m);
        }
        t
    }

    /// Put a mark in the cell of its x and y, if it has both.
    fn place(&mut self, m: &M) {
        let (Some(x), Some(y)) = (m.get("x"), m.get("y")) else {
            return;
        };
        let i = self.cols.iter().position(|c| *c == x.text());
        let j = self.rows.iter().position(|r| *r == y.text());
        if let (Some(i), Some(j)) = (i, j) {
            self.cells[j][i].add(m);
        }
    }

    /// The width a column needs: its widest cell or its name, from 1 to 20.
    fn column_width(&self, i: usize) -> usize {
        let content = self.cells.iter().map(|r| r[i].width()).max().unwrap_or(0);
        content.max(self.cols[i].chars().count()).clamp(1, 20)
    }

    /// The width of the rows' names, at most 16.
    fn head(&self) -> usize {
        let longest = self.rows.iter().map(|r| r.chars().count()).max();
        longest.unwrap_or(0).min(16)
    }

    fn draw(&self, c: &Columns) -> Figure {
        let (w, h) = (c.total(), self.rows.len() + 2);
        let mut cv = Canvas::new(w.max(1), h);
        self.draw_head(&mut cv, c, w);
        for j in 0..self.rows.len() {
            self.draw_row(&mut cv, c, j);
        }
        cv.finish(vec![])
    }

    /// The columns' names, a rule under them, and a line before each
    /// column.
    fn draw_head(&self, cv: &mut Canvas, c: &Columns, w: usize) {
        for (i, name) in self.cols.iter().enumerate() {
            cv.text(c.at[i], 0, &truncate(name, c.widths[i]), Ink::Dim);
        }
        cv.hline(0, w.saturating_sub(1), 1, Ink::Dim);
        for x in &c.at {
            cv.vline(x - 2, 0, cv.h - 1, Ink::Dim);
        }
    }

    /// A row: its name, then each cell, a number to the right of its
    /// column.
    fn draw_row(&self, cv: &mut Canvas, c: &Columns, j: usize) {
        let y = j + 2;
        cv.text(0, y, &truncate(&self.rows[j], c.head), Ink::Dim);
        for (i, cell) in self.cells[j].iter().enumerate() {
            let t = cell.shown(c.widths[i]);
            let right = c.at[i] + c.widths[i] - t.chars().count();
            let x = if cell.number { right } else { c.at[i] };
            cv.text(x, y, &t, cell.ink());
            cv.hit(c.at[i], y, c.widths[i], 1, &cell.prov);
        }
    }
}

/// How a grid's columns are laid out: the width of the rows' names, each
/// column's width, and where each begins.
struct Columns {
    head: usize,
    widths: Vec<usize>,
    at: Vec<usize>,
    /// The width given to the grid.
    width: usize,
}

impl Columns {
    /// The columns each as wide as it needs, the widest narrowed in turn
    /// until the grid fits the width, or none is wider than 3.
    fn fit(t: &Table, width: usize) -> Columns {
        let head = t.head();
        let mut widths: Vec<usize> = (0..t.cols.len()).map(|i| t.column_width(i)).collect();
        while total(head, &widths) > width && narrow(&mut widths) {}
        let at = (0..widths.len())
            .map(|i| head + 3 + total(0, &widths[..i]))
            .collect();
        Columns {
            head,
            widths,
            at,
            width,
        }
    }

    /// The width the grid takes, at most the width given.
    fn total(&self) -> usize {
        total(self.head, &self.widths).min(self.width.max(1))
    }
}

/// The width of a grid with a head and columns of these widths, three
/// characters between them.
fn total(head: usize, widths: &[usize]) -> usize {
    head + widths.iter().map(|x| x + 3).sum::<usize>()
}

/// Narrow the widest column by one, unless it is 3 or narrower; whether
/// one was narrowed.
fn narrow(widths: &mut [usize]) -> bool {
    let Some(i) = (0..widths.len()).max_by_key(|i| widths[*i]) else {
        return false;
    };
    if widths[i] <= 3 {
        return false;
    }
    widths[i] -= 1;
    true
}

// ---- graphs ---------------------------------------------------------

fn graph(ms: &[M]) -> Figure {
    let mut g = Graph::read(ms);
    g.break_cycles();
    let at = Layout::of(&g);
    let ports = Ports::of(&g, &at);
    let mut d = Draw::new(&g, &at, &ports);
    // The edges under the boxes, and their labels after all the lines.
    let labels = d.lines();
    d.labels(labels);
    d.boxes();
    d.cv.finish(g.cycle_note().into_iter().collect())
}

/// A box of a graph: its key, its text and shape, and whether loud, with
/// the facts under it.
struct Node {
    key: String,
    text: String,
    shape: String,
    loud: bool,
    prov: Vec<u64>,
}

/// An edge between two nodes, by their places; laid out reversed when it
/// goes back up, to close a cycle.
struct Edge {
    ends: (usize, usize),
    text: String,
    loud: bool,
    reversed: bool,
}

impl Edge {
    /// Its ends as laid out: from above to below.
    fn laid(&self) -> (usize, usize) {
        let (a, b) = self.ends;
        if self.reversed { (b, a) } else { (a, b) }
    }

    fn ink(&self) -> Ink {
        if self.loud { Ink::Loud } else { Ink::Dim }
    }

    /// Its label, to go where given, else where it may go instead; none
    /// if it has no text.
    fn label(&self, at: (usize, usize), or: (usize, usize)) -> Option<Label> {
        let (text, ink) = (self.text.clone(), self.ink());
        (!self.text.is_empty()).then_some(Label { at, or, text, ink })
    }
}

/// A graph's nodes, in order of first appearance, and its edges.
struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

impl Graph {
    fn read(ms: &[M]) -> Graph {
        let mut g = Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        ms.iter()
            .filter(|m| m.kind == "node")
            .for_each(|m| g.add_node(m));
        ms.iter()
            .filter(|m| m.kind == "edge")
            .for_each(|m| g.add_edge(m));
        g
    }

    /// A node's place, made if it is new: a plain box named by its key.
    fn find(&mut self, key: &str) -> usize {
        if let Some(i) = self.nodes.iter().position(|n| n.key == key) {
            return i;
        }
        self.nodes.push(Node {
            key: key.into(),
            text: key.into(),
            shape: "box".into(),
            loud: false,
            prov: vec![],
        });
        self.nodes.len() - 1
    }

    fn add_node(&mut self, m: &M) {
        let i = self.find(&m.get("id").map(V::text).unwrap_or_default());
        let n = &mut self.nodes[i];
        if let Some(t) = m.get("text") {
            n.text = t.text();
        }
        if let Some(s) = m.get("shape") {
            n.shape = s.text();
        }
        n.loud |= m.loud;
        n.prov.extend(&m.prov);
    }

    /// An edge between two nodes, unless it is from a node to itself.
    fn add_edge(&mut self, m: &M) {
        let (Some(a), Some(b)) = (m.get("from"), m.get("to")) else {
            return;
        };
        let ends = (self.find(&a.text()), self.find(&b.text()));
        let text = m.get("text").map(V::text).unwrap_or_default();
        if ends.0 != ends.1 {
            let (loud, reversed) = (m.loud, false);
            self.edges.push(Edge {
                ends,
                text,
                loud,
                reversed,
            });
        }
    }

    /// Break cycles: an edge back to a node on the current depth-first path
    /// is laid out reversed, and drawn with its arrow at the top.
    fn break_cycles(&mut self) {
        let mut state = vec![Visit::New; self.nodes.len()];
        for v in 0..self.nodes.len() {
            if state[v] == Visit::New {
                self.visit(v, &mut state);
            }
        }
    }

    fn visit(&mut self, v: usize, state: &mut [Visit]) {
        state[v] = Visit::OnPath;
        for k in 0..self.edges.len() {
            let e = &self.edges[k];
            if e.ends.0 != v || e.reversed {
                continue;
            }
            match state[e.ends.1] {
                Visit::New => self.visit(e.ends.1, state),
                Visit::OnPath => self.edges[k].reversed = true,
                Visit::Done => {}
            }
        }
        state[v] = Visit::Done;
    }

    /// Each node's layer: the longest path to it from a source.
    fn layers(&self) -> Vec<usize> {
        let mut layer = vec![0usize; self.nodes.len()];
        for _ in 0..self.nodes.len() {
            if !self.deepen(&mut layer) {
                break;
            }
        }
        layer
    }

    /// Put each edge's lower end below its upper; whether any moved.
    fn deepen(&self, layer: &mut [usize]) -> bool {
        let mut changed = false;
        for (a, b) in self.edges.iter().map(Edge::laid) {
            if layer[b] < layer[a] + 1 {
                layer[b] = layer[a] + 1;
                changed = true;
            }
        }
        changed
    }

    fn reversed(&self) -> usize {
        self.edges.iter().filter(|e| e.reversed).count()
    }

    /// A note of the edges drawn back up, if there are any.
    fn cycle_note(&self) -> Option<String> {
        let n = self.reversed();
        let s = if n == 1 { "" } else { "s" };
        (n > 0).then(|| format!("{n} edge{s} back up, round the right, to close a cycle"))
    }
}

/// Where a depth-first search has been.
#[derive(Clone, Copy, PartialEq)]
enum Visit {
    New,
    OnPath,
    Done,
}

/// A part of an edge between two layers next to each other: from a vertex
/// to one below, which edge it is part of, and whether it is the last.
struct Seg {
    from: usize,
    to: usize,
    edge: usize,
    last: bool,
}

/// Where a graph is drawn. A vertex is a node, or a dummy on a long edge
/// in a layer that edge crosses: the dummies come after all the nodes.
struct Layout {
    /// Each vertex's layer, and the parts of the edges between them.
    layer: Vec<usize>,
    segs: Vec<Seg>,
    /// Each vertex's column and its width.
    x: Vec<usize>,
    width: Vec<usize>,
}

impl Layout {
    fn of(g: &Graph) -> Layout {
        let (layer, segs) = through_dummies(g);
        let width = (0..layer.len()).map(|v| box_width(g, v)).collect();
        let mut at = Layout {
            layer,
            segs,
            x: Vec::new(),
            width,
        };
        at.x = at.columns(&at.ordered());
        at
    }

    fn is_dummy(&self, v: usize, g: &Graph) -> bool {
        v >= g.nodes.len()
    }

    fn depth(&self) -> usize {
        self.layer.iter().copied().max().map_or(0, |d| d + 1)
    }

    /// The vertices of each layer, in order: first by number; then, to
    /// cross less, sweeping down and up, by the mean place of each
    /// vertex's neighbours in the layer before (its barycentre).
    fn ordered(&self) -> Vec<Vec<usize>> {
        let mut order: Vec<Vec<usize>> = vec![vec![]; self.depth()];
        for (v, l) in self.layer.iter().enumerate() {
            order[*l].push(v);
        }
        for sweep in 0..4 {
            self.sweep(&mut order, sweep % 2 == 0);
        }
        order
    }

    fn sweep(&self, order: &mut [Vec<usize>], down: bool) {
        let depth = order.len();
        let layers: Vec<usize> = if down {
            (1..depth).collect()
        } else {
            (0..depth.saturating_sub(1)).rev().collect()
        };
        for l in layers {
            let other = if down { l - 1 } else { l + 1 };
            order[l] = self.reorder(&order[l], &order[other], down);
        }
    }

    /// A layer by the barycentres of its vertices, ties kept in order.
    fn reorder(&self, layer: &[usize], other: &[usize], down: bool) -> Vec<usize> {
        let key = |(i, v): (usize, &usize)| (self.barycentre(*v, i, other, down), i, *v);
        let mut keyed: Vec<(f64, usize, usize)> = layer.iter().enumerate().map(key).collect();
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        keyed.into_iter().map(|(_, _, v)| v).collect()
    }

    /// The mean place, in the other layer, of a vertex's neighbours there;
    /// its own place i if it has none.
    fn barycentre(&self, v: usize, i: usize, other: &[usize], down: bool) -> f64 {
        let pos = |u: usize| other.iter().position(|x| *x == u);
        let neighbour = |s: &Seg| match down {
            true if s.to == v => pos(s.from),
            false if s.from == v => pos(s.to),
            _ => None,
        };
        let ns: Vec<usize> = self.segs.iter().filter_map(neighbour).collect();
        if ns.is_empty() {
            return i as f64;
        }
        ns.iter().sum::<usize>() as f64 / ns.len() as f64
    }

    /// A layer's width: its boxes, three columns apart.
    fn layer_width(&self, l: usize) -> usize {
        let on = (0..self.layer.len()).filter(|v| self.layer[*v] == l);
        on.map(|v| self.width[v] + 3)
            .sum::<usize>()
            .saturating_sub(3)
    }

    /// The width of the widest layer.
    fn body(&self) -> usize {
        let widths = (0..self.depth()).map(|l| self.layer_width(l));
        widths.max().unwrap_or(1).max(1)
    }

    /// Each vertex's column: each layer centred, its boxes three columns
    /// apart.
    fn columns(&self, order: &[Vec<usize>]) -> Vec<usize> {
        let mut x = vec![0usize; self.layer.len()];
        for (l, o) in order.iter().enumerate() {
            let mut at = (self.body() - self.layer_width(l)) / 2;
            for v in o {
                x[*v] = at;
                at += self.width[*v] + 3;
            }
        }
        x
    }

    /// The width of the boxes, and of the loops round the right that edges
    /// back up take, one column each, a column apart.
    fn figure_width(&self, g: &Graph) -> usize {
        let loops = match g.reversed() {
            0 => 0,
            n => 1 + 2 * n,
        };
        self.body() + loops
    }

    /// Boxes three rows high, three rows between layers.
    fn height(&self) -> usize {
        (self.depth() * 6).saturating_sub(3).max(1)
    }

    fn centre(&self, v: usize) -> usize {
        self.x[v] + self.width[v] / 2
    }

    /// The top row of a vertex's layer.
    fn top(&self, v: usize) -> usize {
        self.layer[v] * 6
    }

    /// The middle of a vertex's right side, just outside it.
    fn right_of(&self, v: usize) -> (usize, usize) {
        (self.x[v] + self.width[v], self.top(v) + 1)
    }
}

/// Each node's layer, then a dummy for each layer a long edge crosses,
/// each in its own layer; and the edges' parts, from layer to layer.
fn through_dummies(g: &Graph) -> (Vec<usize>, Vec<Seg>) {
    let mut layer = g.layers();
    let mut segs = Vec::new();
    for (edge, e) in g.edges.iter().enumerate() {
        let (a, b) = e.laid();
        let mut from = a;
        for l in layer[a] + 1..layer[b] {
            layer.push(l);
            let to = layer.len() - 1;
            segs.push(Seg {
                from,
                to,
                edge,
                last: false,
            });
            from = to;
        }
        segs.push(Seg {
            from,
            to: b,
            edge,
            last: true,
        });
    }
    (layer, segs)
}

/// A box's width: its text, cut at 22, and two columns either side; a
/// dummy's is one.
fn box_width(g: &Graph, v: usize) -> usize {
    match g.nodes.get(v) {
        Some(n) => n.text.chars().count().clamp(1, 22) + 4,
        None => 1,
    }
}

/// Where each part of an edge drawn down leaves its upper vertex and
/// enters its lower: spread along the box in the order of the other ends,
/// so that edges into or out of one node stay apart and do not cross
/// there.
struct Ports {
    /// The parts of edges that are not reversed.
    forward: Vec<usize>,
    out: Vec<usize>,
    into: Vec<usize>,
}

impl Ports {
    fn of(g: &Graph, at: &Layout) -> Ports {
        let forward = (0..at.segs.len()).filter(|i| !g.edges[at.segs[*i].edge].reversed);
        let n = at.segs.len();
        let mut p = Ports {
            forward: forward.collect(),
            out: vec![0; n],
            into: vec![0; n],
        };
        for v in 0..at.layer.len() {
            p.place(g, at, v);
        }
        p
    }

    /// The ports of the parts that leave a vertex, and of those that
    /// enter it.
    fn place(&mut self, g: &Graph, at: &Layout, v: usize) {
        let outs = self.ends(at, |s| s.from == v, |s| s.to);
        for (seg, px) in spread(g, at, v, outs) {
            self.out[seg] = px;
        }
        let ins = self.ends(at, |s| s.to == v, |s| s.from);
        for (seg, px) in spread(g, at, v, ins) {
            self.into[seg] = px;
        }
    }

    /// The forward parts that touch a vertex, each with the centre of its
    /// other end.
    fn ends(
        &self,
        at: &Layout,
        touches: impl Fn(&Seg) -> bool,
        other: fn(&Seg) -> usize,
    ) -> Vec<(usize, usize)> {
        let touching = self.forward.iter().filter(|i| touches(&at.segs[**i]));
        touching
            .map(|i| (at.centre(other(&at.segs[*i])), *i))
            .collect()
    }
}

/// Ports along a vertex for parts of edges, by the centres of their other
/// ends: a dummy's, or a lone one, at the centre; else spread evenly
/// inside the box.
fn spread(g: &Graph, at: &Layout, v: usize, mut ends: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    ends.sort();
    let (n, inner) = (ends.len(), at.width[v].saturating_sub(2));
    let alone = at.is_dummy(v, g) || n == 1;
    let px = |k: usize| {
        if alone {
            at.centre(v)
        } else {
            at.x[v] + 1 + (k + 1) * inner / (n + 1)
        }
    };
    ends.into_iter()
        .enumerate()
        .map(|(k, (_, seg))| (seg, px(k)))
        .collect()
}

/// A label to place: where it goes, else where it may go instead.
struct Label {
    at: (usize, usize),
    or: (usize, usize),
    text: String,
    ink: Ink,
}

/// A graph being drawn: the graph, where it lies, its ports, and the
/// canvas.
struct Draw<'a> {
    g: &'a Graph,
    at: &'a Layout,
    ports: &'a Ports,
    cv: Canvas,
}

impl<'a> Draw<'a> {
    fn new(g: &'a Graph, at: &'a Layout, ports: &'a Ports) -> Draw<'a> {
        let cv = Canvas::new(at.figure_width(g), at.height());
        Draw { g, at, ports, cv }
    }

    /// The edges back up, then those down; and their labels.
    fn lines(&mut self) -> Vec<Label> {
        let mut labels = self.loops();
        labels.extend(self.edges());
        labels
    }

    /// Edges back up, each from the right side of its source (below) up
    /// to the right side of its target (above), with the arrow at the
    /// target; each in a column of its own, round the right.
    fn loops(&mut self) -> Vec<Label> {
        let mut column = self.at.body() + 1;
        let mut labels = Vec::new();
        for e in self.g.edges.iter().filter(|e| e.reversed) {
            labels.extend(self.back_up(e, column));
            column += 2;
        }
        labels
    }

    fn back_up(&mut self, e: &Edge, column: usize) -> Option<Label> {
        let (from, to) = (self.at.right_of(e.ends.0), self.at.right_of(e.ends.1));
        self.cv.round_right(from, to, column, e.ink());
        e.label((column + 1, (from.1 + to.1) / 2), from)
    }

    /// The parts of edges drawn down, each from its upper vertex to the
    /// row above its lower one, by way of a run across between the layers.
    fn edges(&mut self) -> Vec<Label> {
        let mut labels = Vec::new();
        for &si in &self.ports.forward {
            labels.extend(self.down(si));
        }
        labels
    }

    fn down(&mut self, si: usize) -> Option<Label> {
        let s = &self.at.segs[si];
        let (e, ink) = (&self.g.edges[s.edge], self.g.edges[s.edge].ink());
        let (from, to) = (self.leaves(si), self.enters(si));
        let mid = self.at.top(s.from) + 4;
        self.cv.elbow(from, mid, to, ink);
        if !s.last {
            return None;
        }
        self.cv.put(to.0, to.1, '▼', ink);
        // A label goes beside the arrow, where the edge is its own; else
        // on its horizontal run.
        e.label((to.0 + 2, to.1), (from.0.max(to.0) + 2, mid))
    }

    /// Where a part of an edge leaves its upper vertex: below a box, or
    /// at the top of a dummy, so that the edge runs on through the
    /// dummy's layer.
    fn leaves(&self, si: usize) -> (usize, usize) {
        let v = self.at.segs[si].from;
        let below = if self.at.is_dummy(v, self.g) { 0 } else { 2 };
        (self.ports.out[si], self.at.top(v) + below)
    }

    /// Where a part of an edge ends: the row above its lower vertex.
    fn enters(&self, si: usize) -> (usize, usize) {
        let v = self.at.segs[si].to;
        (self.ports.into[si], self.at.top(v) - 1)
    }

    /// Each label where it goes if there is room, else where it may go
    /// instead if there is room there; else it is left out.
    fn labels(&mut self, labels: Vec<Label>) {
        for l in labels {
            let n = l.text.chars().count();
            let place = [l.at, l.or]
                .into_iter()
                .find(|(x, y)| self.cv.free(*x, *y, n));
            if let Some((x, y)) = place {
                self.cv.text(x, y, &l.text, l.ink);
            }
        }
    }

    /// The nodes' boxes, over the edges.
    fn boxes(&mut self) {
        for (v, n) in self.g.nodes.iter().enumerate() {
            self.boxed(v, n);
        }
    }

    fn boxed(&mut self, v: usize, n: &Node) {
        let (bx, by, ww) = (self.at.x[v], self.at.top(v), self.at.width[v]);
        let ink = if n.loud { Ink::Loud } else { Ink::Plain };
        self.cv.frame(bx, by, ww, frame_of(&n.shape), ink);
        self.cv
            .text(bx + 2, by + 1, &truncate(&n.text, ww - 4), ink);
        self.port_marks(v, by, ink);
        self.cv.hit(bx, by, ww, 3, &n.prov);
    }

    /// Where edges leave a box and arrive at it.
    fn port_marks(&mut self, v: usize, by: usize, ink: Ink) {
        for &i in &self.ports.forward {
            let s = &self.at.segs[i];
            if s.from == v {
                self.cv.put(self.ports.out[i], by + 2, '┬', ink);
            }
            if s.to == v && by > 0 {
                self.cv.put(self.ports.into[i], by, '┴', ink);
            }
        }
    }
}

/// The rows of a box of a shape: its top, its middle and its bottom.
fn frame_of(shape: &str) -> [[char; 3]; 3] {
    match shape {
        "round" => [['╭', '─', '╮'], ['│', ' ', '│'], ['╰', '─', '╯']],
        "decision" => [['╱', '─', '╲'], ['<', ' ', '>'], ['╲', '─', '╱']],
        _ => [['┌', '─', '┐'], ['│', ' ', '│'], ['└', '─', '┘']],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(f: &str, a: Vec<Term>) -> Term {
        Term::Compound(f.into(), a)
    }

    /// A mark from text such as "bar x:food y:400": channels as name:value,
    /// a value that parses as a number is one; date:1988-11-15 a date.
    fn mk(spec: &str) -> Input {
        let mut words = spec.split_whitespace();
        let kind = words.next().unwrap();
        let args = words
            .map(|w| {
                let (k, v) = w.split_once(':').unwrap();
                let v = v.replace('_', " ");
                let val = if let Ok(n) = v.parse::<i64>() {
                    Term::Int(n)
                } else if let Some(d) = v.strip_prefix("date") {
                    let p: Vec<i64> = d
                        .split('-')
                        .filter(|x| !x.is_empty())
                        .map(|x| x.parse().unwrap())
                        .collect();
                    t("date", p.into_iter().map(Term::Int).collect())
                } else {
                    Term::Atom(v)
                };
                t(k, vec![val])
            })
            .collect();
        Input {
            mark: t(kind, args),
            prov: vec![1],
            loud: false,
        }
    }

    fn draw(specs: &[&str], w: usize, h: usize) -> Vec<String> {
        let ins: Vec<Input> = specs.iter().map(|s| mk(s)).collect();
        let f = render(&ins, w, h);
        let mut out = f.text();
        out.extend(f.notes.iter().map(|n| format!("note: {n}")));
        out
    }

    /// Column of the first occurrence of text in a row, and the row.
    fn at(lines: &[String], text: &str) -> (usize, usize) {
        for (y, l) in lines.iter().enumerate() {
            if let Some(b) = l.find(text) {
                return (l[..b].chars().count(), y);
            }
        }
        panic!("no {text:?} in\n{}", lines.join("\n"))
    }

    /// How many cells of column x hold c.
    fn count(lines: &[String], x: usize, c: char) -> usize {
        lines.iter().filter(|l| l.chars().nth(x) == Some(c)).count()
    }

    #[test]
    fn bars_rise_with_their_values_over_categories_in_order() {
        // Catches bars of the wrong height, categories out of order, a
        // reference rule not drawn, or a value label lost.
        let l = draw(
            &[
                "bar x:income y:2000",
                "bar x:rent y:1200",
                "bar x:food y:400",
                "rule y:1500",
            ],
            40,
            10,
        );
        let (f, fy) = at(&l, "food");
        let (i, _) = at(&l, "income");
        let (r, _) = at(&l, "rent");
        assert!(f < i && i < r, "categories in order:\n{}", l.join("\n"));
        assert_eq!(fy, l.len() - 1, "labels on the last row");
        let h = |x: usize| count(&l, x + 1, '█');
        assert!(
            h(i) > h(r) && h(r) > h(f) && h(f) > 0,
            "{} {} {}",
            h(i),
            h(r),
            h(f)
        );
        assert!(l.iter().any(|x| x.contains("────────────")), "the rule");
        assert!(l.iter().any(|x| x.contains("400")), "a value label");
    }

    #[test]
    fn horizontal_bars_read_down_the_page() {
        // Catches category bands drawn bottom up, or bar lengths unordered.
        let l = draw(
            &[
                "bar y:income x:2000",
                "bar y:rent x:1200",
                "bar y:food x:400",
            ],
            40,
            6,
        );
        let (_, yf) = at(&l, "food");
        let (_, yi) = at(&l, "income");
        let (_, yr) = at(&l, "rent");
        assert!(
            yf < yi && yi < yr,
            "first category at the top:\n{}",
            l.join("\n")
        );
        let len = |y: usize| l[y].matches('█').count();
        assert!(len(yi) > len(yr) && len(yr) > len(yf));
    }

    #[test]
    fn lines_join_points_in_order_with_a_legend_per_series() {
        // Catches series mixed together, or a legend missing.
        let l = draw(
            &[
                "line x:1 y:3 series:a",
                "line x:2 y:5 series:a",
                "line x:3 y:4 series:a",
                "line x:1 y:1 series:b",
                "line x:3 y:6 series:b",
            ],
            30,
            8,
        );
        assert!(
            l.contains(&"note: ● a".to_string()) && l.contains(&"note: ○ b".to_string()),
            "{l:?}"
        );
        assert_eq!(
            l.iter().map(|x| x.matches('●').count()).sum::<usize>(),
            3 + 1,
            "3 points, and the legend"
        );
    }

    #[test]
    fn a_timeline_places_every_label_in_time_order() {
        // Catches labels dropped when two dates are close, or out of order.
        let l = draw(
            &[
                "tick x:date1987-05-15 text:Energia",
                "tick x:date1988-11-15 text:Buran_flight",
                "tick x:date1993-06-30 text:Cancelled",
                "tick x:date1988-12-01 text:Close_one",
            ],
            50,
            6,
        );
        let xs: Vec<usize> = ["Energia", "Buran flight", "Cancelled"]
            .iter()
            .map(|t| at(&l, t).0)
            .collect();
        assert!(xs[0] < xs[1] && xs[1] < xs[2], "{}", l.join("\n"));
        at(&l, "Close one");
        assert!(
            !l.iter().any(|x| x.starts_with("note")),
            "nothing lost: {l:?}"
        );
        at(&l, "1987");
        at(&l, "1993");
    }

    #[test]
    fn spans_lie_on_lanes_between_their_ends() {
        // Catches lanes out of order, or a span not starting at its x1.
        let l = draw(
            &[
                "span y:design x1:1 x2:4 text:design",
                "span y:build x1:3 x2:9 text:build",
                "span y:test x1:8 x2:10 text:test",
            ],
            40,
            6,
        );
        let (_, yb) = at(&l, "build │");
        let (_, yd) = at(&l, "design │");
        let (_, yt) = at(&l, "test │");
        assert!(yb < yd && yd < yt, "{}", l.join("\n"));
        let start = |y: usize| l[y].chars().position(|c| c == '━').unwrap();
        assert!(start(yd) < start(yb) && start(yb) < start(yt));
    }

    #[test]
    fn cells_make_a_grid() {
        // Catches a grid with rows or columns misplaced, or numbers not
        // aligned right.
        let l = draw(
            &[
                "cell x:a y:1 text:income",
                "cell x:b y:1 text:2000",
                "cell x:a y:2 text:rent",
                "cell x:b y:2 text:1200",
                "cell x:a y:3 text:left",
                "cell x:b y:3 text:400",
            ],
            40,
            10,
        );
        assert_eq!(
            l,
            [
                "  │ a      │ b",
                "──┼────────┼─────",
                "1 │ income │ 2000",
                "2 │ rent   │ 1200",
                "3 │ left   │  400"
            ]
        );
    }

    #[test]
    fn a_flowchart_flows_down_with_labels_and_a_cycle() {
        // Catches layers out of order, an arrow missing or doubled, an edge
        // label away from its edge, or a cycle drawn as a forward edge.
        let l = draw(
            &[
                "node id:start text:Start shape:round",
                "node id:ask text:Is_it_late? shape:decision",
                "node id:go text:Go_home",
                "node id:stay text:Keep_working",
                "edge from:start to:ask",
                "edge from:ask to:go text:yes",
                "edge from:ask to:stay text:no",
                "edge from:stay to:ask",
            ],
            60,
            0,
        );
        let (_, ys) = at(&l, "Start");
        let (_, ya) = at(&l, "Is it late?");
        let (xg, yg) = at(&l, "Go home");
        assert!(ys < ya && ya < yg, "{}", l.join("\n"));
        assert_eq!(l.iter().map(|x| x.matches('▼').count()).sum::<usize>(), 3);
        assert_eq!(l.iter().map(|x| x.matches('◀').count()).sum::<usize>(), 1);
        let (xy, yy) = at(&l, "yes");
        assert_eq!(yy, yg - 2, "beside the arrow into Go home");
        assert!(xy < xg + 8);
        assert!(
            l.iter().any(|x| x.contains("< Is it late? >")),
            "a decision's shape"
        );
        assert!(l.iter().any(|x| x.contains("╭───────╮")), "a round shape");
        assert!(l.last().unwrap().contains("back up"));
    }

    #[test]
    fn a_long_edge_goes_round_the_boxes_between() {
        // Catches an edge over two layers drawn through the box between.
        let l = draw(
            &["edge from:a to:b", "edge from:b to:c", "edge from:a to:c"],
            40,
            0,
        );
        let (_, yb) = at(&l, "│ b │");
        let (_, yc) = at(&l, "│ c │");
        assert_eq!(
            l.iter().map(|x| x.matches('▼').count()).sum::<usize>(),
            3,
            "{}",
            l.join("\n")
        );
        assert!(
            l[yb].matches('│').count() >= 3,
            "the long edge beside b:\n{}",
            l.join("\n")
        );
        assert!(yc > yb);
    }

    #[test]
    fn loud_marks_and_where_marks_landed() {
        // Catches a mark on a violated fact drawn quietly, or a click region
        // that misses its mark.
        let mut a = mk("bar x:a y:5");
        a.loud = true;
        a.prov = vec![7];
        let f = render(&[a, mk("bar x:b y:3")], 20, 8);
        let loud: usize = f
            .rows
            .iter()
            .flatten()
            .filter(|(c, i)| *c == '█' && *i == Ink::Loud)
            .count();
        assert!(loud > 0);
        let h = f.hits.iter().find(|h| h.prov == [7]).unwrap();
        assert_eq!(f.rows[h.y][h.x].0, '█');
        let mut c = mk("cell x:a y:1 text:9");
        c.loud = true;
        let g = render(&[c], 20, 0);
        assert!(g.text()[2].contains("9!"), "{:?}", g.text());
    }

    #[test]
    fn marks_that_do_not_fit_the_frame_are_named() {
        // Catches marks silently dropped when kinds are mixed.
        let l = draw(&["node id:a", "bar x:a y:1"], 30, 6);
        assert!(
            l.iter()
                .any(|x| x == "note: 1 marks not drawn: a graph draws only node and edge"),
            "{l:?}"
        );
        let l = draw(&["cell x:a y:1", "bar x:a y:1"], 30, 6);
        assert!(l.iter().any(|x| x.contains("cells not drawn")), "{l:?}");
    }

    #[test]
    fn marks_are_checked() {
        // Catches a mark accepted with a missing, unknown or doubled channel.
        let c = |n: &str| t(n, vec![Term::Var(0)]);
        assert!(channels(&t("bar", vec![c("x"), c("y")])).is_ok());
        assert!(channels(&t("rule", vec![c("y")])).is_ok());
        let cases = [
            (t("pie", vec![c("x")]), "no mark “pie”"),
            (t("bar", vec![c("x")]), "bar needs y(…)"),
            (
                t("bar", vec![c("x"), c("y"), c("colour")]),
                "no channel colour",
            ),
            (t("bar", vec![c("x"), c("x"), c("y")]), "x is given twice"),
            (t("rule", vec![]), "exactly one"),
            (t("rule", vec![c("x"), c("y")]), "exactly one"),
            (
                t("bar", vec![Term::Atom("x".into()), c("y")]),
                "channel(Value)",
            ),
        ];
        for (m, why) in cases {
            let e = channels(&m).unwrap_err();
            assert!(e.contains(why), "{m:?}: {e}");
        }
    }

    #[test]
    fn a_channel_takes_one_value() {
        // Catches x(1, 2) read as x(1).
        let two = t("x", vec![Term::Int(1), Term::Int(2)]);
        let said = channels(&t("rule", vec![two])).unwrap_err();
        assert_eq!(said, "rule: x takes one value");
    }

    #[test]
    fn identical_marks_are_drawn_once_and_loudly_if_any_is_loud() {
        // Catches a violation's mark drawn plainly because a plain copy of
        // it came first.
        let loud = Input {
            loud: true,
            ..mk("dot x:1 y:1")
        };
        let figure = render(&[mk("dot x:1 y:1"), loud], 20, 6);
        let inks = figure
            .rows
            .iter()
            .flatten()
            .filter(|(_, ink)| *ink == Ink::Loud);
        assert_eq!((figure.hits.len(), inks.count() > 0), (1, true));
    }

    #[test]
    fn a_rule_at_an_x_runs_down_the_plot() {
        // Catches a rule given an x not drawn, or drawn across. The picture is what the
        // code drew before plot was restructured.
        let want = [
            "10 │          │        ●",
            "   │          │",
            "   │          │",
            "   │          │",
            "   │          │",
            " 0 │●         │",
            "   └────────────────────",
            "    0                 10",
        ];
        assert_eq!(
            draw(&["dot x:0 y:0", "dot x:10 y:10", "rule x:5"], 24, 8),
            want
        );
    }

    #[test]
    fn a_span_is_labelled_inside_if_its_label_fits_and_after_if_not() {
        // Catches a span's label always inside, or always after. The picture is what the
        // code drew before plot was restructured.
        let want = [
            "a │━inside━━━━━━━━━━━━━━━━━━━━",
            "  │",
            "b │━━━━ out",
            "  │",
            "  └───────────────────────────",
            "   0                        20",
        ];
        assert_eq!(
            draw(
                &[
                    "span x1:0 x2:20 y:a text:inside",
                    "span x1:0 x2:2 y:b text:out"
                ],
                30,
                6
            ),
            want
        );
    }

    #[test]
    fn a_span_without_a_lane_lies_across_the_middle() {
        // Catches a span with no y left out. The picture is what the
        // code drew before plot was restructured.
        let want = [
            "",
            "",
            "",
            " ━━━━━━━━━━━━━━━━━━━━━━━",
            "",
            "",
            "────────────────────────",
            " 0                    10",
        ];
        assert_eq!(draw(&["span x1:0 x2:10"], 24, 8), want);
    }

    #[test]
    fn a_tick_label_with_no_room_is_named() {
        // Catches a tick's label dropped silently when no row has room. The picture is what the
        // code drew before plot was restructured.
        let want = [
            "         bbbbbbbbbb",
            " aaaaaaaaaa",
            "─┴────────┴────────┴",
            " 1                 3",
            "note: no room for the label cccccccccc",
        ];
        assert_eq!(
            draw(
                &[
                    "tick x:1 text:aaaaaaaaaa",
                    "tick x:2 text:bbbbbbbbbb",
                    "tick x:3 text:cccccccccc"
                ],
                20,
                4
            ),
            want
        );
    }

    #[test]
    fn ticks_stack_their_labels_in_order_of_x_with_a_stem_to_the_axis() {
        // Catches ticks stacked in the order given, or a raised label with no stem down to its tick. The picture is what the
        // code drew before plot was restructured.
        let want = [
            "",
            "",
            "",
            "",
            "  c    bbbbbbbbbbbbbbbbbbbbbb",
            " aaaaaaaaaa                  │",
            "─┴┴──────────────────────────┴",
            " 1                          50",
        ];
        assert_eq!(
            draw(
                &[
                    "tick x:50 text:bbbbbbbbbbbbbbbbbbbbbb",
                    "tick x:1 text:aaaaaaaaaa",
                    "tick x:2 text:c"
                ],
                30,
                8
            ),
            want
        );
    }

    #[test]
    fn a_line_is_not_drawn_over_a_bar() {
        // Catches the dots joining a line drawn over a bar already there. The picture is what the
        // code drew before plot was restructured.
        let want = [
            "10 │        █     ·●",
            "   │        █  ···",
            "   │        █··",
            "   │     ···█",
            "   │  ···   █",
            " 0 │●·      █",
            "   └────────────────",
            "    0             10",
        ];
        assert_eq!(
            draw(&["bar x:5 y:10", "line x:0 y:0", "line x:10 y:10"], 20, 8),
            want
        );
    }

    #[test]
    fn a_dot_is_labelled_with_its_text() {
        // Catches a dot's text dropped. The picture is what the
        // code drew before plot was restructured.
        let want = [
            "3 │                ●",
            "  │",
            "  │",
            "1 │● here",
            "  └─────────────────",
            "   1               5",
        ];
        assert_eq!(draw(&["dot x:1 y:1 text:here", "dot x:5 y:3"], 20, 6), want);
    }

    #[test]
    fn ticks_alone_have_no_y_axis() {
        // Catches a y axis drawn with no y scale. The picture is what the
        // code drew before plot was restructured.
        let want = [
            "",
            "",
            " a                    b",
            "─┴─────────────────────┴",
            " 1                     5",
        ];
        assert_eq!(draw(&["tick x:1 text:a", "tick x:5 text:b"], 24, 5), want);
    }

    #[test]
    fn marks_in_one_cell_are_listed_with_commas() {
        // Catches two marks in one cell run together, or one of them lost. The picture is what the code
        // drew before grid was restructured.
        let want = ["  │ a", "──┼─────", "1 │ p, q"];
        assert_eq!(
            draw(&["cell x:a y:1 text:p", "cell x:a y:1 text:q"], 30, 4),
            want
        );
    }

    #[test]
    fn columns_narrow_to_fit_the_width() {
        // Catches columns left as wide as their texts when the grid does not fit. The picture is what the code
        // drew before grid was restructured.
        let want = [
            "  │ a        │ b",
            "──┼──────────┼────────",
            "1 │ aaaaaaa… │ bbbbbb…",
        ];
        assert_eq!(
            draw(
                &[
                    "cell x:a y:1 text:aaaaaaaaaaaaaaa",
                    "cell x:b y:1 text:bbbbbbbbbbbb"
                ],
                22,
                4
            ),
            want
        );
    }

    #[test]
    fn no_column_narrows_below_three() {
        // Catches a column narrowed past three characters. The picture is what the code
        // drew before grid was restructured.
        let want = ["  │ a   │ b", "──┼─────┼───", "1 │ xx… │ xx"];
        assert_eq!(
            draw(
                &[
                    "cell x:a y:1 text:xxxxxx",
                    "cell x:b y:1 text:xxxxxx",
                    "cell x:c y:1 text:xxxxxx"
                ],
                12,
                4
            ),
            want
        );
    }

    #[test]
    fn a_rows_name_is_cut_at_sixteen() {
        // Catches a long row's name given all the width it asks. The picture is what the code
        // drew before grid was restructured.
        let want = [
            "                 │ a",
            "─────────────────┼──",
            "a very long row… │ 1",
        ];
        assert_eq!(
            draw(&["cell x:a y:a_very_long_row_name_indeed text:1"], 60, 4),
            want
        );
    }

    #[test]
    fn a_column_is_at_most_twenty_wide() {
        // Catches a column as wide as its longest text, past twenty. The picture is what the code
        // drew before grid was restructured.
        let want = [
            "  │ a",
            "──┼─────────────────────",
            "1 │ twenty five charact…",
        ];
        assert_eq!(
            draw(&["cell x:a y:1 text:twenty_five_characters_long"], 80, 4),
            want
        );
    }

    #[test]
    fn each_cell_knows_where_its_facts_lie() {
        // Catches a grid whose cells cannot be clicked, or are found in the
        // wrong place. The places are what the code gave before grid was
        // restructured.
        let f = render(&[mk("cell x:a y:1"), mk("cell x:b y:2")], 30, 0);
        let hit = |x, y| Hit {
            x,
            y,
            w: 1,
            h: 1,
            prov: vec![1],
        };
        assert_eq!(f.hits, [hit(4, 2), hit(8, 3)]);
    }

    #[test]
    fn boxes_take_the_shape_asked_for() {
        // Catches a round box or a decision drawn as a plain box. The picture is what
        // the code drew before graph was restructured.
        let want = [
            "╭───╮",
            "│ a │",
            "╰─┬─╯",
            "  │",
            "  │",
            "  ▼",
            "╱─┴─╲",
            "< b >",
            "╲─┬─╱",
            "  │",
            "  │",
            "  ▼",
            "┌─┴─┐",
            "│ c │",
            "└───┘",
        ];
        assert_eq!(
            draw(
                &[
                    "node id:a shape:round",
                    "node id:b shape:decision",
                    "node id:c",
                    "edge from:a to:b",
                    "edge from:b to:c"
                ],
                40,
                0
            ),
            want
        );
    }

    #[test]
    fn an_edges_label_goes_beside_its_arrow() {
        // Catches an edge's label lost, or put on its run when there is room by its arrow. The picture is what
        // the code drew before graph was restructured.
        let want = [
            "    ┌───┐",
            "    │ a │",
            "    └─┬┬┘",
            "      ││",
            "  ┌───┘└──┐",
            "  ▼ yes   ▼",
            "┌─┴─┐   ┌─┴─┐",
            "│ b │   │ c │",
            "└───┘   └───┘",
        ];
        assert_eq!(
            draw(
                &["edge from:a to:b text:yes", "edge from:a to:c text:no"],
                40,
                0
            ),
            want
        );
    }

    #[test]
    fn labels_with_no_room_beside_the_arrow_or_on_the_run_are_dropped() {
        // Catches a label drawn over another, or past the edge of the
        // figure. Both labels here are dropped, without a note: the figure
        // is only as wide as its boxes. The picture is what the code drew
        // before graph was restructured.
        let want = [
            "┌───┐   ┌───┐",
            "│ a │   │ c │",
            "└─┬─┘   └─┬─┘",
            "  │       │",
            "  └───┐┌──┘",
            "      ▼▼",
            "    ┌─┴┴┐",
            "    │ b │",
            "    └───┘",
        ];
        assert_eq!(
            draw(
                &[
                    "edge from:a to:b text:long_label_here",
                    "edge from:c to:b text:another_label"
                ],
                40,
                0
            ),
            want
        );
    }

    #[test]
    fn cycles_are_closed_round_the_right_and_counted() {
        // Catches an edge back up drawn down, its loops run together, or the note not counted in the plural. The picture is what
        // the code drew before graph was restructured.
        let want = [
            "┌───┐",
            "│ a │◀┐",
            "└─┬─┘ │",
            "  │   │",
            "  │   │",
            "  ▼   │",
            "┌─┴─┐ │",
            "│ b │◀┴─┐",
            "└─┬─┘   │",
            "  │     │",
            "  │     │",
            "  ▼     │",
            "┌─┴─┐   │",
            "│ c │───┘",
            "└───┘",
            "note: 2 edges back up, round the right, to close a cycle",
        ];
        assert_eq!(
            draw(
                &[
                    "edge from:a to:b",
                    "edge from:b to:a",
                    "edge from:b to:c",
                    "edge from:c to:b"
                ],
                40,
                0
            ),
            want
        );
    }

    #[test]
    fn a_label_with_no_room_beside_its_loop_is_dropped() {
        // Catches a label drawn past the edge of the figure: there is one
        // column right of the loop, and "back" is dropped, without a note.
        // The picture is what the code drew before graph was restructured.
        let want = [
            "┌───┐",
            "│ a │◀┐",
            "└─┬─┘ │",
            "  │   │",
            "  │   │",
            "  ▼   │",
            "┌─┴─┐ │",
            "│ b │─┘",
            "└───┘",
            "note: 1 edge back up, round the right, to close a cycle",
        ];
        assert_eq!(
            draw(&["edge from:a to:b", "edge from:b to:a text:back"], 40, 0),
            want
        );
    }

    #[test]
    fn edges_from_two_boxes_to_four_and_into_one_are_laid_out_apart() {
        // Catches ports or runs drawn over each other. The layers stay in
        // the order the nodes came: a_layer_is_reordered_so_its_edges_cross_less
        // tests the reordering. The picture is what the code drew before
        // graph was restructured.
        let want = [
            "        ┌───┐   ┌───┐",
            "        │ a │   │ b │",
            "        └─┬┬┘   └─┬┬┘",
            "          ││      ││",
            "  ┌───────┼┴──────┤└──────┐",
            "  ▼       ▼       ▼       ▼",
            "┌─┴─┐   ┌─┴─┐   ┌─┴─┐   ┌─┴─┐",
            "│ d │   │ c │   │ e │   │ f │",
            "└─┬─┘   └─┬─┘   └───┘   └───┘",
            "  │       │",
            "  └───────┴───┬┐",
            "              ▼▼",
            "            ┌─┴┴┐",
            "            │ g │",
            "            └───┘",
        ];
        assert_eq!(
            draw(
                &[
                    "edge from:a to:d",
                    "edge from:b to:c",
                    "edge from:a to:e",
                    "edge from:b to:f",
                    "edge from:c to:g",
                    "edge from:d to:g"
                ],
                60,
                0
            ),
            want
        );
    }

    #[test]
    fn a_long_name_is_cut_to_fit_its_box() {
        // Catches a box as wide as a long name. The picture is what
        // the code drew before graph was restructured.
        let want = [
            "┌────────────────────────┐",
            "│ a very long node text… │",
            "└────────────┬───────────┘",
            "             │",
            "            ┌┘",
            "            ▼",
            "          ┌─┴─┐",
            "          │ b │",
            "          └───┘",
        ];
        assert_eq!(
            draw(
                &[
                    "node id:a text:a_very_long_node_text_that_runs_on",
                    "edge from:a to:b"
                ],
                60,
                0
            ),
            want
        );
    }

    #[test]
    fn long_edges_pass_through_the_layers_between() {
        // Catches an edge across layers drawn through the boxes between. The picture is what
        // the code drew before graph was restructured.
        let want = [
            "    ┌───┐",
            "    │ a │",
            "    └┬┬┬┘",
            "     │││",
            "  ┌──┘└┴┬───┐",
            "  ▼     │   │",
            "┌─┴─┐   │   │",
            "│ b │   │   │",
            "└─┬─┘   │   │",
            "  │     │   │",
            "  └─┐┌──┘ ┌─┘",
            "    ▼▼    │",
            "  ┌─┴┴┐   │",
            "  │ c │   │",
            "  └─┬─┘   │",
            "    │     │",
            "    └─┐┌──┘",
            "      ▼▼",
            "    ┌─┴┴┐",
            "    │ d │",
            "    └───┘",
        ];
        assert_eq!(
            draw(
                &[
                    "edge from:a to:b",
                    "edge from:b to:c",
                    "edge from:a to:c",
                    "edge from:c to:d",
                    "edge from:a to:d"
                ],
                60,
                0
            ),
            want
        );
    }

    #[test]
    fn an_edge_to_itself_is_left_out() {
        // Catches an edge from a node to itself drawn, or breaking the layout. The picture is what
        // the code drew before graph was restructured.
        let want = ["┌───┐   ┌───┐", "│ b │   │ a │", "└───┘   └───┘"];
        assert_eq!(draw(&["edge from:a to:a", "node id:b"], 40, 0), want);
    }

    #[test]
    fn edges_leave_a_box_apart() {
        // Catches edges out of or into one box drawn over each other. The picture is what
        // the code drew before graph was restructured.
        let want = [
            "        ┌───┐",
            "        │ a │",
            "        └┬┬┬┘",
            "         │││",
            "  ┌──────┘│└──────┐",
            "  ▼       ▼       ▼",
            "┌─┴─┐   ┌─┴─┐   ┌─┴─┐",
            "│ b │   │ c │   │ d │",
            "└─┬─┘   └─┬─┘   └─┬─┘",
            "  │       │       │",
            "  └──────┐│┌──────┘",
            "         ▼▼▼",
            "        ┌┴┴┴┐",
            "        │ e │",
            "        └───┘",
        ];
        assert_eq!(
            draw(
                &[
                    "edge from:a to:b",
                    "edge from:a to:c",
                    "edge from:a to:d",
                    "edge from:b to:e",
                    "edge from:c to:e",
                    "edge from:d to:e"
                ],
                60,
                0
            ),
            want
        );
    }

    /// Each cell's ink: p plain, d dim, L loud.
    fn inks(f: &Figure) -> Vec<String> {
        let letter = |i: &Ink| match i {
            Ink::Plain => 'p',
            Ink::Dim => 'd',
            Ink::Loud => 'L',
        };
        f.rows
            .iter()
            .map(|r| r.iter().map(|(_, i)| letter(i)).collect())
            .collect()
    }

    #[test]
    fn a_loud_node_and_edge_are_drawn_loud_and_each_box_can_be_found() {
        // Catches a violation's box or edge drawn plain, or a box the mouse
        // cannot find. The inks and places are what the code gave before
        // graph was restructured.
        let loud = |s: &str| Input {
            loud: true,
            ..mk(s)
        };
        let marks = [
            loud("node id:a"),
            mk("node id:b"),
            loud("edge from:a to:b"),
            mk("edge from:b to:c"),
        ];
        let f = render(&marks, 40, 0);
        let want = [
            "LLLLL", "LLLLL", "LLLLL", "ppLpp", "ppLpp", "ppLpp", "ppppp", "ppppp", "ppppp",
            "ppdpp", "ppdpp", "ppdpp", "ppppp", "ppppp", "ppppp",
        ];
        let place = |y| Hit {
            x: 0,
            y,
            w: 5,
            h: 3,
            prov: vec![1],
        };
        assert_eq!(
            (inks(&f), f.hits),
            (want.map(String::from).to_vec(), vec![place(0), place(6)])
        );
    }

    #[test]
    fn a_layer_is_reordered_so_its_edges_cross_less() {
        // Catches a layer left in the order the nodes came: y before x,
        // whose edges would cross. The picture is what the code drew
        // before graph was restructured.
        let want = [
            "┌───┐   ┌───┐",
            "│ a │   │ b │",
            "└─┬─┘   └─┬─┘",
            "  │       │",
            "  │       │",
            "  ▼       ▼",
            "┌─┴─┐   ┌─┴─┐",
            "│ x │   │ y │",
            "└───┘   └───┘",
        ];
        let specs = [
            "node id:y",
            "node id:x",
            "edge from:a to:x",
            "edge from:b to:y",
        ];
        assert_eq!(draw(&specs, 40, 0), want);
    }

    #[test]
    fn a_short_label_goes_beside_its_loop() {
        // Catches the column right of the loops lost, which holds the
        // label. The picture is what the code drew before graph was
        // restructured.
        let want = [
            "┌───┐",
            "│ a │◀┐",
            "└─┬─┘ │",
            "  │   │",
            "  │   │x",
            "  ▼   │",
            "┌─┴─┐ │",
            "│ b │─┘",
            "└───┘",
            "note: 1 edge back up, round the right, to close a cycle",
        ];
        let specs = ["edge from:a to:b", "edge from:b to:a text:x"];
        assert_eq!(draw(&specs, 40, 0), want);
    }

    #[test]
    fn a_label_goes_on_its_run_when_another_is_beside_the_arrow() {
        // Catches the second place for a label lost: "one" is beside the
        // arrows into b, so "two" goes on its run. The picture is what the
        // code drew before graph was restructured.
        let want = [
            "┌───┐   ┌───┐   ┌────────────────────┐",
            "│ a │   │ c │   │ a much longer name │",
            "└─┬─┘   └─┬─┘   └──────────┬─────────┘",
            "  │       │                │",
            "  └───────┴───┬┐ two  ┌────┘",
            "              ▼▼one   ▼",
            "            ┌─┴┴┐   ┌─┴─┐",
            "            │ b │   │ e │",
            "            └───┘   └───┘",
        ];
        let specs = [
            "edge from:a to:b text:one",
            "edge from:c to:b text:two",
            "edge from:a_much_longer_name to:e",
        ];
        assert_eq!(draw(&specs, 60, 0), want);
    }
}
