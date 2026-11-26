//! The go command: to the item an address names, else to the next item
//! that holds the text (docs/guide.md § 2.4).

mod support;
use support::Room;

#[test]
fn go_goes_to_an_address_or_the_next_item_holding_the_text() {
    // Catches an address read as text, a search from the top and not from
    // the cursor, a match missed for its case, or nothing found taken as
    // the cursor's own item.
    let mut r = Room::new("go");
    r.items(&[
        "Buran flight",
        "Energia launch",
        "notes on buran",
        "Buran again",
    ]);
    r.command("go 2");
    assert_eq!(r.here(), "Energia launch");
    r.command("go buran");
    assert_eq!(r.here(), "notes on buran");
    r.command("go buran");
    assert_eq!(r.here(), "Buran again");
    r.refused("go no such words");
    assert_eq!(r.here(), "Buran again");
    assert_eq!(r.app.session.message, "no item is or holds “no such words”");
}

#[test]
fn go_opens_a_folded_branch_to_reach_its_item() {
    // Catches the cursor put on an item hidden by a fold.
    let mut r = Room::new("go-fold");
    r.items(&["Programme"]);
    r.command("new");
    r.typ("hidden under it");
    r.command("indent");
    r.command("go 1");
    r.command("fold");
    assert_eq!(r.view().folded.len(), 1);
    r.command("go under it");
    assert_eq!(r.here(), "hidden under it");
    assert!(r.view().folded.is_empty(), "the fold is opened to reach it");
}
