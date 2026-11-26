//! interactivity [ROOT]: measure how many things each unit of the project's
//! code and documentation refers to (crates/tools/src/interactivity.rs says
//! exactly what is counted), and list every unit over the limit of 4.
//! Exit status 0 always: this reports; it does not judge.

use std::path::Path;
use tools::interactivity::*;
use tools::{followed, held};

const LIMIT: usize = 4;

/// The Prolog files measured.
const PROLOG: [&str; 7] = [
    "prolog/protocol.pl",
    "prolog/server.pl",
    "prolog/dataflow.pl",
    "prolog/ace.pl",
    "prolog/ace_server.pl",
    "prolog/ace_next.pl",
    "prolog/ace_read.pl",
];

/// The documents measured, where they are there.
const DOCS: [&str; 9] = [
    "README.md",
    "docs/introduction.md",
    "docs/tutorial.md",
    "docs/guide.md",
    "docs/design.md",
    "docs/plan.md",
    "docs/log.md",
    "docs/translator.md",
    "docs/structure.md",
];

/// The project's Rust, measured: the functions of its sources and of its
/// tests, and its source files as modules.
struct Rust {
    fns: Vec<Unit>,
    tests: Vec<Unit>,
    modules: Vec<Unit>,
}

fn main() {
    let root = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let root = Path::new(&root);
    let (rust, blind) = rust_units(root);
    let (preds, sections) = prolog_and_markdown(root);
    print_held_summaries(&rust);
    let groups = groups(&rust, &preds, &sections);
    print_summaries(&groups, &blind);
    report_held("Rust functions (src)", &rust.fns);
    report_held("Rust functions (tests)", &rust.tests);
    report_followed(&rust.fns);
    report_over(&groups[2..]);
    report_sections(&sections);
}

/// The Prolog predicates and the Markdown sections, measured.
fn prolog_and_markdown(root: &Path) -> (Vec<Unit>, Vec<Unit>) {
    let preds = prolog_units(root);
    (preds, measure_markdown(&docs_of(root)))
}

/// A path as the report names it: from the root.
fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string()
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

/// The workspace's crates, by name.
fn crates_of(root: &Path) -> Vec<String> {
    let names = |d: std::fs::ReadDir| {
        d.flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect()
    };
    std::fs::read_dir(root.join("crates"))
        .map(names)
        .unwrap_or_default()
}

/// The Rust files of the crates, by name and text: the sources, and the
/// tests.
type Sources = Vec<(String, String)>;

fn rust_sources(root: &Path) -> (Sources, Sources) {
    let files = rust_files(&root.join("crates")).into_iter();
    let named = files.map(|p| (rel(root, &p), read(&p)));
    named.partition(|(n, _)| !n.contains("/tests/") && !n.ends_with("tests.rs"))
}

/// The Rust measured, and what the measure could not see; or the error,
/// and exit 1.
fn rust_units(root: &Path) -> (Rust, Blind) {
    let (src, tests) = rust_sources(root);
    let all: Sources = src.into_iter().chain(tests.iter().cloned()).collect();
    let (mut rust, blind) = Rust::split(measure_or_exit(&all, &crates_of(root)), &tests);
    let counts = followed::followed(&rust.fns);
    for (u, n) in rust.fns.iter_mut().zip(counts) {
        u.followed = n;
    }
    (rust, blind)
}

/// The functions measured, the files as modules, and what the measure
/// could not see.
type Measured = (Vec<Unit>, Vec<Unit>, Blind);

fn measure_or_exit(all: &[(String, String)], crates: &[String]) -> Measured {
    match measure_rust(all, crates) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("interactivity: {e}");
            std::process::exit(1);
        }
    }
}

impl Rust {
    /// The units of the test files apart from the others; and only the
    /// source files as modules.
    fn split((fns, modules, blind): Measured, test_files: &[(String, String)]) -> (Rust, Blind) {
        let modules = modules
            .into_iter()
            .filter(|u| !in_files(u, test_files))
            .collect();
        let (tests, fns) = split_off(fns, test_files);
        (
            Rust {
                fns,
                tests,
                modules,
            },
            blind,
        )
    }
}

/// Whether a unit is of one of these files.
fn in_files(u: &Unit, files: &[(String, String)]) -> bool {
    files.iter().any(|(n, _)| *n == u.file)
}

/// The units of these files, and the rest.
fn split_off(units: Vec<Unit>, files: &[(String, String)]) -> (Vec<Unit>, Vec<Unit>) {
    units.into_iter().partition(|u| in_files(u, files))
}

/// The predicates of the Prolog files, from SWI-Prolog's cross-reference;
/// none if it cannot be run.
fn prolog_units(root: &Path) -> Vec<Unit> {
    let files = PROLOG.map(|p| root.join(p).display().to_string());
    let xref = std::process::Command::new("swipl")
        .arg("-q")
        .args(["-g", "main", "-t", "halt"])
        .arg(root.join("crates/tools/prolog/measure_xref.pl"))
        .arg("--")
        .args(&files)
        .output();
    match xref {
        Ok(o) if o.status.success() => {
            from_root(root, parse_xref(&String::from_utf8_lossy(&o.stdout)))
        }
        Ok(o) => {
            let why = String::from_utf8_lossy(&o.stderr);
            eprintln!(
                "interactivity: the Prolog cross-reference failed ({}): {why}",
                o.status
            );
            vec![]
        }
        Err(e) => {
            eprintln!("interactivity: cannot run swipl: {e}");
            vec![]
        }
    }
}

/// Units with their files named from the root.
fn from_root(root: &Path, units: Vec<Unit>) -> Vec<Unit> {
    units.into_iter().map(|u| renamed(root, u)).collect()
}

/// A unit with its file named from the root.
fn renamed(root: &Path, mut u: Unit) -> Unit {
    u.file = rel(root, Path::new(&u.file));
    u
}

/// The documents, by name and text, that are there.
fn docs_of(root: &Path) -> Vec<(String, String)> {
    let docs = DOCS.iter().map(|p| (p.to_string(), read(&root.join(p))));
    docs.filter(|(_, t)| !t.is_empty()).collect()
}

fn print_held_summaries(rust: &Rust) {
    println!("Held at a statement: what a function's reader holds, read in order (limit {LIMIT}).");
    println!("Rules: crates/tools/src/held.rs. This is the measure for code.");
    println!();
    println!("Rust functions (src): {}", held::summary(&rust.fns, LIMIT));
    println!(
        "Rust functions (tests): {}",
        held::summary(&rust.tests, LIMIT)
    );
    println!(
        "Rust functions (src), by callers: {}",
        by_callers(&rust.fns)
    );
    println!(
        "Rust functions (src), helpers used once: {}",
        followed_summary(&rust.fns)
    );
    println!();
}

/// How many functions one, two, or three or more call (docs/structure.md,
/// rule 5), none that the measure sees, and how many share their name.
fn by_callers(fns: &[Unit]) -> String {
    let callers = followed::callers(fns);
    let known: Vec<usize> = callers.iter().flatten().map(Vec::len).collect();
    let count = |f: fn(usize) -> bool| known.iter().filter(|n| f(**n)).count();
    format!(
        "{} by one, {} by two, {} by three or more; {} by none seen; {} share a name, callers not known",
        count(|n| n == 1),
        count(|n| n == 2),
        count(|n| n >= 3),
        count(|n| n == 0),
        callers.len() - known.len()
    )
}

/// How many functions have a helper used once, how many have more than
/// the limit, and the most; and the deepest chain of them.
fn followed_summary(fns: &[Unit]) -> String {
    let some = fns.iter().filter(|u| u.followed.0 > 0).count();
    let over = fns.iter().filter(|u| u.followed.0 > LIMIT).count();
    let most = fns.iter().max_by_key(|u| u.followed.0);
    let deepest = fns.iter().max_by_key(|u| u.followed.1);
    let most = most.map_or(String::new(), |u| {
        format!("; most {} ({})", u.followed.0, where_of(u))
    });
    let deepest = deepest.map_or(String::new(), |u| {
        format!("; deepest {} ({})", u.followed.1, where_of(u))
    });
    format!("{some} functions have one; {over} more than {LIMIT}{most}{deepest}")
}

/// The five groups of units, by title.
fn groups<'a>(
    rust: &'a Rust,
    preds: &'a [Unit],
    sections: &'a [Unit],
) -> [(&'static str, &'a [Unit]); 5] {
    [
        ("Rust functions (src)", &rust.fns),
        ("Rust functions (tests)", &rust.tests),
        ("Rust modules (src files)", &rust.modules),
        ("Prolog predicates", preds),
        ("Markdown sections", sections),
    ]
}

fn print_summaries(groups: &[(&str, &[Unit])], blind: &Blind) {
    println!("Interactivity: distinct project referents per unit (limit {LIMIT}).");
    println!(
        "Rules: crates/tools/src/interactivity.rs. Language and standard libraries are assumed known."
    );
    println!();
    for (title, units) in groups {
        println!("{title}: {}", summary(units, LIMIT));
    }
    println!(
        "Ambiguous: {} method calls by names both std and project (in the upper bounds only). Not seen: {} macros whose arguments were not expressions.",
        blind.std_calls_skipped, blind.macros_unparsed
    );
}

/// The functions that hold more than the limit at some statement, the
/// highest first: the peak (to its upper bound), where, and the count of
/// the function's distinct referents.
fn report_held(title: &str, units: &[Unit]) {
    let mut over = held_over(units);
    over.sort_by_key(|u| std::cmp::Reverse(held::lower(u)));
    println!();
    println!("== {title}: hold more than {LIMIT} at a statement, highest first");
    for u in over {
        println!("{}", held_row(u));
    }
}

/// The functions with more helpers used once than the limit, or helpers
/// deeper than it, the most first: helpers, then depth. A report, not a
/// limit: the author sets one.
fn report_followed(fns: &[Unit]) {
    let over = |u: &&Unit| u.followed.0 > LIMIT || u.followed.1 > LIMIT;
    let mut over: Vec<&Unit> = fns.iter().filter(over).collect();
    over.sort_by_key(|u| std::cmp::Reverse(u.followed));
    println!();
    println!("== Rust functions (src): more than {LIMIT} helpers used once, or deeper, most first");
    for u in over {
        let (n, deep) = u.followed;
        println!("{n:>5} deep {deep:>2}  {}", where_of(u));
    }
}

/// The units that may hold more than the limit at a statement.
fn held_over(units: &[Unit]) -> Vec<&Unit> {
    units.iter().filter(|u| held::upper(u) > LIMIT).collect()
}

fn held_row(u: &Unit) -> String {
    let Some(h) = &u.held else {
        return String::new();
    };
    let (helpers, deep) = u.followed;
    let at = format!(
        "(statement {}; {} referents in all; {helpers} helpers used once, {deep} deep)",
        h.at,
        u.load()
    );
    let row = format!("{:>5}  {}  {at}", range_of(h), where_of(u));
    if std::env::var_os("INTERACTIVITY_DETAIL").is_none() {
        return row;
    }
    format!("{row}\n       {}", things_of(h))
}

/// A peak, to its upper bound if that is higher.
fn range_of(h: &held::Held) -> String {
    let peak = h.things.len();
    if h.upper > peak {
        return format!("{peak}–{}", h.upper);
    }
    peak.to_string()
}

/// A unit's file and its name.
fn where_of(u: &Unit) -> String {
    format!("{}  {}", u.file, u.name)
}

fn things_of(h: &held::Held) -> String {
    let things: Vec<&str> = h.things.iter().map(String::as_str).collect();
    things.join(", ")
}

/// Each group's units over the limit, or with more children than it,
/// largest first.
fn report_over(groups: &[(&str, &[Unit])]) {
    for (title, units) in groups {
        let mut over = over_limit(units);
        over.sort_by_key(|u| std::cmp::Reverse(u.load()));
        println!();
        println!("== {title}: over {LIMIT}, largest first");
        for u in over {
            println!("{}", over_row(u));
        }
    }
}

/// The units over the limit, or with more children than it.
fn over_limit(units: &[Unit]) -> Vec<&Unit> {
    units
        .iter()
        .filter(|u| u.upper() > LIMIT || u.children > LIMIT)
        .collect()
}

fn over_row(u: &Unit) -> String {
    let load = format!("{:>3}{:<4}", u.load(), upper_suffix(u));
    let row = format!("{load} {}  {}{}", u.file, u.name, children_suffix(u));
    if std::env::var_os("INTERACTIVITY_DETAIL").is_none() {
        return row;
    }
    format!("{row}\n       {}{}", referents_shown(u), ambiguous_shown(u))
}

/// A unit's upper bound, where it is higher than its load.
fn upper_suffix(u: &Unit) -> String {
    if u.upper() > u.load() {
        return format!("–{}", u.upper());
    }
    String::new()
}

/// How many children a unit has, where they are more than the limit.
fn children_suffix(u: &Unit) -> String {
    if u.children > LIMIT {
        return format!("  [{} children]", u.children);
    }
    String::new()
}

/// A unit's first twelve referents, and how many more.
fn referents_shown(u: &Unit) -> String {
    let shown: Vec<&str> = u.referents.iter().map(String::as_str).take(12).collect();
    let more = u.load().saturating_sub(12);
    let more = if more > 0 {
        format!(", … {more} more")
    } else {
        String::new()
    };
    format!("{}{more}", shown.join(", "))
}

/// A unit's calls by names both std's and the project's, if it has any.
fn ambiguous_shown(u: &Unit) -> String {
    if u.ambiguous.is_empty() {
        return String::new();
    }
    let a: Vec<&str> = u.ambiguous.iter().map(String::as_str).collect();
    format!("\n       ambiguous (std or project): {}", a.join(", "))
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
