//! The sealer program, run as a separate process beside the journal writer.

use memex::journal::Journal;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// A journal directory and a key file of this test's own, neither there yet.
fn fresh(name: &str) -> (PathBuf, PathBuf) {
    let id = std::process::id();
    let dir = std::env::temp_dir().join(format!("adversaria-cli-{name}-{id}"));
    let key = dir.with_extension("key");
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_file(&key);
    (dir, key)
}

fn remove(dir: &Path, key: &Path) {
    fs::remove_dir_all(dir).unwrap();
    fs::remove_file(key).unwrap();
}

/// The sealer program, to run a command on a directory with a key.
fn sealer(command: &str, dir: &Path, key: &Path, rest: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sealer"));
    c.arg(command).arg(dir).arg(key).args(rest);
    c
}

/// A directory and key that `sealer init` has made.
fn initialised(name: &str) -> (PathBuf, PathBuf) {
    let (dir, key) = fresh(name);
    let out = sealer("init", &dir, &key, &[]).output().unwrap();
    let why = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{why}");
    (dir, key)
}

/// The seals written so far: the lines of seals.jsonl after its header.
fn seals_written(dir: &Path) -> usize {
    let seals = fs::read_to_string(dir.join("seals.jsonl"));
    seals.map_or(0, |s| s.lines().count().saturating_sub(1))
}

/// The seals written once there are `n`, or once ten seconds have passed.
fn seals_once_there_are(n: usize, dir: &Path) -> usize {
    let t = Instant::now();
    loop {
        let sealed = seals_written(dir);
        if sealed == n || t.elapsed() > Duration::from_secs(10) {
            return sealed;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Write 200 entries, pausing after every 20 so the sealer runs between.
fn write_entries(j: &mut Journal) {
    for i in 0..200 {
        j.append("edit", json!({"n": i})).unwrap();
        if i % 20 == 0 {
            std::thread::sleep(Duration::from_millis(7));
        }
    }
}

#[test]
fn seals_entries_as_they_are_written() {
    // Catches a sealer loop that misses entries written while it runs, or
    // signs a torn line, as a separate process and not only as a library.
    let (dir, key) = initialised("run");
    let mut j = Journal::open(&dir).unwrap();
    let mut child = sealer("run", &dir, &key, &["--interval-ms", "5"])
        .spawn()
        .unwrap();
    write_entries(&mut j);
    let sealed = seals_once_there_are(200, &dir);
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(sealed, 200);
    assert_eq!(check::check_dir(&dir).unwrap().seals.len(), 200);
    remove(&dir, &key);
}

// The next three catch the program exiting 0 on bad arguments or an exposed
// key.

#[test]
fn no_arguments_is_a_usage_error() {
    let code = Command::new(env!("CARGO_BIN_EXE_sealer"))
        .output()
        .unwrap()
        .status
        .code();
    assert_eq!(code, Some(2));
}

#[test]
fn a_second_init_is_refused() {
    let (dir, key) = initialised("twice");
    let again = sealer("init", &dir, &key, &[]).status().unwrap();
    assert_eq!(again.code(), Some(1));
    remove(&dir, &key);
}

#[test]
fn a_key_that_others_can_read_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, key) = initialised("exposed");
    fs::set_permissions(&key, fs::Permissions::from_mode(0o640)).unwrap();
    Journal::open(&dir)
        .unwrap()
        .append("edit", json!({}))
        .unwrap();
    let out = sealer("run", &dir, &key, &["--once"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("readable by others"));
    remove(&dir, &key);
}
