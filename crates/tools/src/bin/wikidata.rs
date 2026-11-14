//! wikidata --items Q2548,Q15180 --out subset.pl
//! wikidata --min-sitelinks 60 --limit 100000 --out subset.pl
//!
//! Write a subset of Wikidata as Prolog facts (see the crate's comment): the
//! items' truthy statements without external identifiers, the English labels
//! of the items and of the properties and items they use, and the
//! properties' constraints. Queries go in batches of 200 items.

use std::collections::BTreeSet;
use std::io::Write;
use tools::*;

fn main() {
    if let Err(e) = run() {
        eprintln!("wikidata: {e}");
        std::process::exit(1);
    }
}

const USAGE: &str = "usage: wikidata (--items Q1,Q2 | --min-sitelinks K --limit N) --out FILE";

fn run() -> Result<(), String> {
    let args = Args(std::env::args().skip(1).collect());
    let out = args.get("--out").ok_or(USAGE)?;
    let items = items_of(&args)?;
    eprintln!("{} items", items.len());
    let mut facts = Facts::of(&items);
    facts.statements(&items)?;
    facts.labels()?;
    facts.constraints()?;
    facts.write(&out)
}

/// The command's arguments.
struct Args(Vec<String>);

impl Args {
    /// The value after a flag, if it is given.
    fn get(&self, flag: &str) -> Option<String> {
        let at = self.0.iter().position(|a| a == flag)?;
        self.0.get(at + 1).cloned()
    }
}

/// The items named, or the items with the most sitelinks.
fn items_of(args: &Args) -> Result<Vec<String>, String> {
    match (args.get("--items"), args.get("--min-sitelinks")) {
        (Some(list), _) => Ok(listed(&list)),
        (None, Some(k)) => top_items(args, &k),
        _ => Err("give --items or --min-sitelinks".into()),
    }
}

/// The items of a list such as Q2548,Q15180.
fn listed(list: &str) -> Vec<String> {
    let items = list.split(',').map(|s| s.trim().to_string());
    items.filter(|s| !s.is_empty()).collect()
}

/// The items with at least k sitelinks, at most --limit of them.
fn top_items(args: &Args, k: &str) -> Result<Vec<String>, String> {
    let k: u32 = k.parse().map_err(|_| "--min-sitelinks takes a number")?;
    let limit = args.get("--limit").unwrap_or("100000".into());
    let n: u32 = limit.parse().map_err(|_| "--limit takes a number")?;
    let r = sparql(&top_query(k, n))?;
    let item = |r: &&serde_json::Map<String, serde_json::Value>| {
        entity_id(r.get("item")?["value"].as_str()?).map(String::from)
    };
    Ok(rows(&r).iter().filter_map(item).collect())
}

/// The facts found so far, and every item and property they mention.
struct Facts {
    lines: Vec<String>,
    mentioned: BTreeSet<String>,
}

impl Facts {
    fn of(items: &[String]) -> Facts {
        let mentioned = items.iter().cloned().collect();
        Facts {
            lines: Vec::new(),
            mentioned,
        }
    }

    /// The items' statements, in batches of 200.
    fn statements(&mut self, items: &[String]) -> Result<(), String> {
        let batches = items.len().div_ceil(200);
        for (k, batch) in items.chunks(200).enumerate() {
            let r = sparql(&statements_query(batch))?;
            self.mention(&r);
            self.lines.extend(statement_facts(&r));
            eprintln!("statements: batch {} of {batches}", k + 1);
        }
        Ok(())
    }

    /// The property, and the item if the value is one, of each row.
    fn mention(&mut self, r: &serde_json::Value) {
        for row in rows(r) {
            self.mentioned.extend(mentions(row));
        }
    }

    /// The English labels of everything mentioned.
    fn labels(&mut self) -> Result<(), String> {
        let mentioned: Vec<String> = self.mentioned.iter().cloned().collect();
        for batch in mentioned.chunks(200) {
            self.lines
                .extend(label_facts(&sparql(&labels_query(batch))?));
        }
        Ok(())
    }

    /// The constraints of every property mentioned.
    fn constraints(&mut self) -> Result<(), String> {
        let props: Vec<String> = self
            .mentioned
            .iter()
            .filter(|m| m.starts_with('P'))
            .cloned()
            .collect();
        for batch in props.chunks(200) {
            self.lines
                .extend(constraint_facts(&sparql(&constraints_query(batch))?));
        }
        Ok(())
    }

    /// The facts, sorted and each once, to a file.
    fn write(mut self, out: &str) -> Result<(), String> {
        self.lines.sort();
        self.lines.dedup();
        let mut f = std::fs::File::create(out).map_err(|e| format!("{out}: {e}"))?;
        let header = "% A subset of Wikidata (CC0), written by crates/tools/src/bin/wikidata.rs.";
        writeln!(f, "{header}").map_err(|e| e.to_string())?;
        for l in &self.lines {
            writeln!(f, "{l}").map_err(|e| e.to_string())?;
        }
        eprintln!("{} facts written to {out}", self.lines.len());
        Ok(())
    }
}

/// A statement's property, and its value if that is an item.
fn mentions(row: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let p = row.get("p").and_then(|p| p["value"].as_str());
    let v = row
        .get("v")
        .and_then(|v| v["value"].as_str())
        .and_then(entity_id);
    p.into_iter().chain(v).map(String::from).collect()
}
