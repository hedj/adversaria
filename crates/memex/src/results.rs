//! A results file: values an analysis measured, with their provenance in
//! W3C PROV terms (docs/structure.md § The code's structure). It is read
//! whole, or refused whole, before anything of it is written.

use crate::app::App;
use crate::docs::Block;
use crate::notebook::read_input;
use crate::outline::TOP;
use format::terms::Term;
use serde_json::{Value, json};
use std::path::Path;

/// A results file: its source, the analysis script at its commit; its
/// provenance; and each result.
pub struct Results {
    pub source: String,
    pub prov: Value,
    pub measured: Vec<Measured>,
}

/// One result: its name, its value and uncertainty, and its unit.
pub struct Measured {
    pub name: String,
    pub value: (f64, f64),
    pub unit: String,
}

impl Results {
    /// Read a results file, or say what is wrong with it.
    pub fn parse(bytes: &[u8]) -> Result<Results, String> {
        let v: Value =
            serde_json::from_slice(bytes).map_err(|e| format!("not a results file: {e}"))?;
        if v["format"] != "memex-results" {
            return Err("not a results file: its format is not memex-results".into());
        }
        if v["version"] != 1 {
            return Err(format!(
                "results of version {} are not known here",
                v["version"]
            ));
        }
        let results = v["results"]
            .as_array()
            .ok_or("not a results file: no list of results")?;
        let measured = results
            .iter()
            .map(Measured::parse)
            .collect::<Result<_, _>>()?;
        Ok(Results {
            source: source_of(&v["prov"]["activity"]),
            prov: v["prov"].clone(),
            measured,
        })
    }
}

/// The source of results: the analysis script, at its commit.
fn source_of(activity: &Value) -> String {
    let script = activity["script"].as_str().unwrap_or("analysis");
    format!(
        "{script}@{}",
        activity["commit"].as_str().unwrap_or("uncommitted")
    )
}

impl Measured {
    fn parse(r: &Value) -> Result<Measured, String> {
        let name = r["name"]
            .as_str()
            .ok_or("a result needs a name")?
            .to_string();
        let number = |k: &str| {
            r[k].as_f64()
                .ok_or(format!("result {name}: {k} must be a number"))
        };
        let value = (number("value")?, number("uncertainty")?);
        let unit = r["unit"].as_str().unwrap_or("").to_string();
        Ok(Measured { name, value, unit })
    }

    /// How the result reads in the outline.
    pub fn text(&self) -> String {
        let (value, u) = self.value;
        format!("{} = {value} ± {u} {}", self.name, self.unit)
    }

    /// The claim a source makes of the result.
    pub fn claim(&self, source: &str) -> Term {
        let c = |f: &str, a: Vec<Term>| Term::Compound(f.into(), a);
        let (value, u) = self.value;
        let measured = c(
            "measured",
            vec![
                Term::Float(value),
                Term::Float(u),
                Term::Str(self.unit.clone()),
            ],
        );
        let result = c("result", vec![Term::Atom(self.name.clone()), measured]);
        c("claims", vec![Term::Str(source.into()), result])
    }
}

impl App {
    /// Import results: an exchange file of values with their provenance (W3C
    /// PROV terms), as claims whose source is the analysis.
    pub(crate) fn import_results(&mut self, file: &str) -> Result<String, String> {
        let bytes = read_input(Path::new(file))?;
        let results = Results::parse(&bytes)?;
        let import = self.write_results_import(file, &bytes, &results)?;
        let top = self.write_results_heading(&results.source, import)?;
        for m in &results.measured {
            self.write_measured(m, &results.source, top, import)?;
        }
        Ok(format!("imported the results of {}", results.source))
    }

    /// The import entry of a results file: its blob, its source, the
    /// directory its inputs are named from, and its provenance.
    fn write_results_import(
        &mut self,
        file: &str,
        bytes: &[u8],
        results: &Results,
    ) -> Result<u64, String> {
        let blob = self.notebook.store_blob(bytes)?;
        let base = directory_of(file);
        let (source, prov) = (&results.source, &results.prov);
        let body = json!({"blob": blob, "format": "memex-results", "source": source, "base": base, "prov": prov});
        Ok(self.write_here("import", body)?.seq)
    }

    /// The heading the results go under, last in the outline.
    fn write_results_heading(&mut self, source: &str, import: u64) -> Result<u64, String> {
        let head = Block::plain(0, "heading", format!("Results of {source}"));
        self.append_block(&head, TOP, import)
    }

    /// One result: its item, last under the heading, and the claim its
    /// source makes of it.
    fn write_measured(
        &mut self,
        m: &Measured,
        source: &str,
        top: u64,
        import: u64,
    ) -> Result<(), String> {
        let id = self.append_block(&Block::plain(1, "para", m.text()), top, import)?;
        self.write_fact(id, &m.claim(source), Some(source))
    }
}

/// The directory a file is in, as the system names it; none if it cannot
/// be named.
fn directory_of(file: &str) -> String {
    let path = std::fs::canonicalize(file).ok();
    let dir = path.and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()));
    dir.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_without_results_or_of_another_version_is_refused() {
        // Catches a file with no results list imported as no results, a
        // heading and nothing under it, and a version the reader does not
        // know read as if it were known (the code review, D-B5).
        let parsed = |v: Value| Results::parse(v.to_string().as_bytes()).map(|r| r.measured.len());
        let kept = serde_json::json!({"format": "memex-results", "version": 1, "results": []});
        assert_eq!(parsed(kept), Ok(0));
        let no_list = serde_json::json!({"format": "memex-results", "version": 1});
        assert!(parsed(no_list).unwrap_err().contains("no list of results"));
        let later = serde_json::json!({"format": "memex-results", "version": 2, "results": []});
        assert!(parsed(later).unwrap_err().contains("version 2"));
    }
}
