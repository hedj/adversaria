//! Drawing the screen from the interface state (docs/design.md § A user
//! story, scene 1). Drawing reads the state and changes nothing, so the same
//! state always draws the same screen.

use crate::actions::command_help;
use crate::app::{Active, App, Click, Mode, PaneKind, Quasi, ScopeLevel};
use crate::outline::{Layer, Link, Outline, Run, TOP};
use crate::reason::{
    self, ConstraintEval, Finding, Prov, Solution, Sourced, Suggestion, WorldEval,
};
use crate::view;
use format::terms::Term;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
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
    let [top, body, foot @ ..] = screen_areas(f.area());
    draw_top(f, app, top);
    let hits = draw_body(f, app, body);
    draw_prompt(f, app, foot[0], foot[1]);
    hits
}

/// The screen's rows: the top bar, the body, and the foot's two lines.
fn screen_areas(area: Rect) -> [Rect; 4] {
    let rows = [
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
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

fn draw_top(f: &mut Frame, app: &App, area: Rect) {
    f.render_widget(Paragraph::new(top_left(app)), area);
    if let Some(right) = top_right(app, area.width) {
        f.render_widget(Paragraph::new(right), area);
    }
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
/// at the right, dim, if it fits in the room the left leaves.
fn top_right(app: &App, width: u16) -> Option<Line<'static>> {
    let text = match app.reason.eval[0].as_ref().map(status) {
        Some(s) if !s.is_empty() => s,
        _ => format!("pane {} of 2", app.session.view.focus + 1),
    };
    let room = (width as usize).saturating_sub(top_left(app).width() + 1);
    let fits = text.chars().count() <= room;
    fits.then(|| Line::styled(text, dim()).right_aligned())
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
    run_spans(&app.notebook.outline, id)
}

/// A derived item's text, filled in from the current world, if it has
/// been evaluated.
fn derived_spans(app: &App, id: u64) -> Option<Vec<Span<'static>>> {
    let d = app.notebook.outline.derived.get(&id)?;
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

fn draw_outline(f: &mut Frame, app: &App, pane: usize, area: Rect) -> Hits {
    let mut hits = Hits::default();
    let block = Block::default().borders(if pane == 0 {
        Borders::RIGHT
    } else {
        Borders::NONE
    });
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [head, rows_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(inner);
    f.render_widget(Paragraph::new(Span::styled(title(app), dim())), head);
    let rows = app.visible();
    let cursor = app.session.view.panes[pane].cursor;
    let h = rows_area.height as usize;
    let focused =
        app.session.view.focus == pane && matches!(app.session.mode, Mode::Normal | Mode::Quasi(_));
    // Every line of the outline, then the window onto them: items, their
    // marks and their views are lines of different heights, so the window
    // is chosen by line, keeping the cursor's line in view.
    let mut lines: Vec<Line> = Vec::new();
    // Clickable places: (line, column, width, what).
    let mut places: Vec<(usize, usize, usize, Click)> = Vec::new();
    let mut cursor_line = None;
    let mut cursor_depth = 0;
    // Each item takes at least a line, so only the items within a screen's
    // height of the cursor's can be in the window: build those alone (a long
    // outline would otherwise be drawn whole on every keystroke).
    let at_item = rows.iter().position(|(id, _)| *id == cursor).unwrap_or(0);
    let first = at_item.saturating_sub(h);
    for (id, depth) in rows.iter().skip(first).take(2 * h + 1) {
        if *id == cursor {
            cursor_line = Some(lines.len());
            cursor_depth = *depth;
        }
        places.push((
            lines.len(),
            0,
            rows_area.width as usize,
            Click::Item { pane, id: *id },
        ));
        let kids = app.notebook.outline.children(*id).len();
        let folded = app.session.view.folded.contains(id);
        let mark = match (kids, folded) {
            (0, _) => "•",
            (_, true) => "▶",
            _ => "▼",
        };
        let mut spans = vec![
            Span::raw("  ".repeat(*depth)),
            Span::styled(format!("{mark} "), dim()),
        ];
        if *depth == 0 {
            spans.push(Span::raw(format!("{}. ", app.notebook.outline.number(*id))));
        }
        let is_cursor = *id == cursor;
        spans.extend(item_spans(app, *id, is_cursor && focused));
        if folded {
            spans.push(Span::styled(
                format!(" {}", app.notebook.outline.descendants(*id)),
                dim(),
            ));
        }
        let marks = app.reason.eval[0]
            .as_ref()
            .map(|e| item_marks(e, &app.notebook.outline, *id))
            .unwrap_or_default();
        let mut line = Line::from(spans);
        if is_cursor {
            line = line.style(if app.session.view.scope == ScopeLevel::Branch && focused {
                Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
            } else {
                Style::default().add_modifier(Modifier::REVERSED)
            });
        }
        lines.push(line);
        let stale = app
            .session
            .stale()
            .get(id)
            .map(|files| (true, format!("input changed since: {}", files.join(", "))));
        for (violation, m) in marks.into_iter().chain(stale) {
            let style = if violation {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                dim()
            };
            lines.push(Line::styled(
                format!("{}  ⚑ {m}", "  ".repeat(*depth + 1)),
                style,
            ));
        }
        // A view, drawn under its item.
        if app.notebook.outline.views.contains_key(id) {
            let indent = 2 * depth + 2;
            let width = (rows_area.width as usize).saturating_sub(indent + 1);
            let fig = view_figure(app, 0, *id, width, 12);
            let top = lines.len();
            lines.extend(figure_lines(&fig, indent));
            for hit in &fig.hits {
                if let Some(item) = hit_item(app, hit) {
                    for dy in 0..hit.area.h {
                        places.push((
                            top + hit.area.y + dy,
                            indent + hit.area.x,
                            hit.area.w,
                            Click::Item { pane, id: item },
                        ));
                    }
                }
            }
        }
    }
    if rows.is_empty() {
        lines.push(Line::styled("(empty: type to write the first item)", dim()));
    }
    // Two lines below the cursor's are kept in view where there is room,
    // but never at the cost of the cursor's own.
    let at = cursor_line.unwrap_or(0);
    let offset = (at + 3)
        .saturating_sub(h)
        .min(at)
        .min(lines.len().saturating_sub(1));
    let shown: Vec<Line> = lines.into_iter().skip(offset).take(h).collect();
    f.render_widget(Paragraph::new(shown), rows_area);
    for (l, x, w, click) in places {
        if l >= offset && l < offset + h {
            let y = rows_area.y + (l - offset) as u16;
            hits.add(
                rows_area,
                Rect::new(rows_area.x + x as u16, y, w as u16, 1),
                click,
            );
        }
    }
    // The text cursor, in the focused outline pane.
    if app.session.view.focus == pane
        && matches!(app.session.mode, Mode::Normal)
        && cursor != TOP
        && let Some(r) = cursor_line
            .and_then(|l| l.checked_sub(offset))
            .filter(|r| *r < h)
    {
        let num = if cursor_depth == 0 {
            app.notebook.outline.number(cursor).len() + 2
        } else {
            0
        };
        let x = rows_area.x as usize + 2 * cursor_depth + 2 + num + app.session.view.pos;
        if x < (rows_area.x + rows_area.width) as usize {
            f.set_cursor_position(Position {
                x: x as u16,
                y: (rows_area.y as usize + r) as u16,
            });
        }
    }
    hits
}

/// The view at an item, drawn as world k computes it.
pub fn view_figure(app: &App, k: usize, item: u64, width: usize, height: usize) -> view::Figure {
    match view_inputs(app, k, item) {
        None => view::Figure::default(),
        Some(inputs) if inputs.is_empty() => view::Figure::noted("(no marks: no solutions yet)"),
        Some(inputs) => view::render(&inputs, width, height),
    }
}

/// The marks of the view at an item in world k, if the item has a view.
fn view_inputs(app: &App, k: usize, item: u64) -> Option<Vec<view::Input>> {
    let layers = app.notebook.outline.views.get(&item)?;
    let Some(eval) = app.reason.eval[k].as_ref() else {
        return Some(vec![]);
    };
    let bad = eval.violated_facts();
    Some(
        layers
            .iter()
            .flat_map(|l| layer_inputs(eval, l, &bad))
            .collect(),
    )
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
    for &(c, ink) in row {
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

/// The style each ink is drawn in.
fn ink_style(ink: view::Ink) -> Style {
    match ink {
        view::Ink::Plain => Style::default(),
        view::Ink::Dim => dim(),
        view::Ink::Loud => bold(),
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
    match app.reason.eval[0].as_ref() {
        Some(e) if !item_marks(e, &app.notebook.outline, id).is_empty() => {
            wrapped(constraint_panel(app, e, id))
        }
        _ => Paragraph::new(links_lines(&app.notebook.outline, id)),
    }
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
    Paragraph::new(Line::styled(text, dim()))
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
    let (q, input, listing) = (a.q, a.input.as_str(), a.listing);
    let lines: Vec<Line> = match q {
        Quasi::Command => command_help(input).into_iter().map(Line::raw).collect(),
        _ => {
            let m = app.leap_matches(input, q == Quasi::LeapForward);
            let shown = if listing { m.len() } else { m.len().min(5) };
            let mut ls = vec![Line::styled(format!("LEAP · {} MATCHES", m.len()), dim())];
            for (k, id) in m.iter().take(shown).enumerate() {
                let mut spans = Vec::new();
                let mut chain = Vec::new();
                let mut at = app.notebook.outline.parent(*id).filter(|p| *p != TOP);
                while let Some(p) = at.filter(|p| *p != TOP) {
                    chain.push(p);
                    at = app.notebook.outline.parent(p);
                }
                chain.reverse();
                let num = app
                    .notebook
                    .outline
                    .number(chain.first().copied().unwrap_or(*id));
                spans.push(Span::raw(format!("{num}. ")));
                for p in chain {
                    spans.push(Span::raw(format!("{} › ", app.notebook.outline.text(p))));
                }
                let t = app.notebook.outline.text(*id);
                let lt = t.to_lowercase();
                match lt
                    .find(&input.to_lowercase())
                    .filter(|_| lt.len() == t.len())
                {
                    Some(i) => {
                        let j = i + input.len();
                        spans.push(Span::raw(t[..i].to_string()));
                        spans.push(Span::styled(t[i..j].to_string(), bold()));
                        spans.push(Span::raw(t[j..].to_string()));
                    }
                    None => spans.push(Span::raw(t)),
                }
                let mut line = Line::from(spans);
                if k == 0 {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                ls.push(line);
            }
            ls
        }
    };
    let h = (lines.len() as u16 + 2).min(area.height);
    let r = Rect {
        x: area.x,
        y: area.y + area.height - h,
        width: area.width.saturating_sub(1),
        height: h,
    };
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL)),
        r,
    );
}

fn draw_prompt(f: &mut Frame, app: &App, prompt: Rect, guide: Rect) {
    let (p, g): (Line, String) = match &app.session.mode {
        Mode::Quasi(a) => {
            let shown = match a.q {
                Quasi::Command => app.guided(&a.input),
                _ => a.input.clone(),
            };
            let label = if a.latched {
                format!(" {} (latched) ", a.q.label())
            } else {
                format!(" {} ", a.q.label())
            };
            let line = Line::from(vec![
                Span::styled(label, Style::default().add_modifier(Modifier::REVERSED)),
                Span::raw(" "),
                Span::raw(shown),
            ]);
            let end = if a.latched {
                "Enter, or tap again,".to_string()
            } else {
                format!(
                    "release {}",
                    if a.q == Quasi::Command {
                        "Command"
                    } else if a.q == Quasi::Scope {
                        "Scope"
                    } else if a.q == Quasi::Suppose {
                        "Suppose"
                    } else {
                        "Leap"
                    }
                )
            };
            let g = match a.q {
                Quasi::LeapForward | Quasi::LeapBack => format!(
                    "{end} to go to the first match · Esc completes · ? lists every match · Leap again repeats"
                ),
                Quasi::Command => format!(
                    "{end} to run on the target · Esc completes · ? lists what may come next"
                ),
                Quasi::Scope => {
                    format!("Space widens the target: word, statement, branch · {end} keeps it")
                }
                Quasi::Suppose => format!("name the world · {end} starts it"),
            };
            (line, g)
        }
        Mode::ConfirmRetract { target, dependents } => {
            let line = Line::from(vec![
                Span::styled(
                    " RETRACT ",
                    Style::default().add_modifier(Modifier::REVERSED),
                ),
                Span::raw(format!(" entry {target}")),
            ]);
            let dep = if dependents.is_empty() {
                "nothing depends on it".to_string()
            } else {
                format!(
                    "entries that depend on it and will no longer apply: {}",
                    dependents
                        .iter()
                        .map(|d| d.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            (line, format!("{dep} · Enter retracts · Esc keeps it"))
        }
        Mode::ConfirmSuppose { text, readings, at } => {
            let line = Line::from(vec![
                Span::styled(
                    " SUPPOSE ",
                    Style::default().add_modifier(Modifier::REVERSED),
                ),
                Span::raw(format!(" {text}")),
            ]);
            let facts =
                crate::store::facts(app.journal().entries(), app.notebook.tip).unwrap_or_default();
            let r = reason::restate(&facts, &readings[*at]);
            let more = if readings.len() > 1 {
                " · Tab next reading"
            } else {
                ""
            };
            (
                line,
                format!("Do you mean: Suppose that {r}.  ⏎ yes · Esc no{more}"),
            )
        }
        Mode::Normal => match hint(app) {
            Some((p, g)) if app.session.message.is_empty() => (Line::raw(p), g),
            _ => (Line::raw(""), app.session.message.clone()),
        },
    };
    f.render_widget(Paragraph::new(p), prompt);
    f.render_widget(Paragraph::new(Span::styled(g, dim())), guide);
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
                d.iter().map(|p| p.0.clone()).collect::<Vec<_>>().join(", "),
                d.iter().map(|p| p.1.clone()).collect::<Vec<_>>().join(", "),
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
    let against = findings.iter().flat_map(|f| against_item(f, outline, item));
    against.map(|a| a.mark(outline)).collect()
}

/// A fact of an item against another fact of a finding that rests on both.
struct Against<'a> {
    finding: &'a Finding<'a>,
    here: &'a Term,
    there: Sourced<'a>,
}

impl Against<'_> {
    /// The mark: how the facts differ, and where the other comes from.
    fn mark(&self, outline: &Outline) -> (bool, String) {
        let (a, b) = difference(self.here, self.there.1);
        let from = where_from(outline, self.there.0);
        self.finding.mark(&format!("{a} here, {b} in {from}"))
    }
}

/// A finding against an item, if it rests on one of the item's facts:
/// the first of them against each other fact.
fn against_item<'a>(f: &'a Finding, outline: &Outline, item: u64) -> Vec<Against<'a>> {
    let (mine, others) = split_by_item(f.sol, outline, item);
    let Some((_, here)) = mine.first() else {
        return vec![];
    };
    let against = |there| Against {
        finding: f,
        here,
        there,
    };
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
fn status(e: &WorldEval) -> String {
    let v: usize = e.constraints.iter().map(|c| c.violations.len()).sum();
    let d: usize = e.constraints.iter().map(|c| c.disagreements.len()).sum();
    let mut parts = Vec::new();
    if v > 0 {
        parts.push(format!(
            "{v} constraint violation{}",
            if v == 1 { "" } else { "s" }
        ));
    }
    if d > 0 {
        parts.push(format!(
            "{d} disagreement{} between sources",
            if d == 1 { "" } else { "s" }
        ));
    }
    parts.join(" · ")
}

/// The predicates a goal or body reads, by name.
fn predicates(t: &Term, out: &mut Vec<String>) {
    if let Term::Compound(f, a) = t {
        if f == "," || f == ";" || f == "\\+" {
            a.iter().for_each(|x| predicates(x, out));
        } else if ![
            "is", "=", "\\=", "<", ">", "=<", ">=", "=:=", "=\\=", "==", "\\==",
        ]
        .contains(&f.as_str())
        {
            out.push(f.clone());
        }
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

/// The right pane for an item with marks: the constraint, its rule, its
/// solutions, and what depends on the facts it reads.
fn constraint_panel(app: &App, e: &WorldEval, item: u64) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for c in &e.constraints {
        let touches = |sols: &Vec<reason::Solution>| {
            sols.iter().any(|s| {
                s.prov
                    .iter()
                    .any(|p| app.notebook.outline.fact_item.get(&p.seq()) == Some(&item))
            })
        };
        if !touches(&c.violations) && !touches(&c.disagreements) {
            continue;
        }
        lines.push(Line::styled(
            format!("CONSTRAINT · {}", c.constraint.name.to_uppercase()),
            dim(),
        ));
        lines.push(Line::raw(""));
        for l in rule_lines(&c.constraint.rule) {
            lines.push(Line::raw(format!("  {l}")));
        }
        lines.push(Line::raw(""));
        for (sols, title, style) in [
            (&c.violations, "violations", bold()),
            (
                &c.disagreements,
                "disagreements between sources",
                Style::default(),
            ),
        ] {
            if sols.is_empty() {
                continue;
            }
            lines.push(Line::styled(
                format!("{title} · recomputed at journal {}", e.at),
                dim(),
            ));
            for sol in sols {
                let facts: Vec<(usize, &Term)> = sol.facts.iter().enumerate().collect();
                let key = match facts.first().map(|(_, f)| inner(f)) {
                    Some(Term::Compound(_, a)) => {
                        let shared: Vec<String> = (0..a.len())
                            .filter(|k| facts.iter().all(|(_, f)| matches!(inner(f), Term::Compound(_, b) if b.get(*k) == a.get(*k))))
                            .map(|k| reason::show(&a[k]))
                            .collect();
                        shared.join(", ")
                    }
                    _ => String::new(),
                };
                let parts: Vec<String> = facts
                    .iter()
                    .map(|(s, f)| {
                        let (d, _) = difference(
                            f,
                            facts.iter().find(|(t, _)| t != s).map(|x| x.1).unwrap_or(f),
                        );
                        format!("{d} ({})", where_from(&app.notebook.outline, &sol.prov[*s]))
                    })
                    .collect();
                lines.push(Line::styled(format!("{key}: {}", parts.join(" · ")), style));
            }
            lines.push(Line::raw(""));
        }
        // What depends on the facts this constraint reads: derived items
        // whose queries read the same predicates.
        let mut reads = Vec::new();
        if let Some(Term::Compound(_, a)) = node_def(e, c.seq) {
            predicates(&a[2], &mut reads);
        }
        let mut deps: Vec<u64> = app
            .notebook
            .outline
            .derived
            .keys()
            .filter(|d| {
                let mut r = Vec::new();
                if let Some(Term::Compound(_, a)) = node_def(e, **d) {
                    predicates(&a[1], &mut r);
                }
                r.iter().any(|p| reads.contains(p))
            })
            .copied()
            .collect();
        deps.sort_by_key(|d| app.notebook.outline.in_order().iter().position(|x| x == d));
        if !deps.is_empty() {
            lines.push(Line::styled("depends on these facts", dim()));
            for d in deps {
                let text = match (
                    app.notebook.outline.derived.get(&d),
                    e.nodes.derived.get(&d),
                ) {
                    (Some(t), Some(v)) => reason::fill(&t.template, &t.names, v, &e.facts),
                    _ => app.notebook.outline.text(d),
                };
                let short: String = text.chars().take(40).collect();
                lines.push(Line::raw(format!(
                    "  {} {short}{}",
                    app.notebook.outline.number(d),
                    if text.chars().count() > 40 {
                        " …"
                    } else {
                        ""
                    }
                )));
            }
        }
    }
    lines
}

/// The right pane when a claim is interrogated (one key, F2).
fn interrogation(app: &App, item: u64) -> Vec<Line<'static>> {
    let Some(e) = app.reason.eval[0].as_ref() else {
        return vec![Line::raw("not yet evaluated")];
    };
    let mut lines = vec![
        Line::styled("INTERROGATE · WHAT IS CLAIMED HERE", dim()),
        Line::raw(""),
        Line::raw(app.notebook.outline.text(item)),
        Line::raw(""),
    ];
    for (seq, src, p) in app.claims_of(item) {
        lines.push(Line::raw(format!(
            "claims: {} — from {src}",
            reason::restate(&e.facts, &p)
        )));
        lines.push(Line::styled(format!("  entry {seq}"), dim()));
        lines.push(Line::styled("who else claims it", dim()));
        let others: Vec<String> = e
            .facts
            .iter()
            .filter(|(s, f)| *s != seq && inner(f) == &p)
            .map(|(s, f)| match f {
                Term::Compound(n, a) if n == "claims" => {
                    format!(
                        "  {} ({})",
                        place(&app.notebook.outline, *s),
                        reason::show(&a[0])
                    )
                }
                _ => format!("  {} (a belief)", place(&app.notebook.outline, *s)),
            })
            .collect();
        if others.is_empty() {
            lines.push(Line::raw("  no one else"));
        }
        lines.extend(others.into_iter().map(Line::raw));
        lines.push(Line::styled("who claims the contrary", dim()));
        let mut contrary = Vec::new();
        for c in &e.constraints {
            for sol in c.disagreements.iter().chain(&c.violations) {
                if !sol.prov.iter().any(|q| q.seq() == seq) {
                    continue;
                }
                for (k, o) in sol.prov.iter().enumerate().filter(|(_, q)| q.seq() != seq) {
                    {
                        let f = &sol.facts[k];
                        contrary.push(format!(
                            "  {}: {} ({})",
                            where_from(&app.notebook.outline, o),
                            reason::restate(&e.facts, inner(f)),
                            c.constraint.name
                        ));
                    }
                }
            }
        }
        if contrary.is_empty() {
            contrary.push("  no one".into());
        }
        lines.extend(contrary.into_iter().map(Line::raw));
        lines.push(Line::styled("the source's record", dim()));
        let path_adopts: Vec<&format::record::Entry> = app
            .journal()
            .entries()
            .iter()
            .filter(|x| {
                x.kind() == "adopt" && x.body().is_some_and(|b| b["source"] == src.as_str())
            })
            .collect();
        let retracted = path_adopts
            .iter()
            .filter(|a| {
                app.journal().entries().iter().any(|r| {
                    r.kind() == "retract"
                        && r.body().and_then(|b| b["target"].as_u64()) == Some(a.seq)
                })
            })
            .count();
        lines.push(Line::raw(format!(
            "  {} of its claims adopted, {retracted} of them retracted since",
            path_adopts.len()
        )));
    }
    lines.push(Line::styled("if adopted", dim()));
    lines.push(Line::raw(format!(
        "  {}",
        app.reason
            .preview
            .clone()
            .unwrap_or_else(|| "not yet computed".into())
    )));
    lines.push(Line::raw(""));
    // The hint follows, drawn by draw_links, where a click can find it.
    lines
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
/// for row. Rows are the items that state facts or show derived values.
fn draw_worlds(f: &mut Frame, app: &App, area: Rect) {
    let Some(sup) = &app.reason.supposition else {
        return;
    };
    if sup.name.starts_with("revision of ") {
        draw_revision(f, app, area);
        return;
    }
    let [l, r] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);
    let actual = app.notebook.outline_at(sup.actual);
    let worlds = [
        (
            &actual,
            app.reason.eval[1].as_ref(),
            "WORLD · ACTUAL".to_string(),
        ),
        (
            &app.notebook.outline,
            app.reason.eval[0].as_ref(),
            format!("WORLD · SUPPOSE {}", sup.name.to_uppercase()),
        ),
    ];
    let mut rows: Vec<u64> = actual
        .in_order()
        .into_iter()
        .filter(|i| actual.fact_item.values().any(|x| x == i) || actual.derived.contains_key(i))
        .collect();
    for i in app.notebook.outline.in_order() {
        if (app.notebook.outline.fact_item.values().any(|x| *x == i)
            || app.notebook.outline.derived.contains_key(&i))
            && !rows.contains(&i)
        {
            rows.push(i);
        }
    }
    let text = |o: &Outline, e: Option<&WorldEval>, i: u64| -> String {
        match (
            o.derived.get(&i),
            e.and_then(|e| e.nodes.derived.get(&i).map(|v| (e, v))),
        ) {
            (Some(d), Some((e, v))) => reason::fill(&d.template, &d.names, v, &e.facts),
            _ => o.text(i),
        }
    };
    let left_facts: Vec<u64> = worlds[0]
        .1
        .map(|e| e.facts.iter().map(|x| x.0).collect())
        .unwrap_or_default();
    let right_facts: Vec<u64> = worlds[1]
        .1
        .map(|e| e.facts.iter().map(|x| x.0).collect())
        .unwrap_or_default();
    let set_aside: Vec<u64> = left_facts
        .iter()
        .filter(|s| !right_facts.contains(s))
        .copied()
        .collect();
    for (k, (o, e, title), area) in [(0, worlds[0].clone(), l), (1, worlds[1].clone(), r)] {
        let mut lines = vec![Line::styled(title, dim()), Line::raw("")];
        for i in &rows {
            let here = text(o, e, *i);
            let there = text(worlds[1 - k].0, worlds[1 - k].1, *i);
            let gone = k == 1 && set_aside.iter().any(|s| actual.fact_item.get(s) == Some(i));
            let mut spans = vec![Span::styled(
                here.clone(),
                if here != there && k == 1 {
                    bold()
                } else {
                    Style::default()
                },
            )];
            if here != there && k == 0 {
                spans.push(Span::styled("  - - -", dim()));
            }
            if gone {
                spans.push(Span::styled("  (fact retracted here)", dim()));
            }
            lines.push(Line::from(spans));
        }
        lines.push(Line::raw(""));
        if let Some(e) = e {
            for c in &e.constraints {
                let mut t = format!(
                    "Constraint {}: {} violation{}",
                    c.constraint.name,
                    c.violations.len(),
                    if c.violations.len() == 1 { "" } else { "s" }
                );
                if !c.disagreements.is_empty() {
                    t.push_str(&format!(
                        ", {} disagreement{}",
                        c.disagreements.len(),
                        if c.disagreements.len() == 1 { "" } else { "s" }
                    ));
                }
                if k == 1 && !set_aside.is_empty() {
                    t.push_str(&format!(
                        " · {} retracted here",
                        set_aside
                            .iter()
                            .map(|s| place(&actual, *s))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                lines.push(Line::raw(t));
            }
        }
        // Each view, drawn from this world's facts: the left column is
        // the actual world (eval[1]), the right the supposed one (eval[0]).
        let views: Vec<u64> = o
            .in_order()
            .into_iter()
            .filter(|i| app.notebook.outline.views.contains_key(i))
            .collect();
        for i in views {
            lines.push(Line::raw(""));
            lines.push(Line::raw(o.text(i)));
            let width = (area.width as usize).saturating_sub(4);
            let fig = view_figure(app, 1 - k, i, width, 8);
            lines.extend(figure_lines(&fig, 2));
        }
        f.render_widget(
            Paragraph::new(lines).block(Block::default().borders(if k == 0 {
                Borders::RIGHT
            } else {
                Borders::NONE
            })),
            area,
        );
    }
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
        assert_eq!(style(Ink::Loud), bold());
        assert_eq!(style(Ink::Plain), Style::default());
    }
}
