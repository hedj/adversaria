//! Plumbing: a right-click on a text opens a menu of the commands its
//! rules offer (docs/guide.md § 2.3, The mouse and Plumbing; crate::plumb).

mod support;
use memex::app::{Key, Mode};
use memex::config::Config;
use memex::present::Presentation;
use ratatui::style::Color;
use support::Room;

/// Three items, and a right-click on "2".
fn menu_on_two(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Buran flight", "Energia launch", "Notes"]);
    r.app.plumb("2", (4, 3));
    r
}

/// The menu's offers, by their command lines, if it is open.
fn offered(r: &Room) -> Vec<String> {
    match &r.app.session.mode {
        Mode::Menu(m) => m.offers.iter().map(|o| o.line.clone()).collect(),
        _ => vec![],
    }
}

#[test]
fn a_menu_offers_what_matches_and_enter_runs_the_first() {
    // Catches a menu not opened, an offer missing, or Enter not running
    // the one chosen.
    let mut r = menu_on_two("menu-enter");
    assert_eq!(
        offered(&r),
        ["go 2", "link to 2", "transclude from 2", "find 2"]
    );
    assert!(
        r.shows("act on “2”"),
        "the title names the text: {:#?}",
        r.lines()
    );
    assert!(r.shows(" 1 go to 2 "), "{:#?}", r.lines());
    r.tap(&[Key::Enter]);
    assert_eq!(r.app.session.mode, Mode::Normal);
    assert_eq!(r.here(), "Energia launch");
}

#[test]
fn keys_choose_a_number_runs_and_esc_closes() {
    // Catches Down not choosing the next, Up not going round, a number
    // not running its offer, Esc running one, or a stray key closing it.
    let mut r = menu_on_two("menu-keys");
    r.tap(&[Key::Down, Key::Char('x')]);
    assert!(matches!(&r.app.session.mode, Mode::Menu(m) if m.chosen == 1));
    r.tap(&[Key::Up]);
    assert!(matches!(&r.app.session.mode, Mode::Menu(m) if m.chosen == 0));
    r.tap(&[Key::Up]);
    assert!(
        matches!(&r.app.session.mode, Mode::Menu(m) if m.chosen == m.offers.len() - 1),
        "round from the top to the last"
    );
    r.tap(&[Key::Escape]);
    assert_eq!(r.app.session.mode, Mode::Normal);
    assert_eq!(r.here(), "Notes", "Esc ran nothing");
    r.app.plumb("2", (4, 3));
    r.tap(&[Key::Char('2')]);
    assert_eq!(r.count("link"), 1, "the second offer, link to 2, ran");
}

#[test]
fn a_click_on_an_offer_runs_it_and_elsewhere_closes() {
    // Catches the menu's entries not clickable, a click on what is under
    // the menu acting, or a click elsewhere leaving it open.
    let mut r = menu_on_two("menu-click");
    let go = Presentation::Command("go 2".into());
    let hits = r.hits();
    assert!(
        hits.0.iter().position(|(_, c)| *c == go) == Some(0),
        "the menu is clicked first"
    );
    r.app.click(&go);
    assert_eq!(r.here(), "Energia launch");
    r.app.plumb("2", (4, 3));
    r.app.click(&Presentation::Command("help".into()));
    assert_eq!(r.app.session.mode, Mode::Normal);
    assert_eq!(r.app.session.message, "", "the click only closed the menu");
}

#[test]
fn nothing_offered_says_so_and_a_retract_still_asks() {
    // Catches an empty menu opened, or a command that asks first (retract)
    // run without asking when it comes from the menu.
    let mut r = menu_on_two("menu-retract");
    r.tap(&[Key::Escape]);
    r.app.plumb("   ", (0, 0));
    assert_eq!(r.app.session.mode, Mode::Normal);
    assert!(r.app.session.message.starts_with("nothing acts on"));
    // #3 is the first item's entry, after the session's start entry (2).
    r.app.plumb("#3", (0, 0));
    assert_eq!(
        offered(&r)[..2],
        ["go #3", "retract entry 3"],
        "#3 is an address too"
    );
    r.tap(&[Key::Char('2')]);
    assert!(matches!(
        r.app.session.mode,
        Mode::ConfirmRetract { target: 3, .. }
    ));
}

#[test]
fn the_menu_has_a_ground_of_its_own() {
    // Catches the menu drawn on the paper, so that the screen under it
    // shows through and its edge is lost.
    let r = menu_on_two("menu-ground");
    let ground = r.style_of("2 link here to 2").bg;
    assert!(matches!(ground, Some(Color::Rgb(..))), "{ground:?}");
    let paper = r.style_of("notebook").bg;
    assert_ne!(ground, paper, "the menu stands apart from the paper");
    let lines = r.lines();
    let top = lines.iter().find(|l| l.contains("act on “2”")).unwrap();
    assert!(
        top.contains("─ act on “2” ─"),
        "a rule each side of the title: {top}"
    );
    let row = lines.iter().find(|l| l.contains("1 go to 2")).unwrap();
    let inside = row.split('│').nth(1).unwrap_or("");
    assert!(
        inside.ends_with("  "),
        "the entry is filled to the border: {row}"
    );
}

#[test]
fn the_menu_leaves_its_row_in_view() {
    // Catches the menu, or the paper round it, drawn over the row that
    // was right-clicked, so the text it acts on is hidden.
    let mut r = Room::new("menu-row");
    r.items(&["Buran flight", "Energia launch", "Notes"]);
    let (x, y) = r.find("Energia launch");
    let row = r.lines()[y as usize].clone();
    r.app.plumb("Energia", (x, y));
    assert_eq!(r.lines()[y as usize], row, "{:#?}", r.lines());
    assert!(r.shows("act on “Energia”"));
}

/// A room whose configuration has one plumbing rule.
fn with_rule(name: &str, rule: &str) -> Room {
    let plumber = Config::parse(rule).unwrap().plumber().unwrap();
    let mut r = Room::new(name);
    r.app.session.plumber = plumber;
    r
}

#[test]
fn the_users_rules_are_offered_first() {
    // Catches the configuration's rules not given to the session.
    let rule = "[[plumb]]\nname = \"query $0\"\nmatch = 'Q\\d+'\nrun = \"query wd('$0', P, V)\"\n";
    let mut r = with_rule("menu-user", rule);
    r.app.plumb("Q2548", (0, 0));
    assert_eq!(offered(&r)[0], "query wd('Q2548', P, V)");
}

#[test]
fn the_cursors_own_item_named_is_no_place_to_go_or_link() {
    // Catches a right-click on an item's own name offering to go to it,
    // link it to itself, or show it in its own place.
    let mut r = Room::new("plumb-self");
    r.items(&["Buran flight", "Energia launch"]);
    r.app.plumb("2", (4, 3));
    assert_eq!(offered(&r), ["find 2"], "item 2 is the cursor's");
}
