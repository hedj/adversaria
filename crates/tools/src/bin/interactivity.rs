//! interactivity [ROOT]: measure how many things each unit of the project's
//! code and documentation refers to (crates/tools/src/interactivity.rs says
//! exactly what is counted), and list every unit over the limit of 4.
//! Exit status 0 always: this reports; it does not judge.

use std::path::Path;
use tools::interactivity::*;

const LIMIT: usize = 4;

fn main() {
    let root = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let root = Path::new(&root);
    let crates: Vec<String> = std::fs::read_dir(root.join("crates"))
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    let rel = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    let read = |p: &Path| std::fs::read_to_string(p).unwrap_or_default();
    let (src, tests): (Vec<_>, Vec<_>) = rust_files(&root.join("crates"))
        .into_iter()
        .map(|p| (rel(&p), read(&p)))
        .partition(|(n, _)| !n.contains("/tests/") && !n.ends_with("tests.rs"));
    let mut all = src.clone();
    all.extend(tests.clone());
    let (fns, modules, blind) = match measure_rust(&all, &crates) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("interactivity: {e}");
            std::process::exit(1);
        }
    };
    let is_test = |u: &Unit| tests.iter().any(|(n, _)| *n == u.file);
    let (test_fns, src_fns): (Vec<Unit>, Vec<Unit>) = fns.into_iter().partition(is_test);
    let src_modules: Vec<Unit> = modules.into_iter().filter(|u| !is_test(u)).collect();
    let pl: Vec<String> = [
        "prolog/protocol.pl",
        "prolog/server.pl",
        "prolog/dataflow.pl",
    ]
    .iter()
    .map(|p| root.join(p).display().to_string())
    .collect();
    let xref = std::process::Command::new("swipl")
        .arg("-q")
        .args(["-g", "main", "-t", "halt"])
        .arg(root.join("crates/tools/prolog/measure_xref.pl"))
        .arg("--")
        .args(&pl)
        .output();
    let preds: Vec<Unit> = match xref {
        Ok(o) => parse_xref(&String::from_utf8_lossy(&o.stdout))
            .into_iter()
            .map(|mut u| {
                u.file = rel(Path::new(&u.file));
                u
            })
            .collect(),
        Err(e) => {
            eprintln!("interactivity: cannot run swipl: {e}");
            vec![]
        }
    };
    let docs: Vec<(String, String)> = [
        "README.md",
        "docs/tutorial.md",
        "docs/guide.md",
        "docs/design.md",
        "docs/plan.md",
        "docs/log.md",
        "docs/translator.md",
        "docs/structure.md",
    ]
    .iter()
    .map(|p| (p.to_string(), read(&root.join(p))))
    .filter(|(_, t)| !t.is_empty())
    .collect();
    let sections = measure_markdown(&docs);

    println!("Interactivity: distinct project referents per unit (limit {LIMIT}).");
    println!(
        "Rules: crates/tools/src/interactivity.rs. Language and standard libraries are assumed known."
    );
    println!();
    for (title, units) in [
        ("Rust functions (src)", &src_fns),
        ("Rust functions (tests)", &test_fns),
        ("Rust modules (src files)", &src_modules),
        ("Prolog predicates", &preds),
        ("Markdown sections", &sections),
    ] {
        println!("{title}: {}", summary(units, LIMIT));
    }
    println!(
        "Ambiguous: {} method calls by names both std and project (in the upper bounds only). Not seen: {} macros whose arguments were not expressions.",
        blind.std_calls_skipped, blind.macros_unparsed
    );
    for (title, units) in [
        ("Rust functions (src)", &src_fns),
        ("Rust modules (src files)", &src_modules),
        ("Prolog predicates", &preds),
        ("Markdown sections", &sections),
        ("Rust functions (tests)", &test_fns),
    ] {
        let mut over: Vec<&Unit> = units
            .iter()
            .filter(|u| u.upper() > LIMIT || u.children > LIMIT)
            .collect();
        over.sort_by_key(|u| std::cmp::Reverse(u.load()));
        println!();
        println!("== {title}: over {LIMIT}, largest first");
        for u in over {
            let shown: Vec<&str> = u.referents.iter().map(String::as_str).take(12).collect();
            let more = u.load().saturating_sub(12);
            println!(
                "{:>3}{:<4} {}  {}{}{}",
                u.load(),
                if u.upper() > u.load() {
                    format!("–{}", u.upper())
                } else {
                    String::new()
                },
                u.file,
                u.name,
                if u.children > LIMIT {
                    format!("  [{} children]", u.children)
                } else {
                    String::new()
                },
                if std::env::var_os("INTERACTIVITY_DETAIL").is_some() {
                    format!(
                        "\n       {}{}{}",
                        shown.join(", "),
                        if more > 0 {
                            format!(", … {more} more")
                        } else {
                            String::new()
                        },
                        if u.ambiguous.is_empty() {
                            String::new()
                        } else {
                            let a: Vec<&str> = u.ambiguous.iter().map(String::as_str).collect();
                            format!("\n       ambiguous (std or project): {}", a.join(", "))
                        }
                    )
                } else {
                    String::new()
                }
            );
        }
    }
    report_sections(&sections);
}

/// The Markdown sections that refer forward, or that are over the limit
/// only when each term defined together counts apart.
fn report_sections(sections: &[Unit]) {
    println!();
    println!("== Markdown sections: refer forward, or over {LIMIT} counting each term apart");
    for u in sections {
        let strict = u.strict.unwrap_or(0);
        if u.forward > 0 || strict > LIMIT {
            println!(
                "  forward {}  strict {:>2}  {}  {}",
                u.forward, strict, u.file, u.name
            );
        }
    }
}
