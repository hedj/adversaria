//! The list of commands that the Command quasimode shows: what may come
//! next, in the guide's groups (docs/guide.md § 2.4).

use super::{bold, dim, fits};
use crate::actions::{Command, Group, candidates, command_groups, command_usage};
use crate::app::App;
use ratatui::text::{Line, Span};

/// What may come next in Command: the command's slots and help, once
/// it is named, then the words its first noun may be; else the commands
/// that start so, in the guide's groups, each with what it does and its
/// key, if one is bound to it; or, where those do not fit, each group's
/// names alone; or that none starts so.
pub(super) fn command_lines(app: &App, input: &str, inside: (u16, u16)) -> Vec<Line<'static>> {
    if let Some(lines) = usage_lines(input) {
        return lines;
    }
    let groups = command_groups(input);
    if groups.is_empty() {
        return vec![none_starts(input)];
    }
    let described = described_commands(app, &groups, inside.0 as usize);
    match fits(&described, inside, 0) {
        true => described,
        false => command_index(app, &groups),
    }
}

/// Once a line names its command: its slots and help, then the words its
/// first noun may be, each with what it means.
fn usage_lines(input: &str) -> Option<Vec<Line<'static>>> {
    let usage = Line::raw(command_usage(input)?);
    let words = candidates(input).into_iter();
    let words = words.map(|(w, what)| Line::raw(format!("  {w} — {what}")));
    Some(std::iter::once(usage).chain(words).collect())
}

/// That no command starts as the line does.
fn none_starts(input: &str) -> Line<'static> {
    let typed = input.split_whitespace().next().unwrap_or("");
    Line::styled(format!("no command starts with “{typed}”"), dim())
}

/// Each group's heading, bold, then its commands in the order of their
/// names, which is how a reader who has typed their start looks for one,
/// each with what it does and its key, at the right edge of a width.
fn described_commands(app: &App, groups: &[Group], width: usize) -> Vec<Line<'static>> {
    groups
        .iter()
        .flat_map(|g| described_group(app, g, width))
        .collect()
}

/// A group's heading, bold, then its commands by name, described.
fn described_group(app: &App, (heading, cs): &Group, width: usize) -> Vec<Line<'static>> {
    let each = described_each(app, cs, width);
    std::iter::once(Line::styled(*heading, bold()))
        .chain(each)
        .collect()
}

/// Each of a group's commands, by name, described.
fn described_each(app: &App, cs: &[&'static Command], width: usize) -> Vec<Line<'static>> {
    let mut cs = cs.to_vec();
    cs.sort_by_key(|c| c.name);
    cs.iter().map(|c| described(app, c, width)).collect()
}

/// A command with what it does and, at the right edge of a width, its
/// key, if one is bound to it.
fn described(app: &App, c: &Command, width: usize) -> Line<'static> {
    let what = format!("{} — {}", c.name, c.brief);
    match app.session.keys.bound_to(c.name) {
        Some(key) => with_key(what, key, width),
        None => Line::raw(what),
    }
}

/// Words, then a key, dim, at the right edge of a width: at least two
/// cells after the words.
fn with_key(what: String, key: String, width: usize) -> Line<'static> {
    let used = what.chars().count() + key.chars().count();
    let gap = " ".repeat(width.saturating_sub(used).max(2));
    Line::from(vec![Span::raw(what + &gap), Span::styled(key, dim())])
}

/// Each group's heading and names, then how to see what one does.
fn command_index(app: &App, groups: &[Group]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = groups.iter().flat_map(|g| indexed(app, g)).collect();
    let how = "type a command's first letters to see what it does";
    lines.push(Line::styled(how, dim()));
    lines
}

/// A group's heading, bold, then its commands' names, each with its key.
fn indexed(app: &App, (heading, cs): &Group) -> [Line<'static>; 2] {
    [
        Line::styled(*heading, bold()),
        Line::from(named_commands(app, cs)),
    ]
}

/// Commands' names, two cells apart, each with its key.
fn named_commands(app: &App, cs: &[&Command]) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (i, c) in cs.iter().enumerate() {
        let gap = if i == 0 { "" } else { "  " };
        spans.push(Span::raw(format!("{gap}{}", c.name)));
        spans.extend(key_span(app, c.name, " "));
    }
    spans
}

/// The key bound to a command, dim, after a gap, if one is.
fn key_span(app: &App, name: &str, gap: &str) -> Option<Span<'static>> {
    let key = app.session.keys.bound_to(name)?;
    Some(Span::styled(format!("{gap}{key}"), dim()))
}
