//! The foot of the screen: what is typed or asked, and under it what to do
//! next, or the message; for each quasimode, what it is doing and what its
//! release will do (docs/guide.md § 2.1).

use super::*;

/// The foot: what is typed or asked, and under it what to do, or the
/// message.
pub(super) fn draw_prompt(f: &mut Frame, lines: [Line<'static>; 2], rows: [Rect; 2]) {
    let [line, said] = lines;
    f.render_widget(wrapped(vec![line]), rows[0]);
    f.render_widget(wrapped(vec![said]), rows[1]);
}

/// The foot's two lines: what is typed, and what the memex says, dim.
pub(super) fn foot_lines(app: &App) -> [Line<'static>; 2] {
    let (line, said) = foot(app);
    [line, Line::styled(said, dim())]
}

/// The foot's two lines, by the mode.
fn foot(app: &App) -> (Line<'static>, String) {
    match &app.session.mode {
        Mode::Quasi(a) => (quasi_line(app, a), quasi_said(app, a)),
        Mode::ConfirmRetract {
            target,
            dependents,
            asked,
        } => retract_foot(app, *target, dependents, *asked),
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

/// What the memex says under a quasimode: why it did not end, if it was
/// refused, with how to leave it; else what it does and how it ends.
fn quasi_said(app: &App, a: &Active) -> String {
    let refused = &app.session.message;
    match (refused.is_empty(), refused.contains("Esc")) {
        (true, _) => quasi_guide(a),
        (false, true) => refused.clone(),
        (false, false) => format!("{refused} · Esc cancels"),
    }
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
    format!("no command, so read as {read} · {said}Esc cancels · {end} {does}")
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
fn retract_foot(
    app: &App,
    target: u64,
    dependents: &[u64],
    by: TakeBack,
) -> (Line<'static>, String) {
    let (name, about) = by.asking(target, &app.notebook.said_of(target));
    (asked(name, about), retract_guide(dependents, by))
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
fn retract_guide(dependents: &[u64], by: TakeBack) -> String {
    let numbers: Vec<String> = dependents.iter().map(u64::to_string).collect();
    let dep = match numbers.is_empty() {
        true => "nothing depends on it".to_string(),
        false => format!(
            "entries that depend on it and will no longer apply: {}",
            numbers.join(", ")
        ),
    };
    format!("{dep} · {}", by.answers())
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
