//! Drawing the screen from the interface state (docs/design.md § A user
//! story, scene 1). Drawing reads the state and changes nothing, so the same
//! state always draws the same screen.

use crate::app::{App, Mode, PaneKind, Quasi, ScopeLevel, command_help};
use crate::outline::TOP;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}
fn bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub fn draw(f: &mut Frame, app: &App) {
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
    for (i, area) in [(0, left), (1, right)] {
        match app.view.panes[i].kind {
            PaneKind::Outline => draw_outline(f, app, i, area),
            PaneKind::Links => draw_links(f, app, area),
        }
    }
    if let Mode::Quasi(a) = &app.mode
        && matches!(a.q, Quasi::LeapForward | Quasi::LeapBack | Quasi::Command)
        && (a.listing || !a.input.is_empty())
    {
        draw_list(f, app, a.q, &a.input, a.listing, left);
    }
    draw_prompt(f, app, prompt, guide);
}

fn draw_top(f: &mut Frame, app: &App, area: Rect) {
    let n = app.journal().entries().len();
    let left = Line::from(vec![
        Span::styled("room ", dim()),
        Span::styled(app.workspace.clone(), bold()),
        Span::raw("   "),
        Span::styled("world ", dim()),
        Span::styled(app.world.clone(), bold()),
        Span::raw("   "),
        Span::styled("journal ", dim()),
        Span::styled(n.to_string(), bold()),
    ]);
    f.render_widget(Paragraph::new(left), area);
    let right = format!("pane {} of 2", app.view.focus + 1);
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

fn draw_outline(f: &mut Frame, app: &App, pane: usize, area: Rect) {
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
    let at = rows.iter().position(|(id, _)| *id == cursor).unwrap_or(0);
    let h = rows_area.height as usize;
    let offset = (at + 3).saturating_sub(h);
    let focused = app.view.focus == pane && matches!(app.mode, Mode::Normal | Mode::Quasi(_));
    let mut lines = Vec::new();
    for (id, depth) in rows.iter().skip(offset).take(h) {
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
        let mut line = Line::from(spans);
        if is_cursor {
            line = line.style(if app.view.scope == ScopeLevel::Branch && focused {
                Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
            } else {
                Style::default().add_modifier(Modifier::REVERSED)
            });
        }
        lines.push(line);
    }
    if rows.is_empty() {
        lines.push(Line::styled("(empty: type to write the first item)", dim()));
    }
    f.render_widget(Paragraph::new(lines), rows_area);
    // The text cursor, in the focused outline pane.
    if app.view.focus == pane
        && matches!(app.mode, Mode::Normal)
        && cursor != TOP
        && let Some(&(_, depth)) = rows.get(at)
    {
        let num = if depth == 0 {
            app.outline.number(cursor).len() + 2
        } else {
            0
        };
        let x = rows_area.x as usize + 2 * depth + 2 + num + app.view.pos;
        // The cursor's row may be off screen when the pane has fewer rows
        // than the offset assumes (a terminal a few lines high).
        let shown = at.checked_sub(offset).filter(|r| *r < h);
        if let Some(r) = shown
            && x < (rows_area.x + rows_area.width) as usize
        {
            let y = rows_area.y as usize + r;
            f.set_cursor_position(Position {
                x: x as u16,
                y: y as u16,
            });
        }
    }
}

fn inverse(kind: &str) -> String {
    match kind {
        "cites" => "cited by".into(),
        "supports" => "supported by".into(),
        "opposes" => "opposed by".into(),
        k => format!("{k} (from)"),
    }
}

fn draw_links(f: &mut Frame, app: &App, area: Rect) {
    let inner = Rect {
        x: area.x + 2,
        width: area.width.saturating_sub(2),
        ..area
    };
    let id = app.view.panes[0].cursor;
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
        Mode::Normal => (Line::raw(""), app.message.clone()),
    };
    f.render_widget(Paragraph::new(p), prompt);
    f.render_widget(Paragraph::new(Span::styled(g, dim())), guide);
}
