//! Milestone 6's acceptance checks (docs/plan.md, Milestone 6): documents
//! and results in and out.

use memex::app::{App, Event, Key, Quasi};
use memex::docs::{self, Block};
use std::path::{Path, PathBuf};

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("adversaria-m6-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn tap(app: &mut App, k: Key) {
    app.handle(Event::down(k));
    app.handle(Event::up(k));
}

fn command(app: &mut App, line: &str) {
    app.handle(Event::down(Key::Quasi(Quasi::Command)));
    for c in line.chars() {
        tap(app, Key::Char(c));
    }
    app.handle(Event::up(Key::Quasi(Quasi::Command)));
    let bad = [
        "no command",
        "not a",
        "cannot",
        "pandoc:",
        "check quote, or",
        "the target",
        "there is no",
    ];
    assert!(
        !bad.iter().any(|b| app.session.message.starts_with(b)),
        "{line}: {}",
        app.session.message
    );
}

fn typ(app: &mut App, text: &str) {
    for c in text.chars() {
        tap(app, Key::Char(c));
    }
}

/// Leap to the next item whose text holds a pattern; that item.
fn leap(app: &mut App, pattern: &str) -> u64 {
    app.handle(Event::down(Key::Quasi(Quasi::LeapForward)));
    typ(app, pattern);
    app.handle(Event::up(Key::Quasi(Quasi::LeapForward)));
    app.cursor()
}

fn open(dir: &Path) -> App {
    let mut app = App::open(&dir.join("journal")).unwrap();
    app.set_durable(false);
    app
}

const ESSAY: &str = r#"# How Buran's flight software was reviewed

The programme flew *once*, in 1988 [@buran1988], with energy $E = mc^2$ to spare.

## DRAKON

Charts read **top to bottom**; see [the manual](https://example.org/drakon).

$$\int_0^1 x \, dx = \frac{1}{2}$$

- the skewer
    - the further right, the worse
- silhouettes

1. first
2. second

> Reviewers read each chart in turn.

```
print("hello")
```

| Chart | Pages |
|---|---|
| launch | 12 |
| landing | 9 |

A footnote here.[^1] And an image: ![a chart](chart.png)

[^1]: Not kept.
"#;

/// The blocks an outline holds, read back from an import: style, depth and
/// text, the shape a round trip must keep.
fn shape(blocks: &[Block]) -> Vec<(String, usize, String)> {
    blocks
        .iter()
        .map(|b| (b.style.clone(), b.depth, b.text.clone()))
        .collect()
}

#[test]
fn markdown_round_trip_keeps_structure_citations_and_mathematics() {
    // Plan: "Markdown and Word round trips preserve headings, items,
    // citations and mathematics (as LaTeX); losses are listed, not hidden."
    let dir = temp("md");
    let src = write_essay(&dir, "essay.md");
    let mut app = open(&dir);
    command(&mut app, &format!("import document {}", src.display()));
    the_losses_are_listed_and_recorded(&app);
    citations_became_facts(&app);
    mathematics_stayed_latex(&app);
    // Export, and import the export into a fresh journal: the same shape.
    let out = exported(&mut app, &dir.join("out.md"));
    the_round_trip_keeps_the_shape(&src, &out);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The essay, written as Markdown to a file in a directory.
fn write_essay(dir: &Path, name: &str) -> PathBuf {
    let src = dir.join(name);
    std::fs::write(&src, ESSAY).unwrap();
    src
}

/// The import says what it lost, and records it.
fn the_losses_are_listed_and_recorded(app: &App) {
    let said = &app.session.message;
    assert!(said.contains("losses: footnote 1, image 1"), "{said}");
    let mut entries = app.journal().entries().iter();
    let imp = entries
        .find(|e| e.kind() == "import")
        .unwrap()
        .body()
        .unwrap();
    let lost = serde_json::json!({"footnote": 1, "image": 1});
    assert_eq!(imp["losses"], lost, "losses recorded");
}

/// Citations became facts.
fn citations_became_facts(app: &App) {
    let facts = memex::store::facts(app.journal().entries(), app.notebook.tip).unwrap();
    let shown = |f: &format::terms::Term| format::terms::encode(f).unwrap().to_string();
    assert!(facts.iter().any(|(_, f)| shown(f).contains("buran1988")));
}

/// Mathematics stayed LaTeX, displayed and inline.
fn mathematics_stayed_latex(app: &App) {
    let texts = outline_texts(app);
    let integral = "\\int_0^1 x \\, dx = \\frac{1}{2}".to_string();
    assert!(texts.contains(&integral), "{texts:?}");
    assert!(texts.iter().any(|t| t.contains("$E = mc^2$")));
}

/// The text of each item, in the outline's order.
fn outline_texts(app: &App) -> Vec<String> {
    let outline = &app.notebook.outline;
    outline
        .in_order()
        .iter()
        .map(|i| outline.text(*i))
        .collect()
}

/// Export to a file, which records its source entries: the file.
fn exported(app: &mut App, out: &Path) -> PathBuf {
    command(app, &format!("export {}", out.display()));
    let text = std::fs::read_to_string(out).unwrap();
    assert!(
        text.contains("source-entries:"),
        "the export records its source entries"
    );
    out.to_path_buf()
}

/// The export reads back as the source does, losing nothing more, and the
/// source is the essay: two headings, one formula, one citation.
fn the_round_trip_keeps_the_shape(src: &Path, out: &Path) {
    let (first, _, _) = docs::read(src).unwrap();
    let (second, losses, _) = docs::read(out).unwrap();
    assert_eq!(shape(&second), shape(&first));
    assert!(
        losses.is_empty(),
        "nothing more lost on the way back: {losses:?}"
    );
    let count = |style: &str| first.iter().filter(|b| b.style == style).count();
    assert_eq!((count("heading"), count("math")), (2, 1));
    assert_eq!(cited(&first), ["buran1988"]);
}

/// What blocks cite, in order.
fn cited(blocks: &[Block]) -> Vec<String> {
    blocks.iter().flat_map(|b| b.cites.clone()).collect()
}

#[test]
fn word_round_trip_keeps_structure_citations_and_mathematics() {
    // The same through Word: Markdown to .docx by Pandoc, imported, exported
    // to .docx by the memex, imported again.
    let dir = temp("docx");
    let md = write_essay(&dir, "essay.md");
    let docx = to_docx(&md, &dir.join("essay.docx"));
    let (said, out) = through_the_memex(&dir, &docx, "out.docx");
    word_keeps_the_shape(&docx, &out, &md);
    eprintln!("Word import: {said}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A Markdown file made into Word by Pandoc: the Word file.
fn to_docx(md: &Path, docx: &Path) -> PathBuf {
    let mut pandoc = std::process::Command::new("pandoc");
    pandoc
        .args(["-f", "markdown", "-t", "docx", "-o"])
        .arg(docx)
        .arg(md);
    assert!(pandoc.status().unwrap().success());
    docx.to_path_buf()
}

/// A document imported into a memex in a directory, then exported to a
/// file of a name there: what the import said, and the export.
fn through_the_memex(dir: &Path, doc: &Path, name: &str) -> (String, PathBuf) {
    let mut app = open(dir);
    command(&mut app, &format!("import document {}", doc.display()));
    let said = app.session.message.clone();
    let out = dir.join(name);
    command(&mut app, &format!("export {}", out.display()));
    (said, out)
}

/// The Word round trip keeps the outline, and against the Markdown
/// original, the same headings, items and citations, and the mathematics
/// as LaTeX.
fn word_keeps_the_shape(docx: &Path, out: &Path, md: &Path) {
    let (first, _, _) = docs::read(docx).unwrap();
    let (second, _, _) = docs::read(out).unwrap();
    assert_eq!(
        shape(&second),
        shape(&first),
        "the Word round trip changed the outline"
    );
    let (orig, _, _) = docs::read(md).unwrap();
    assert_eq!(styles(&first), styles(&orig), "Word lost structure");
    assert_eq!(headings(&first), headings(&orig));
    assert_eq!(cited(&first), ["buran1988"], "citations through Word");
    let math = first.iter().find(|b| b.style == "math").unwrap();
    let latex = math.text.contains("\\int") && math.text.contains("\\frac");
    assert!(latex, "{}", math.text);
}

/// Each block's style and depth.
fn styles(blocks: &[Block]) -> Vec<(String, usize)> {
    blocks.iter().map(|x| (x.style.clone(), x.depth)).collect()
}

/// The text of each heading.
fn headings(blocks: &[Block]) -> Vec<String> {
    let headings = blocks.iter().filter(|x| x.style == "heading");
    headings.map(|x| x.text.clone()).collect()
}

#[test]
fn a_revised_document_opens_a_world_of_changes() {
    // Plan: "Re-importing a revised document opens a world, and a table
    // lists each change to adopt or retract."
    let dir = temp("revise");
    let src = dir.join("essay.md");
    let mut app = open(&dir);
    import_text(
        &mut app,
        &src,
        "# Essay\n\nBuran flew in 1988.\n\nIt was unmanned.\n\nThe end.\n",
    );
    let actual = app.notebook.outline.clone();
    import_text(
        &mut app,
        &src,
        "# Essay\n\nBuran flew in 1989.\n\nThe end.\n\nA new closing line.\n",
    );
    the_revision_lists_each_change(&app);
    keep_all_but_the_deletion(&mut app);
    let kept = [
        "Essay",
        "Buran flew in 1989.",
        "It was unmanned.",
        "The end.",
        "A new closing line.",
    ];
    assert_eq!(outline_texts(&app), kept);
    assert_ne!(app.notebook.outline, actual);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Write a text to a file, and import it as a document.
fn import_text(app: &mut App, src: &Path, text: &str) {
    std::fs::write(src, text).unwrap();
    command(app, &format!("import document {}", src.display()));
}

/// A revision of essay.md is open, listing the change, the deletion and
/// the addition.
fn the_revision_lists_each_change(app: &App) {
    let sup = app.reason.supposition.as_ref();
    assert!(sup.is_some_and(|s| s.name == "revision of essay.md"));
    let changes = app.revision_changes();
    let whats: Vec<&str> = changes.iter().map(|c| c.1.as_str()).collect();
    for want in [
        "changed 1.1: “Buran flew in 1988.” → “Buran flew in 1989.”",
        "deleted 1.2: “It was unmanned.”",
        "added: “A new closing line.”",
    ] {
        assert!(whats.iter().any(|w| w.starts_with(want)), "{whats:?}");
    }
}

/// Keep all but the deletion: retract it, then commit.
fn keep_all_but_the_deletion(app: &mut App) {
    let changes = app.revision_changes();
    let deletion = changes
        .iter()
        .find(|c| c.1.starts_with("deleted"))
        .unwrap()
        .0;
    command(app, &format!("retract {deletion}"));
    tap(app, Key::Enter);
    command(app, "commit");
    assert!(app.reason.supposition.is_none());
}

/// A results file that an analysis of data.csv wrote with the Python
/// helper, imported.
fn imported_results(dir: &Path) -> App {
    std::fs::write(dir.join("data.csv"), "t,delay\n1,1.5\n2,1.6\n").unwrap();
    let helper = concat!(env!("CARGO_MANIFEST_DIR"), "/../../helpers/python");
    let script = format!(
        "import sys; sys.path.insert(0, {helper:?}); from memex_results import write_results; \
         write_results('results.json', [('mean_delay', 1.55, 0.05, 's')], ['data.csv'], script='analysis.py')"
    );
    let mut python = std::process::Command::new("python3");
    let run = python.args(["-c", &script]).current_dir(dir).status();
    assert!(run.unwrap().success());
    let mut app = open(dir);
    let file = dir.join("results.json");
    command(&mut app, &format!("import results {}", file.display()));
    app
}

#[test]
fn an_imported_result_keeps_its_source() {
    let dir = temp("results-source");
    let mut app = imported_results(&dir);
    let result = leap(&mut app, "mean_delay");
    let outline = &app.notebook.outline;
    assert!(outline.source[&result].starts_with("analysis.py@"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_ruling_names_the_blob_it_read() {
    let dir = temp("ruling-blob");
    let src = dir.join("talk.md");
    std::fs::write(&src, "# Talk\n\nThe 1989 flight was the only one.\n").unwrap();
    let mut app = open(&dir);
    command(&mut app, &format!("import document {}", src.display()));
    leap(&mut app, "The 1989");
    command(&mut app, "check quote");
    let ruling = last_body(&app)["facts"][0].to_string();
    assert!(ruling.contains(last_blob(&app).as_str()), "{ruling}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The body of the last entry.
fn last_body(app: &App) -> serde_json::Value {
    app.journal()
        .entries()
        .last()
        .unwrap()
        .body()
        .unwrap()
        .clone()
}

/// The blob of the last import.
fn last_blob(app: &App) -> String {
    let mut imports = app
        .journal()
        .entries()
        .iter()
        .filter(|e| e.kind() == "import");
    imports.next_back().unwrap().body().unwrap()["blob"]
        .as_str()
        .unwrap()
        .into()
}

#[test]
fn a_changed_input_marks_its_results() {
    // Plan: "A changed input file marks the results that depended on it."
    // The results file is written by the Python helper an analysis calls.
    let dir = temp("results");
    let mut app = imported_results(&dir);
    app.evaluate().unwrap();
    assert!(app.session.stale().is_empty(), "nothing has changed yet");
    let result = leap(&mut app, "mean_delay");
    assert_eq!(
        app.notebook.outline.text(result),
        "mean_delay = 1.55 ± 0.05 s"
    );
    let facts = memex::store::facts(app.journal().entries(), app.notebook.tip).unwrap();
    assert!(
        facts
            .iter()
            .any(|(_, f)| memex::reason::show(f).contains("measured(1.55, 0.05, \"s\")")),
        "{facts:?}"
    );
    std::fs::write(dir.join("data.csv"), "t,delay\n1,1.5\n2,1.9\n").unwrap();
    app.evaluate().unwrap();
    assert_eq!(
        app.session.stale().get(&result),
        Some(&vec!["data.csv".to_string()])
    );
    let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(140, 30)).unwrap();
    t.draw(|f| memex::ui::draw(f, &app)).unwrap();
    let buf = t.backend().buffer().clone();
    let shown = (0..30)
        .map(|y| {
            (0..140)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(shown.contains("⚑ input changed since: data.csv"), "{shown}");
    let lines: Vec<&str> = shown.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.contains("• mean_delay = 1.55"))
        .unwrap();
    assert!(
        lines[at + 1].contains("⚑ input changed since"),
        "under its result"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_quote_check() {
    // § Testing claims, the quote: quoted while the text is in the source
    // (spaces, hyphens and case aside), misquoted once it is not.
    let dir = temp("quote");
    let src = dir.join("talk.md");
    std::fs::write(&src, "# Talk\n\nThe 1989 flight was the only one.\n").unwrap();
    let mut app = open(&dir);
    command(&mut app, &format!("import document {}", src.display()));
    let item = leap(&mut app, "The 1989");
    command(&mut app, "check quote");
    assert_eq!(app.session.message, "quote check: quoted");
    tap(&mut app, Key::Home);
    for _ in 0.."The ".len() {
        tap(&mut app, Key::Right);
    }
    typ(&mut app, "first ");
    app.flush();
    command(&mut app, "check quote");
    assert_eq!(app.session.message, "quote check: misquoted");
    let e = app.journal().entries().last().unwrap();
    assert!(
        memex::reason::show(&format::terms::decode(&e.body().unwrap()["facts"][0]).unwrap())
            .starts_with(&format!("ruling({item}, quote, misquoted, "))
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A local server that stands in for Crossref, holding these records by
/// their DOI under 10.5555/, and its port.
fn crossref(dir: &Path, records: &[(&str, &str)]) -> (std::process::Child, u16) {
    let www = dir.join("www");
    std::fs::create_dir_all(www.join("10.5555")).unwrap();
    for (name, record) in records {
        std::fs::write(www.join("10.5555").join(name), record).unwrap();
    }
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let server = http_server(&www, port);
    wait_until_listening(port);
    (server, port)
}

/// Python's web server of a directory, on a port.
fn http_server(www: &Path, port: u16) -> std::process::Child {
    let mut python = std::process::Command::new("python3");
    let args = [
        "-m",
        "http.server",
        &port.to_string(),
        "--bind",
        "127.0.0.1",
    ];
    let quiet = std::process::Stdio::null;
    python
        .args(args)
        .current_dir(www)
        .stdout(quiet())
        .stderr(quiet())
        .spawn()
        .unwrap()
}

/// Wait until a server listens on a port: a fixed pause was too short
/// under the load of the whole suite (seen in nix develop: "unavailable").
fn wait_until_listening(port: u16) {
    let listening = || std::net::TcpStream::connect(("127.0.0.1", port)).is_ok();
    let up = (0..200).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(50));
        listening()
    });
    assert!(up, "the fake Crossref did not start within 10 s");
}

/// A memex whose source check asks the fake Crossref at a port.
fn asking(dir: &Path, port: u16) -> App {
    let mut app = open(dir);
    app.session
        .set_crossref(&format!("http://127.0.0.1:{port}/"));
    app
}

/// The source check's ruling on a new claim with a DOI and a title.
fn ruling(app: &mut App, doi: &str, title: &str) -> String {
    tap(app, Key::Enter);
    typ(app, "a claim");
    app.flush();
    command(app, &format!("fact doi(paper, \"{doi}\")"));
    command(app, &format!("fact title(paper, \"{title}\")"));
    command(app, "check source");
    let said = app.session.message.as_str();
    said.trim_start_matches("source check: ").to_string()
}

const GOOD: (&str, &str) = (
    "good",
    r#"{"status":"ok","message":{"title":["Buran's flight software"]}}"#,
);

#[test]
fn the_source_check() {
    // § Testing claims, the source: fetched by DOI, never trusting a copy
    // that came with the claim. A local server stands in for Crossref.
    let dir = temp("source");
    let retracted =
        r#"{"status":"ok","message":{"title":["Bad"],"updated-by":[{"type":"retraction"}]}}"#;
    let (mut server, port) = crossref(&dir, &[GOOD, ("retracted", retracted)]);
    let mut app = asking(&dir, port);
    let alike = ruling(&mut app, "10.5555/good", "buran's flight-software");
    assert_eq!(
        alike, "found",
        "title alike, spaces, hyphens and case aside"
    );
    assert_eq!(
        ruling(&mut app, "10.5555/good", "Something else"),
        "misattributed"
    );
    assert_eq!(ruling(&mut app, "10.5555/retracted", "Bad"), "retracted");
    assert_eq!(ruling(&mut app, "10.5555/missing", "x"), "not_found");
    server.kill().unwrap();
    server.wait().unwrap();
    assert_eq!(ruling(&mut app, "10.5555/good", "x"), "unavailable");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_source_retracted_by_a_relation_is_retracted() {
    let dir = temp("source-relation");
    let record = r#"{"message":{"title":["Bad"],"relation":{"is-retracted-by":[{"id":"x"}]}}}"#;
    let (mut server, port) = crossref(&dir, &[("bad", record)]);
    let said = ruling(&mut asking(&dir, port), "10.5555/bad", "Bad");
    server.kill().unwrap();
    assert_eq!(said, "retracted");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_record_that_is_no_json_is_unavailable() {
    let dir = temp("source-garbled");
    let (mut server, port) = crossref(&dir, &[("garbled", "<html>")]);
    let said = ruling(&mut asking(&dir, port), "10.5555/garbled", "x");
    server.kill().unwrap();
    assert_eq!(said, "unavailable");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_source_ruling_names_the_hash_of_the_record() {
    let dir = temp("source-hash");
    let (mut server, port) = crossref(&dir, &[GOOD]);
    let mut app = asking(&dir, port);
    ruling(&mut app, "10.5555/good", "x");
    server.kill().unwrap();
    let hash = format::record::sha256_hex(GOOD.1.as_bytes());
    assert!(last_body(&app)["facts"][0].to_string().contains(&hash));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_quote_check_reads_past_markup() {
    // Catches the item's Markdown compared with the document's plain text.
    let dir = temp("quote-markup");
    let src = dir.join("talk.md");
    std::fs::write(&src, "# Talk\n\nThe *1989* flight was the only one.\n").unwrap();
    let mut app = open(&dir);
    command(&mut app, &format!("import document {}", src.display()));
    leap(&mut app, "The *1989*");
    command(&mut app, "check quote");
    assert_eq!(app.session.message, "quote check: quoted");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// An essay of one line, imported, then revised and imported again: its
/// revision opens a world.
fn revised(dir: &Path) -> App {
    let src = dir.join("essay.md");
    std::fs::write(&src, "# Essay\n\nBuran flew in 1988.\n").unwrap();
    let mut app = open(dir);
    command(&mut app, &format!("import document {}", src.display()));
    std::fs::write(&src, "# Essay\n\nBuran flew in 1989.\n").unwrap();
    command(&mut app, &format!("import document {}", src.display()));
    app
}

#[test]
fn a_revision_lists_only_its_own_changes() {
    // Catches the edits of the actual world listed among the revision's.
    let dir = temp("revise-own");
    let changes = revised(&dir).revision_changes();
    assert_eq!(changes.len(), 1, "{changes:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_retracted_change_of_a_revision_is_marked() {
    let dir = temp("revise-retracted");
    let mut app = revised(&dir);
    let change = app.revision_changes()[0].0;
    command(&mut app, &format!("retract {change}"));
    tap(&mut app, Key::Enter);
    assert!(app.revision_changes()[0].2, "the change is retracted");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_changed_input_marks_only_its_own_results() {
    // Catches the items of every import marked when one import's input
    // changes.
    let dir = temp("results-own");
    let mut app = imported_results(&dir);
    std::fs::write(dir.join("talk.md"), "# Talk\n\nIt flew.\n").unwrap();
    command(
        &mut app,
        &format!("import document {}", dir.join("talk.md").display()),
    );
    std::fs::write(dir.join("data.csv"), "t,delay\n1,1.5\n2,1.9\n").unwrap();
    app.evaluate().unwrap();
    let flew = leap(&mut app, "It flew.");
    assert!(!app.session.stale().contains_key(&flew));
    std::fs::remove_dir_all(&dir).unwrap();
}
