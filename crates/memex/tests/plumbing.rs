//! Plumbing: a right-click on a text opens a menu of the commands its
//! rules offer (docs/guide.md § 2.3, The mouse; crate::plumb).

mod support;
use memex::app::{Click, Key, Mode};
use memex::config::Config;
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
    assert_eq!(offered(&r), ["go 2", "link to 2"]);
    assert!(r.shows("act on"), "{:#?}", r.lines());
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
        matches!(&r.app.session.mode, Mode::Menu(m) if m.chosen == 1),
        "round from the top"
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
    let go = Click::Command("go 2".into());
    let hits = r.hits();
    assert!(
        hits.0.iter().position(|(_, c)| *c == go) == Some(0),
        "the menu is clicked first"
    );
    r.app.click(&go);
    assert_eq!(r.here(), "Energia launch");
    r.app.plumb("2", (4, 3));
    r.app.click(&Click::Command("help".into()));
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
    r.app.plumb("#2", (0, 0));
    assert_eq!(
        offered(&r)[..2],
        ["go #2", "retract 2"],
        "#2 is an address too"
    );
    r.tap(&[Key::Char('2')]);
    assert!(matches!(
        r.app.session.mode,
        Mode::ConfirmRetract { target: 2, .. }
    ));
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
    assert_eq!(offered(&r), ["query wd('Q2548', P, V)", "go Q2548"]);
}
