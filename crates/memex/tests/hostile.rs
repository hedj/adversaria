//! Hostile input: what an imported file, a fetched record or a fact could
//! do beyond being data. Each test plants something that would act if the
//! memex let it (read a local file, run a script, make many requests, crash
//! the memex) and checks that it does not.

use memex::app::{App, Event, Key, Quasi};
use std::path::{Path, PathBuf};

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("adversaria-hostile-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn tap(app: &mut App, k: Key) {
    app.handle(Event::down(k));
    app.handle(Event::up(k));
}

fn typ(app: &mut App, s: &str) {
    for c in s.chars() {
        tap(app, Key::Char(c));
    }
}

/// Run a command; return the message it left.
fn command(app: &mut App, line: &str) -> String {
    app.handle(Event::down(Key::Quasi(Quasi::Command)));
    typ(app, line);
    app.handle(Event::up(Key::Quasi(Quasi::Command)));
    let _ = app.evaluate();
    app.message.clone()
}

fn open(dir: &Path) -> App {
    let mut app = App::open(&dir.join("journal")).unwrap();
    app.set_durable(false);
    app
}

/// Every item's text, joined.
fn all_text(app: &App) -> String {
    app.outline
        .in_order()
        .into_iter()
        .map(|id| app.outline.text(id))
        .collect::<Vec<_>>()
        .join("\n")
}

const SECRET: &str = "SECRET-KEY-MATERIAL-7f3a";

#[test]
fn an_imported_document_cannot_include_local_files() {
    // A LaTeX file can \input or \include any file the user can read, such as
    // the sealer's key; Pandoc follows them unless sandboxed. Catches an
    // import that copies such a file into the permanent journal.
    let dir = temp("include");
    let secret = dir.join("secret.tex");
    std::fs::write(&secret, format!("{SECRET}\n")).unwrap();
    let doc = dir.join("paper.tex");
    let s = secret.display().to_string();
    std::fs::write(
        &doc,
        format!(
            "Before.\n\n\\input{{{s}}}\n\n\\include{{{}}}\n\nAfter.\n",
            s.trim_end_matches(".tex")
        ),
    )
    .unwrap();
    let mut app = open(&dir);
    command(&mut app, &format!("import document {}", doc.display()));
    let text = all_text(&app);
    assert!(
        text.contains("Before."),
        "the document itself was imported: {text:?}\n{}",
        app.message
    );
    assert!(
        !text.contains(SECRET),
        "the included file was copied in: {text:?}"
    );
    let journal = std::fs::read_to_string(dir.join("journal/journal.jsonl")).unwrap();
    assert!(
        !journal.contains(SECRET),
        "the journal holds the included file"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
