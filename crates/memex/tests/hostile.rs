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

/// One item holding `text`, exported to `file`; the file's bytes.
fn export_one(name: &str, text: &str, file: &str) -> Vec<u8> {
    let dir = temp(name);
    let mut app = open(&dir);
    typ(&mut app, text);
    app.flush();
    let out = dir.join(file);
    let m = command(&mut app, &format!("export {}", out.display()));
    let bytes = std::fs::read(&out).unwrap_or_else(|e| panic!("no export ({m}): {e}"));
    std::fs::remove_dir_all(&dir).unwrap();
    bytes
}

#[test]
fn exported_html_runs_no_script_from_text() {
    // Text that came in as words (a Word paragraph reading "<script>…") must
    // leave as words. Catches item text passed to Pandoc as raw HTML.
    let html = String::from_utf8(export_one(
        "script",
        "Note <script>alert(1)</script> done.",
        "out.html",
    ))
    .unwrap();
    assert!(!html.contains("<script>alert"), "{html}");
    assert!(html.contains("done."), "{html}");
}

#[test]
fn exported_html_has_no_javascript_links() {
    // Catches a link whose target is a script, kept from imported text.
    let html = String::from_utf8(export_one(
        "jslink",
        "See [this](javascript:alert(2)) and [that](https://example.org/).",
        "out.html",
    ))
    .unwrap();
    assert!(!html.contains("javascript:"), "{html}");
    assert!(
        html.contains("https://example.org/"),
        "ordinary links are kept: {html}"
    );
}

#[test]
fn exported_latex_holds_no_raw_tex_from_text() {
    // Catches text exported as LaTeX commands, which read files when the
    // .tex is compiled.
    let tex = String::from_utf8(export_one(
        "rawtex",
        "Here \\input{/etc/hostname} ends.",
        "out.tex",
    ))
    .unwrap();
    assert!(!tex.contains("\\input{/etc/hostname}"), "{tex}");
}

#[test]
fn an_exported_word_file_embeds_no_local_file() {
    // Markdown image syntax in text would make Pandoc embed that file in the
    // .docx. Catches an export that carries a local file out.
    let dir = temp("img-secret");
    let secret = dir.join("secret.txt");
    std::fs::write(&secret, SECRET).unwrap();
    let docx = export_one(
        "img",
        &format!("Look ![x]({}) here.", secret.display()),
        "out.docx",
    );
    let has = |needle: &[u8]| docx.windows(needle.len()).any(|w| w == needle);
    assert!(!has(b"word/media/"), "the .docx carries a media file");
    assert!(has(b"word/document.xml"), "it is a Word file");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A fake Crossref: answers 404 to every request; returns the paths asked.
fn fake_crossref() -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use std::io::{BufRead, BufReader, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/works/", l.local_addr().unwrap());
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || {
        for c in l.incoming().flatten() {
            let mut r = BufReader::new(c.try_clone().unwrap());
            let mut line = String::new();
            let _ = r.read_line(&mut line);
            s2.lock()
                .unwrap()
                .push(line.split(' ').nth(1).unwrap_or("").to_string());
            let mut c = c;
            let _ = c.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });
    (base, seen)
}

#[test]
fn a_doi_is_one_request_for_that_doi() {
    // curl reads [1-5] and {a,b} in a URL as patterns and makes one request
    // per expansion; [1-1000000] would be a million requests to Crossref.
    // Real DOIs contain brackets too. Catches a DOI passed to curl unescaped.
    let dir = temp("doi");
    let (base, seen) = fake_crossref();
    let mut app = open(&dir);
    app.set_crossref(&base);
    typ(&mut app, "A paper.");
    app.flush();
    command(&mut app, "fact doi(p, \"10.1002/x[1-5]{a,b} y\")");
    let m = command(&mut app, "check source");
    let seen = seen.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec!["/works/10.1002/x%5B1-5%5D%7Ba%2Cb%7D%20y".to_string()],
        "{seen:?}"
    );
    assert!(m.contains("not_found"), "{m}");
    std::fs::remove_dir_all(&dir).unwrap();
}
