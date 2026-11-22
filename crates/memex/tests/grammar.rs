//! The grammar (docs/concepts-next.md § The grammar): a verb typed alone
//! takes its nouns' defaults; for a noun with none now, the memex prompts,
//! Command latched with the verb typed, and says why there is none.

mod support;

use memex::app::{Key, Mode, Quasi};
use support::Room;

/// Three items, A, B and C, the cursor on C.
fn abc(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&["Alder", "Birch", "Cedar"]);
    r
}

/// The texts of the items, in order.
fn texts(r: &Room) -> Vec<String> {
    let order = r.outline().in_order();
    order.iter().map(|i| r.text(*i)).collect()
}

/// The Command line typed, if a prompt is open.
fn asked(r: &Room) -> Option<String> {
    match &r.app.session.mode {
        Mode::Quasi(a) if a.latched() => Some(a.input.clone()),
        _ => None,
    }
}

/// The foot's first row, as a held Command line shows it.
fn foot(r: &Room) -> String {
    let lines = r.lines_at(140, 12);
    lines[10].trim_end().to_string()
}

#[test]
fn move_alone_moves_after_the_item_last_jumped_from() {
    // Catches the default not taken, so move alone refused as before.
    let mut r = abc("grammar-move");
    r.command("go 1");
    r.command("move");
    assert_eq!(texts(&r), ["Birch", "Cedar", "Alder"]);
    assert_eq!(asked(&r), None);
}

#[test]
fn move_alone_with_no_jump_asks_and_says_why() {
    // Catches a refusal where the rule says to prompt, or a prompt that
    // does not say why there is no default.
    let mut r = abc("grammar-move-ask");
    r.command("move");
    assert_eq!(asked(&r).as_deref(), Some("move "));
    let said = &r.app.session.message;
    assert!(
        said.starts_with("no jump came from another item: move (after) ‹item›"),
        "{said}"
    );
    r.typ("1");
    r.tap(&[Key::Enter]);
    assert_eq!(texts(&r), ["Alder", "Cedar", "Birch"]);
    assert_eq!(asked(&r), None);
}

#[test]
fn enter_at_a_prompt_with_nothing_given_asks_again_and_esc_leaves() {
    // Catches Enter running the verb alone into a refusal, or Esc leaving
    // something written.
    let mut r = abc("grammar-import-ask");
    let before = r.app.journal().entries().len();
    r.command("import");
    assert_eq!(asked(&r).as_deref(), Some("import "));
    r.tap(&[Key::Enter]);
    assert_eq!(asked(&r).as_deref(), Some("import "), "it asks again");
    r.tap(&[Key::Escape]);
    assert_eq!(r.app.session.mode, Mode::Normal);
    assert_eq!(r.app.journal().entries().len(), before);
}

#[test]
fn link_alone_links_to_the_item_last_jumped_from_as_a_link() {
    let mut r = abc("grammar-link");
    let order = r.outline().in_order();
    r.command("go 1");
    r.command("link");
    let (out, _) = r.outline().links_of(order[0]);
    let ends: Vec<(u64, String)> = out.iter().map(|l| (l.to, l.kind.clone())).collect();
    assert_eq!(ends, [(order[2], "link".to_string())]);
}

#[test]
fn copy_alone_copies_after_itself_and_go_alone_goes_back() {
    let mut r = abc("grammar-copy-go");
    r.command("go 1");
    r.command("copy");
    assert_eq!(texts(&r), ["Alder", "Alder", "Birch", "Cedar"]);
    r.command("go");
    assert_eq!(r.text(r.app.cursor()), "Cedar", "back where go 1 came from");
}

#[test]
fn show_alone_shows_the_target_as_a_chart() {
    let mut r = abc("grammar-show");
    let view = r.app.cursor();
    r.command("show");
    assert_eq!(r.outline().views[&view].len(), 1);
    assert_eq!(r.app.session.message, "shown as a chart");
}

#[test]
fn a_verb_held_alone_shows_its_nouns_kinds_and_defaults() {
    // Catches the prompt not shown before the release runs it, so the
    // default could not be seen before it was taken.
    let mut r = abc("grammar-foot");
    r.command("go 1");
    r.hold(Quasi::Command, "move");
    assert_eq!(foot(&r), " COMMAND  move (after) ‹item› [3]");
    r.tap(&[Key::Escape]);
    r.hold(Quasi::Command, "import");
    assert_eq!(foot(&r), " COMMAND  import (from) ‹text›");
}

#[test]
fn a_bound_key_takes_the_defaults_too() {
    // Catches defaults taken by a typed line only: F7 is `go back`, and a
    // key bound to `move` alone moves after the item jumped from.
    let toml = "[keys]\nInsert = \"move\"\n";
    let mut r = abc("grammar-bound");
    r.app.session.keys = memex::config::Config::parse(toml).unwrap().keys();
    r.command("go 1");
    r.tap(&[Key::Special(memex::app::Special::Insert)]);
    assert_eq!(texts(&r), ["Birch", "Cedar", "Alder"]);
}

#[test]
fn suppose_alone_supposes_what_the_target_states_or_asks() {
    // Catches the target's statement not taken, or taken from an item the
    // memex reads nothing in.
    let mut r = Room::new("grammar-suppose");
    r.items(&["Alder", "The date of Buran-flight is 1988."]);
    r.app.idle();
    r.command("suppose");
    assert_eq!(asked(&r), None, "{}", r.app.session.message);
    r.tap(&[Key::Escape]);
    r.command("go 1");
    r.command("suppose");
    assert_eq!(asked(&r).as_deref(), Some("suppose "));
    let said = &r.app.session.message;
    assert!(
        said.starts_with("the target states nothing the memex reads"),
        "{said}"
    );
}

#[test]
fn check_alone_checks_the_one_kind_the_target_has_or_asks() {
    // Catches the kind not inferred from what the target states, or one
    // guessed where the target has neither.
    let mut r = abc("grammar-check");
    r.command("fact doi(paper, \"10.1000/xyz\")");
    r.hold(Quasi::Command, "check");
    assert_eq!(foot(&r), " COMMAND  check (which) ‹word› [source]");
    r.tap(&[Key::Escape]);
    r.command("go 1");
    r.command("check");
    assert_eq!(asked(&r).as_deref(), Some("check "));
    let said = &r.app.session.message;
    assert!(
        said.starts_with("the target has both a quote and a source"),
        "{said}"
    );
}

#[test]
fn the_item_jumped_from_is_no_default_for_itself() {
    // Catches the item jumped from taken when the cursor is back on it by
    // an arrow, which is no jump, so move alone refused to move an item
    // into its own branch.
    let mut r = abc("grammar-self");
    r.command("go 2");
    r.tap(&[Key::Down]);
    assert_eq!(r.app.session.trail.back, [r.app.cursor()]);
    r.command("move");
    assert_eq!(asked(&r).as_deref(), Some("move "));
}

#[test]
fn a_verb_with_no_target_refuses_before_it_asks() {
    // Catches a prompt for where to move to with nothing to move.
    let mut r = Room::new("grammar-no-target");
    r.command("move");
    assert_eq!(asked(&r), None);
    assert_eq!(r.app.session.message, "there is no item to act on");
}
