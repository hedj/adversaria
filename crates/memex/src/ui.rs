//! Drawing the screen from the interface state (docs/design.md § A user
//! story, scene 1). Drawing reads the state and changes nothing, so the same
//! state always draws the same screen.

mod wrap;

use crate::actions::command_help;
use crate::app::{Active, App, Click, Mode, PaneKind, Quasi, ScopeLevel, View};
use crate::outline::{Layer, Link, Outline, Run, TOP};
use crate::reason::{
    self, ConstraintEval, Finding, Prov, Solution, Sourced, Suggestion, WorldEval,
};
use crate::view;
use format::terms::Term;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use std::collections::BTreeMap;
use wrap::{Wrapped, wrap};

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
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
fn mark_line(indent: &str, (loud, m): (bool, String)) -> Line<'static> {
    let sign = if loud { "‼" } else { "?" };
    Line::styled(format!("{indent}  {sign} {m}"), finding_style(loud))
}

/// A finding's style, by whether it is a violation.
fn finding_style(loud: bool) -> Style {
    if loud { violation() } else { disagreement() }
}

fn bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

/// Where a click lands: the places drawn that a click acts on, in the
/// frame just drawn.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Hits(pub Vec<(Rect, Click)>);

impl Hits {
    /// What a click at (x, y) would act on, if anything.
    pub fn at(&self, x: u16, y: u16) -> Option<&Click> {
        self.0
            .iter()
            .find(|(r, _)| r.contains(Position { x, y }))
            .map(|(_, c)| c)
    }
    /// The clicks outside an area.
    fn outside(mut self, area: Rect) -> Hits {
        self.0.retain(|(r, _)| r.intersection(area).is_empty());
        self
    }

    fn add(&mut self, area: Rect, r: Rect, c: Click) {
        let r = r.intersection(area);
        if r.width > 0 && r.height > 0 {
            self.0.push((r, c));
        }
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    draw_hits(f, app);
}

/// Draw the screen, and say where clicks would land.
pub fn draw_hits(f: &mut Frame, app: &App) -> Hits {
    let body = draw_edges(f, app);
    let hits = draw_body(f, app, body);
    match MenuBox::open(app) {
        Some(menu) => menu.draw(f, hits),
        None => hits,
    }
}

/// What a right-click offers, as drawn at its cell: each entry's line,
/// whether it is the one chosen, and what a click on it runs.
struct MenuBox {
    entries: Vec<(String, bool, Click)>,
    at: (u16, u16),
}

impl MenuBox {
    /// The menu, if one is open.
    fn open(app: &App) -> Option<MenuBox> {
        let Mode::Menu(m) = &app.session.mode else {
            return None;
        };
        let entry = |(text, chosen, line)| (text, chosen, Click::Command(line));
        let entries = m.entries().into_iter().map(entry).collect();
        Some(MenuBox { entries, at: m.at })
    }

    /// The menu in a box below its cell, or above it where there is no
    /// room below, inside the screen; its entries are clicked first.
    fn draw(&self, f: &mut Frame, under: Hits) -> Hits {
        let area = placed(f.area(), self.at, self.size(f.area()));
        f.render_widget(Clear, area);
        let block = Block::default().borders(Borders::ALL).title(" act on ");
        f.render_widget(Paragraph::new(self.lines()).block(block), area);
        let mut hits = self.hits(area);
        hits.0.extend(under.outside(area).0);
        hits
    }

    /// Each entry's line, cut to 60 characters, the chosen one reversed.
    fn lines(&self) -> Vec<Line<'static>> {
        let line = |(text, chosen, _): &(String, bool, Click)| {
            let shown = format!(" {} ", shortened(text, 62));
            let style = if *chosen {
                reversed()
            } else {
                Style::default()
            };
            Line::styled(shown, style)
        };
        self.entries.iter().map(line).collect()
    }

    /// The box's width and height, with its border, within the screen.
    fn size(&self, screen: Rect) -> (u16, u16) {
        let widest = self.lines().iter().map(Line::width).max().unwrap_or(0);
        let w = (widest as u16 + 2).min(screen.width);
        (w, (self.entries.len() as u16 + 2).min(screen.height))
    }

    /// Where a click on each entry that fits lands: its row in the box.
    fn hits(&self, area: Rect) -> Hits {
        let fits = area.height.saturating_sub(2) as usize;
        let each = self.entries.iter().take(fits).enumerate();
        Hits(
            each.map(|(i, e)| (entry_row(area, i), e.2.clone()))
                .collect(),
        )
    }
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
/// below.
fn top_of(height: u16, y: u16, h: u16) -> u16 {
    if y + 1 + h <= height {
        y + 1
    } else {
        y.saturating_sub(h)
    }
}

/// A text cut to so many characters, with an ellipsis if it was longer.
fn shortened(text: &str, most: usize) -> String {
    match text.chars().count() > most {
        true => text.chars().take(most - 1).chain(['…']).collect(),
        false => text.to_string(),
    }
}

/// Draw the top bar and the foot, each as many rows as its lines take
/// wrapped; the rows between them: the body's.
fn draw_edges(f: &mut Frame, app: &App) -> Rect {
    let edges = Edges::of(app, f.area().width);
    let [top, body, foot @ ..] = screen_areas(f.area(), &edges);
    edges.draw(f, top, foot);
    body
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
fn draw_body(f: &mut Frame, app: &App, area: Rect) -> Hits {
    if app.reason.supposition.is_some() {
        draw_worlds(f, app, area);
        return Hits::default();
    }
    draw_panes(f, app, area)
}

/// The two panes, and a quasimode's list over the left one: no click
/// lands on what it covers.
fn draw_panes(f: &mut Frame, app: &App, area: Rect) -> Hits {
    let [left, right] = pane_areas(area);
    let mut hits = draw_pane(f, app, 1, right);
    hits.0.extend(draw_pane(f, app, 0, left).0);
    match app.session.listed() {
        Some(a) => cover(f, app, a, left, hits),
        None => hits,
    }
}

/// The body's panes: three fifths of it, then two fifths.
fn pane_areas(area: Rect) -> [Rect; 2] {
    let halves = [Constraint::Percentage(60), Constraint::Percentage(40)];
    Layout::horizontal(halves).areas(area)
}

/// Pane i, as its kind is drawn.
fn draw_pane(f: &mut Frame, app: &App, i: usize, area: Rect) -> Hits {
    match app.session.view.pane_kind(i) {
        PaneKind::Outline => draw_outline(f, app, i, area),
        PaneKind::Links => draw_links(f, app, area),
        PaneKind::Suggestions => draw_suggestions(f, app, area),
    }
}

/// A quasimode's list over an area, and the clicks outside it.
fn cover(f: &mut Frame, app: &App, a: &Active, area: Rect, hits: Hits) -> Hits {
    draw_list(f, app, a, area);
    hits.outside(area)
}

/// The top bar: on the left the room, its world and the journal; on the
/// right what reasoning found, or the pane in focus. When the right does
/// not fit beside the left, it takes a row of its own.
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

/// The top bar's left: the room, its world or worlds, and how many
/// entries the journal holds.
fn top_left(app: &App) -> Line<'static> {
    let mut spans = vec![
        Span::styled("room ", dim()),
        Span::styled(app.journal().room(), bold()),
        Span::raw("   "),
    ];
    spans.extend(world_spans(app));
    let n = app.journal().entries().len();
    spans.extend([
        Span::raw("   "),
        Span::styled("journal ", dim()),
        Span::styled(n.to_string(), bold()),
    ]);
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

/// The top bar's right: the world's status, else the pane with focus,
/// The top bar's right, dim: what reasoning found, or else the pane in
/// focus.
fn top_right(app: &App) -> Line<'static> {
    match app.reason.eval[0].as_ref().map(status) {
        Some(found) if !found.spans.is_empty() => found,
        _ => Line::styled(format!("pane {} of 2", app.session.view.focus + 1), dim()),
    }
}

/// The right pane for a view: its layers, each with its goal and how many
/// marks it gives, and the view's notes.
fn view_panel(app: &App, id: u64, layers: &[Layer]) -> Vec<Line<'static>> {
    let mut lines = vec![layers_heading(layers), Line::raw("")];
    lines.extend(layers.iter().flat_map(|l| layer_lines(app, l)));
    lines.extend(view_notes(app, id));
    let hint = "a click on a mark goes to the item that states its fact";
    lines.extend([Line::raw(""), Line::styled(hint, dim())]);
    lines
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
fn item_spans(app: &App, id: u64, focused: bool) -> Vec<Span<'static>> {
    let mut spans = item_text(app, id, focused);
    spans.extend(item_notes(&app.notebook.outline, id));
    spans
}

/// An item's text as drawn: derived text filled in; else, with focus at
/// the cursor in word scope, the text underlined as the target; else the
/// text as typed, if it is not yet written; else its runs.
fn item_text(app: &App, id: u64, focused: bool) -> Vec<Span<'static>> {
    if let Some(derived) = derived_spans(app, id) {
        return derived;
    }
    let target = focused && app.session.view.in_word();
    let typed: String = app.text_of(id).iter().collect();
    if target {
        return vec![Span::styled(
            typed,
            Style::default().add_modifier(Modifier::UNDERLINED),
        )];
    }
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
    match app.notebook.outline.names.get("title") {
        Some(id) => format!(
            "OUTLINE · {}",
            app.notebook.outline.text(*id).to_uppercase()
        ),
        None => "OUTLINE".into(),
    }
}

/// An outline pane: its frame and title, then every line of the outline
/// near the cursor, and the window onto them that keeps the cursor's line
/// in view.
fn draw_outline(f: &mut Frame, app: &App, pane: usize, area: Rect) -> Hits {
    let rows = outline_frame(f, app, pane, area);
    let outline = OutlinePane { app, pane, rows };
    outline.drawn().show(f, &outline)
}

/// An outline pane's frame, ruled off from the pane on its right, and its
/// title: the area left for its rows.
fn outline_frame(f: &mut Frame, app: &App, pane: usize, area: Rect) -> Rect {
    let inner = draw_frame(f, pane, area);
    let [head, rows] = title_and_rows(inner);
    f.render_widget(dim_text(title(app)), head);
    rows
}

/// A pane's frame, the first ruled off from the second: the area inside.
fn draw_frame(f: &mut Frame, pane: usize, area: Rect) -> Rect {
    let border = if pane == 0 {
        Borders::RIGHT
    } else {
        Borders::NONE
    };
    let block = Block::default().borders(border);
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
}

impl<'a> OutlinePane<'a> {
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

    /// Whether the pane has the focus, outside a dialogue: the cursor's
    /// item shows its target.
    fn focused(&self) -> bool {
        self.has_focus() && matches!(self.app.session.mode, Mode::Normal | Mode::Quasi(_))
    }

    /// The lines of the items within a screen's height of the cursor's:
    /// each item takes at least a line, so no other can be in the window,
    /// and a long outline is not drawn whole on every keystroke.
    fn drawn(&self) -> Drawn {
        let rows = self.app.visible();
        let mut drawn = Drawn::default();
        for (id, depth) in self.near(&rows) {
            self.item(&mut drawn, *id, *depth);
        }
        if rows.is_empty() {
            let empty = "(empty: type to write the first item)";
            drawn.push(Line::styled(empty, dim()), 0);
        }
        drawn
    }

    /// The rows within a screen's height of the cursor's row, either side.
    fn near<'r>(&self, rows: &'r [(u64, usize)]) -> &'r [(u64, usize)] {
        near(rows, self.cursor(), self.rows.height as usize)
    }

    /// An item's lines: the item, a click on all of it; its marks; and its
    /// view, if it is one.
    fn item(&self, drawn: &mut Drawn, id: u64, depth: usize) {
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
        drawn.push_lines(self.mark_lines(id, depth), set_in + 2);
        if self.outline().views.contains_key(&id) {
            self.view_lines(drawn, id, 2 * depth + 2);
        }
    }

    /// An item's line: its place in the tree, its text, how many items a
    /// fold hides; reversed at the cursor.
    fn item_line(&self, id: u64, depth: usize) -> Line<'static> {
        let at_cursor = id == self.cursor();
        let mut spans = self.item_head(id, depth);
        spans.extend(item_spans(self.app, id, at_cursor && self.focused()));
        spans.extend(self.folded_count(id));
        let line = Line::from(spans);
        if at_cursor {
            line.style(self.cursor_style())
        } else {
            line
        }
    }

    /// How many cells an item's head takes: the column its text starts at,
    /// under which its wrapped rows start.
    fn head_width(&self, id: u64, depth: usize) -> usize {
        Line::from(self.item_head(id, depth)).width()
    }

    /// An item's indent and its mark of the tree; at the top level, its
    /// number.
    fn item_head(&self, id: u64, depth: usize) -> Vec<Span<'static>> {
        let indent = Span::raw("  ".repeat(depth));
        let mut spans = vec![indent, self.bullet(id)];
        if depth == 0 {
            spans.push(Span::raw(format!("{}. ", self.outline().number(id))));
        }
        spans
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

    /// The cursor's line reversed; bold too in branch scope, with focus.
    fn cursor_style(&self) -> Style {
        let reversed = Style::default().add_modifier(Modifier::REVERSED);
        let branch = self.view().scope == ScopeLevel::Branch;
        if branch && self.focused() {
            reversed.add_modifier(Modifier::BOLD)
        } else {
            reversed
        }
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
        let hint = self.hint_here(id)?;
        let indent = "  ".repeat(depth + 1);
        Some(Line::styled(format!("{indent}  {hint}"), dim()))
    }

    /// Under the item at the cursor, its [[links]] that name no item yet,
    /// and how to make them.
    fn unmade_line(&self, id: u64, depth: usize) -> Option<Line<'static>> {
        let names = self.unmade_here(id)?;
        let indent = "  ".repeat(depth + 1);
        let hint = format!(
            "{indent}  not yet items: {} · make makes them",
            names.join(", ")
        );
        Some(Line::styled(hint, dim()))
    }

    /// The [[links]] of the item at the cursor that name no item, if it is
    /// this item and has any.
    fn unmade_here(&self, id: u64) -> Option<Vec<String>> {
        let names = Some(id)
            .filter(|i| *i == self.cursor())
            .map(|i| self.outline().unresolved(i))?;
        (!names.is_empty()).then_some(names)
    }

    /// The hint of an item, if it is the item at the cursor.
    fn hint_here(&self, id: u64) -> Option<&str> {
        let here = id == self.cursor();
        self.app.session.prose_hint(id).filter(|_| here)
    }

    /// An item's marks, each under it, set in: bold if loud, else dim.
    fn mark_lines(&self, id: u64, depth: usize) -> Vec<Line<'static>> {
        let indent = "  ".repeat(depth + 1);
        let line = |mark| mark_line(&indent, mark);
        self.marks_of(id).into_iter().map(line).collect()
    }

    /// An item's marks: each finding on its facts, then, if its inputs
    /// changed since it was made, which.
    fn marks_of(&self, id: u64) -> Vec<(bool, String)> {
        let found = self.app.reason.eval[0].as_ref();
        let mut marks = found
            .map(|e| item_marks(e, self.outline(), id))
            .unwrap_or_default();
        marks.extend(self.stale_mark(id));
        marks
    }

    /// The loud mark of an item whose input files changed since it was
    /// made: which.
    fn stale_mark(&self, id: u64) -> Option<(bool, String)> {
        let files = self.app.session.stale().get(&id)?.join(", ");
        Some((true, format!("input changed since: {files}")))
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

    /// A mark of a view a click, on each of its lines, if an item states
    /// its fact.
    fn hit_places(&self, hit: &view::Hit, at: (usize, usize)) -> Option<Vec<Place>> {
        let click = self.click(hit_item(self.app, hit)?);
        let lines = hit.area.lines_at(at);
        Some(
            lines
                .into_iter()
                .map(|l| Place::of(l, click.clone()))
                .collect(),
        )
    }

    /// A click that selects an item in this pane.
    fn click(&self, id: u64) -> Click {
        Click::Item {
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
    /// mark and, at the top level, number, and the text before it.
    fn text_column(&self, depth: usize) -> usize {
        let number = match depth {
            0 => self.outline().number(self.cursor()).len() + 2,
            _ => 0,
        };
        2 * depth + 2 + number + self.typed_cells()
    }

    /// How many cells the text before the text cursor takes.
    fn typed_cells(&self) -> usize {
        cells(&self.app.text_of(self.cursor()), self.view().pos)
    }
}

/// The rows within a screen's height of the cursor's row, either side.
fn near(rows: &[(u64, usize)], cursor: u64, height: usize) -> &[(u64, usize)] {
    let at = rows.iter().position(|(id, _)| *id == cursor).unwrap_or(0);
    let first = at.saturating_sub(height);
    &rows[first..rows.len().min(first + 2 * height + 1)]
}

/// The lines of an outline pane, before they are wrapped and their window
/// is chosen: each line and the indent of its wrapped rows, each place a
/// click acts on, and the cursor's line and depth.
#[derive(Default)]
struct Drawn {
    lines: Vec<(Line<'static>, usize)>,
    places: Vec<Place>,
    cursor: Option<(usize, usize)>,
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
    click: Click,
}

impl Place {
    /// A place on a line: the line, its first column and its width.
    fn of((line, x, width): (usize, usize, usize), click: Click) -> Place {
        let span = (x, width);
        Place { line, span, click }
    }

    /// A click on each row of it in the window.
    fn add_to(&self, hits: &mut Hits, laid: &Laid, window: Window) {
        let spans = laid.spans(self.line, self.reach());
        window.add(hits, spans, &self.click);
    }

    /// The columns it covers on its line: [from, to).
    fn reach(&self) -> (usize, usize) {
        let (x, width) = self.span;
        (x, x.saturating_add(width))
    }
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
    fn add(&self, hits: &mut Hits, spans: Vec<(usize, usize, usize)>, click: &Click) {
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

    /// Draw the rows in the window.
    fn draw(&self, f: &mut Frame, rows: Vec<Line<'static>>) {
        let height = self.area.height as usize;
        let shown: Vec<Line> = rows.into_iter().skip(self.offset).take(height).collect();
        f.render_widget(Paragraph::new(shown), self.area);
    }
}

impl Drawn {
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
    fn show(self, f: &mut Frame, outline: &OutlinePane) -> Hits {
        let laid = Laid::new(&self.lines, outline.rows.width as usize);
        let cursor = self.text_place(&laid, outline);
        let window = Window::over(outline.rows, cursor);
        outline.put_text_cursor(f, window.place(cursor));
        let hits = self.hits(&laid, window);
        window.draw(f, laid.rows());
        hits
    }

    /// Where the text cursor is drawn among the rows: its row and column.
    fn text_place(&self, laid: &Laid, outline: &OutlinePane) -> Option<(usize, usize)> {
        let (line, depth) = self.cursor?;
        Some(laid.place(line, outline.text_column(depth)))
    }

    /// The places in a window, each where it is drawn.
    fn hits(&self, laid: &Laid, window: Window) -> Hits {
        let mut hits = Hits::default();
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

/// The item a click on a mark goes to: the item stating the first fact
/// the mark rests on.
fn hit_item(app: &App, hit: &view::Hit) -> Option<u64> {
    hit.prov.iter().find_map(|s| {
        // A fact from the outline itself rests on the item's own entry.
        if app.notebook.outline.items.contains_key(s) {
            Some(*s)
        } else {
            app.notebook.outline.fact_item.get(s).copied()
        }
    })
}

fn inverse(kind: &str) -> String {
    match kind {
        "cites" => "cited by".into(),
        "supports" => "supported by".into(),
        "opposes" => "opposed by".into(),
        k => format!("{k} (from)"),
    }
}

/// The links pane: the interrogation, if one is open; else what the
/// selected item needs (right_panel).
fn draw_links(f: &mut Frame, app: &App, area: Rect) -> Hits {
    let inner = inset(area);
    if let Some(item) = app.session.view.interrogate {
        return draw_interrogation(f, app, item, inner);
    }
    f.render_widget(right_panel(app, app.session.view.selected()), inner);
    Hits::default()
}

/// What the selected item needs: its view's layers, the constraints its
/// marks come from, or its links.
fn right_panel(app: &App, id: u64) -> Paragraph<'static> {
    if let Some(layers) = app.notebook.outline.views.get(&id) {
        return wrapped(view_panel(app, id, layers));
    }
    let mut lines = match app.reason.eval[0].as_ref() {
        Some(e) if !item_marks(e, &app.notebook.outline, id).is_empty() => {
            let outline = &app.notebook.outline;
            ConstraintPane { outline, e }.of(id)
        }
        _ => links_lines(&app.notebook.outline, id),
    };
    lines.extend(read_as_lines(&app.notebook.outline, id));
    wrapped(lines)
}

/// What an item's text, as it is now, was read as, in Prolog: each
/// statement by its kind, as the reader wrote it, and its #tags.
fn read_as_lines(outline: &Outline, id: u64) -> Vec<Line<'static>> {
    let all = read_as(outline, id);
    if all.is_empty() {
        return vec![];
    }
    let heading = [Line::raw(""), Line::styled("READ AS", dim())];
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

/// An interrogation, and under it the hint of what adopts and what takes
/// an adoption back, each part a click on its command.
fn draw_interrogation(f: &mut Frame, app: &App, item: u64, inner: Rect) -> Hits {
    let lines = interrogation(app, item);
    let used = wrapped_height(&lines, inner.width);
    f.render_widget(wrapped(lines), inner);
    draw_adopt_hint(f, app, row_below(inner, used))
}

/// The row of an area after its first rows: none high if it is outside.
fn row_below(area: Rect, used: u16) -> Rect {
    let y = area.y.saturating_add(used);
    let height = u16::from(y < area.bottom());
    Rect { y, height, ..area }
}

/// The hint "F3 adopts · F4 retracts an adoption", by the keys bound,
/// each part a click on its command.
fn draw_adopt_hint(f: &mut Frame, app: &App, row: Rect) -> Hits {
    let adopt = hint_part(app, "adopt", "adopts");
    let parts = [adopt, hint_part(app, "unadopt", "retracts an adoption")];
    f.render_widget(dim_text(joined(&parts)), row);
    let mut along = Along::new(row);
    parts
        .into_iter()
        .for_each(|(text, click)| along.part(&text, click));
    along.hits
}

/// A part of a hint: the key bound to a command and what it does, and a
/// click on the command.
fn hint_part(app: &App, command: &str, does: &str) -> (String, Click) {
    let key = app.session.keys.key_for(command);
    (format!("{key} {does}"), Click::Command(command.into()))
}

/// The texts of a hint's parts, " · " between them.
fn joined(parts: &[(String, Click)]) -> String {
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
    hits: Hits,
}

impl Along {
    fn new(row: Rect) -> Self {
        let hits = Hits::default();
        Along {
            row,
            x: row.x,
            hits,
        }
    }

    /// A part's text a click, then " · " before the next part.
    fn part(&mut self, text: &str, click: Click) {
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
fn links_lines(outline: &Outline, id: u64) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::styled("LINKS OF THE SELECTED ITEM", dim()),
        Line::raw(""),
    ];
    if id == TOP {
        return lines;
    }
    lines.extend([Line::raw(outline.text(id)), Line::raw("")]);
    lines.extend(link_lines(outline, id));
    lines.extend(transclusion_lines(outline, id));
    lines
}

/// An item's links, under a heading for each kind, numbered in order.
fn link_lines(outline: &Outline, id: u64) -> Vec<Line<'static>> {
    let rows = numbered_links(outline, id);
    let groups = rows.chunk_by(|a, b| a.0 == b.0);
    groups.flat_map(kind_lines).collect()
}

/// A heading of a kind of link, and its links.
fn kind_lines(group: &[(String, String)]) -> Vec<Line<'static>> {
    let heading = Line::styled(group[0].0.clone(), dim());
    let links = group.iter().map(|(_, link)| Line::raw(link.clone()));
    std::iter::once(heading).chain(links).collect()
}

/// Each of an item's links: its kind's heading, and the item at its other
/// end with its number for follow.
fn numbered_links(outline: &Outline, id: u64) -> Vec<(String, String)> {
    let numbered = link_ends(outline, id).into_iter().enumerate();
    let line = |n: usize, end| format!("{}  {}", labelled_end(outline, end), n + 1);
    numbered
        .map(|(n, (kind, end))| (kind, line(n, end)))
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
/// under the item that holds it.
fn transclusion_lines(outline: &Outline, id: u64) -> Vec<Line<'static>> {
    let shown_in = outline.transcluded_in(id);
    if shown_in.is_empty() {
        return vec![];
    }
    let places = shown_in
        .into_iter()
        .map(|t| Line::raw(transcluded_at(outline, t)));
    std::iter::once(Line::styled("transcluded in ←", dim()))
        .chain(places)
        .collect()
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
fn draw_list(f: &mut Frame, app: &App, a: &Active, area: Rect) {
    let lines = list_shown(app, a, area);
    let r = foot_box(area, &lines);
    f.render_widget(Clear, r);
    f.render_widget(boxed(lines), r);
}

/// Lines wrapped in a box with borders all round.
fn boxed(lines: Vec<Line<'static>>) -> Paragraph<'static> {
    wrapped(lines).block(Block::default().borders(Borders::ALL))
}

/// What a quasimode's list shows in its box at the foot of an area: as
/// much of the list as fits.
fn list_shown(app: &App, a: &Active, area: Rect) -> Vec<Line<'static>> {
    fitted(list_lines(app, a), area)
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
fn list_lines(app: &App, a: &Active) -> Vec<Line<'static>> {
    match a.q() {
        Quasi::Command => command_help(&a.input).into_iter().map(Line::raw).collect(),
        Quasi::Suppose => suppose_lines(a),
        q => leap_lines(app, a, q == Quasi::LeapForward),
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
fn leap_lines(app: &App, a: &Active, forward: bool) -> Vec<Line<'static>> {
    let matches = app.leap_matches(&a.input, forward);
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

/// Where a pattern is in a text, ignoring case: its start and end, if
/// both fall between characters of the text as written (a letter's small
/// form can take more bytes, as "İ" does).
fn match_at(text: &str, pattern: &str) -> Option<(usize, usize)> {
    let (lt, p) = (text.to_lowercase(), pattern.to_lowercase());
    let i = lt.find(&p).filter(|_| lt.len() == text.len())?;
    let j = i + p.len();
    (text.is_char_boundary(i) && text.is_char_boundary(j)).then_some((i, j))
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

/// What a held quasimode does, and how it ends.
fn quasi_guide(a: &Active) -> String {
    let end = release_text(a);
    match a.q() {
        Quasi::LeapForward | Quasi::LeapBack => format!(
            "{end} to go to the first match · Esc completes · ? lists every match · Leap again repeats"
        ),
        Quasi::Command => {
            format!("{end} to run on the target · Esc completes · ? lists what may come next")
        }
        Quasi::Scope => {
            format!("Space widens the target: word, statement, branch · {end} keeps it")
        }
        Quasi::Suppose => match a.hint().and_then(|h| h.said.as_ref()) {
            None => format!(
                "type a statement · ? lists what may come next · {end} offers its readings, or a world of that name if none"
            ),
            Some(said) => format!("{said} · {end} offers its readings"),
        },
    }
}

/// How a quasimode ends: by Enter or a second tap when latched, else by
/// its key's release.
fn release_text(a: &Active) -> String {
    if a.latched() {
        return "Enter, or tap again,".into();
    }
    let name = match a.q() {
        Quasi::Command => "Command",
        Quasi::Scope => "Scope",
        Quasi::Suppose => "Suppose",
        Quasi::LeapForward | Quasi::LeapBack => "Leap",
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

/// The foot in normal mode: a hint, while there is no message; else the
/// message.
fn normal_foot(app: &App) -> (Line<'static>, String) {
    match hint(app) {
        Some((p, g)) if app.session.message.is_empty() => (Line::raw(p), g),
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
pub fn item_marks(e: &WorldEval, outline: &Outline, item: u64) -> Vec<(bool, String)> {
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

/// A mark in parts: whether it is loud, what it says of the item's own
/// fact, and the other facts it is against, each with where it is.
struct Mark {
    loud: bool,
    head: String,
    rest: Vec<String>,
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
    fn text(self) -> (bool, String) {
        match self.rest.is_empty() {
            true => (self.loud, self.head),
            false => (
                self.loud,
                format!("{}; {}", self.head, self.rest.join("; ")),
            ),
        }
    }
}

/// Marks that say the same of the item, one each: the facts they are
/// against, in order, each once. So a rule the item breaks with others
/// is one mark, not one for each other fact or each solution.
fn merged(marks: Vec<Mark>) -> Vec<(bool, String)> {
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
    let states = |i: &u64| outline.text(*i).trim_end_matches('.') == name;
    let item = outline.in_order().into_iter().find(states);
    item.map_or_else(
        || format!("“{name}”"),
        |i| format!("rule {}", outline.number(i)),
    )
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
    let spans: Vec<Span<'static>> = shown.map(count_span).collect();
    Line::from(interleaved(spans, Span::styled(" · ", dim())))
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

/// Spans with a separator between each two.
fn interleaved(spans: Vec<Span<'static>>, sep: Span<'static>) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for (i, s) in spans.into_iter().enumerate() {
        if i > 0 {
            out.push(sep.clone());
        }
        out.push(s);
    }
    out
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
    fn of(&self, item: u64) -> Vec<Line<'static>> {
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
    fn constraint(&self, c: &ConstraintEval) -> Vec<Line<'static>> {
        let found = self.found(c);
        [constraint_head(c), found, self.dependents(c)].concat()
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
        let heading = format!("{title} · recomputed at journal {}", self.e.at);
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

/// The right pane when a claim is interrogated (one key, F2): each of the
/// item's claims, then what adopting them would do. The hint follows,
/// drawn by draw_interrogation, where a click can find it.
fn interrogation(app: &App, item: u64) -> Vec<Line<'static>> {
    let Some(e) = app.reason.eval[0].as_ref() else {
        return vec![Line::raw("not yet evaluated")];
    };
    let mut lines = interrogation_head(&app.notebook.outline, item);
    let claims = app.claims_of(item);
    lines.extend(claims.into_iter().flat_map(|c| claim_lines(app, e, c)));
    lines.extend(adoption_lines(app));
    lines
}

/// The interrogation's heading, and the item interrogated.
fn interrogation_head(outline: &Outline, item: u64) -> Vec<Line<'static>> {
    let heading = Line::styled("INTERROGATE · WHAT IS CLAIMED HERE", dim());
    vec![
        heading,
        Line::raw(""),
        Line::raw(outline.text(item)),
        Line::raw(""),
    ]
}

/// A claim interrogated: what it says and its entry, who else claims it,
/// who claims the contrary, and its source's record.
fn claim_lines(app: &App, e: &WorldEval, claim: (u64, String, Term)) -> Vec<Line<'static>> {
    let asking = Asking::of(app, e, &claim);
    let mut lines = asking.said();
    lines.extend(section(
        "who else claims it",
        asking.claimants(),
        "no one else",
    ));
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

    /// What the claim says, restated, with its source, and its entry.
    fn said(&self) -> Vec<Line<'static>> {
        let (seq, src, p) = self.claim;
        let said = format!("claims: {} — from {src}", self.restated(p));
        vec![
            Line::raw(said),
            Line::styled(format!("  entry {seq}"), dim()),
        ]
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
fn adoption_lines(app: &App) -> [Line<'static>; 3] {
    let preview = app
        .reason
        .preview
        .clone()
        .unwrap_or_else(|| "not yet computed".into());
    [
        Line::styled("if adopted", dim()),
        Line::raw(format!("  {preview}")),
        Line::raw(""),
    ]
}

/// Fading hint: while the cursor is on an item with marks, how to try the
/// other reading; it stops once Suppose has been used four times.
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
    let left = 4usize.checked_sub(used).filter(|n| *n > 0)?;
    Some((
        "Hold SUPPOSE and type a statement to try it in a new world. Nothing in this world changes.".into(),
        format!("fading hint · shown until Suppose has been used {left} more time{}", if left == 1 { "" } else { "s" }),
    ))
}

/// A supposition: the actual world and the supposed one side by side, row
/// for row; or a revision, as its own pane draws it.
fn draw_worlds(f: &mut Frame, app: &App, area: Rect) {
    let Some(sup) = &app.reason.supposition else {
        return;
    };
    match sup.name.starts_with("revision of ") {
        true => draw_revision(f, app, area),
        false => draw_sides(f, app, area),
    }
}

/// The open supposition side by side, if there is one.
fn draw_sides(f: &mut Frame, app: &App, area: Rect) {
    if let Some(sides) = Sides::of(app) {
        sides.draw(f, area);
    }
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
fn side_by_side(columns: [Vec<Line<'static>>; 2], widths: [u16; 2]) -> [Vec<Line<'static>>; 2] {
    let pairs: Vec<_> = line_pairs(&columns)
        .map(|l| padded_pair(l, widths))
        .collect();
    [0, 1].map(|k| pairs.iter().flat_map(|p| p[k].clone()).collect())
}

/// The columns' lines, pair by pair: the ith of each, or none where a
/// column is shorter.
fn line_pairs<'a>(
    columns: &'a [Vec<Line<'static>>; 2],
) -> impl Iterator<Item = [Option<&'a Line<'static>>; 2]> {
    let n = columns[0].len().max(columns[1].len());
    (0..n).map(move |i| columns.each_ref().map(|c| c.get(i)))
}

/// A pair of lines, one from each column, wrapped, the shorter padded to
/// the taller's rows.
fn padded_pair(lines: [Option<&Line<'static>>; 2], widths: [u16; 2]) -> [Vec<Line<'static>>; 2] {
    let mut pair = [0, 1].map(|k| wrapped_rows(lines[k], widths[k]));
    let height = pair.iter().map(Vec::len).max().unwrap_or(0);
    pair.iter_mut()
        .for_each(|rows| rows.resize(height, Line::raw("")));
    pair
}

/// A line's rows wrapped to a width, later rows set in by two; none for
/// no line.
fn wrapped_rows(line: Option<&Line<'static>>, width: u16) -> Vec<Line<'static>> {
    line.map_or_else(Vec::new, |l| wrap(l, width as usize, 2).rows)
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
        let facts = self.eval.map(|e| e.facts.as_slice()).unwrap_or_default();
        facts.iter().map(|(s, _)| *s).collect()
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
    fn columns(&self, widths: [u16; 2]) -> [Vec<Line<'static>>; 2] {
        [0, 1].map(|k| self.column(k, widths[k]))
    }

    /// A world's column: its title, its rows, its constraints, and its
    /// views.
    fn column(&self, k: usize, width: u16) -> Vec<Line<'static>> {
        let mut lines = self.rows_of(k);
        lines.extend(self.constraint_lines(k));
        lines.extend(self.view_lines(k, width));
        lines
    }

    /// A world's title, then its rows, then a blank line.
    fn rows_of(&self, k: usize) -> Vec<Line<'static>> {
        let head = [Line::styled(self.side(k).title, dim()), Line::raw("")];
        let rows = self.rows.iter().map(|i| self.row(k, *i));
        head.into_iter()
            .chain(rows)
            .chain([Line::raw("")])
            .collect()
    }

    fn row(&self, k: usize, i: u64) -> Line<'static> {
        match k {
            0 => self.actual_row(i),
            _ => self.supposed_row(i),
        }
    }

    /// A row of the actual world: the item's text there, with a dashed
    /// trail where the supposed world differs.
    fn actual_row(&self, i: u64) -> Line<'static> {
        let here = self.side(0).text(i);
        let trail = here != self.side(1).text(i);
        let trail = trail.then(|| Span::styled("  - - -", dim()));
        Line::from_iter(std::iter::once(Span::raw(here)).chain(trail))
    }

    /// A row of the supposed world: the item's text there, bold where it
    /// differs from the actual world, and whether its fact is set aside.
    fn supposed_row(&self, i: u64) -> Line<'static> {
        let here = self.side(1).text(i);
        let differs = here != self.side(0).text(i);
        let here = Span::styled(here, if differs { bold() } else { Style::default() });
        let gone = self
            .sets_aside_at(i)
            .then(|| Span::styled("  (fact retracted here)", dim()));
        Line::from_iter(std::iter::once(here).chain(gone))
    }

    /// Whether the supposed world sets aside a fact of an item of the
    /// actual world.
    fn sets_aside_at(&self, i: u64) -> bool {
        let actual = &self.actual;
        self.aside.entries.iter().any(|s| actual.states(i, *s))
    }

    /// A line for each constraint of a world: how many violations and
    /// disagreements it has, and, in the supposed world, what it sets
    /// aside.
    fn constraint_lines(&self, k: usize) -> Vec<Line<'static>> {
        let Some(e) = self.side(k).eval else {
            return vec![];
        };
        let mut lines: Vec<String> = e
            .constraints
            .iter()
            .map(|c| self.constraint_line(k, c))
            .collect();
        if k == 1 {
            lines.extend(self.unexplained_line());
        }
        lines.into_iter().map(Line::raw).collect()
    }

    fn constraint_line(&self, k: usize, c: &ConstraintEval) -> String {
        let violations = counted(c.violations.len(), "violation");
        let mut line = format!("Constraint {}: {violations}", c.constraint.name);
        if !c.disagreements.is_empty() {
            line += &format!(", {}", counted(c.disagreements.len(), "disagreement"));
        }
        let under = self.set_aside_where(|cs| cs.is_some_and(|cs| cs.contains(&c.seq)));
        if k == 1 && !under.is_empty() {
            line += &format!(" · {under} retracted here");
        }
        line
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

/// The items that state facts or show derived values: the actual
/// world's in its order, then the supposed world's that it lacks.
fn shared_rows(actual: &Outline, supposed: &Outline) -> Vec<u64> {
    let mut rows = fact_rows(actual);
    let more: Vec<u64> = fact_rows(supposed)
        .into_iter()
        .filter(|i| !rows.contains(i))
        .collect();
    rows.extend(more);
    rows
}

/// An outline's items that state facts or show derived values, in order.
fn fact_rows(o: &Outline) -> Vec<u64> {
    let stating = |i: &u64| o.fact_item.values().any(|x| x == i) || o.derived.contains_key(i);
    o.in_order().into_iter().filter(stating).collect()
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
fn draw_suggestions(f: &mut Frame, app: &App, area: Rect) -> Hits {
    let inner = inset(area);
    let column = suggestion_lines(app, Column::new(inner));
    f.render_widget(
        Paragraph::new(column.lines).wrap(Wrap { trim: false }),
        inner,
    );
    column.hits
}

/// The suggestion pane's column: how many are shown of all, each shown
/// one, and how to act on them.
fn suggestion_lines(app: &App, mut column: Column) -> Column {
    let suggestions = &app.reason.suggestions;
    let shown = suggestions.shown();
    let heading = format!(
        "SUGGESTIONS · {} OF {}",
        shown.len(),
        suggestions.list.len()
    );
    column.lines = vec![Line::styled(heading, dim()), Line::raw("")];
    let blocks = shown
        .iter()
        .enumerate()
        .map(|(i, s)| suggestion_block(i, s));
    for (block, click) in blocks {
        column.clickable(block, click);
    }
    if shown.is_empty() {
        column.lines.push(Line::styled("nothing to suggest", dim()));
    }
    let hint = "accept N adopts · dismiss N dismisses";
    column
        .lines
        .extend([Line::raw(""), Line::styled(hint, dim())]);
    column
}

/// The i-th suggestion shown, numbered from 1, with why it was offered; a
/// click on it accepts it.
fn suggestion_block(i: usize, s: &Suggestion) -> (Vec<Line<'static>>, Click) {
    let title = Line::raw(format!("{}. {}", i + 1, s.describe()));
    let why = Line::styled(format!("   {}", s.evidence), dim());
    (
        vec![title, why],
        Click::Command(format!("accept {}", i + 1)),
    )
}

/// The lines of a pane, wrapped in its area, and what a click on them
/// acts on.
struct Column {
    lines: Vec<Line<'static>>,
    area: Rect,
    hits: Hits,
}

impl Column {
    fn new(area: Rect) -> Self {
        Column {
            lines: vec![],
            area,
            hits: Hits::default(),
        }
    }

    /// Add lines that a click on any of them, wrapped lines too, acts on.
    fn clickable(&mut self, block: Vec<Line<'static>>, click: Click) {
        let rows = self.rows_of(&block);
        self.hits.add(self.area, rows, click);
        self.lines.extend(block);
    }

    /// The rows that lines would take after those already added.
    fn rows_of(&self, block: &[Line<'static>]) -> Rect {
        let top = wrapped_height(&self.lines, self.area.width);
        let height = wrapped_height(block, self.area.width);
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

/// A document revision's world: each change, one entry each, to keep or
/// retract (retract N) before the world is committed or closed.
fn draw_revision(f: &mut Frame, app: &App, area: Rect) {
    let Some(lines) = revision_panel(app) else {
        return;
    };
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// The panel of the world a supposition opened, if one is open.
fn revision_panel(app: &App) -> Option<Vec<Line<'static>>> {
    let sup = app.reason.supposition.as_ref()?;
    Some(revision_lines(&sup.name, &app.revision_changes()))
}

/// The lines of a revision's panel: its name, each change, and how to
/// keep or undo them.
fn revision_lines(name: &str, changes: &[(u64, String, bool)]) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::styled(format!("WORLD · {}", name.to_uppercase()), dim()),
        Line::raw(""),
    ];
    lines.extend(changes.iter().map(change_line));
    if changes.is_empty() {
        lines.push(Line::styled("no changes", dim()));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("retract N undoes one change here · commit makes the rest real · close leaves the actual world as it was", dim()));
    lines
}

/// One change of a revision: its entry and what it does, dim if retracted.
fn change_line((seq, what, retracted): &(u64, String, bool)) -> Line<'static> {
    let line = format!("#{seq}  {what}");
    if *retracted {
        Line::styled(format!("{line}  (retracted)"), dim())
    } else {
        Line::raw(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
