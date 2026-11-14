//! Milestone 4's acceptance checks (docs/plan.md, Milestone 4): claims,
//! constraints and worlds, with a real SWI-Prolog behind each world.

use memex::app::{App, Event, Key, Mode, Quasi};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Modifier;
use std::path::{Path, PathBuf};

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("adversaria-m4-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn tap(app: &mut App, k: Key) {
    app.handle(Event::down(k));
    app.handle(Event::up(k));
}

fn typ(app: &mut App, s: &str) {
    for c in s.chars() {
        tap(app, Key::Char(c));
    }
}

fn command(app: &mut App, line: &str) {
    app.handle(Event::down(Key::Quasi(Quasi::Command)));
    typ(app, line);
    app.handle(Event::up(Key::Quasi(Quasi::Command)));
    assert!(
        !app.message.starts_with("no command") && !app.message.starts_with("not a"),
        "{line}: {}",
        app.message
    );
}

fn item(app: &mut App, text: &str) {
    tap(app, Key::Enter);
    typ(app, text);
}

fn leap(app: &mut App, pattern: &str) {
    app.handle(Event::down(Key::Quasi(Quasi::LeapForward)));
    typ(app, pattern);
    app.handle(Event::up(Key::Quasi(Quasi::LeapForward)));
    assert!(app.message.is_empty(), "leap {pattern}: {}", app.message);
    app.evaluate().unwrap(); // as the terminal loop does when the journal moves
}

struct Screen {
    lines: Vec<String>,
    buf: ratatui::buffer::Buffer,
}

fn screen(app: &App) -> Screen {
    let (w, h) = (200, 45);
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| memex::ui::draw(f, app)).unwrap();
    let buf = t.backend().buffer().clone();
    let lines = (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect();
    Screen { lines, buf }
}

impl Screen {
    fn has(&self, text: &str) -> bool {
        self.lines.iter().any(|l| l.contains(text))
    }
    fn assert(&self, text: &str) {
        assert!(
            self.has(text),
            "screen lacks {text:?}:\n{}",
            self.lines.join("\n")
        );
    }
    /// The modifiers of the first cell where text starts.
    fn style_of(&self, text: &str) -> Modifier {
        for (y, l) in self.lines.iter().enumerate() {
            if let Some(i) = l.find(text) {
                let x = l[..i].chars().count() as u16;
                return self.buf[(x, y as u16)].modifier;
            }
        }
        panic!("no {text:?}")
    }
}

/// The scene's essay: a belief, a claim, a derived sentence, a timeline and
/// the constraint one date per event.
fn scene(dir: &Path) -> App {
    let mut app = App::open(dir).unwrap();
    app.set_durable(false);
    typ(&mut app, "The programme");
    item(&mut app, "Buran flew once, unmanned, on 15 November 1988.");
    tap(&mut app, Key::Tab);
    command(&mut app, "fact date(buran_flight, 1988)");
    command(&mut app, "fact label(buran_flight, \"Buran's flight\")");
    item(&mut app, "Energia, its launcher, first flew in May 1987.");
    command(&mut app, "fact date(energia_launch, 1987)");
    command(
        &mut app,
        "fact label(energia_launch, \"Energia's first launch\")",
    );
    item(&mut app, "derived");
    command(
        &mut app,
        "derive The flight came {N:years} after Energia's first launch. where date(buran_flight, B), date(energia_launch, E), N is B - E",
    );
    item(&mut app, "How charts were reviewed");
    tap(&mut app, Key::BackTab);
    item(&mut app, "Talk notes: “the 1989 flight was the only one”.");
    tap(&mut app, Key::Tab);
    command(
        &mut app,
        "claim date(buran_flight, 1989) from my notes, conference talk",
    );
    item(&mut app, "Timeline (chart)");
    tap(&mut app, Key::BackTab);
    command(&mut app, "derive timeline where date(E, Y)");
    command(
        &mut app,
        "constraint one date per event: false :- date(E, D1), date(E, D2), D1 \\= D2.",
    );
    assert!(
        app.message.contains("fires on a planted violation"),
        "{}",
        app.message
    );
    app.flush();
    app.evaluate().unwrap();
    app
}

#[test]
fn scene_two_as_written() {
    // Plan: "The user story's scene 2 works as written": a claim from a talk,
    // attributed to its speaker, reported as a disagreement between sources,
    // not a broken fact; both items marked, naming the other; the right pane
    // shows the rule, the disagreement, and what depends on the date; a
    // fading hint suggests Suppose.
    let dir = temp("scene2");
    let mut app = scene(&dir);
    leap(&mut app, "talk notes");
    let s = screen(&app);
    s.assert("1 disagreement between sources");
    assert!(
        !s.has("constraint violation"),
        "a claim is not a belief: no violation"
    );
    s.assert("⚑ one date per event, sources disagree: 1988 here, 1989 in 2.1");
    s.assert("⚑ one date per event, sources disagree: 1989 here, 1988 in 1.1");
    assert!(
        s.style_of("⚑ one date per event, sources disagree: 1988")
            .contains(Modifier::DIM),
        "shown less loudly"
    );
    s.assert("[my notes, conference talk]");
    s.assert("The flight came a year after Energia's first launch. [derived]");
    s.assert("CONSTRAINT · ONE DATE PER EVENT");
    s.assert("    date(E, D1),");
    s.assert("    D1 \\= D2.");
    s.assert(&format!(
        "disagreements between sources · recomputed at journal {}",
        app.journal().entries().len()
    ));
    s.assert("buran_flight: 1988 (1.1) · 1989 (2.1)");
    s.assert("depends on these facts");
    s.assert("1.3 The flight came a year after");
    s.assert("    3 1987 Energia's first launch ── 1988 Bura …");
    s.assert("Hold SUPPOSE and type a statement to try it in a new world. Nothing in this world changes.");
    s.assert("fading hint · shown until Suppose has been used 4 more times");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn adopting_turns_a_disagreement_into_a_violation_and_back() {
    // Plan: adopt and retract with one key each. Catches an adoption that
    // does not make the claim a belief (so the conflict stays a mere
    // disagreement), or a retraction that does not undo it.
    let dir = temp("adopt");
    let mut app = scene(&dir);
    leap(&mut app, "talk notes");
    tap(&mut app, Key::F(3));
    let adopt = app.journal().entries().last().unwrap().clone();
    assert_eq!(adopt.kind(), "adopt");
    assert_eq!(
        adopt.body().unwrap()["source"],
        "my notes, conference talk",
        "the belief keeps its evidence"
    );
    app.evaluate().unwrap();
    let s = screen(&app);
    s.assert("1 constraint violation");
    assert!(
        s.style_of("⚑ one date per event, violation: 1988 here")
            .contains(Modifier::BOLD),
        "a violation is shown loudly"
    );
    tap(&mut app, Key::F(4));
    app.evaluate().unwrap();
    let s = screen(&app);
    s.assert("1 disagreement between sources");
    assert!(!s.has("constraint violation"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn interrogation_answers_its_five_questions() {
    // Plan: "interrogate with one key: who else claims it, who claims the
    // contrary, its provenance, the source's record, and a preview of
    // adopting it as a world".
    let dir = temp("interrogate");
    let mut app = scene(&dir);
    leap(&mut app, "talk notes");
    tap(&mut app, Key::F(2));
    app.evaluate().unwrap();
    let s = screen(&app);
    s.assert("INTERROGATE · WHAT IS CLAIMED HERE");
    s.assert("claims: the date of Buran's flight is 1989 — from my notes, conference talk");
    s.assert("  entry ");
    s.assert("who else claims it");
    s.assert("no one else");
    s.assert("who claims the contrary");
    s.assert("1.1: the date of Buran's flight is 1988 (one date per event)");
    s.assert("the source's record");
    s.assert("0 of its claims adopted, 0 of them retracted since");
    s.assert("adopting it would raise 1 violation of one date per event");
    s.assert("F3 adopts · F4 retracts an adoption");
    // The hint follows the configuration, not the default keys.
    app.bindings =
        memex::config::Config::parse("[keys]\nF3 = \"\"\nInsert = \"adopt\"\nF4 = \"\"\n")
            .unwrap()
            .bindings;
    let s2 = screen(&app);
    s2.assert("Insert adopts · command unadopt retracts an adoption");
    app.bindings = memex::config::Config::default().bindings;
    assert_eq!(
        app.journal().entries().last().unwrap().body().unwrap()["op"],
        "interrogate",
        "interrogation is journaled as an action"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn scene_three_as_written() {
    // Plan: "The user story's scene 3 works as written, except the
    // restatement by APE". Hold Suppose and type "the flight was in 1989";
    // "do you mean?"; Enter. A second world opens beside the actual one,
    // joined line by line: the derived sentence changes from "a year" to
    // "two years", the timeline's point moves, and the constraint has no
    // violation, because the source's date is retracted there. The actual
    // world is untouched. Then close the supposition.
    let dir = temp("scene3");
    let mut app = scene(&dir);
    let actual_head = app.tip;
    let actual_facts = memex::store::facts(app.journal().entries(), actual_head).unwrap();
    app.handle(Event::down(Key::Quasi(Quasi::Suppose)));
    typ(&mut app, "the flight was in 1989");
    app.handle(Event::up(Key::Quasi(Quasi::Suppose)));
    assert!(matches!(app.mode, Mode::ConfirmSuppose { .. }));
    let s = screen(&app);
    s.assert("SUPPOSE  the flight was in 1989");
    s.assert("Do you mean: Suppose that the date of Buran's flight is 1989.  ⏎ yes · Esc no");
    tap(&mut app, Key::Enter);
    assert!(
        app.message.contains("set aside in this world: 1.1"),
        "{}",
        app.message
    );
    let s = screen(&app);
    s.assert("worlds actual | suppose: the flight was in 1989");
    s.assert("WORLD · ACTUAL");
    s.assert("WORLD · SUPPOSE THE FLIGHT WAS IN 1989");
    s.assert("The flight came a year after Energia's first launch.  - - -");
    s.assert("The flight came two years after Energia's first launch.");
    s.assert("1987 Energia's first launch ── 1988 Buran's flight");
    s.assert("1987 Energia's first launch ── 1989 Buran's flight");
    s.assert("Constraint one date per event: 0 violations, 1 disagreement");
    s.assert("Constraint one date per event: 0 violations · 1.1 retracted here");
    s.assert("(fact retracted here)");
    // The actual world is untouched.
    assert_eq!(
        memex::store::facts(app.journal().entries(), actual_head).unwrap(),
        actual_facts
    );
    // A world is a branch of the undo tree: the supposition hangs off the actual head.
    let sup = app
        .journal()
        .entries()
        .iter()
        .find(|e| e.kind() == "suppose")
        .unwrap();
    assert_eq!(sup.parent_seq(), Some(actual_head));
    // Close it: back to the actual world, the branch kept.
    command(&mut app, "close");
    app.evaluate().unwrap();
    assert!(app.supposition.is_none());
    assert_eq!(app.world, "actual");
    let s = screen(&app);
    s.assert("1 disagreement between sources");
    s.assert("The flight came a year after Energia's first launch.");
    assert_eq!(
        memex::store::facts(app.journal().entries(), app.tip).unwrap(),
        actual_facts
    );
    assert!(
        app.journal()
            .entries()
            .iter()
            .any(|e| e.kind() == "suppose"),
        "the supposition stays in the journal"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_constraint_that_cannot_fire_is_not_kept() {
    // Plan: "Each constraint the user adopts is tried against a planted
    // violation". Catches a broken constraint kept, as if it guarded anything.
    let dir = temp("planted");
    let mut app = scene(&dir);
    let n = app.journal().entries().len();
    app.handle(Event::down(Key::Quasi(Quasi::Command)));
    typ(
        &mut app,
        "constraint broken: false :- date(E, D), date(E, D), D \\= D.",
    );
    app.handle(Event::up(Key::Quasi(Quasi::Command)));
    assert!(app.message.contains("did not fire"), "{}", app.message);
    assert_eq!(app.journal().entries().len(), n, "nothing written");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_constraint_on_negative_or_zero_values_can_be_kept() {
    // The planted check gave variables only 1, 2, ..., so a constraint
    // such as "no balance below zero" could never fire on them and was
    // refused. Catches that, and a constraint that cannot fire kept.
    let dir = temp("planted-sign");
    let mut app = scene(&dir);
    for (line, kept) in [
        ("constraint overdrawn: false :- balance(B), B < 0.", true),
        ("constraint empty: false :- stock(S), S =< 0.", true),
        (
            "constraint both: false :- low(A), high(B), A < 0, B > 0.",
            true,
        ),
        ("constraint never: false :- n(X), X < 0, X > 0.", false),
    ] {
        app.handle(Event::down(Key::Quasi(Quasi::Command)));
        typ(&mut app, line);
        app.handle(Event::up(Key::Quasi(Quasi::Command)));
        assert_eq!(
            app.message.contains("kept: it fires"),
            kept,
            "{line}: {}",
            app.message
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn recomputation_touches_only_dependent_nodes() {
    // Plan: "Recomputation after a change touches only the nodes that depend
    // on it (show the count, from milestone 0's spike method)." Each node
    // counts its evaluations. A new date for a third event re-evaluates the
    // nodes that read every date (the constraint, the timeline) once each;
    // not the sentence, which reads only the dates of Buran's flight and
    // Energia's launch; and not the capital constraint.
    let dir = temp("recompute");
    let mut app = scene(&dir);
    item(&mut app, "Paris is the capital of France.");
    command(&mut app, "fact capital(france, paris)");
    command(
        &mut app,
        "constraint one capital per country: false :- capital(C, X), capital(C, Y), X \\= Y.",
    );
    app.flush();
    app.evaluate().unwrap();
    let constraints: Vec<(u64, String)> = app
        .outline
        .constraints
        .iter()
        .map(|(s, c)| (*s, c.name.clone()))
        .collect();
    let derived: Vec<u64> = app.outline.derived.keys().copied().collect();
    let nodes: Vec<(u64, String)> = constraints
        .into_iter()
        .chain(derived.iter().map(|d| {
            (
                *d,
                app.outline.derived[d]
                    .template
                    .chars()
                    .take(16)
                    .collect::<String>(),
            )
        }))
        .collect();
    let count = |app: &mut App| -> Vec<i64> {
        nodes
            .iter()
            .map(|(id, _)| app.current_engine().unwrap().evals(*id).unwrap())
            .collect()
    };
    let before = count(&mut app);
    item(&mut app, "The programme was cancelled in 1993.");
    command(&mut app, "fact date(cancellation, 1993)");
    app.flush();
    app.evaluate().unwrap();
    let after = count(&mut app);
    let delta: Vec<(String, i64)> = nodes
        .iter()
        .zip(before.iter().zip(&after))
        .map(|((_, n), (b, a))| (n.clone(), a - b))
        .collect();
    eprintln!("evaluations after one new date fact: {delta:?}");
    for (name, d) in &delta {
        let want = match name.as_str() {
            "one date per event" | "timeline" => 1,
            "one capital per country" | "The flight came " => 0,
            n => panic!("unexpected node {n}"),
        };
        assert_eq!(*d, want, "{name}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn replay_reproduces_claims_constraints_and_worlds() {
    // Catches state of milestone 4 that lives outside the journal: a replay
    // must give the same outline, view, world and reasoning.
    let dir = temp("replay4");
    let mut app = scene(&dir);
    leap(&mut app, "talk notes");
    tap(&mut app, Key::F(2));
    tap(&mut app, Key::F(3));
    tap(&mut app, Key::F(4));
    app.handle(Event::down(Key::Quasi(Quasi::Suppose)));
    typ(&mut app, "the flight was in 1989");
    app.handle(Event::up(Key::Quasi(Quasi::Suppose)));
    tap(&mut app, Key::Enter);
    app.flush();
    app.evaluate().unwrap();
    let mut r = App::open(&dir).unwrap();
    r.evaluate().unwrap();
    assert_eq!(r.outline, app.outline);
    assert_eq!(r.view, app.view);
    assert_eq!(
        (r.tip, &r.world, &r.supposition),
        (app.tip, &app.world, &app.supposition)
    );
    assert_eq!(r.eval, app.eval);
    let n = screen(&app).lines.len() - 1;
    assert_eq!(screen(&r).lines[..n], screen(&app).lines[..n]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_text_cursor_sits_on_its_item_below_mark_lines() {
    // Catches the text cursor drawn by item count, which puts it on a mark
    // line when an item above has marks (found 12 November 2026).
    use ratatui::backend::Backend;
    let dir = temp("cursor-row");
    let mut app = scene(&dir);
    leap(&mut app, "talk notes");
    let mut t = Terminal::new(TestBackend::new(200, 45)).unwrap();
    t.draw(|f| memex::ui::draw(f, &app)).unwrap();
    let p = t.backend_mut().get_cursor_position().unwrap();
    let buf = t.backend().buffer().clone();
    let row: String = (0..200)
        .map(|x| buf[(x, p.y)].symbol().to_string())
        .collect();
    assert!(row.contains("Talk notes"), "the cursor is on {row:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Which command the last world-ending view entry records: "close" or
/// "commit".
fn ended_by(app: &App) -> &'static str {
    let e = app
        .journal()
        .entries()
        .iter()
        .rev()
        .find(|e| e.kind() == "view" && e.body().is_some_and(|b| b["op"] == "world"))
        .expect("a world-ending view entry");
    let b = e.body().unwrap();
    ["close", "commit"]
        .into_iter()
        .find(|k| b[*k] == true)
        .unwrap_or("neither")
}

#[test]
fn ending_a_world_records_whether_it_was_closed_or_committed() {
    let dir = temp("ended-by");
    let mut app = App::open(&dir).unwrap();
    command(&mut app, "suppose first");
    command(&mut app, "close");
    let closed = ended_by(&app);
    command(&mut app, "suppose second");
    command(&mut app, "commit");
    assert_eq!((closed, ended_by(&app)), ("close", "commit"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_item_with_no_claim_cannot_be_adopted() {
    // Catches adopt writing nothing, silently, for an item that states a
    // belief but no claim.
    let dir = temp("adopt-none");
    let mut app = App::open(&dir).unwrap();
    item(&mut app, "the sky");
    command(&mut app, "fact colour(sky, blue)");
    let before = app.journal().entries().len();
    command(&mut app, "adopt");
    assert_eq!(
        (app.message.as_str(), app.journal().entries().len()),
        ("the target states no claim to adopt", before)
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_retracted_constraint_is_not_read() {
    // Catches a world reading every constraint in its outline, including
    // one whose definition is retracted in it.
    let dir = temp("retracted-constraint");
    let mut app = App::open(&dir).unwrap();
    item(&mut app, "rules");
    command(&mut app, "constraint same: false :- p(X), q(X).");
    let seq = app.journal().entries().last().unwrap().seq;
    let count = |app: &mut App| {
        app.evaluate().unwrap();
        app.eval[0].as_ref().map_or(0, |e| e.constraints.len())
    };
    let before = count(&mut app);
    command(&mut app, &format!("retract {seq}"));
    tap(&mut app, Key::Enter);
    assert_eq!((before, count(&mut app)), (1, 0));
    std::fs::remove_dir_all(&dir).unwrap();
}
