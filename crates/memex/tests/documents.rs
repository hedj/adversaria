//! Importing a document: its blocks become items after the outline, and a
//! second import of the same file is a revision (docs/plan.md, Milestone 6).

mod support;

use support::Room;

/// What importing a document of this name and text says.
fn import(r: &mut Room, name: &str, text: &str) -> String {
    let file = r.dir.join(name);
    std::fs::write(&file, text).unwrap();
    r.command(&format!("import document {}", file.display()));
    r.app.session.message.clone()
}

#[test]
fn a_document_that_loses_nothing_says_so() {
    let mut r = Room::new("doc-none");
    let said = import(&mut r, "a.md", "# A\n\nText.\n");
    assert_eq!(said, "imported a.md: 2 items; losses: none");
}

#[test]
fn another_document_is_no_revision() {
    let mut r = Room::new("doc-other");
    import(&mut r, "a.md", "# A\n");
    let said = import(&mut r, "b.md", "# B\n");
    assert_eq!(
        (said.as_str(), r.app.reason.world()),
        ("imported b.md: 1 items; losses: none", "actual")
    );
}

#[test]
fn a_document_goes_after_the_outline() {
    let mut r = Room::new("doc-after");
    r.items(&["before"]);
    import(&mut r, "a.md", "# A\n");
    let texts: Vec<String> = r.app.visible().iter().map(|(id, _)| r.text(*id)).collect();
    assert_eq!(texts, ["before", "A"]);
}

/// Import a document, then a revision of it: each item after, by its
/// parent's text and its own.
fn revised(r: &mut Room, first: &str, second: &str) -> Vec<(String, String)> {
    import(r, "d.md", first);
    revised_again(r, second)
}

/// Import a revision of d.md: each item after, by its parent's text and
/// its own.
fn revised_again(r: &mut Room, text: &str) -> Vec<(String, String)> {
    import(r, "d.md", text);
    let outline = &r.app.notebook.outline;
    let parent = |id: u64| {
        outline
            .parent(id)
            .map_or(String::new(), |p| outline.text(p))
    };
    outline
        .in_order()
        .into_iter()
        .map(|id| (parent(id), outline.text(id)))
        .collect()
}

/// How many deletions the journal holds.
fn deletes(r: &Room) -> usize {
    let bodies = r.app.journal().entries().iter().filter_map(|e| e.body());
    bodies.filter(|b| b["op"] == "delete").count()
}

/// Items as revised expects them: by their parent's text and their own.
fn items(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(p, t)| (p.to_string(), t.to_string()))
        .collect()
}

#[test]
fn a_block_whose_style_changed_is_a_new_item() {
    let mut r = Room::new("rev-style");
    let after = revised(&mut r, "# T\n\nBody text.\n", "# T\n\n## Body text.\n");
    assert_eq!(
        (after, deletes(&r)),
        (items(&[("", "T"), ("T", "Body text.")]), 1)
    );
}

#[test]
fn a_new_block_goes_under_the_block_it_follows_in_the_document() {
    let mut r = Room::new("rev-nested");
    let after = revised(
        &mut r,
        "# T\n\nOld text.\n",
        "# T\n\n## Sub\n\nNew text.\n\nOld text.\n",
    );
    let expected = items(&[
        ("", "T"),
        ("T", "Sub"),
        ("Sub", "New text."),
        ("T", "Old text."),
    ]);
    assert_eq!(after, expected);
}

#[test]
fn a_new_block_goes_after_the_one_before_it() {
    let mut r = Room::new("rev-after");
    let (first, second) = (
        "# T\n\nFirst text.\n\nLast text.\n",
        "# T\n\nFirst text.\n\nMiddle text.\n\nLast text.\n",
    );
    let expected = items(&[
        ("", "T"),
        ("T", "First text."),
        ("T", "Middle text."),
        ("T", "Last text."),
    ]);
    assert_eq!(revised(&mut r, first, second), expected);
}

#[test]
fn a_dropped_block_that_holds_a_kept_one_stays() {
    let mut r = Room::new("rev-keeps");
    let after = revised(&mut r, "# A\n\nKept text.\n", "Kept text.\n");
    assert_eq!(
        (after, deletes(&r)),
        (items(&[("", "A"), ("A", "Kept text.")]), 0)
    );
}

#[test]
fn a_dropped_branch_is_deleted_once() {
    let mut r = Room::new("rev-gone");
    let after = revised(&mut r, "# A\n\n## B\n\nGone text.\n", "Other text.\n");
    assert_eq!((after, deletes(&r)), (items(&[("", "Other text.")]), 1));
}

#[test]
fn a_revision_names_the_import_it_revises() {
    let mut r = Room::new("rev-revises");
    import(&mut r, "d.md", "# A\n");
    let first = r.last("import");
    import(&mut r, "d.md", "# B\n");
    let revision = r
        .app
        .journal()
        .entries()
        .iter()
        .rev()
        .find(|e| e.kind() == "import");
    assert_eq!(revision.unwrap().body().unwrap()["revises"], first);
}

#[test]
fn a_new_block_goes_after_the_last_of_its_own_section() {
    let mut r = Room::new("rev-section");
    let (first, second) = (
        "# A\n\nA text.\n\n# B\n\nB text.\n",
        "# A\n\nA text.\n\n# B\n\nNew text.\n\nB text.\n",
    );
    let expected = items(&[
        ("", "A"),
        ("A", "A text."),
        ("", "B"),
        ("B", "New text."),
        ("B", "B text."),
    ]);
    assert_eq!(revised(&mut r, first, second), expected);
}

#[test]
fn a_new_block_stays_with_a_document_the_user_moved() {
    let mut r = Room::new("rev-moved");
    r.items(&["holder"]);
    import(&mut r, "d.md", "# A\n\nText.\n");
    r.leap(memex::app::Quasi::LeapForward, "A");
    r.tap(&[memex::app::Key::Tab]);
    let after = revised_again(&mut r, "Intro text.\n\n# A\n\nText.\n");
    assert_eq!(
        after,
        items(&[
            ("", "holder"),
            ("holder", "Intro text."),
            ("holder", "A"),
            ("A", "Text.")
        ])
    );
}
