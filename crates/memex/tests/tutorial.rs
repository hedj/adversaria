//! docs/tutorial.md, performed: each step's keys, and each thing the
//! tutorial says the screen will show. The test also checks that every
//! command it types and every text it expects appears in the tutorial word
//! for word, so the two cannot drift apart unnoticed.

use memex::app::{App, Event, Key, Mode, Quasi};
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
    fn command(&mut self, line: &str) {
        self.hold(Quasi::Command, line);
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
        self.hold(Quasi::LeapForward, leap);
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
    let dir = temp("all");
    let mut app = App::open(&dir).unwrap();
    app.set_durable(false);
    let mut t = T {
        app,
        dump: std::env::var_os("TUTORIAL_DUMP").is_some(),
    };

    t.look("start");
    t.shows("(empty: type to write the first item)");

    part_one(&mut t);

    // Part C, then part D, in the same session.
    part_two(&mut t);
    part_three(&mut t);

    // Step 16: leave. The journal, opened again, replays to the same budget.
    t.key(Key::F(10));
    assert!(t.app.session.quit);
    t.app.flush();
    let mut app = replayed(&dir);
    app.set_durable(false);
    app.evaluate().unwrap();
    let t = T { app, dump: t.dump };
    t.shows("Left over: 300 [derived]");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Steps 2 to 8: an outline, facts read from what is written, a derived
/// sentence, a claim and a constraint, Leap, interrogation and adoption,
/// and a supposition.
fn part_one(t: &mut T) {
    // Step 2: an outline.
    t.typ("The programme");
    t.tap(Key::Enter);
    t.typ("Buran flew once, unmanned, on 15 November 1988.");
    t.tap(Key::Tab);
    t.pause();
    t.look("outline");
    t.shows("▼ 1. The programme");
    t.shows("  • Buran flew once, unmanned, on 15 November 1988.");
    t.shows("prose: not ACE, it stops at Buran <> flew once, unmanned, on 15 November 1988.");

    // Step 3: facts, written so that the memex reads them.
    t.tap(Key::Enter);
    t.typ("The date of Buran-flight is 1988. The label of Buran-flight is \"Buran's flight\".");
    t.tap(Key::Enter);
    t.typ("The date of Energia-launch is 1987. The label of Energia-launch is \"Energia's first launch\".");
    t.pause();
    t.look("facts");
    t.read("The date of Buran-flight is 1988.");
    t.read("The date of Energia-launch is 1987.");
    t.lacks("prose: not ACE");
    t.shows("READ AS");
    t.shows("fact  date(energia_launch, 1987)");
    t.shows("fact  label(energia_launch, \"Energia's first launch\")");

    // Step 4: a derived sentence.
    t.tap(Key::Enter);
    t.typ("gap");
    t.command("derive The flight came {N:years} after Energia's first launch. where date(buran_flight, B), date(energia_launch, E), N is B - E");
    t.look("derived");
    t.shows("The flight came a year after Energia's first launch. [derived]");
    t.command("query date(E, Y)");
    t.says("2 answers: E = buran_flight, Y = 1988 · E = energia_launch, Y = 1987");

    // Step 5: a claim and a constraint.
    t.tap(Key::Enter);
    t.typ("Talk-notes says that the date of Buran-flight is 1989.");
    t.tap(Key::Enter);
    t.typ("Nothing has more than 1 date.");
    t.pause();
    t.look("constraint");
    t.says("constraint Nothing has more than 1 date kept: it fires on a planted violation");
    t.shows("1 disagreement between sources");
    t.shows("⚑ Nothing has more than 1 date, sources disagree: 1988 here, 1989 in 1.5");
    t.lacks("constraint violation");

    // Step 6: Leap, from the end of the outline, wrapping to the top.
    t.hold(Quasi::LeapForward, "date of buran");
    t.look("leap");
    assert!(t.here().starts_with("The date of Buran-flight"));
    t.shows("CONSTRAINT · NOTHING HAS MORE THAN 1 DATE");

    // Step 7: interrogate, adopt, unadopt.
    t.hold(Quasi::LeapForward, "talk");
    t.key(Key::F(2));
    t.look("interrogate");
    t.shows("INTERROGATE · WHAT IS CLAIMED HERE");
    t.shows("who claims the contrary");
    t.shows("adopting it would raise 1 violation of Nothing has more than 1 date");
    t.key(Key::F(3));
    t.look("adopt");
    t.shows("1 constraint violation");
    t.shows("⚑ Nothing has more than 1 date, violation: 1988 here, 1989 in 1.5");
    t.key(Key::F(4));
    t.look("unadopt");
    t.says("retracted");
    t.lacks("constraint violation");
    t.shows("1 disagreement between sources");

    // Step 8: suppose.
    t.hold(Quasi::Suppose, "the flight was in 1989");
    assert!(matches!(t.app.session.mode, Mode::ConfirmSuppose { .. }));
    t.look("suppose?");
    t.shows("Do you mean: Suppose that the date of Buran's flight is 1989.  ⏎ yes · Esc no");
    t.key(Key::Enter);
    t.look("supposed");
    t.says("set aside in this world: 1.2");
    t.shows("WORLD · SUPPOSE THE FLIGHT WAS IN 1989");
    t.shows("The flight came two years after Energia's first launch.");
    t.command("close");
    t.look("closed");
    t.says("it stays in the journal");
    assert_eq!(t.app.reason.world(), "actual");
    t.shows("The flight came a year after Energia's first launch.");
}

fn part_two(t: &mut T) {
    // Step 9: boxes after the last item of part B, at the top level.
    t.hold(Quasi::LeapForward, "nothing has");
    t.tap(Key::Enter);
    t.tap(Key::BackTab);
    t.typ("Budget");
    t.tap(Key::Enter);
    t.tap(Key::Tab);
    t.typ("The income is 2000.");
    t.tap(Key::Enter);
    t.typ("The rent is 1200.");
    t.tap(Key::Enter);
    t.typ("The food is 400.");
    t.tap(Key::Enter);
    t.typ("If the rent is X and the food is Y then the cost is X + Y.");
    t.tap(Key::Enter);
    t.typ("If the income is X and the cost is Y then the rest is X - Y.");
    t.pause();
    t.look("boxes");
    t.says("rule rest/1 kept");
    t.read("The food is 400.");
    t.tap(Key::Enter);
    t.typ("left");
    t.command("derive Left over: {L} where rest(L)");
    t.look("left over");
    t.shows("Left over: 400 [derived]");
    t.tap(Key::Enter);
    t.typ("What is the rest?");
    t.pause();
    t.look("question");
    t.read("What is the rest?");
    t.shows("→ 400");

    // Step 10: constraints.
    t.tap(Key::Enter);
    t.typ("It is false that the cost is greater than the income.");
    t.pause();
    t.look("within income");
    t.says("constraint It is false that the cost is greater than the income kept: it fires on a planted violation");
    t.command("constraint one value per box: false :- cell(B, X), cell(B, Y), X \\= Y.");
    t.says("constraint one value per box kept: it fires on a planted violation");

    // Step 11: change a value.
    t.retype_end("food is 4", 4, "900.");
    t.look("food 900");
    t.shows("Left over: -100 [derived]");
    t.shows("1 constraint violation");
    t.retype_end("food is 9", 4, "500.");
    t.look("food 500");
    t.shows("Left over: 300 [derived]");
    t.lacks("constraint violation");

    // Step 12: what if.
    t.hold(Quasi::Suppose, "rent is 1600");
    t.look("suppose rent?");
    t.shows("Do you mean: Suppose that the cell of rent is 1600.  ⏎ yes · Esc no");
    t.key(Key::Enter);
    t.look("supposed rent");
    t.says("set aside in this world: 2.2; left for you: 1 violation of It is false that the cost is greater than the income");
    t.shows("Left over: -100");
    t.command("close");
    t.shows("Left over: 300 [derived]");
}

fn part_three(t: &mut T) {
    let chart = a_chart_with_a_limit(t);
    loud_while_over_income(t, chart);
    the_boxes_as_a_grid(t);
    the_dates_as_a_timeline(t);
}

/// Step 13: a chart of the boxes, with a limit: its item.
fn a_chart_with_a_limit(t: &mut T) -> u64 {
    t.hold(Quasi::LeapForward, "left over");
    t.tap(Key::Enter);
    t.tap(Key::BackTab);
    t.typ("Spending chart");
    t.command("layer bar(x(C), y(V)) where cell(C, V)");
    t.command("layer rule(y(I)) where cell(income, I)");
    t.look("chart");
    t.shows("VIEW · 2 LAYERS");
    let chart = find(&t.app, "Spending chart");
    let fig = view_of(t, chart).text();
    for label in ["food", "income", "rent", "500", "1200"] {
        assert!(fig.iter().any(|l| l.contains(label)), "{label}: {fig:#?}");
    }
    chart
}

/// Step 13: food at 900 breaks "spending within income" with rent and
/// income, so the three bars are loud; back at 500, none is.
fn loud_while_over_income(t: &mut T, chart: u64) {
    t.retype_end("food is 5", 4, "900.");
    t.look("chart 900");
    let fig = view_of(t, chart);
    assert_eq!(loud_runs(&fig), 3, "income, rent and food bars bold");
    assert!(loud_cells(&fig) > 0);
    t.retype_end("food is 9", 4, "500.");
    assert_eq!(loud_cells(&view_of(t, chart)), 0);
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

/// Step 14: the same boxes as a grid.
fn the_boxes_as_a_grid(t: &mut T) {
    t.tap(Key::Enter);
    t.typ("Budget table");
    t.command("layer cell(x(value), y(C), text(V)) where cell(C, V)");
    t.look("table");
    t.shows("food   │   500");
    t.shows("income │  2000");
}

/// Step 15: part A's dates as a timeline; the claimed date is not a
/// belief, so not drawn.
fn the_dates_as_a_timeline(t: &mut T) {
    t.tap(Key::Enter);
    t.typ("Timeline of the programme");
    t.command("layer tick(x(Y), text(L)) where date(E, Y), label(E, L)");
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
