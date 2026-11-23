//! The verbs a right-click offers for an item (docs/concepts-next.md
//! § How far, move 1; crate::verbs).

mod support;

use memex::app::{Key, Mode, Special};
use support::Room;

/// A right-click where a text starts, on the word there, as the window
/// makes it: the menu's offers, by their words.
fn menu_on(r: &mut Room, text: &str) -> Vec<String> {
    let at = r.find(text);
    let word = text.split_whitespace().next().unwrap_or("");
    let hits = r.hits();
    r.app.right_click(&hits, at, word, false);
    match &r.app.session.mode {
        memex::app::Mode::Menu(m) => m.offers.iter().map(|o| o.label.clone()).collect(),
        _ => vec![],
    }
}

/// Three items, Alder, Birch and Cedar, the cursor on Cedar.
fn abc(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Alder", "Birch", "Cedar"]);
    r
}

/// Run the menu's offer whose words start so: Down to it, then Enter,
/// since only the first nine have numbers.
fn choose(r: &mut Room, words: &str) {
    let offers = match &r.app.session.mode {
        memex::app::Mode::Menu(m) => m.offers.clone(),
        other => panic!("no menu: {other:?}"),
    };
    let at = offers.iter().position(|o| o.label.starts_with(words));
    let at = at.unwrap_or_else(|| panic!("{words} not offered: {offers:?}"));
    (0..at).for_each(|_| r.tap(&[Key::Down]));
    r.tap(&[Key::Enter]);
}

/// The texts of the items, in order.
fn texts(r: &Room) -> Vec<String> {
    let order = r.outline().in_order();
    order.iter().map(|i| r.text(*i)).collect()
}

#[test]
fn an_item_offers_its_verbs_after_what_plumbing_offers() {
    // Catches the verbs missing from the right-click, so a learner had
    // to recall each, or put before the offers that are about the text.
    let mut r = abc("verbs-plain");
    let offers = menu_on(&mut r, "Birch");
    let verbs = [
        "link it to an item…",
        "move it after an item…  (Ctrl+X, Ctrl+V)",
        "copy it after itself  (Ctrl+C, Ctrl+V)",
        "a new item after it  (Enter)",
        "show it as a chart",
        "show another item's text after it…",
        "delete it",
    ];
    assert!(offers.ends_with(&verbs.map(String::from)), "{offers:#?}");
    let at = |w: &str| offers.iter().position(|o| o == w);
    assert!(
        at("find Birch") < at(verbs[0]),
        "plumbing's first: {offers:#?}"
    );
}

#[test]
fn a_verb_with_a_default_names_it_and_runs_with_it() {
    // Catches an offer that runs with a default its words do not name.
    let mut r = abc("verbs-default");
    r.command("go 1");
    let offers = menu_on(&mut r, "Alder");
    let moving = "move it after 3, the item you came from  (Ctrl+X, Ctrl+V)";
    assert!(offers.contains(&moving.to_string()), "{offers:#?}");
    choose(&mut r, "move it after 3");
    assert_eq!(texts(&r), ["Birch", "Cedar", "Alder"]);
}

#[test]
fn a_verb_with_no_default_asks_for_its_noun() {
    let mut r = abc("verbs-ask");
    menu_on(&mut r, "Birch");
    choose(&mut r, "show another item's text");
    let asks = matches!(&r.app.session.mode, Mode::Quasi(a) if a.input == "transclude ");
    assert!(asks, "{:?}", r.app.session.mode);
}

#[test]
fn only_the_verbs_that_apply_to_the_item_now_are_offered() {
    // Catches a verb offered where it does nothing (show on a view) or
    // named for less than it does (delete of a branch).
    let mut r = abc("verbs-apply");
    r.command("go 2");
    r.command("indent");
    r.command("go 1");
    r.command("show");
    let offers = menu_on(&mut r, "Alder");
    assert!(
        offers.contains(&"delete it and its branch".to_string()),
        "{offers:#?}"
    );
    assert!(!offers.contains(&"show it as a chart".to_string()));
    let said = |o: &&String| o.starts_with("retract its last entry");
    assert!(
        offers.iter().any(|o| said(&o) && o.ends_with("(F4)")),
        "{offers:#?}"
    );
}

#[test]
fn what_an_item_states_and_carries_adds_the_verbs_for_them() {
    // Catches suppose, find, check and adopt fix not offered where they
    // apply.
    let mut r = Room::new("verbs-states");
    r.items(&[
        "The date of Buran-flight is 1988.",
        "Notes #draft [[Nowhere]]",
    ]);
    r.command("fact doi(paper, \"10.1000/xyz\")");
    r.app.idle();
    let stated = menu_on(&mut r, "The date");
    assert!(
        stated
            .iter()
            .any(|o| o.starts_with("suppose what it states")),
        "{stated:#?}"
    );
    r.tap(&[Key::Escape]);
    let noted = menu_on(&mut r, "Notes");
    let wanted = [
        "make the items its [[links]] name: Nowhere",
        "check its source",
        "find the items labelled #draft",
    ];
    for w in wanted {
        assert!(noted.contains(&w.to_string()), "{w}: {noted:#?}");
    }
    assert!(
        !noted.iter().any(|o| o.starts_with("suppose")),
        "{noted:#?}"
    );
}

#[test]
fn an_adoption_taken_back_is_offered_once() {
    // Catches retract offered twice for one entry: as the adoption taken
    // back, and as the item's last entry.
    let mut r = Room::new("verbs-adopted");
    r.items(&["Talk-notes says that the date of Buran-flight is 1989."]);
    r.app.idle();
    r.command("adopt");
    let offers = menu_on(&mut r, "Talk-notes");
    assert!(
        offers.contains(&"take back its adoption".to_string()),
        "{offers:#?}"
    );
    assert!(
        !offers
            .iter()
            .any(|o| o.starts_with("retract its last entry"))
    );
}

#[test]
fn a_link_offers_the_verbs_of_the_item_at_its_other_end() {
    // Catches the verbs offered for a link while the cursor stays on the
    // item it is from, so delete deleted the wrong item.
    let mut r = Room::new("verbs-link");
    r.items(&["Alder", "Birch", "See [[Birch]]"]);
    r.app.idle();
    let rows = r.lines_at(160, 40);
    let beside = |row: &String| row.rfind("2. Birch  1").filter(|i| *i > 60);
    let (y, i) = rows
        .iter()
        .enumerate()
        .find_map(|(y, row)| Some((y, beside(row)?)))
        .unwrap();
    let x = rows[y][..i].chars().count() as u16;
    let hits = r.hits();
    r.app.right_click(&hits, (x, y as u16), "Birch", false);
    assert_eq!(
        r.text(r.app.cursor()),
        "Birch",
        "the cursor goes to the far end"
    );
    choose(&mut r, "delete it");
    assert_eq!(texts(&r), ["Alder", "See [[Birch]]"]);
}

/// The menu open, if one is.
fn menu(r: &Room) -> Option<&memex::session::Menu> {
    match &r.app.session.mode {
        Mode::Menu(m) => Some(m),
        _ => None,
    }
}

#[test]
fn the_menu_key_opens_the_right_clicks_menu_at_the_cursors_item() {
    // Catches the verbs out of the keyboard's reach, or the menu drawn
    // away from the item it acts on.
    let mut r = abc("verbs-menu-key");
    r.command("go 2");
    r.tap(&[Key::Special(Special::Menu)]);
    let m = menu(&r).expect("a menu");
    assert_eq!((m.text.as_str(), m.at), ("Birch", None));
    assert!(
        m.offers.iter().any(|o| o.label == "delete it"),
        "{:?}",
        m.offers
    );
    let rows = r.lines_at(100, 30);
    let row = |t: &str| rows.iter().position(|l| l.contains(t)).unwrap();
    assert_eq!(row("act on “Birch”"), row("• 2. Birch") + 2, "{rows:#?}");
}

#[test]
fn the_menu_key_plumbs_the_word_at_the_text_cursor() {
    let mut r = Room::new("verbs-menu-word");
    r.items(&["Undo #5"]);
    r.tap(&[Key::Special(Special::Menu)]);
    let lines: Vec<&str> = menu(&r)
        .unwrap()
        .offers
        .iter()
        .map(|o| o.line.as_str())
        .collect();
    assert_eq!(lines[0], "retract entry 5", "{lines:?}");
}

#[test]
fn the_menu_key_with_no_item_or_bound_does_its_other_job() {
    let mut r = Room::new("verbs-menu-none");
    r.tap(&[Key::Special(Special::Menu)]);
    assert_eq!(r.app.session.message, "there is no item to act on");
    let toml = "[keys]\nMenu = \"new\"\n";
    r.app.session.keys = memex::config::Config::parse(toml).unwrap().keys();
    r.tap(&[Key::Special(Special::Menu)]);
    assert_eq!(
        r.outline().in_order().len(),
        1,
        "the binding ran: a new item"
    );
}
