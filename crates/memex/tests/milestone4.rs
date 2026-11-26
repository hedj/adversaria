//! Milestone 4's acceptance checks (docs/plan.md, Milestone 4): claims,
//! constraints and worlds, with a real SWI-Prolog behind each world.

use memex::app::{App, Event, Key, Mode, Quasi};
use memex::config::Config;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier, Style};
use serde_json::json;

mod support;
use support::{Room, replayed};

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
    support::command(app, line);
}

fn item(app: &mut App, text: &str) {
    tap(app, Key::Enter);
    typ(app, text);
}

fn leap(app: &mut App, pattern: &str) {
    app.handle(Event::down(Key::Quasi(Quasi::LeapDown)));
    typ(app, pattern);
    app.handle(Event::up(Key::Quasi(Quasi::LeapDown)));
    assert!(
        app.session.message.is_empty(),
        "leap {pattern}: {}",
        app.session.message
    );
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
    fn style_of(&self, text: &str) -> Style {
        for (y, l) in self.lines.iter().enumerate() {
            if let Some(i) = l.find(text) {
                let x = l[..i].chars().count() as u16;
                return self.buf[(x, y as u16)].style();
            }
        }
        panic!("no {text:?}")
    }
}

/// The scene's essay: a belief, a claim, a derived sentence, a timeline and
/// the constraint one date per event.
fn scene(name: &str) -> Room {
    let mut app = Room::new(&format!("m4-{name}"));
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
        app.session.message.contains("fires on a planted violation"),
        "{}",
        app.session.message
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
    let mut app = scene("scene2");
    leap(&mut app, "talk notes");
    let s = screen(&app);
    s.assert("1 disagreement between sources");
    assert!(
        !s.has("constraint violation"),
        "a claim is not a belief: no violation"
    );
    s.assert("? sources disagree under “one date per event”: 1988 here; 1989 in 2.1");
    s.assert("? sources disagree under “one date per event”: 1989 here; 1988 in 1.1");
    assert!(
        s.style_of("? sources disagree under “one date per event”: 1988")
            .fg
            == memex::ui::disagreement().fg,
        "shown as a disagreement, amber"
    );
    s.assert("[my notes, conference talk]");
    s.assert("The flight came a year after Energia's first launch. [derived]");
    s.assert("CONSTRAINT · ONE DATE PER EVENT");
    s.assert("    date(E, D1),");
    s.assert("    D1 \\= D2.");
    s.assert("│  disagreements between sources");
    s.assert("buran_flight: 1988 (1.1) · 1989 (2.1)");
    s.assert("depends on these facts");
    s.assert("1.3 The flight came a year after");
    s.assert("    3 1987 Energia's first launch ── 1988 Bura …");
    s.assert("Try a what-if: tap Super to suppose a statement, type it, and press Enter. The actual world stays as it is.");
}

#[test]
fn adopting_turns_a_disagreement_into_a_violation_and_back() {
    // Plan: adopt and retract with one key each. Catches an adoption that
    // does not make the claim a belief (so the conflict stays a mere
    // disagreement), or a retraction that does not undo it.
    let mut app = scene("adopt");
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
    let top = s.style_of("1 constraint violation").fg;
    assert_eq!(
        top,
        memex::ui::violation().fg,
        "the top bar's count, red too"
    );
    assert!(
        s.style_of("‼ breaks “one date per event”: 1988 here").fg == memex::ui::violation().fg,
        "a violation is shown as one, red"
    );
    tap(&mut app, Key::F(4));
    tap(&mut app, Key::Enter);
    app.evaluate().unwrap();
    let s = screen(&app);
    s.assert("1 disagreement between sources");
    assert!(!s.has("constraint violation"));
}

#[test]
fn the_inspector_answers_a_claims_five_questions() {
    // Plan: "interrogate with one key: who else claims it, who claims the
    // contrary, its provenance, the source's record, and a preview of
    // adopting it as a world". Since decision 4 of docs/concepts.md, with
    // no key: the inspector asks them at the claim's item.
    let mut app = scene("interrogate");
    leap(&mut app, "talk notes");
    app.evaluate().unwrap();
    let s = screen(&app);
    s.assert("CLAIMS OF 2.1 · WHO SAYS THEM");
    s.assert("claim  2.1: the date of Buran's flight is 1989");
    s.assert("   from my notes, conference talk");
    s.assert("who else claims it");
    s.assert("no one else");
    s.assert("who claims the contrary");
    s.assert("1.1: the date of Buran's flight is 1988 (one date per event)");
    s.assert("the source's record");
    s.assert("0 of its claims adopted, 0 of them retracted since");
    s.assert("adopting it would raise 1 violation of one date per event");
    s.assert("F3 adopts · F4 retracts");
    tap(&mut app, Key::Down);
    tap(&mut app, Key::Up);
    app.flush();
    let last = app
        .journal()
        .entries()
        .last()
        .unwrap()
        .body()
        .unwrap()
        .clone();
    assert_eq!(
        (&last["op"], &last["inspects"]),
        (&json!("move"), &json!(app.cursor())),
        "the check is journaled: the view entry names the claim item shown"
    );
}

#[test]
fn the_interrogation_hint_follows_the_configured_keys() {
    // Catches a hint that names the default keys after they are rebound.
    let mut app = scene("interrogate-keys");
    leap(&mut app, "talk notes");
    app.evaluate().unwrap();
    let config = Config::parse("[keys]\nF3 = \"\"\nInsert = \"adopt\"\nF4 = \"\"\n");
    app.session.keys = config.unwrap().keys();
    screen(&app).assert("Insert adopts · command retract retracts");
}

/// The scene, with a second source that claims what the talk notes claim.
fn wikipedia_too(name: &str) -> Room {
    let mut app = scene(name);
    leap(&mut app, "talk notes");
    item(&mut app, "Wikipedia gives 1989 too.");
    command(&mut app, "claim date(buran_flight, 1989) from Wikipedia");
    app.flush();
    app
}

/// What the interrogation of the talk notes shows.
fn talk_interrogated(app: &mut App) -> Screen {
    leap(app, "Talk notes:");
    app.evaluate().unwrap();
    screen(app)
}

#[test]
fn the_contrary_of_a_claim_is_not_the_claim_itself() {
    // Catches a finding's rows that keep the claim's own fact.
    let mut app = scene("interrogate-own");
    let s = talk_interrogated(&mut app);
    s.assert("1.1: the date of Buran's flight is 1988 (one date per event)");
    assert!(!s.has("is 1989 (one date per event)"));
}

#[test]
fn an_adoption_kept_is_not_counted_as_retracted() {
    // Catches a record that counts each adoption as retracted.
    let mut app = scene("interrogate-kept");
    leap(&mut app, "talk notes");
    tap(&mut app, Key::F(3));
    let s = talk_interrogated(&mut app);
    s.assert("1 of its claims adopted, 0 of them retracted since");
}

#[test]
fn another_source_that_claims_the_fact_is_listed() {
    // Catches a list of who else claims it that keeps beliefs alone.
    let mut app = wikipedia_too("interrogate-others");
    talk_interrogated(&mut app).assert("2.2 (Wikipedia)");
}

#[test]
fn another_sources_adoption_is_not_on_the_record() {
    // Catches a source's record that counts every adoption.
    let mut app = wikipedia_too("interrogate-record");
    tap(&mut app, Key::F(3));
    let s = talk_interrogated(&mut app);
    s.assert("0 of its claims adopted, 0 of them retracted since");
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
    let mut app = scene("scene3");
    let actual_head = app.notebook.tip;
    let actual_facts = facts_at(&app, actual_head);
    suppose_offers_its_reading(&mut app);
    tap(&mut app, Key::Enter);
    the_worlds_are_side_by_side(&app);
    // The actual world is untouched.
    assert_eq!(facts_at(&app, actual_head), actual_facts);
    // A world is a branch of the undo tree: the supposition hangs off the actual head.
    let sup = first_of_kind(&app, "suppose").expect("a supposition");
    assert_eq!(sup.parent_seq(), Some(actual_head));
    // Close it: back to the actual world, the branch kept.
    command(&mut app, "close");
    closing_returns_to_the_actual_world(&mut app, &actual_facts);
}

/// The facts of the world whose head is an entry.
fn facts_at(app: &App, head: u64) -> Vec<(u64, format::terms::Term)> {
    memex::store::facts(app.journal().entries(), head).unwrap()
}

/// The first entry of a kind, if there is one.
fn first_of_kind<'a>(app: &'a App, kind: &str) -> Option<&'a format::record::Entry> {
    app.journal().entries().iter().find(|e| e.kind() == kind)
}

/// Scene 3, Suppose held on "the flight was in 1989" and released: "do you
/// mean?", with the reading.
fn suppose_offers_its_reading(app: &mut App) {
    app.handle(Event::down(Key::Quasi(Quasi::Suppose)));
    typ(app, "the flight was in 1989");
    app.handle(Event::up(Key::Quasi(Quasi::Suppose)));
    assert!(matches!(app.session.mode, Mode::ConfirmSuppose { .. }));
    let s = screen(app);
    s.assert("SUPPOSE  the flight was in 1989");
    s.assert("Do you mean: Suppose that the date of Buran's flight is 1989.  Enter: yes · Esc: no");
}

/// Scene 3, the reading taken: the source's date set aside, and the two
/// worlds side by side, the sentence, the timeline and the constraint
/// each changed.
fn the_worlds_are_side_by_side(app: &App) {
    let said = &app.session.message;
    assert!(said.contains("set aside in this world: 1.1"), "{said}");
    let s = screen(app);
    s.assert("worlds actual | suppose: the flight was in 1989");
    s.assert("WORLD · ACTUAL");
    s.assert("WORLD · SUPPOSE THE FLIGHT WAS IN 1989");
    s.assert("The flight came a year after Energia's first launch.  - - -");
    s.assert("The flight came two years after Energia's first launch.");
    s.assert("1987 Energia's first launch ── 1988 Buran's flight");
    s.assert("1987 Energia's first launch ── 1989 Buran's flight");
    s.assert("Constraint one date per event:");
    s.assert("  0 violations, 1 disagreement");
    s.assert("  0 violations, 0 disagreements");
    s.assert("  1.1 set aside here");
    s.assert("The programme");
    s.assert("How charts were reviewed");
    // The item states the date and the label; only the date is set aside,
    // so the note names it and the text, still a label, is not struck.
    // The note wraps after "date(buran_flight,".
    let gone = "1.1. Buran flew once, unmanned, on 15 November 1988.  (1.1 set aside here: date(buran_flight,";
    s.assert(gone);
    let struck = s.style_of(gone).add_modifier;
    assert!(
        !struck.contains(Modifier::CROSSED_OUT),
        "an item that keeps a fact is not struck through"
    );
    let changed = s.style_of("The flight came two years").add_modifier;
    assert!(
        changed.contains(Modifier::BOLD | Modifier::UNDERLINED),
        "the change is marked"
    );
    let one = s.style_of("1 disagreement").fg;
    assert_eq!(
        one,
        memex::ui::disagreement().fg,
        "a count not zero is coloured"
    );
    let zero = s.style_of("0 disagreements");
    assert!(
        matches!(zero.fg, None | Some(Color::Reset)),
        "a count of zero is plain"
    );
    let mark = Modifier::BOLD | Modifier::UNDERLINED;
    assert!(
        zero.add_modifier.contains(mark),
        "a count that changed is marked"
    );
    let same = s.style_of("0 violations").add_modifier;
    assert!(
        !same.contains(Modifier::BOLD),
        "a count the same in both is not"
    );
}

/// Scene 3, closed: the actual world again, as it was, and the
/// supposition kept in the journal.
fn closing_returns_to_the_actual_world(app: &mut App, actual_facts: &[(u64, format::terms::Term)]) {
    app.evaluate().unwrap();
    assert!(app.reason.supposition.is_none());
    assert_eq!(app.reason.world(), "actual");
    let s = screen(app);
    s.assert("1 disagreement between sources");
    s.assert("The flight came a year after Energia's first launch.");
    assert_eq!(facts_at(app, app.notebook.tip), actual_facts);
    let kept = first_of_kind(app, "suppose").is_some();
    assert!(kept, "the supposition stays in the journal");
}

#[test]
fn a_constraint_that_cannot_fire_is_not_kept() {
    // Plan: "Each constraint the user adopts is tried against a planted
    // violation". Catches a broken constraint kept, as if it guarded anything.
    let mut app = scene("planted");
    let n = app.journal().entries().len();
    app.handle(Event::down(Key::Quasi(Quasi::Command)));
    typ(
        &mut app,
        "constraint broken: false :- date(E, D), date(E, D), D \\= D.",
    );
    app.handle(Event::up(Key::Quasi(Quasi::Command)));
    assert!(
        app.session.message.contains("did not fire"),
        "{}",
        app.session.message
    );
    assert_eq!(app.journal().entries().len(), n, "nothing written");
}

#[test]
fn a_constraint_on_negative_or_zero_values_can_be_kept() {
    // The planted check gave variables only 1, 2, ..., so a constraint
    // such as "no balance below zero" could never fire on them and was
    // refused. Catches that, and a constraint that cannot fire kept.
    let mut app = scene("planted-sign");
    for (line, kept) in [
        ("constraint overdrawn: false :- balance(B), B < 0.", true),
        ("constraint empty: false :- stock(S), S =< 0.", true),
        (
            "constraint both: false :- low(A), high(B), A < 0, B > 0.",
            true,
        ),
        ("constraint never: false :- n(X), X < 0, X > 0.", false),
    ] {
        app.typed_command(line);
        let said = &app.session.message;
        assert_eq!(said.contains("kept: it fires"), kept, "{line}: {said}");
    }
}

#[test]
fn recomputation_touches_only_dependent_nodes() {
    // Plan: "Recomputation after a change touches only the nodes that depend
    // on it (show the count, from milestone 0's spike method)." Each node
    // counts its evaluations. A new date for a third event re-evaluates the
    // nodes that read every date (the constraint, the timeline) once each;
    // not the sentence, which reads only the dates of Buran's flight and
    // Energia's launch; and not the capital constraint.
    let mut app = scene("recompute");
    with_a_capital(&mut app);
    let nodes = nodes_of(&app.notebook.outline);
    let before = evals(&mut app, &nodes);
    with_a_new_date(&mut app);
    let after = evals(&mut app, &nodes);
    let delta = deltas(&nodes, &before, &after);
    eprintln!("evaluations after one new date fact: {delta:?}");
    for (name, d) in &delta {
        assert_eq!(*d, evaluations_wanted(name), "{name}");
    }
}

/// The scene, with a capital and a constraint of one capital per country,
/// evaluated.
fn with_a_capital(app: &mut App) {
    item(app, "Paris is the capital of France.");
    command(app, "fact capital(france, paris)");
    let one_capital = "false :- capital(C, X), capital(C, Y), X \\= Y.";
    command(
        app,
        &format!("constraint one capital per country: {one_capital}"),
    );
    app.flush();
    app.evaluate().unwrap();
}

/// A third event's date, evaluated.
fn with_a_new_date(app: &mut App) {
    item(app, "The programme was cancelled in 1993.");
    command(app, "fact date(cancellation, 1993)");
    app.flush();
    app.evaluate().unwrap();
}

/// The outline's nodes, each with a name: its constraints by name, and
/// its derived items by their template's first 16 characters.
fn nodes_of(o: &memex::outline::Outline) -> Vec<(u64, String)> {
    let constraints = o.constraints.iter().map(|(s, c)| (*s, c.name.clone()));
    let template = |d: &u64| o.derived[d].template.chars().take(16).collect::<String>();
    let derived = o.derived.keys().map(|d| (*d, template(d)));
    constraints.chain(derived).collect()
}

/// How many times each node has been evaluated.
fn evals(app: &mut App, nodes: &[(u64, String)]) -> Vec<i64> {
    let engine = app.current_engine().unwrap();
    nodes
        .iter()
        .map(|(id, _)| engine.evals(*id).unwrap())
        .collect()
}

/// How many more times each node was evaluated, by its name.
fn deltas(nodes: &[(u64, String)], before: &[i64], after: &[i64]) -> Vec<(String, i64)> {
    let counts = before.iter().zip(after);
    nodes
        .iter()
        .zip(counts)
        .map(|((_, n), (b, a))| (n.clone(), a - b))
        .collect()
}

/// How many more times a node should be evaluated after a new date: once
/// for those that read every date, the constraint and the timeline; not at
/// all for the sentence, which reads only two, nor for the capitals.
fn evaluations_wanted(name: &str) -> i64 {
    match name {
        "one date per event" | "timeline" => 1,
        "one capital per country" | "The flight came " => 0,
        n => panic!("unexpected node {n}"),
    }
}

#[test]
fn a_second_adoption_is_the_one_retracted() {
    // Catches the retraction of an item's first adoption, already
    // retracted, in place of its last.
    let mut app = scene("adopt-twice");
    leap(&mut app, "talk notes");
    tap(&mut app, Key::F(3));
    tap(&mut app, Key::F(4));
    tap(&mut app, Key::Enter);
    tap(&mut app, Key::F(3));
    let second = app.journal().entries().last().unwrap().seq;
    tap(&mut app, Key::F(4));
    tap(&mut app, Key::Enter);
    assert!(
        app.session
            .message
            .starts_with(&format!("entry {second} retracted")),
        "{}",
        app.session.message
    );
}

#[test]
fn a_disagreement_shown_first_is_recorded() {
    // Catches the first observation written only when it shows a
    // violation: the scene's first shows a disagreement alone.
    let app = scene("observed-first");
    let entries = app.journal().entries();
    let first = entries
        .iter()
        .find_map(|e| e.body().filter(|b| b["op"] == "observed"));
    let first = first.expect("an observation was recorded");
    assert_eq!(first["violations"], serde_json::json!([]));
    assert_ne!(first["disagreements"], serde_json::json!([]));
}

#[test]
fn replay_reproduces_claims_constraints_and_worlds() {
    // Catches state of milestone 4 that lives outside the journal: a replay
    // must give the same outline, view, world and reasoning.
    let mut app = scene("replay4");
    interrogate_adopt_retract_suppose(&mut app);
    let mut r = replayed(&app.dir);
    r.evaluate().unwrap();
    assert_eq!(r.notebook.outline, app.notebook.outline);
    assert_eq!(r.session.view, app.session.view);
    assert_eq!(r.notebook.tip, app.notebook.tip);
    assert_eq!(r.reason.world(), app.reason.world());
    assert_eq!(r.reason.supposition, app.reason.supposition);
    assert_eq!(r.reason.eval, app.reason.eval);
    let n = screen(&app).lines.len() - 1;
    assert_eq!(screen(&r).lines[..n], screen(&app).lines[..n]);
}

/// On the talk notes: interrogate, adopt, retract the adoption, and
/// suppose the flight was in 1989; flushed and evaluated.
fn interrogate_adopt_retract_suppose(app: &mut App) {
    leap(app, "talk notes");
    tap(app, Key::F(3));
    tap(app, Key::F(4));
    tap(app, Key::Enter);
    app.handle(Event::down(Key::Quasi(Quasi::Suppose)));
    typ(app, "the flight was in 1989");
    app.handle(Event::up(Key::Quasi(Quasi::Suppose)));
    tap(app, Key::Enter);
    app.flush();
    app.evaluate().unwrap();
}

#[test]
fn the_text_cursor_sits_on_its_item_below_mark_lines() {
    // Catches the text cursor drawn by item count, which puts it on a mark
    // line when an item above has marks (found 12 November 2026).
    use ratatui::backend::Backend;
    let mut app = scene("cursor-row");
    leap(&mut app, "talk notes");
    let mut t = Terminal::new(TestBackend::new(200, 45)).unwrap();
    t.draw(|f| memex::ui::draw(f, &app)).unwrap();
    let p = t.backend_mut().get_cursor_position().unwrap();
    let buf = t.backend().buffer().clone();
    let row: String = (0..200)
        .map(|x| buf[(x, p.y)].symbol().to_string())
        .collect();
    assert!(row.contains("Talk notes"), "the cursor is on {row:?}");
}

/// Which command the last world-ending view entry records: "close" or
/// "commit".
fn ended_by(app: &App) -> &'static str {
    let b = last_world_ending(app);
    let ways = ["close", "commit"];
    ways.into_iter()
        .find(|k| b[*k] == true)
        .unwrap_or("neither")
}

/// The body of the last view entry that ends a world.
fn last_world_ending(app: &App) -> &serde_json::Value {
    let ends = |e: &format::record::Entry| {
        e.kind() == "view" && e.body().is_some_and(|b| b["op"] == "world")
    };
    let e = app.journal().entries().iter().rev().find(|e| ends(e));
    e.expect("a world-ending view entry").body().unwrap()
}

#[test]
fn ending_a_world_records_whether_it_was_closed_or_committed() {
    let mut app = Room::new("m4-ended-by");
    command(&mut app, "suppose first");
    command(&mut app, "close");
    let closed = ended_by(&app);
    command(&mut app, "suppose second");
    command(&mut app, "commit");
    assert_eq!((closed, ended_by(&app)), ("close", "commit"));
}

#[test]
fn an_item_with_no_claim_cannot_be_adopted() {
    // Catches adopt writing nothing, silently, for an item that states a
    // belief but no claim.
    let mut app = Room::new("m4-adopt-none");
    item(&mut app, "the sky");
    command(&mut app, "fact colour(sky, blue)");
    let before = app.entry_count();
    support::refused(&mut app, "adopt");
    assert_eq!(app.session.message, "the target states no claim to adopt");
    assert_eq!(app.entry_count(), before);
}

#[test]
fn a_retracted_constraint_is_not_read() {
    // Catches a world reading every constraint in its outline, including
    // one whose definition is retracted in it. The fact keeps the world
    // read once the constraint is gone: a world with no facts reads nothing.
    let mut app = Room::new("m4-retracted-constraint");
    item(&mut app, "rules");
    command(&mut app, "fact p(1)");
    command(&mut app, "constraint same: false :- p(X), q(X).");
    let seq = app.journal().entries().last().unwrap().seq;
    let before = constraints_read(&mut app);
    command(&mut app, &format!("retract {seq}"));
    tap(&mut app, Key::Enter);
    assert_eq!((before, constraints_read(&mut app)), (Some(1), Some(0)));
}

/// How many constraints the world reads, once evaluated; none if it was
/// not.
fn constraints_read(app: &mut App) -> Option<usize> {
    app.evaluate().unwrap();
    let e = app.reason.eval[0].as_ref();
    e.map(|e| e.constraints.len())
}

#[test]
fn an_observation_is_not_recorded_again_after_reopening() {
    // Catches the last observation kept only while the memex runs: after a
    // replay, what is shown unchanged is not written again.
    let room = scene("observed-again");
    let entries = room.journal().entries().len();
    let mut app = replayed(&room.dir);
    app.evaluate().unwrap();
    assert_eq!(app.journal().entries().len(), entries);
}

#[test]
fn an_observation_names_each_finding_by_its_constraint_and_facts() {
    // The measures read observations, so what they hold is the journal's:
    // each finding as its constraint and the facts it rests on, sorted,
    // and the world they were seen in.
    let app = scene("observed-names");
    let entries = app.journal().entries().iter();
    let observed = entries.filter_map(|e| e.body().filter(|b| b["op"] == "observed"));
    let first = observed.into_iter().next().unwrap();
    let names = "one date per event: date(buran_flight, 1988) · date(buran_flight, 1989)";
    assert_eq!(first["disagreements"], serde_json::json!([names]));
    assert_eq!(first["world"], "actual");
}

#[test]
fn what_reasoning_records_of_its_own_is_not_reasoned_over_again() {
    // Catches where the journal was taken before reasoning ran: what
    // reasoning recorded of its own, what it showed, moved the journal,
    // and the window reasoned over the same world again at its next frame.
    let mut r = Room::new("reasoned-once");
    r.items(&[
        "The date of Buran-flight is 1988.",
        "The date of Buran-flight is 1989.",
        "Nothing has more than 1 date.",
    ]);
    r.app.idle();
    let observed = r.count("view");
    assert!(r.app.reason_if_moved(), "the items moved the journal");
    assert!(
        r.count("view") > observed,
        "reasoning recorded what it showed"
    );
    assert!(!r.app.reason_if_moved());
}

/// Two dates for one flight and a rule against two, read: the violation
/// shown and recorded.
fn two_dates(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&[
        "The date of Buran-flight is 1988.",
        "The date of Buran-flight is 1989.",
        "Nothing has more than 1 date.",
    ]);
    r.app.idle();
    r.app.evaluate().unwrap();
    assert_eq!(
        last_observed(&r)["violations"].as_array().map(Vec::len),
        Some(1)
    );
    r
}

/// The body of the last observed entry.
fn last_observed(r: &Room) -> serde_json::Value {
    let bodies = r.app.journal().entries().iter().filter_map(|e| e.body());
    let last = bodies.rev().find(|b| b["op"] == "observed");
    last.cloned().expect("an observed entry")
}

/// What the room does, read and reasoned over: why the last observed
/// entry says each violation went.
fn why_gone(r: &mut Room, act: impl FnOnce(&mut Room)) -> Vec<String> {
    act(r);
    r.app.flush();
    r.app.idle();
    r.app.evaluate().unwrap();
    let last = last_observed(r);
    let gone = last["gone"].as_array().cloned().unwrap_or_default();
    gone.iter()
        .map(|g| g["why"].as_str().unwrap_or("").to_string())
        .collect()
}

/// The cursor on the nth item shown, its text cursor at the end.
fn at_end_of(r: &mut Room, n: usize) {
    r.put_cursor(r.outline().in_order()[n]);
    r.tap(&[Key::End]);
}

// The next four catch every violation that goes counted as corrected,
// though its rule was deleted, an item under it deleted, or its text
// made unread.

#[test]
fn a_violation_whose_rule_is_deleted_went_with_its_rule() {
    let mut r = two_dates("gone-rule");
    let gone = why_gone(&mut r, |r| (at_end_of(r, 2), r.command("delete")).1);
    assert_eq!(gone, ["rule"]);
}

#[test]
fn a_violation_whose_item_is_deleted_went_with_the_item() {
    let mut r = two_dates("gone-deleted");
    let gone = why_gone(&mut r, |r| (at_end_of(r, 0), r.command("delete")).1);
    assert_eq!(gone, ["deleted"]);
}

#[test]
fn a_violation_whose_text_no_longer_reads_went_unread() {
    let mut r = two_dates("gone-unread");
    let gone = why_gone(&mut r, |r| {
        (at_end_of(r, 0), r.typ(" — so the notes say")).1
    });
    assert_eq!(gone, ["unread"]);
}

#[test]
fn a_violation_whose_fact_is_mended_was_corrected() {
    let mut r = two_dates("gone-corrected");
    let mend = |r: &mut Room| r.tap(&[Key::Backspace; 5]);
    let gone = why_gone(&mut r, |r| (at_end_of(r, 0), mend(r), r.typ("1989.")).2);
    assert_eq!(gone, ["corrected"]);
}

#[test]
fn a_violation_gone_in_a_supposition_is_recorded_gone_once_adopted() {
    // Catches a reason recorded for a supposed world, which is not the
    // actual one; or none when adopting the world takes the violation
    // away, since what is shown does not change then.
    let mut r = two_dates("gone-supposed");
    r.command("suppose that w");
    let delete_first = |r: &mut Room| {
        r.put_cursor(r.outline().in_order()[0]);
        r.command("delete");
    };
    assert_eq!(why_gone(&mut r, delete_first), Vec::<String>::new());
    assert_ne!(last_observed(&r)["world"], "actual");
    assert_eq!(why_gone(&mut r, |r| r.command("adopt world")), ["deleted"]);
    assert_eq!(last_observed(&r)["world"], "actual");
}
