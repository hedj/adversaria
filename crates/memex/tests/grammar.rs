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

/// Shift with a key, some times.
fn shift(r: &mut Room, s: memex::app::Special, times: usize) {
    (0..times).for_each(|_| r.tap(&[Key::Special(s)]));
}

#[test]
fn delete_alone_deletes_the_items_selected_each_with_its_branch() {
    // Catches delete taking the cursor's item alone, or deleting an item
    // twice, once with its parent's branch and once of its own.
    let mut r = abc("grammar-delete-items");
    r.command("go 2");
    r.command("indent");
    r.command("go 1");
    shift(&mut r, memex::app::Special::ShiftDown, 2);
    r.command("delete");
    assert_eq!(texts(&r), [] as [&str; 0]);
    let said = &r.app.session.message;
    assert!(
        said.starts_with("deleted 2 items with their branches"),
        "{said}"
    );
}

#[test]
fn delete_alone_erases_characters_selected_and_refuses_text_dragged() {
    let mut r = abc("grammar-delete-text");
    shift(&mut r, memex::app::Special::ShiftLeft, 3);
    r.command("delete");
    assert_eq!(texts(&r), ["Alder", "Birch", "Ce"]);
    r.app.select_text("Birch");
    r.command("delete");
    assert_eq!(texts(&r), ["Alder", "Birch", "Ce"], "nothing deleted");
    let said = &r.app.session.message;
    assert_eq!(
        said,
        "delete takes items, or text selected with Shift in one item"
    );
}

#[test]
fn find_alone_takes_the_characters_selected() {
    // Catches the selection not taken for a text noun, so find alone
    // showed the links.
    let mut r = abc("grammar-find-noun");
    r.command("go 2");
    select_first(&mut r, 3);
    r.command("find");
    assert_eq!(found(&r).as_deref(), Some("Bir"));
}

/// Select the first characters of the cursor's item, with Shift.
fn select_first(r: &mut Room, n: usize) {
    r.tap(&[Key::Home]);
    shift(r, memex::app::Special::ShiftRight, n);
}

/// The words of what the list beside the outline lists, if find listed it.
fn found(r: &Room) -> Option<String> {
    r.app.session.found.as_ref().map(|f| f.words())
}

#[test]
fn go_alone_takes_the_text_dragged() {
    // Catches the text dragged not taken for go's place, so go alone went
    // back along the trail.
    let mut r = abc("grammar-go-noun");
    r.app.select_text("Alder");
    r.command("go");
    assert_eq!(r.text(r.app.cursor()), "Alder");
}

#[test]
fn a_verb_on_one_item_refuses_several_selected() {
    // Catches move taking the cursor's item from among those selected,
    // so the others stayed where they were.
    let mut r = abc("grammar-several");
    shift(&mut r, memex::app::Special::ShiftUp, 1);
    r.command("move after 1");
    assert_eq!(texts(&r), ["Alder", "Birch", "Cedar"]);
    let said = &r.app.session.message;
    assert_eq!(
        said,
        "move acts on one item, and 2 are selected; delete takes them all"
    );
}

#[test]
fn text_selected_fills_no_noun_after_the_target() {
    // Catches the text selected taken for show's (as), which follows its
    // first noun, the target: the selection fills a first noun alone.
    let mut r = abc("grammar-text-target");
    shift(&mut r, memex::app::Special::ShiftLeft, 3);
    r.command("show");
    assert_eq!(r.app.session.message, "shown as a chart");
}

/// What importing a file of this name and text, named alone, says.
fn import_alone(r: &mut Room, name: &str, text: &str) -> String {
    let file = r.dir.join(name);
    std::fs::write(&file, text).unwrap();
    r.command(&format!("import {}", file.display()));
    r.app.session.message.clone()
}

#[test]
fn import_reads_a_file_as_the_kind_its_name_ends_in() {
    // Catches the kind still needed as a word of its own, or a file read
    // as another kind than its ending says.
    let mut r = Room::new("grammar-import-kinds");
    let said = import_alone(&mut r, "a.md", "# A\n\nText.\n");
    assert_eq!(said, "imported a.md: 2 items; losses: none");
    let said = import_alone(&mut r, "r.json", r#"{"format": "other"}"#);
    assert_eq!(said, "not a results file: its format is not memex-results");
    let said = import_alone(&mut r, "w.pl", "not prolog (");
    assert!(said.contains("is not a Wikidata subset"), "{said}");
}

#[test]
fn import_of_a_file_as_an_address_is_a_revision() {
    // Catches `as ADDRESS` lost when the kind is not named, so the copy
    // arrived as a document of its own.
    let mut r = Room::new("grammar-import-revision");
    import_alone(&mut r, "essay.md", "# Essay\n\nOne.\n");
    let copy = r.dir.join("essay-v2.md");
    std::fs::write(&copy, "# Essay\n\nOne, revised.\n").unwrap();
    r.command(&format!("import {} as 1", copy.display()));
    let said = &r.app.session.message;
    assert!(said.starts_with("revision of essay.md:"), "{said}");
}
