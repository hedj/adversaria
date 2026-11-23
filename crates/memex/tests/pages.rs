//! Labels as links (docs/concepts-seven.md, change 4): `#name` and
//! `[[name]]` link to one page, the item of that name if there is one,
//! else a virtual page, made of the items that link to it.

mod support;

use memex::world::outline_facts;
use support::Room;

/// The item whose text this is.
fn titled(r: &Room, text: &str) -> u64 {
    let order = r.outline().in_order();
    *order.iter().find(|i| r.text(**i) == text).expect(text)
}

/// The outline's item_link facts, shown.
fn item_links(r: &Room) -> Vec<String> {
    let facts = outline_facts(r.outline());
    let shown = facts.iter().map(|(_, t)| memex::reason::show(t));
    shown.filter(|s| s.starts_with("item_link")).collect()
}

#[test]
fn a_label_and_links_name_one_page_whatever_their_case_spaces_or_hyphens() {
    // Catches a label and a [[link]] of one name read as two pages.
    let mut r = Room::new("page-one");
    r.items(&[
        "Launch plan #buran_flight",
        "See [[Buran flight]].",
        "Also [[buran-flight]].",
        "Other",
    ]);
    r.command("find #buran_flight");
    assert!(r.shows("LINKED TO #buran_flight · 3"), "{:#?}", r.lines());
    assert!(!r.shows("4. 4. Other"), "{:#?}", r.lines());
}

#[test]
fn the_page_of_a_name_an_item_has_is_that_item() {
    // Catches a label's carrier taken for the page, where an item has the
    // page's name.
    let mut r = Room::new("page-real");
    r.items(&[
        "Draft",
        "Drafting notes",
        "The essay #draft",
        "See [[draft]].",
    ]);
    r.app.idle();
    let (page, essay) = (titled(&r, "Draft"), titled(&r, "The essay #draft"));
    let see = titled(&r, "See [[draft]].");
    assert_eq!(inline_to(&r, see), vec![page]);
    r.command("go draft");
    assert_eq!(r.app.cursor(), page);
    assert!(item_links(&r).contains(&format!("item_link({essay}, {page}, label)")));
}

#[test]
fn a_page_one_item_links_to_is_reached_as_it_and_one_two_link_to_is_shown() {
    let mut r = Room::new("page-virtual");
    r.items(&["One #draft", "Other"]);
    r.command("go draft");
    assert_eq!(r.text(r.app.cursor()), "One #draft");
    r.command("go 2");
    r.typ(" [[draft]]");
    r.command("go 1");
    r.command("go draft");
    assert_eq!(r.text(r.app.cursor()), "One #draft", "the cursor stays");
    assert!(r.shows("LINKED TO #draft · 2"), "{:#?}", r.lines());
}

#[test]
fn a_virtual_page_is_a_fact_by_its_name() {
    // Catches a label or a [[link]] to a page with no item that a rule
    // cannot see.
    let mut r = Room::new("page-facts");
    r.items(&["The essay #draft", "See [[plan]]."]);
    let (essay, see) = (titled(&r, "The essay #draft"), titled(&r, "See [[plan]]."));
    let links = item_links(&r);
    assert!(
        links.contains(&format!("item_link({essay}, draft, label)")),
        "{links:#?}"
    );
    assert!(
        links.contains(&format!("item_link({see}, plan, link)")),
        "{links:#?}"
    );
}

#[test]
fn a_hash_alone_addresses_no_item() {
    // Catches the empty name taken for a page, so that `#` reached an
    // item with no text.
    let mut r = Room::new("page-empty");
    r.items(&["Other", ""]);
    assert_eq!(r.app.resolve("#"), None);
}

/// The items an item's [[links]] go to.
fn inline_to(r: &Room, item: u64) -> Vec<u64> {
    let links = r.outline().inline_links_of(item);
    links.iter().map(|l| l.to).collect()
}

#[test]
fn a_page_is_the_item_named_so_else_the_first_whose_text_it_is() {
    // Catches a later item of the page's text taken for the page, or a
    // text preferred to a name given.
    let mut r = Room::new("page-which");
    r.items(&["Draft", "Draft", "Plan", "See [[draft]].", "Essay #draft"]);
    r.app.idle();
    let see = titled(&r, "See [[draft]].");
    let first = r.outline().in_order()[0];
    assert_eq!(inline_to(&r, see), vec![first]);
    r.command("go 3");
    r.command("name draft");
    let (plan, essay) = (titled(&r, "Plan"), titled(&r, "Essay #draft"));
    assert_eq!(inline_to(&r, see), vec![plan]);
    let fact = format!("item_link({essay}, {plan}, label)");
    assert!(
        item_links(&r).contains(&fact),
        "the facts find the same page"
    );
}

#[test]
fn two_names_of_one_page_give_the_same_item_each_time() {
    // Catches the names kept in a hash map, whose order differs from one
    // opening of a notebook to the next; they are kept sorted.
    for n in 0..12 {
        let mut r = Room::new(&format!("page-names-{n}"));
        r.items(&["Alpha", "Beta"]);
        r.command("go 1");
        r.command("name Draft");
        r.command("go 2");
        r.command("name draft");
        let alpha = titled(&r, "Alpha");
        assert_eq!(r.outline().resolve("draft"), Some(alpha), "run {n}");
    }
}

#[test]
fn a_page_s_item_is_found_whatever_its_case_spaces_or_hyphens() {
    // Catches a page's item matched by its text in lower case alone, so a
    // name of two words, written with a hyphen, found no item.
    let mut r = Room::new("page-spelling");
    r.items(&["Buran  Flight", "See [[buran-flight]]."]);
    r.app.idle();
    let see = titled(&r, "See [[buran-flight]].");
    assert_eq!(inline_to(&r, see), vec![titled(&r, "Buran  Flight")]);
}

/// The inspector's lines at 160 columns: what is right of the rule.
fn inspector(r: &Room) -> Vec<String> {
    let lines = r.lines_at(160, 40);
    let right = |l: &String| l.split_once('│').map(|(_, r)| r.trim().to_string());
    lines
        .iter()
        .filter_map(right)
        .filter(|l| !l.is_empty())
        .collect()
}

/// A page, its item; an essay with a label to it and one to a virtual
/// page; and an item with a [[link]] to a page with no item.
fn paged(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&[
        "Draft",
        "The essay #draft #urgent",
        "See [[plan]].",
        "Notes #urgent",
    ]);
    r.app.idle();
    r
}

#[test]
fn an_item_s_inspector_lists_the_pages_it_links_to() {
    // Catches a label shown nowhere but in what the text was read as, or
    // a [[link]] to no item shown nowhere at all.
    let mut r = paged("page-rows");
    r.command("go 2");
    let shown = inspector(&r);
    let at = shown
        .iter()
        .position(|l| l == "pages →")
        .unwrap_or_else(|| panic!("{shown:#?}"));
    assert_eq!(shown[at + 1], "#draft → 1. Draft");
    assert_eq!(shown[at + 2], "#urgent · no item · linked from 2");
    r.command("go 3");
    assert!(
        inspector(&r).contains(&"[[plan]] · no item · linked from 1".to_string()),
        "{:#?}",
        inspector(&r)
    );
}

#[test]
fn a_click_on_a_page_goes_to_its_item_or_lists_what_links_to_it() {
    let mut r = paged("page-click");
    r.command("go 2");
    let at = r.find("#urgent · no item");
    let hits = r.hits();
    r.app.click_at(&hits, at);
    assert!(r.shows("LINKED TO #urgent · 2"), "{:#?}", r.lines());
    r.command("find");
    let at = r.find("#draft → 1. Draft");
    let hits = r.hits();
    r.app.click_at(&hits, at);
    assert_eq!(r.text(r.app.cursor()), "Draft");
}

#[test]
fn a_page_s_inspector_lists_the_items_that_label_it_and_not_as_mentions() {
    // Catches a label to a page that the page does not show, or shows as
    // a mention not linked, as if the label were not a link.
    let mut r = paged("page-labelled-by");
    r.command("go 1");
    let shown = inspector(&r);
    let at = shown
        .iter()
        .position(|l| l == "labelled by ←")
        .unwrap_or_else(|| panic!("{shown:#?}"));
    assert_eq!(shown[at + 1], "2. The essay #draft #urgent");
    assert!(
        !shown.iter().any(|l| l.starts_with("mentioned in")),
        "{shown:#?}"
    );
}

#[test]
fn an_item_whose_text_another_has_first_is_no_page() {
    // Catches a second item of the page's text taken for the page too.
    let mut r = Room::new("page-second");
    r.items(&["Draft", "Draft", "The essay #draft"]);
    r.command("go 2");
    assert!(
        !inspector(&r).iter().any(|l| l == "labelled by ←"),
        "{:#?}",
        inspector(&r)
    );
    r.command("go 1");
    assert!(
        inspector(&r).iter().any(|l| l == "labelled by ←"),
        "{:#?}",
        inspector(&r)
    );
}

#[test]
fn a_page_given_by_name_lists_the_items_that_label_it() {
    let mut r = paged("page-named");
    r.command("go 3");
    r.command("name urgent");
    let shown = inspector(&r);
    assert!(shown.iter().any(|l| l == "labelled by ←"), "{shown:#?}");
    assert!(
        shown.contains(&"4. Notes #urgent".to_string()),
        "{shown:#?}"
    );
}

#[test]
fn a_text_that_does_not_read_as_a_name_is_not_looked_up_as_a_page() {
    // Catches every item's inspector reading every other item's text to
    // find its labels: only a short name is looked for, as for mentions.
    let mut r = Room::new("page-long");
    r.items(&[
        "one two three four five six seven",
        "x #one_two_three_four_five_six_seven",
    ]);
    r.command("go 1");
    assert!(
        !inspector(&r).iter().any(|l| l == "labelled by ←"),
        "{:#?}",
        inspector(&r)
    );
    r.command("name plan");
    let shown = inspector(&r);
    assert!(
        !shown.iter().any(|l| l == "labelled by ←"),
        "named, its text is still no page: {shown:#?}"
    );
}
