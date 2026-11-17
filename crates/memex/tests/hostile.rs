//! Hostile input: what an imported file, a fetched record or a fact could
//! do beyond being data. Each test plants something that would act if the
//! memex let it (read a local file, run a script, make many requests, crash
//! the memex) and checks that it does not.

use format::terms::Term;
use memex::app::{App, Event, Key, Quasi};
use std::collections::BTreeMap;
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
    app.session.message.clone()
}

fn open(dir: &Path) -> App {
    let mut app = App::open(&dir.join("journal")).unwrap();
    app.set_durable(false);
    app
}

/// Every item's text, joined.
fn all_text(app: &App) -> String {
    app.notebook
        .outline
        .in_order()
        .into_iter()
        .map(|id| app.notebook.outline.text(id))
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
        app.session.message
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
    app.session.set_crossref(&base);
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

#[test]
fn a_wikidata_file_holds_only_ground_facts_of_its_three_shapes() {
    // The loader reads with read_term and keeps wd/3, wd_label/2 and
    // wd_constraint/2. Catches a file accepted with a fact that is not data
    // the tool writes: a variable (wd_label(X, "…") names every entity), a
    // directive, a clause, or a value nested deep enough to crash a reader.
    let dir = temp("wd");
    let deep = format!(
        "wd('Q1', 'P1', {}x{}).\n",
        "f(".repeat(5000),
        ")".repeat(5000)
    );
    let cases = [
        ("variable", "wd_label(X, \"anything\").\n".to_string()),
        ("directive", ":- initialization(halt).\n".to_string()),
        ("clause", "wd('Q1', 'P1', x) :- true.\n".to_string()),
        ("compound value", "wd('Q1', 'P1', f(x)).\n".to_string()),
        (
            "label not a string",
            "wd_label('Q1', 'Buran').\n".to_string(),
        ),
        ("deep", deep),
    ];
    let mut app = open(&dir);
    typ(&mut app, "Buran.");
    app.flush();
    command(&mut app, "fact date(buran_flight, 1988)");
    for (what, text) in cases {
        let f = dir.join(format!("{}.pl", what.replace(' ', "_")));
        std::fs::write(&f, format!("wd('Q2', 'P31', 'Q5').\n{text}")).unwrap();
        let before = app.journal().entries().len();
        let m = command(&mut app, &format!("import wikidata {}", f.display()));
        assert!(m.contains("not a Wikidata subset"), "{what}: {m}");
        assert!(
            !app.journal().entries()[before..]
                .iter()
                .any(|e| e.kind() == "import"),
            "{what}: the import was written"
        );
    }
    // A good file still imports, and the world still evaluates.
    let good = dir.join("good.pl");
    std::fs::write(&good, "wd('Q2', 'P31', 'Q5').\nwd('Q2', 'P619', date(1988, 11, 15)).\nwd('Q2', 'P1', 3).\nwd('Q2', 'P2', 2.5).\nwd('Q2', 'P3', \"text\").\nwd_label('Q2', \"Buran\").\nwd_constraint('P619', single_value).\n").unwrap();
    let m = command(&mut app, &format!("import wikidata {}", good.display()));
    assert!(m.starts_with("imported wikidata"), "{m}");
    app.evaluate().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A file a planted goal would create; the tests check it never appears.
fn marker(dir: &Path) -> (PathBuf, String) {
    let m = dir.join("RAN");
    let s = m.display().to_string();
    (m, s)
}

#[test]
fn a_definition_cannot_run_a_side_effect() {
    // Constraint bodies and derived goals call built-ins; shell/1 is one.
    // Catches a definition, typed or arriving as a believed fact, that runs
    // a command, writes a file or stops the reasoner.
    let dir = temp("defs");
    let (m, ms) = marker(&dir);
    let mut app = open(&dir);
    typ(&mut app, "Buran.");
    app.flush();
    command(&mut app, "fact date(buran_flight, 1988)");
    let cases = [
        format!("constraint c1: false :- date(E, Y), shell('touch {ms}')."),
        format!("constraint c2: false :- date(E, Y), \\+ shell('touch {ms}')."),
        format!("constraint c3: false :- date(E, Y), (Y > 0 ; shell('touch {ms}'))."),
        format!("constraint c4: false :- date(E, Y), findall(x, shell('touch {ms}'), _)."),
        "constraint c5: false :- date(E, Y), halt.".to_string(),
        "constraint c6: false :- date(E, Y), assertz(date(x, 1)).".to_string(),
        "constraint c7: false :- date(E, Y), G = halt, call(G).".to_string(),
        format!("derive x {{N}} where date(buran_flight, N), shell('touch {ms}')"),
        format!("fact node_def(99, query([], shell('touch {ms}')))"),
        format!("fact rule_def(r, p(1), shell('touch {ms}'))"),
        format!("claim node_def(98, query([], shell('touch {ms}'))) from a document"),
    ];
    for line in &cases {
        let msg = command(&mut app, line);
        assert!(
            msg.contains("not allowed") || msg.contains("reserved"),
            "{line}: {msg}"
        );
        assert!(!m.exists(), "{line} ran a command");
    }
    // What constraints are for still works: comparison, arithmetic,
    // negation and disjunction over facts.
    let ok = command(
        &mut app,
        "constraint late: false :- date(E, Y), \\+ (Y < 1900 ; Y > 2100), Z is Y mod 4, Z =:= 1.",
    );
    assert!(ok.contains("kept"), "{ok}");
    app.evaluate().unwrap();
    assert!(!m.exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A compound term of a name and its arguments.
fn compound(f: &str, a: Vec<Term>) -> Term {
    Term::Compound(f.into(), a)
}

fn atom(x: &str) -> Term {
    Term::Atom(x.into())
}

/// A goal that makes a file, which a side effect would leave behind.
fn touch(path: &str) -> Term {
    compound("shell", vec![atom(&format!("touch {path}"))])
}

/// A definition, node 9, whose goal makes a file: stored as a definition
/// arrives, as node_def(9, query([], Goal)).
fn touching_definition(path: &str) -> Term {
    let query = compound("query", vec![atom("[]"), touch(path)]);
    compound("node_def", vec![Term::Int(9), query])
}

/// The value of a node: node(Id, Value).
fn node(id: i64) -> Term {
    compound("node", vec![Term::Int(id), Term::Var(0)])
}

#[test]
fn a_definition_sent_straight_to_the_server_runs_no_command() {
    // Past the memex's own checks: facts sent straight to the server, as a
    // journal written by something else would be replayed. Catches a
    // reasoner that calls any built-in.
    let dir = temp("server-definition");
    let (m, ms) = marker(&dir);
    let mut p = memex::prolog::Prolog::start().unwrap();
    p.assert_fact(&touching_definition(&ms), 1).unwrap();
    let r = p.all(&node(9), 1);
    assert!(format!("{r:?}").contains("not_allowed"), "{r:?}");
    assert!(!m.exists(), "evaluating the definition ran the command");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Facts the server refuses to store: a clause, a module-qualified fact,
/// and facts of the server's own predicates.
fn reserved_facts(path: &str) -> Vec<Term> {
    let abc = vec![atom("a"), atom("b"), atom("c")];
    vec![
        compound(":-", vec![atom("p"), touch(path)]),
        compound(":", vec![atom("lists"), compound("append", abc)]),
        compound("fact_ref", vec![Term::Int(1), atom("x"), atom("y")]),
        compound("wd", vec![atom("Q1"), atom("P1"), atom("x")]),
        compound("query_engine", vec![atom("a"), atom("b")]),
    ]
}

/// Send each fact to the server, and see each refused as reserved.
fn all_refused(p: &mut memex::prolog::Prolog, facts: Vec<Term>) {
    for (i, f) in facts.into_iter().enumerate() {
        let r = p.assert_fact(&f, 10 + i as u64);
        assert!(format!("{r:?}").contains("reserved"), "{f:?}: {r:?}");
    }
}

#[test]
fn a_clause_or_the_servers_own_fact_sent_straight_is_refused() {
    // Past the memex's own checks, as above. Catches a server that stores
    // a clause, which would run when its head is called.
    let dir = temp("server-clauses");
    let (m, ms) = marker(&dir);
    let mut p = memex::prolog::Prolog::start().unwrap();
    all_refused(&mut p, reserved_facts(&ms));
    p.all(&atom("p"), 1).ok();
    assert!(!m.exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Import a results file into a new memex: what the memex says, and the
/// results it marks as resting on changed inputs.
fn import_and_mark(journal: &Path, results: &Path) -> (String, BTreeMap<u64, Vec<String>>) {
    let mut app = App::open(journal).unwrap();
    app.set_durable(false);
    let said = command(&mut app, &format!("import results {}", results.display()));
    (said, app.notebook.stale_results())
}

#[test]
fn a_results_file_cannot_make_the_memex_read_forever() {
    // The stale-result check reads each input the results file names, on
    // every evaluation. Catches an input such as a named pipe (blocks
    // forever) or /dev/zero (reads until memory runs out) being read.
    let dir = temp("results");
    let fifo = dir.join("pipe");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let results = dir.join("results.json");
    std::fs::write(
        &results,
        serde_json::json!({
            "format": "memex-results", "version": 1,
            "results": [{"name": "x", "value": 1.0, "uncertainty": 0.1, "unit": "s"}],
            "prov": {"activity": {"script": "a.py", "commit": "abc"},
                     "used": [{"path": fifo.display().to_string(), "sha256": "00"},
                              {"path": "/dev/zero", "sha256": "00"}]}
        })
        .to_string(),
    )
    .unwrap();
    let journal = dir.join("journal");
    let (tx, rx) = std::sync::mpsc::channel();
    let res = results.clone();
    std::thread::spawn(move || tx.send(import_and_mark(&journal, &res)));
    let (m, stale) = rx
        .recv_timeout(std::time::Duration::from_secs(20))
        .expect("the memex was still reading after 20 s");
    assert!(m.starts_with("imported the results"), "{m}");
    let marked: Vec<String> = stale.values().flatten().cloned().collect();
    assert!(marked.iter().any(|p| p.ends_with("pipe")), "{marked:?}");
    assert!(marked.iter().any(|p| p == "/dev/zero"), "{marked:?}");
}

/// Run a command in a memex of its own: the message it leaves, if it is
/// done within 20 s.
fn done_in_time(name: &str, line: String) -> String {
    let dir = temp(name);
    let task = move || command(&mut open(&dir), &line);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || tx.send(task()));
    let waited = rx.recv_timeout(std::time::Duration::from_secs(20));
    waited.expect("the memex was still reading after 20 s")
}

/// A named pipe at a path; nothing ever writes to it.
fn pipe(path: &Path) -> String {
    let made = std::process::Command::new("mkfifo").arg(path).status();
    assert!(made.unwrap().success());
    path.display().to_string()
}

#[test]
fn an_import_refuses_a_pipe_rather_than_wait_on_it() {
    // Catches an import that reads a named pipe, which blocks forever: a
    // results file, or one named as a document.
    let dir = temp("import-pipes");
    for (name, kind) in [("pipe.json", "results"), ("pipe.md", "document")] {
        let line = format!("import {kind} {}", pipe(&dir.join(name)));
        let said = done_in_time(&format!("import-pipe-{kind}"), line);
        assert!(said.ends_with("not a regular file"), "{kind}: {said}");
    }
}

#[test]
fn an_import_refuses_a_device_rather_than_read_it_without_end() {
    // Catches an import that reads /dev/zero until memory runs out.
    let said = done_in_time("import-zero", "import results /dev/zero".into());
    assert!(said.ends_with("not a regular file"), "{said}");
}
