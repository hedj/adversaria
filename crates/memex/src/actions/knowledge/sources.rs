//! Where what is known comes from: importing documents, results and
//! Wikidata, exporting the outline, and checking quotes and sources.

use crate::app::App;
use serde_json::json;
use std::path::Path;

impl App {
    pub(crate) fn cmd_import(&mut self, words: &[&str], _: u64) -> Result<(), String> {
        let usage = "import (from) wikidata and a file made by the wikidata tool";
        let (what, rest) = words.split_first().ok_or(usage)?;
        let file = rest.join(" ");
        match *what {
            "document" => self.import_document(&file),
            "results" => self.import_results(&file),
            "wikidata" if !file.is_empty() => self.import_wikidata(&file),
            _ => Err(usage.into()),
        }
    }

    /// Import a file the wikidata tool made: its facts arrive as claims.
    fn import_wikidata(&mut self, file: &str) -> Result<(), String> {
        let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        self.vet_wikidata(file)?;
        let hash = self.store_blob(&bytes)?;
        self.write_wikidata_import(file, &hash)?;
        self.screen.message = format!(
            "imported wikidata: {} bytes, blob {}",
            bytes.len(),
            &hash[..12]
        );
        Ok(())
    }

    fn vet_wikidata(&mut self, file: &str) -> Result<(), String> {
        self.engine(0)?
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

    pub(crate) fn cmd_export(&mut self, words: &[&str], _: u64) -> Result<(), String> {
        let (file, section) = self.export_args(words);
        if file.is_empty() {
            return Err("export (to) a file, and an item number for one section".into());
        }
        self.export(&file, section)
    }

    /// The file to export to, and the section: the last word, if it is an
    /// item's number and there are words before it.
    fn export_args(&self, words: &[&str]) -> (String, Option<u64>) {
        match words.split_last() {
            Some((last, rest))
                if !rest.is_empty() && self.record.outline.by_number(last).is_some() =>
            {
                (rest.join(" "), self.record.outline.by_number(last))
            }
            _ => (words.join(" "), None),
        }
    }

    pub(crate) fn cmd_check(&mut self, words: &[&str], target: u64) -> Result<(), String> {
        let ruling = match words.first().copied() {
            Some("quote") => self.check_quote(target)?,
            Some("source") => self.check_source(target)?,
            _ => return Err("check quote, or check source".into()),
        };
        self.screen.message = format!("{} check: {ruling}", words[0]);
        Ok(())
    }
}
