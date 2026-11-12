//! The reference checker of a journal directory (docs/design.md § The
//! journal format, "Checking"): both chains, every signature against
//! `keys.jsonl`, and every blob's name. It needs nothing from the memex.
//!
//! It checks the files in this order, each line before the next, so the
//! first error names the first line that is wrong: `keys.jsonl`,
//! `journal.jsonl`, `seals.jsonl`, then `blobs/`.

use ed25519_dalek::{Signature, VerifyingKey};
use format::chain::{self, LineError, line_error};
use format::record::{
    self, BLOBS, Entry, JOURNAL, KEYS, KEYS_FORMAT, Key, SEALS, SEALS_FORMAT, Seal,
};
use std::fs;
use std::io;
use std::path::Path;

/// What a journal directory holds, once checked.
#[derive(Debug)]
pub struct Report {
    pub keys: Vec<Key>,
    pub entries: Vec<Entry>,
    pub seals: Vec<Seal>,
    pub blobs: usize,
}

fn read(dir: &Path, file: &str) -> Result<String, LineError> {
    fs::read(dir.join(file))
        .map_err(|e| line_error(file, 0, format!("cannot read: {e}")))
        .and_then(|b| {
            String::from_utf8(b).map_err(|e| {
                let line = 1 + e.as_bytes()[..e.utf8_error().valid_up_to()]
                    .iter()
                    .filter(|c| **c == b'\n')
                    .count();
                line_error(file, line, "not UTF-8")
            })
        })
}

/// Check `keys.jsonl`: a header, then distinct Ed25519 public keys.
pub fn check_keys(text: &str) -> Result<Vec<Key>, LineError> {
    let lines = chain::lines(KEYS, text)?;
    record::check_header(lines[0], KEYS_FORMAT).map_err(|m| line_error(KEYS, 1, m))?;
    let mut keys: Vec<Key> = Vec::new();
    for (i, line) in lines.iter().enumerate().skip(1) {
        let k = Key::parse(line).map_err(|m| line_error(KEYS, i + 1, m))?;
        if keys.iter().any(|o| o.fingerprint == k.fingerprint) {
            return Err(line_error(KEYS, i + 1, "this key is listed twice"));
        }
        VerifyingKey::from_bytes(&k.bytes())
            .map_err(|_| line_error(KEYS, i + 1, "not a valid Ed25519 public key"))?;
        keys.push(k);
    }
    Ok(keys)
}

/// Check `seals.jsonl` against the journal's entries and the keys. Seal n
/// seals entry n. With `all`, every entry must be sealed.
pub fn check_seals(
    text: &str,
    entries: &[Entry],
    keys: &[Key],
    all: bool,
) -> Result<Vec<Seal>, LineError> {
    let lines = chain::lines(SEALS, text)?;
    record::check_header(lines[0], SEALS_FORMAT).map_err(|m| line_error(SEALS, 1, m))?;
    let mut seals: Vec<Seal> = Vec::new();
    for (i, line) in lines.iter().enumerate().skip(1) {
        let err = |m: String| line_error(SEALS, i + 1, m);
        let s = Seal::parse(line).map_err(err)?;
        if s.seq != i as u64 {
            return Err(err(format!("seq is {}, expected {i}", s.seq)));
        }
        let Some(e) = entries.get(i - 1) else {
            return Err(err(format!(
                "seals entry {i}, which the journal does not hold"
            )));
        };
        if s.entry_hash != e.hash {
            return Err(err(format!("entry_hash is not the hash of entry {i}")));
        }
        if s.prev.as_ref() != seals.last().map(|p| &p.hash) {
            return Err(err("prev is not the hash of the seal before".into()));
        }
        let Some(k) = keys.iter().find(|k| k.fingerprint == s.author) else {
            return Err(err("author is not a key in keys.jsonl".into()));
        };
        let vk = VerifyingKey::from_bytes(&k.bytes()).expect("checked in check_keys");
        let sig = Signature::from_bytes(&record::from_hex(&s.sig, 64).unwrap().try_into().unwrap());
        let msg = record::from_hex(&s.entry_hash, 32).unwrap();
        if vk.verify_strict(&msg, &sig).is_err() {
            return Err(err("the signature does not verify".into()));
        }
        seals.push(s);
    }
    if all && seals.len() < entries.len() {
        let n = seals.len() + 1;
        return Err(line_error(
            JOURNAL,
            n + 1,
            format!("entry {n} is not sealed"),
        ));
    }
    Ok(seals)
}

/// Check that each file in `blobs/` is named by the SHA-256 of its content.
pub fn check_blobs(dir: &Path) -> Result<usize, LineError> {
    let blobs = dir.join(BLOBS);
    let list = match fs::read_dir(&blobs) {
        Ok(l) => l,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(line_error(BLOBS, 0, format!("cannot read: {e}"))),
    };
    let mut names: Vec<String> = Vec::new();
    for item in list {
        let item = item.map_err(|e| line_error(BLOBS, 0, e.to_string()))?;
        names.push(item.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    for name in &names {
        let file = format!("{BLOBS}/{name}");
        let bytes = fs::read(blobs.join(name))
            .map_err(|e| line_error(&file, 0, format!("cannot read: {e}")))?;
        if record::sha256_hex(&bytes) != *name {
            return Err(line_error(
                &file,
                0,
                "the name is not the SHA-256 of the content",
            ));
        }
    }
    Ok(names.len())
}

/// Check a whole journal directory. Every entry must be sealed.
pub fn check_dir(dir: &Path) -> Result<Report, LineError> {
    let keys = check_keys(&read(dir, KEYS)?)?;
    let entries = chain::check_journal(&read(dir, JOURNAL)?)?;
    let seals = check_seals(&read(dir, SEALS)?, &entries, &keys, true)?;
    let blobs = check_blobs(dir)?;
    Ok(Report {
        keys,
        entries,
        seals,
        blobs,
    })
}
