//! The check program: exit codes and the message that names the line.

use ed25519_dalek::{Signer, SigningKey};
use format::record::*;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// An empty directory of this test's own.
fn empty_dir(name: &str) -> PathBuf {
    let id = std::process::id();
    let dir = std::env::temp_dir().join(format!("adversaria-check-cli-{name}-{id}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The three files of a journal directory, each by its name and format.
const FILES: [(&str, &str); 3] = [
    (KEYS, KEYS_FORMAT),
    (JOURNAL, JOURNAL_FORMAT),
    (SEALS, SEALS_FORMAT),
];

/// A file of a journal directory, by its name and format: a header, then
/// these records, a line each.
fn write(dir: &Path, (file, format): (&str, &str), records: &[String]) {
    let lines: String = records.iter().map(|r| format!("{r}\n")).collect();
    fs::write(dir.join(file), format!("{}\n{lines}", header(format))).unwrap();
}

/// The body of an entry 1 of this text.
fn init(text: &str) -> Value {
    json!({ "op": "init", "text": text, "version": JOURNAL_VERSION })
}

/// Edits of these bodies, each after the one before.
fn chained(bodies: &[Value]) -> Vec<Entry> {
    let mut entries = Vec::<Entry>::new();
    for (seq, body) in (1..).zip(bodies) {
        let parent = entries.last().map(|p| (p.seq, p.hash.as_str()));
        let e = Entry::new(seq, parent, EntryKind::Edit, body.clone()).unwrap();
        entries.push(e);
    }
    entries
}

/// An entry's seal by a key, after the seal of this hash, on the date the
/// journals here are sealed.
fn seal_of(sk: &SigningKey, key: &Key, e: &Entry, prev: Option<&str>) -> Seal {
    let sig = to_hex(&sk.sign(&from_hex(&e.hash, 32).unwrap()).to_bytes());
    let stamp = Stamp::new("2026-11-12T09:30:00Z", &key.fingerprint, &sig);
    Seal::new(e.id(), stamp, prev).unwrap()
}

/// The records of each file of a journal of edits of these bodies, each
/// sealed by one key: the key, the entries, and their seals.
fn sealed_lines(bodies: &[Value]) -> [Vec<String>; 3] {
    let sk = SigningKey::from_bytes(&[9u8; 32]);
    let key = Key::new(&sk.verifying_key().to_bytes()).unwrap();
    let entries = chained(bodies);
    let mut seals = Vec::<Seal>::new();
    for e in &entries {
        let prev = seals.last().map(|s| s.hash.as_str());
        seals.push(seal_of(&sk, &key, e, prev));
    }
    let entries = entries.iter().map(Entry::to_line);
    let seals = seals.iter().map(Seal::to_line);
    let lines = |l: Vec<_>| l.into_iter().map(Result::unwrap).collect();
    [vec![key.to_line()], entries.collect(), seals.collect()].map(lines)
}

/// A journal directory of edits of these bodies, sealed.
fn sealed_dir(name: &str, bodies: &[Value]) -> PathBuf {
    let dir = empty_dir(name);
    for (file, lines) in FILES.iter().zip(sealed_lines(bodies)) {
        write(&dir, *file, &lines);
    }
    dir
}

/// The check program's exit code and what it prints, on these arguments.
fn check(args: &[&Path]) -> (Option<i32>, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_check"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into(),
    )
}

// Each of these catches a wrong exit code, or an error printed without its
// file and line.

#[test]
fn a_sealed_journal_is_ok() {
    let dir = sealed_dir("ok", &[init("hello")]);
    let ok = "ok: 1 entries, 1 seals, 1 keys, 0 blobs\n";
    assert_eq!(check(&[&dir]), (Some(0), ok.into()));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_entry_that_does_not_read_is_listed_and_the_journal_still_checks() {
    // Catches an entry whose body does not read passed over unsaid, or
    // taken for a fault of the record, which is whole.
    let text = json!({ "op": "text", "item": 1, "add": "B" });
    let dir = sealed_dir("unread", &[init("hello"), text]);
    let said = "ok: 2 entries, 2 seals, 1 keys, 0 blobs\n\
                entry 2 (edit) does not read, so the memex leaves it out: missing field `spans`\n";
    assert_eq!(check(&[&dir]), (Some(0), said.into()));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_changed_byte_is_named_by_its_file_and_line() {
    let dir = sealed_dir("changed", &[init("hello")]);
    let [_, entries, _] = sealed_lines(&[init("hello")]);
    let changed: Vec<String> = entries
        .iter()
        .map(|l| l.replace("hello", "hullo"))
        .collect();
    write(&dir, FILES[1], &changed);
    let fault = "journal.jsonl:2: body_hash does not match the body\n";
    assert_eq!(check(&[&dir]), (Some(1), fault.into()));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_file_that_cannot_be_read_is_named() {
    let dir = empty_dir("unreadable");
    let (code, printed) = check(&[&dir]);
    assert_eq!(code, Some(1));
    assert!(
        printed.starts_with("keys.jsonl: cannot read: "),
        "{printed}"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn no_directory_is_a_usage_error() {
    assert_eq!(check(&[]).0, Some(2));
}
