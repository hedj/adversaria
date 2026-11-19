//! What an item's text was read as, in Prolog, in the right pane
//! (docs/guide.md § 3.1).

mod support;

use memex::app::Key;
use support::Room;

/// The right pane's rows at 160 by 40, each as drawn.
fn right_pane(r: &Room) -> Vec<String> {
    let rows = r.lines_at(160, 40);
    let right = |l: &String| l.split('│').nth(1).unwrap_or("").trim().to_string();
    rows.iter().map(right).collect()
}

/// A room with one item of each kind of statement, read.
fn statements(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&[
        "The date of Buran-flight is 1988.",
        "Talk-notes says that the date of Buran-flight is 1989.",
        "If the income is X and the rent is Y then the rest is X - Y.",
        "Nothing has more than 1 date.",
        "What is an animal?",
        "The income is 2000. #money",
        "Buran flew once, unmanned.",
    ]);
    r.app.idle();
    r
}

#[test]
fn each_statement_is_shown_in_its_prolog_form() {
    // Catches a kind of statement not shown, or shown in another form
    // than the reader wrote and the memex runs.
    let mut r = statements("read-as-forms");
    let order = r.outline().in_order();
    let expected = [
        "fact  date(buran_flight, 1988)",
        "claim of Talk-notes  date(buran_flight, 1989)",
        "rule  rest(A):-cell(income, B), cell(rent, C), A is B-C",
        "constraint  false:-date(A, B), date(A, C), B\\==C",
        "question  findall(C, (animal(A), (label(A,",
        "fact  cell(income, 2000)",
    ];
    for (item, shown) in order.iter().zip(expected) {
        r.put_cursor(*item);
        let pane = right_pane(&r);
        assert!(pane.iter().any(|l| l == "READ AS"), "{pane:#?}");
        assert!(
            pane.iter().any(|l| l.starts_with(shown)),
            "{shown}: {pane:#?}"
        );
    }
}

#[test]
fn an_items_tags_are_shown_too() {
    let mut r = statements("read-as-tags");
    let item = r.outline().in_order()[5];
    r.put_cursor(item);
    let tag = format!("tag  item_tag({item}, money)");
    assert!(right_pane(&r).contains(&tag), "{:#?}", right_pane(&r));
}

#[test]
fn prose_and_a_text_not_yet_read_again_show_nothing() {
    // Catches prose given a reading, or the reading of an old text shown
    // for a text that was changed and not read again yet.
    let mut r = statements("read-as-none");
    let order = r.outline().in_order();
    r.put_cursor(order[6]);
    assert!(
        !right_pane(&r).iter().any(|l| l == "READ AS"),
        "{:#?}",
        right_pane(&r)
    );
    r.put_cursor(order[0]);
    r.tap(&[Key::End, Key::Backspace, Key::Backspace]);
    r.app.flush();
    assert!(
        !right_pane(&r).iter().any(|l| l.contains("1988")),
        "{:#?}",
        right_pane(&r)
    );
}
