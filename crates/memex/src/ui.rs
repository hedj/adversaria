//! Drawing the screen from the interface state (docs/design.md § A user
//! story, scene 1). Drawing reads the state and changes nothing, so the same
//! state always draws the same screen.

mod commands;
mod constraint;
mod foot;
mod interrogation;
mod matches;
mod notebook;
mod offers;
mod outline;
mod pages;
mod scroll;
mod worlds;
mod wrap;

use crate::app::{Active, App, Hold, Mode, PaneKind, Quasi, TakeBack, View};
use crate::marks::{MarkLine, difference, inner, item_marks, stating_rule, where_from};
use crate::outline::{Layer, Link, Outline, Run, TOP};
use crate::present::Presentation;
use crate::reason::{self, ConstraintEval, Prov, Solution, WorldEval};
use crate::view;
use constraint::ConstraintPane;
use foot::{draw_prompt, foot_lines};
use format::record::EntryKind;
use format::terms::Term;
use interrogation::interrogation;
use outline::draw_outline;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
pub use scroll::Fit;
use scroll::draw_scrolled;
use std::collections::BTreeMap;
use worlds::{Sides, counted, rule_lines, ruled};
use wrap::{Hung, Wrapped, hung, wrap};

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}
/// The ground of the cursor's line: a tint that every text colour reads
/// on, so the line keeps what its colours say.
pub fn cursor() -> Style {
    Style::default().bg(Color::Rgb(0x2c, 0x3c, 0x52))
}

/// What is selected: reversed, as a desktop program draws a selection.
pub fn selected() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}

/// A violation: beliefs that break a rule. Red, as an error is.
pub fn violation() -> Style {
    Style::default().fg(Color::Rgb(0xf0, 0x50, 0x50))
}

/// A disagreement between sources: a claim against a belief or another
/// claim. Amber, as a warning is: to look at, not yet an error.
pub fn disagreement() -> Style {
    Style::default().fg(Color::Rgb(0xf2, 0xc0, 0x3a))
}

/// A mark's line under its item: ‼ for a violation, ? for a
/// disagreement, each in its colour.
fn mark_line(indent: &str, m: &MarkLine) -> Line<'static> {
    let sign = if m.loud { "‼" } else { "?" };
    Line::styled(
        format!("{indent}  {sign} {}", m.text),
        finding_style(m.loud),
    )
}

/// A finding's style, by whether it is a violation.
fn finding_style(loud: bool) -> Style {
    if loud { violation() } else { disagreement() }
}

fn bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

/// Places grown by each of others that touches one of them, until none
/// is left that does.
fn grown(mut found: Vec<Rect>, others: &[Rect]) -> Vec<Rect> {
    while let Some(next) = next_touching(&found, others) {
        found.push(next);
    }
    found
}

/// The first of others not yet found that touches one found.
fn next_touching(found: &[Rect], others: &[Rect]) -> Option<Rect> {
    others.iter().find(|r| joins(found, r)).copied()
}

/// Whether a place, not yet found, touches one that is.
fn joins(found: &[Rect], r: &Rect) -> bool {
    !found.contains(r) && found.iter().any(|f| touching(f, r))
}

/// Whether two places touch, one on the row after the other, their
/// columns overlapping.
fn touching(a: &Rect, b: &Rect) -> bool {
    let rows = a.bottom() == b.top() || b.bottom() == a.top();
    rows && a.left() < b.right() && b.left() < a.right()
}

/// The presentations of the frame just drawn, each with the place it
/// was drawn in: where a click lands, and what it stands for; and what
/// was drawn of the inspector, which the wheel and PageDown scroll.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Presentations(pub Vec<(Rect, Presentation)>, pub Option<Fit>);

impl Presentations {
    /// What a click at (x, y) would act on, if anything.
    pub fn at(&self, x: u16, y: u16) -> Option<&Presentation> {
        self.0
            .iter()
            .find(|(r, _)| r.contains(Position { x, y }))
            .map(|(_, c)| c)
    }
    /// The places of the presentation at a cell that make one occurrence
    /// of it: the place there, and those of the same presentation that
    /// touch it, row on row (a wrapped line, a bar), not where the same
    /// object is drawn again elsewhere.
    pub fn occurrence_at(&self, x: u16, y: u16) -> Vec<Rect> {
        let Some(p) = self.at(x, y) else {
            return vec![];
        };
        let same = self.places_of(p);
        let here = same
            .iter()
            .copied()
            .filter(|r| r.contains(Position { x, y }));
        grown(here.collect(), &same)
    }

    /// Each place a presentation is drawn in.
    fn places_of(&self, p: &Presentation) -> Vec<Rect> {
        self.0
            .iter()
            .filter(|(_, q)| q == p)
            .map(|(r, _)| *r)
            .collect()
    }

    /// The clicks outside an area.
    fn outside(mut self, area: Rect) -> Presentations {
        self.0.retain(|(r, _)| r.intersection(area).is_empty());
        self
    }

    fn add(&mut self, area: Rect, r: Rect, c: Presentation) {
        let r = r.intersection(area);
        if r.width > 0 && r.height > 0 {
            self.0.push((r, c));
        }
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    draw_hits(f, app);
}

/// Draw the screen, and say where each presentation is drawn.
pub fn draw_hits(f: &mut Frame, app: &App) -> Presentations {
    let (body, foot) = draw_edges(f, app);
    let hits = draw_body(f, app, body);
    let hits = match MenuBox::open(app, &hits) {
        Some(menu) => menu.draw(f, hits),
        None => hits,
    };
    show_pointed(f, app, &hits);
    document_pointed(f, app, &hits, foot);
    hits
}

/// What the mouse does over the presentation under the pointer, on a
/// row of the foot, cut to it, as Genera's mouse documentation line
/// said (documentation_row).
fn document_pointed(f: &mut Frame, app: &App, hits: &Presentations, foot: [Rect; 2]) {
    if let Some(p) = pointed_sensitive(app, hits) {
        draw_documentation(f, app.documentation(p), documentation_row(app, foot));
    }
}

/// The foot's row for the mouse documentation: the first with nothing
/// open, where a hint would be, so the message stays; else the second,
/// the first holding what is typed.
fn documentation_row(app: &App, foot: [Rect; 2]) -> Rect {
    foot[usize::from(app.session.mode != Mode::Normal)]
}

/// A line of documentation over a row, dim, cut to it.
fn draw_documentation(f: &mut Frame, said: String, row: Rect) {
    f.render_widget(Clear, row);
    f.render_widget(Paragraph::new(Line::styled(said, dim())), row);
}

/// The presentation under the pointer, if a click on it does something
/// now: its text underlined where the pointer is, on every row of that
/// occurrence, but not where the same object is drawn elsewhere.
fn show_pointed(f: &mut Frame, app: &App, hits: &Presentations) {
    let Some((x, y)) = app.session.pointer else {
        return;
    };
    if pointed_sensitive(app, hits).is_some() {
        for area in hits.occurrence_at(x, y) {
            underline_text(f.buffer_mut(), area);
        }
    }
}

/// The presentation under the pointer, if a click on it does something
/// now.
fn pointed_sensitive<'a>(app: &App, hits: &'a Presentations) -> Option<&'a Presentation> {
    let p = app.session.pointer.and_then(|(x, y)| hits.at(x, y))?;
    app.sensitive(p).then_some(p)
}

/// Each row of an area underlined from its first character to its last.
fn underline_text(buf: &mut ratatui::buffer::Buffer, area: Rect) {
    let area = area.intersection(buf.area);
    for y in area.top()..area.bottom() {
        let row = Rect {
            y,
            height: 1,
            ..area
        };
        if let Some((a, b)) = written_span(buf, row) {
            (a..=b).for_each(|x| buf[(x, y)].modifier.insert(Modifier::UNDERLINED));
        }
    }
}

/// The first and last columns of a row of cells that hold a character.
fn written_span(buf: &ratatui::buffer::Buffer, row: Rect) -> Option<(u16, u16)> {
    let written = |x: &u16| !buf[(*x, row.y)].symbol().trim().is_empty();
    let mut xs = (row.left()..row.right()).filter(written);
    let first = xs.next()?;
    Some((first, xs.next_back().unwrap_or(first)))
}

/// A menu's entries, each its words, whether it is chosen, and the
/// command a click on it runs.
fn offered_commands(entries: Vec<(String, bool, String)>) -> Vec<(String, bool, Presentation)> {
    let entry = |(text, chosen, line)| (text, chosen, Presentation::Command(line));
    entries.into_iter().map(entry).collect()
}

/// The first cell where the cursor's item is drawn, if it is.
fn cursor_cell(app: &App, drawn: &Presentations) -> Option<(u16, u16)> {
    let id = app.cursor();
    let at = drawn
        .0
        .iter()
        .find(|(_, p)| matches!(p, Presentation::Item { id: i, .. } if *i == id));
    at.map(|(r, _)| (r.x, r.y))
}

/// What a right-click offers, as drawn at its cell: each entry's line,
/// whether it is the one chosen, and what a click on it runs.
struct MenuBox {
    title: String,
    entries: Vec<(String, bool, Presentation)>,
    at: (u16, u16),
}

impl MenuBox {
    /// The menu, if one is open, at its cell, or else at the cursor's
    /// item as drawn.
    fn open(app: &App, drawn: &Presentations) -> Option<MenuBox> {
        let Mode::Menu(m) = &app.session.mode else {
            return None;
        };
        let at = m.at.or_else(|| cursor_cell(app, drawn)).unwrap_or((0, 0));
        Some(MenuBox::of(&m.text, m.entries(), at))
    }

    /// A menu about a text, its entries, at a cell.
    fn of(text: &str, entries: Vec<(String, bool, String)>, at: (u16, u16)) -> MenuBox {
        let title = format!(" act on “{}” ", shortened(text, 40));
        let entries = offered_commands(entries);
        MenuBox { title, entries, at }
    }

    /// The menu in a box below its cell, or above it where there is no
    /// room below, inside the screen, with a cell of paper round it so
    /// the text under it does not run into its edge; its entries are
    /// clicked first.
    fn draw(&self, f: &mut Frame, under: Presentations) -> Presentations {
        let area = placed(f.area(), self.at, self.size(f.area()));
        clear_round(f, area);
        f.render_widget(self.widget(), area);
        let mut hits = self.hits(area);
        let under = under.outside(area);
        hits.0.extend(under.0);
        hits.1 = under.1;
        hits
    }

    /// The box: its entries in its border, on a ground of its own.
    fn widget(&self) -> Paragraph<'static> {
        Paragraph::new(self.lines())
            .block(self.frame())
            .style(raised())
    }

    /// The box's border, its title in the middle of the top edge.
    fn frame(&self) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .title(self.title.clone())
            .title_alignment(Alignment::Center)
    }

    /// Each entry's line, cut to 60 characters and filled to the box's
    /// width in cells, the chosen one reversed.
    fn lines(&self) -> Vec<Line<'static>> {
        let inner = self.inner_width();
        let line = |(text, chosen, _): &(String, bool, Presentation)| {
            let text = shortened(text, 62);
            let fill = " ".repeat((inner - 1).saturating_sub(cells_of(&text)));
            let shown = format!(" {text}{fill}");
            let style = if *chosen {
                reversed()
            } else {
                Style::default()
            };
            Line::styled(shown, style)
        };
        self.entries.iter().map(line).collect()
    }

    /// The width inside the border, in cells: the widest entry with a
    /// space each side, or the title with a rule and a space each side.
    fn inner_width(&self) -> usize {
        let entry =
            |(text, _, _): &(String, bool, Presentation)| cells_of(&shortened(text, 62)) + 2;
        let widest = self.entries.iter().map(entry).max().unwrap_or(0);
        widest.max(cells_of(&self.title) + 2)
    }

    /// The box's width and height, with its border, within the screen.
    fn size(&self, screen: Rect) -> (u16, u16) {
        let w = (self.inner_width() as u16 + 2).min(screen.width);
        (w, (self.entries.len() as u16 + 2).min(screen.height))
    }

    /// Where a click on each entry that fits lands: its row in the box.
    fn hits(&self, area: Rect) -> Presentations {
        let fits = area.height.saturating_sub(2) as usize;
        let each = self.entries.iter().take(fits).enumerate();
        Presentations(
            each.map(|(i, e)| (entry_row(area, i), e.2.clone()))
                .collect(),
            None,
        )
    }
}

/// Clear an area and a cell of paper round it, within the screen.
fn clear_round(f: &mut Frame, area: Rect) {
    let round = area.outer(ratatui::layout::Margin::new(1, 1));
    f.render_widget(Clear, round.intersection(f.area()));
}

/// The row of a box's entry, inside its border.
fn entry_row(area: Rect, i: usize) -> Rect {
    let inner = area.inner(ratatui::layout::Margin::new(1, 1));
    Rect {
        y: inner.y + i as u16,
        height: 1,
        ..inner
    }
}

/// A box of a size at a cell, inside the screen.
fn placed(screen: Rect, at: (u16, u16), (w, h): (u16, u16)) -> Rect {
    Rect::new(
        at.0.min(screen.width - w),
        top_of(screen.height, at.1, h),
        w,
        h,
    )
}

/// A box's top row: below its cell, or above it where there is no room
/// below, with a row between them, which the paper round the box takes,
/// so the box never covers its cell's row.
fn top_of(height: u16, y: u16, h: u16) -> u16 {
    if y + 2 + h <= height {
        y + 2
    } else {
        y.saturating_sub(h + 1)
    }
}

/// A text cut to so many characters, with an ellipsis if it was longer.
/// The cells a text takes on the screen: two for a wide character.
fn cells_of(text: &str) -> usize {
    Span::raw(text).width()
}

fn shortened(text: &str, most: usize) -> String {
    match text.chars().count() > most {
        true => text.chars().take(most - 1).chain(['…']).collect(),
        false => text.to_string(),
    }
}

/// Draw the top bar and the foot, each as many rows as its lines take
/// wrapped; the rows between them: the body's; and the foot's rows.
fn draw_edges(f: &mut Frame, app: &App) -> (Rect, [Rect; 2]) {
    let edges = Edges::of(app, f.area().width);
    let [top, body, foot @ ..] = screen_areas(f.area(), &edges);
    edges.draw(f, top, foot);
    (body, foot)
}

/// The top bar: its left (with the right beside it, when it fits), and
/// the right on a row of its own, when it does not.
struct Top {
    left: Line<'static>,
    right: Option<Line<'static>>,
}

impl Top {
    /// Its left, wrapped; under it, the right, if it has a row of its own.
    fn draw(self, f: &mut Frame, area: Rect) {
        let [left, right] = self.rows(area);
        f.render_widget(wrapped(vec![self.left]), left);
        f.render_widget(Paragraph::new(self.right.unwrap_or_default()), right);
    }

    /// The rows of an area for its left, and for its right: the last row,
    /// if the right has a row of its own.
    fn rows(&self, area: Rect) -> [Rect; 2] {
        let below = u16::from(self.right.is_some());
        Layout::vertical([Constraint::Min(1), Constraint::Length(below)]).areas(area)
    }
}

/// The lines at the screen's edges: the top bar's, and the foot's two.
struct Edges {
    top: Top,
    foot: [Line<'static>; 2],
}

impl Edges {
    fn of(app: &App, width: u16) -> Edges {
        let top = top_lines(app, width);
        Edges {
            top,
            foot: foot_lines(app),
        }
    }

    /// Draw the top bar and the foot in their rows.
    fn draw(self, f: &mut Frame, top: Rect, foot: [Rect; 2]) {
        self.top.draw(f, top);
        draw_prompt(f, self.foot, foot);
    }

    /// How many rows each takes wrapped to a width: the top bar (its left
    /// up to two, and a row for its right if that has its own), what is
    /// typed, and what the memex says (up to three each).
    fn rows(&self, width: u16) -> [u16; 3] {
        let rows =
            |l: &Line<'static>, most| wrapped_height(std::slice::from_ref(l), width).clamp(1, most);
        let top = rows(&self.top.left, 2) + u16::from(self.top.right.is_some());
        let [typed, said] = self.foot.each_ref().map(|l| rows(l, 3));
        [top, typed, said]
    }
}

/// The screen's rows: the top bar, the body, and the foot's two lines.
fn screen_areas(area: Rect, edges: &Edges) -> [Rect; 4] {
    let [top, typed, said] = edges.rows(area.width);
    let rows = [
        Constraint::Length(top),
        Constraint::Min(3),
        Constraint::Length(typed),
        Constraint::Length(said),
    ];
    Layout::vertical(rows).areas(area)
}

/// The body: the two worlds while a supposition is open, else the panes.
fn draw_body(f: &mut Frame, app: &App, area: Rect) -> Presentations {
    match &app.reason.supposition {
        Some(sup) => draw_worlds(f, app, &sup.name, area),
        None => draw_panes(f, app, area),
    }
}

/// A supposition, by its name: the actual world and the supposed one side
/// by side, row for row; or a revision's world: each change, one entry
/// each, to keep or retract (retract N) before the world is committed or
/// closed.
fn draw_worlds(f: &mut Frame, app: &App, name: &str, area: Rect) -> Presentations {
    if app.reason.in_revision() {
        return draw_presented(f, revision_lines(name, &app.revision_changes()), area);
    }
    if let Some(sides) = Sides::of(app) {
        sides.draw(f, area);
    }
    Presentations::default()
}

/// The two panes, and a quasimode's list over the left one: no click
/// lands on what its box covers.
fn draw_panes(f: &mut Frame, app: &App, area: Rect) -> Presentations {
    let [left, right] = pane_areas(area);
    let mut hits = draw_pane(f, app, 1, right);
    hits.0.extend(draw_pane(f, app, 0, left).0);
    focus_heading(f, app, [left, right]);
    hits.outside(list_box(f, app, left))
}

/// A quasimode's list over the foot of an area, if one is shown: the
/// box it takes; else none.
fn list_box(f: &mut Frame, app: &App, area: Rect) -> Rect {
    match app.session.listed() {
        Some(a) => draw_list(f, app, a, area),
        None => Rect::default(),
    }
}

/// The heading of the pane with focus, on its first row, drawn bright
/// and bold, the other's dim: which pane takes the keys, shown where it is.
fn focus_heading(f: &mut Frame, app: &App, panes: [Rect; 2]) {
    let pane = panes[app.session.view.focus.min(1)];
    let row = Rect {
        height: 1.min(pane.height),
        ..pane
    };
    let bright = Style::default()
        .remove_modifier(Modifier::DIM)
        .add_modifier(Modifier::BOLD);
    f.buffer_mut().set_style(row, bright);
}

/// The body's panes: three fifths of it, then two fifths.
fn pane_areas(area: Rect) -> [Rect; 2] {
    let halves = [Constraint::Percentage(60), Constraint::Percentage(40)];
    Layout::horizontal(halves).areas(area)
}

/// Pane i, as its kind is drawn.
fn draw_pane(f: &mut Frame, app: &App, i: usize, area: Rect) -> Presentations {
    match app.session.view.pane_kind(i) {
        PaneKind::Outline => draw_outline(f, app, i, area),
        PaneKind::Links => draw_links(f, app, area),
        PaneKind::Suggestions => draw_suggestions(f, app, area),
        PaneKind::Matches => matches::draw(f, app, area),
    }
}

/// The top bar: on the left the room, its world and the journal; on the
/// right what reasoning found. When the right does not fit beside the
/// left, it takes a row of its own.
fn top_lines(app: &App, width: u16) -> Top {
    let left = top_left(app);
    let right = top_right(app);
    let gap = (width as usize).checked_sub(left.width() + right.width() + 1);
    match gap {
        Some(gap) => Top {
            left: beside(left, gap + 1, right),
            right: None,
        },
        None => Top {
            left,
            right: Some(right.right_aligned()),
        },
    }
}

/// A line, then a gap, then another line.
fn beside(line: Line<'static>, gap: usize, right: Line<'static>) -> Line<'static> {
    let gap = Span::raw(" ".repeat(gap));
    let spans = line.spans.into_iter().chain([gap]).chain(right.spans);
    Line::from_iter(spans)
}

/// The top bar's left: the notebook's name, and its world or worlds.
fn top_left(app: &App) -> Line<'static> {
    let mut spans = vec![
        Span::styled("notebook ", dim()),
        Span::styled(app.journal().room(), bold()),
        Span::raw("   "),
    ];
    spans.extend(world_spans(app));
    Line::from(spans)
}

/// The world shown: the actual one, or both while a supposition is open.
fn world_spans(app: &App) -> Vec<Span<'static>> {
    match &app.reason.supposition {
        Some(s) => vec![
            Span::styled("worlds ", dim()),
            Span::styled("actual", bold()),
            Span::raw(" | "),
            Span::styled(format!("suppose: {}", s.name), bold()),
        ],
        None => vec![
            Span::styled("world ", dim()),
            Span::styled(app.reason.world().to_string(), bold()),
        ],
    }
}

/// The top bar's right: what reasoning found, if anything.
fn top_right(app: &App) -> Line<'static> {
    app.reason.eval[0].as_ref().map(status).unwrap_or_default()
}

/// A view's panel: its layers, each presenting itself, with its entry and
/// how many marks it gives; what was not drawn; and what a click does.
fn view_panel(app: &App, id: u64, layers: &[Layer]) -> Vec<Presented> {
    let mut lines = layers_head(layers);
    lines.extend(layers.iter().flat_map(|l| presented_layer(app, l, id)));
    lines.extend(plain(view_notes(app, id)));
    let hint = "a click on a mark goes to the item that states its fact";
    lines.extend(plain(vec![Line::raw(""), Line::styled(hint, dim())]));
    lines
}

/// The view panel's heading, and a blank line under it.
fn layers_head(layers: &[Layer]) -> Vec<Presented> {
    plain(vec![layers_heading(layers), Line::raw("")])
}

/// A layer's lines, each presenting the layer of its view.
fn presented_layer(app: &App, layer: &Layer, view: u64) -> Vec<Presented> {
    let presented = Presentation::Layer {
        entry: layer.seq,
        view,
    };
    let lines = layer_lines(app, layer).into_iter();
    lines.map(|l| (l, Some(presented.clone()))).collect()
}

/// The view panel's heading: how many layers the view has.
fn layers_heading(layers: &[Layer]) -> Line<'static> {
    let s = if layers.len() == 1 { "" } else { "S" };
    Line::styled(format!("VIEW · {} LAYER{s}", layers.len()), dim())
}

/// A layer's goal, then its entry and how many marks it gives in the
/// current world.
fn layer_lines(app: &App, layer: &Layer) -> [Line<'static>; 2] {
    let goal = Line::raw(format!("layer {} where {}", layer.mark, layer.goal));
    let n = app.reason.mark_count(layer.seq);
    let s = if n == 1 { "" } else { "s" };
    let entry = format!("  entry {} · {n} mark{s}", layer.seq);
    [goal, Line::styled(entry, dim())]
}

/// The notes of the view at an item, after a blank line, if it has any.
fn view_notes(app: &App, id: u64) -> Vec<Line<'static>> {
    let notes = view_figure(app, 0, id, 60, 12).notes;
    if notes.is_empty() {
        return vec![];
    }
    let lines = notes.into_iter().map(|n| Line::styled(n, dim()));
    std::iter::once(Line::raw("")).chain(lines).collect()
}

/// The citations of an item, shown dimmed after its text.
fn cites(outline: &Outline, id: u64) -> String {
    let (out, _) = outline.links_of(id);
    out.iter()
        .filter(|l| l.kind == "cites")
        .map(|l| format!(" [{}]", outline.text(l.to)))
        .collect()
}

/// An item as drawn in the outline: its text, then its source and what
/// it cites, dim.
fn item_spans(app: &App, id: u64) -> Vec<Span<'static>> {
    let mut spans = item_text(app, id);
    spans.extend(item_notes(&app.notebook.outline, id));
    spans
}

/// An item's text as drawn: derived text filled in; else the text as
/// typed, if it is not yet written; else its runs.
fn item_text(app: &App, id: u64) -> Vec<Span<'static>> {
    if let Some(derived) = derived_spans(app, id) {
        return derived;
    }
    let typed: String = app.text_of(id).iter().collect();
    if typed != app.notebook.outline.text(id) {
        return vec![Span::raw(typed)];
    }
    let spans = run_spans(&app.notebook.outline, id);
    match app.notebook.outline.has_meaning(id) {
        true => spans
            .into_iter()
            .map(|s| s.patch_style(read_text()))
            .collect(),
        false => spans,
    }
}

/// Text the memex has read as statements: a formal meaning to it, in a
/// colour of its own, so the user sees which text it reads.
fn read_text() -> Style {
    Style::default().fg(Color::Cyan)
}

/// A question's answer, filled in from the current world, if the item
/// asks one and it has been evaluated.
fn answer_of(app: &App, id: u64) -> Option<String> {
    let d = app.notebook.outline.derived.get(&id).filter(|d| d.under)?;
    app.reason.eval[0].as_ref()?.derived_text(id, d)
}

/// A derived item's text, filled in from the current world, if it has
/// been evaluated; not a question's answer, which is drawn under its text.
fn derived_spans(app: &App, id: u64) -> Option<Vec<Span<'static>>> {
    let d = app.notebook.outline.derived.get(&id).filter(|d| !d.under)?;
    let e = app.reason.eval[0].as_ref()?;
    let text = e.derived_text(id, d)?;
    Some(vec![Span::raw(text), Span::styled(" [derived]", dim())])
}

/// An item's runs: its own text plain, and text from other items marked
/// with where it is from.
fn run_spans(outline: &Outline, id: u64) -> Vec<Span<'static>> {
    let runs = outline.runs(id).into_iter();
    runs.flat_map(|r| run_span(outline, id, r)).collect()
}

/// A run of an item: plain if it is the item's own, else marked.
fn run_span(outline: &Outline, id: u64, run: Run) -> Vec<Span<'static>> {
    if run.origin == id {
        return vec![Span::raw(run.text)];
    }
    let from = Span::styled(format!("↖{}", outline.number(run.origin)), dim());
    let style = Style::default().add_modifier(Modifier::ITALIC | Modifier::UNDERLINED);
    vec![Span::styled(run.text, style), from]
}

/// What follows an item's text, dim: its source, and what it cites.
fn item_notes(outline: &Outline, id: u64) -> Vec<Span<'static>> {
    let source = outline.source.get(&id).map(|src| format!(" [{src}]"));
    let cites = Some(cites(outline, id)).filter(|c| !c.is_empty());
    let notes = source.into_iter().chain(cites);
    notes.map(|n| Span::styled(n, dim())).collect()
}

fn title(app: &App) -> String {
    let outline = &app.notebook.outline;
    let named = outline.names.get("title").copied();
    match named.or(app.reason.title).filter(|i| outline.contains(*i)) {
        Some(id) => format!("OUTLINE · {}", untitled(&outline.text(id)).to_uppercase()),
        None => "OUTLINE".into(),
    }
}

/// A title's text without the label that makes it the title.
fn untitled(text: &str) -> String {
    let words = text
        .split_whitespace()
        .filter(|w| !w.eq_ignore_ascii_case("#title"));
    words.collect::<Vec<_>>().join(" ")
}

/// The view at an item, drawn as world k computes it.
pub fn view_figure(app: &App, k: usize, item: u64, width: usize, height: usize) -> view::Figure {
    match view_marks(app, k, item) {
        None => view::Figure::default(),
        Some(marks) => marks.figure(width, height),
    }
}

/// What a view computes in a world: its marks, and a note for each layer
/// that cannot answer, and why.
#[derive(Default)]
struct ViewMarks {
    inputs: Vec<view::Input>,
    failed: Vec<String>,
}

impl ViewMarks {
    /// The marks drawn, then the notes of the layers that cannot answer.
    /// With no marks and no such layer, a note that there are no
    /// solutions yet.
    fn figure(&self, width: usize, height: usize) -> view::Figure {
        let mut fig = match (self.inputs.is_empty(), self.failed.is_empty()) {
            (false, _) => view::render(&self.inputs, width, height),
            (true, true) => view::Figure::noted("(no marks: no solutions yet)"),
            (true, false) => view::Figure::default(),
        };
        fig.notes.extend_from_slice(&self.failed);
        fig
    }
}

/// What the view at an item computes in world k, if the item has a view.
fn view_marks(app: &App, k: usize, item: u64) -> Option<ViewMarks> {
    let layers = app.notebook.outline.views.get(&item)?;
    let Some(eval) = app.reason.eval[k].as_ref() else {
        return Some(ViewMarks::default());
    };
    let bad = eval.violated_facts();
    let inputs = layers.iter().flat_map(|l| layer_inputs(eval, l, &bad));
    let failed = layers.iter().filter_map(|l| layer_failure(eval, l));
    let (inputs, failed) = (inputs.collect(), failed.collect());
    Some(ViewMarks { inputs, failed })
}

/// A layer's note, if it cannot answer in a world: why.
fn layer_failure(e: &WorldEval, l: &Layer) -> Option<String> {
    let why = e.nodes.failed.get(&l.seq)?;
    Some(format!("(layer {} cannot answer: {why})", l.seq))
}

/// A layer's marks in a world, each loud if it rests on a violated fact.
fn layer_inputs(
    eval: &WorldEval,
    layer: &Layer,
    violated: &std::collections::BTreeSet<u64>,
) -> Vec<view::Input> {
    let marks = eval.nodes.marks(layer.seq);
    let inputs = marks
        .iter()
        .map(|(mark, prov)| view::Input::of(mark, prov, violated));
    inputs.collect()
}

/// A figure as lines, indented; notes after it, dim.
fn figure_lines(fig: &view::Figure, indent: usize) -> Vec<Line<'static>> {
    let pad = " ".repeat(indent);
    let mut lines: Vec<Line> = fig.rows.iter().map(|row| row_line(row, &pad)).collect();
    lines.extend(
        fig.notes
            .iter()
            .map(|n| Line::styled(format!("{pad}{n}"), dim())),
    );
    lines
}

/// A row of a figure as a line: a pad, then its runs of one ink.
fn row_line(row: &[(char, view::Ink)], pad: &str) -> Line<'static> {
    let mut line = vec![Span::raw(pad.to_string())];
    line.extend(ink_spans(row));
    Line::from(line)
}

/// A row's runs of one ink, to its last mark, each in its style.
fn ink_spans(row: &[(char, view::Ink)]) -> Vec<Span<'static>> {
    let mut runs: Vec<(String, view::Ink)> = vec![];
    for (c, ink) in row.iter().map(neutral_fill) {
        match runs.last_mut() {
            Some((run, i)) if *i == ink => run.push(c),
            _ => runs.push((c.to_string(), ink)),
        }
    }
    if let Some((run, _)) = runs.last_mut() {
        run.truncate(run.trim_end().len());
    }
    runs.retain(|(run, _)| !run.is_empty());
    let spans = runs
        .into_iter()
        .map(|(run, ink)| Span::styled(run, ink_style(ink)));
    spans.collect()
}

/// A chart's fill (a bar's blocks) is drawn plain even where it rests on
/// a violation: each number may be right where only their sum breaks a
/// rule, so the bar's value, not its whole area, shows it.
fn neutral_fill(&(c, ink): &(char, view::Ink)) -> (char, view::Ink) {
    let fill = ('▁'..='█').contains(&c) || matches!(c, '▌' | '▐');
    match (fill, ink) {
        (true, view::Ink::Loud) => (c, view::Ink::Plain),
        _ => (c, ink),
    }
}

/// The style each ink is drawn in.
fn ink_style(ink: view::Ink) -> Style {
    match ink {
        view::Ink::Plain => Style::default(),
        view::Ink::Dim => dim(),
        view::Ink::Loud => violation(),
    }
}

/// The fact a mark presents: the first it rests on that an item states,
/// by its entry, with that item.
fn hit_fact(app: &App, hit: &view::Hit) -> Option<(u64, u64)> {
    hit.prov.iter().find_map(|s| {
        // A fact from the outline itself rests on the item's own entry.
        if app.notebook.outline.items.contains_key(s) {
            Some((*s, *s))
        } else {
            app.notebook.outline.fact_item.get(s).map(|i| (*s, *i))
        }
    })
}

fn inverse(kind: &str) -> String {
    match kind {
        "cites" => "cited by".into(),
        "supports" => "supported by".into(),
        "opposes" => "opposed by".into(),
        k => format!("{k} this"),
    }
}

/// The links pane: the interrogation, if one is open; else what the
/// selected item needs (right_panel).
fn draw_links(f: &mut Frame, app: &App, area: Rect) -> Presentations {
    let inner = inset(area);
    match app.inspected_claim() {
        Some(item) => draw_claimed(f, app, item, inner),
        None => draw_scrolled(f, selected_panel(app), inner, app.inspector_rows()),
    }
}

/// What the selected item needs.
fn selected_panel(app: &App) -> Vec<Presented> {
    right_panel(app, app.session.view.selected())
}

/// A line, and the object it presents, if it presents one.
type Presented = (Line<'static>, Option<Presentation>);

/// Lines that present nothing.
fn plain(lines: Vec<Line<'static>>) -> Vec<Presented> {
    lines.into_iter().map(|l| (l, None)).collect()
}

/// Lines drawn hung in an area, each that presents an object recorded
/// where it is drawn, all its rows.
fn draw_presented(f: &mut Frame, lines: Vec<Presented>, area: Rect) -> Presentations {
    let mut column = Column::new(area);
    for (line, presented) in lines {
        match presented {
            Some(p) => column.clickable(vec![line], p),
            None => column.lines.push(line),
        }
    }
    f.render_widget(Hung(column.lines), area);
    column.hits
}

/// What the selected item needs: its view's layers, the constraints its
/// marks come from, or its links.
fn right_panel(app: &App, id: u64) -> Vec<Presented> {
    if id == TOP {
        return notebook::notebook_lines(app);
    }
    if let Some(layers) = app.notebook.outline.views.get(&id) {
        return view_panel(app, id, layers);
    }
    let mut lines = match app.reason.eval[0].as_ref() {
        Some(e) if !item_marks(e, &app.notebook.outline, id).is_empty() => {
            let outline = &app.notebook.outline;
            ConstraintPane { outline, e }.of(id)
        }
        _ => links_lines(&app.notebook.outline, id),
    };
    lines.extend(plain(read_as_lines(&app.notebook.outline, id)));
    lines.extend(plain(not_read_lines(app, id)));
    lines
}

/// Whether why a text was not read is worth a row: why ACE was not read,
/// always; why a text is prose, only if it reads as a sentence (it ends in
/// a full stop, question or exclamation mark), not under a heading or a
/// name.
fn says_why(text: &str, hint: &str) -> bool {
    let sentence = text.trim_end().ends_with(['.', '?', '!']);
    !hint.starts_with("prose:") || sentence
}

/// Why an item's text was not read, if it was written in this session and
/// was not: under the heading of its number, as what it was read as is.
fn not_read_lines(app: &App, id: u64) -> Vec<Line<'static>> {
    let text = app.notebook.outline.text(id);
    let Some(why) = app.session.prose_hint(id).filter(|h| says_why(&text, h)) else {
        return vec![];
    };
    let named = format!("NOT READ · {}", app.notebook.outline.number(id));
    vec![
        Line::raw(""),
        Line::styled(named, dim()),
        Line::raw(why.to_string()),
    ]
}

/// What an item's text, as it is now, was read as, in Prolog: each
/// statement by its kind, as the reader wrote it, and its #tags.
fn read_as_lines(outline: &Outline, id: u64) -> Vec<Line<'static>> {
    let all = read_as(outline, id);
    if all.is_empty() {
        return vec![];
    }
    let named = format!("READ AS · {}", outline.number(id));
    let heading = [Line::raw(""), Line::styled(named, dim())];
    heading
        .into_iter()
        .chain(all.into_iter().map(read_as_line))
        .collect()
}

/// An item's statements, by kind, as its last reading of its text as it
/// is now wrote them, then its tags.
fn read_as(outline: &Outline, id: u64) -> Vec<(String, String)> {
    let text = outline.text(id);
    let read = outline.readings.get(&id).filter(|r| r.text == text);
    let said = read.map_or(&[][..], |r| r.said.as_slice());
    let tag = |t: String| ("tag".to_string(), format!("item_tag({id}, {t})"));
    said.iter()
        .cloned()
        .chain(crate::outline::tags(&text).into_iter().map(tag))
        .collect()
}

/// A statement's line: its kind, dim, then its Prolog.
fn read_as_line((kind, prolog): (String, String)) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{kind}  "), dim()),
        Span::raw(prolog),
    ])
}

/// Lines wrapped to the width they are drawn in.
fn wrapped(lines: Vec<Line<'static>>) -> Paragraph<'static> {
    Paragraph::new(lines).wrap(Wrap { trim: false })
}

/// A claim item in the inspector, and under it the hint of what adopts
/// and what takes an adoption back, each part a click on its command.
/// Taller than the inspector, the lines scroll, and the hint keeps its
/// last row.
fn draw_claimed(f: &mut Frame, app: &App, item: u64, inner: Rect) -> Presentations {
    let lines = claimed(app, item);
    let (above, hint) = hint_place(inner, rows_of(&lines, inner.width));
    let mut hits = draw_scrolled(f, lines, above, app.inspector_rows());
    hits.0.extend(draw_adopt_hint(f, app, hint).0);
    hits
}

/// Where a claim's lines, of so many rows, and the hint under them go in
/// an area: the hint right under the lines if they fit, else on its last
/// row, the lines scrolling above it.
fn hint_place(area: Rect, used: u16) -> (Rect, Rect) {
    match used < area.height {
        true => (area, row_below(area, used)),
        false => last_row_apart(area),
    }
}

/// An area less its last row, and that row.
fn last_row_apart(area: Rect) -> (Rect, Rect) {
    let rest = area.height.saturating_sub(1);
    let last = Rect {
        y: area.y + rest,
        height: area.height.min(1),
        ..area
    };
    (
        Rect {
            height: rest,
            ..area
        },
        last,
    )
}

/// The rows that lines take, hung at a width.
fn rows_of(lines: &[Presented], width: u16) -> u16 {
    let drawn: Vec<Line<'static>> = lines.iter().map(|(l, _)| l.clone()).collect();
    hung(&drawn, width as usize).len() as u16
}

/// A claim item's lines in the inspector: the findings that mark it, what
/// its text was read as, and its claims' questions.
fn claimed(app: &App, item: u64) -> Vec<Presented> {
    let mut lines = findings_of(app, item);
    lines.extend(plain(read_as_lines(&app.notebook.outline, item)));
    if !lines.is_empty() {
        lines.push((Line::raw(""), None));
    }
    lines.extend(interrogation(app, item));
    lines
}

/// The constraints whose findings mark an item, if any do.
fn findings_of(app: &App, item: u64) -> Vec<Presented> {
    let outline = &app.notebook.outline;
    let Some(e) = app.reason.eval[0].as_ref() else {
        return vec![];
    };
    if item_marks(e, outline, item).is_empty() {
        return vec![];
    }
    ConstraintPane { outline, e }.of(item)
}

/// The row of an area after its first rows: none high if it is outside.
fn row_below(area: Rect, used: u16) -> Rect {
    let y = area.y.saturating_add(used);
    let height = u16::from(y < area.bottom());
    Rect { y, height, ..area }
}

/// The hint "F3 adopts · F4 retracts", by the keys bound,
/// each part a click on its command.
fn draw_adopt_hint(f: &mut Frame, app: &App, row: Rect) -> Presentations {
    let adopt = hint_part(app, "adopt", "adopts");
    let parts = [adopt, hint_part(app, "retract", "retracts")];
    f.render_widget(dim_text(joined(&parts)), row);
    let mut along = Along::new(row);
    parts
        .into_iter()
        .for_each(|(text, click)| along.part(&text, click));
    along.hits
}

/// A part of a hint: the key bound to a command and what it does, and a
/// click on the command.
fn hint_part(app: &App, command: &str, does: &str) -> (String, Presentation) {
    let key = app.session.keys.key_for(command);
    (
        format!("{key} {does}"),
        Presentation::Command(command.into()),
    )
}

/// The texts of a hint's parts, " · " between them.
fn joined(parts: &[(String, Presentation)]) -> String {
    let texts: Vec<&str> = parts.iter().map(|(text, _)| text.as_str()).collect();
    texts.join(" · ")
}

/// A text drawn dim.
fn dim_text(text: String) -> Paragraph<'static> {
    wrapped(vec![Line::styled(text, dim())])
}

/// A row of a pane, part by part, each part's text a click: where the
/// next part starts, and the clicks so far.
struct Along {
    row: Rect,
    x: u16,
    hits: Presentations,
}

impl Along {
    fn new(row: Rect) -> Self {
        let hits = Presentations::default();
        Along {
            row,
            x: row.x,
            hits,
        }
    }

    /// A part's text a click, then " · " before the next part.
    fn part(&mut self, text: &str, click: Presentation) {
        let width = text.chars().count() as u16;
        let at = Rect {
            x: self.x,
            width,
            ..self.row
        };
        self.hits.add(self.row, at, click);
        self.x += width + 3;
    }
}

/// The links pane's list: the item, its links from it and into it, each
/// kind once and each link numbered as follow numbers it, and where its
/// text is shown in place.
fn links_lines(outline: &Outline, id: u64) -> Vec<Presented> {
    let heading = Line::styled(format!("LINKS OF {}", outline.number(id)), dim());
    let mut lines = plain(vec![heading, Line::raw("")]);
    lines.extend(plain(vec![Line::raw(outline.text(id)), Line::raw("")]));
    let named = outline.named_by(id);
    lines.extend(links_found(outline, id, &named.labels));
    lines.extend(mention_lines(outline, &named.mentions));
    lines
}

/// An item's links, where it is transcluded, and its pages, or that it
/// has none.
fn links_found(outline: &Outline, id: u64, labels: &[u64]) -> Vec<Presented> {
    let mut found = link_lines(outline, id);
    found.extend(transclusion_lines(outline, id));
    found.extend(pages::page_lines(outline, id, labels));
    if found.is_empty() {
        found.push((Line::styled("no links", dim()), None));
    }
    found
}

/// What an open [[link]] is offered: the names that fit, or that none
/// does.
fn completions_said(typed: &str, found: &[String]) -> String {
    match found.len() {
        0 => format!("[[ no item is named “{typed}…”; adopt fix makes one once the link is closed"),
        _ => format!("[[ {} · Tab completes", listed_names(found)),
    }
}

/// The first five names, apart by " · ", then how many more.
fn listed_names(names: &[String]) -> String {
    let first = names[..names.len().min(5)].join(" · ");
    match names.len().saturating_sub(5) {
        0 => first,
        more => format!("{first} · {more} more"),
    }
}

/// An item, presented in the outline pane, where a click puts the cursor.
fn item_presented(id: u64) -> Option<Presentation> {
    Some(Presentation::Item { pane: 0, id })
}

/// What the links pane says with no item selected.
/// An item's links, under a heading for each kind, numbered in order.
fn link_lines(outline: &Outline, id: u64) -> Vec<Presented> {
    let rows = numbered_links(outline, id);
    let groups = rows.chunk_by(|a, b| a.0 == b.0);
    groups.flat_map(kind_lines).collect()
}

/// A heading of a kind of link, and its links, each presenting the item
/// at its other end.
fn kind_lines(group: &[(String, String, u64)]) -> Vec<Presented> {
    let heading = (Line::styled(group[0].0.clone(), dim()), None);
    std::iter::once(heading)
        .chain(group.iter().map(link_line))
        .collect()
}

/// A link's line, presenting the item at its other end.
fn link_line((_, text, end): &(String, String, u64)) -> Presented {
    (
        Line::raw(text.clone()),
        Some(Presentation::Link { to: *end }),
    )
}

/// Each of an item's links: its kind's heading, the item at its other
/// end with its number for follow, and that item.
fn numbered_links(outline: &Outline, id: u64) -> Vec<(String, String, u64)> {
    let numbered = link_ends(outline, id).into_iter().enumerate();
    let line = |n: usize, end| format!("{}  {}", labelled_end(outline, end), n + 1);
    numbered
        .map(|(n, (kind, end))| (kind, line(n, end), end))
        .collect()
}

/// An item's links, each by its kind's heading and the item at its other
/// end: those from it, then those into it.
fn link_ends(outline: &Outline, id: u64) -> Vec<(String, u64)> {
    let (out, into) = outline.links_of(id);
    let outgoing = out.into_iter().map(link_out);
    outgoing.chain(into.into_iter().map(link_in)).collect()
}

/// A link from the item: "cites →", and the item it reaches.
fn link_out(l: &Link) -> (String, u64) {
    (format!("{} →", l.kind), l.to)
}

/// A link into the item: "cited by ←", and the item it comes from.
fn link_in(l: &Link) -> (String, u64) {
    (format!("{} ←", inverse(&l.kind)), l.from)
}

/// An item at a link's end: its number and its text.
fn labelled_end(outline: &Outline, id: u64) -> String {
    format!("  {}. {}", outline.number(id), outline.text(id))
}

/// Where an item's text is shown in place: each item that transcludes it,
/// under the item that holds it, each presented.
fn transclusion_lines(outline: &Outline, id: u64) -> Vec<Presented> {
    let shown_in = outline.transcluded_in(id);
    if shown_in.is_empty() {
        return vec![];
    }
    let heading = (Line::styled("transcluded in ←", dim()), None);
    let places = shown_in.into_iter().map(|t| transcluded_line(outline, t));
    std::iter::once(heading).chain(places).collect()
}

/// The items that name an item without linking to it, under a heading,
/// each a click away.
fn mention_lines(outline: &Outline, mentions: &[u64]) -> Vec<Presented> {
    if mentions.is_empty() {
        return vec![];
    }
    let heading = (Line::styled("mentioned in, not linked ←", dim()), None);
    let lines = mentions.iter().map(|m| mention_line(outline, *m));
    std::iter::once(heading).chain(lines).collect()
}

/// An item that mentions another: its address and text; a click jumps.
fn mention_line(outline: &Outline, m: u64) -> Presented {
    (
        Line::raw(labelled_end(outline, m)),
        Some(Presentation::Link { to: m }),
    )
}

/// Where an item is transcluded, presenting the item that shows it.
fn transcluded_line(outline: &Outline, t: u64) -> Presented {
    (Line::raw(transcluded_at(outline, t)), item_presented(t))
}

/// An item that transcludes another, by the item that holds it, else its
/// number.
fn transcluded_at(outline: &Outline, t: u64) -> String {
    match outline.parent_item(t) {
        Some(p) => format!("  {} › {}", outline.text(p), outline.text(t)),
        None => labelled_end(outline, t),
    }
}

/// The matches of a Leap, or the commands a Command may be, in a box at the
/// foot of the outline pane.
fn draw_list(f: &mut Frame, app: &App, a: &Active, area: Rect) -> Rect {
    let lines = list_shown(app, a, area);
    let r = foot_box(area, &lines);
    f.render_widget(Clear, r);
    f.render_widget(boxed(lines), r);
    r
}

/// Lines wrapped in a box with borders all round.
fn boxed(lines: Vec<Line<'static>>) -> Paragraph<'static> {
    wrapped(lines).block(Block::default().borders(Borders::ALL))
}

/// What a quasimode's list shows in its box at the foot of an area: as
/// much of the list as fits.
fn list_shown(app: &App, a: &Active, area: Rect) -> Vec<Line<'static>> {
    fitted(list_lines(app, a, list_inside(area)), area)
}

/// The width and height inside a list's box at the foot of an area: less
/// its borders, and a column for the pane's border.
fn list_inside(area: Rect) -> (u16, u16) {
    (area.width.saturating_sub(3), area.height.saturating_sub(2))
}

/// As many of the lines as fit inside a list's box at the foot of an area
/// once wrapped; if some do not, the last row says how many more there
/// are.
fn fitted(lines: Vec<Line<'static>>, area: Rect) -> Vec<Line<'static>> {
    let inside = list_inside(area);
    let total = lines.len();
    let mut kept: Vec<Line<'static>> = Vec::new();
    for line in lines {
        kept.push(line);
        if !fits(&kept, inside, total - kept.len()) {
            kept.pop();
            break;
        }
    }
    let more = total - kept.len();
    if more > 0 {
        kept.push(Line::styled(
            format!("… and {more} more: type more to narrow the list"),
            dim(),
        ));
    }
    kept
}

/// Whether lines fit inside a box once wrapped, with a row to spare for
/// saying how many more there are, when there are more.
fn fits(lines: &[Line<'static>], (width, height): (u16, u16), more: usize) -> bool {
    let room = height.saturating_sub(u16::from(more > 0));
    wrapped_height(lines, width) <= room
}

/// A box at the foot of an area, as high as its lines wrapped and its
/// borders need, at most the area, and a column short of it, for the
/// pane's border.
fn foot_box(area: Rect, lines: &[Line<'static>]) -> Rect {
    let rows = wrapped_height(lines, list_inside(area).0);
    let height = (rows + 2).min(area.height);
    let y = area.y + area.height - height;
    let width = area.width.saturating_sub(1);
    Rect {
        y,
        width,
        height,
        ..area
    }
}

/// What a quasimode lists: the commands a Command may be, or a Leap's
/// matches.
fn list_lines(app: &App, a: &Active, inside: (u16, u16)) -> Vec<Line<'static>> {
    match a.q() {
        Quasi::Command if a.sentence().is_some() => suppose_lines(a),
        Quasi::Command | Quasi::Suppose => commands::command_lines(app, &a.input, inside),
        Quasi::LeapDown | Quasi::LeapUp => leap_lines(app, a),
    }
}

/// What may come next in Suppose, listed with help; its first line, the
/// heading, dim.
fn suppose_lines(a: &Active) -> Vec<Line<'static>> {
    let listed = a.hint().into_iter().flat_map(|h| h.listing.iter().cloned());
    let lines = listed.enumerate().map(|(k, l)| {
        if k == 0 {
            Line::styled(l, dim())
        } else {
            Line::raw(l)
        }
    });
    lines.collect()
}

/// A Leap's matches: how many, then the first five, or every one while
/// listing, the one it goes to first reversed.
fn leap_lines(app: &App, a: &Active) -> Vec<Line<'static>> {
    let matches = app.leap_found(a);
    let mut lines = vec![leap_heading(&matches)];
    let shown = if a.listing {
        matches.len()
    } else {
        matches.len().min(5)
    };
    lines.extend(match_lines(app, a, &matches[..shown]));
    lines
}

/// How many matches a Leap has.
fn leap_heading(matches: &[u64]) -> Line<'static> {
    Line::styled(format!("LEAP · {} MATCHES", matches.len()), dim())
}

/// A Leap's matches shown, the one it goes to first reversed.
fn match_lines(app: &App, a: &Active, ids: &[u64]) -> Vec<Line<'static>> {
    let outline = &app.notebook.outline;
    let lines = ids.iter().map(|id| match_line(outline, *id, &a.input));
    let first = |(k, l): (usize, Line<'static>)| if k == 0 { l.style(reversed()) } else { l };
    lines.enumerate().map(first).collect()
}

/// The ground of a box drawn over the screen, a step lighter than the
/// paper, so it stands apart from what is under it.
fn raised() -> Style {
    Style::default().bg(Color::Rgb(0x34, 0x34, 0x3c))
}

fn reversed() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}

/// A match of a Leap: where it is, then its text with the match bold.
fn match_line(outline: &Outline, id: u64, pattern: &str) -> Line<'static> {
    let mut spans = vec![Span::raw(trail(outline, id))];
    spans.extend(text_spans(outline.text(id), pattern));
    Line::from(spans)
}

/// A text, with where a pattern is in it bold, if it can say.
fn text_spans(text: String, pattern: &str) -> Vec<Span<'static>> {
    match match_at(&text, pattern) {
        Some((i, j)) => bold_between(&text, i, j).to_vec(),
        None => vec![Span::raw(text)],
    }
}

/// Where an item is: its top-level item's number, then the text of each
/// item above it, from the top.
fn trail(outline: &Outline, id: u64) -> String {
    let mut above: Vec<u64> =
        std::iter::successors(outline.parent_item(id), |p| outline.parent_item(*p)).collect();
    above.reverse();
    let number = outline.number(above.first().copied().unwrap_or(id));
    let names: String = above
        .iter()
        .map(|p| format!("{} › ", outline.text(*p)))
        .collect();
    format!("{number}. {names}")
}

/// Where a pattern is in a text, ignoring case: the first stretch of the
/// text, between its own characters, whose small form is the pattern's.
/// (Offsets in the small form are not the text's: "İ" grows, "K" shrinks.)
/// A small form has no fewer characters, so a match has no more than the
/// pattern.
fn match_at(text: &str, pattern: &str) -> Option<(usize, usize)> {
    let starts = text.char_indices().map(|(i, _)| i);
    starts
        .into_iter()
        .find_map(|i| Some((i, match_end(text, i, pattern)?)))
}

/// The end of a match of a pattern, ignoring case, that starts at a place
/// in a text, if one does.
fn match_end(text: &str, i: usize, pattern: &str) -> Option<usize> {
    let small = pattern.to_lowercase();
    let mut ends = ends_from(text, i).take(pattern.chars().count());
    ends.find(|&j| text[i..j].to_lowercase() == small)
}

/// The ends of the stretches of a text from a place, each one character
/// longer than the last.
fn ends_from(text: &str, i: usize) -> impl Iterator<Item = usize> + '_ {
    let chars = text[i..].char_indices();
    chars.map(move |(k, c)| i + k + c.len_utf8())
}

/// A text with the part between two places bold.
fn bold_between(text: &str, i: usize, j: usize) -> [Span<'static>; 3] {
    let part = |a: usize, b: usize| text[a..b].to_string();
    [
        Span::raw(part(0, i)),
        Span::styled(part(i, j), bold()),
        Span::raw(part(j, text.len())),
    ]
}

fn place(outline: &Outline, seq: u64) -> String {
    match outline.fact_item.get(&seq) {
        Some(i) => outline.number(*i),
        None => format!("entry {seq}"),
    }
}

/// What the top bar says of the current world's constraints.
fn status(e: &WorldEval) -> Line<'static> {
    let v: usize = e.constraints.iter().map(|c| c.violations.len()).sum();
    let d: usize = e.constraints.iter().map(|c| c.disagreements.len()).sum();
    let counts = [
        (
            v,
            ["constraint violation", "constraint violations"],
            violation(),
        ),
        (
            d,
            [
                "disagreement between sources",
                "disagreements between sources",
            ],
            disagreement(),
        ),
    ];
    let shown = counts.into_iter().filter(|(n, ..)| *n > 0);
    let spans = shown.map(count_span);
    // A separator between each two.
    Line::from_iter(spans.flat_map(|s| [Span::styled(" · ", dim()), s]).skip(1))
}

/// A count of findings of a kind, in its colour.
fn count_span((n, names, style): (usize, [&str; 2], Style)) -> Span<'static> {
    Span::styled(counted_finding(n, names), style)
}

/// How many findings of a kind: "1 constraint violation", "2
/// disagreements between sources".
fn counted_finding(n: usize, [one, many]: [&str; 2]) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Fading hint: while the cursor is on an item with marks, how to try the
/// other reading, on one line, with no count of its showings; it stops
/// once Suppose has been used four times (docs/design.md).
fn hint(app: &App) -> Option<(String, String)> {
    let e = app.reason.eval[0].as_ref()?;
    if item_marks(e, &app.notebook.outline, app.cursor()).is_empty() {
        return None;
    }
    let used = app
        .journal()
        .entries()
        .iter()
        .filter(|x| x.kind() == EntryKind::Suppose)
        .count();
    if used >= 4 {
        return None;
    }
    let key = app.session.keys.quasi_key(Quasi::Suppose);
    let try_it = format!(
        "Try a what-if: tap {key} to suppose a statement, type it, and press Enter. The actual world stays as it is."
    );
    Some((try_it, String::new()))
}

/// While a supposition is open, how to leave it: a revision's pane says
/// its own.
fn leaving_hint(app: &App) -> Option<String> {
    app.reason.supposition.as_ref()?;
    let leave = "dismiss world returns to the actual world · adopt world makes this one actual";
    (!app.reason.in_revision()).then(|| leave.to_string())
}

/// A pane's area less a margin of two on the left, where its text goes.
fn inset(area: Rect) -> Rect {
    let width = area.width.saturating_sub(2);
    Rect {
        x: area.x + 2,
        width,
        ..area
    }
}

/// The suggestion pane: few in view, ranked by evidence, each with why it
/// was offered. It never takes the keyboard: accept or dismiss by command.
fn draw_suggestions(f: &mut Frame, app: &App, area: Rect) -> Presentations {
    draw_presented(f, suggestion_lines(app), inset(area))
}

/// The suggestion pane's lines: how many are shown of all, each shown
/// one in an offer's rows, and how to act on them.
fn suggestion_lines(app: &App) -> Vec<Presented> {
    let all = app.reason.suggestions.list.len();
    let shown = app.suggestions_open();
    let heading = format!("SUGGESTIONS · {} OF {all}", shown.len());
    let mut lines = plain(vec![Line::styled(heading, dim()), Line::raw("")]);
    lines.extend(shown.iter().flat_map(|o| offers::offer_row(o, None)));
    if shown.is_empty() {
        lines.push((Line::styled("nothing to suggest", dim()), None));
    }
    let hint = "adopt N adopts · dismiss N dismisses";
    lines.extend(plain(vec![Line::raw(""), Line::styled(hint, dim())]));
    lines
}

/// The lines of a pane, wrapped in its area, and what a click on them
/// acts on.
struct Column {
    lines: Vec<Line<'static>>,
    area: Rect,
    hits: Presentations,
}

impl Column {
    fn new(area: Rect) -> Self {
        Column {
            lines: vec![],
            area,
            hits: Presentations::default(),
        }
    }

    /// Add lines that a click on any of them, wrapped lines too, acts on.
    fn clickable(&mut self, block: Vec<Line<'static>>, click: Presentation) {
        let rows = self.rows_of(&block);
        self.hits.add(self.area, rows, click);
        self.lines.extend(block);
    }

    /// The rows that lines would take after those already added.
    fn rows_of(&self, block: &[Line<'static>]) -> Rect {
        let width = self.area.width as usize;
        let top = hung(&self.lines, width).len() as u16;
        let height = hung(block, width).len() as u16;
        Rect {
            y: self.area.y.saturating_add(top),
            height,
            ..self.area
        }
    }
}

/// How many rows lines take, wrapped to a width.
fn wrapped_height(lines: &[Line<'static>], width: u16) -> u16 {
    let p = Paragraph::new(lines.to_vec()).wrap(Wrap { trim: false });
    p.line_count(width) as u16
}

/// The lines of a revision's panel: its name, each change in an offer's
/// rows, and how to keep or undo them.
fn revision_lines(name: &str, changes: &[(u64, String, bool)]) -> Vec<Presented> {
    let heading = Line::styled(format!("WORLD · {}", name.to_uppercase()), dim());
    let mut lines = plain(vec![heading, Line::raw("")]);
    lines.extend(changes.iter().flat_map(|c| offers::change_rows(name, c)));
    if changes.is_empty() {
        lines.push((Line::styled("no changes", dim()), None));
    }
    let how = "dismiss change N rejects one change · adopt world keeps the rest · dismiss world drops them all";
    lines.extend(plain(vec![Line::raw(""), Line::styled(how, dim())]));
    lines
}

/// The keys an empty notebook names, each by the key configured for it.
fn first_steps_said(app: &App) -> [String; 6] {
    [
        "Enter makes a new item · Tab puts it under the one above".to_string(),
        command_step(app),
        leap_step(app),
        "right-click anything for what can act on it".into(),
        undo_step(app),
        "docs/introduction.md says what this is for · docs/tutorial.md is a first session".into(),
    ]
}

/// How to run a command, and the key that lists them.
fn command_step(app: &App) -> String {
    let command = app.session.keys.quasi_key(Quasi::Command);
    let palette = app.session.keys.key_for("palette");
    format!("tap {command} (Command), type a command, press Enter to run it · {palette} lists them")
}

/// How to leap to an item, and back.
fn leap_step(app: &App) -> String {
    let keys = &app.session.keys;
    let (leap, back) = (
        keys.quasi_key(Quasi::LeapDown),
        keys.quasi_key(Quasi::LeapUp),
    );
    format!("hold {leap} (Leap) and type, to jump to an item with that text · {back} leaps up")
}

/// How to undo a mistake.
fn undo_step(app: &App) -> String {
    let command = app.session.keys.quasi_key(Quasi::Command);
    format!("a mistake? tap {command}, type undo, press Enter · nothing is ever lost")
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_menu_of_wide_text_is_as_wide_as_its_cells() {
        // Catches the menu box sized by characters: a title quoting wide
        // text was cut, and an entry ran past the border (the code review,
        // G-B5).
        let menu = MenuBox {
            title: " act on “東京大学” ".into(),
            entries: vec![(
                "go 東京大学".into(),
                false,
                Presentation::Command("go".into()),
            )],
            at: (0, 0),
        };
        assert_eq!(
            menu.inner_width(),
            19 + 2,
            "the title: 15 characters, 19 cells"
        );
        let line = &menu.lines()[0];
        assert_eq!(line.width(), menu.inner_width(), "{line:?}");
    }

    #[test]
    fn a_match_is_found_between_the_texts_own_letters() {
        // Catches the bold put on the wrong letters where small forms
        // change byte lengths: in "İİabcK" (a Kelvin sign last), "a" was
        // found at "c" (the code review, G-B3).
        let text = "İİabc\u{212A}";
        let (i, j) = match_at(text, "a").unwrap();
        assert_eq!(&text[i..j], "a");
        let (i, j) = match_at("Buran ENERGIA", "energia").unwrap();
        assert_eq!(&"Buran ENERGIA"[i..j], "ENERGIA");
        assert_eq!(match_at("Buran", ""), None);
        assert_eq!(match_at("Buran", "x"), None);
    }

    use view::Ink;

    fn figure(row: &str, ink: Ink) -> view::Figure {
        let rows = vec![row.chars().map(|c| (c, ink)).collect()];
        view::Figure {
            rows,
            hits: vec![],
            notes: vec!["a note".into()],
        }
    }

    fn texts(line: &Line) -> Vec<String> {
        line.spans.iter().map(|s| s.content.to_string()).collect()
    }

    #[test]
    fn a_figure_is_set_in_by_its_indent_rows_and_notes_alike() {
        let lines = figure_lines(&figure("ab", Ink::Plain), 3);
        assert_eq!(texts(&lines[0]), ["   ", "ab"]);
        assert_eq!(texts(&lines[1]), ["   a note"]);
    }

    #[test]
    fn a_row_ends_at_its_last_mark() {
        let lines = figure_lines(&figure("ab  ", Ink::Plain), 0);
        assert_eq!(texts(&lines[0]), ["", "ab"]);
    }

    #[test]
    fn a_row_is_split_where_its_ink_changes() {
        let mut fig = figure("ab", Ink::Plain);
        fig.rows[0][1].1 = Ink::Dim;
        assert_eq!(texts(&figure_lines(&fig, 0)[0]), ["", "a", "b"]);
    }

    #[test]
    fn a_row_of_blanks_is_its_pad_alone() {
        let lines = figure_lines(&figure("  ", Ink::Dim), 1);
        assert_eq!(texts(&lines[0]), [" "]);
    }

    #[test]
    fn each_ink_has_its_style() {
        let style = |ink| figure_lines(&figure("a", ink), 0)[0].spans[1].style;
        assert_eq!(style(Ink::Dim), dim());
        assert_eq!(style(Ink::Loud), violation());
        assert_eq!(style(Ink::Plain), Style::default());
    }
}
