//! Tools beside the memex (docs/plan.md, Layout). `wikidata`: a subset of
//! Wikidata as a file of Prolog facts (docs/translator.md § The Wikidata
//! subset), each statement to be read as a claim whose source is Wikidata.
//!
//! The file holds three kinds of fact, one per line, readable by
//! SWI-Prolog's read_term/3:
//! * `wd(Item, Property, Value)`: a truthy statement. Value is an item
//!   ('Q42'), `date(Y, M, D)`, a number, or a string. External identifiers
//!   are left out.
//! * `wd_label(Entity, "English label")`.
//! * `wd_constraint(Property, Kind)`: from the property's P2302 statements;
//!   Kind is single_value, distinct_values, or the constraint's item.

pub mod held;
pub mod interactivity;
pub mod measures;

use serde_json::Value;
use std::process::Command;

pub const ENDPOINT: &str = "https://query.wikidata.org/sparql";
/// Wikidata asks automated clients to name themselves. A generic name: it
/// names neither the user nor the project.
pub const USER_AGENT: &str = "wikidata-subset-tool/0.1 (prototype)";

/// A Prolog atom, quoted when it must be.
pub fn atom(s: &str) -> String {
    let plain = s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if plain {
        s.to_string()
    } else {
        format!("'{}'", escape(s, '\''))
    }
}

/// A Prolog string in double quotes.
pub fn string(s: &str) -> String {
    format!("\"{}\"", escape(s, '"'))
}

fn escape(s: &str, quote: char) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 => out.push_str(&format!("\\x{:x}\\", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The id at the end of an entity URI, if it is one.
pub fn entity_id(uri: &str) -> Option<&str> {
    uri.strip_prefix("http://www.wikidata.org/entity/")
        .filter(|id| id.starts_with('Q') || id.starts_with('P'))
}

/// A SPARQL result value as a Prolog term, or None if it has no form here.
pub fn value(v: &Value) -> Option<String> {
    let s = v["value"].as_str()?;
    match v["type"].as_str()? {
        "uri" => entity_id(s).map(atom),
        "literal" | "typed-literal" => literal(v, s),
        _ => None,
    }
}

/// A literal: a date, a number, or English (or untagged) text; none of
/// another type or language.
fn literal(v: &Value, s: &str) -> Option<String> {
    match v["datatype"].as_str() {
        Some("http://www.w3.org/2001/XMLSchema#dateTime") => date_value(s),
        Some("http://www.w3.org/2001/XMLSchema#decimal")
        | Some("http://www.w3.org/2001/XMLSchema#integer")
        | Some("http://www.w3.org/2001/XMLSchema#double") => number_value(s),
        Some(_) => None,
        None => text_value(v, s),
    }
}

/// "+1988-11-15T00:00:00Z", or "-0500-01-01T00:00:00Z", as date(Y, M, D).
/// Precision is not kept.
fn date_value(s: &str) -> Option<String> {
    let neg = s.starts_with('-');
    let date = s.trim_start_matches(['+', '-']).split('T').next()?;
    let mut parts = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    Some(format!("date({}, {m}, {d})", if neg { -y } else { y }))
}

/// A whole number as itself, else a finite float.
fn number_value(s: &str) -> Option<String> {
    let n = s.trim_start_matches('+');
    if let Ok(i) = n.parse::<i64>() {
        return Some(i.to_string());
    }
    let x = n.parse::<f64>().ok().filter(|x| x.is_finite());
    x.map(|x| format!("{x:?}"))
}

/// Text in English, or with no language.
fn text_value(v: &Value, s: &str) -> Option<String> {
    match v["xml:lang"].as_str() {
        None | Some("en") => Some(string(s)),
        Some(_) => None,
    }
}

/// The rows of a SPARQL JSON result.
pub fn rows(result: &Value) -> Vec<&serde_json::Map<String, Value>> {
    result["results"]["bindings"]
        .as_array()
        .map(|a| a.iter().filter_map(|b| b.as_object()).collect())
        .unwrap_or_default()
}

/// Statements: rows of ?item ?p ?v.
pub fn statement_facts(result: &Value) -> Vec<String> {
    rows(result)
        .into_iter()
        .filter_map(|r| {
            let item = entity_id(r.get("item")?["value"].as_str()?)?;
            let p = r.get("p")?["value"].as_str()?;
            let v = value(r.get("v")?)?;
            Some(format!("wd({}, {}, {v}).", atom(item), atom(p)))
        })
        .collect()
}

/// Labels: rows of ?e ?l.
pub fn label_facts(result: &Value) -> Vec<String> {
    rows(result)
        .into_iter()
        .filter_map(|r| {
            let e = entity_id(r.get("e")?["value"].as_str()?)?;
            let l = r.get("l")?["value"].as_str()?;
            Some(format!("wd_label({}, {}).", atom(e), string(l)))
        })
        .collect()
}

/// Property constraints: rows of ?prop ?c.
pub fn constraint_facts(result: &Value) -> Vec<String> {
    rows(result)
        .into_iter()
        .filter_map(|r| {
            let p = entity_id(r.get("prop")?["value"].as_str()?)?;
            let c = entity_id(r.get("c")?["value"].as_str()?)?;
            let kind = match c {
                "Q19474404" => "single_value".to_string(),
                "Q21502410" => "distinct_values".to_string(),
                c => atom(c),
            };
            Some(format!("wd_constraint({}, {kind}).", atom(p)))
        })
        .collect()
}

pub fn values_clause(ids: &[String]) -> String {
    ids.iter()
        .map(|i| format!("wd:{i}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn statements_query(items: &[String]) -> String {
    format!(
        "SELECT ?item ?p ?v WHERE {{ VALUES ?item {{ {} }} ?item ?wdt ?v . ?prop wikibase:directClaim ?wdt ; wikibase:propertyType ?t . \
         FILTER(?t != wikibase:ExternalId) BIND(STRAFTER(STR(?prop), \"entity/\") AS ?p) }}",
        values_clause(items)
    )
}

pub fn labels_query(entities: &[String]) -> String {
    format!(
        "SELECT ?e ?l WHERE {{ VALUES ?e {{ {} }} ?e rdfs:label ?l FILTER(LANG(?l) = \"en\") }}",
        values_clause(entities)
    )
}

pub fn constraints_query(props: &[String]) -> String {
    format!(
        "SELECT ?prop ?c WHERE {{ VALUES ?prop {{ {} }} ?prop p:P2302 ?s . ?s ps:P2302 ?c }}",
        values_clause(props)
    )
}

pub fn top_query(min_sitelinks: u32, limit: u32) -> String {
    format!(
        "SELECT ?item WHERE {{ ?item wikibase:sitelinks ?n . FILTER(?n >= {min_sitelinks}) }} LIMIT {limit}"
    )
}

/// Run a SPARQL query with curl, which honours the machine's proxy and
/// certificates. Returns the parsed JSON result.
pub fn sparql(query: &str) -> Result<Value, String> {
    let run = query_command(query).output();
    let out = run.map_err(|e| format!("cannot run curl: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "the query service refused: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("not a SPARQL JSON result: {e}"))
}

/// curl, asking the endpoint for a SPARQL query's result as JSON, within
/// five minutes, and failing on an HTTP error.
fn query_command(query: &str) -> Command {
    let mut curl = Command::new("curl");
    let json = "Accept: application/sparql-results+json";
    curl.args(["-sS", "--fail", "--max-time", "300", "-G"]);
    curl.args(["-A", USER_AGENT, "-H", json, "--data-urlencode"]);
    curl.arg(format!("query={query}")).arg(ENDPOINT);
    curl
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_query_asks_curl_for_json_from_the_endpoint() {
        // Catches an argument of the query's command lost or moved.
        let command = query_command("SELECT 1");
        let args: Vec<_> = command.get_args().map(|a| a.to_string_lossy()).collect();
        let json = "Accept: application/sparql-results+json";
        let curl = [
            "-sS",
            "--fail",
            "--max-time",
            "300",
            "-G",
            "-A",
            USER_AGENT,
            "-H",
            json,
        ];
        let query = ["--data-urlencode", "query=SELECT 1", ENDPOINT];
        assert_eq!(args, [&curl[..], &query[..]].concat());
    }

    #[test]
    fn text_in_english_is_kept_and_other_languages_are_not() {
        // Catches a label tagged "en" dropped, or one in another language
        // kept.
        let text = |lang: &str| json!({"type": "literal", "value": "Buran", "xml:lang": lang});
        assert_eq!(
            (value(&text("en")), value(&text("fr"))),
            (Some(string("Buran")), None)
        );
    }

    #[test]
    fn values_become_terms() {
        // Catches a date, number, item or string written in a form Prolog
        // reads differently, or a value that should be left out kept.
        let lit = |v: &str, dt: &str| json!({"type": "literal", "value": v, "datatype": dt});
        let xsd = |t: &str| format!("http://www.w3.org/2001/XMLSchema#{t}");
        assert_eq!(
            value(&lit("1988-11-15T00:00:00Z", &xsd("dateTime"))).unwrap(),
            "date(1988, 11, 15)"
        );
        assert_eq!(
            value(&lit("-0500-01-01T00:00:00Z", &xsd("dateTime"))).unwrap(),
            "date(-500, 1, 1)"
        );
        assert_eq!(value(&lit("+2", &xsd("decimal"))).unwrap(), "2");
        assert_eq!(value(&lit("1.5", &xsd("decimal"))).unwrap(), "1.5");
        assert_eq!(
            value(&json!({"type": "uri", "value": "http://www.wikidata.org/entity/Q2548"}))
                .unwrap(),
            "'Q2548'"
        );
        assert_eq!(
            value(&json!({"type": "literal", "value": "Buran \"1K\"\\x"})).unwrap(),
            "\"Buran \\\"1K\\\"\\\\x\""
        );
        assert_eq!(
            value(&json!({"type": "literal", "value": "Буран", "xml:lang": "ru"})),
            None
        );
        assert_eq!(
            value(&json!({"type": "uri", "value": "http://commons.wikimedia.org/x.jpg"})),
            None
        );
        assert_eq!(
            value(&lit("x", "http://www.opengis.net/ont/geosparql#wktLiteral")),
            None
        );
    }

    #[test]
    fn atoms_are_quoted_when_needed() {
        // Catches an atom Prolog would read as a variable or a different atom.
        assert_eq!(atom("Q42"), "'Q42'");
        assert_eq!(atom("single_value"), "single_value");
        assert_eq!(atom("it's"), "'it\\'s'");
    }
}
