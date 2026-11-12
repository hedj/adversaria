//! Milestone 3's acceptance checks (docs/plan.md, Milestone 3): the outline
//! and the quasimodes, driven by key events and drawn into a test terminal.

use memex::app::{App, Event, Key, Mode, Quasi};
use memex::journal::Journal;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("adversaria-m3-{name}-{}", std::process::id()));
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

/// Hold a quasimode key, type, and keep holding (release separately).
fn hold(app: &mut App, q: Quasi, s: &str) {
    app.handle(Event::down(Key::Quasi(q)));
    typ(app, s);
}

fn release(app: &mut App, q: Quasi) {
    app.handle(Event::up(Key::Quasi(q)));
}

fn command(app: &mut App, line: &str) {
    hold(app, Quasi::Command, line);
    release(app, Quasi::Command);
}

/// Write a new item after the cursor's (Enter), with its text.
fn item(app: &mut App, text: &str) {
    tap(app, Key::Enter);
    typ(app, text);
}

fn screen(app: &App, w: u16, h: u16) -> Vec<String> {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| memex::ui::draw(f, app)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

fn shows(screen: &[String], text: &str) -> bool {
    screen.iter().any(|l| l.contains(text))
}

/// The essay of the user story, typed in.
fn essay(dir: &std::path::Path) -> App {
    let mut app = App::open(dir).unwrap();
    app.set_durable(false);
    typ(&mut app, "The programme");
    item(&mut app, "Buran flew once, unmanned, on 15 November 1988.");
    tap(&mut app, Key::Tab);
    item(&mut app, "Energia, its launcher, first flew in May 1987.");
    item(&mut app, "The programme was cancelled in 1993.");
    item(&mut app, "DRAKON");
    tap(&mut app, Key::BackTab);
    item(&mut app, "origins in the Buran programme");
    tap(&mut app, Key::Tab);
    item(&mut app, "How charts were reviewed");
    tap(&mut app, Key::BackTab);
    item(&mut app, "who reviewed the charts");
    tap(&mut app, Key::Tab);
    item(&mut app, "Open questions");
    tap(&mut app, Key::BackTab);
    item(&mut app, "the drafts of chapter 2");
    tap(&mut app, Key::Tab);
    item(&mut app, "Sources");
    tap(&mut app, Key::BackTab);
    item(&mut app, "source 4");
    tap(&mut app, Key::Tab);
    item(&mut app, "Timeline (chart)");
    tap(&mut app, Key::BackTab);
    app.flush();
    // Links: DRAKON supports 3; 3 cites DRAKON; the timeline transcludes DRAKON.
    command(&mut app, "link 2 3 as supports");
    command(&mut app, "link 3 2 as cites");
    command(&mut app, "link 1.1 source 4 as cites");
    hold(&mut app, Quasi::LeapForward, "timeline");
    release(&mut app, Quasi::LeapForward);
    item(&mut app, "");
    tap(&mut app, Key::Tab);
    app.flush();
    command(&mut app, "transclude 2");
    command(&mut app, "undo"); // the empty item stays; the transclusion goes after it
    command(&mut app, "redo");
    // Back to the first fact, as in the scene.
    hold(&mut app, Quasi::LeapBack, "buran flew");
    release(&mut app, Quasi::LeapBack);
    app.flush();
    app
}

#[test]
fn scene_one_as_written() {
    // Plan: "The user story's scene 1 works as written." You hold Leap and
    // type "dra"; three matches appear with the guide line beneath. You
    // release Leap and land on the DRAKON section; nothing was opened, saved
    // or chosen from a menu. The right pane shows the selected item's links:
    // what it supports, what cites it, and where it is transcluded.
    let dir = temp("scene1");
    let mut app = essay(&dir);
    assert_eq!(
        app.outline.text(app.cursor()),
        "Buran flew once, unmanned, on 15 November 1988."
    );
    let before = app.journal().entries().len();

    hold(&mut app, Quasi::LeapForward, "dra");
    let s = screen(&app, 140, 40);
    assert!(shows(&s, "LEAP · 3 MATCHES"), "{}", s.join("\n"));
    assert!(shows(&s, "2. DRAKON"));
    assert!(
        shows(&s, "6. Timeline (chart) › DRAKON"),
        "the transcluded copy matches too"
    );
    assert!(shows(&s, "the drafts of chapter 2"));
    assert!(shows(&s, "LEAP  dra"));
    assert!(shows(
        &s,
        "release Leap to go to the first match · Esc completes · ? lists every match · Leap again repeats"
    ));
    assert_eq!(
        app.journal().entries().len(),
        before,
        "nothing is written while Leap is held"
    );

    release(&mut app, Quasi::LeapForward);
    assert_eq!(app.outline.text(app.cursor()), "DRAKON");
    assert_eq!(app.mode, Mode::Normal);
    let after = app.journal().entries();
    assert_eq!(after.len(), before + 1, "one entry, for the Leap");
    let leap = after.last().unwrap();
    assert_eq!(
        (leap.kind.as_str(), &leap.body.as_ref().unwrap()["op"]),
        ("view", &json!("leap"))
    );
    assert_eq!(leap.body.as_ref().unwrap()["pattern"], json!("dra"));

    let s = screen(&app, 140, 40);
    let right: Vec<String> = s
        .iter()
        .map(|l| l.chars().skip(84).collect::<String>())
        .collect();
    for want in [
        "LINKS OF THE SELECTED ITEM",
        "supports →",
        "3. How charts were reviewed",
        "cited by ←",
        "transcluded in ←",
        "Timeline (chart)",
    ] {
        assert!(
            shows(&right, want),
            "right pane lacks {want:?}:\n{}",
            right.join("\n")
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn leap_recognition_listing_latching_and_repeat() {
    // Catches Esc not completing, ? not listing, a tap not latching, or Leap
    // again not repeating.
    let dir = temp("leap");
    let mut app = essay(&dir);
    hold(&mut app, Quasi::LeapForward, "sour");
    tap(&mut app, Key::Escape);
    match &app.mode {
        Mode::Quasi(a) => assert_eq!(
            a.input, "source",
            "recognition completes to the shared word"
        ),
        m => panic!("{m:?}"),
    }
    release(&mut app, Quasi::LeapForward);
    assert_eq!(app.outline.text(app.cursor()), "Sources");
    // A tap latches; typing then Enter goes.
    tap(&mut app, Key::Quasi(Quasi::LeapBack));
    assert!(matches!(&app.mode, Mode::Quasi(a) if a.latched));
    typ(&mut app, "energia");
    tap(&mut app, Key::Enter);
    assert!(app.outline.text(app.cursor()).starts_with("Energia"));
    // Leap again repeats the last pattern: a tap to latch, a tap to go.
    tap(&mut app, Key::Quasi(Quasi::LeapForward));
    tap(&mut app, Key::Quasi(Quasi::LeapForward));
    assert!(
        app.outline.text(app.cursor()).starts_with("Energia"),
        "the only match is where we are"
    );
    // ? lists every match.
    hold(&mut app, Quasi::LeapForward, "o?");
    let s = screen(&app, 140, 60);
    let n = app.leap_matches("o", true).len();
    assert!(n > 5 && shows(&s, &format!("LEAP · {n} MATCHES")));
    assert!(
        s.iter()
            .filter(|l| l.contains(" › ") || l.trim_start().starts_with("│"))
            .count()
            >= n.min(50)
    );
    release(&mut app, Quasi::LeapForward);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn command_guide_words_recognition_and_help() {
    // Catches guide words not filled in, recognition on Escape not completing,
    // or ? not listing what may come next.
    let dir = temp("command");
    let mut app = essay(&dir);
    assert_eq!(app.guided("link 1.1 source 4"), "link 1.1 (to) source 4");
    assert_eq!(
        app.guided("link 1.1 source 4 as supports"),
        "link 1.1 (to) source 4 (as) supports"
    );
    assert_eq!(app.guided("move 3"), "move (after) 3");
    hold(&mut app, Quasi::Command, "li");
    tap(&mut app, Key::Escape);
    match &app.mode {
        Mode::Quasi(a) => assert_eq!(a.input, "link "),
        m => panic!("{m:?}"),
    }
    typ(&mut app, "?");
    let s = screen(&app, 140, 40);
    assert!(shows(&s, "link (from) … (to) … (as) …"), "{}", s.join("\n"));
    release(&mut app, Quasi::Command);
    assert!(
        !app.message.is_empty(),
        "an incomplete link says what it needs"
    );
    // An unknown command writes nothing.
    let n = app.journal().entries().len();
    command(&mut app, "frobnicate");
    assert_eq!(app.journal().entries().len(), n);
    assert!(app.message.contains("no command"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn undo_redo_and_branches() {
    // Catches undo that loses later state, redo that cannot return, or an
    // edit after undo that does not start a branch.
    let dir = temp("undo");
    let mut app = App::open(&dir).unwrap();
    app.set_durable(false);
    typ(&mut app, "first");
    item(&mut app, "second");
    app.flush();
    let second = app.cursor();
    command(&mut app, "undo"); // the text of "second"
    assert_eq!(app.outline.text(second), "");
    command(&mut app, "undo"); // the creation of "second"
    assert!(!app.outline.items.contains_key(&second));
    command(&mut app, "redo");
    command(&mut app, "redo");
    assert_eq!(app.outline.text(second), "second");
    command(&mut app, "redo");
    assert!(app.message.contains("nothing to redo"));
    command(&mut app, "undo");
    command(&mut app, "undo");
    let undo_seq = app.tip;
    item(&mut app, "another");
    app.flush();
    assert!(app.redo.is_empty(), "an edit after undo clears redo");
    let e = app
        .journal()
        .entries()
        .iter()
        .find(|e| e.seq > undo_seq && e.kind == "edit")
        .unwrap();
    assert_eq!(
        e.parent,
        Some(undo_seq),
        "the edit follows the undo: a new branch"
    );
    // The undone branch is still in the journal.
    assert!(
        app.journal()
            .entries()
            .iter()
            .any(|e| e.body.as_ref().is_some_and(|b| b["add"] == "second"))
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn retraction_lists_dependents_then_keeps_later_work() {
    // Plan: "retraction of any earlier entry, listing what depends on it
    // before confirming". Catches a retraction without the list, without
    // confirmation, or that loses work that does not depend on it.
    let dir = temp("retract");
    let mut app = App::open(&dir).unwrap();
    app.set_durable(false);
    typ(&mut app, "a claim");
    item(&mut app, "independent work");
    app.flush();
    let claim = app.outline.by_number("1").unwrap();
    command(&mut app, "link 1 2 as supports");
    hold(&mut app, Quasi::LeapBack, "claim");
    release(&mut app, Quasi::LeapBack);
    typ(&mut app, " (revised)");
    app.flush();
    let n = app.journal().entries().len();
    command(&mut app, &format!("retract {claim}"));
    let deps = match &app.mode {
        Mode::ConfirmRetract { dependents, .. } => dependents.clone(),
        m => panic!("{m:?}"),
    };
    assert_eq!(
        deps.len(),
        3,
        "the first text, the link and the revision depend on the claim: {deps:?}"
    );
    let s = screen(&app, 140, 30);
    assert!(shows(&s, "depend on it"), "{}", s.join("\n"));
    assert_eq!(
        app.journal().entries().len(),
        n,
        "nothing written before confirming"
    );
    tap(&mut app, Key::Escape);
    assert!(app.outline.items.contains_key(&claim), "Escape keeps it");
    command(&mut app, &format!("retract {claim}"));
    tap(&mut app, Key::Enter);
    assert!(!app.outline.items.contains_key(&claim));
    assert_eq!(
        app.outline.text(app.outline.by_number("1").unwrap()),
        "independent work"
    );
    command(&mut app, &format!("retract {claim}"));
    assert!(app.message.contains("already retracted"));
    command(&mut app, "undo"); // undoing the retraction brings the claim back
    assert_eq!(app.outline.text(claim), "a claim (revised)");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn replay_reproduces_the_screen() {
    // Plan: "Every edit, undo and retraction is a journal entry, and replay
    // reproduces the screen state." A session of every kind of action, then
    // a fresh replay of its journal: the two screens are the same cell for
    // cell, but for the message line (feedback, not state).
    let dir = temp("replay");
    let mut app = essay(&dir);
    command(&mut app, "name title");
    typ(&mut app, " Buran");
    command(&mut app, "undo");
    command(&mut app, "redo");
    hold(&mut app, Quasi::LeapForward, "open");
    release(&mut app, Quasi::LeapForward);
    command(&mut app, "fold");
    hold(&mut app, Quasi::Scope, " ");
    release(&mut app, Quasi::Scope);
    hold(&mut app, Quasi::LeapForward, "energia");
    release(&mut app, Quasi::LeapForward);
    app.handle(Event::down(Key::Quasi(Quasi::Scope)));
    release(&mut app, Quasi::Scope);
    tap(&mut app, Key::Quasi(Quasi::Scope)); // latched, then a second tap ends it at word scope
    command(&mut app, "delete");
    let to_retract = app
        .journal()
        .entries()
        .iter()
        .rev()
        .find(|e| e.kind == "link")
        .unwrap()
        .seq;
    command(&mut app, &format!("retract {to_retract}"));
    tap(&mut app, Key::Enter);
    hold(&mut app, Quasi::Suppose, "the flight was in 1989");
    release(&mut app, Quasi::Suppose);
    command(&mut app, "pane outline");
    command(&mut app, "switch");
    tap(&mut app, Key::Down);
    tap(&mut app, Key::Right);
    app.flush();
    let live = screen(&app, 140, 40);
    let replayed = App::open(&dir).unwrap();
    let again = screen(&replayed, 140, 40);
    let n = live.len() - 1; // the last line holds messages
    assert_eq!(live[..n], again[..n]);
    assert_eq!(
        (replayed.tip, &replayed.redo, &replayed.world),
        (app.tip, &app.redo, &app.world)
    );
    assert_eq!(replayed.view, app.view);
    assert_eq!(replayed.outline, app.outline);
    assert_eq!(app.world, "the flight was in 1989");
    // Every kind the session used is in the journal.
    let kinds: std::collections::BTreeSet<&str> = app
        .journal()
        .entries()
        .iter()
        .map(|e| e.kind.as_str())
        .collect();
    for k in [
        "edit", "link", "name", "undo", "redo", "retract", "suppose", "view",
    ] {
        assert!(kinds.contains(k), "no {k} entry");
    }
    // And the journal checks as a chain.
    let text = std::fs::read_to_string(dir.join("journal.jsonl")).unwrap();
    assert!(format::chain::check_journal(&text).is_ok());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn keystroke_to_visible_change_on_ten_thousand_items() {
    // Plan: "Keystroke to visible change within 0.1 s for an outline of
    // 10,000 items, measured and logged." Measured as the time to handle a
    // key and draw the whole screen into a 160 x 50 test terminal; writing
    // to a real terminal is not included.
    let dir = temp("timing");
    {
        let mut j = Journal::open(&dir).unwrap();
        j.durable = false;
        j.append("edit", json!({"op": "init"})).unwrap();
        let mut last_top = None;
        for t in 0..100u64 {
            let text = format!("Section {t}");
            let e = j.append("edit", json!({"op":"create","under":0,"after":last_top,"add":text,"spans":[[0,0,text.chars().count()]]})).unwrap();
            last_top = Some(e.seq);
            let mut last = None;
            for c in 0..99u64 {
                let text = format!(
                    "Item {t}.{c}: a statement about Buran, Energia or DRAKON number {}",
                    t * 100 + c
                );
                let e = j.append("edit", json!({"op":"create","under":e.seq,"after":last,"add":text,"spans":[[0,0,text.chars().count()]]})).unwrap();
                last = Some(e.seq);
            }
        }
    }
    let t0 = Instant::now();
    let mut app = App::open(&dir).unwrap();
    let opened = t0.elapsed();
    app.set_durable(false);
    assert_eq!(app.outline.items.len(), 10_000);
    let mut term = Terminal::new(TestBackend::new(160, 50)).unwrap();
    let mut worst = Duration::ZERO;
    let mut total = Duration::ZERO;
    let mut n = 0;
    let mut time = |app: &mut App, ev: Event| {
        let t = Instant::now();
        app.handle(ev);
        term.draw(|f| memex::ui::draw(f, app)).unwrap();
        let d = t.elapsed();
        worst = worst.max(d);
        total += d;
        n += 1;
    };
    for _ in 0..20 {
        time(&mut app, Event::down(Key::Down));
    }
    for c in "typed".chars() {
        time(&mut app, Event::down(Key::Char(c)));
    }
    time(&mut app, Event::down(Key::Quasi(Quasi::LeapForward)));
    for c in "number 9876".chars() {
        time(&mut app, Event::down(Key::Char(c)));
    }
    time(&mut app, Event::up(Key::Quasi(Quasi::LeapForward)));
    time(&mut app, Event::down(Key::Enter));
    let mean = total / n;
    eprintln!(
        "10,000 items: replayed in {opened:?}; {n} keystrokes, mean {mean:?}, worst {worst:?} (handle + draw, 160x50)"
    );
    assert!(
        app.outline.text(app.cursor()).is_empty(),
        "Enter made a new item"
    );
    assert!(
        worst < Duration::from_millis(100),
        "worst keystroke took {worst:?}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn replay_matches_after_every_action() {
    // The screen comparison above cannot see the terminal cursor, and a late
    // view entry can hide an early difference. So after each step: flush,
    // replay into a fresh app, and compare the whole state, view included.
    let dir = temp("every");
    let mut app = App::open(&dir).unwrap();
    app.set_durable(false);
    type Step = Box<dyn Fn(&mut App)>;
    let steps: Vec<Step> = vec![
        Box::new(|a| typ(a, "Buran")),
        Box::new(|a| tap(a, Key::Left)),
        Box::new(|a| typ(a, "x")),
        Box::new(|a| item(a, "second")),
        Box::new(|a| tap(a, Key::Tab)),
        Box::new(|a| tap(a, Key::Home)),
        Box::new(|a| tap(a, Key::Backspace)),
        Box::new(|a| tap(a, Key::Up)),
        Box::new(|a| command(a, "fold")),
        Box::new(|a| command(a, "undo")),
        Box::new(|a| command(a, "redo")),
        Box::new(|a| command(a, "unfold")),
        Box::new(|a| {
            hold(a, Quasi::LeapForward, "sec");
            release(a, Quasi::LeapForward)
        }),
        Box::new(|a| command(a, "link 1 1.1 as supports")),
        Box::new(|a| command(a, "switch")),
        Box::new(|a| command(a, "switch")),
        Box::new(|a| {
            hold(a, Quasi::Scope, " ");
            release(a, Quasi::Scope)
        }),
    ];
    for (i, step) in steps.iter().enumerate() {
        step(&mut app);
        app.flush();
        let r = App::open(&dir).unwrap();
        assert_eq!(r.view, app.view, "view differs after step {i}");
        assert_eq!(r.outline, app.outline, "outline differs after step {i}");
        assert_eq!(
            (r.tip, &r.redo, &r.world),
            (app.tip, &app.redo, &app.world),
            "after step {i}"
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn draws_at_any_size() {
    // Catches a panic when the terminal is tiny (a pseudo-terminal starts at
    // 0 x 0; found by tests/memex_in_a_pty.py).
    let dir = temp("sizes");
    let mut app = essay(&dir);
    hold(&mut app, Quasi::LeapForward, "dra");
    for (w, h) in [(0, 0), (1, 1), (10, 3), (20, 5), (200, 4), (3, 100)] {
        screen(&app, w, h);
    }
    release(&mut app, Quasi::LeapForward);
    for (w, h) in [(0, 0), (1, 1), (10, 3), (20, 5), (200, 4), (3, 100)] {
        screen(&app, w, h);
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn backspace_erases_then_leaves_a_latched_quasimode() {
    // Catches Backspace that leaves the quasimode while text remains, or
    // that never leaves it once the text is gone.
    let dir = temp("backspace");
    let mut app = App::open(&dir).unwrap();
    app.set_durable(false);
    // Tap Command to latch it, type two letters.
    tap(&mut app, Key::Quasi(Quasi::Command));
    typ(&mut app, "he");
    let input = |app: &App| match &app.mode {
        Mode::Quasi(a) => Some(a.input.clone()),
        _ => None,
    };
    assert_eq!(input(&app).as_deref(), Some("he"));
    tap(&mut app, Key::Backspace);
    assert_eq!(input(&app).as_deref(), Some("h"), "one character erased");
    tap(&mut app, Key::Backspace);
    assert_eq!(input(&app).as_deref(), Some(""), "still latched, now empty");
    tap(&mut app, Key::Backspace);
    assert!(matches!(app.mode, Mode::Normal), "left the quasimode");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_question_mark_inside_quotes_is_text() {
    // In Command, ? lists what may come next; inside a quoted string it is
    // part of the string. Catches "Is it late?" losing its question mark.
    let dir = temp("question");
    let mut app = App::open(&dir).unwrap();
    app.set_durable(false);
    tap(&mut app, Key::Quasi(Quasi::Command));
    typ(&mut app, "fact q(\"Is it late?\", 'why?') ?");
    let (input, listing) = match &app.mode {
        Mode::Quasi(a) => (a.input.clone(), a.listing),
        m => panic!("{m:?}"),
    };
    assert_eq!(
        input, "fact q(\"Is it late?\", 'why?') ",
        "inside quotes, ? is text"
    );
    assert!(listing, "outside them, ? still lists");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_command_on_an_item_needs_one() {
    // Catches a command that acts on the target run with no item at all.
    let dir = temp("no-target");
    let mut app = App::open(&dir).unwrap();
    let before = app.journal().entries().len();
    command(&mut app, "fold");
    assert_eq!(
        (app.message.as_str(), app.journal().entries().len()),
        ("there is no item to act on", before)
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
