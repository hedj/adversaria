//! The sealer program, run as a separate process beside the journal writer.

use memex::journal::Journal;
use serde_json::json;
use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn seals_entries_as_they_are_written() {
    // Catches a sealer loop that misses entries written while it runs, or
    // signs a torn line, as a separate process and not only as a library.
    let bin = env!("CARGO_BIN_EXE_sealer");
    let dir = std::env::temp_dir().join(format!("adversaria-cli-{}", std::process::id()));
    let key = dir.with_extension("key");
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_file(&key);
    let out = Command::new(bin)
        .args(["init", dir.to_str().unwrap(), key.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut j = Journal::open(&dir).unwrap();
    let mut child = Command::new(bin)
        .args([
            "run",
            dir.to_str().unwrap(),
            key.to_str().unwrap(),
            "--interval-ms",
            "5",
        ])
        .spawn()
        .unwrap();
    for i in 0..200 {
        j.append("edit", json!({"n": i})).unwrap();
        if i % 20 == 0 {
            std::thread::sleep(Duration::from_millis(7));
        }
    }
    let t = Instant::now();
    let sealed = loop {
        let n = fs::read_to_string(dir.join("seals.jsonl"))
            .map_or(0, |s| s.lines().count().saturating_sub(1));
        if n == 200 || t.elapsed() > Duration::from_secs(10) {
            break n;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(sealed, 200);
    let r = check::check_dir(&dir).unwrap();
    assert_eq!(r.seals.len(), 200);
    fs::remove_dir_all(&dir).unwrap();
    fs::remove_file(&key).unwrap();
}

#[test]
fn usage_and_bad_key_exit_nonzero() {
    // Catches the program exiting 0 on bad arguments or an exposed key.
    use std::os::unix::fs::PermissionsExt;
    let bin = env!("CARGO_BIN_EXE_sealer");
    assert_eq!(Command::new(bin).output().unwrap().status.code(), Some(2));
    let dir = std::env::temp_dir().join(format!("adversaria-cli-bad-{}", std::process::id()));
    let key = dir.with_extension("key");
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_file(&key);
    assert!(
        Command::new(bin)
            .args(["init", dir.to_str().unwrap(), key.to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(
        Command::new(bin)
            .args(["init", dir.to_str().unwrap(), key.to_str().unwrap()])
            .status()
            .unwrap()
            .code(),
        Some(1)
    );
    fs::set_permissions(&key, fs::Permissions::from_mode(0o640)).unwrap();
    Journal::open(&dir)
        .unwrap()
        .append("edit", json!({}))
        .unwrap();
    let out = Command::new(bin)
        .args([
            "run",
            dir.to_str().unwrap(),
            key.to_str().unwrap(),
            "--once",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("readable by others"));
    fs::remove_dir_all(&dir).unwrap();
    fs::remove_file(&key).unwrap();
}
