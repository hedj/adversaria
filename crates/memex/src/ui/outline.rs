//! An outline pane: its frame and title, its items laid out in rows with
//! their marks and notes, the window of them that fits around the cursor,
//! and what lies beyond it above and below (docs/guide.md § 1.1).

use super::*;

/// An outline pane: its frame and title, then every line of the outline
/// near the cursor, and the window onto them that keeps the cursor's line
/// in view.
pub(super) fn draw_outline(f: &mut Frame, app: &App, pane: usize, area: Rect) -> Presentations {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
