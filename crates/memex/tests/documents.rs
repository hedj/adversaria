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
