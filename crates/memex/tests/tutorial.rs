//! docs/tutorial.md, performed: each step's keys, and each thing the
//! tutorial says the screen will show. The test also checks that every
//! command it types and every text it expects appears in the tutorial word
//! for word, so the two cannot drift apart unnoticed.

use memex::app::{App, Event, Key, Mode, Quasi, Special};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use std::path::PathBuf;

mod support;
use support::replayed;

const TUTORIAL: &str = include_str!("../../../docs/tutorial.md");

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("adversaria-tut-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

struct T {
    app: App,
    dump: bool,
}

impl T {
    fn tap(&mut self, k: Key) {
        self.app.handle(Event::down(k));
        self.app.handle(Event::up(k));
    }
    /// Type text; the tutorial must show it.
    fn typ(&mut self, s: &str) {
        in_tutorial(s);
        for c in s.chars() {
            self.tap(Key::Char(c));
        }
    }
    /// Hold a quasimode, type, release.
    fn hold(&mut self, q: Quasi, s: &str) {
        in_tutorial(s);
        self.app.handle(Event::down(Key::Quasi(q)));
        for c in s.chars() {
            self.tap(Key::Char(c));
        }
        self.app.handle(Event::up(Key::Quasi(q)));
        self.app.evaluate().unwrap(); // as the terminal loop does
    }
    /// Tap a quasimode's key, type, and press Enter, as the tutorial
    /// teaches.
    fn tapped(&mut self, q: Quasi, s: &str) {
        in_tutorial(s);
        self.tap(Key::Quasi(q));
        for c in s.chars() {
            self.tap(Key::Char(c));
        }
        self.tap(Key::Enter);
        self.app.evaluate().unwrap(); // as the terminal loop does
    }
    fn command(&mut self, line: &str) {
        self.tapped(Quasi::Command, line);
        assert!(
            !self.app.session.message.starts_with("no command")
                && !self.app.session.message.starts_with("not a"),
            "{line}: {}",
            self.app.session.message
        );
    }
    /// Pause, as the user does: the memex writes what is pending and reads
    /// what waits to be read, as the window does after a second.
    fn pause(&mut self) {
        self.app.idle();
        self.app.evaluate().unwrap();
    }
    /// Leap to an item, put its text's end in place of its last `n`
    /// characters, and pause.
    fn retype_end(&mut self, leap: &str, n: usize, end: &str) {
        self.hold(Quasi::LeapDown, leap);
        self.tap(Key::End);
        for _ in 0..n {
            self.tap(Key::Backspace);
        }
        self.typ(end);
        self.pause();
    }
    fn key(&mut self, k: Key) {
        self.tap(k);
        self.app.evaluate().unwrap();
    }
    /// The screen drawn, 200 by 45.
    fn buffer(&self) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(200, 45)).unwrap();
        t.draw(|f| memex::ui::draw(f, &self.app)).unwrap();
        t.backend().buffer().clone()
    }
    fn screen(&self) -> Vec<String> {
        let buf = self.buffer();
        let row = |y| (0..200).map(|x| buf[(x, y)].symbol()).collect::<String>();
        (0..45).map(|y| row(y).trim_end().to_string()).collect()
    }
    /// The screen shows this, and the tutorial says so.
    fn shows(&self, text: &str) {
        in_tutorial(text);
        let s = self.screen();
        assert!(
            s.iter().any(|l| l.contains(text)),
            "screen lacks {text:?}:\n{}",
            s.join("\n")
        );
    }
    /// The outline shows this text in the colour of text the memex has
    /// read, and the tutorial says so.
    fn read(&self, text: &str) {
        in_tutorial(text);
        let (x, y) = self.in_outline(text);
        let fg = self.buffer()[(x, y)].style().fg;
        assert_eq!(fg, Some(Color::Cyan), "{text:?} is not drawn as read");
    }
    /// Where the outline, the left half of the screen, first shows a
    /// text: its column and row. The right pane may show it too.
    fn in_outline(&self, text: &str) -> (u16, u16) {
        let at = place_in_left(&self.screen(), text);
        at.unwrap_or_else(|| panic!("the outline lacks {text:?}"))
    }
    fn lacks(&self, text: &str) {
        in_tutorial(text);
        let s = self.screen();
        assert!(
            !s.iter().any(|l| l.contains(text)),
            "screen has {text:?}:\n{}",
            s.join("\n")
        );
    }
    fn says(&self, text: &str) {
        in_tutorial(text);
        assert!(
            self.app.session.message.contains(text),
            "message {:?} lacks {text:?}",
            self.app.session.message
        );
    }
    /// The text of the item at the cursor.
    fn here(&self) -> String {
        self.app.notebook.outline.text(self.app.cursor())
    }
    /// The mouse pointer over where a text is first drawn.
    fn point_at(&mut self, text: &str) {
        self.app.session.pointer = Some(self.place_of(text));
    }
    /// A right-click where a text is first drawn, on the word there.
    fn right_click_on(&mut self, text: &str) {
        let (at, hits) = (self.place_of(text), self.presented());
        self.app.right_click(&hits, at, text, false);
    }
    /// What the screen presents, and where.
    fn presented(&self) -> memex::ui::Presentations {
        let mut term = Terminal::new(TestBackend::new(200, 45)).unwrap();
        let mut hits = memex::ui::Presentations::default();
        term.draw(|f| hits = memex::ui::draw_hits(f, &self.app))
            .unwrap();
        hits
    }
    /// Where a text is first drawn on the screen: its column and row.
    fn place_of(&self, text: &str) -> (u16, u16) {
        let lines = self.screen();
        let y = lines.iter().position(|l| l.contains(text)).expect(text);
        let before = lines[y].split(text).next().unwrap_or_default();
        (before.chars().count() as u16, y as u16)
    }
    fn look(&self, at: &str) {
        if self.dump {
            println!(
                "---- {at} (message: {:?})\n{}",
                self.app.session.message,
                self.screen().join("\n")
            );
        }
    }
}

/// Where a text first begins in the left half of the screen's lines: its
/// column and row.
fn place_in_left(lines: &[String], text: &str) -> Option<(u16, u16)> {
    for (y, line) in lines.iter().enumerate() {
        if let Some(x) = left_column(line, text) {
            return Some((x, y as u16));
        }
    }
    None
}

/// The column at which a text begins in a screen line, if it begins in
/// the left half, where the outline is.
fn left_column(line: &str, text: &str) -> Option<u16> {
    let column = line[..line.find(text)?].chars().count();
    (column < 100).then_some(column as u16)
}

fn in_tutorial(s: &str) {
    if std::env::var_os("TUTORIAL_DUMP").is_some() {
        return;
    }
    assert!(TUTORIAL.contains(s), "docs/tutorial.md does not say {s:?}");
}

#[test]
fn the_tutorial_works_as_written() {
    // The tutorial's paths are from the repository, where `make run` starts.
    std::env::set_current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).unwrap();
    let dir = temp("all");
    let mut app = App::open(&dir).unwrap();
    app.set_durable(false);
    let mut t = T {
        app,
        dump: std::env::var_os("TUTORIAL_DUMP").is_some(),
    };

    t.look("start");
    t.shows("Type to write the first item.");

    part_a(&mut t);
    part_b(&mut t);
    part_c(&mut t);
    part_d(&mut t);

    // Step 17: leave. The journal, opened again, replays to the same budget.
    t.key(Key::F(10));
    assert!(t.app.session.quit);
    t.app.flush();
    let mut again = replayed(&dir);
    again.set_durable(false);
    again.evaluate().unwrap();
    std::mem::swap(&mut t.app, &mut again);
    t.shows("→ 300");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Steps 2 to 5: a fact beside prose, a claim and the rule that catches
/// it, the claim questioned, adopted and taken back, and supposed.
fn part_a(t: &mut T) {
    a_fact_and_prose(t);
    a_claim_and_a_rule(t);
    the_claim_questioned(t);
    the_claim_supposed(t);
}

/// Step 2: a fact the memex reads, beside prose it keeps as written.
fn a_fact_and_prose(t: &mut T) {
    t.typ("The programme");
    t.tap(Key::Enter);
    t.tap(Key::Tab);
    t.typ("The date of Buran-flight is 1988. The label of Buran-flight is \"Buran's flight\".");
    t.tap(Key::Enter);
    t.typ("Buran flew once, unmanned, on 15 November 1988.");
    t.pause();
    t.look("fact and prose");
    t.shows("▼ 1. The programme");
    t.read("The date of Buran-flight is 1988.");
    t.shows("prose: not ACE, it stops at Buran <> flew once, unmanned, on 15 November 1988.");
    t.tap(Key::Up);
    t.shows("READ AS");
    t.shows("fact  date(buran_flight, 1988)");
    t.shows("fact  label(buran_flight, \"Buran's flight\")");
    t.tap(Key::Down);
}

/// Step 3: a claim from a source, and the rule that catches it.
fn a_claim_and_a_rule(t: &mut T) {
    t.tap(Key::Enter);
    t.typ("Talk-notes says that the date of Buran-flight is 1989.");
    t.tap(Key::Enter);
    t.typ("Nothing has more than 1 date.");
    t.pause();
    t.says("constraint Nothing has more than 1 date kept: it fires on a planted violation");
    t.shows("1 disagreement between sources");
    t.shows("? sources disagree under constraint 1.4: 1988 here; 1989 in 1.3");
    t.lacks("constraint violation");
}

/// Step 4: the claim questioned, adopted, and the adoption taken back.
fn the_claim_questioned(t: &mut T) {
    t.point_at("Talk-notes says");
    t.shows("click: go to 1.3 · right-click for: adopt its claims; adopt them instead of what they contradict; the verbs that apply to it");
    t.app.session.pointer = None;
    t.tap(Key::Up);
    assert!(t.here().starts_with("Talk-notes"));
    t.pause();
    t.shows("CLAIMS OF 1.3 · WHO SAYS THEM");
    t.shows("who claims the contrary");
    t.shows("adopting it would raise 1 violation of Nothing has more than 1 date");
    t.key(Key::F(3));
    t.shows("1 constraint violation");
    t.shows("‼ breaks constraint 1.4: 1988 here; 1989 in 1.3");
    t.key(Key::F(4));
    t.shows("Enter retracts · Esc keeps it");
    t.key(Key::Enter);
    t.says("retracted");
    t.lacks("constraint violation");
}

/// Step 5: the claim supposed true, in a world beside the actual one.
fn the_claim_supposed(t: &mut T) {
    // Held: the foot names Suppose and says what to do, then, as the
    // sentence is typed, that it is not controlled English, which the
    // tutorial says to let be.
    t.tap(Key::Quasi(Quasi::Suppose));
    t.shows("COMMAND (latched)  suppose (that)");
    t.shows(
        "type a statement · ? lists what may come next · Esc cancels · Enter, or tap again, offers its readings",
    );
    t.typ("the flight was in 1989");
    t.shows("not ACE: the flight <> was in 1989.");
    t.shows("Enter, or tap again, offers its readings");
    t.key(Key::Enter);
    assert!(matches!(t.app.session.mode, Mode::ConfirmSuppose { .. }));
    t.shows("Do you mean: Suppose that the date of Buran's flight is 1989.  Enter: yes · Esc: no");
    t.key(Key::Enter);
    t.says("set aside in this world: 1.1");
    t.shows("WORLD · SUPPOSE THE FLIGHT WAS IN 1989");
    t.shows("worlds actual | suppose: the flight was in 1989");
    t.shows("supposed: date(buran_flight, 1989)");
    t.shows("dismiss world returns to the actual world · adopt world makes this one actual");
    t.shows("0 violations, 0 disagreements");
    t.shows("(1.1 set");
    t.shows("aside here: date(buran_flight, 1988))"); // the note wraps there
    t.command("dismiss world");
    t.says("it stays in the journal");
    assert_eq!(t.app.reason.world(), "actual");
}

/// Steps 6 to 10: a draft imported, a co-author's revision taken in,
/// Leap, a quotation changed and caught, and a note linked, followed and
/// left by back.
fn part_b(t: &mut T) {
    a_draft_imported(t);
    a_revision_taken_in(t);
    t.hold(Quasi::LeapDown, "fully automatic");
    t.look("leap");
    assert!(t.here().starts_with("The chief designer wrote"));
    word_found(t);
    t.command("find");
    a_quotation_checked(t);
    a_note_linked(t);
}

/// Step 10: a note linked to the essay's heading by a [[link]] completed
/// with Tab; the links pane; follow, and back.
fn a_note_linked(t: &mut T) {
    t.tap(Key::End);
    t.tap(Key::Enter);
    t.typ("Check this against [[How");
    t.look("completion");
    t.shows("[[ How Buran’s flight software was reviewed · Tab completes");
    t.key(Key::Tab);
    t.pause();
    t.look("linked");
    t.shows("LINKS OF 2.1.3");
    t.shows("link →");
    t.shows("2. How Buran’s flight software was reviewed  1");
    t.command("go link 1");
    t.look("followed");
    t.shows("link this ←");
    t.shows("2.1.3. Check this against [[How Buran’s flight software was reviewed]]  1");
    t.key(Key::F(7));
    assert!(
        t.here().starts_with("Check this against"),
        "back to the note"
    );
    t.key(Key::F(8));
    assert!(t.here().starts_with("How Buran"), "forward again");
    linked_back(t);
    t.key(Key::F(7));
}

/// Step 8: a word selected with Shift, and found by `find` alone.
fn word_found(t: &mut T) {
    t.key(Key::Left);
    (0..9).for_each(|_| t.key(Key::Special(Special::ShiftLeft)));
    t.command("find");
    t.shows("MATCHES OF “automatic”");
}

/// Step 10: `link` held alone shows its defaults, and its release
/// links the heading back to the note jumped from.
fn linked_back(t: &mut T) {
    t.app.handle(Event::down(Key::Quasi(Quasi::Command)));
    "link".chars().for_each(|c| t.tap(Key::Char(c)));
    t.shows("link (from 2) (to) ‹item› [2.1.3] (as) ‹word› [link]");
    t.app.handle(Event::up(Key::Quasi(Quasi::Command)));
    let heading = t.app.cursor();
    assert_eq!(t.app.notebook.outline.links_of(heading).0.len(), 1);
}

/// Step 6: a document, imported as items.
fn a_draft_imported(t: &mut T) {
    t.command("import document docs/tutorial/draft/essay.md");
    t.says("imported essay.md: 6 items; losses: none");
    t.shows("▼ 2. How Buran’s flight software was reviewed");
    t.shows("Two teams reviewed the flight software.");
}

/// Step 7: the co-author's version, its changes one by one; one is
/// rejected, the other kept.
fn a_revision_taken_in(t: &mut T) {
    t.command("import document docs/tutorial/revised/essay.md");
    for e in t.app.journal().entries() {
        eprintln!(
            "DBG {} {} {}",
            e.seq,
            e.kind(),
            e.body().map(|b| b["op"].to_string()).unwrap_or_default()
        );
    }
    t.look("revision");
    t.shows("WORLD · REVISION OF ESSAY.MD");
    t.shows("entry 52  changed 2.2.1: “Two teams reviewed the flight software.” → “Three teams reviewed the flight software, one of them outside the programme.”");
    t.shows("entry 53  added: “Each team wrote its findings in the DRAKON language.”");
    t.right_click_on("entry 52");
    t.shows("reject this change");
    t.key(Key::Escape);
    t.command("dismiss change 52");
    t.shows("nothing depends on it · Enter retracts · Esc keeps it");
    t.key(Key::Enter);
    t.look("retracted");
    t.command("adopt world");
    t.look("committed");
    assert_eq!(t.app.reason.world(), "actual");
    t.shows("Two teams reviewed the flight software.");
    t.shows("Each team wrote its findings in the DRAKON language.");
}

/// Step 9: a quotation changed, and the check that catches it.
fn a_quotation_checked(t: &mut T) {
    t.retype_end("fully automatic", "fully automatic.".len(), "automatic.");
    t.command("check quote");
    t.says("quote check: misquoted");
    t.retype_end(
        "landing was automatic",
        "automatic.".len(),
        "fully automatic.",
    );
    t.command("check quote");
    t.says("quote check: quoted");
}

/// Steps 11 to 14: boxes and rules in sentences, a question, a rule
/// between them, a change, and a what-if.
fn part_c(t: &mut T) {
    boxes_and_rules(t);
    rules_between_the_boxes(t);
    a_value_changed(t);
    a_what_if(t);
}

/// Step 11: boxes, rules over them, and a question answered.
fn boxes_and_rules(t: &mut T) {
    t.hold(Quasi::LeapDown, "nothing has");
    t.tap(Key::Enter);
    t.tap(Key::BackTab);
    t.typ("Budget");
    t.tap(Key::Enter);
    t.tap(Key::Tab);
    t.typ("The income is 2000.");
    for line in [
        "The rent is 1200.",
        "The food is 400.",
        "If the rent is X and the food is Y then the cost is X + Y.",
        "If the income is X and the cost is Y then the rest is X - Y.",
        "What is the rest?",
    ] {
        t.tap(Key::Enter);
        t.typ(line);
    }
    t.pause();
    t.look("boxes");
    t.read("The food is 400.");
    t.read("What is the rest?");
    t.shows("→ 400");
    t.command("find What is the cost?");
    t.says("1 answer: Answer = 1600");
}

/// Step 12: rules between the boxes, both in sentences.
fn rules_between_the_boxes(t: &mut T) {
    t.tap(Key::Enter);
    t.typ("It is false that the cost is greater than the income.");
    t.pause();
    t.says("constraint It is false that the cost is greater than the income kept: it fires on a planted violation");
    t.tap(Key::Enter);
    t.typ("Nothing has more than 1 value.");
    t.pause();
    t.look("within income");
    t.says("constraint Nothing has more than 1 value kept: it fires on a planted violation");
}

/// Step 13: a value changed, and what follows from it.
fn a_value_changed(t: &mut T) {
    t.retype_end("food is 4", 4, "900.");
    t.look("food 900");
    t.shows("→ -100");
    t.shows("1 constraint violation");
    t.retype_end("food is 9", 4, "500.");
    t.shows("→ 300");
    t.lacks("constraint violation");
}

/// Step 14: what if the rent rose.
fn a_what_if(t: &mut T) {
    t.tapped(Quasi::Suppose, "rent is 1600");
    t.look("suppose rent?");
    t.shows("Do you mean: Suppose that the rent is 1600.  Enter: yes · Esc: no");
    t.key(Key::Enter);
    t.look("supposed rent");
    t.says("set aside in this world: 2.2; left for you: 1 violation of It is false that the cost is greater than the income");
    t.shows("→ -100");
    t.shows("(2.2 set aside here)");
    t.shows("1 violation");
    t.command("dismiss world");
    t.shows("→ 300");
}

/// Steps 15 and 16: a chart of the boxes, and text and a timeline
/// computed from part A's dates.
fn part_d(t: &mut T) {
    let chart = a_chart_with_a_limit(t);
    loud_while_over_income(t, chart);
    a_sentence_computed(t);
    the_dates_as_a_timeline(t);
}

/// Step 16: a second date, a sentence computed from both, and a query.
fn a_sentence_computed(t: &mut T) {
    t.hold(Quasi::LeapDown, "buran flew once, unmanned");
    t.tap(Key::Enter);
    t.typ("The date of Energia-launch is 1987. The label of Energia-launch is \"Energia's first launch\".");
    t.pause();
    t.read("The date of Energia-launch is 1987.");
    t.tap(Key::Enter);
    t.typ("The flight came {N:years} after Energia's first launch. `date(buran_flight, B), date(energia_launch, E), N is B - E`");
    t.pause();
    t.look("derived");
    t.shows("The flight came a year after Energia's first launch. [derived]");
    t.command("find `date(E, Y)`");
    t.says("2 answers: E = buran_flight, Y = 1988 · E = energia_launch, Y = 1987");
}

/// Step 15: a chart of the boxes, with a limit: its item.
fn a_chart_with_a_limit(t: &mut T) -> u64 {
    t.hold(Quasi::LeapDown, "what is the rest");
    t.tap(Key::Enter);
    t.tap(Key::BackTab);
    t.typ("Spending chart");
    t.command("show as chart");
    t.command("show refline(y(I)) where box(income, I)");
    t.look("chart");
    t.shows("VIEW · 2 LAYERS");
    let chart = find(&t.app, "Spending chart");
    let fig = view_of(t, chart).text();
    for label in ["food", "income", "rent", "500", "1200"] {
        assert!(fig.iter().any(|l| l.contains(label)), "{label}: {fig:#?}");
    }
    chart
}

/// Step 15: food at 900 breaks "spending within income" with rent and
/// income, so the three bars are loud; back at 500, none is.
fn loud_while_over_income(t: &mut T, chart: u64) {
    t.retype_end("food is 5", 4, "900.");
    t.look("chart 900");
    let fig = view_of(t, chart);
    assert_eq!(loud_runs(&fig), 3, "income, rent and food bars bold");
    assert!(loud_cells(&fig) > 0);
    // As drawn: the value above each of the three bars, red; the chart
    // keeps a row above its bars, so the income bar's value has room.
    in_tutorial("`900`, `2000` and `1200`, turn red");
    let mut red = chart_labels(t);
    red.sort_by_key(|(v, _)| v.parse::<i64>().unwrap_or(i64::MIN));
    assert_eq!(
        red,
        [
            ("900".to_string(), true),
            ("1200".to_string(), true),
            ("2000".to_string(), true)
        ],
        "{red:?}"
    );
    t.retype_end("food is 9", 4, "500.");
    assert_eq!(loud_cells(&view_of(t, chart)), 0);
}

/// The values drawn in a chart's rows on screen, in order, each with
/// whether it is drawn red; the axis's numbers, left of its line, are
/// not values.
fn chart_labels(t: &T) -> Vec<(String, bool)> {
    let buf = t.buffer();
    let red = memex::ui::violation().fg;
    let red_at = |x: u16, y: u16| Some(buf[(x, y)].fg) == red;
    let labels = chart_rows(&t.screen()).into_iter().flat_map(numbers_in);
    labels.map(|(x, y, v)| (v, red_at(x, y))).collect()
}

/// The rows of a chart on screen: each row's number, the column after
/// its axis, and the text after it.
fn chart_rows(lines: &[String]) -> Vec<(u16, usize, String)> {
    let axis = |l: &String| {
        let i = l.find('│')?;
        let left = l[..i].trim();
        left.chars().all(|c| c.is_ascii_digit()).then_some(i)
    };
    let row = |(y, l): (usize, &String)| {
        let i = axis(l)?;
        let after = l[i + '│'.len_utf8()..].to_string();
        Some((y as u16, l[..i].chars().count() + 1, after))
    };
    lines.iter().enumerate().filter_map(row).collect()
}

/// The numbers in a chart's row, each with its column and row.
fn numbers_in((y, start, text): (u16, usize, String)) -> Vec<(u16, u16, String)> {
    let words = words_at(&text, start).into_iter();
    words
        .filter(|(_, w)| is_number(w))
        .map(|(x, w)| (x as u16, y, w))
        .collect()
}

/// Whether a word is a whole number.
fn is_number(word: &str) -> bool {
    word.chars().all(|c| c.is_ascii_digit() || c == '-')
}

/// The words of a text that starts at a column, each with its column.
fn words_at(text: &str, from: usize) -> Vec<(usize, String)> {
    let mut words = Vec::new();
    let mut current: Option<(usize, String)> = None;
    for (i, c) in text.chars().enumerate() {
        match (c == ' ', current.as_mut()) {
            (false, Some((_, w))) => w.push(c),
            (false, None) => current = Some((from + i, c.to_string())),
            (true, _) => words.extend(current.take()),
        }
    }
    words.extend(current);
    words
}

/// The view at an item, as the outline draws it, 100 wide.
fn view_of(t: &T, item: u64) -> memex::view::Figure {
    memex::ui::view_figure(&t.app, 0, item, 100, 12)
}

/// How many cells of a figure are loud bars.
fn loud_cells(fig: &memex::view::Figure) -> usize {
    let cells = fig.rows.iter().flatten();
    cells
        .filter(|(c, i)| *c == '█' && *i == memex::view::Ink::Loud)
        .count()
}

/// How many runs of side-by-side columns hold a loud bar: a loud bar's
/// width each.
fn loud_runs(fig: &memex::view::Figure) -> usize {
    let cols = loud_columns(fig);
    let gaps = cols.windows(2).filter(|p| p[1] != p[0] + 1).count();
    gaps + usize::from(!cols.is_empty())
}

/// The columns of a figure that hold a loud bar's cell.
fn loud_columns(fig: &memex::view::Figure) -> Vec<usize> {
    let w = fig.rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let loud = Some(&('█', memex::view::Ink::Loud));
    (0..w)
        .filter(|x| fig.rows.iter().any(|r| r.get(*x) == loud))
        .collect()
}

/// Step 16: part A's dates as a timeline; the claimed date is not a
/// belief, so not drawn.
fn the_dates_as_a_timeline(t: &mut T) {
    t.tap(Key::Enter);
    t.typ("Timeline of the programme");
    t.command("show tick(x(Y), text(L)) where date(E, Y), label(E, L)");
    t.look("timeline");
    let tl = find(&t.app, "Timeline of the programme");
    let fig = view_of(t, tl).text().join("\n");
    for label in ["Energia's first launch", "Buran's flight"] {
        in_tutorial(label);
        assert!(fig.contains(label), "{fig}");
    }
    assert!(
        !fig.contains("1989"),
        "the claimed date is not a belief, so not drawn: {fig}"
    );
}

fn find(app: &App, text: &str) -> u64 {
    let outline = &app.notebook.outline;
    let mut items = outline.in_order().into_iter();
    items
        .find(|i| outline.text(*i) == text)
        .unwrap_or_else(|| panic!("no item {text:?}"))
}

#[test]
fn the_configuration_for_a_desktop_that_takes_super_is_accepted() {
    // Catches the tutorial's configuration refused at start (a key given
    // two jobs, a name misspelt), or not moving Suppose off Super.
    let blocks: Vec<&str> = TUTORIAL.split("```toml\n").skip(1).collect();
    assert_eq!(blocks.len(), 1, "one configuration in the tutorial");
    let block = blocks[0].split("```").next().unwrap();
    let config = memex::config::Config::parse(block).unwrap_or_else(|e| panic!("{e}: {block}"));
    let suppose = config.quasimodes.iter().find(|(q, _)| *q == Quasi::Suppose);
    let keys = format!("{:?}", suppose.map(|(_, k)| k));
    assert!(
        keys.contains("RightControl") && !keys.contains("Super"),
        "{keys}"
    );
}
