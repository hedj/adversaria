//! docs/tutorial.md, performed: each step's keys, and each thing the
//! tutorial says the screen will show. The test also checks that every
//! command it types and every text it expects appears in the tutorial word
//! for word, so the two cannot drift apart unnoticed.

use memex::app::{App, Event, Key, Mode, Quasi};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
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
    fn key(&mut self, k: Key) {
        self.tap(k);
        self.app.evaluate().unwrap();
    }
    fn screen(&self) -> Vec<String> {
        let (w, h) = (200, 45);
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| memex::ui::draw(f, &self.app)).unwrap();
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

/// Steps 2 to 8: an outline, beliefs, a derived sentence, a claim and a
/// constraint, Leap, interrogation and adoption, and a supposition.
fn part_one(t: &mut T) {
    // Step 2: an outline.
    t.typ("The programme");
    t.tap(Key::Enter);
    t.typ("Buran flew once, unmanned, on 15 November 1988.");
    t.tap(Key::Tab);
    t.app.flush();
    t.look("outline");
    t.shows("▼ 1. The programme");
    t.shows("  • Buran flew once, unmanned, on 15 November 1988.");

    // Step 3: beliefs.
    t.command("fact date(buran_flight, 1988)");
    t.command("fact label(buran_flight, \"Buran's flight\")");
    t.tap(Key::Enter);
    t.typ("Energia, its launcher, first flew in May 1987.");
    t.command("fact date(energia_launch, 1987)");
    t.command("fact label(energia_launch, \"Energia's first launch\")");
    t.look("facts");

    // Step 4: a derived sentence.
    t.tap(Key::Enter);
    t.typ("gap");
    t.command("derive The flight came {N:years} after Energia's first launch. where date(buran_flight, B), date(energia_launch, E), N is B - E");
    t.look("derived");
    t.shows("The flight came a year after Energia's first launch. [derived]");

    // Step 5: a claim and a constraint.
    t.tap(Key::Enter);
    t.typ("Talk notes: the 1989 flight was the only one.");
    t.command("claim date(buran_flight, 1989) from my notes, conference talk");
    t.command("constraint one date per event: false :- date(E, D1), date(E, D2), D1 \\= D2.");
    t.look("constraint");
    t.says("constraint one date per event kept: it fires on a planted violation");
    t.shows("1 disagreement between sources");
    t.shows("⚑ one date per event, sources disagree: 1988 here, 1989 in 1.4");
    t.lacks("constraint violation");

    // Step 6: Leap, from the end of the outline, wrapping to the top.
    t.hold(Quasi::LeapForward, "buran");
    t.look("leap");
    assert!(t.here().starts_with("Buran"));
    t.shows("CONSTRAINT · ONE DATE PER EVENT");

    // Step 7: interrogate, adopt, unadopt.
    t.hold(Quasi::LeapForward, "talk");
    t.key(Key::F(2));
    t.look("interrogate");
    t.shows("INTERROGATE · WHAT IS CLAIMED HERE");
    t.shows("who claims the contrary");
    t.shows("adopting it would raise 1 violation of one date per event");
    t.key(Key::F(3));
    t.look("adopt");
    t.shows("1 constraint violation");
    t.shows("⚑ one date per event, violation: 1988 here, 1989 in 1.4");
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
    t.says("set aside in this world: 1.1");
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
    t.hold(Quasi::LeapForward, "talk");
    t.tap(Key::Enter);
    t.tap(Key::BackTab);
    t.typ("Budget");
    t.tap(Key::Enter);
    t.typ("income");
    t.tap(Key::Tab);
    t.command("fact cell(income, 2000)");
    t.command("derive Income: {V} where cell(income, V)");
    t.look("income");
    t.shows("Income: 2000 [derived]");

    //         More boxes, and one computed from them.
    t.tap(Key::Enter);
    t.typ("rent");
    t.command("fact cell(rent, 1200)");
    t.command("derive Rent: {V} where cell(rent, V)");
    t.tap(Key::Enter);
    t.typ("food");
    t.command("fact cell(food, 400)");
    t.command("derive Food: {V} where cell(food, V)");
    t.tap(Key::Enter);
    t.typ("left");
    t.command(
        "derive Left over: {L} where cell(income, I), cell(rent, R), cell(food, F), L is I - R - F",
    );
    t.look("left over");
    t.shows("Left over: 400 [derived]");

    // Step 10: rules.
    t.command("constraint one value per cell: false :- cell(C, A), cell(C, B), A \\= B.");
    t.command("constraint spending within income: false :- cell(income, I), cell(rent, R), cell(food, F), R + F > I.");
    t.says("constraint spending within income kept: it fires on a planted violation");

    // Step 11: change a value.
    t.command("set cell(food, 900)");
    t.look("set 900");
    t.says("set; replaced the fact at 2.3; left for you: 1 violation of spending within income");
    t.shows("Food: 900 [derived]");
    t.shows("Left over: -100 [derived]");
    t.shows("1 constraint violation");
    t.command("set cell(food, 500)");
    t.look("set 500");
    t.shows("Left over: 300 [derived]");
    t.lacks("constraint violation");

    // Step 12: what if.
    t.hold(Quasi::Suppose, "rent is 1600");
    t.look("suppose rent?");
    t.shows("Do you mean: Suppose that the cell of rent is 1600.  ⏎ yes · Esc no");
    t.key(Key::Enter);
    t.look("supposed rent");
    t.says("set aside in this world: 2.2; left for you: 1 violation of spending within income");
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
    t.command("set cell(food, 900)");
    t.look("chart 900");
    let fig = view_of(t, chart);
    assert_eq!(loud_runs(&fig), 3, "income, rent and food bars bold");
    assert!(loud_cells(&fig) > 0);
    t.command("set cell(food, 500)");
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
