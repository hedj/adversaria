//! Drawing the screen from the interface state (docs/design.md § A user
//! story, scene 1). Drawing reads the state and changes nothing, so the same
//! state always draws the same screen.

mod commands;
mod matches;
mod notebook;
mod offers;
mod pages;
mod wrap;

use crate::app::{Active, App, Hold, Mode, PaneKind, Quasi, View};
use crate::outline::{Layer, Link, Outline, Run, TOP};
use crate::present::Presentation;
use crate::reason::{self, ConstraintEval, Finding, Prov, Solution, Sourced, WorldEval};
use crate::view;
use format::terms::Term;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
/// was drawn in: where a click lands, and what it stands for.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Presentations(pub Vec<(Rect, Presentation)>);

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
        hits.0.extend(under.outside(area).0);
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

/// An outline pane: its frame and title, then every line of the outline
/// near the cursor, and the window onto them that keeps the cursor's line
/// in view.
fn draw_outline(f: &mut Frame, app: &App, pane: usize, area: Rect) -> Presentations {
    let [head, rows] = outline_frame(f, app, pane, area);
    let outline = OutlinePane::new(app, pane, rows);
    let mut hits = outline.drawn().show(f, &outline);
    hits.0.push((head, Presentation::Notebook { pane }));
    hits
}

/// An outline pane's frame, ruled off from the pane on its right, and its
/// title, which presents the notebook: the title's area, and the area
/// left for its rows.
fn outline_frame(f: &mut Frame, app: &App, pane: usize, area: Rect) -> [Rect; 2] {
    let inner = draw_frame(f, pane, area);
    let [head, rows] = title_and_rows(inner);
    f.render_widget(dim_text(title(app)), head);
    [Rect { height: 1, ..head }, rows]
}

/// A pane's frame, the first ruled off from the second: the area inside.
fn draw_frame(f: &mut Frame, pane: usize, area: Rect) -> Rect {
    let block = ruled(pane);
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

/// A pane's two lines of title, and the rows below.
fn title_and_rows(area: Rect) -> [Rect; 2] {
    Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(area)
}

/// An outline pane being drawn: the memex, the pane, and the area of its
/// rows.
struct OutlinePane<'a> {
    app: &'a App,
    pane: usize,
    rows: Rect,
    /// The items selected, if the pane has focus (crate::selection).
    selected: Vec<u64>,
}

impl<'a> OutlinePane<'a> {
    /// A pane of the outline, on rows of the screen; the pane with focus
    /// shows the items selected.
    fn new(app: &'a App, pane: usize, rows: Rect) -> OutlinePane<'a> {
        let selected = match pane == app.session.view.focus {
            true => app.selected_items(),
            false => vec![],
        };
        OutlinePane {
            app,
            pane,
            rows,
            selected,
        }
    }

    fn outline(&self) -> &'a Outline {
        &self.app.notebook.outline
    }

    fn view(&self) -> &'a View {
        &self.app.session.view
    }

    /// The item at the pane's cursor.
    fn cursor(&self) -> u64 {
        self.view().panes[self.pane].cursor
    }

    fn has_focus(&self) -> bool {
        self.view().focus == self.pane
    }

    /// The lines of the items within a screen's height of the cursor's:
    /// each item takes at least a line, so no other can be in the window,
    /// and a long outline is not drawn whole on every keystroke.
    fn drawn(&self) -> Drawn {
        let rows = self.app.visible();
        let mut drawn = self.drawn_near(&rows);
        if rows.is_empty() {
            drawn.push_lines(self.first_steps(), 2);
        }
        drawn
    }

    /// What an empty notebook shows: the first thing to do, plainly, and
    /// the keys to know, dim, each key first.
    fn first_steps(&self) -> Vec<Line<'static>> {
        let steps = first_steps_said(self.app);
        let step = |s: String| Line::styled(format!("  {s}"), dim());
        std::iter::once(Line::raw("Type to write the first item."))
            .chain([Line::raw("")])
            .chain(steps.into_iter().map(step))
            .collect()
    }

    /// Where the rows within a screen's height of the cursor's row lie.
    fn near(&self, rows: &[(u64, usize)]) -> std::ops::Range<usize> {
        near_range(rows, self.cursor(), self.rows.height as usize)
    }

    /// The items near the cursor's, drawn, with how many lie before and
    /// after them.
    fn drawn_near(&self, rows: &[(u64, usize)]) -> Drawn {
        let range = self.near(rows);
        let mut drawn = Drawn::around(&range, rows.len());
        for (id, depth) in &rows[range] {
            self.item(&mut drawn, *id, *depth);
        }
        drawn
    }

    /// An item's lines: the item, a click on all of it; its marks; and its
    /// view, if it is one.
    fn item(&self, drawn: &mut Drawn, id: u64, depth: usize) {
        drawn.starts.push(drawn.lines.len());
        if id == self.cursor() {
            drawn.cursor = Some((drawn.lines.len(), depth));
        }
        let line = (drawn.lines.len(), 0, usize::MAX);
        drawn.places.push(Place::of(line, self.click(id)));
        drawn.push(self.item_line(id, depth), self.head_width(id, depth));
        let set_in = 2 * depth + 4;
        drawn.push_lines(self.answer_line(id, depth), set_in + 2);
        drawn.push_lines(self.prose_line(id, depth), set_in);
        drawn.push_lines(self.unmade_line(id, depth), set_in);
        drawn.push_lines(self.completion_line(id, depth), set_in);
        self.push_marks(drawn, id, depth);
        if self.outline().views.contains_key(&id) {
            self.view_lines(drawn, id, 2 * depth + 2);
        }
    }

    /// An item's line: its place in the tree, its text, how many items a
    /// fold hides; on the cursor's ground at the cursor.
    fn item_line(&self, id: u64, depth: usize) -> Line<'static> {
        let at_cursor = id == self.cursor();
        let mut spans = self.item_head(id, depth);
        spans.extend(self.text_spans(id));
        spans.extend(self.folded_count(id));
        let line = Line::from(spans);
        if at_cursor {
            line.style(cursor())
        } else {
            line
        }
    }

    /// An item's text as drawn: with what is selected reversed, and dim if
    /// it is cut and not yet pasted (crate::habits).
    fn text_spans(&self, id: u64) -> Vec<Span<'static>> {
        let spans = self
            .selected_spans(id)
            .unwrap_or_else(|| item_spans(self.app, id));
        match self.app.cut_items().contains(&id) {
            true => spans.into_iter().map(|s| s.patch_style(dim())).collect(),
            false => spans,
        }
    }

    /// An item's text with what is selected in it reversed, if anything
    /// is: all of it, among items selected; or characters of the
    /// cursor's, drawn as typed, as text being edited is.
    fn selected_spans(&self, id: u64) -> Option<Vec<Span<'static>>> {
        if self.selected.contains(&id) {
            let spans = item_spans(self.app, id).into_iter();
            return Some(spans.map(|s| s.patch_style(selected())).collect());
        }
        let at = (id == self.cursor() && self.has_focus()).then_some(id)?;
        let range = self.app.selected_range()?;
        let text = self.app.text_of(at);
        let part = |r: std::ops::Range<usize>| text[r].iter().collect::<String>();
        let picked = Span::styled(part(range.clone()), selected());
        let after = part(range.end..text.len());
        Some(vec![
            Span::raw(part(0..range.start)),
            picked,
            Span::raw(after),
        ])
    }

    /// How many cells an item's head takes: the column its text starts at,
    /// under which its wrapped rows start.
    fn head_width(&self, id: u64, depth: usize) -> usize {
        Line::from(self.item_head(id, depth)).width()
    }

    /// An item's indent, its mark of the tree and its address, as the
    /// foot, the menus and the commands quote it ("1.3").
    fn item_head(&self, id: u64, depth: usize) -> Vec<Span<'static>> {
        let indent = Span::raw("  ".repeat(depth));
        let address = Span::raw(format!("{}. ", self.outline().number(id)));
        vec![indent, self.bullet(id), address]
    }

    /// An item's bullet: its tree mark, in its style.
    fn bullet(&self, id: u64) -> Span<'static> {
        Span::styled(format!("{} ", self.tree_mark(id)), self.bullet_style(id))
    }

    /// The style of an item's bullet: dim, or, for an item that states
    /// a rule with findings, the colour of the worst.
    fn bullet_style(&self, id: u64) -> Style {
        let found = self.rule_stated(id);
        match found.map(|c| (c.violations.is_empty(), c.disagreements.is_empty())) {
            Some((false, _)) => violation(),
            Some((true, false)) => disagreement(),
            _ => dim(),
        }
    }

    /// The rule an item states, as reasoning found it: the one named by
    /// its text.
    fn rule_stated(&self, id: u64) -> Option<&ConstraintEval> {
        let text = self.outline().text(id);
        let e = self.app.reason.eval[0].as_ref()?;
        e.constraint_named(text.trim_end_matches('.'))
    }

    /// • for an item with none under it, ▶ for a folded one, ▼ for one
    /// open.
    fn tree_mark(&self, id: u64) -> &'static str {
        let kids = self.outline().children(id).len();
        match (kids, self.view().folded.contains(&id)) {
            (0, _) => "•",
            (_, true) => "▶",
            _ => "▼",
        }
    }

    /// How many items a folded item hides.
    fn folded_count(&self, id: u64) -> Option<Span<'static>> {
        let count = || format!(" {}", self.outline().descendants(id));
        let folded = self.view().folded.contains(&id);
        folded.then(|| Span::styled(count(), dim()))
    }

    /// Under a question, its answer in the current world, once evaluated.
    fn answer_line(&self, id: u64, depth: usize) -> Option<Line<'static>> {
        let answer = answer_of(self.app, id)?;
        let indent = Span::raw("  ".repeat(depth + 2));
        Some(Line::from(vec![
            indent,
            Span::styled("→ ", dim()),
            Span::raw(answer),
        ]))
    }

    /// Under the item at the cursor, why its text was not read, if it was
    /// written in this session and was not: a quiet hint, nowhere else.
    fn prose_line(&self, id: u64, depth: usize) -> Option<Line<'static>> {
        let hint = self.hint_here(id).filter(|h| self.says_why(id, h))?;
        let hint = match self.app.session.prose_repair(id) {
            Some(mended) => format!("not read: did you mean “{mended}”? adopt fix mends it"),
            None => hint.to_string(),
        };
        let indent = "  ".repeat(depth + 1);
        Some(Line::styled(format!("{indent}  {hint}"), dim()))
    }

    /// Whether a hint says something worth the row: why ACE was not read,
    /// always; why a text is prose, only if it reads as a sentence (it
    /// ends in a full stop, question or exclamation mark), not under a
    /// heading or a name.
    fn says_why(&self, id: u64, hint: &str) -> bool {
        let text = self.outline().text(id);
        let sentence = text.trim_end().ends_with(['.', '?', '!']);
        !hint.starts_with("prose:") || sentence
    }

    /// Under the item at the cursor, its [[links]] that name no item yet,
    /// and how to make them.
    fn unmade_line(&self, id: u64, depth: usize) -> Option<Line<'static>> {
        let names = self.unmade_here(id)?;
        let indent = "  ".repeat(depth + 1);
        let hint = format!(
            "{indent}  not yet items: {} · adopt fix makes them",
            names.join(", ")
        );
        Some(Line::styled(hint, dim()))
    }

    /// Under the item at the cursor, while its text cursor is in an open
    /// `[[`, the names that fit what is typed, and that Tab completes.
    fn completion_line(&self, id: u64, depth: usize) -> Option<Line<'static>> {
        let here = id == self.cursor() && self.has_focus();
        let (typed, found) = self.app.link_completions().filter(|_| here)?;
        let indent = "  ".repeat(depth + 1);
        let said = completions_said(&typed, &found);
        Some(Line::styled(format!("{indent}  {said}"), dim()))
    }

    /// The [[links]] of the item at the cursor that name no item, while
    /// its text cursor is in one of them (docs/concepts-seven.md, decision
    /// 3): a link to no item is a page of its own, so the hint is for a
    /// typo, seen as it is typed.
    fn unmade_here(&self, id: u64) -> Option<Vec<String>> {
        let here = id == self.cursor() && self.has_focus();
        let link = self.app.link_here().filter(|_| here)?;
        let names = self.outline().unresolved(id);
        names.contains(&link).then_some(names)
    }

    /// The hint of an item, if it is the item at the cursor.
    fn hint_here(&self, id: u64) -> Option<&str> {
        let here = id == self.cursor();
        self.app.session.prose_hint(id).filter(|_| here)
    }

    /// An item's marks, each under it, set in.
    fn push_marks(&self, drawn: &mut Drawn, id: u64, depth: usize) {
        let hang = 2 * depth + 6;
        for (line, presented) in self.mark_lines(id, depth) {
            drawn.push_presenting(line, hang, presented);
        }
    }

    /// An item's marks, loud or not, each with what it presents.
    fn mark_lines(&self, id: u64, depth: usize) -> Vec<(Line<'static>, Option<Presentation>)> {
        let indent = "  ".repeat(depth + 1);
        let marks = self.marks_of(id).into_iter();
        marks
            .map(|m| (mark_line(&indent, &m), self.mark_presented(&m, id)))
            .collect()
    }

    /// What a mark under an item presents: the constraint, if it is a
    /// finding of one.
    fn mark_presented(&self, m: &MarkLine, under: u64) -> Option<Presentation> {
        m.constraint.map(|c| self.constraint_presented(c, under))
    }

    /// A constraint, presented in this pane by a mark under an item, by
    /// its entry, with the item whose text states it.
    fn constraint_presented(&self, entry: u64, under: u64) -> Presentation {
        let item = self.constraint_item(entry);
        Presentation::Constraint {
            pane: self.pane,
            entry,
            item,
            under: Some(under),
        }
    }

    /// The item whose text states a constraint, by its entry, if one does.
    fn constraint_item(&self, entry: u64) -> Option<u64> {
        let eval = self.app.reason.eval[0].as_ref()?;
        stating_rule(self.outline(), &eval.constraint_of(entry)?.constraint.name)
    }

    /// An item's marks: each finding on its facts, then, if its inputs
    /// changed since it was made, which.
    fn marks_of(&self, id: u64) -> Vec<MarkLine> {
        let found = self.app.reason.eval[0].as_ref();
        let mut marks = found
            .map(|e| item_marks(e, self.outline(), id))
            .unwrap_or_default();
        marks.extend(self.stale_mark(id));
        marks
    }

    /// The loud mark of an item whose input files changed since it was
    /// made: which.
    fn stale_mark(&self, id: u64) -> Option<MarkLine> {
        let files = self.app.session.stale().get(&id)?.join(", ");
        let text = format!("input changed since: {files}");
        Some(MarkLine {
            loud: true,
            text,
            constraint: None,
        })
    }

    /// A view, drawn under its item, set in; a click on a mark goes to the
    /// item that states its fact.
    fn view_lines(&self, drawn: &mut Drawn, id: u64, indent: usize) {
        let fig = self.figure(id, indent);
        let places = self.view_places(&fig, (drawn.lines.len(), indent));
        drawn.push_lines(figure_lines(&fig, indent), indent);
        drawn.places.extend(places);
    }

    /// A view's figure, as wide as the pane leaves it, set in.
    fn figure(&self, id: u64, indent: usize) -> view::Figure {
        let width = (self.rows.width as usize).saturating_sub(indent + 1);
        view_figure(self.app, 0, id, width, 12)
    }

    /// The places of a view's marks, for a figure that starts at a line
    /// and a column.
    fn view_places(&self, fig: &view::Figure, at: (usize, usize)) -> Vec<Place> {
        let places = fig.hits.iter().filter_map(|hit| self.hit_places(hit, at));
        places.flatten().collect()
    }

    /// A mark of a view, presenting its fact on each of its lines, if an
    /// item states the fact.
    fn hit_places(&self, hit: &view::Hit, at: (usize, usize)) -> Option<Vec<Place>> {
        let click = self.fact_presented(hit)?;
        let lines = hit.area.lines_at(at).into_iter();
        Some(lines.map(|l| Place::of(l, click.clone())).collect())
    }

    /// The fact a mark of a view presents, in this pane, if an item
    /// states it.
    fn fact_presented(&self, hit: &view::Hit) -> Option<Presentation> {
        let (entry, item) = hit_fact(self.app, hit)?;
        Some(Presentation::Fact {
            pane: self.pane,
            entry,
            item,
        })
    }

    /// A click that selects an item in this pane.
    fn click(&self, id: u64) -> Presentation {
        Presentation::Item {
            pane: self.pane,
            id,
        }
    }

    /// The text cursor, if this pane shows it and its row is in the
    /// window: a row of the window and a column.
    fn put_text_cursor(&self, f: &mut Frame, at: Option<(usize, usize)>) {
        if let Some((row, x)) = at.filter(|_| self.shows_text_cursor()) {
            let at = (self.rows.x + x as u16, self.rows.y + row as u16);
            f.set_cursor_position(Position::new(at.0, at.1));
        }
    }

    /// Whether this pane shows the text cursor: it has the focus, in
    /// normal mode. With the cursor on no item, no row is the cursor's.
    fn shows_text_cursor(&self) -> bool {
        self.has_focus() && matches!(self.app.session.mode, Mode::Normal)
    }

    /// The text cursor's column on its line: after its item's indent, tree
    /// mark and address, and the text before it.
    fn text_column(&self, depth: usize) -> usize {
        let address = self.outline().number(self.cursor()).len() + 2;
        2 * depth + 2 + address + self.typed_cells()
    }

    /// How many cells the text before the text cursor takes.
    fn typed_cells(&self) -> usize {
        cells(&self.app.text_of(self.cursor()), self.view().pos)
    }
}

/// Where the rows within a screen's height of the cursor's row lie among
/// all the rows.
fn near_range(rows: &[(u64, usize)], cursor: u64, height: usize) -> std::ops::Range<usize> {
    let at = rows.iter().position(|(id, _)| *id == cursor).unwrap_or(0);
    let first = at.saturating_sub(height);
    first..rows.len().min(first + 2 * height + 1)
}

/// The lines of an outline pane, before they are wrapped and their window
/// is chosen: each line and the indent of its wrapped rows, each place a
/// click acts on, and the cursor's line and depth.
#[derive(Default)]
struct Drawn {
    lines: Vec<(Line<'static>, usize)>,
    places: Vec<Place>,
    cursor: Option<(usize, usize)>,
    /// The line each item drawn starts on, and how many items lie before
    /// and after those drawn.
    starts: Vec<usize>,
    skipped: (usize, usize),
}

/// An outline pane's lines wrapped to its width: each line's rows, and the
/// row each line starts on.
struct Laid {
    wrapped: Vec<Wrapped>,
    first: Vec<usize>,
}

impl Laid {
    /// Lines wrapped to a width, each with its hang.
    fn new(lines: &[(Line<'static>, usize)], width: usize) -> Laid {
        let wrapped: Vec<_> = lines.iter().map(|(l, h)| wrap(l, width, *h)).collect();
        let sizes = wrapped.iter().map(|w| w.rows.len());
        let first = sizes
            .scan(0, |at, n| Some(std::mem::replace(at, *at + n)))
            .collect();
        Laid { wrapped, first }
    }

    /// The row and column where a column of a line is drawn.
    fn place(&self, line: usize, col: usize) -> (usize, usize) {
        let (k, x) = self.wrapped[line].place(col);
        (self.first[line] + k, x)
    }

    /// Where a reach of a line's columns, [from, to), is drawn: each row,
    /// its first column and its width.
    fn spans(&self, line: usize, (from, to): (usize, usize)) -> Vec<(usize, usize, usize)> {
        let below = self.first[line];
        let rows = self.wrapped[line].spans((from, to)).into_iter();
        rows.map(|(k, x, w)| (below + k, x, w)).collect()
    }

    /// The rows a line takes, wrapped.
    fn rows_of(&self, line: usize) -> std::ops::Range<usize> {
        let first = self.first[line];
        first..first + self.wrapped[line].rows.len()
    }

    /// How many rows all the lines take.
    fn len(&self) -> usize {
        self.wrapped.iter().map(|w| w.rows.len()).sum()
    }

    /// All the rows, in order.
    fn rows(self) -> Vec<Line<'static>> {
        self.wrapped.into_iter().flat_map(|w| w.rows).collect()
    }
}

/// A place a click acts on: its line, its column and width, and what it
/// does.
struct Place {
    line: usize,
    span: (usize, usize),
    click: Presentation,
}

impl Place {
    /// A place on a line: the line, its first column and its width.
    fn of((line, x, width): (usize, usize, usize), click: Presentation) -> Place {
        let span = (x, width);
        Place { line, span, click }
    }

    /// A click on each row of it in the window.
    fn add_to(&self, hits: &mut Presentations, laid: &Laid, window: Window) {
        let spans = laid.spans(self.line, self.reach());
        window.add(hits, spans, &self.click);
    }

    /// The columns it covers on its line: [from, to).
    fn reach(&self) -> (usize, usize) {
        let (x, width) = self.span;
        (x, x.saturating_add(width))
    }
}

/// An outline pane's area for its rows: all of it, or, when they do not
/// fit, all but its last row, which says how many lie beyond.
fn kept_for_beyond(area: Rect, more: bool) -> Rect {
    match more && area.height > 1 {
        true => Rect {
            height: area.height - 1,
            ..area
        },
        false => area,
    }
}

/// What lies beyond an outline pane's window: how many items wholly
/// above and below it, and whether an item in it runs on below.
struct Beyond {
    above: usize,
    below: usize,
    cut: bool,
}

impl Beyond {
    /// From the rows each item takes and the window's edges: how many
    /// items lie wholly above it and wholly below it, and whether one in
    /// it runs on below it.
    fn of(rows: &[(usize, usize)], (top, bottom): (usize, usize)) -> Beyond {
        let above = rows.iter().filter(|(_, end)| *end <= top).count();
        let below = rows.iter().filter(|(first, _)| *first >= bottom).count();
        let cut = runs_past(rows, bottom);
        Beyond { above, below, cut }
    }

    /// With the items not drawn, before and after those that are.
    fn skipping(self, (before, after): (usize, usize)) -> Beyond {
        Beyond {
            above: self.above + before,
            below: self.below + after,
            ..self
        }
    }
}

/// Whether an item runs on past a row: starts before it, ends after.
fn runs_past(rows: &[(usize, usize)], row: usize) -> bool {
    rows.iter().any(|(first, end)| *first < row && *end > row)
}

/// What lies beyond the window, in words: "↑ 26 items above · ↓ 3 items
/// below", each side only if any do; "↓ continues below" when only the
/// last item in view runs on.
fn beyond_text(b: Beyond) -> String {
    let side = |n: usize, arrow: &str, side: &str| format!("{arrow} {} {side}", counted(n, "item"));
    let mut said = vec![];
    if b.above > 0 {
        said.push(side(b.above, "↑", "above"));
    }
    match (b.below, b.cut) {
        (0, true) => said.push("↓ continues below".to_string()),
        (0, false) => {}
        (n, _) => said.push(side(n, "↓", "below")),
    }
    said.join(" · ")
}

/// The rows of an outline pane shown in its area: from the first, its
/// offset among all the rows.
#[derive(Clone, Copy)]
struct Window {
    area: Rect,
    offset: usize,
}

impl Window {
    /// The window over an area that keeps a row in view (the text
    /// cursor's), with two rows below it where there is room, but never at
    /// the cost of its own.
    fn over(area: Rect, at: Option<(usize, usize)>) -> Window {
        let row = at.map_or(0, |(row, _)| row);
        let offset = (row + 3).saturating_sub(area.height as usize).min(row);
        Window { area, offset }
    }

    /// The row of the window that a row is, if it is in it.
    fn row(&self, row: usize) -> Option<u16> {
        let y = row.checked_sub(self.offset)?;
        (y < self.area.height as usize).then_some(y as u16)
    }

    /// A place among the rows, on the window: its row of the window and
    /// its column.
    fn place(&self, at: Option<(usize, usize)>) -> Option<(usize, usize)> {
        let (row, x) = at?;
        Some((self.row(row)? as usize, x))
    }

    /// A click on each of a row's columns that is in the window: each a
    /// row, its first column and its width.
    fn add(
        &self,
        hits: &mut Presentations,
        spans: Vec<(usize, usize, usize)>,
        click: &Presentation,
    ) {
        let rects = spans.into_iter().filter_map(|s| self.rect(s));
        rects.for_each(|r| hits.add(self.area, r, click.clone()));
    }

    /// Where a row's columns are drawn, if the row is in the window.
    fn rect(&self, (row, x, w): (usize, usize, usize)) -> Option<Rect> {
        let y = self.row(row)?;
        Some(self.at(x, y, w))
    }

    /// The cells of a row of the window, from a column, of a width.
    fn at(&self, x: usize, y: u16, w: usize) -> Rect {
        Rect::new(self.area.x + x as u16, self.area.y + y, w as u16, 1)
    }

    /// Lay the cursor's ground under rows in the window, across its whole
    /// width, so the cursor's line is found at a glance however short.
    fn ground(&self, f: &mut Frame, rows: std::ops::Range<usize>) {
        let width = self.area.width as usize;
        for y in rows.filter_map(|r| self.row(r)) {
            f.buffer_mut().set_style(self.at(0, y, width), cursor());
        }
    }

    /// Draw the rows in the window.
    fn draw(&self, f: &mut Frame, rows: Vec<Line<'static>>) {
        let height = self.area.height as usize;
        let shown: Vec<Line> = rows.into_iter().skip(self.offset).take(height).collect();
        f.render_widget(Paragraph::new(shown), self.area);
    }

    /// On the row below the window, kept for it, how many rows lie above
    /// and below it: so the outline is known to go on, and nothing it
    /// shows is covered.
    fn beyond(&self, f: &mut Frame, counts: Beyond) {
        let row = Rect::new(self.area.x, self.area.bottom(), self.area.width, 1);
        f.render_widget(
            Paragraph::new(Line::styled(beyond_text(counts), dim())),
            row,
        );
    }

    /// Its first row among all the rows, and the one after its last.
    fn edges(&self) -> (usize, usize) {
        (self.offset, self.offset + self.area.height as usize)
    }
}

impl Drawn {
    /// Nothing drawn yet, to draw a range of so many items: those before
    /// and after it are skipped.
    fn around(range: &std::ops::Range<usize>, total: usize) -> Drawn {
        Drawn {
            skipped: (range.start, total - range.end),
            ..Drawn::default()
        }
    }

    /// Add a line that presents an object over all its width, if it
    /// presents one.
    fn push_presenting(&mut self, line: Line<'static>, hang: usize, p: Option<Presentation>) {
        let whole = (self.lines.len(), 0, usize::MAX);
        self.places.extend(p.map(|p| Place::of(whole, p)));
        self.push(line, hang);
    }

    /// Add a line, whose wrapped rows are set in by a hang.
    fn push(&mut self, line: Line<'static>, hang: usize) {
        self.lines.push((line, hang));
    }

    /// Add lines, each with the same hang.
    fn push_lines(&mut self, lines: impl IntoIterator<Item = Line<'static>>, hang: usize) {
        lines.into_iter().for_each(|l| self.push(l, hang));
    }

    /// The lines wrapped to the pane's width; the window onto them that
    /// keeps the text cursor in view; then the text cursor, and the places
    /// in the window.
    fn show(self, f: &mut Frame, outline: &OutlinePane) -> Presentations {
        let laid = Laid::new(&self.lines, outline.rows.width as usize);
        let cursor = self.text_place(&laid, outline);
        let window = self.window(&laid, outline.rows, cursor);
        outline.put_text_cursor(f, window.place(cursor));
        let hits = self.hits(&laid, window);
        window.ground(f, self.cursor_rows(&laid));
        self.say_beyond(f, &laid, window, outline.rows);
        window.draw(f, laid.rows());
        hits
    }

    /// The window over the pane's rows that keeps the text cursor in
    /// view: all the pane, or, when not all the items are shown, all but
    /// its last row.
    fn window(&self, laid: &Laid, rows: Rect, cursor: Option<(usize, usize)>) -> Window {
        let more = laid.len() > rows.height as usize || self.skipped != (0, 0);
        Window::over(kept_for_beyond(rows, more), cursor)
    }

    /// On the row kept for it, if one is, how many items lie beyond.
    fn say_beyond(&self, f: &mut Frame, laid: &Laid, window: Window, rows: Rect) {
        if window.area != rows {
            window.beyond(f, self.beyond(laid, window));
        }
    }

    /// What lies beyond the window.
    fn beyond(&self, laid: &Laid, window: Window) -> Beyond {
        Beyond::of(&self.item_rows(laid), window.edges()).skipping(self.skipped)
    }

    /// The rows each item drawn takes: its first, and the one after its
    /// last.
    fn item_rows(&self, laid: &Laid) -> Vec<(usize, usize)> {
        let ends = self
            .starts
            .iter()
            .skip(1)
            .copied()
            .chain([self.lines.len()]);
        let lines = self.starts.iter().zip(ends);
        lines
            .map(|(s, e)| (laid.first[*s], laid.rows_of(e - 1).end))
            .collect()
    }

    /// The rows the cursor's item takes, wrapped: from its first, how many.
    fn cursor_rows(&self, laid: &Laid) -> std::ops::Range<usize> {
        let Some((line, _)) = self.cursor else {
            return 0..0;
        };
        laid.rows_of(line)
    }

    /// Where the text cursor is drawn among the rows: its row and column.
    fn text_place(&self, laid: &Laid, outline: &OutlinePane) -> Option<(usize, usize)> {
        let (line, depth) = self.cursor?;
        Some(laid.place(line, outline.text_column(depth)))
    }

    /// The places in a window, each where it is drawn.
    fn hits(&self, laid: &Laid, window: Window) -> Presentations {
        let mut hits = Presentations::default();
        let places = self.places.iter();
        places.for_each(|p| p.add_to(&mut hits, laid, window));
        hits
    }
}

/// How many cells the first characters of a text take: a wide character,
/// such as 日, takes two.
fn cells(text: &[char], n: usize) -> usize {
    let before: String = text.iter().take(n).collect();
    Span::raw(before).width()
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
        None => draw_presented(f, selected_panel(app), inner),
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
    lines
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
fn draw_claimed(f: &mut Frame, app: &App, item: u64, inner: Rect) -> Presentations {
    let lines = claimed(app, item);
    let used = rows_of(&lines, inner.width);
    let mut hits = draw_presented(f, lines, inner);
    hits.0
        .extend(draw_adopt_hint(f, app, row_below(inner, used)).0);
    hits
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
        0 => format!("[[ no item is named “{typed}…”; make makes one once the link is closed"),
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

/// The foot: what is typed or asked, and under it what to do, or the
/// message.
fn draw_prompt(f: &mut Frame, lines: [Line<'static>; 2], rows: [Rect; 2]) {
    let [line, said] = lines;
    f.render_widget(wrapped(vec![line]), rows[0]);
    f.render_widget(wrapped(vec![said]), rows[1]);
}

/// The foot's two lines: what is typed, and what the memex says, dim.
fn foot_lines(app: &App) -> [Line<'static>; 2] {
    let (line, said) = foot(app);
    [line, Line::styled(said, dim())]
}

/// The foot's two lines, by the mode.
fn foot(app: &App) -> (Line<'static>, String) {
    match &app.session.mode {
        Mode::Quasi(a) => (quasi_line(app, a), quasi_guide(a)),
        Mode::ConfirmRetract { target, dependents } => retract_foot(*target, dependents),
        Mode::ConfirmSuppose {
            text, readings, at, ..
        } => suppose_foot(app, text, readings, *at),
        Mode::Menu { .. } => (
            asked("ACT ON", String::new()),
            "Up, Down choose · Enter or a number runs · Esc closes · or click one".into(),
        ),
        Mode::Normal => normal_foot(app),
    }
}

/// A held quasimode's name, reversed, and what is typed in it.
fn quasi_line(app: &App, a: &Active) -> Line<'static> {
    let shown = if a.q() == Quasi::Command {
        app.guided(&a.input)
    } else {
        a.input.clone()
    };
    Line::from(vec![quasi_name(a), Span::raw(" "), Span::raw(shown)])
}

/// A quasimode's name, and whether it is latched, reversed.
fn quasi_name(a: &Active) -> Span<'static> {
    let latched = if a.latched() { " (latched)" } else { "" };
    Span::styled(format!(" {}{latched} ", a.q().label()), reversed())
}

/// What a held quasimode does, and how it ends. While its key is down
/// with nothing else pressed, a release keeps it on rather than ending
/// it, so the foot says so, and how to end it by holding.
fn quasi_guide(a: &Active) -> String {
    if let Some(n) = a.near() {
        return near_warning(n);
    }
    match a.hold {
        Hold::Down => quasi_fresh(a.key),
        Hold::Used | Hold::Latched => quasi_doing(a),
    }
}

/// Leap's warning before it makes an item: what is near the pattern, and
/// that Enter again makes it.
fn near_warning(n: &crate::app::Near) -> String {
    let p = &n.pattern;
    format!(
        "no item holds “{p}”; {} holds “{}” · Enter again makes “{p}”, or change the pattern",
        n.place, n.found
    )
}

/// The foot of a quasimode just entered: a release keeps it on, so the
/// rest can be typed without holding; or what to do while holding.
fn quasi_fresh(q: Quasi) -> String {
    let holding = match q {
        Quasi::LeapDown | Quasi::LeapUp => {
            "or type and release Leap to go to the first match · ? lists every match · Leap again repeats"
        }
        Quasi::Command => {
            "or type and release Command to run on the target · ? lists what may come next"
        }
        Quasi::Suppose => {
            "or type a statement and release Suppose for its readings, or a world of that name if none · ? lists what may come next"
        }
    };
    format!("release now to type without holding · {holding}")
}

/// What a quasimode does, and how it ends.
fn quasi_doing(a: &Active) -> String {
    let end = release_text(a);
    match a.q() {
        Quasi::LeapDown | Quasi::LeapUp => format!(
            "{end} to go to the first match · Tab completes · Esc cancels · ? lists every match · Leap again repeats"
        ),
        _ if a.sentence().is_some() => sentence_doing(a, &end),
        Quasi::Command | Quasi::Suppose => {
            format!(
                "{end} to run on the target · Tab completes · Esc cancels · ? lists what may come next"
            )
        }
    }
}

/// What a Command line holding a sentence does: what APE makes of the
/// sentence, and what the line ends in, the readings of a supposition or
/// APE's verdict.
fn sentence_doing(a: &Active, end: &str) -> String {
    let (does, or_world) = match (a.reads(), a.supposes()) {
        (true, _) => return read_doing(a, end),
        (false, true) => ("offers its readings", ", or a world of that name if none"),
        (false, false) => ("says what APE makes of it", ""),
    };
    match a.hint().and_then(|h| h.said.as_ref()) {
        None => format!(
            "type a statement · ? lists what may come next · Esc cancels · {end} {does}{or_world}"
        ),
        Some(said) => format!("{said} · {end} {does}"),
    }
}

/// What a Command line read by its form does: that it is no command, what
/// it is read as, what APE makes of it, and what ending it does.
fn read_doing(a: &Active, end: &str) -> String {
    let (read, does) = line_read_as(&a.input);
    let said = ape_said(a);
    format!("no command, so read as {read} · {said}{end} {does}")
}

/// What APE makes of a Command line's sentence, and a stop after it; or
/// nothing, if it has said nothing yet.
fn ape_said(a: &Active) -> String {
    let said = a.hint().and_then(|h| h.said.as_ref());
    said.map_or(String::new(), |s| format!("{s} · "))
}

/// What a line read by its form is read as, and what ending it does.
fn line_read_as(line: &str) -> (&'static str, &'static str) {
    match line.trim_end().ends_with('?') {
        true => ("a question", "answers it"),
        false => ("a statement", "offers its readings, if it has any"),
    }
}

/// How a quasimode ends: by Enter or a second tap when latched, else by
/// its key's release.
fn release_text(a: &Active) -> String {
    if a.latched() {
        return "Enter, or tap again,".into();
    }
    let name = match a.key {
        Quasi::Command => "Command",
        Quasi::Suppose => "Suppose",
        Quasi::LeapDown | Quasi::LeapUp => "Leap",
    };
    format!("release {name}")
}

/// The foot while a retraction is asked: the entry, what depends on it,
/// and how to answer.
fn retract_foot(target: u64, dependents: &[u64]) -> (Line<'static>, String) {
    (
        asked("RETRACT", format!(" entry {target}")),
        retract_guide(dependents),
    )
}

/// The foot while a supposition's reading is offered: the statement, the
/// reading restated, and how to answer.
fn suppose_foot(app: &App, text: &str, readings: &[Term], at: usize) -> (Line<'static>, String) {
    (
        asked("SUPPOSE", format!(" {text}")),
        suppose_guide(app, readings, at),
    )
}

/// A question's name, reversed, and what it is about.
fn asked(name: &str, about: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {name} "), reversed()),
        Span::raw(about),
    ])
}

/// What retracting an entry would undo, and how to answer.
fn retract_guide(dependents: &[u64]) -> String {
    let numbers: Vec<String> = dependents.iter().map(u64::to_string).collect();
    let dep = match numbers.is_empty() {
        true => "nothing depends on it".to_string(),
        false => format!(
            "entries that depend on it and will no longer apply: {}",
            numbers.join(", ")
        ),
    };
    format!("{dep} · Enter retracts · Esc keeps it")
}

/// The reading of a supposition offered, restated, and how to answer.
fn suppose_guide(app: &App, readings: &[Term], at: usize) -> String {
    let facts = app.notebook.facts_now().unwrap_or_default();
    let r = reason::restate(&facts, &readings[at]);
    let more = if readings.len() > 1 {
        " · Tab next reading"
    } else {
        ""
    };
    format!("Do you mean: Suppose that {r}.  Enter: yes · Esc: no{more}")
}

/// The foot in normal mode: in a supposition, how to leave it, above any
/// message; else a hint, while there is no message; else the message.
fn normal_foot(app: &App) -> (Line<'static>, String) {
    if let Some(leave) = leaving_hint(app) {
        return (Line::styled(leave, dim()), app.session.message.clone());
    }
    match hint(app) {
        Some((p, g)) if app.session.message.is_empty() => (Line::styled(p, dim()), g),
        _ => (Line::raw(""), app.session.message.clone()),
    }
}

/// The fact a fact entry states: for a claim, what is claimed.
fn inner(t: &Term) -> &Term {
    match t {
        Term::Compound(n, a) if n == "claims" && a.len() == 2 => &a[1],
        t => t,
    }
}

/// How two facts differ: the arguments that are not equal, here and there.
fn difference(a: &Term, b: &Term) -> (String, String) {
    match (inner(a), inner(b)) {
        (Term::Compound(f, x), Term::Compound(g, y)) if f == g && x.len() == y.len() => {
            let d: Vec<(String, String)> = x
                .iter()
                .zip(y)
                .filter(|(p, q)| p != q)
                .map(|(p, q)| (reason::show(p), reason::show(q)))
                .collect();
            (
                d.iter().map(|p| p.0.clone()).collect::<Vec<_>>().join(" "),
                d.iter().map(|p| p.1.clone()).collect::<Vec<_>>().join(" "),
            )
        }
        (x, y) => (reason::show(x), reason::show(y)),
    }
}

fn place(outline: &Outline, seq: u64) -> String {
    match outline.fact_item.get(&seq) {
        Some(i) => outline.number(*i),
        None => format!("entry {seq}"),
    }
}

/// The marks under an item: (is a violation, text) for each constraint
/// solution that rests on one of its facts.
pub fn item_marks(e: &WorldEval, outline: &Outline, item: u64) -> Vec<MarkLine> {
    let findings: Vec<Finding> = e
        .constraints
        .iter()
        .flat_map(ConstraintEval::findings)
        .collect();
    let parts = findings
        .iter()
        .flat_map(|f| finding_marks(f, outline, item));
    merged(parts.collect())
}

/// The marks of each item that has any, in the outline's order: each
/// finding gathered once, under the items whose facts it rests on, not
/// once for each item, as `item_marks` would.
pub fn marks_by_item(e: &WorldEval, outline: &Outline) -> Vec<(u64, Vec<MarkLine>)> {
    let findings: Vec<Finding> = e
        .constraints
        .iter()
        .flat_map(ConstraintEval::findings)
        .collect();
    let mut under: HashMap<u64, Vec<&Finding>> = HashMap::new();
    for f in &findings {
        for item in items_of(f.sol, outline) {
            under.entry(item).or_default().push(f);
        }
    }
    let marks = |id| Some((id, marks_under(under.remove(&id)?, outline, id)));
    outline.in_order().into_iter().filter_map(marks).collect()
}

/// Whether a finding is marked under an item: it rests on the item's
/// facts.
pub(crate) fn marked_under(f: &Finding, outline: &Outline, item: u64) -> bool {
    items_of(f.sol, outline).contains(&item)
}

/// The items whose facts a solution rests on, each once.
fn items_of(sol: &Solution, outline: &Outline) -> BTreeSet<u64> {
    let items = sol
        .prov
        .iter()
        .filter_map(|p| outline.fact_item.get(&p.seq()));
    items.copied().collect()
}

/// An item's marks, from the findings that rest on its facts.
fn marks_under(findings: Vec<&Finding>, outline: &Outline, item: u64) -> Vec<MarkLine> {
    let parts = findings
        .into_iter()
        .filter_map(|f| finding_marks(f, outline, item));
    merged(parts.collect())
}

/// A mark in parts: whether it is loud, what it says of the item's own
/// fact, and the other facts it is against, each with where it is.
struct Mark {
    loud: bool,
    head: String,
    rest: Vec<String>,
    constraint: u64,
}

/// A mark under an item, as drawn: whether it is loud, its text, and the
/// entry of the constraint it is a finding of, if it is one.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkLine {
    pub loud: bool,
    pub text: String,
    pub constraint: Option<u64>,
}

impl Mark {
    /// Whether it says the same of the item as another: the same kind,
    /// the same rule, the same fact here.
    fn says_same(&self, other: &Mark) -> bool {
        self.loud == other.loud && self.head == other.head
    }

    /// Name more other facts, each once.
    fn add(&mut self, rest: Vec<String>) {
        for r in rest {
            if !self.rest.contains(&r) {
                self.rest.push(r);
            }
        }
    }

    /// Its text: what it says of the item, then the other facts.
    fn text(self) -> MarkLine {
        let text = match self.rest.is_empty() {
            true => self.head,
            false => format!("{}; {}", self.head, self.rest.join("; ")),
        };
        let (loud, constraint) = (self.loud, Some(self.constraint));
        MarkLine {
            loud,
            text,
            constraint,
        }
    }
}

/// Marks that say the same of the item, one each: the facts they are
/// against, in order, each once. So a rule the item breaks with others
/// is one mark, not one for each other fact or each solution.
fn merged(marks: Vec<Mark>) -> Vec<MarkLine> {
    let mut out: Vec<Mark> = Vec::new();
    for m in marks {
        match out.iter_mut().find(|o| o.says_same(&m)) {
            Some(o) => o.add(m.rest),
            None => out.push(m),
        }
    }
    out.into_iter().map(Mark::text).collect()
}

/// A finding's mark under an item: its first fact there against each
/// other fact, or, if it rests on the item's facts alone, those facts.
fn finding_marks(f: &Finding, outline: &Outline, item: u64) -> Option<Mark> {
    let m = Marking { f, outline, item };
    m.alone().or_else(|| m.against())
}

/// A finding, marked under an item of an outline.
struct Marking<'a> {
    f: &'a Finding<'a>,
    outline: &'a Outline,
    item: u64,
}

impl<'a> Marking<'a> {
    /// The finding's facts, each with its provenance: the item's, and
    /// the rest.
    fn split(&self) -> (Vec<Sourced<'a>>, Vec<Sourced<'a>>) {
        split_by_item(self.f.sol, self.outline, self.item)
    }

    /// The mark of a finding that rests on the item's facts alone: those
    /// facts.
    fn alone(&self) -> Option<Mark> {
        let (mine, others) = self.split();
        if mine.is_empty() || !others.is_empty() {
            return None;
        }
        let facts: Vec<String> = mine.iter().map(|(_, t)| reason::show(inner(t))).collect();
        Some(self.mark(&facts.join("; "), vec![]))
    }

    /// The mark of a finding that rests on the item's facts and others:
    /// its first fact here, against each other fact.
    fn against(&self) -> Option<Mark> {
        let against = against_item(self.f, self.outline, self.item);
        let here = against.first()?.here_text();
        let theres = against.iter().map(|a| a.there_text(self.outline));
        Some(self.mark(&here, theres.collect()))
    }

    /// A mark: what the finding is, by the rule it is of, then what it
    /// says of the item's fact, and the others.
    fn mark(&self, here: &str, rest: Vec<String>) -> Mark {
        let head = format!("{}: {here} here", finding_kind(self.f, self.outline));
        Mark {
            loud: self.f.violation,
            head,
            rest,
            constraint: self.f.seq,
        }
    }
}

/// What a finding is, by the rule it is of.
fn finding_kind(f: &Finding, outline: &Outline) -> String {
    let rule = rule_named(outline, f.name);
    match f.violation {
        true => format!("breaks {rule}"),
        false => format!("sources disagree under {rule}"),
    }
}

/// A rule, by the address of the item that states it when one does (its
/// text is the rule's name), else by its name: short, as each mark under
/// each item names it.
fn rule_named(outline: &Outline, name: &str) -> String {
    stating_rule(outline, name).map_or_else(
        || format!("“{name}”"),
        |i| format!("constraint {}", outline.number(i)),
    )
}

/// The item whose text states a rule of a name, if one does.
pub(crate) fn stating_rule(outline: &Outline, name: &str) -> Option<u64> {
    let states = |i: &u64| outline.text(*i).trim_end_matches('.') == name;
    outline.in_order().into_iter().find(states)
}

/// A fact of an item against another fact of a finding that rests on both.
struct Against<'a> {
    here: &'a Term,
    there: Sourced<'a>,
}

impl Against<'_> {
    /// What the item's fact holds that the other's does not.
    fn here_text(&self) -> String {
        difference(self.here, self.there.1).0
    }

    /// What the other fact holds that the item's does not, and where it
    /// comes from.
    fn there_text(&self, outline: &Outline) -> String {
        let b = difference(self.here, self.there.1).1;
        format!("{b} in {}", where_from(outline, self.there.0))
    }
}

/// A finding against an item, if it rests on one of the item's facts:
/// the first of them against each other fact.
fn against_item<'a>(f: &'a Finding, outline: &Outline, item: u64) -> Vec<Against<'a>> {
    let (mine, others) = split_by_item(f.sol, outline, item);
    let Some((_, here)) = mine.first() else {
        return vec![];
    };
    let against = |there| Against { here, there };
    others.into_iter().map(against).collect()
}

/// A solution's facts, each with its provenance: those an item states,
/// and the rest.
fn split_by_item<'a>(
    sol: &'a Solution,
    outline: &Outline,
    item: u64,
) -> (Vec<Sourced<'a>>, Vec<Sourced<'a>>) {
    sol.sourced()
        .partition(|(p, _)| outline.states(item, p.seq()))
}

/// Where a fact comes from: its item's number; else a claim's source; else
/// its entry.
fn where_from(outline: &Outline, p: &Prov) -> String {
    match (outline.fact_item.get(&p.seq()), p) {
        (Some(i), _) => outline.number(*i),
        (None, Prov::Claim(src, _)) => src.clone(),
        (None, Prov::Belief(s)) => format!("entry {s}"),
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

/// The predicates a goal or body calls, by name, and through the world's
/// rules, those their bodies call.
fn predicates(e: &WorldEval, t: &Term, out: &mut Vec<String>) {
    let (f, a) = match t {
        Term::Compound(f, a) => (f, a.as_slice()),
        Term::Atom(f) => (f, &[][..]),
        _ => return,
    };
    if let Some(goals) = goals_within(f, a) {
        return goals.iter().for_each(|x| predicates(e, x, out));
    }
    if out.contains(f) {
        return;
    }
    out.push(f.clone());
    rule_bodies(e, f).for_each(|b| predicates(e, b, out));
}

/// Whether a fact of the world, believed or claimed, is of a predicate,
/// by name: what a goal of it may read. A builtin is never one.
fn is_stated(e: &WorldEval, name: &str) -> bool {
    e.facts.iter().any(|(_, f)| head_name(inner(f)) == name)
}

/// The bodies of the world's rules for a predicate, by name.
fn rule_bodies<'a>(e: &'a WorldEval, name: &'a str) -> impl Iterator<Item = &'a Term> {
    e.facts.iter().filter_map(move |(_, f)| match f {
        Term::Compound(n, a) if n == "rule_def" && a.len() == 3 && head_name(&a[1]) == name => {
            Some(&a[2])
        }
        _ => None,
    })
}

/// The name of a rule's head or of a fact: its predicate.
fn head_name(t: &Term) -> &str {
    match t {
        Term::Compound(n, _) | Term::Atom(n) => n,
        _ => "",
    }
}

/// The goals a control construct calls, as body_literal in
/// prolog/dataflow.pl finds them; none if it is not one.
fn goals_within<'a>(f: &str, a: &'a [Term]) -> Option<&'a [Term]> {
    match (f, a.len()) {
        ("," | ";" | "->" | "*->" | "forall", 2) | ("\\+", 1) => Some(a),
        ("findall", 3 | 4) => Some(&a[1..2]),
        _ => None,
    }
}

fn node_def(e: &WorldEval, id: u64) -> Option<&Term> {
    e.facts.iter().find_map(|(_, f)| match f {
        Term::Compound(n, a) if n == "node_def" && a.len() == 2 && a[0] == Term::Int(id as i64) => {
            Some(&a[1])
        }
        _ => None,
    })
}

/// A constraint's heading, then its rule, set in.
fn constraint_head(c: &ConstraintEval) -> Vec<Line<'static>> {
    let heading = format!("CONSTRAINT · {}", c.constraint.name.to_uppercase());
    let rule = rule_lines(&c.constraint.rule);
    let rule = rule.into_iter().map(|l| Line::raw(format!("  {l}")));
    let head = [Line::styled(heading, dim()), Line::raw("")];
    head.into_iter()
        .chain(rule)
        .chain([Line::raw("")])
        .collect()
}

/// The constraints of a world, drawn against the outline.
struct ConstraintPane<'a> {
    outline: &'a Outline,
    e: &'a WorldEval,
}

impl ConstraintPane<'_> {
    /// The right pane for an item with marks: each constraint with a
    /// finding that rests on one of its facts.
    fn of(&self, item: u64) -> Vec<Presented> {
        let touching = self.e.constraints.iter().filter(|c| self.touches(c, item));
        touching.flat_map(|c| self.constraint(c)).collect()
    }

    /// Whether a constraint has a finding that rests on one of an item's
    /// facts.
    fn touches(&self, c: &ConstraintEval, item: u64) -> bool {
        let found = c.violations.iter().chain(&c.disagreements);
        let mut prov = found.flat_map(|s| &s.prov);
        prov.any(|p| self.outline.states(item, p.seq()))
    }

    /// A constraint: its heading and rule, its findings, and what depends
    /// on the facts it reads.
    fn constraint(&self, c: &ConstraintEval) -> Vec<Presented> {
        let mut head = plain(constraint_head(c));
        head[0].1 = Some(self.presented(c));
        [head, plain(self.found(c)), plain(self.dependents(c))].concat()
    }

    /// A constraint, presented in the outline pane, with the item whose
    /// text states it.
    fn presented(&self, c: &ConstraintEval) -> Presentation {
        let item = stating_rule(self.outline, &c.constraint.name);
        Presentation::Constraint {
            pane: 0,
            entry: c.seq,
            item,
            under: None,
        }
    }

    /// A constraint's violations, loud, then its disagreements.
    fn found(&self, c: &ConstraintEval) -> Vec<Line<'static>> {
        let violations = self.kind("violations", &c.violations, violation());
        let disagreements = "disagreements between sources";
        let disagreements = self.kind(disagreements, &c.disagreements, disagreement());
        [violations, disagreements].concat()
    }

    /// Findings of one kind, if there are any: a heading that says when
    /// they were recomputed, then a row for each.
    fn kind(&self, title: &str, found: &[Solution], style: Style) -> Vec<Line<'static>> {
        if found.is_empty() {
            return vec![];
        }
        let heading = title.to_string();
        let rows = found.iter().map(|s| Line::styled(self.row(s), style));
        let heading = std::iter::once(Line::styled(heading, dim()));
        heading.chain(rows).chain([Line::raw("")]).collect()
    }

    /// A finding's row: what its facts share, then how each differs and
    /// where it comes from; for one fact, the fact.
    fn row(&self, sol: &Solution) -> String {
        if let [f] = sol.facts.as_slice() {
            let from = where_from(self.outline, &sol.prov[0]);
            return format!("{} ({from})", reason::show(inner(f)));
        }
        let parts = self.parts(sol);
        match shared_key(&sol.facts).as_str() {
            "" => parts,
            key => format!("{key}: {parts}"),
        }
    }

    /// Each fact of a finding, how it differs and where it comes from.
    fn parts(&self, sol: &Solution) -> String {
        let parts: Vec<String> = (0..sol.facts.len()).map(|i| self.part(sol, i)).collect();
        parts.join(" · ")
    }

    /// A fact of a finding, by its place: how it differs from the first
    /// other fact, and where it comes from.
    fn part(&self, sol: &Solution, i: usize) -> String {
        let from = where_from(self.outline, &sol.prov[i]);
        format!("{} ({from})", differing(&sol.facts, i))
    }

    /// What depends on the facts a constraint reads, if anything: the
    /// derived items that read one of its predicates.
    fn dependents(&self, c: &ConstraintEval) -> Vec<Line<'static>> {
        let deps = self.derived_reading(&self.fact_reads(c.seq));
        if deps.is_empty() {
            return vec![];
        }
        let heading = std::iter::once(Line::styled("depends on these facts", dim()));
        let rows = deps.into_iter().map(|d| self.dependent(d));
        heading.chain(rows).chain([Line::raw("")]).collect()
    }

    /// The predicates a definition reads that the world's facts state.
    fn fact_reads(&self, id: u64) -> Vec<String> {
        let mut reads = definition_reads(self.e, id);
        reads.retain(|p| is_stated(self.e, p));
        reads
    }

    /// The derived items that read one of these predicates, in outline
    /// order.
    fn derived_reading(&self, reads: &[String]) -> Vec<u64> {
        let reading = |d: &u64| {
            definition_reads(self.e, *d)
                .iter()
                .any(|p| reads.contains(p))
        };
        let mut deps: Vec<u64> = self
            .outline
            .derived
            .keys()
            .copied()
            .filter(reading)
            .collect();
        let order = self.outline.in_order();
        deps.sort_by_key(|d| order.iter().position(|x| x == d));
        deps
    }

    /// A derived item that depends on the facts: its number, and its text,
    /// cut at 40 characters.
    fn dependent(&self, d: u64) -> Line<'static> {
        let text = self.derived_text(d);
        let short: String = text.chars().take(40).collect();
        let more = if text.chars().count() > 40 {
            " …"
        } else {
            ""
        };
        Line::raw(format!("  {} {short}{more}", self.outline.number(d)))
    }

    /// A derived item's text in the world, or as the outline holds it.
    fn derived_text(&self, d: u64) -> String {
        let derive = self.outline.derived.get(&d);
        let text = derive.and_then(|t| self.e.derived_text(d, t));
        text.unwrap_or_else(|| self.outline.text(d))
    }
}

/// What a finding's facts share: each argument of the first that every
/// fact has in the same place.
fn shared_key(facts: &[Term]) -> String {
    let shared: Vec<String> = shared_args(facts).map(reason::show).collect();
    shared.join(", ")
}

/// The arguments of the first fact that every fact has in the same place.
fn shared_args(facts: &[Term]) -> impl Iterator<Item = &Term> {
    let n = facts.first().map_or(0, arity);
    (0..n).filter_map(move |k| shared_at(facts, k))
}

/// The first fact's argument at a place, if every fact has it there.
fn shared_at(facts: &[Term], k: usize) -> Option<&Term> {
    let first = arg_of(facts.first()?, k)?;
    facts
        .iter()
        .all(|f| arg_of(f, k) == Some(first))
        .then_some(first)
}

/// How many arguments a fact has.
fn arity(f: &Term) -> usize {
    match inner(f) {
        Term::Compound(_, a) => a.len(),
        _ => 0,
    }
}

/// A fact's argument at a place.
fn arg_of(f: &Term, k: usize) -> Option<&Term> {
    match inner(f) {
        Term::Compound(_, a) => a.get(k),
        _ => None,
    }
}

/// How the fact at a place differs from the first other fact, or from
/// itself if it is alone.
fn differing(facts: &[Term], at: usize) -> String {
    let others = facts.iter().enumerate().filter(|(i, _)| *i != at);
    let other = others.map(|(_, f)| f).next().unwrap_or(&facts[at]);
    difference(&facts[at], other).0
}

/// What a definition reads: the predicates its goal calls.
fn definition_reads(e: &WorldEval, id: u64) -> Vec<String> {
    let mut reads = vec![];
    if let Some(goal) = definition_goal(e, id) {
        predicates(e, goal, &mut reads);
    }
    reads
}

/// A definition's goal: the last argument of its node,
/// constraint(Name, Vars, Body) or query(Vars, Goal).
fn definition_goal(e: &WorldEval, id: u64) -> Option<&Term> {
    match node_def(e, id)? {
        Term::Compound(_, a) => a.last(),
        _ => None,
    }
}

/// The right pane at a claim item, or one interrogated: each of the
/// item's claims, then what adopting them would do. The hint follows,
/// drawn by draw_interrogation, where a click can find it.
fn interrogation(app: &App, item: u64) -> Vec<Presented> {
    let Some(e) = app.reason.eval[0].as_ref() else {
        return plain(vec![Line::raw("not yet evaluated")]);
    };
    let mut lines = interrogation_head(&app.notebook.outline, item);
    lines.extend(claims_asked(app, e, item));
    lines.extend(plain(adoption_lines(app, item).to_vec()));
    lines
}

/// Each claim an item states, and what is asked of it.
fn claims_asked(app: &App, e: &WorldEval, item: u64) -> Vec<Presented> {
    let claims = app.claims_shown(item).into_iter();
    claims.flat_map(|c| claim_lines(app, e, item, c)).collect()
}

/// The heading of a claim item's questions, and the item.
fn interrogation_head(outline: &Outline, item: u64) -> Vec<Presented> {
    let named = format!("CLAIMS OF {} · WHO SAYS THEM", outline.number(item));
    let heading = Line::styled(named, dim());
    plain(vec![
        heading,
        Line::raw(""),
        Line::raw(outline.text(item)),
        Line::raw(""),
    ])
}

/// A claim interrogated: the claim, in an offer's rows, then who else
/// claims it, who claims the contrary, and its source's record.
fn claim_lines(app: &App, e: &WorldEval, item: u64, claim: (u64, String, Term)) -> Vec<Presented> {
    let mut lines = claim_row(app, item, &claim);
    lines.extend(plain(claim_questions(app, e, &claim)));
    lines
}

/// A claim an item states, in an offer's rows: dim, and said so, once
/// adopted.
fn claim_row(app: &App, item: u64, claim: &(u64, String, Term)) -> Vec<Presented> {
    let (_, source, fact) = claim;
    let open = app.claim_open(item, source.clone(), fact);
    let adopted = app.not_adopted(vec![claim.clone()]).is_empty();
    offers::offer_row(&open, adopted.then_some("adopted"))
}

/// What is asked of a claim: who else claims it, who claims the
/// contrary, and its source's record.
fn claim_questions(app: &App, e: &WorldEval, claim: &(u64, String, Term)) -> Vec<Line<'static>> {
    let asking = Asking::of(app, e, claim);
    let mut lines = section("who else claims it", asking.claimants(), "no one else");
    lines.extend(section(
        "who claims the contrary",
        asking.contrary(),
        "no one",
    ));
    lines.extend(section(
        "the source's record",
        vec![record(app, &claim.1)],
        "",
    ));
    lines
}

/// A claim being interrogated: the outline it is in, what reasoning
/// found, and the claim: its entry, its source and its fact.
struct Asking<'a> {
    outline: &'a Outline,
    eval: &'a WorldEval,
    claim: &'a (u64, String, Term),
}

impl<'a> Asking<'a> {
    fn of(app: &'a App, eval: &'a WorldEval, claim: &'a (u64, String, Term)) -> Self {
        let outline = &app.notebook.outline;
        Asking {
            outline,
            eval,
            claim,
        }
    }

    /// A fact restated in words, by the world's labels.
    fn restated(&self, f: &Term) -> String {
        reason::restate(&self.eval.facts, inner(f))
    }

    /// Who else states the claim's fact: each other entry, by its place,
    /// with its source, or as a belief.
    fn claimants(&self) -> Vec<String> {
        let (seq, _, p) = self.claim;
        let others = self.eval.stating(p, *seq);
        others
            .into_iter()
            .map(|(s, f)| claimant(self.outline, s, f))
            .collect()
    }

    /// Who claims the contrary: each other fact of each disagreement that
    /// rests on the claim, constraint by constraint. A violation rests on
    /// beliefs alone, never on a claim.
    fn contrary(&self) -> Vec<String> {
        let found = self.eval.constraints.iter().flat_map(disagreements);
        let resting = found.filter(|(_, sol)| sol.prov.iter().any(|q| q.seq() == self.claim.0));
        resting
            .flat_map(|(name, sol)| self.against(name, sol))
            .collect()
    }

    /// The facts of a finding other than the claim's, each a row of who
    /// claims the contrary.
    fn against(&self, name: &str, sol: &Solution) -> Vec<String> {
        let others = sol.sourced().filter(|(q, _)| q.seq() != self.claim.0);
        others.map(|(q, f)| self.contrary_row(name, q, f)).collect()
    }

    /// A row of who claims the contrary: where the fact comes from, the
    /// fact restated, and the constraint it breaks.
    fn contrary_row(&self, name: &str, q: &Prov, f: &Term) -> String {
        let said = self.restated(f);
        let from = where_from(self.outline, q);
        format!("  {from}: {said} ({name})")
    }
}

/// A constraint's disagreements, by its name.
fn disagreements(c: &ConstraintEval) -> impl Iterator<Item = (&str, &Solution)> {
    let name = c.constraint.name.as_str();
    c.disagreements.iter().map(move |sol| (name, sol))
}

/// A section of an interrogation: its heading, then its rows, or, if it
/// has none, what says so.
fn section(heading: &str, rows: Vec<String>, none: &str) -> Vec<Line<'static>> {
    let rows = if rows.is_empty() {
        vec![format!("  {none}")]
    } else {
        rows
    };
    let heading = Line::styled(heading.to_string(), dim());
    std::iter::once(heading)
        .chain(rows.into_iter().map(Line::raw))
        .collect()
}

/// An entry that states a fact: where it is, and its source, or that it
/// is a belief.
fn claimant(outline: &Outline, seq: u64, f: &Term) -> String {
    let at = place(outline, seq);
    match f {
        Term::Compound(n, a) if n == "claims" => format!("  {at} ({})", reason::evidence(&a[0])),
        _ => format!("  {at} (a belief)"),
    }
}

/// A source's record: how many of its claims were adopted, and how many
/// of those adoptions were retracted since, anywhere in the journal.
fn record(app: &App, src: &str) -> String {
    let entries = app.journal().entries();
    let adopted = adoptions_from(entries, src);
    let retracted = retracted_among(entries, &adopted);
    format!(
        "  {} of its claims adopted, {retracted} of them retracted since",
        adopted.len()
    )
}

/// How many of these entries a retraction names, anywhere in the journal.
fn retracted_among(entries: &[format::record::Entry], seqs: &[u64]) -> usize {
    let retracted = |s: &&u64| entries.iter().any(|r| retracts(r, **s));
    seqs.iter().filter(retracted).count()
}

/// The entries that adopt a claim from a source, by number.
fn adoptions_from(entries: &[format::record::Entry], src: &str) -> Vec<u64> {
    let adopting = entries.iter().filter(|x| adopts_from(x, src));
    adopting.map(|x| x.seq).collect()
}

/// Whether an entry adopts a claim from a source.
fn adopts_from(e: &format::record::Entry, src: &str) -> bool {
    e.kind() == "adopt" && e.body().is_some_and(|b| b["source"] == src)
}

/// Whether an entry retracts another, by its number.
fn retracts(e: &format::record::Entry, target: u64) -> bool {
    e.kind() == "retract" && e.body().and_then(|b| b["target"].as_u64()) == Some(target)
}

/// What adopting the interrogated claims would do, then the blank line
/// before the hint.
fn adoption_lines(app: &App, item: u64) -> [Line<'static>; 3] {
    let preview = match &app.reason.preview {
        Some((of, said)) if *of == item => said.clone(),
        _ => "not yet computed".into(),
    };
    [
        Line::styled("if adopted", dim()),
        Line::raw(format!("  {preview}")),
        Line::raw(""),
    ]
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
        .filter(|x| x.kind() == "suppose")
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

/// A world's column, by its place, in its half.
fn draw_column(f: &mut Frame, k: usize, area: Rect, rows: Vec<Line<'static>>) {
    f.render_widget(Paragraph::new(rows).block(ruled(k)), area);
}

/// The first world is ruled off from the second.
fn ruled(k: usize) -> Block<'static> {
    let border = if k == 0 {
        Borders::RIGHT
    } else {
        Borders::NONE
    };
    Block::default().borders(border)
}

/// The widths inside the two halves: the first less its rule.
fn column_widths(halves: [Rect; 2]) -> [u16; 2] {
    [halves[0].width.saturating_sub(1), halves[1].width]
}

/// Two columns of lines wrapped to their widths, each later row of a line
/// set in by two, and kept row for row: each pair of lines takes as many
/// rows as the taller of the two.
fn side_by_side(columns: [Vec<WorldLine>; 2], widths: [u16; 2]) -> [Vec<Line<'static>>; 2] {
    let pairs: Vec<_> = line_pairs(&columns)
        .map(|l| padded_pair(l, widths))
        .collect();
    [0, 1].map(|k| pairs.iter().flat_map(|p| p[k].clone()).collect())
}

/// The columns' lines, pair by pair: the ith of each, or none where a
/// column is shorter.
fn line_pairs(columns: &[Vec<WorldLine>; 2]) -> impl Iterator<Item = [Option<&WorldLine>; 2]> {
    let n = columns[0].len().max(columns[1].len());
    (0..n).map(move |i| columns.each_ref().map(|c| c.get(i)))
}

/// A pair of lines, one from each column, wrapped, the shorter padded to
/// the taller's rows.
fn padded_pair(lines: [Option<&WorldLine>; 2], widths: [u16; 2]) -> [Vec<Line<'static>>; 2] {
    let mut pair = [0, 1].map(|k| wrapped_rows(lines[k], widths[k]));
    let height = pair.iter().map(Vec::len).max().unwrap_or(0);
    pair.iter_mut()
        .for_each(|rows| rows.resize(height, Line::raw("")));
    pair
}

/// A line's rows wrapped to a width, later rows set in by its hang; none
/// for no line.
fn wrapped_rows(line: Option<&WorldLine>, width: u16) -> Vec<Line<'static>> {
    line.map_or_else(Vec::new, |(l, hang)| wrap(l, width as usize, *hang).rows)
}

/// A line of a world's column, and the hang of its wrapped rows.
type WorldLine = (Line<'static>, usize);

/// Lines that are not an item's row, their wrapped rows set in by two.
fn hanging(lines: impl IntoIterator<Item = Line<'static>>) -> Vec<WorldLine> {
    lines.into_iter().map(|l| (l, 2)).collect()
}

/// One world of a supposition: its outline, what reasoning found in it,
/// and its title.
struct Side<'a> {
    outline: &'a Outline,
    eval: Option<&'a WorldEval>,
    title: String,
}

impl<'a> Side<'a> {
    /// The actual world, at its outline: the second evaluation.
    fn actual(app: &'a App, outline: &'a Outline) -> Self {
        let title = "WORLD · ACTUAL".into();
        let eval = app.reason.eval[1].as_ref();
        Side {
            outline,
            eval,
            title,
        }
    }

    /// The supposed world, the current one, by its name: the first
    /// evaluation.
    fn supposed(app: &'a App, name: &str) -> Self {
        let title = format!("WORLD · SUPPOSE {}", name.to_uppercase());
        let eval = app.reason.eval[0].as_ref();
        let outline = &app.notebook.outline;
        Side {
            outline,
            eval,
            title,
        }
    }

    /// An item's text in this world: derived text as the world fills it,
    /// else as the outline holds it.
    fn text(&self, i: u64) -> String {
        match self.filled(i) {
            Some((true, answer)) => format!("{} → {answer}", self.outline.text(i)),
            Some((false, text)) => text,
            None => self.outline.text(i),
        }
    }

    /// What this world fills in for an item, if it derives anything:
    /// whether it is a question's answer, and the text.
    fn filled(&self, i: u64) -> Option<(bool, String)> {
        let (d, e) = self.outline.derived.get(&i).zip(self.eval)?;
        Some((d.under, e.derived_text(i, d)?))
    }

    /// The entries whose facts this world states.
    fn fact_seqs(&self) -> Vec<u64> {
        self.facts().iter().map(|(s, _)| *s).collect()
    }

    /// The facts this world states, each by its entry.
    fn facts(&self) -> &'a [(u64, Term)] {
        self.eval.map(|e| e.facts.as_slice()).unwrap_or_default()
    }
}

/// A supposition drawn side by side: the actual world's outline, at the
/// head it was sprouted from, and the supposition's name; the rows the
/// worlds share; and what the supposed world sets aside, and why. Rows
/// are the items that state facts or show derived values.
struct Sides<'a> {
    app: &'a App,
    actual: Outline,
    name: String,
    rows: Vec<u64>,
    aside: SetAside,
}

/// The entries a supposed world sets aside, and the constraints each was
/// set aside under, where its retraction records them.
#[derive(Default)]
struct SetAside {
    entries: Vec<u64>,
    reasons: BTreeMap<u64, Vec<u64>>,
}

impl SetAside {
    /// The entries whose recorded constraints, if any, pass a test.
    fn chosen(&self, test: impl Fn(Option<&Vec<u64>>) -> bool) -> Vec<u64> {
        let passes = |s: &&u64| test(self.reasons.get(*s));
        self.entries.iter().filter(passes).copied().collect()
    }
}

impl<'a> Sides<'a> {
    /// The open supposition, if there is one.
    fn of(app: &'a App) -> Option<Self> {
        let sup = app.reason.supposition.as_ref()?;
        let actual = app.notebook.outline_at(sup.actual);
        let sides = Sides::new(app, actual, sup.name.clone());
        Some(sides.with_set_aside())
    }

    /// The sides of a supposition by this name, beside the actual world's
    /// outline, with nothing yet set aside.
    fn new(app: &'a App, actual: Outline, name: String) -> Self {
        let rows = shared_rows(&actual, &app.notebook.outline);
        Sides {
            app,
            actual,
            name,
            rows,
            aside: SetAside::default(),
        }
    }

    /// The sides, with what the supposed world sets aside and why.
    fn with_set_aside(mut self) -> Self {
        let entries = set_aside(&[self.side(0), self.side(1)]);
        let reasons = reasons_of(self.app, &entries);
        self.aside = SetAside { entries, reasons };
        self
    }

    /// A world, by its place: the actual one, then the supposed one.
    fn side(&self, k: usize) -> Side<'_> {
        match k {
            0 => Side::actual(self.app, &self.actual),
            _ => Side::supposed(self.app, &self.name),
        }
    }

    /// The two columns, each half the area, wrapped and kept row for row:
    /// each pair of lines takes as many rows as the taller of the two.
    fn draw(&self, f: &mut Frame, area: Rect) {
        let halves = Layout::horizontal([Constraint::Percentage(50); 2]).areas::<2>(area);
        let widths = column_widths(halves);
        let rows = side_by_side(self.columns(widths), widths);
        for (k, rows) in rows.into_iter().enumerate() {
            draw_column(f, k, halves[k], rows);
        }
    }

    /// The two worlds' columns, for their widths.
    fn columns(&self, widths: [u16; 2]) -> [Vec<WorldLine>; 2] {
        [0, 1].map(|k| self.column(k, widths[k]))
    }

    /// A world's column: its title, its rows, its constraints, and its
    /// views.
    fn column(&self, k: usize, width: u16) -> Vec<WorldLine> {
        let mut lines = self.rows_of(k);
        lines.extend(hanging(self.constraint_lines(k)));
        lines.extend(hanging(self.view_lines(k, width)));
        lines
    }

    /// A world's title, what the supposed world supposes (blank rows on
    /// the actual side, to keep the worlds row for row), then its rows,
    /// each wrapped under its text, then a blank line.
    fn rows_of(&self, k: usize) -> Vec<WorldLine> {
        let mut lines = self.title_lines(k);
        lines.extend(self.item_rows(k));
        lines.extend(hanging([Line::raw("")]));
        lines
    }

    /// A world's title, a blank line, and what the supposed world
    /// supposes.
    fn title_lines(&self, k: usize) -> Vec<WorldLine> {
        let title = [Line::styled(self.side(k).title, dim()), Line::raw("")];
        hanging(title.into_iter().chain(self.supposed_lines(k)))
    }

    /// A world's rows, each hung under its text, after its head.
    fn item_rows(&self, k: usize) -> Vec<WorldLine> {
        let row = |i: &u64| (self.row(k, *i), self.head(*i).len());
        self.rows.iter().map(row).collect()
    }

    /// The rows of what the supposed world supposes, each fact as the
    /// notebook reads it; on the actual side, as many blank rows.
    fn supposed_lines(&self, k: usize) -> Vec<Line<'static>> {
        let facts = self.supposed_facts();
        match k {
            0 => vec![Line::raw(""); facts.len()],
            _ => facts.into_iter().map(supposed_line).collect(),
        }
    }

    /// The facts the supposed world states that the actual one does not,
    /// on no item: what was supposed.
    fn supposed_facts(&self) -> Vec<String> {
        let actual = self.side(0).fact_seqs();
        let facts = self.side(1).facts().iter();
        let supposed = facts.filter(|(s, _)| self.only_supposed(*s, &actual));
        supposed.map(|(_, f)| crate::reason::show(f)).collect()
    }

    /// Whether an entry's fact is the supposed world's alone, and on no
    /// item: not among the actual world's.
    fn only_supposed(&self, seq: u64, actual: &[u64]) -> bool {
        let on_item = self.app.notebook.outline.fact_item.contains_key(&seq);
        !actual.contains(&seq) && !on_item
    }

    fn row(&self, k: usize, i: u64) -> Line<'static> {
        match k {
            0 => self.actual_row(i),
            _ => self.supposed_row(i),
        }
    }

    /// A row of the actual world: the item's text there, after its head,
    /// with a dashed trail where the supposed world differs.
    fn actual_row(&self, i: u64) -> Line<'static> {
        let here = self.side(0).text(i);
        let trail = here != self.side(1).text(i);
        let trail = trail.then(|| Span::styled("  - - -", dim()));
        let text = Span::raw(format!("{}{here}", self.head(i)));
        Line::from_iter(std::iter::once(text).chain(trail))
    }

    /// A row of the supposed world: the item's text there, set in by its
    /// level; bold and underlined where it differs from the actual world;
    /// struck through where all it states is set aside; and a note where
    /// any of it is.
    fn supposed_row(&self, i: u64) -> Line<'static> {
        let aside = self.aside_at(i);
        let whole = !aside.is_empty() && aside.len() == self.stated_at(i);
        let text = self.supposed_text(i, whole);
        let said = self.set_aside_note(i, whole, &aside);
        Line::from_iter(std::iter::once(text).chain(said))
    }

    /// An item's text in the supposed world, after its head, in its style
    /// there.
    fn supposed_text(&self, i: u64, whole: bool) -> Span<'static> {
        let here = format!("{}{}", self.head(i), self.side(1).text(i));
        Span::styled(here, self.supposed_style(i, whole))
    }

    /// How a row of the supposed world is drawn: struck through if all
    /// it states is set aside, else marked where its text differs.
    fn supposed_style(&self, i: u64, whole: bool) -> Style {
        let differs = self.side(1).text(i) != self.side(0).text(i);
        changed_style(differs, whole)
    }

    /// A note on a row whose facts are set aside, if any are, with the
    /// row's place, so that the constraint which set them aside can name
    /// it; and, where the row states more than is set aside, the facts
    /// that are.
    fn set_aside_note(&self, i: u64, whole: bool, aside: &[u64]) -> Option<Span<'static>> {
        if aside.is_empty() {
            return None;
        }
        let place = self.actual.number(i);
        let note = match whole {
            true => format!("  ({place} set aside here)"),
            false => format!("  ({place} set aside here: {})", self.facts_shown(aside)),
        };
        Some(Span::styled(note, dim()))
    }

    /// The facts of entries of the actual world, as Prolog, apart by
    /// semicolons.
    fn facts_shown(&self, entries: &[u64]) -> String {
        let facts = self.side(0).facts().iter();
        let shown = facts.filter(|(s, _)| entries.contains(s));
        let shown: Vec<String> = shown.map(|(_, f)| crate::reason::show(f)).collect();
        shown.join("; ")
    }

    /// An item's row's head: two cells for each level below the top,
    /// then its address, as the outline shows it, so a note's "1.1" can
    /// be found.
    fn head(&self, i: u64) -> String {
        let outline = self.outline_of(i);
        format!(
            "{}{}. ",
            "  ".repeat(outline.depth_of(i)),
            outline.number(i)
        )
    }

    /// The outline that holds an item: the actual world's, else the
    /// supposed world's, for an item only it has.
    fn outline_of(&self, i: u64) -> &Outline {
        match self.actual.contains(i) {
            true => &self.actual,
            false => &self.app.notebook.outline,
        }
    }

    /// The entries the supposed world sets aside whose facts an item of
    /// the actual world states.
    fn aside_at(&self, i: u64) -> Vec<u64> {
        let actual = &self.actual;
        let at = self.aside.entries.iter().filter(|s| actual.states(i, **s));
        at.copied().collect()
    }

    /// How many facts an item of the actual world states there.
    fn stated_at(&self, i: u64) -> usize {
        let at = self.side(0).fact_seqs().into_iter();
        at.filter(|s| self.actual.states(i, *s)).count()
    }

    /// The lines of each constraint of a world: how many violations and
    /// disagreements it has, and, in the supposed world, what it sets
    /// aside.
    fn constraint_lines(&self, k: usize) -> Vec<Line<'static>> {
        let Some(e) = self.side(k).eval else {
            return vec![];
        };
        let each = e.constraints.iter();
        let mut lines: Vec<Line<'static>> =
            each.flat_map(|c| self.constraint_block(k, c)).collect();
        if k == 1 {
            lines.extend(self.unexplained_line().map(Line::raw));
        }
        lines
    }

    /// A constraint's lines in a world, the same number in each so the
    /// worlds stay row for row: its name; both counts, so the worlds
    /// compare count for count; and, where it sets facts aside, a line
    /// naming them, blank in the actual world.
    fn constraint_block(&self, k: usize, c: &ConstraintEval) -> Vec<Line<'static>> {
        let name = Line::raw(format!("Constraint {}:", c.constraint.name));
        let mut lines = vec![name, Line::from(self.counts_in(k, c))];
        let under = self.set_aside_where(|cs| cs.is_some_and(|cs| cs.contains(&c.seq)));
        if !under.is_empty() {
            let said = if k == 1 {
                format!("  {under} set aside here")
            } else {
                String::new()
            };
            lines.push(Line::raw(said));
        }
        lines
    }

    /// A constraint's counts in a world, set in; in the supposed world, a
    /// count that differs from the actual world's is marked as changed.
    fn counts_in(&self, k: usize, c: &ConstraintEval) -> Vec<Span<'static>> {
        let mut spans = finding_counts(c);
        if let (1, Some(a)) = (k, self.actual_of(c)) {
            mark_changes(&mut spans, a, c);
        }
        std::iter::once(Span::raw("  ")).chain(spans).collect()
    }

    /// The actual world's evaluation of a constraint.
    fn actual_of(&self, c: &ConstraintEval) -> Option<&ConstraintEval> {
        let e = self.side(0).eval?;
        e.constraints.iter().find(|a| a.seq == c.seq)
    }

    /// The facts set aside whose retraction records no constraint (one
    /// retracted by hand, or written before retractions recorded them),
    /// on a line of their own.
    fn unexplained_line(&self) -> Option<String> {
        let places = self.set_aside_where(|cs| cs.is_none());
        (!places.is_empty()).then(|| format!("Retracted here: {places}"))
    }

    /// Where the facts set aside are, in the actual world, of those whose
    /// recorded constraints pass a test.
    fn set_aside_where(&self, test: impl Fn(Option<&Vec<u64>>) -> bool) -> String {
        let chosen = self.aside.chosen(test);
        let places: Vec<String> = chosen.iter().map(|s| place(&self.actual, *s)).collect();
        places.join(", ")
    }

    /// Each view of a world, drawn from its facts.
    fn view_lines(&self, k: usize, width: u16) -> Vec<Line<'static>> {
        let views = self.views_in(k).into_iter();
        views.flat_map(|i| self.view(k, i, width)).collect()
    }

    /// The items of a world that are views, in its order.
    fn views_in(&self, k: usize) -> Vec<u64> {
        let views = &self.app.notebook.outline.views;
        let items = self.side(k).outline.in_order().into_iter();
        items.filter(|i| views.contains_key(i)).collect()
    }

    /// A view in a world's column: a blank line, its item's text, then
    /// its figure.
    fn view(&self, k: usize, i: u64, width: u16) -> Vec<Line<'static>> {
        [self.view_head(k, i), self.figure(k, i, width)].concat()
    }

    /// A blank line, then a view's item's text in a world.
    fn view_head(&self, k: usize, i: u64) -> Vec<Line<'static>> {
        vec![Line::raw(""), Line::raw(self.side(k).outline.text(i))]
    }

    /// A view's figure in a world's column, set in. The actual world is
    /// the second evaluation (eval[1]), the supposed the first.
    fn figure(&self, k: usize, i: u64, width: u16) -> Vec<Line<'static>> {
        let inner = (width as usize).saturating_sub(4);
        figure_lines(&view_figure(self.app, 1 - k, i, inner, 8), 2)
    }
}

/// The items of both worlds but their views, which follow the rows, so
/// each world is seen whole, its notes and claims beside its facts: the
/// actual world's in its order, then the supposed world's that it lacks.
fn shared_rows(actual: &Outline, supposed: &Outline) -> Vec<u64> {
    let mut rows = actual.in_order();
    let more: Vec<u64> = supposed
        .in_order()
        .into_iter()
        .filter(|i| !rows.contains(i))
        .collect();
    rows.extend(more);
    rows.retain(|i| !supposed.views.contains_key(i));
    rows
}

/// How a supposed world's row is drawn: struck through where its
/// fact is set aside; bold and underlined where its text differs.
fn changed_style(differs: bool, gone: bool) -> Style {
    match (gone, differs) {
        (true, _) => Style::default().add_modifier(Modifier::CROSSED_OUT),
        (false, true) => changed(),
        (false, false) => Style::default(),
    }
}

/// A row of what the supposed world supposes: a fact, marked as differing.
fn supposed_line(fact: String) -> Line<'static> {
    let said = Span::styled("supposed: ", dim());
    Line::from(vec![said, Span::styled(fact, changed())])
}

/// What differs between the worlds: bold and underlined.
fn changed() -> Style {
    Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

/// A constraint's counts, each marked where it differs from the actual
/// world's.
fn mark_changes(spans: &mut [Span<'static>; 3], actual: &ConstraintEval, c: &ConstraintEval) {
    mark_changed(&mut spans[0], actual.violations.len() != c.violations.len());
    mark_changed(
        &mut spans[2],
        actual.disagreements.len() != c.disagreements.len(),
    );
}

/// A span marked as changed, bold and underlined, if it is.
fn mark_changed(span: &mut Span<'static>, is_changed: bool) {
    if is_changed {
        span.style = span.style.patch(changed());
    }
}

/// A constraint's counts: its violations, then its disagreements.
fn finding_counts(c: &ConstraintEval) -> [Span<'static>; 3] {
    [
        tinted_count(c.violations.len(), "violation", violation()),
        Span::raw(", "),
        tinted_count(c.disagreements.len(), "disagreement", disagreement()),
    ]
}

/// A count of findings, in their colour when not zero.
fn tinted_count(n: usize, kind: &str, style: Style) -> Span<'static> {
    let style = if n > 0 { style } else { Style::default() };
    Span::styled(counted(n, kind), style)
}

/// The entries whose facts the actual world states and the supposed one
/// does not.
/// The constraints each of these entries was set aside under, from the
/// retraction of it on the current branch that records them.
fn reasons_of(app: &App, set_aside: &[u64]) -> BTreeMap<u64, Vec<u64>> {
    let branch = app.notebook.branch(app.notebook.tip);
    let reasons = branch.entries.iter().filter_map(|e| recorded_reason(e));
    reasons.filter(|(t, _)| set_aside.contains(t)).collect()
}

/// A retraction's target and the constraints it records, if it records
/// any.
fn recorded_reason(e: &format::record::Entry) -> Option<(u64, Vec<u64>)> {
    let body = e.body().filter(|_| e.kind() == "retract")?;
    let target = body["target"].as_u64()?;
    let constraints = body.get("constraints")?.as_array()?;
    Some((
        target,
        constraints.iter().filter_map(|c| c.as_u64()).collect(),
    ))
}

fn set_aside(worlds: &[Side; 2]) -> Vec<u64> {
    let kept = worlds[1].fact_seqs();
    let actual = worlds[0].fact_seqs();
    actual.into_iter().filter(|s| !kept.contains(s)).collect()
}

/// A count and its noun: "1 violation", "2 violations".
fn counted(n: usize, noun: &str) -> String {
    let s = if n == 1 { "" } else { "s" };
    format!("{n} {noun}{s}")
}

/// A rule laid out as in the design: the head, then one literal of the body
/// per line, split only at commas outside brackets.
fn rule_lines(rule: &str) -> Vec<String> {
    let (head, body) = match rule.split_once(":-") {
        Some((h, b)) => (format!("{} :-", h.trim()), b.trim()),
        None => return vec![rule.to_string()],
    };
    let mut out = vec![head];
    let (mut depth, mut cur) = (0i32, String::new());
    for ch in body.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            _ => {}
        }
        if ch == ',' && depth == 0 {
            out.push(format!("  {},", cur.trim()));
            cur.clear();
        } else {
            cur.push(ch);
        }
    }
    out.push(format!("  {}", cur.trim()));
    out
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
    format!("hold {leap} (Leap) and type, to jump to an item with that text · {back} jumps back")
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

    #[test]
    fn what_lies_beyond_the_window_is_counted_in_items() {
        // Catches an item partly shown counted as beyond, the last item
        // in view running on past the foot said nothing of, or items not
        // drawn left out of the count.
        let rows = [(0, 2), (2, 3), (3, 9), (9, 10)];
        let b = Beyond::of(&rows, (2, 5));
        assert_eq!((b.above, b.below, b.cut), (1, 1, true));
        assert_eq!(
            beyond_text(Beyond::of(&rows, (2, 5)).skipping((25, 0))),
            "↑ 26 items above · ↓ 1 item below"
        );
        let cut = Beyond {
            above: 0,
            below: 0,
            cut: true,
        };
        assert_eq!(beyond_text(cut), "↓ continues below");
    }
    use view::Ink;

    #[test]
    fn a_rule_that_reaches_itself_is_read_once() {
        // Catches an endless walk of a journal's recursive rules, which
        // the rule command refuses but a journal may hold.
        let call = |n: &str| Term::Atom(n.into());
        let rule = |h, b| Term::Compound("rule_def".into(), vec![Term::Int(1), call(h), call(b)]);
        let e = WorldEval {
            facts: vec![(1, rule("a", "b")), (2, rule("b", "a"))],
            ..WorldEval::default()
        };
        let mut reads = vec![];
        predicates(&e, &call("a"), &mut reads);
        assert_eq!(reads, ["a", "b"]);
    }

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
