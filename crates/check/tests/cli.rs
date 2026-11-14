//! The check program: exit codes and the message that names the line.

use ed25519_dalek::{Signer, SigningKey};
use format::record::*;
use serde_json::json;
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

/// A file of a journal directory: a header, then one record.
fn write(dir: &Path, file: &str, format: &str, record: &str) {
    fs::write(dir.join(file), format!("{}\n{record}\n", header(format))).unwrap();
}

/// The one entry of the journals here, and its seal by a key.
fn sealed_entry(text: &str) -> (Key, Entry, Seal) {
    let sk = SigningKey::from_bytes(&[9u8; 32]);
    let key = Key::new(&sk.verifying_key().to_bytes()).unwrap();
    let e = Entry::new(1, None, "edit", json!({ "text": text })).unwrap();
    let sig = to_hex(&sk.sign(&from_hex(&e.hash, 32).unwrap()).to_bytes());
    let stamp = Stamp::new("2026-11-12T09:30:00Z", &key.fingerprint, &sig);
    let seal = Seal::new(e.id(), stamp, None).unwrap();
    (key, e, seal)
}

/// A journal directory of one entry, "hello", sealed.
fn sealed_dir(name: &str) -> PathBuf {
    let dir = empty_dir(name);
    let (key, e, seal) = sealed_entry("hello");
    write(&dir, KEYS, KEYS_FORMAT, &key.to_line().unwrap());
    write(&dir, JOURNAL, JOURNAL_FORMAT, &e.to_line().unwrap());
    write(&dir, SEALS, SEALS_FORMAT, &seal.to_line().unwrap());
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
    let dir = sealed_dir("ok");
    let ok = "ok: 1 entries, 1 seals, 1 keys, 0 blobs\n";
    assert_eq!(check(&[&dir]), (Some(0), ok.into()));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_changed_byte_is_named_by_its_file_and_line() {
    let dir = sealed_dir("changed");
    let (_, e, _) = sealed_entry("hello");
    let changed = e.to_line().unwrap().replace("hello", "hullo");
    write(&dir, JOURNAL, JOURNAL_FORMAT, &changed);
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
