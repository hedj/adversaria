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
        !bad.iter().any(|b| app.message.starts_with(b)),
        "{line}: {}",
        app.message
    );
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
    let src = dir.join("essay.md");
    std::fs::write(&src, ESSAY).unwrap();
    let mut app = open(&dir);
    command(&mut app, &format!("import document {}", src.display()));
    assert!(
        app.message.contains("losses: footnote 1, image 1"),
        "{}",
        app.message
    );
    let imp = app
        .journal()
        .entries()
        .iter()
        .find(|e| e.kind == "import")
        .unwrap()
        .clone();
    assert_eq!(
        imp.body.as_ref().unwrap()["losses"],
        serde_json::json!({"footnote": 1, "image": 1}),
        "losses recorded"
    );
    // Citations became facts; mathematics stayed LaTeX.
    let facts = memex::store::facts(app.journal().entries(), app.tip).unwrap();
    assert!(facts.iter().any(|(_, f)| {
        format::terms::encode(f)
            .unwrap()
            .to_string()
            .contains("buran1988")
    }));
    let texts: Vec<String> = app
        .outline
        .in_order()
        .iter()
        .map(|i| app.outline.text(*i))
        .collect();
    assert!(
        texts.contains(&"\\int_0^1 x \\, dx = \\frac{1}{2}".to_string()),
        "{texts:?}"
    );
    assert!(texts.iter().any(|t| t.contains("$E = mc^2$")));
    // Export, and import the export into a fresh journal: the same shape.
    let out = dir.join("out.md");
    command(&mut app, &format!("export {}", out.display()));
    let exported = std::fs::read_to_string(&out).unwrap();
    assert!(
        exported.contains("source-entries:"),
        "the export records its source entries"
    );
    let (first, _, _) = docs::read(&src).unwrap();
    let (second, losses, _) = docs::read(&out).unwrap();
    assert_eq!(shape(&second), shape(&first));
    assert!(
        losses.is_empty(),
        "nothing more lost on the way back: {losses:?}"
    );
    assert_eq!(first.iter().filter(|b| b.style == "heading").count(), 2);
    assert_eq!(first.iter().filter(|b| b.style == "math").count(), 1);
    assert_eq!(
        first
            .iter()
            .flat_map(|b| b.cites.clone())
            .collect::<Vec<_>>(),
        vec!["buran1988".to_string()]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn word_round_trip_keeps_structure_citations_and_mathematics() {
    // The same through Word: Markdown to .docx by Pandoc, imported, exported
    // to .docx by the memex, imported again.
    let dir = temp("docx");
    let md = dir.join("essay.md");
    std::fs::write(&md, ESSAY).unwrap();
    let docx = dir.join("essay.docx");
    let ok = std::process::Command::new("pandoc")
        .args(["-f", "markdown", "-t", "docx", "-o"])
        .arg(&docx)
        .arg(&md)
        .status()
        .unwrap();
    assert!(ok.success());
    let mut app = open(&dir);
    command(&mut app, &format!("import document {}", docx.display()));
    let losses_msg = app.message.clone();
    let out = dir.join("out.docx");
    command(&mut app, &format!("export {}", out.display()));
    let (first, _, _) = docs::read(&docx).unwrap();
    let (second, _, _) = docs::read(&out).unwrap();
    assert_eq!(
        shape(&second),
        shape(&first),
        "the Word round trip changed the outline"
    );
    // Against the Markdown original: the same headings, items and citations,
    // and the mathematics as LaTeX.
    let (orig, _, _) = docs::read(&md).unwrap();
    let styles = |b: &[Block]| {
        b.iter()
            .map(|x| (x.style.clone(), x.depth))
            .collect::<Vec<_>>()
    };
    assert_eq!(styles(&first), styles(&orig), "Word lost structure");
    let headings = |b: &[Block]| {
        b.iter()
            .filter(|x| x.style == "heading")
            .map(|x| x.text.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(headings(&first), headings(&orig));
    assert_eq!(
        first
            .iter()
            .flat_map(|b| b.cites.clone())
            .collect::<Vec<_>>(),
        vec!["buran1988".to_string()],
        "citations through Word"
    );
    let math = first.iter().find(|b| b.style == "math").unwrap();
    assert!(
        math.text.contains("\\int") && math.text.contains("\\frac"),
        "{}",
        math.text
    );
    eprintln!("Word import: {losses_msg}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_revised_document_opens_a_world_of_changes() {
    // Plan: "Re-importing a revised document opens a world, and a table
    // lists each change to adopt or retract."
    let dir = temp("revise");
    let src = dir.join("essay.md");
    std::fs::write(
        &src,
        "# Essay\n\nBuran flew in 1988.\n\nIt was unmanned.\n\nThe end.\n",
    )
    .unwrap();
    let mut app = open(&dir);
    command(&mut app, &format!("import document {}", src.display()));
    let actual = app.outline.clone();
    std::fs::write(
        &src,
        "# Essay\n\nBuran flew in 1989.\n\nThe end.\n\nA new closing line.\n",
    )
    .unwrap();
    command(&mut app, &format!("import document {}", src.display()));
    assert!(
        app.supposition
            .as_ref()
            .is_some_and(|s| s.name == "revision of essay.md")
    );
    let changes = app.revision_changes();
    let whats: Vec<&str> = changes.iter().map(|c| c.1.as_str()).collect();
    assert!(
        whats
            .iter()
            .any(|w| w.starts_with("changed 1.1: “Buran flew in 1988.” → “Buran flew in 1989.”")),
        "{whats:?}"
    );
    assert!(
        whats
            .iter()
            .any(|w| w.starts_with("deleted 1.2: “It was unmanned.”")),
        "{whats:?}"
    );
    assert!(
        whats
            .iter()
            .any(|w| w.starts_with("added: “A new closing line.”")),
        "{whats:?}"
    );
    // Keep all but the deletion: retract it, then commit.
    let deletion = changes
        .iter()
        .find(|c| c.1.starts_with("deleted"))
        .unwrap()
        .0;
    command(&mut app, &format!("retract {deletion}"));
    tap(&mut app, Key::Enter);
    command(&mut app, "commit");
    assert!(app.supposition.is_none());
    let texts: Vec<String> = app
        .outline
        .in_order()
        .iter()
        .map(|i| app.outline.text(*i))
        .collect();
    assert_eq!(
        texts,
        vec![
            "Essay",
            "Buran flew in 1989.",
            "It was unmanned.",
            "The end.",
            "A new closing line."
        ]
    );
    assert_ne!(app.outline, actual);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_changed_input_marks_its_results() {
    // Plan: "A changed input file marks the results that depended on it."
    // The results file is written by the Python helper an analysis calls.
    let dir = temp("results");
    std::fs::write(dir.join("data.csv"), "t,delay\n1,1.5\n2,1.6\n").unwrap();
    let helper = concat!(env!("CARGO_MANIFEST_DIR"), "/../../helpers/python");
    let script = format!(
        "import sys; sys.path.insert(0, {helper:?}); from memex_results import write_results; \
         write_results('results.json', [('mean_delay', 1.55, 0.05, 's')], ['data.csv'], script='analysis.py')"
    );
    let ok = std::process::Command::new("python3")
        .args(["-c", &script])
        .current_dir(&dir)
        .status()
        .unwrap();
    assert!(ok.success());
    let mut app = open(&dir);
    command(
        &mut app,
        &format!("import results {}", dir.join("results.json").display()),
    );
    app.evaluate().unwrap();
    assert!(app.stale.is_empty(), "nothing has changed yet");
    let result = app
        .outline
        .in_order()
        .into_iter()
        .find(|i| app.outline.text(*i).starts_with("mean_delay"))
        .unwrap();
    assert_eq!(app.outline.text(result), "mean_delay = 1.55 ± 0.05 s");
    let facts = memex::store::facts(app.journal().entries(), app.tip).unwrap();
    assert!(
        facts
            .iter()
            .any(|(_, f)| memex::reason::show(f).contains("measured(1.55, 0.05, \"s\")")),
        "{facts:?}"
    );
    std::fs::write(dir.join("data.csv"), "t,delay\n1,1.5\n2,1.9\n").unwrap();
    app.evaluate().unwrap();
    assert_eq!(app.stale.get(&result), Some(&vec!["data.csv".to_string()]));
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
    let item = app
        .outline
        .in_order()
        .into_iter()
        .find(|i| app.outline.text(*i).starts_with("The 1989"))
        .unwrap();
    app.view.panes[0].cursor = item;
    command(&mut app, "check quote");
    assert_eq!(app.message, "quote check: quoted");
    app.view.pos = 4;
    for c in "first ".chars() {
        tap(&mut app, Key::Char(c));
    }
    app.flush();
    command(&mut app, "check quote");
    assert_eq!(app.message, "quote check: misquoted");
    let e = app.journal().entries().last().unwrap();
    assert!(
        memex::reason::show(&format::terms::decode(&e.body.as_ref().unwrap()["facts"][0]).unwrap())
            .starts_with(&format!("ruling({item}, quote, misquoted, "))
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_source_check() {
    // § Testing claims, the source: fetched by DOI, never trusting a copy
    // that came with the claim. A local server stands in for Crossref.
    let dir = temp("source");
    let www = dir.join("www").join("10.5555");
    std::fs::create_dir_all(&www).unwrap();
    std::fs::write(
        www.join("good"),
        r#"{"status":"ok","message":{"title":["Buran's flight software"]}}"#,
    )
    .unwrap();
    std::fs::write(
        www.join("retracted"),
        r#"{"status":"ok","message":{"title":["Bad"],"updated-by":[{"type":"retraction"}]}}"#,
    )
    .unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut server = std::process::Command::new("python3")
        .args([
            "-m",
            "http.server",
            &port.to_string(),
            "--bind",
            "127.0.0.1",
        ])
        .current_dir(dir.join("www"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(700));
    let mut app = open(&dir);
    app.set_crossref(&format!("http://127.0.0.1:{port}/"));
    let rule = |app: &mut App, doi: &str, title: &str| -> String {
        tap(app, Key::Enter);
        for c in "a claim".chars() {
            tap(app, Key::Char(c));
        }
        app.flush();
        command(app, &format!("fact doi(paper, \"{doi}\")"));
        command(app, &format!("fact title(paper, \"{title}\")"));
        command(app, "check source");
        app.message.trim_start_matches("source check: ").to_string()
    };
    assert_eq!(
        rule(&mut app, "10.5555/good", "buran's flight-software"),
        "found",
        "title alike, spaces, hyphens and case aside"
    );
    assert_eq!(
        rule(&mut app, "10.5555/good", "Something else"),
        "misattributed"
    );
    assert_eq!(rule(&mut app, "10.5555/retracted", "Bad"), "retracted");
    assert_eq!(rule(&mut app, "10.5555/missing", "x"), "not_found");
    server.kill().unwrap();
    server.wait().unwrap();
    assert_eq!(rule(&mut app, "10.5555/good", "x"), "unavailable");
    std::fs::remove_dir_all(&dir).unwrap();
}
