//! Milestone 5's acceptance checks (docs/plan.md, Milestone 5): Wikidata and
//! suggestions without a model. tests/fixtures/wikidata-buran.pl is real
//! Wikidata (CC0), made by `wikidata --items Q2548,Q4273185,Q1010251,Q80811`.

use format::terms::Term;
use memex::app::{App, Event, Key, PaneKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

mod support;
use support::{Room, replayed};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/wikidata-buran.pl"
);

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
    app.evaluate().unwrap();
}

fn item(app: &mut App, text: &str) {
    tap(app, Key::Enter);
    typ(app, text);
    app.flush();
}

fn screen(app: &App) -> Vec<String> {
    let (w, h) = (200, 45);
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

fn assert_shows(s: &[String], text: &str) {
    assert!(
        s.iter().any(|l| l.contains(text)),
        "screen lacks {text:?}:\n{}",
        s.join("\n")
    );
}

fn described(app: &App) -> Vec<String> {
    app.reason
        .suggestions
        .shown()
        .iter()
        .map(|s| s.describe())
        .collect()
}

/// Accept the shown suggestion whose description starts so.
fn accept(app: &mut App, start: &str) {
    let n = described(app)
        .iter()
        .position(|d| d.starts_with(start))
        .unwrap_or_else(|| panic!("no suggestion {start:?} in {:?}", described(app)))
        + 1;
    command(app, &format!("accept {n}"));
}

/// The number of the last entry of a kind.
fn last_of(app: &App, kind: &str) -> u64 {
    let entries = app.journal().entries().iter();
    entries.rev().find(|e| e.kind() == kind).unwrap().seq
}

fn open(name: &str) -> Room {
    let mut app = Room::new(&format!("m5-{name}"));
    app.set_durable(false);
    app
}

#[test]
fn a_planted_wrong_date_is_reported_against_wikidata() {
    // Plan: "A planted wrong date in the user's notes is reported against
    // Wikidata." The notes say Buran flew in 1989; Wikidata says 1988.
    let mut app = notes_of_1989("planted-date");
    command(&mut app, &format!("import wikidata {FIXTURE}"));
    the_import_is_kept(&app);
    the_spaceplane_comes_first(&app);
    command(&mut app, "pane suggestions");
    accept(&mut app, "buran_flight is Wikidata Q2548");
    the_adoption_keeps_its_evidence(&app);
    accept(&mut app, "date is Wikidata's P606");
    let s = screen(&app);
    assert_shows(&s, "1 disagreement between sources");
    let mark = "? sources disagree under “one date per event”: 1989 here; 1988 in Wikidata";
    assert_shows(&s, mark);
}

/// Notes that say Buran flew in 1989, with its label and the constraint of
/// one date per event.
fn notes_of_1989(name: &str) -> Room {
    let mut app = open(name);
    typ(&mut app, "Buran flew once, unmanned, in 1989.");
    app.flush();
    command(&mut app, "fact date(buran_flight, 1989)");
    command(&mut app, "fact label(buran_flight, \"Buran's flight\")");
    let one_date = "false :- date(E, D1), date(E, D2), D1 \\= D2.";
    command(
        &mut app,
        &format!("constraint one date per event: {one_date}"),
    );
    app
}

/// The last entry is an import from Wikidata, whose blob is kept.
fn the_import_is_kept(app: &Room) {
    let import = app.journal().entries().last().unwrap().body().unwrap();
    assert_eq!(import["source"], "Wikidata");
    let blob = app.dir.join("blobs").join(import["blob"].as_str().unwrap());
    assert!(blob.exists());
    assert_eq!(app.journal().entries().last().unwrap().kind(), "import");
}

/// The spaceplane, not the village of the same name, comes first, for a
/// launch within five years.
fn the_spaceplane_comes_first(app: &App) {
    let described = described(app);
    let first = described.first().map(String::as_str);
    assert_eq!(
        first,
        Some("buran_flight is Wikidata Q2548"),
        "{described:?}"
    );
    let evidence = &app.reason.suggestions.shown()[0].evidence;
    assert!(evidence.contains("within five years"));
}

/// The last adoption keeps the evidence of what it accepted.
fn the_adoption_keeps_its_evidence(app: &App) {
    let adopt = &app.journal().entries()[last_of(app, "adopt") as usize - 1];
    let evidence = adopt.body().unwrap()["suggestion"]["evidence"].as_str();
    assert!(
        evidence.unwrap().contains("Q2548"),
        "the adoption keeps its evidence"
    );
}

/// Sputnik 1's launch, its Wikidata mappings accepted, and Wikidata's
/// constraint of one launch per entity adopted.
fn launch_constraint(name: &str) -> Room {
    let mut app = open(name);
    typ(&mut app, "Sputnik 1 was launched in 1957.");
    app.flush();
    command(&mut app, "fact launched(sputnik_1, 1957)");
    command(&mut app, "fact label(sputnik_1, \"Sputnik 1\")");
    command(&mut app, &format!("import wikidata {FIXTURE}"));
    accept(&mut app, "sputnik_1 is Wikidata Q80811");
    accept(&mut app, "launched is Wikidata's P619");
    accept(
        &mut app,
        "constraint one launched per entity (Wikidata P619)",
    );
    app
}

#[test]
fn wikidata_offers_its_own_constraint() {
    // Plan: "Wikidata's property constraints offered as constraints". On
    // real data: P619 (UTC date of spacecraft launch) has a single-value
    // constraint. Tried against a planted violation before it is kept.
    let app = launch_constraint("wd-constraint");
    assert!(
        app.notebook
            .outline
            .constraints
            .values()
            .any(|c| c.name == "one launched per entity (Wikidata P619)")
    );
    let e = app.journal().entries().last().unwrap();
    assert_eq!(
        (e.kind(), &e.body().unwrap()["op"]),
        ("adopt", &serde_json::json!("constraint"))
    );
    assert!(
        !described(&app).iter().any(|d| d.starts_with("constraint")),
        "not offered again once adopted"
    );
}

/// Places, each part of a country and located in it: a rule to mine.
fn mined(name: &str) -> Room {
    let mut app = open(name);
    typ(&mut app, "Places");
    app.flush();
    for f in [
        "part_of(baikonur, kazakhstan)",
        "part_of(plesetsk, russia)",
        "located_in(baikonur, kazakhstan)",
        "located_in(plesetsk, russia)",
    ] {
        command(&mut app, &format!("fact {f}"));
    }
    app
}

#[test]
fn a_mined_rule_shows_its_evidence_and_derives_once_adopted() {
    // Plan: "rules mined from the facts (... a simpler miner with support
    // and confidence)". Catches a rule offered without its support and
    // exceptions, or one that derives nothing once adopted.
    let mut app = mined("rules");
    accept(&mut app, "rule located_in(_");
    let e = app.journal().entries().last().unwrap();
    assert!(
        e.body().unwrap()["suggestion"]["evidence"]
            .as_str()
            .unwrap()
            .contains("support 2, confidence 1.00, 0 exceptions")
    );
    item(&mut app, "Vostochny");
    command(&mut app, "fact part_of(vostochny, russia)");
    item(&mut app, "derived");
    command(
        &mut app,
        "derive Vostochny is in {C}. where located_in(vostochny, C)",
    );
    assert_shows(&screen(&app), "Vostochny is in russia. [derived]");
}

/// Buran's flight and two papers, each the same as a work of a Wikidata
/// subset of synthetic works, QTEST1 to QTEST3: QTEST3 is about Buran and
/// cited by both others, and it is not in the store.
fn references(name: &str) -> Room {
    let mut app = open(name);
    let subset = app.dir.join("subset.pl");
    std::fs::write(
        &subset,
        "wd('Q2548', 'P606', date(1988, 11, 15)).\nwd_label('Q2548', \"Buran\").\n\
         wd('QTEST1', 'P31', 'Q13442814').\nwd('QTEST1', 'P921', 'Q2548').\nwd('QTEST1', 'P2860', 'QTEST3').\n\
         wd('QTEST2', 'P31', 'Q13442814').\nwd('QTEST2', 'P2860', 'QTEST1').\nwd('QTEST2', 'P2860', 'QTEST3').\n\
         wd('QTEST3', 'P31', 'Q13442814').\nwd('QTEST3', 'P921', 'Q2548').\nwd_label('QTEST3', \"The obvious answer\").\n",
    )
    .unwrap();
    typ(&mut app, "Buran flew once, in 1988.");
    app.flush();
    command(&mut app, "fact date(buran_flight, 1988)");
    command(&mut app, "fact same_as(buran_flight, 'Q2548')");
    item(&mut app, "Paper A");
    command(&mut app, "fact same_as(paper_a, 'QTEST1')");
    item(&mut app, "Paper B");
    command(&mut app, "fact same_as(paper_b, 'QTEST2')");
    command(&mut app, &format!("import wikidata {}", subset.display()));
    app
}

#[test]
fn no_suggested_reference_names_a_work_outside_the_store() {
    // Plan: "No suggested reference names a work outside the store (test
    // with a store that lacks the obvious answer)." The works here are
    // synthetic, ids QTEST1 to QTEST3: QTEST3 is about Buran and cited by
    // both others, the obvious answer, and it is not in the store.
    let mut app = references("refs");
    let only = ["cite paper_a for the claims about buran_flight"];
    assert_eq!(suggested_references(&app), only);
    names_no_work_outside_the_store(&app);
    accept(&mut app, "cite paper_a");
    let (a, b) = (numbered(&app, "1"), numbered(&app, "2"));
    assert_eq!(last_link(&app), (a, b, "cites".to_string()));
}

/// The references suggested, described.
fn suggested_references(app: &App) -> Vec<String> {
    let list = app.reason.suggestions.list.iter();
    let refs = list.filter(|s| s.kind_name() == "reference");
    refs.map(|s| s.describe()).collect()
}

/// No suggestion names QTEST3, the work outside the store, nor rests on it.
fn names_no_work_outside_the_store(app: &App) {
    for s in &app.reason.suggestions.list {
        let named = s.describe().contains("QTEST3") || s.evidence.contains("QTEST3");
        assert!(!named, "names a work outside the store: {s:?}");
    }
}

/// The item of a number in the outline.
fn numbered(app: &App, n: &str) -> u64 {
    app.notebook.outline.by_number(n).unwrap()
}

/// The outline's last link: from, to, and its kind.
fn last_link(app: &App) -> (u64, u64, String) {
    let link = app.notebook.outline.links.last().unwrap();
    (link.from, link.to, link.kind.clone())
}

#[test]
fn the_pane_keeps_its_rules() {
    // Plan: "never takes the keyboard, never reorders under the pointer,
    // ranked by evidence, few in view, dismissals recorded".
    let mut app = many_suggestions("pane");
    few_in_view_ranked_by_evidence(&app);
    // It never takes the keyboard: typing goes to the outline.
    typ(&mut app, " here");
    app.flush();
    assert_eq!(app.notebook.outline.text(app.cursor()), "Places here");
    let first = dismissing_the_first(&mut app);
    the_order_holds_while_the_suggestions_are_listed(&mut app);
    a_replay_keeps_the_dismissal(&mut app, &first);
}

/// Places, with four pairs of predicates over three pairs of entities:
/// more suggestions than the pane shows.
fn many_suggestions(name: &str) -> Room {
    let mut app = open(name);
    typ(&mut app, "Places");
    app.flush();
    let preds = ["p1", "p2", "p3", "p4"]
        .iter()
        .zip(["q1", "q2", "q3", "q4"]);
    for (p, q) in preds {
        for (x, y) in [("a", "x"), ("b", "y"), ("c", "z")] {
            command(&mut app, &format!("fact {p}({x}, {y})"));
            command(&mut app, &format!("fact {q}({x}, {y})"));
        }
    }
    app
}

/// More suggestions than five, five in view, and scores that never rise
/// down the list.
fn few_in_view_ranked_by_evidence(app: &App) {
    let list = &app.reason.suggestions.list;
    assert!(list.len() > 5, "{} suggestions", list.len());
    assert_eq!(app.reason.suggestions.shown().len(), 5, "few in view");
    let scores: Vec<i64> = list.iter().map(|s| s.score).collect();
    assert!(scores.windows(2).all(|w| w[0] >= w[1]), "{scores:?}");
}

/// Dismiss the first suggestion shown: it does not come back, and the
/// dismissal is journaled. What it described.
fn dismissing_the_first(app: &mut App) -> String {
    let first = described(app)[0].clone();
    command(app, "dismiss 1");
    assert!(!described(app).contains(&first));
    let last = app.journal().entries().last().unwrap().body().unwrap();
    assert_eq!(last["op"], "dismiss");
    first
}

/// While the suggestions are listed, the order holds as new evidence
/// arrives; none is shown twice when one leaves; and once the list is
/// closed, the order is the ranking again.
fn the_order_holds_while_the_suggestions_are_listed(app: &mut App) {
    list_the_suggestions(app);
    let before = described(app);
    for (x, y) in [("d", "w"), ("e", "v"), ("f", "u")] {
        command(app, &format!("fact p4({x}, {y})"));
        command(app, &format!("fact q1({x}, {y})"));
    }
    let during = described(app);
    kept_ahead(&before, &during);
    none_shown_twice(app);
    ranked_once_no_longer_listed(app, &during);
}

fn list_the_suggestions(app: &mut App) {
    command(app, "find suggestions");
    let view = &app.session.view;
    assert_eq!(view.panes[1].kind, PaneKind::Suggestions);
}

/// A suggestion whose evidence no longer holds may leave; those that stay
/// keep their order, ahead of new ones.
fn kept_ahead(before: &[String], during: &[String]) {
    let kept: Vec<&String> = before.iter().filter(|b| during.contains(b)).collect();
    assert!(!kept.is_empty(), "the test needs some suggestions to stay");
    let first: Vec<&String> = during.iter().take(kept.len()).collect();
    assert_eq!(first, kept, "reordered under the pointer");
}

/// One that leaves the held order gives its place to the next, and none
/// is shown twice.
fn none_shown_twice(app: &mut App) {
    command(app, "dismiss 1");
    let after = described(app);
    let distinct: std::collections::BTreeSet<&String> = after.iter().collect();
    assert_eq!((distinct.len(), after.len()), (5, 5), "{after:?}");
}

/// Once the list is closed, the order is the ranking again.
fn ranked_once_no_longer_listed(app: &mut App, during: &[String]) {
    let list = app.reason.suggestions.list.iter().take(5);
    let ranked: Vec<String> = list.map(|s| s.describe()).collect();
    assert_ne!(during, ranked, "the test needs the held order to differ");
    command(app, "find");
    assert_eq!(
        described(app),
        ranked,
        "the order is held after the list is closed"
    );
}

/// A replay keeps the dismissal: the first is not suggested again.
fn a_replay_keeps_the_dismissal(app: &mut Room, first: &str) {
    app.flush();
    let mut r = replayed(&app.dir);
    r.evaluate().unwrap();
    assert_eq!(
        r.reason.suggestions.dismissed,
        app.reason.suggestions.dismissed
    );
    assert!(!suggests(&r, first));
}

/// Whether a memex suggests what a text describes.
fn suggests(app: &App, text: &str) -> bool {
    let mut list = app.reason.suggestions.list.iter();
    list.any(|s| s.describe() == text)
}

#[test]
fn a_suggestion_is_offered_once() {
    // The measures count the suggestions offered (drawn on the screen), so
    // each is recorded once, even after more changes or a replay.
    let mut app = open("offered");
    typ(&mut app, "Places");
    app.flush();
    command(&mut app, "pane suggestions");
    for p in ["p1", "p2", "p3"] {
        command(&mut app, &format!("fact {p}(a, x)"));
        command(&mut app, &format!("fact {p}(b, y)"));
    }
    command(&mut app, "fact p1(c, z)");
    let offered = offered_keys(&app);
    assert!(!offered.is_empty(), "the test needs suggestions offered");
    let mut once = offered.clone();
    once.sort();
    once.dedup();
    assert_eq!(offered.len(), once.len(), "offered twice: {offered:?}");
    app.flush();
    let mut r = replayed(&app.dir);
    r.evaluate().unwrap();
    assert_eq!(offered_keys(&r), offered, "offered again after a replay");
}

/// The keys of the suggestions offered, in the order the journal records
/// them.
fn offered_keys(app: &App) -> Vec<String> {
    let offers = app.journal().entries().iter().filter_map(|e| e.body());
    let offers = offers.filter(|b| b["op"] == "offer");
    let keys = offers.flat_map(|b| b["keys"].as_array().cloned().unwrap_or_default());
    keys.map(|k| k.as_str().unwrap().to_string()).collect()
}

#[test]
fn a_retracted_import_is_not_read() {
    // Catches a world that still reads a Wikidata import retracted from it.
    let mut app = open("retracted-import");
    typ(&mut app, "Buran flew once, unmanned, in 1989.");
    app.flush();
    command(&mut app, "fact date(buran_flight, 1989)");
    command(&mut app, &format!("import wikidata {FIXTURE}"));
    assert!(
        !described(&app).is_empty(),
        "the test needs the import read"
    );
    let retract = format!("retract {}", last_of(&app, "import"));
    command(&mut app, &retract);
    tap(&mut app, Key::Enter);
    app.evaluate().unwrap();
    assert_eq!(described(&app), Vec::<String>::new());
}

#[test]
fn an_adopted_constraint_keeps_its_rule() {
    let app = launch_constraint("wd-constraint-rule");
    let rules = app.notebook.outline.constraints.values();
    assert!(rules.into_iter().any(|c| c.rule.starts_with("false :- ")));
}

#[test]
fn an_adopted_rule_is_numbered_by_its_own_entry() {
    let mut app = mined("rule-number");
    accept(&mut app, "rule located_in(_");
    let seq = Term::Int(last_of(&app, "adopt") as i64);
    assert!(matches!(last_fact(&app), Term::Compound(_, a) if a[0] == seq));
}

#[test]
fn an_accepted_reference_cites_the_work_from_the_item_it_is_about() {
    let mut app = references("refs-accepted");
    accept(&mut app, "cite paper_a for the claims about buran_flight");
    assert_eq!(last_fact(&app), cites(&app, 0, 1));
}

/// The fact that the item at one place in the outline cites the item at
/// another.
fn cites(app: &App, from: usize, to: usize) -> Term {
    let order = app.notebook.outline.in_order();
    let item = |k: usize| Term::Int(order[k] as i64);
    Term::Compound("cites".into(), vec![item(from), item(to)])
}

/// The first fact of the last entry.
fn last_fact(app: &App) -> Term {
    let e = app.journal().entries().last().unwrap();
    format::terms::decode(&e.body().unwrap()["facts"][0]).unwrap()
}
