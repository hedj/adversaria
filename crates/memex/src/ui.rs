//! Drawing the screen from the interface state (docs/design.md § A user
//! story, scene 1). Drawing reads the state and changes nothing, so the same
//! state always draws the same screen.

use crate::actions::command_help;
use crate::app::{App, Click, Mode, PaneKind, Quasi, ScopeLevel};
use crate::outline::{Outline, TOP};
use crate::reason::{self, Prov, WorldEval};
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
    let mut hits = Hits::default();
    let [top, body, prompt, guide] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(f.area());
    draw_top(f, app, top);
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(body);
    if app.supposition.is_some() {
        draw_worlds(f, app, body);
        draw_prompt(f, app, prompt, guide);
        return hits;
    }
    for (i, area) in [(0, left), (1, right)] {
        match app.view.panes[i].kind {
            PaneKind::Outline => draw_outline(f, app, i, area, &mut hits),
            PaneKind::Links => draw_links(f, app, area, &mut hits),
            PaneKind::Suggestions => draw_suggestions(f, app, area, &mut hits),
        }
    }
    if let Mode::Quasi(a) = &app.mode
        && matches!(a.q, Quasi::LeapForward | Quasi::LeapBack | Quasi::Command)
        && (a.listing || !a.input.is_empty())
    {
        draw_list(f, app, a.q, &a.input, a.listing, left);
        // The list covers the outline: no item under it is clickable.
        hits.0.retain(|(r, _)| r.intersection(left).is_empty());
    }
    draw_prompt(f, app, prompt, guide);
    hits
}

fn draw_top(f: &mut Frame, app: &App, area: Rect) {
    let n = app.journal().entries().len();
    let world = match &app.supposition {
        Some(s) => vec![
            Span::styled("worlds ", dim()),
            Span::styled("actual", bold()),
            Span::raw(" | "),
            Span::styled(format!("suppose: {}", s.name), bold()),
        ],
        None => vec![
            Span::styled("world ", dim()),
            Span::styled(app.world.clone(), bold()),
        ],
    };
    let mut parts = vec![
        Span::styled("room ", dim()),
        Span::styled(app.workspace.clone(), bold()),
        Span::raw("   "),
    ];
    parts.extend(world);
    let left = Line::from(vec![
        Span::raw(""),
        Span::raw("   "),
        Span::styled("journal ", dim()),
        Span::styled(n.to_string(), bold()),
    ]);
    let mut left = left;
    left.spans.splice(0..1, parts);
    f.render_widget(Paragraph::new(left), area);
    let right = match app.eval[0].as_ref().map(status) {
        Some(s) if !s.is_empty() => s,
        _ => format!("pane {} of 2", app.view.focus + 1),
    };
    let w = right.chars().count() as u16;
    if area.width > w {
        f.render_widget(
            Paragraph::new(Span::styled(right, dim())),
            Rect {
                x: area.x + area.width - w,
                width: w,
                ..area
            },
        );
    }
}

/// The right pane for a view: its layers, each with its goal and how many
/// marks it gives, and the view's notes.
fn view_panel(app: &App, id: u64, layers: &[crate::outline::Layer]) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::styled(
            format!(
                "VIEW · {} LAYER{}",
                layers.len(),
                if layers.len() == 1 { "" } else { "S" }
            ),
            dim(),
        ),
        Line::raw(""),
    ];
    for l in layers {
        let n = app.eval[0]
            .as_ref()
            .and_then(|e| e.nodes.layers.get(&l.seq))
            .map_or(0, |m| m.len());
        lines.push(Line::raw(format!("layer {} where {}", l.mark, l.goal)));
        lines.push(Line::styled(
            format!(
                "  entry {} · {n} mark{}",
                l.seq,
                if n == 1 { "" } else { "s" }
            ),
            dim(),
        ));
    }
    let fig = view_figure(app, 0, id, 60, 12);
    if !fig.notes.is_empty() {
        lines.push(Line::raw(""));
        lines.extend(fig.notes.into_iter().map(|n| Line::styled(n, dim())));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "a click on a mark goes to the item that states its fact",
        dim(),
    ));
    lines
}

/// The citations of an item, shown dimmed after its text.
fn cites(app: &App, id: u64) -> String {
    let (out, _) = app.outline.links_of(id);
    out.iter()
        .filter(|l| l.kind == "cites")
        .map(|l| format!(" [{}]", app.outline.text(l.to)))
        .collect()
}

fn item_spans(app: &App, id: u64, focused: bool) -> Vec<Span<'static>> {
    let pending = app.text_of(id);
    let runs = app.outline.runs(id);
    let mut out = Vec::new();
    if pending.iter().collect::<String>() != app.outline.text(id) {
        out.push(Span::raw(pending.iter().collect::<String>()));
    } else {
        for r in runs {
            if r.origin == id {
                out.push(Span::raw(r.text));
            } else {
                out.push(Span::styled(
                    r.text,
                    Style::default().add_modifier(Modifier::ITALIC | Modifier::UNDERLINED),
                ));
                out.push(Span::styled(
                    format!("↖{}", app.outline.number(r.origin)),
                    dim(),
                ));
            }
        }
    }
    if focused && app.view.scope == ScopeLevel::Word {
        // Scope: show the word as the target.
        out = vec![Span::styled(
            pending.iter().collect::<String>(),
            Style::default().add_modifier(Modifier::UNDERLINED),
        )];
    }
    if let (Some(d), Some(e)) = (app.outline.derived.get(&id), app.eval[0].as_ref())
        && let Some(v) = e.nodes.derived.get(&id)
    {
        out = vec![
            Span::raw(reason::fill(&d.template, &d.names, v, &e.facts)),
            Span::styled(" [derived]", dim()),
        ];
    }
    if let Some(src) = app.outline.source.get(&id) {
        out.push(Span::styled(format!(" [{src}]"), dim()));
    }
    let c = cites(app, id);
    if !c.is_empty() {
        out.push(Span::styled(c, dim()));
    }
    out
}

fn title(app: &App) -> String {
    match app.outline.names.get("title") {
        Some(id) => format!("OUTLINE · {}", app.outline.text(*id).to_uppercase()),
        None => "OUTLINE".into(),
    }
}

fn draw_outline(f: &mut Frame, app: &App, pane: usize, area: Rect, hits: &mut Hits) {
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
    let cursor = app.view.panes[pane].cursor;
    let h = rows_area.height as usize;
    let focused = app.view.focus == pane && matches!(app.mode, Mode::Normal | Mode::Quasi(_));
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
        let kids = app.outline.children(*id).len();
        let folded = app.view.folded.contains(id);
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
            spans.push(Span::raw(format!("{}. ", app.outline.number(*id))));
        }
        let is_cursor = *id == cursor;
        spans.extend(item_spans(app, *id, is_cursor && focused));
        if folded {
            spans.push(Span::styled(
                format!(" {}", app.outline.descendants(*id)),
                dim(),
            ));
        }
        let marks = app.eval[0]
            .as_ref()
            .map(|e| item_marks(e, &app.outline, *id))
            .unwrap_or_default();
        let mut line = Line::from(spans);
        if is_cursor {
            line = line.style(if app.view.scope == ScopeLevel::Branch && focused {
                Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
            } else {
                Style::default().add_modifier(Modifier::REVERSED)
            });
        }
        lines.push(line);
        let stale = app
            .stale
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
        if app.outline.views.contains_key(id) {
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
    let at = cursor_line.unwrap_or(0);
    let offset = (at + 3)
        .saturating_sub(h)
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
    if app.view.focus == pane
        && matches!(app.mode, Mode::Normal)
        && cursor != TOP
        && let Some(r) = cursor_line
            .and_then(|l| l.checked_sub(offset))
            .filter(|r| *r < h)
    {
        let num = if cursor_depth == 0 {
            app.outline.number(cursor).len() + 2
        } else {
            0
        };
        let x = rows_area.x as usize + 2 * cursor_depth + 2 + num + app.view.pos;
        if x < (rows_area.x + rows_area.width) as usize {
            f.set_cursor_position(Position {
                x: x as u16,
                y: (rows_area.y as usize + r) as u16,
            });
        }
    }
}

/// The facts in violations in world k, by seq: marks resting on them are
/// drawn loudly.
fn violated(app: &App, k: usize) -> std::collections::BTreeSet<u64> {
    app.eval[k]
        .as_ref()
        .map(|e| {
            e.constraints
                .iter()
                .flat_map(|c| &c.violations)
                .flat_map(|v| v.prov.iter().map(|p| p.seq()))
                .collect()
        })
        .unwrap_or_default()
}

/// The view at an item, drawn as world k computes it.
pub fn view_figure(app: &App, k: usize, item: u64, width: usize, height: usize) -> view::Figure {
    let Some(layers) = app.outline.views.get(&item) else {
        return view::Figure::default();
    };
    let bad = violated(app, k);
    let inputs: Vec<view::Input> = app.eval[k]
        .as_ref()
        .map(|e| {
            layers
                .iter()
                .flat_map(|l| e.nodes.layers.get(&l.seq).cloned().unwrap_or_default())
                .map(|(mark, prov)| view::Input {
                    loud: prov.iter().any(|s| bad.contains(s)),
                    mark,
                    prov,
                })
                .collect()
        })
        .unwrap_or_default();
    if inputs.is_empty() {
        return view::Figure {
            notes: vec!["(no marks: no solutions yet)".into()],
            ..Default::default()
        };
    }
    view::render(&inputs, width, height)
}

/// A figure as lines, indented; notes after it, dim.
fn figure_lines(fig: &view::Figure, indent: usize) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = fig
        .rows
        .iter()
        .map(|row| {
            let mut spans = vec![Span::raw(" ".repeat(indent))];
            let mut run = String::new();
            let mut ink = view::Ink::Plain;
            let style = |i: view::Ink| match i {
                view::Ink::Plain => Style::default(),
                view::Ink::Dim => dim(),
                view::Ink::Loud => bold(),
            };
            for (c, i) in row {
                if *i != ink && !run.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut run), style(ink)));
                }
                ink = *i;
                run.push(*c);
            }
            let run = run.trim_end().to_string();
            if !run.is_empty() {
                spans.push(Span::styled(run, style(ink)));
            }
            Line::from(spans)
        })
        .collect();
    out.extend(
        fig.notes
            .iter()
            .map(|n| Line::styled(format!("{}{n}", " ".repeat(indent)), dim())),
    );
    out
}

/// The item a click on a mark goes to: the item stating the first fact
/// the mark rests on.
fn hit_item(app: &App, hit: &view::Hit) -> Option<u64> {
    hit.prov.iter().find_map(|s| {
        // A fact from the outline itself rests on the item's own entry.
        if app.outline.items.contains_key(s) {
            Some(*s)
        } else {
            app.outline.fact_item.get(s).copied()
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

fn draw_links(f: &mut Frame, app: &App, area: Rect, hits: &mut Hits) {
    let inner = Rect {
        x: area.x + 2,
        width: area.width.saturating_sub(2),
        ..area
    };
    let id = app.view.panes[0].cursor;
    if let Some(i) = app.view.interrogate {
        let p = Paragraph::new(interrogation(app, i)).wrap(Wrap { trim: false });
        let used = p.line_count(inner.width) as u16;
        f.render_widget(p, inner);
        // The hint on its own line after the answers, each part clickable.
        let (a, u) = (
            format!("{} adopts", app.key_for("adopt")),
            format!("{} retracts an adoption", app.key_for("unadopt")),
        );
        let y = inner.y.saturating_add(used);
        if y < inner.y + inner.height {
            let w = |s: &str| s.chars().count() as u16;
            f.render_widget(
                Paragraph::new(Line::styled(format!("{a} · {u}"), dim())),
                Rect::new(inner.x, y, inner.width, 1),
            );
            hits.add(
                inner,
                Rect::new(inner.x, y, w(&a), 1),
                Click::Command("adopt".into()),
            );
            hits.add(
                inner,
                Rect::new(inner.x + w(&a) + 3, y, w(&u), 1),
                Click::Command("unadopt".into()),
            );
        }
        return;
    }
    if let Some(layers) = app.outline.views.get(&id) {
        f.render_widget(
            Paragraph::new(view_panel(app, id, layers)).wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }
    if let Some(e) = app.eval[0].as_ref()
        && !item_marks(e, &app.outline, id).is_empty()
    {
        f.render_widget(
            Paragraph::new(constraint_panel(app, e, id)).wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }
    let mut lines = vec![
        Line::styled("LINKS OF THE SELECTED ITEM", dim()),
        Line::raw(""),
    ];
    if id != TOP {
        lines.push(Line::raw(app.outline.text(id)));
        lines.push(Line::raw(""));
        let (out, into) = app.outline.links_of(id);
        let label = |l: u64| format!("  {}. {}", app.outline.number(l), app.outline.text(l));
        let mut kinds: Vec<&str> = out.iter().map(|l| l.kind.as_str()).collect();
        kinds.dedup();
        let mut n = 0;
        for k in kinds {
            lines.push(Line::styled(format!("{k} →"), dim()));
            for l in out.iter().filter(|l| l.kind == k) {
                n += 1;
                lines.push(Line::raw(format!("{}  {n}", label(l.to))));
            }
        }
        let mut kinds: Vec<&str> = into.iter().map(|l| l.kind.as_str()).collect();
        kinds.dedup();
        for k in kinds {
            lines.push(Line::styled(format!("{} ←", inverse(k)), dim()));
            for l in into.iter().filter(|l| l.kind == k) {
                n += 1;
                lines.push(Line::raw(format!("{}  {n}", label(l.from))));
            }
        }
        let tr = app.outline.transcluded_in(id);
        if !tr.is_empty() {
            lines.push(Line::styled("transcluded in ←", dim()));
            for t in tr {
                // The container shows where the text appears.
                let path = match app.outline.parent(t).filter(|p| *p != TOP) {
                    Some(p) => format!("  {} › {}", app.outline.text(p), app.outline.text(t)),
                    None => label(t),
                };
                lines.push(Line::raw(path));
            }
        }
    }
    f.render_widget(Paragraph::new(lines), inner);
}

/// The matches of a Leap, or the commands a Command may be, in a box at the
/// foot of the outline pane.
fn draw_list(f: &mut Frame, app: &App, q: Quasi, input: &str, listing: bool, area: Rect) {
    let lines: Vec<Line> = match q {
        Quasi::Command => command_help(input).into_iter().map(Line::raw).collect(),
        _ => {
            let m = app.leap_matches(input, q == Quasi::LeapForward);
            let shown = if listing { m.len() } else { m.len().min(5) };
            let mut ls = vec![Line::styled(format!("LEAP · {} MATCHES", m.len()), dim())];
            for (k, id) in m.iter().take(shown).enumerate() {
                let mut spans = Vec::new();
                let mut chain = Vec::new();
                let mut at = app.outline.parent(*id).filter(|p| *p != TOP);
                while let Some(p) = at.filter(|p| *p != TOP) {
                    chain.push(p);
                    at = app.outline.parent(p);
                }
                chain.reverse();
                let num = app.outline.number(chain.first().copied().unwrap_or(*id));
                spans.push(Span::raw(format!("{num}. ")));
                for p in chain {
                    spans.push(Span::raw(format!("{} › ", app.outline.text(p))));
                }
                let t = app.outline.text(*id);
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
    let (p, g): (Line, String) = match &app.mode {
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
            let facts = crate::store::facts(app.journal().entries(), app.tip).unwrap_or_default();
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
            Some((p, g)) if app.message.is_empty() => (Line::raw(p), g),
            _ => (Line::raw(""), app.message.clone()),
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
    let mut out = Vec::new();
    for c in &e.constraints {
        for (sols, violation) in [(&c.violations, true), (&c.disagreements, false)] {
            for sol in sols {
                let mine = |p: &Prov| outline.fact_item.get(&p.seq()) == Some(&item);
                let Some(h) = sol.prov.iter().position(mine) else {
                    continue;
                };
                let kind = if violation {
                    "violation"
                } else {
                    "sources disagree"
                };
                for (o, p) in sol.prov.iter().enumerate().filter(|(_, p)| !mine(p)) {
                    let (a, b) = difference(&sol.facts[h], &sol.facts[o]);
                    out.push((
                        violation,
                        format!(
                            "{}, {kind}: {a} here, {b} in {}",
                            c.constraint.name,
                            where_from(outline, p)
                        ),
                    ));
                }
            }
        }
    }
    out
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
                    .any(|p| app.outline.fact_item.get(&p.seq()) == Some(&item))
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
                        format!("{d} ({})", where_from(&app.outline, &sol.prov[*s]))
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
        deps.sort_by_key(|d| app.outline.in_order().iter().position(|x| x == d));
        if !deps.is_empty() {
            lines.push(Line::styled("depends on these facts", dim()));
            for d in deps {
                let text = match (app.outline.derived.get(&d), e.nodes.derived.get(&d)) {
                    (Some(t), Some(v)) => reason::fill(&t.template, &t.names, v, &e.facts),
                    _ => app.outline.text(d),
                };
                let short: String = text.chars().take(40).collect();
                lines.push(Line::raw(format!(
                    "  {} {short}{}",
                    app.outline.number(d),
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
    let Some(e) = app.eval[0].as_ref() else {
        return vec![Line::raw("not yet evaluated")];
    };
    let mut lines = vec![
        Line::styled("INTERROGATE · WHAT IS CLAIMED HERE", dim()),
        Line::raw(""),
        Line::raw(app.outline.text(item)),
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
                    format!("  {} ({})", place(&app.outline, *s), reason::show(&a[0]))
                }
                _ => format!("  {} (a belief)", place(&app.outline, *s)),
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
                            where_from(&app.outline, o),
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
        app.preview
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
    let e = app.eval[0].as_ref()?;
    if item_marks(e, &app.outline, app.cursor()).is_empty() {
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
    let Some(sup) = &app.supposition else { return };
    if sup.name.starts_with("revision of ") {
        draw_revision(f, app, area);
        return;
    }
    let [l, r] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);
    let actual = app.outline_at(sup.actual);
    let worlds = [
        (&actual, app.eval[1].as_ref(), "WORLD · ACTUAL".to_string()),
        (
            &app.outline,
            app.eval[0].as_ref(),
            format!("WORLD · SUPPOSE {}", sup.name.to_uppercase()),
        ),
    ];
    let mut rows: Vec<u64> = actual
        .in_order()
        .into_iter()
        .filter(|i| actual.fact_item.values().any(|x| x == i) || actual.derived.contains_key(i))
        .collect();
    for i in app.outline.in_order() {
        if (app.outline.fact_item.values().any(|x| *x == i) || app.outline.derived.contains_key(&i))
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
            .filter(|i| app.outline.views.contains_key(i))
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

/// The suggestion pane: few in view, ranked by evidence, each with why it
/// was offered. It never takes the keyboard: accept or dismiss by command.
fn draw_suggestions(f: &mut Frame, app: &App, area: Rect, hits: &mut Hits) {
    let inner = Rect {
        x: area.x + 2,
        width: area.width.saturating_sub(2),
        ..area
    };
    let shown = app.shown_suggestions();
    let mut lines = vec![
        Line::styled(
            format!("SUGGESTIONS · {} OF {}", shown.len(), app.suggestions.len()),
            dim(),
        ),
        Line::raw(""),
    ];
    let mut y = inner.y.saturating_add(lines.len() as u16);
    for (i, s) in shown.iter().enumerate() {
        let these = vec![
            Line::raw(format!("{}. {}", i + 1, s.describe())),
            Line::styled(format!("   {}", s.evidence), dim()),
        ];
        // A click anywhere on a suggestion, wrapped lines included, accepts it.
        let h = Paragraph::new(these.clone())
            .wrap(Wrap { trim: false })
            .line_count(inner.width) as u16;
        hits.add(
            inner,
            Rect::new(inner.x, y, inner.width, h),
            Click::Command(format!("accept {}", i + 1)),
        );
        y = y.saturating_add(h);
        lines.extend(these);
    }
    if shown.is_empty() {
        lines.push(Line::styled("nothing to suggest", dim()));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("accept N adopts · dismiss N dismisses", dim()));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

/// A document revision's world: each change, one entry each, to keep or
/// retract (retract N) before the world is committed or closed.
fn draw_revision(f: &mut Frame, app: &App, area: Rect) {
    let Some(sup) = &app.supposition else { return };
    let mut lines = vec![
        Line::styled(format!("WORLD · {}", sup.name.to_uppercase()), dim()),
        Line::raw(""),
    ];
    let changes = app.revision_changes();
    for (seq, what, retracted) in &changes {
        let line = format!("#{seq}  {what}");
        lines.push(if *retracted {
            Line::styled(format!("{line}  (retracted)"), dim())
        } else {
            Line::raw(line)
        });
    }
    if changes.is_empty() {
        lines.push(Line::styled("no changes", dim()));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("retract N undoes one change here · commit makes the rest real · close leaves the actual world as it was", dim()));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}
