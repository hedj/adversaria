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
    pairs(r)
}

/// Each item, by its parent's text and its own.
fn pairs(r: &Room) -> Vec<(String, String)> {
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
        ("Sub", "Old text."),
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
fn a_kept_block_is_moved_out_of_a_dropped_one_before_it_goes() {
    // Catches a kept block deleted with the block it was under.
    let mut r = Room::new("rev-keeps");
    let after = revised(&mut r, "# A\n\nKept text.\n", "Kept text.\n");
    assert_eq!((after, deletes(&r)), (items(&[("", "Kept text.")]), 1));
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
    assert_eq!(r.body_of(r.last("import"))["revises"], first);
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

#[test]
fn a_new_block_two_levels_down_goes_under_the_block_before_it() {
    // As an import of the same file puts it: under the nearest block
    // shallower than it, not first in the outline.
    let mut r = Room::new("rev-jump");
    let after = revised(
        &mut r,
        "# T\n\nText one.\n",
        "# T\n\n### Deep\n\nText one.\n",
    );
    assert_eq!(
        after,
        items(&[("", "T"), ("T", "Deep"), ("Deep", "Text one.")])
    );
}

#[test]
fn a_revision_leaves_the_items_of_other_documents() {
    let mut r = Room::new("rev-others");
    import(&mut r, "other.md", "# Other\n\nOther text.\n");
    let after = revised(&mut r, "# T\n\nOld text.\n", "# T\n\nNew text.\n");
    let expected = items(&[
        ("", "Other"),
        ("Other", "Other text."),
        ("", "T"),
        ("T", "New text."),
    ]);
    assert_eq!(after, expected);
}

/// Export, with these words after the file: what the memex says, and what
/// the file holds.
fn export(r: &mut Room, words: &str) -> (String, String) {
    let file = r.dir.join("out.md");
    r.command(&format!("export {} {words}", file.display()));
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    (r.app.session.message.clone(), text)
}

#[test]
fn an_item_of_the_users_with_children_exports_as_a_heading() {
    let mut r = Room::new("export-heading");
    r.items(&["Top", "Child"]);
    r.tap(&[memex::app::Key::Tab]);
    let (_, text) = export(&mut r, "");
    assert!(
        text.contains("# Top\n") && text.contains("\nChild\n"),
        "{text}"
    );
}

#[test]
fn a_section_exports_alone_from_its_own_depth() {
    let mut r = Room::new("export-section");
    r.items(&["A", "B", "Below B"]);
    r.tap(&[memex::app::Key::Tab]);
    r.tap(&[memex::app::Key::Up, memex::app::Key::Tab]);
    let (_, text) = export(&mut r, "1.1");
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines.contains(&"# B") && !lines.contains(&"# A"), "{text}");
}

#[test]
fn an_export_cites_the_entries_its_text_came_from() {
    let mut r = Room::new("export-sources");
    r.items(&["A"]);
    let (said, _) = export(&mut r, "");
    assert_eq!(said, "exported 1 items to out.md, citing 2 source entries");
}

/// Whether a line of the screen is this, spaces at its end aside.
fn shows(r: &Room, line: &str) -> bool {
    r.lines().iter().any(|l| l.trim_end() == line)
}

#[test]
fn a_revision_is_shown_change_by_change() {
    let mut r = Room::new("rev-screen");
    revised(&mut r, "# T\n\nOld text.\n", "# T\n\nNew text.\n");
    let change = format!(
        "#{}  changed 1.1: “Old text.” → “New text.”",
        r.last("edit")
    );
    assert!(
        shows(&r, "WORLD · REVISION OF D.MD") && shows(&r, &change),
        "{:?}",
        r.lines()
    );
}

#[test]
fn a_revision_without_changes_says_so() {
    let mut r = Room::new("rev-screen-none");
    revised(&mut r, "# T\n", "# T\n");
    assert!(shows(&r, "no changes"), "{:?}", r.lines());
}

#[test]
fn a_retracted_change_is_shown_retracted() {
    let mut r = Room::new("rev-screen-retracted");
    revised(&mut r, "# T\n\nOld text.\n", "# T\n\nNew text.\n");
    let change = r.last("edit");
    r.command(&format!("retract {change}"));
    r.tap(&[memex::app::Key::Enter]);
    let line = format!("#{change}  changed 1.1: “Old text.” → “New text.”  (retracted)");
    assert!(shows(&r, &line), "{:?}", r.lines());
}

#[test]
fn the_top_bar_keeps_its_left_whole() {
    // Catches the right side of the bar drawn over the left when both do
    // not fit: here a revision's world, on 80 columns.
    let mut r = Room::new("rev-top");
    revised(&mut r, "# T\n", "# T\n");
    let rows = r.lines();
    let top = rows[..3]
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(top.contains("suppose: revision of d.md"), "{top}");
}

#[test]
fn an_export_of_a_section_that_is_not_there_says_so() {
    // Catches the item's number read as part of the file's name, which
    // was then refused for its format.
    let mut r = Room::new("export-no-section");
    r.items(&["Alpha"]);
    let out = r.dir.join("out.md");
    r.command(&format!("export {} 99", out.display()));
    assert_eq!(r.app.session.message, "there is no item 99");
    assert!(!out.exists());
}

#[test]
fn a_heading_whose_level_changed_moves_as_a_fresh_import_places_it() {
    // Catches a kept block left under its old parent: the revision showed
    // no change, and after commit the outline disagreed with the document.
    let first = "# T\n\n## Part\n\nUnder the part.\n\n## Second\n\nUnder second.\n";
    let second = first.replace("## Second", "### Second");
    let mut r = Room::new("rev-level");
    let after = revised(&mut r, first, &second);
    let mut fresh = Room::new("rev-level-fresh");
    import(&mut fresh, "d.md", &second);
    assert_eq!(after, pairs(&fresh));
    assert!(
        r.shows("moved 1.2: “Second” under “Part”"),
        "the move is a change"
    );
}

#[test]
fn a_dropped_block_holding_the_users_own_item_stays_with_it() {
    // Catches the user's own note deleted with the block the new version
    // drops, unnamed in the change that deletes the block.
    let mut r = Room::new("rev-own");
    import(&mut r, "d.md", "# T\n\n## Gone\n\nGone text.\n");
    let gone = r.app.notebook.outline.in_order()[2];
    r.put_cursor(gone);
    r.items(&["", "My own note."]);
    let after = revised_again(&mut r, "# T\n");
    let kept = items(&[("", "T"), ("T", "Gone"), ("Gone", "My own note.")]);
    assert_eq!(after, kept);
}

#[test]
fn a_file_renamed_is_imported_as_a_revision_of_the_item_named() {
    // Catches a co-author's copy saved under another name arriving as a
    // second document, with no changes shown: the third review's eighth
    // finding.
    let mut r = Room::new("doc-as");
    import(&mut r, "essay.md", "# Essay\n\nOne.\n");
    let file = r.dir.join("essay-v2.md");
    std::fs::write(&file, "# Essay\n\nOne, revised.\n").unwrap();
    r.command(&format!("import document {} as 1", file.display()));
    assert!(
        r.app.session.message.starts_with("revision of essay.md:"),
        "{}",
        r.app.session.message
    );
    assert_eq!(r.app.reason.world(), "revision of essay.md");
    assert_eq!(pairs(&r)[1], ("Essay".into(), "One, revised.".into()));
}

#[test]
fn a_later_revision_under_the_new_name_finds_the_same_document() {
    // Catches the renamed import recorded under its own name, so that the
    // next import of essay.md, or of essay-v2.md, finds only half its
    // history.
    let mut r = Room::new("doc-as-again");
    import(&mut r, "essay.md", "# Essay\n\nOne.\n");
    let file = r.dir.join("essay-v2.md");
    std::fs::write(&file, "# Essay\n\nOne.\n\nTwo.\n").unwrap();
    r.command(&format!("import document {} as 1", file.display()));
    r.command("commit");
    let said = import(&mut r, "essay.md", "# Essay\n\nOne.\n\nTwo.\n\nThree.\n");
    assert!(said.starts_with("revision of essay.md:"), "{said}");
    let texts: Vec<String> = pairs(&r).into_iter().map(|(_, t)| t).collect();
    assert_eq!(texts, ["Essay", "One.", "Two.", "Three."], "Two. is known");
}

#[test]
fn an_item_not_imported_is_no_document_to_revise() {
    let mut r = Room::new("doc-as-not");
    r.items(&["Notes"]);
    let file = r.dir.join("essay.md");
    std::fs::write(&file, "# Essay\n").unwrap();
    r.command(&format!("import document {} as 1", file.display()));
    assert_eq!(
        r.app.session.message,
        "1 did not come from an imported document"
    );
}
