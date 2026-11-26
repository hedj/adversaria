//! Editing the outline: typing and erasing in an item, folding and
//! linking, and where the cursors go when the outline changes (docs/plan.md,
//! Milestone 3).

mod support;

use format::record::EntryKind;
use memex::app::Key;
use support::Room;

#[test]
fn backspace_erases_before_the_text_cursor_and_moves_it_back() {
    let mut r = Room::new("backspace");
    r.items(&["abc"]);
    r.tap(&[Key::Left, Key::Left, Key::Backspace]);
    r.typ("X");
    assert_eq!(r.here(), "Xbc");
}

#[test]
fn delete_erases_after_the_text_cursor() {
    let mut r = Room::new("delete");
    r.items(&["abc"]);
    r.tap(&[Key::Left, Key::Left, Key::Delete]);
    assert_eq!(r.here(), "ac");
}

#[test]
fn text_typed_into_one_item_is_shown_in_that_item_alone() {
    let mut r = Room::new("typed");
    r.items(&["one", "two"]);
    r.typ("!");
    let first = r.app.visible()[0].0;
    assert_eq!((r.text(first), r.here()), ("one".into(), "two!".into()));
}

#[test]
fn a_moved_cursor_waits_to_be_written() {
    let mut r = Room::new("moved");
    r.items(&["one", "two"]);
    r.tap(&[Key::Up]);
    assert!(r.app.pending(), "the move is not yet written");
    r.app.flush();
    assert!(!r.app.pending());
}

#[test]
fn moving_to_an_item_puts_the_text_cursor_at_its_end() {
    let mut r = Room::new("end");
    r.items(&["one", "two"]);
    r.tap(&[Key::Home, Key::Up]);
    r.typ("!");
    assert_eq!(r.here(), "one!");
}

#[test]
fn deleting_the_item_at_the_cursor_moves_it_to_the_item_before() {
    let mut r = Room::new("before");
    r.items(&["one", "two", "three"]);
    r.command("delete");
    assert_eq!(r.here(), "two");
}

#[test]
fn deleting_the_first_item_moves_the_cursor_to_the_next() {
    let mut r = Room::new("first");
    r.items(&["one", "two"]);
    r.tap(&[Key::Up]);
    r.command("delete");
    assert_eq!(r.here(), "two");
}

#[test]
fn a_deleted_item_leaves_the_folds() {
    let mut r = Room::new("folds");
    r.items(&["one", "two"]);
    let two = r.app.cursor();
    r.command("fold");
    r.command("delete");
    assert!(!r.view().folded.contains(&two));
}

#[test]
fn the_text_cursor_keeps_within_the_item_it_moves_to() {
    let mut r = Room::new("pos");
    r.items(&["a", "bcdef"]);
    r.command("delete");
    assert_eq!(r.view().pos, 1);
}

#[test]
fn a_write_that_fails_is_shown() {
    let mut r = Room::new("fails");
    r.typ("one");
    std::fs::remove_dir_all(&r.dir).unwrap();
    r.app.flush();
    assert!(
        r.app.session.message.contains("os error"),
        "{}",
        r.app.session.message
    );
}

#[test]
fn a_folded_item_hides_its_branch() {
    let mut r = Room::new("fold");
    r.items(&["one", "two"]);
    r.tap(&[Key::Tab, Key::Up]);
    r.command("fold");
    assert_eq!(r.app.visible().len(), 1);
}

#[test]
fn a_link_says_what_it_joined() {
    let mut r = Room::new("link");
    r.items(&["claim", "source"]);
    r.command("link 1 2 as cites");
    assert_eq!(r.app.session.message, "linked 1 to 2 as cites");
}

#[test]
fn a_citation_is_shown_after_the_text_that_cites() {
    let mut r = Room::new("cites");
    r.items(&["claim", "source"]);
    r.command("link 1 2 as cites");
    assert!(r.lines().iter().any(|l| l.contains("claim [source]")));
}

#[test]
fn an_edit_keeps_the_entries_of_the_text_around_it() {
    let mut r = Room::new("runs");
    r.items(&["abc"]);
    r.tap(&[Key::Home, Key::Right]);
    r.typ("X");
    r.app.flush();
    let [(a, 1), (x, 1), (bc, 2)] = r.spans()[..] else {
        panic!("{:?}", r.spans())
    };
    assert!(a == bc && x != a, "{:?}", r.spans());
}

#[test]
fn an_erased_character_adds_no_span() {
    let mut r = Room::new("erased");
    r.items(&["abc"]);
    r.tap(&[Key::Left, Key::Backspace]);
    r.app.flush();
    let lengths: Vec<usize> = r.spans().iter().map(|s| s.1).collect();
    assert_eq!(lengths, [1, 1]);
}

#[test]
fn a_character_repeated_at_the_end_is_added() {
    let mut r = Room::new("repeated");
    r.items(&["aa"]);
    r.typ("a");
    r.app.flush();
    let lengths: Vec<usize> = r.spans().iter().map(|s| s.1).collect();
    assert_eq!(lengths, [2, 1]);
}

#[test]
fn text_typed_and_erased_writes_no_edit() {
    let mut r = Room::new("unchanged");
    r.items(&["abc"]);
    let edits = r.count(EntryKind::Edit);
    r.typ("x");
    r.tap(&[Key::Backspace]);
    r.app.flush();
    assert_eq!(r.count(EntryKind::Edit), edits);
}

#[test]
fn a_command_that_works_leaves_no_earlier_refusal_on_the_message_line() {
    let mut r = Room::new("stale-message");
    r.items(&["Alpha", "Beta"]);
    r.refused("outdent");
    assert_eq!(
        r.app.session.message,
        "a top-level item cannot be outdented"
    );
    r.command("copy after 1");
    assert_eq!(r.app.session.message, "", "the copy worked");
}

#[test]
fn a_name_that_reads_as_an_item_or_an_entry_number_is_refused() {
    // Catches a name that no address can reach: a number, or # and a
    // number, is read as an item's number or an entry's first.
    let mut r = Room::new("names-unreachable");
    r.items(&["Alpha"]);
    for name in ["1", "1.2", "#2"] {
        r.refused(&format!("name {name}"));
        assert!(r.app.session.message.starts_with("name not kept"), "{name}");
    }
    assert!(r.app.notebook.outline.names.is_empty());
}

#[test]
fn delete_word_deletes_the_word_at_the_text_cursor() {
    // Catches the word delete lost with the Scope quasimode, which was
    // its only way: `delete word` does it now.
    let mut r = Room::new("delete-word");
    r.items(&["one two three"]);
    r.tap(&[
        Key::Left,
        Key::Left,
        Key::Left,
        Key::Left,
        Key::Left,
        Key::Left,
        Key::Left,
    ]);
    r.command("delete word");
    assert_eq!(r.here(), "one three");
    assert_eq!(r.outline().in_order().len(), 1, "the item stays");
}

#[test]
fn delete_without_a_word_deletes_the_item_and_its_branch() {
    let mut r = Room::new("delete-item");
    r.items(&["Alpha", "Beta"]);
    r.tap(&[Key::Tab]);
    r.tap(&[Key::Up]);
    r.command("delete");
    assert!(r.outline().in_order().is_empty(), "{:?}", r.lines());
}

#[test]
fn delete_with_another_word_says_what_it_takes() {
    let mut r = Room::new("delete-other");
    r.items(&["Alpha"]);
    r.tap(&[memex::app::Key::Quasi(memex::app::Quasi::Command)]);
    r.typ("delete it");
    r.tap(&[Key::Enter]);
    assert_eq!(r.app.session.message, "delete, or delete word");
    assert_eq!(r.outline().in_order().len(), 1);
}

#[test]
fn an_old_journals_word_scope_is_ignored() {
    // Catches a journal written before Scope was removed (its view entries
    // record "scope", and an op "scope") refused, or read so that a plain
    // `delete` still deletes only a word.
    let dir = std::env::temp_dir().join(format!("adversaria-old-scope-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut j = memex::journal::Journal::open(&dir).unwrap();
    j.append(EntryKind::Edit, serde_json::json!({"op": "init"}))
        .unwrap();
    let made = j.append(EntryKind::Edit, serde_json::json!({"op": "create", "under": 0, "after": null, "add": "one two", "spans": [[0, 0, 7]]})).unwrap();
    let panes = serde_json::json!([{"kind": "outline", "cursor": made.seq}, {"kind": "links", "cursor": 0}]);
    j.append(EntryKind::View, serde_json::json!({"op": "scope", "panes": panes, "focus": 0, "pos": 2, "folded": [], "scope": "word"})).unwrap();
    drop(j);
    let mut app = memex::app::App::open(&dir).unwrap();
    assert_eq!(app.cursor(), made.seq);
    app.run_command("delete");
    assert!(
        app.notebook.outline.in_order().is_empty(),
        "the item, not a word"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn fold_hides_a_branch_and_fold_again_shows_it() {
    // Catches two commands for one switch: fold hid and unfold showed,
    // where an outliner has one toggle.
    let mut r = Room::new("fold-toggle");
    r.items(&["Alpha", "Beta"]);
    r.tap(&[Key::Tab, Key::Up]);
    r.command("fold");
    assert!(!r.shows("Beta"), "hidden: {:?}", r.lines());
    r.command("fold");
    assert!(r.shows("Beta"), "shown again: {:?}", r.lines());
}
