//! Where what is known comes from: importing documents, results and
//! Wikidata, exporting the outline, and checking quotes and sources.

use crate::app::App;
use crate::outline::reads_as_number;
use serde_json::json;
use std::path::Path;

impl App {
    /// Import a file, of the kind named first, or else of the kind its
    /// name ends in (kind_of).
    pub(crate) fn cmd_import(&mut self, words: &[&str], _: u64) -> Result<String, String> {
        let usage = "import (from) a file: a document, results (.json) or a Wikidata subset (.pl); a document as ADDRESS is a revision of the one that item came from";
        let (what, rest) = words.split_first().ok_or(usage)?;
        let (what, file) = match *what {
            "document" | "results" | "wikidata" => (*what, rest.join(" ")),
            _ => (kind_of(&words.join(" ")), words.join(" ")),
        };
        match what {
            "document" => match file.rsplit_once(" as ") {
                Some((file, at)) => self.import_revision_of(file.trim(), at.trim()),
                None => self.import_document(&file),
            },
            "results" => self.import_results(&file),
            "wikidata" if !file.is_empty() => self.import_wikidata(&file),
            _ => Err(usage.into()),
        }
    }

    /// Import a file the wikidata tool made: its facts arrive as claims.
    fn import_wikidata(&mut self, file: &str) -> Result<String, String> {
        let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        self.vet_wikidata(file)?;
        let hash = self.notebook.store_blob(&bytes)?;
        self.write_wikidata_import(file, &hash)?;
        Ok(format!(
            "imported wikidata: {} bytes, blob {}",
            bytes.len(),
            &hash[..12]
        ))
    }

    fn vet_wikidata(&mut self, file: &str) -> Result<(), String> {
        self.reason
            .engine(0)?
            .check_wikidata(Path::new(file))
            .map_err(|e| format!("{file} is not a Wikidata subset: {e}"))
    }

    /// The import entry, with its provenance after W3C PROV: what it was
    /// derived from, under what licence, and the file it came in.
    fn write_wikidata_import(&mut self, file: &str, hash: &str) -> Result<(), String> {
        let name = Path::new(file)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.write_here(
            "import",
            json!({"blob": hash, "format": "wikidata-subset", "source": "Wikidata", "file": name,
                   "prov": {"prov:wasDerivedFrom": "https://www.wikidata.org/", "license": "CC0-1.0", "prov:wasGeneratedBy": "crates/tools/src/bin/wikidata.rs"}}),
        )?;
        Ok(())
    }

    pub(crate) fn cmd_export(&mut self, words: &[&str], _: u64) -> Result<String, String> {
        let (file, section) = self.export_args(words)?;
        if file.is_empty() {
            return Err("export (to) a file, and an item number for one section".into());
        }
        self.export(&file, section)
    }

    /// The file to export to, and the section: the last word, if it reads
    /// as an item's number and there are words before it; it must name an
    /// item.
    fn export_args(&self, words: &[&str]) -> Result<(String, Option<u64>), String> {
        match words.split_last() {
            Some((last, rest)) if !rest.is_empty() && reads_as_number(last) => {
                let item = self.notebook.outline.by_number(last);
                let item = item.ok_or(format!("there is no item {last}"))?;
                Ok((rest.join(" "), Some(item)))
            }
            _ => Ok((words.join(" "), None)),
        }
    }

    /// What `check` checks by default: the target's quote, if it was
    /// imported from a document, or the source it states; neither if it
    /// has both or none, so the memex asks.
    pub(crate) fn check_kind(&self, target: u64) -> Option<String> {
        let quote = self.notebook.quote(target).is_ok();
        let source = self.notebook.states_a_source(target);
        match (quote, source) {
            (true, false) => Some("quote".into()),
            (false, true) => Some("source".into()),
            _ => None,
        }
    }

    pub(crate) fn cmd_check(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let ruling = match words.first().copied() {
            Some("quote") => self.check_quote(target)?,
            Some("source") => self.check_source(target)?,
            _ => return Err("check quote, or check source".into()),
        };
        Ok(format!("{} check: {ruling}", words[0]))
    }
}

/// The kind of file a name ends in: a Wikidata subset (`.pl`, as the
/// wikidata tool writes), results (`.json`), and else a document, which
/// Pandoc reads or refuses. A name followed by `as ADDRESS` ends in the
/// address, so it is a document's, the one kind that has revisions.
fn kind_of(file: &str) -> &'static str {
    match Path::new(file.trim()).extension().and_then(|e| e.to_str()) {
        Some("pl") => "wikidata",
        Some("json") => "results",
        _ => "document",
    }
}
