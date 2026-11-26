//! Two worlds side by side: the supposed world and the actual, each a
//! column with its findings and its facts, the facts they share in one row
//! and those that differ marked, and what the supposition set aside
//! (docs/guide.md § 2.4, Worlds and history).

use super::*;
use format::body::Body;
use format::record::EntryKind;

/// A world's column, by its place, in its half.
fn draw_column(f: &mut Frame, k: usize, area: Rect, rows: Vec<Line<'static>>) {
    f.render_widget(Paragraph::new(rows).block(ruled(k)), area);
}

/// The first world is ruled off from the second.
pub(super) fn ruled(k: usize) -> Block<'static> {
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
pub(super) struct Sides<'a> {
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
    pub(super) fn of(app: &'a App) -> Option<Self> {
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
    pub(super) fn draw(&self, f: &mut Frame, area: Rect) {
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
    let body = Body::of(e.body().filter(|_| e.kind() == EntryKind::Retract)?);
    Some((body.target()?, body.constraints()?))
}

fn set_aside(worlds: &[Side; 2]) -> Vec<u64> {
    let kept = worlds[1].fact_seqs();
    let actual = worlds[0].fact_seqs();
    actual.into_iter().filter(|s| !kept.contains(s)).collect()
}

/// A count and its noun: "1 violation", "2 violations".
pub(super) fn counted(n: usize, noun: &str) -> String {
    let s = if n == 1 { "" } else { "s" };
    format!("{n} {noun}{s}")
}

/// A rule laid out as in the design: the head, then one literal of the body
/// per line, split only at commas outside brackets.
pub(super) fn rule_lines(rule: &str) -> Vec<String> {
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
