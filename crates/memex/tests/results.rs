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
    r.leap(memex::app::Quasi::LeapForward, "x =");
    assert_eq!(r.here(), "x = 1.5 ± 0.1 s");
}
