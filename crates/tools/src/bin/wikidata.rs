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

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let out = get("--out")
        .ok_or("usage: wikidata (--items Q1,Q2 | --min-sitelinks K --limit N) --out FILE")?;
    let items: Vec<String> = match (get("--items"), get("--min-sitelinks")) {
        (Some(list), _) => list
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        (None, Some(k)) => {
            let k: u32 = k.parse().map_err(|_| "--min-sitelinks takes a number")?;
            let n: u32 = get("--limit")
                .unwrap_or("100000".into())
                .parse()
                .map_err(|_| "--limit takes a number")?;
            let r = sparql(&top_query(k, n))?;
            rows(&r)
                .iter()
                .filter_map(|r| entity_id(r.get("item")?["value"].as_str()?).map(String::from))
                .collect()
        }
        _ => return Err("give --items or --min-sitelinks".into()),
    };
    eprintln!("{} items", items.len());
    let mut lines: Vec<String> = Vec::new();
    let mut mentioned: BTreeSet<String> = items.iter().cloned().collect();
    for (k, batch) in items.chunks(200).enumerate() {
        let r = sparql(&statements_query(batch))?;
        for row in rows(&r) {
            if let Some(p) = row.get("p").and_then(|p| p["value"].as_str()) {
                mentioned.insert(p.to_string());
            }
            if let Some(v) = row
                .get("v")
                .and_then(|v| v["value"].as_str())
                .and_then(entity_id)
            {
                mentioned.insert(v.to_string());
            }
        }
        lines.extend(statement_facts(&r));
        eprintln!(
            "statements: batch {} of {}",
            k + 1,
            items.len().div_ceil(200)
        );
    }
    let mentioned: Vec<String> = mentioned.into_iter().collect();
    for batch in mentioned.chunks(200) {
        lines.extend(label_facts(&sparql(&labels_query(batch))?));
    }
    let props: Vec<String> = mentioned
        .iter()
        .filter(|m| m.starts_with('P'))
        .cloned()
        .collect();
    for batch in props.chunks(200) {
        lines.extend(constraint_facts(&sparql(&constraints_query(batch))?));
    }
    lines.sort();
    lines.dedup();
    let mut f = std::fs::File::create(&out).map_err(|e| format!("{out}: {e}"))?;
    writeln!(
        f,
        "% A subset of Wikidata (CC0), written by crates/tools/src/bin/wikidata.rs."
    )
    .map_err(|e| e.to_string())?;
    for l in &lines {
        writeln!(f, "{l}").map_err(|e| e.to_string())?;
    }
    eprintln!("{} facts written to {out}", lines.len());
    Ok(())
}
