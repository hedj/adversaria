//! Milestone 3's acceptance checks (docs/plan.md, Milestone 3): the outline
//! and the quasimodes, driven by key events and drawn into a test terminal.

use memex::app::{App, Event, Key, Mode, Quasi};
use memex::journal::Journal;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod support;
use support::{Room, replayed};

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
    support::command(app, line);
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
    hold(&mut app, Quasi::LeapDown, "timeline");
    release(&mut app, Quasi::LeapDown);
    item(&mut app, "");
    tap(&mut app, Key::Tab);
    app.flush();
    command(&mut app, "transclude 2");
    command(&mut app, "undo"); // the empty item stays; the transclusion goes after it
    command(&mut app, "redo");
    // Back to the first fact, as in the scene.
    hold(&mut app, Quasi::LeapUp, "buran flew");
    release(&mut app, Quasi::LeapUp);
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
    let buran = "Buran flew once, unmanned, on 15 November 1988.";
    assert_eq!(cursor_text(&app), buran);
    let before = app.journal().entries().len();
    hold(&mut app, Quasi::LeapDown, "dra");
    leap_held_lists_matches(&app, before);
    release(&mut app, Quasi::LeapDown);
    leap_released_lands_on_drakon(&app, before);
    the_right_pane_shows_drakons_links(&app);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The text of the item at the cursor, as the outline holds it.
fn cursor_text(app: &App) -> String {
    app.notebook.outline.text(app.cursor())
}

/// Scene 1, Leap held on "dra": three matches, the transcluded copy among
/// them, the guide line beneath, and nothing written.
fn leap_held_lists_matches(app: &App, before: usize) {
    let s = screen(app, 140, 40);
    assert!(shows(&s, "LEAP · 3 MATCHES"), "{}", s.join("\n"));
    assert!(shows(&s, "2. DRAKON"));
    let copy = "6. Timeline (chart) › DRAKON";
    assert!(shows(&s, copy), "the transcluded copy matches too");
    assert!(shows(&s, "the drafts of chapter 2"));
    assert!(shows(&s, "LEAP DOWN  dra"));
    let guide = "release Leap to go to the first match · Tab completes · Esc cancels · ? lists every match · Leap again repeats";
    assert!(shows(&s, guide));
    let written = app.journal().entries().len();
    assert_eq!(written, before, "nothing is written while Leap is held");
}

/// Scene 1, Leap released: on DRAKON, in normal mode, with one entry
/// written, the Leap.
fn leap_released_lands_on_drakon(app: &App, before: usize) {
    assert_eq!(cursor_text(app), "DRAKON");
    assert_eq!(app.session.mode, Mode::Normal);
    let after = app.journal().entries();
    assert_eq!(after.len(), before + 1, "one entry, for the Leap");
    let leap = after.last().unwrap().body().unwrap();
    assert_eq!(
        (&leap["op"], &leap["pattern"]),
        (&json!("leap"), &json!("dra"))
    );
    assert_eq!(after.last().unwrap().kind(), "view");
}

/// Scene 1, on DRAKON: the right pane lists what it supports, what cites
/// it, and where it is transcluded.
fn the_right_pane_shows_drakons_links(app: &App) {
    let s = screen(app, 140, 40);
    let right: Vec<String> = s.iter().map(|l| l.chars().skip(84).collect()).collect();
    for want in [
        "LINKS OF 2",
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
}

#[test]
fn leap_recognition_listing_latching_and_repeat() {
    // Catches Tab not completing, ? not listing, a tap not latching, or Leap
    // again not repeating.
    let dir = temp("leap");
    let mut app = essay(&dir);
    hold(&mut app, Quasi::LeapDown, "sour");
    tap(&mut app, Key::Tab);
    match &app.session.mode {
        Mode::Quasi(a) => assert_eq!(
            a.input, "source",
            "recognition completes to the shared word"
        ),
        m => panic!("{m:?}"),
    }
    release(&mut app, Quasi::LeapDown);
    assert_eq!(app.notebook.outline.text(app.cursor()), "Sources");
    // A tap latches; typing then Enter goes.
    tap(&mut app, Key::Quasi(Quasi::LeapUp));
    assert!(matches!(&app.session.mode, Mode::Quasi(a) if a.latched()));
    typ(&mut app, "energia");
    tap(&mut app, Key::Enter);
    assert!(
        app.notebook
            .outline
            .text(app.cursor())
            .starts_with("Energia")
    );
    // Leap again repeats the last pattern: a tap to latch, a tap to go.
    tap(&mut app, Key::Quasi(Quasi::LeapDown));
    tap(&mut app, Key::Quasi(Quasi::LeapDown));
    assert!(
        app.notebook
            .outline
            .text(app.cursor())
            .starts_with("Energia"),
        "the only match is where we are"
    );
    // ? lists every match.
    hold(&mut app, Quasi::LeapDown, "o?");
    let s = screen(&app, 140, 60);
    let n = app.leap_matches("o", true).len();
    assert!(n > 5 && shows(&s, &format!("LEAP · {n} MATCHES")));
    assert!(
        s.iter()
            .filter(|l| l.contains(" › ") || l.trim_start().starts_with("│"))
            .count()
            >= n.min(50)
    );
    release(&mut app, Quasi::LeapDown);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn command_guide_words_recognition_and_help() {
    // Catches guide words not filled in, recognition on Tab not completing,
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
    tap(&mut app, Key::Tab);
    match &app.session.mode {
        Mode::Quasi(a) => assert_eq!(a.input, "link "),
        m => panic!("{m:?}"),
    }
    typ(&mut app, "?");
    let s = screen(&app, 140, 40);
    assert!(shows(&s, "link (from) … (to) … (as) …"), "{}", s.join("\n"));
    release(&mut app, Quasi::Command);
    assert!(
        !app.session.message.is_empty(),
        "an incomplete link says what it needs"
    );
    // An unknown command writes nothing.
    let n = app.journal().entries().len();
    support::refused(&mut app, "frobnicate");
    assert_eq!(app.journal().entries().len(), n);
    assert!(app.session.message.contains("no command"));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A room of two items, "first" then "second": the room, and the second.
fn two_items(name: &str) -> (Room, u64) {
    let mut r = Room::new(name);
    r.items(&["first", "second"]);
    let second = r.app.cursor();
    (r, second)
}

#[test]
fn undo_redo_and_branches() {
    // Catches undo that loses later state, or redo that cannot return.
    let (mut r, second) = two_items("undo");
    r.command("undo"); // the text of "second"
    assert_eq!(r.text(second), "");
    let said = &r.app.session.message;
    assert!(
        said.contains(": edited 2: “second”"),
        "says what it undid: {said}"
    );
    r.command("undo"); // the creation of "second"
    assert!(!r.outline().contains(second));
    r.command("redo");
    r.command("redo");
    assert_eq!(r.text(second), "second");
    r.refused("redo");
    assert!(r.app.session.message.contains("nothing to redo"));
}

/// The first edit after an entry.
fn edit_after(r: &Room, seq: u64) -> &format::record::Entry {
    let mut entries = r.app.journal().entries().iter();
    entries.find(|e| e.seq > seq && e.kind() == "edit").unwrap()
}

/// Whether the journal holds an entry that adds an item of a text.
fn added(r: &Room, text: &str) -> bool {
    let mut entries = r.app.journal().entries().iter();
    entries.any(|e| e.body().is_some_and(|b| b["add"] == text))
}

#[test]
fn an_edit_after_undo_starts_a_branch() {
    // Catches an edit after undo that does not start a branch, or that
    // loses the branch undone.
    let (mut r, _) = two_items("branch");
    r.command("undo");
    r.command("undo");
    let undo_seq = r.app.notebook.tip;
    r.items(&["", "another"]);
    assert!(
        r.app.notebook.redo.is_empty(),
        "an edit after undo clears redo"
    );
    let parent = edit_after(&r, undo_seq).parent_seq();
    assert_eq!(
        parent,
        Some(undo_seq),
        "the edit follows the undo: a new branch"
    );
    assert!(
        added(&r, "second"),
        "the undone branch is still in the journal"
    );
}

/// A room where a link and a revision depend on a claim, and an item does
/// not: the room, and the claim.
fn claim_with_dependents(name: &str) -> (Room, u64) {
    let mut r = Room::new(name);
    r.items(&["a claim", "independent work"]);
    let claim = r.app.notebook.outline.by_number("1").unwrap();
    r.command("link 1 2 as supports");
    r.leap(Quasi::LeapUp, "claim");
    r.typ(" (revised)");
    r.app.flush();
    (r, claim)
}

/// What a retraction waiting to be confirmed lists as depending on it.
fn dependents(r: &Room) -> usize {
    match &r.app.session.mode {
        Mode::ConfirmRetract { dependents, .. } => dependents.len(),
        m => panic!("{m:?}"),
    }
}

#[test]
fn a_retraction_lists_what_depends_on_it_and_waits() {
    // Plan: "retraction of any earlier entry, listing what depends on it
    // before confirming". Catches a retraction without the list, or
    // without confirmation.
    let (mut r, claim) = claim_with_dependents("retract-list");
    let n = r.entry_count();
    r.command(&format!("retract {claim}"));
    let deps = "the first text, the link and the revision depend on the claim";
    assert_eq!(dependents(&r), 3, "{deps}");
    assert!(r.shows("depend on it"), "{:?}", r.lines());
    assert_eq!(r.entry_count(), n, "nothing written before confirming");
    r.tap(&[Key::Escape]);
    assert!(r.outline().contains(claim), "Escape keeps it");
}

#[test]
fn retraction_lists_dependents_then_keeps_later_work() {
    // Catches a confirmed retraction that loses work that does not depend
    // on it, or that cannot be undone.
    let (mut r, claim) = claim_with_dependents("retract");
    let retract = format!("retract {claim}");
    r.command(&retract);
    r.tap(&[Key::Escape]);
    r.command(&retract);
    r.tap(&[Key::Enter]);
    assert!(!r.outline().contains(claim));
    let first = r.outline().by_number("1").unwrap();
    assert_eq!(r.text(first), "independent work");
    r.refused(&retract);
    assert!(r.app.session.message.contains("already retracted"));
    r.command("undo"); // undoing the retraction brings the claim back
    assert_eq!(r.text(claim), "a claim (revised)");
}

/// Whether the notebook's outline is the one replayed from the root to
/// its tip.
fn as_replayed(r: &Room) -> bool {
    let n = &r.app.notebook;
    n.outline == n.outline_at(n.tip)
}

#[test]
fn undo_redo_retraction_and_worlds_give_the_outline_replayed_from_the_root() {
    // Catches an outline started from one kept in a pause that is not
    // the outline a replay from the root gives: one kept off the tip's
    // path, the entries after it missed, or one kept past a retraction.
    let (mut r, claim) = claim_with_dependents("kept-outlines");
    r.app.idle();
    r.items(&["", "later", "last"]);
    let steps = ["undo", "undo", "redo", "undo", "undo", "undo", "redo"];
    for (n, line) in steps.into_iter().enumerate() {
        r.command(line);
        assert!(as_replayed(&r), "{n}: {line}");
        if n % 2 == 1 {
            r.app.idle();
        }
    }
    r.command(&format!("retract {claim}"));
    r.tap(&[Key::Enter]);
    assert!(as_replayed(&r), "the retraction");
    for line in ["suppose maybe", "close", "undo", "redo"] {
        r.command(line);
        assert!(as_replayed(&r), "{line}");
        r.app.idle();
    }
}

#[test]
fn replay_reproduces_the_screen() {
    // Plan: "Every edit, undo and retraction is a journal entry, and replay
    // reproduces the screen state." A session of every kind of action, then
    // a fresh replay of its journal: the two screens are the same cell for
    // cell, but for the message line (feedback, not state).
    let dir = temp("replay");
    let mut app = essay(&dir);
    every_kind_of_action(&mut app);
    replay_matches(&app, &replayed(&dir));
    assert_eq!(app.reason.world(), "the flight was in 1989");
    journal_holds_every_kind(&app);
    // And the journal checks as a chain.
    let text = std::fs::read_to_string(dir.join("journal.jsonl")).unwrap();
    assert!(format::chain::check_journal(&text).is_ok());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// On the essay: a name, typing, undo and redo, Leap, fold, each scope, a
/// deletion, a retraction confirmed, a supposition, a pane and a switch,
/// and moving the cursor.
fn every_kind_of_action(app: &mut App) {
    command(app, "name title");
    typ(app, " Buran");
    command(app, "undo");
    command(app, "redo");
    hold(app, Quasi::LeapDown, "open");
    release(app, Quasi::LeapDown);
    command(app, "fold");
    hold(app, Quasi::LeapDown, "energia");
    release(app, Quasi::LeapDown);
    command(app, "delete word");
    command(app, &format!("retract {}", last_of_kind(app, "link")));
    tap(app, Key::Enter);
    hold(app, Quasi::Suppose, "the flight was in 1989");
    release(app, Quasi::Suppose);
    command(app, "pane outline");
    command(app, "switch");
    tap(app, Key::Down);
    tap(app, Key::Right);
    app.flush();
}

/// The number of the last entry of a kind.
fn last_of_kind(app: &App, kind: &str) -> u64 {
    let mut entries = app.journal().entries().iter().rev();
    entries.find(|e| e.kind() == kind).unwrap().seq
}

/// A replay is the live memex: the same screen but for its message line,
/// and the same state.
fn replay_matches(app: &App, replayed: &App) {
    let (live, again) = (screen(app, 140, 40), screen(replayed, 140, 40));
    let n = live.len() - 1; // the last line holds messages
    assert_eq!(live[..n], again[..n]);
    same_state(replayed, app, "at the end");
}

/// A replay holds the live memex's state: its view and outline, its tip,
/// redo list and world.
fn same_state(replayed: &App, app: &App, at: &str) {
    assert_eq!(replayed.session.view, app.session.view, "view differs {at}");
    let outline = &app.notebook.outline;
    assert_eq!(&replayed.notebook.outline, outline, "outline differs {at}");
    assert_eq!(replayed.notebook.tip, app.notebook.tip, "tip differs {at}");
    assert_eq!(
        replayed.notebook.redo, app.notebook.redo,
        "redo differs {at}"
    );
    assert_eq!(
        replayed.reason.world(),
        app.reason.world(),
        "world differs {at}"
    );
}

/// Every kind of entry the session used is in the journal.
fn journal_holds_every_kind(app: &App) {
    let kinds: std::collections::BTreeSet<&str> =
        app.journal().entries().iter().map(|e| e.kind()).collect();
    for k in [
        "edit", "link", "name", "undo", "redo", "retract", "suppose", "view",
    ] {
        assert!(kinds.contains(k), "no {k} entry");
    }
}

#[test]
fn keystroke_to_visible_change_on_ten_thousand_items() {
    // Plan: "Keystroke to visible change within 0.1 s for an outline of
    // 10,000 items, measured and logged." Measured as the time to handle a
    // key and draw the whole screen into a 160 x 50 test terminal; writing
    // to a real terminal is not included.
    let dir = temp("timing");
    ten_thousand_items(&dir);
    let (mut app, opened) = open_ten_thousand(&dir);
    let mut timer = Timer::new();
    timer.keystrokes(&mut app);
    eprintln!("{}", timer.report(opened));
    assert!(cursor_text(&app).is_empty(), "Enter made a new item");
    let worst = timer.worst;
    assert!(
        worst < Duration::from_millis(100),
        "worst keystroke took {worst:?}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A journal of 10,000 items: 100 sections of 99 items each.
fn ten_thousand_items(dir: &std::path::Path) {
    let mut j = Journal::open(dir).unwrap();
    j.durable = false;
    j.append("edit", json!({"op": "init"})).unwrap();
    let mut last_top = None;
    for t in 0..100u64 {
        last_top = Some(section(&mut j, t, last_top));
    }
}

/// A section and its 99 items, after the last section: the section's
/// entry.
fn section(j: &mut Journal, t: u64, after: Option<u64>) -> u64 {
    let top = create(j, 0, after, format!("Section {t}"));
    items_under(j, top, (0..99u64).map(|c| item_text(t, c)));
    top
}

/// The text of item c of section t.
fn item_text(t: u64, c: u64) -> String {
    let n = t * 100 + c;
    format!("Item {t}.{c}: a statement about Buran, Energia or DRAKON number {n}")
}

/// Items of these texts under one, each after the last.
fn items_under(j: &mut Journal, top: u64, texts: impl Iterator<Item = String>) {
    let mut last = None;
    for text in texts {
        last = Some(create(j, top, last, text));
    }
}

/// An item of a text created under one item, after another: its entry.
fn create(j: &mut Journal, under: u64, after: Option<u64>, text: String) -> u64 {
    let spans = [[0, 0, text.chars().count()]];
    let body = json!({"op":"create","under":under,"after":after,"add":text,"spans":spans});
    j.append("edit", body).unwrap().seq
}

/// A memex opened on a directory of 10,000 items, and how long its replay
/// took.
fn open_ten_thousand(dir: &std::path::Path) -> (App, Duration) {
    let t0 = Instant::now();
    let mut app = App::open(dir).unwrap();
    let opened = t0.elapsed();
    app.set_durable(false);
    assert_eq!(app.notebook.outline.items.len(), 10_000);
    (app, opened)
}

/// Keystrokes timed: each handled, and the whole screen drawn into a 160 x
/// 50 test terminal.
struct Timer {
    term: Terminal<TestBackend>,
    worst: Duration,
    total: Duration,
    n: u32,
}

impl Timer {
    fn new() -> Timer {
        let term = Terminal::new(TestBackend::new(160, 50)).unwrap();
        let (worst, total, n) = (Duration::ZERO, Duration::ZERO, 0);
        Timer {
            term,
            worst,
            total,
            n,
        }
    }

    /// Handle a key and draw the screen, timed.
    fn time(&mut self, app: &mut App, ev: Event) {
        let t = Instant::now();
        app.handle(ev);
        self.term.draw(|f| memex::ui::draw(f, app)).unwrap();
        self.note(t.elapsed());
    }

    fn note(&mut self, d: Duration) {
        self.worst = self.worst.max(d);
        self.total += d;
        self.n += 1;
    }

    /// Twenty steps down, a word typed, a Leap to a number, and a new item.
    fn keystrokes(&mut self, app: &mut App) {
        for _ in 0..20 {
            self.time(app, Event::down(Key::Down));
        }
        self.typed(app, "typed");
        self.time(app, Event::down(Key::Quasi(Quasi::LeapDown)));
        self.typed(app, "number 9876");
        self.time(app, Event::up(Key::Quasi(Quasi::LeapDown)));
        self.time(app, Event::down(Key::Enter));
    }

    fn typed(&mut self, app: &mut App, text: &str) {
        for c in text.chars() {
            self.time(app, Event::down(Key::Char(c)));
        }
    }

    /// What was measured, to log.
    fn report(&self, opened: Duration) -> String {
        let mean = self.total / self.n;
        let (n, worst) = (self.n, self.worst);
        format!(
            "10,000 items: replayed in {opened:?}; {n} keystrokes, mean {mean:?}, worst {worst:?} (handle + draw, 160x50)"
        )
    }
}

/// One action on a memex.
type Step = Box<dyn Fn(&mut App)>;

/// A step of each kind: the editing steps, then the commands and
/// quasimodes.
fn every_step() -> Vec<Step> {
    editing_steps().into_iter().chain(command_steps()).collect()
}

/// Typing, the cursor's keys, a new item, indenting and erasing.
fn editing_steps() -> Vec<Step> {
    vec![
        Box::new(|a| typ(a, "Buran")),
        Box::new(|a| tap(a, Key::Left)),
        Box::new(|a| typ(a, "x")),
        Box::new(|a| item(a, "second")),
        Box::new(|a| tap(a, Key::Tab)),
        Box::new(|a| tap(a, Key::Home)),
        Box::new(|a| tap(a, Key::Backspace)),
        Box::new(|a| tap(a, Key::Up)),
    ]
}

/// Fold, undo, redo, fold again (showing it), Leap, a link, a second
/// outline pane, a switch to it and back, and a word deleted.
fn command_steps() -> Vec<Step> {
    vec![
        Box::new(|a| command(a, "fold")),
        Box::new(|a| command(a, "undo")),
        Box::new(|a| command(a, "redo")),
        Box::new(|a| command(a, "fold")),
        Box::new(|a| {
            hold(a, Quasi::LeapDown, "sec");
            release(a, Quasi::LeapDown)
        }),
        Box::new(|a| command(a, "link 1 1.1 as supports")),
        Box::new(|a| command(a, "pane outline")),
        Box::new(|a| command(a, "switch")),
        Box::new(|a| command(a, "switch")),
        Box::new(|a| command(a, "delete word")),
    ]
}

#[test]
fn replay_matches_after_every_action() {
    // The screen comparison above cannot see the terminal cursor, and a late
    // view entry can hide an early difference. So after each step: flush,
    // replay into a fresh app, and compare the whole state, view included.
    let dir = temp("every");
    let mut app = App::open(&dir).unwrap();
    app.set_durable(false);
    for (i, step) in every_step().iter().enumerate() {
        step(&mut app);
        replays_the_same(&dir, &mut app, &format!("after step {i}"));
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Flush, replay the journal into a fresh memex, and see the same state.
fn replays_the_same(dir: &std::path::Path, app: &mut App, at: &str) {
    app.flush();
    same_state(&replayed(dir), app, at);
}

#[test]
fn draws_at_any_size() {
    // Catches a panic when the screen is tiny (a pseudo-terminal started at
    // 0 x 0, found by the terminal's test; a window may be one cell).
    let dir = temp("sizes");
    let mut app = essay(&dir);
    hold(&mut app, Quasi::LeapDown, "dra");
    for (w, h) in [(0, 0), (1, 1), (10, 3), (20, 5), (200, 4), (3, 100)] {
        screen(&app, w, h);
    }
    release(&mut app, Quasi::LeapDown);
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
    let input = |app: &App| match &app.session.mode {
        Mode::Quasi(a) => Some(a.input.clone()),
        _ => None,
    };
    assert_eq!(input(&app).as_deref(), Some("he"));
    tap(&mut app, Key::Backspace);
    assert_eq!(input(&app).as_deref(), Some("h"), "one character erased");
    tap(&mut app, Key::Backspace);
    assert_eq!(input(&app).as_deref(), Some(""), "still latched, now empty");
    tap(&mut app, Key::Backspace);
    assert!(
        matches!(app.session.mode, Mode::Normal),
        "left the quasimode"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_question_mark_inside_quotes_is_text() {
    // In Command, ? lists what may come next; inside a quoted string it is
    // part of the string. Catches "Is it late?" losing its question mark.
    let mut r = Room::new("question");
    r.tap(&[Key::Quasi(Quasi::Command)]);
    r.typ("fact q(\"Is it late?\", 'why?') ?");
    let Mode::Quasi(a) = &r.app.session.mode else {
        panic!("{:?}", r.app.session.mode)
    };
    let typed = "fact q(\"Is it late?\", 'why?') ";
    assert_eq!(a.input, typed, "inside quotes, ? is text");
    assert!(a.listing, "outside them, ? still lists");
}

#[test]
fn a_command_on_an_item_needs_one() {
    // Catches a command that acts on the target run with no item at all.
    let mut r = Room::new("no-target");
    let before = r.app.journal().entries().len();
    r.refused("fold");
    assert_eq!(r.app.session.message, "there is no item to act on");
    assert_eq!(r.app.journal().entries().len(), before);
}

#[test]
fn a_write_that_fails_inside_a_command_with_nothing_to_say_is_reported() {
    // Catches a silent command that clears the message line after it ran,
    // and with it what a write inside the command reported: fold writes a
    // view entry, which fails here, since a directory is where the
    // journal's file was.
    let dir = temp("silent");
    let mut app = App::open(&dir).unwrap();
    typ(&mut app, "A");
    app.flush();
    std::fs::remove_file(dir.join("journal.jsonl")).unwrap();
    std::fs::create_dir(dir.join("journal.jsonl")).unwrap();
    command(&mut app, "fold");
    assert!(
        !app.session.message.is_empty(),
        "the failed write is reported"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn closing_a_world_shows_the_actual_outline_again() {
    // Catches a view entry that leaves a world without replaying the
    // outline of the world it returns to.
    let dir = temp("close");
    let mut app = App::open(&dir).unwrap();
    typ(&mut app, "actual");
    command(&mut app, "suppose maybe");
    item(&mut app, "supposed");
    command(&mut app, "close");
    assert_eq!(
        app.notebook.outline.in_order().len(),
        1,
        "only the actual item"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_purge_leaves_what_can_be_redone() {
    // Catches a purge, which Journal::purge writes, that forgets the
    // entries undone before it.
    let dir = temp("purge-redo");
    let mut app = App::open(&dir).unwrap();
    typ(&mut app, "A");
    command(&mut app, "undo");
    let redo = std::mem::take(&mut app.notebook.redo);
    drop(app);
    Journal::open(&dir).unwrap().purge(1, "privacy").unwrap();
    assert_eq!(
        (App::open(&dir).unwrap().notebook.redo, redo.len()),
        (redo, 1)
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn leaping_again_after_reopening_repeats_the_last_pattern() {
    // Catches a last leap kept only while the memex runs: the journal
    // holds it.
    let dir = temp("leap-again");
    let mut app = essay(&dir);
    hold(&mut app, Quasi::LeapUp, "energia");
    release(&mut app, Quasi::LeapUp);
    app.flush();
    drop(app);
    let mut app = App::open(&dir).unwrap();
    tap(&mut app, Key::Down);
    assert!(!app.text_of(app.cursor()).starts_with(&['E', 'n', 'e']));
    tap(&mut app, Key::Quasi(Quasi::LeapDown));
    tap(&mut app, Key::Quasi(Quasi::LeapDown));
    assert!(app.text_of(app.cursor()).starts_with(&['E', 'n', 'e']));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn focus_lost_with_a_quasimode_key_alone_held_leaves_it_doing_nothing() {
    // Catches Alt+Tab leaving Leap back held (its release goes to another
    // window), or taken for a tap that latches it, or writing anything.
    let dir = temp("focus-alone");
    let mut app = essay(&dir);
    let before = app.journal().entries().len();
    hold(&mut app, Quasi::LeapUp, "");
    app.focus_lost();
    assert_eq!(app.session.mode, Mode::Normal);
    assert_eq!(app.journal().entries().len(), before);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn focus_lost_after_typing_in_a_quasimode_keeps_it_open_until_enter() {
    // Catches what was typed lost, or the Leap made though its key was
    // never released here.
    let dir = temp("focus-typed");
    let mut app = essay(&dir);
    hold(&mut app, Quasi::LeapDown, "dra");
    app.focus_lost();
    assert!(matches!(&app.session.mode, Mode::Quasi(a) if a.latched() && a.input == "dra"));
    release(&mut app, Quasi::LeapDown);
    assert!(
        matches!(&app.session.mode, Mode::Quasi(_)),
        "a late release does not end it"
    );
    tap(&mut app, Key::Enter);
    assert_eq!(app.notebook.outline.text(app.cursor()), "DRAKON");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "timed in a release build, as the window runs: make test-timing"
)]
fn a_key_and_the_reasoning_it_causes_take_under_100ms_on_ten_thousand_items() {
    // Plan: "Keystroke to visible change within 0.1 s for an outline of
    // 10,000 items". Timed as the window does it: the key handled, the
    // world reasoned over again if the journal moved, and the screen drawn.
    // Each item states a fact, and a rule reads them all. The keys timed
    // each move the journal. A pause, which writes what is pending and
    // reads it, is timed a step at a time, as the window takes it, and a
    // key pressed during a step is timed with the step, as it waits for it.
    // A key may come during any step, so the slowest step, with what the
    // key took after its step, is held to the limit too.
    let dir = temp("timing-reasoned");
    ten_thousand_items(&dir);
    dated_items(&dir);
    let (mut app, opened) = open_ten_thousand(&dir);
    app.start_parser();
    let rule = "constraint one date: false :- date(E, A), date(E, B), A \\= B.";
    command(&mut app, rule);
    let mut timer = Reasoned::new(&mut app);
    timer.moving_keys(&mut app);
    eprintln!("{}", timer.report(opened));
    let worst = timer.worst;
    assert!(worst.1 < Duration::from_millis(100), "worst: {worst:?}");
    let any = timer.during_any_step();
    assert!(
        any < Duration::from_millis(100),
        "during the slowest step: {any:?}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// One fact for each item of the 10,000: the date of an event of its own,
/// one of a hundred years.
fn dated_items(dir: &std::path::Path) {
    let mut j = Journal::open(dir).unwrap();
    j.durable = false;
    let items: Vec<u64> = j
        .entries()
        .iter()
        .filter(|e| is_create(e))
        .map(|e| e.seq)
        .collect();
    for (n, item) in items.into_iter().enumerate() {
        let date = json!({"f": "date", "args": [{"a": format!("event_{n}")}, 1900 + n % 100]});
        let body = json!({"op": "facts", "item": item, "facts": [date]});
        j.append("edit", body).unwrap();
    }
}

/// Whether an entry makes an item.
fn is_create(e: &format::record::Entry) -> bool {
    e.kind() == "edit" && e.body().is_some_and(|b| b["op"] == "create")
}

/// Keys timed as the window handles them: handled, reasoned over if the
/// journal moved, and drawn into a 160 x 50 test terminal.
struct Reasoned {
    term: Terminal<TestBackend>,
    worst: (&'static str, Duration),
    times: Vec<(&'static str, Duration)>,
    /// The slowest step of a pause, alone: what a key waits for at most.
    slowest_step: Duration,
    /// What the key pressed during a step took after the step.
    after_step: Duration,
}

impl Reasoned {
    /// The timer, after the world is reasoned over once, as the window
    /// does when it opens.
    fn new(app: &mut App) -> Reasoned {
        let mut r = Reasoned {
            term: Terminal::new(TestBackend::new(160, 50)).unwrap(),
            worst: ("none", Duration::ZERO),
            times: vec![],
            slowest_step: Duration::ZERO,
            after_step: Duration::ZERO,
        };
        r.reason(app);
        r
    }

    /// A key pressed during the slowest step of a pause: that step and
    /// what the key took after its own, each measured, added.
    fn during_any_step(&self) -> Duration {
        self.slowest_step + self.after_step
    }

    /// A step of a pause, timed, and alone: the slowest so far noted.
    fn pause_step(&mut self, app: &mut App) {
        let mut step = Duration::ZERO;
        self.time(app, "pause step", |a| step = timed(|| a.pause_step()));
        self.slowest_step = self.slowest_step.max(step);
    }

    /// Reason again if the journal moved, as the window's draw does.
    fn reason(&mut self, app: &mut App) {
        app.reason_if_moved();
        assert!(app.reason.eval[0].is_some(), "{}", app.session.message);
    }

    /// An action, then reasoning and drawing, timed under a name; how
    /// long they took.
    fn time(&mut self, app: &mut App, name: &'static str, act: impl FnOnce(&mut App)) -> Duration {
        let t = Instant::now();
        act(app);
        self.reason(app);
        self.term.draw(|f| memex::ui::draw(f, app)).unwrap();
        self.note(name, t.elapsed());
        t.elapsed()
    }

    /// A time taken, under a name, and the worst so far.
    fn note(&mut self, name: &'static str, took: Duration) {
        self.times.push((name, took));
        if took > self.worst.1 {
            self.worst = (name, took);
        }
    }

    /// A key pressed, timed.
    fn key(&mut self, app: &mut App, name: &'static str, k: Key) {
        self.time(app, name, |a| a.handle(Event::down(k)));
    }

    /// A pause, timed a step at a time, as the window takes it: a key
    /// pressed during it waits for one step, and the reasoning that step
    /// causes.
    fn pause(&mut self, app: &mut App) {
        while app.idle_work() {
            self.pause_step(app);
        }
    }

    /// Keys that move the journal, each timed; the keys between them that
    /// do not (typing) are handled untimed.
    fn moving_keys(&mut self, app: &mut App) {
        self.writing(app);
        self.editing(app);
    }

    /// A new item typed into, and the pause after it, during whose second
    /// step, the first part of the lexicon, a key is pressed: the window
    /// handles the key once the step ends, and stops the pause.
    fn writing(&mut self, app: &mut App) {
        self.key(app, "Enter", Key::Enter);
        typ(app, "dated");
        self.key(app, "Down after typing", Key::Down);
        self.pause_step(app);
        let mut step = Duration::ZERO;
        let took = self.time(app, "Enter during a pause step", |a| {
            step = timed(|| a.pause_step());
            a.handle(Event::down(Key::Enter));
        });
        self.slowest_step = self.slowest_step.max(step);
        self.after_step = took - step;
        self.pause(app);
    }

    /// An item made and the empty item erased, the erasing undone, and the
    /// pause after them.
    fn editing(&mut self, app: &mut App) {
        self.key(app, "Enter", Key::Enter);
        self.key(app, "Backspace erasing", Key::Backspace);
        self.undo(app);
        self.pause(app);
    }

    /// Ctrl+Z as the window gets it, the Command quasimode held over z:
    /// timed from its release, which runs the undo.
    fn undo(&mut self, app: &mut App) {
        let command = Key::Quasi(Quasi::Command);
        app.handle(Event::down(command));
        typ(app, "z");
        self.time(app, "Ctrl+Z", |a| a.handle(Event::up(command)));
    }

    /// What was measured, to log.
    fn report(&self, opened: Duration) -> String {
        let each: Vec<String> = self
            .times
            .iter()
            .map(|(n, d)| format!("{n} {d:.1?}"))
            .collect();
        let (name, worst) = self.worst;
        let (step, after) = (self.slowest_step, self.after_step);
        format!(
            "10,000 dated items and a rule: replayed in {opened:?}; worst {worst:?} ({name}); slowest pause step {step:.1?}, and a key after its step {after:.1?}; each, with reasoning and drawing: {}",
            each.join(", ")
        )
    }
}

/// How long something took.
fn timed<T>(f: impl FnOnce() -> T) -> Duration {
    let t = Instant::now();
    f();
    t.elapsed()
}
