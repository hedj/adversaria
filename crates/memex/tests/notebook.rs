//! The notebook's inspector (docs/concepts-seven.md, change 1): with the
//! cursor above the first item, every offer that waits to be decided.

mod support;

use memex::present::Presentation;
use support::Room;

/// Two dates for one flight, one a claim, and a rule that finds them
/// against each other; the cursor on the notebook.
fn dated(name: &str) -> Room {
    let mut r = Room::new(name);
    r.items(&[
        "The date of Buran-flight is 1988.",
        "Talk-notes says that the date of Buran-flight is 1989.",
        "Nothing has more than 1 date.",
    ]);
    r.app.idle();
    r.app.evaluate().unwrap();
    r.command("go 1");
    r.command("go parent");
    r
}

/// The inspector's lines, at 160 columns: what is right of the rule.
fn inspector(r: &Room) -> Vec<String> {
    let lines = r.lines_at(160, 40);
    let right = |l: &String| l.split_once('│').map(|(_, r)| r.trim().to_string());
    lines
        .iter()
        .filter_map(right)
        .filter(|l| !l.is_empty())
        .collect()
}

#[test]
fn go_parent_from_the_top_level_goes_to_the_notebook() {
    // Catches `go parent` refused at the top level, with nowhere above
    // the first item to go.
    let r = dated("notebook-parent");
    assert_eq!(r.app.cursor(), 0);
    assert!(
        inspector(&r)[0].starts_with("THE NOTEBOOK · "),
        "{:#?}",
        inspector(&r)
    );
}

#[test]
fn the_notebook_lists_findings_then_claims_each_with_its_source() {
    // Catches an offer missing from the one place that lists them, or
    // listed without where it came from.
    let r = dated("notebook-offers");
    let shown = inspector(&r);
    let at = |t: &str| shown.iter().position(|l| l.starts_with(t));
    let (finding, claim) = (at("finding  1:"), at("claim  2: "));
    assert!(finding.is_some() && claim.is_some(), "{shown:#?}");
    assert!(finding < claim, "findings first");
    assert!(finding < at("finding  2:"), "in the outline's order");
    let claim = claim.unwrap();
    assert_eq!(shown[claim + 1], "from Talk-notes");
    assert!(shown.contains(&"from your rule “Nothing has more than 1 date”".to_string()));
}

#[test]
fn a_claim_adopted_is_no_longer_open() {
    let mut r = dated("notebook-adopted");
    r.command("go 2");
    r.command("adopt");
    r.app.evaluate().unwrap();
    r.command("go 1");
    r.command("go parent");
    assert!(
        !inspector(&r).iter().any(|l| l.starts_with("claim  ")),
        "{:#?}",
        inspector(&r)
    );
}

#[test]
fn a_click_on_an_offer_goes_to_it() {
    // Catches an offer that presents nothing, so it could not be acted on.
    let mut r = dated("notebook-click");
    let at = r.find("claim  2: ");
    let hits = r.hits();
    let claim = |p: &Presentation| matches!(p, Presentation::Item { id, .. } if r.text(*id).starts_with("Talk-notes"));
    assert!(hits.at(at.0, at.1).is_some_and(claim));
    r.app.click_at(&hits, at);
    assert!(r.text(r.app.cursor()).starts_with("Talk-notes"));
}

#[test]
fn a_click_on_the_outline_s_title_goes_to_the_notebook() {
    // Catches a title that cannot be clicked to come back to the notebook.
    let mut r = dated("notebook-title");
    r.command("go 2");
    let at = r.find("OUTLINE");
    let hits = r.hits();
    let title = hits.at(at.0, at.1).unwrap();
    assert_eq!(
        r.app.documentation(title),
        "click: go to what waits for you to decide"
    );
    r.app.click_at(&hits, at);
    assert_eq!(r.app.cursor(), 0, "back on the notebook");
}

#[test]
fn a_suggestion_and_a_mend_are_offers_too() {
    // Catches an offer of the memex's left out.
    let mut r = Room::new("notebook-kinds");
    r.items(&[
        "Buran flight",
        "Notes on the buran flight launch",
        "The flight was in 1989.",
    ]);
    r.app.idle();
    r.app.evaluate().unwrap();
    r.command("go parent");
    let shown = inspector(&r);
    let has = |t: &str| shown.iter().any(|l| l.starts_with(t));
    assert!(
        has("suggestion 1  link “Notes on the buran flight launch”"),
        "{shown:#?}"
    );
    assert!(has("mend  3: “The flight is in 1989.”"), "{shown:#?}");
}

#[test]
fn with_nothing_open_the_notebook_says_so() {
    let mut r = Room::new("notebook-none");
    r.items(&["A note"]);
    r.app.evaluate().unwrap();
    r.command("go parent");
    let shown = inspector(&r);
    assert_eq!(shown[0], "THE NOTEBOOK · 0 OPEN OFFERS");
    assert!(
        shown.contains(&"nothing waits for you to decide".to_string()),
        "{shown:#?}"
    );
}

#[test]
fn a_right_click_on_a_claim_adopts_it() {
    // Catches an offer whose right-click offers nothing to decide it, or
    // acts on the notebook in place of the item.
    let mut r = dated("notebook-right-click");
    let at = r.find("claim  2: ");
    let hits = r.hits();
    r.app.right_click(&hits, at, "claim", false);
    let offers = match &r.app.session.mode {
        memex::app::Mode::Menu(m) => m.offers.clone(),
        other => panic!("no menu: {other:?}"),
    };
    let adopt = offers.iter().position(|o| o.line == "adopt");
    let adopt = adopt.unwrap_or_else(|| panic!("{offers:#?}"));
    (0..adopt).for_each(|_| r.tap(&[memex::app::Key::Down]));
    r.tap(&[memex::app::Key::Enter]);
    assert!(
        r.app.session.message.starts_with("adopted"),
        "{}",
        r.app.session.message
    );
}

#[test]
fn a_long_list_shows_twelve_and_counts_the_rest() {
    // Catches a list that runs off the screen, or that hides offers
    // without saying how many.
    let mut r = Room::new("notebook-long");
    let claims: Vec<String> = (1..=14)
        .map(|n| {
            format!(
                "Source-{n} says that the date of Flight-{n} is {}.",
                1900 + n
            )
        })
        .collect();
    r.items(&claims.iter().map(String::as_str).collect::<Vec<_>>());
    r.app.idle();
    r.app.evaluate().unwrap();
    r.command("go parent");
    let shown = inspector(&r);
    assert_eq!(shown[0], "THE NOTEBOOK · 14 OPEN OFFERS", "{shown:#?}");
    assert_eq!(
        shown.iter().filter(|l| l.starts_with("claim  ")).count(),
        12
    );
    assert!(
        shown.iter().any(|l| l.starts_with("2 more · ")),
        "{shown:#?}"
    );
}

#[test]
fn claims_are_listed_in_the_outline_s_order() {
    // Catches claims listed in the order they were written, so a moved
    // item's claim sat where the item no longer is.
    let mut r = Room::new("notebook-order");
    r.items(&[
        "Source-a says that the date of Flight-a is 1901.",
        "Source-b says that the date of Flight-b is 1902.",
    ]);
    r.command("go 1");
    r.command("move after 2");
    r.app.idle();
    r.app.evaluate().unwrap();
    r.command("go parent");
    let shown = inspector(&r);
    let at = |t: &str| shown.iter().position(|l| l.contains(t));
    assert!(at("1902") < at("1901"), "{shown:#?}");
}

#[test]
fn a_right_click_on_a_finding_offers_its_item_s_claims_and_its_rule() {
    // Catches a finding that forgets the item it is under, so its menu
    // offered the rule alone.
    let mut r = dated("notebook-finding");
    let at = r.find("finding  2: ");
    let hits = r.hits();
    r.app.right_click(&hits, at, "finding", false);
    let lines: Vec<String> = match &r.app.session.mode {
        memex::app::Mode::Menu(m) => m.offers.iter().map(|o| o.line.clone()).collect(),
        other => panic!("no menu: {other:?}"),
    };
    assert!(lines.contains(&"adopt".to_string()), "{lines:#?}");
    assert!(
        lines.contains(&"go 3".to_string()),
        "the rule's item: {lines:#?}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("retract entry ")),
        "{lines:#?}"
    );
}
