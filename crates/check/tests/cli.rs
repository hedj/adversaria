//! The check program: exit codes and the message that names the line.

use ed25519_dalek::{Signer, SigningKey};
use format::record::*;
use serde_json::json;
use std::fs;
use std::process::Command;

#[test]
fn exit_codes_and_named_line() {
    // Catches a wrong exit code, or an error printed without file and line.
    let dir = std::env::temp_dir().join(format!("adversaria-check-cli-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let sk = SigningKey::from_bytes(&[9u8; 32]);
    let key = Key::new(&sk.verifying_key().to_bytes()).unwrap();
    let e = Entry::new(1, None, "edit", json!({"text": "hello"})).unwrap();
    let sig = to_hex(&sk.sign(&from_hex(&e.hash, 32).unwrap()).to_bytes());
    let stamp = Stamp::new("2026-11-12T09:30:00Z", &key.fingerprint, &sig);
    let s = Seal::new(e.id(), stamp, None).unwrap();
    let file = |f: &str, fmt: &str, line: String| {
        fs::write(dir.join(f), format!("{}\n{line}\n", header(fmt))).unwrap()
    };
    file(KEYS, KEYS_FORMAT, key.to_line().unwrap());
    file(JOURNAL, JOURNAL_FORMAT, e.to_line().unwrap());
    file(SEALS, SEALS_FORMAT, s.to_line().unwrap());

    let bin = env!("CARGO_BIN_EXE_check");
    let out = Command::new(bin).arg(&dir).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "ok: 1 entries, 1 seals, 1 keys, 0 blobs\n"
    );

    file(
        JOURNAL,
        JOURNAL_FORMAT,
        e.to_line().unwrap().replace("hello", "hullo"),
    );
    let out = Command::new(bin).arg(&dir).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "journal.jsonl:2: body_hash does not match the body\n"
    );

    assert_eq!(Command::new(bin).output().unwrap().status.code(), Some(2));
    fs::remove_dir_all(&dir).unwrap();
}
