//! The checks of a claim: its quote against the document it came from,
//! and its source against Crossref. Each writes its ruling as a fact
//! about the claim (docs/structure.md § The code's structure).

use crate::app::App;
use crate::notebook::Notebook;
use crate::session::Session;
use format::terms::Term;
use serde_json::Value;

/// A claimed source: its DOI, and the title claimed for it, if any.
struct Source {
    doi: String,
    title: Option<String>,
}

impl Source {
    /// The source an item's facts claim.
    fn of(facts: &[Term]) -> Result<Source, String> {
        let doi = stated(facts, "doi").ok_or("the target states no doi(Entity, \"10.…\") fact")?;
        // A dot segment would make the fetch climb out of /works (curl
        // removes them), to a record that answers for no DOI.
        if !doi.starts_with("10.") || doi.split('/').any(|s| s == "." || s == "..") {
            return Err(format!("“{doi}” is not a DOI"));
        }
        let title = stated(facts, "title");
        Ok(Source { doi, title })
    }

    /// Ask Crossref, through the session, for the source's record: the
    /// ruling, and the hash of the record read.
    fn check(&self, session: &Session) -> Result<(&'static str, String), String> {
        let (body, code) = fetch(&session.source_url(&self.doi))?;
        let ruling = source_ruling(&code, &body, self.title.as_deref());
        Ok((ruling, format::record::sha256_hex(body.as_bytes())))
    }
}

impl App {
    /// The quote check: does the item's text still appear in the document it
    /// was imported from? Spaces, hyphens and case are ignored.
    pub(crate) fn check_quote(&mut self, item: u64) -> Result<String, String> {
        let quote = self.notebook.quote(item)?;
        let ruling = quote.ruling()?;
        self.write_ruling(item, "quote", ruling, &quote.blob)?;
        Ok(ruling.into())
    }

    /// The source check: fetch the item's DOI from Crossref, never trusting a
    /// copy that came with the claim, and rule: found, misattributed (the
    /// title differs from the one claimed), not_found, unavailable, retracted.
    pub(crate) fn check_source(&mut self, item: u64) -> Result<String, String> {
        let source = Source::of(&self.notebook.facts_of(item)?)?;
        let (ruling, hash) = source.check(&self.session)?;
        self.write_ruling(item, "source", ruling, &hash)?;
        Ok(ruling.into())
    }

    /// A ruling is a fact about the claim, with the hash of what it examined.
    fn write_ruling(
        &mut self,
        item: u64,
        check: &str,
        ruling: &str,
        hash: &str,
    ) -> Result<(), String> {
        let fact = ruling_fact(item, check, ruling, hash);
        self.write_fact(item, &fact, None)
    }
}

/// The string a fact of this name gives its entity, name(Entity, "...").
fn stated(facts: &[Term], name: &str) -> Option<String> {
    facts.iter().find_map(|f| match f {
        Term::Compound(n, a) if n == name && a.len() == 2 => match &a[1] {
            Term::Str(s) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    })
}

/// Fetch a record from the web: its body and its HTTP status code.
fn fetch(url: &str) -> Result<(String, String), String> {
    let out = std::process::Command::new("curl")
        .args([
            // No URL patterns ([1-9], {a,b}): one request, for this DOI.
            "--globoff",
            // Only the web, and no more than Crossref's records need.
            "--proto",
            "=http,https",
            "--max-filesize",
            "10000000",
            "-sS",
            "--max-time",
            "30",
            "--noproxy",
            "localhost,127.0.0.1",
            "-A",
            "source-check/0.1 (prototype)",
            "-w",
            "\n%{http_code}",
        ])
        .arg(url)
        .output()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", ""));
    Ok((body.to_string(), code.to_string()))
}

/// What Crossref's answer says of a claimed source: not found,
/// unavailable, retracted, misattributed (its title differs from the one
/// claimed, spaces, hyphens and case aside), or found.
fn source_ruling(code: &str, body: &str, claimed: Option<&str>) -> &'static str {
    let record = match code.trim() {
        "404" => return "not_found",
        "200" => serde_json::from_str::<Value>(body),
        _ => return "unavailable",
    };
    let Ok(v) = record else { return "unavailable" };
    let m = &v["message"];
    if is_retracted(m) {
        return "retracted";
    }
    let title = m["title"][0].as_str().unwrap_or("");
    let differs = claimed.is_some_and(|t| crate::docs::normal(t) != crate::docs::normal(title));
    if differs { "misattributed" } else { "found" }
}

/// Whether a Crossref record says the work is retracted, by an update or
/// by a relation.
fn is_retracted(m: &Value) -> bool {
    let updates = m["updated-by"].as_array().into_iter().flatten();
    let by_update = updates.clone().any(|u| u["type"] == "retraction");
    let relation = m["relation"]["is-retracted-by"].as_array();
    by_update || relation.is_some_and(|a| !a.is_empty())
}

/// A check's ruling on an item, with the hash of what it read.
fn ruling_fact(item: u64, check: &str, ruling: &str, hash: &str) -> Term {
    let (check, ruling) = (Term::Atom(check.into()), Term::Atom(ruling.into()));
    let args = vec![
        Term::Int(item as i64),
        check,
        ruling,
        Term::Str(hash.into()),
    ];
    Term::Compound("ruling".into(), args)
}

/// A DOI as a URL path: every byte but letters, digits, "-._~" and "/"
/// percent-encoded, as Crossref's API asks (RFC 3986's unreserved set).
pub fn encode_doi(doi: &str) -> String {
    doi.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl Notebook {
    /// Whether an item's facts state a source the source check can check.
    pub(crate) fn states_a_source(&self, item: u64) -> bool {
        let facts = self.facts_of(item);
        facts.is_ok_and(|f| Source::of(&f).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_doi_with_a_dot_segment_is_refused() {
        // Catches "10.1/../../members" fetched as Crossref's /members,
        // which answers, so the claim was ruled found with no title
        // claimed (the code review, D-B2).
        let doi = |d: &str| {
            let fact = Term::Compound(
                "doi".into(),
                vec![Term::Atom("x".into()), Term::Str(d.into())],
            );
            Source::of(&[fact]).map(|s| s.doi)
        };
        assert_eq!(doi("10.1038/nphys1170").unwrap(), "10.1038/nphys1170");
        for bad in ["10.1/../../members", "10.1/./x", "members"] {
            assert!(doi(bad).unwrap_err().contains("is not a DOI"), "{bad}");
        }
    }
}
