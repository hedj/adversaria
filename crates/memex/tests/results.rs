//! Importing a results file: values with their provenance, as claims whose
//! source is the analysis; and what a malformed file gets (docs/plan.md,
//! Milestone 6).

mod support;

use support::Room;

/// What importing a results file of this text says.
fn import(r: &mut Room, text: &str) -> String {
    let file = r.dir.join("results.json");
    std::fs::write(&file, text).unwrap();
    r.command(&format!("import results {}", file.display()));
    r.app.session.message.clone()
}

/// A results file of these results, from analysis.py at commit abc.
fn results(results: &str) -> String {
    let prov = r#"{"activity": {"script": "analysis.py", "commit": "abc"}, "used": []}"#;
    format!(
        r#"{{"format": "memex-results", "version": 1, "results": [{results}], "prov": {prov}}}"#
    )
}

#[test]
fn a_file_that_is_no_json_is_refused() {
    let mut r = Room::new("results-garbled");
    assert!(import(&mut r, "<html>").starts_with("not a results file: "));
}

#[test]
fn a_file_of_another_format_is_refused() {
    let mut r = Room::new("results-format");
    let said = import(&mut r, r#"{"format": "other"}"#);
    assert_eq!(said, "not a results file: its format is not memex-results");
}

#[test]
fn a_result_without_a_name_is_refused() {
    let mut r = Room::new("results-unnamed");
    let said = import(&mut r, &results(r#"{"value": 1.0, "uncertainty": 0.1}"#));
    assert_eq!(said, "a result needs a name");
}

#[test]
fn a_result_whose_value_is_no_number_is_refused() {
    let mut r = Room::new("results-nan");
    let said = import(
        &mut r,
        &results(r#"{"name": "x", "value": "big", "uncertainty": 0.1}"#),
    );
    assert_eq!(said, "result x: value must be a number");
}

#[test]
fn a_result_is_its_name_value_uncertainty_and_unit() {
    let mut r = Room::new("results-text");
    let said = import(
        &mut r,
        &results(r#"{"name": "x", "value": 1.5, "uncertainty": 0.1, "unit": "s"}"#),
    );
    assert_eq!(said, "imported the results of analysis.py@abc");
    r.leap(memex::app::Quasi::LeapDown, "x =");
    assert_eq!(r.here(), "x = 1.5 ± 0.1 s");
}

#[test]
fn a_result_is_claimed_by_its_source() {
    let mut r = Room::new("results-claim");
    import(
        &mut r,
        &results(r#"{"name": "x", "value": 1.5, "uncertainty": 0.1, "unit": "s"}"#),
    );
    let claim = "claims(\"analysis.py@abc\", result(x, measured(1.5, 0.1, \"s\")))";
    assert!(r.facts().contains(&claim.to_string()), "{:?}", r.facts());
}

#[test]
fn results_follow_the_outline_in_their_order() {
    let mut r = Room::new("results-order");
    r.items(&["before"]);
    let two = r#"{"name": "a", "value": 1, "uncertainty": 0}, {"name": "b", "value": 2, "uncertainty": 0}"#;
    import(&mut r, &results(two));
    let texts: Vec<String> = r.app.visible().iter().map(|(id, _)| r.text(*id)).collect();
    assert_eq!(
        texts,
        [
            "before",
            "Results of analysis.py@abc",
            "a = 1 ± 0 ",
            "b = 2 ± 0 "
        ]
    );
}

#[test]
fn a_malformed_results_file_writes_nothing() {
    // An import is all or nothing: a result refused after others were
    // written would leave half a file in a journal kept for good.
    let mut r = Room::new("results-nothing");
    let entries = r.app.journal().entries().len();
    import(
        &mut r,
        &results(r#"{"name": "x", "value": 1.0, "uncertainty": 0.1}, {"value": 2.0}"#),
    );
    assert_eq!(r.app.journal().entries().len(), entries);
}

#[test]
fn importing_a_file_again_mends_its_blob_if_it_was_torn() {
    // Catches a blob written in place: a write cut short leaves a file
    // under the content's hash that is not that content, which every
    // later import of the same file then left as it was.
    let mut r = Room::new("results-torn-blob");
    let text = results(r#"{"name": "g", "value": 9.81, "uncertainty": 0.01, "unit": "m/s2"}"#);
    import(&mut r, &text);
    let blob = r
        .dir
        .join("blobs")
        .join(format::record::sha256_hex(text.as_bytes()));
    std::fs::write(&blob, &text[..10]).unwrap();
    import(&mut r, &text);
    assert_eq!(std::fs::read_to_string(&blob).unwrap(), text);
}
