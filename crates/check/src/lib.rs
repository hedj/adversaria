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
use std::string::FromUtf8Error;

/// What a journal directory holds, once checked.
#[derive(Debug)]
pub struct Report {
    pub keys: Vec<Key>,
    pub entries: Vec<Entry>,
    pub seals: Vec<Seal>,
    pub blobs: usize,
}

/// A file of the directory as text.
fn read(dir: &Path, file: &str) -> Result<String, LineError> {
    let bytes = read_bytes(dir, file)?;
    String::from_utf8(bytes).map_err(|e| line_error(file, bad_line(&e), "not UTF-8"))
}

/// A file of the directory; if it cannot be read, an error naming it.
fn read_bytes(dir: &Path, file: &str) -> Result<Vec<u8>, LineError> {
    let cannot = |e: io::Error| line_error(file, 0, format!("cannot read: {e}"));
    fs::read(dir.join(file)).map_err(cannot)
}

/// The line on which a file stops being UTF-8.
fn bad_line(e: &FromUtf8Error) -> usize {
    let good = &e.as_bytes()[..e.utf8_error().valid_up_to()];
    1 + good.iter().filter(|c| **c == b'\n').count()
}

/// Check `keys.jsonl`: a header, then distinct Ed25519 public keys.
pub fn check_keys(text: &str) -> Result<Vec<Key>, LineError> {
    let mut keys: Vec<Key> = Vec::new();
    for line in chain::records(KEYS, KEYS_FORMAT, text)? {
        let key = line.read(|t| new_key(&keys, t))?;
        keys.push(key);
    }
    Ok(keys)
}

/// A line's key: a valid Ed25519 public key, not listed before.
fn new_key(keys: &[Key], line: &str) -> Result<Key, String> {
    let k = Key::parse(line)?;
    if keys.iter().any(|o| o.fingerprint == k.fingerprint) {
        return Err("this key is listed twice".into());
    }
    VerifyingKey::from_bytes(&k.bytes()).map_err(|_| "not a valid Ed25519 public key")?;
    Ok(k)
}

/// Check `seals.jsonl` against the journal's entries and the keys. Seal n
/// seals entry n.
pub fn check_seals(text: &str, entries: &[Entry], keys: &[Key]) -> Result<Vec<Seal>, LineError> {
    let mut seals = Seals::new(entries, keys);
    for line in chain::records(SEALS, SEALS_FORMAT, text)? {
        line.read(|t| seals.read(t))?;
    }
    Ok(seals.read)
}

/// Seals being checked against the journal's entries and the keys.
struct Seals<'a> {
    entries: &'a [Entry],
    keys: &'a [Key],
    /// The seals read so far.
    read: Vec<Seal>,
}

impl<'a> Seals<'a> {
    fn new(entries: &'a [Entry], keys: &'a [Key]) -> Self {
        let read = Vec::new();
        Seals {
            entries,
            keys,
            read,
        }
    }

    /// Read the next line: a seal in its place, signed by a known key.
    fn read(&mut self, line: &str) -> Result<(), String> {
        let s = Seal::parse(line)?;
        self.check_place(&s)?;
        self.check_signature(&s)?;
        self.read.push(s);
        Ok(())
    }

    /// Seal n seals entry n, which the journal must hold, and gives the
    /// hash of the seal before it.
    fn check_place(&self, s: &Seal) -> Result<(), String> {
        let n = self.read.len() as u64 + 1;
        if s.entry.seq != n {
            return Err(format!("seq is {}, expected {n}", s.entry.seq));
        }
        if s.entry.hash != self.entry(n)?.hash {
            return Err(format!("entry_hash is not the hash of entry {n}"));
        }
        self.check_prev(s)
    }

    fn entry(&self, n: u64) -> Result<&Entry, String> {
        let held = self.entries.get(n as usize - 1);
        held.ok_or_else(|| format!("seals entry {n}, which the journal does not hold"))
    }

    fn check_prev(&self, s: &Seal) -> Result<(), String> {
        if s.prev.as_ref() != self.read.last().map(|p| &p.hash) {
            return Err("prev is not the hash of the seal before".into());
        }
        Ok(())
    }

    /// The seal's author is a key in keys.jsonl, and signed the entry's
    /// hash.
    fn check_signature(&self, s: &Seal) -> Result<(), String> {
        let by_author = |k: &&Key| k.fingerprint == s.stamp.author;
        let key = self.keys.iter().find(by_author);
        let key = key.ok_or("author is not a key in keys.jsonl")?;
        if !signed(key, &s.entry.hash, &s.stamp.sig) {
            return Err("the signature does not verify".into());
        }
        Ok(())
    }
}

/// Whether a signature is the key's signature of a hash; both are
/// hexadecimal of a length that their records have checked.
fn signed(key: &Key, hash: &str, sig: &str) -> bool {
    let key = VerifyingKey::from_bytes(&key.bytes()).expect("checked in check_keys");
    let sig: [u8; 64] = record::from_hex(sig, 64).unwrap().try_into().unwrap();
    let hash = record::from_hex(hash, 32).unwrap();
    key.verify_strict(&hash, &Signature::from_bytes(&sig))
        .is_ok()
}

/// Every entry must be sealed: the first that is not is an error at its
/// line of the journal.
pub fn all_sealed(seals: &[Seal], entries: &[Entry]) -> Result<(), LineError> {
    if seals.len() >= entries.len() {
        return Ok(());
    }
    let n = seals.len() + 1;
    Err(line_error(
        JOURNAL,
        n + 1,
        format!("entry {n} is not sealed"),
    ))
}

/// Check that each file in `blobs/` is named by the SHA-256 of its content.
pub fn check_blobs(dir: &Path) -> Result<usize, LineError> {
    let names = blob_names(dir)?;
    names.iter().try_for_each(|name| check_blob(dir, name))?;
    Ok(names.len())
}

/// The names of the files in `blobs/`, in order; none if it is not there.
fn blob_names(dir: &Path) -> Result<Vec<String>, LineError> {
    let list = match fs::read_dir(dir.join(BLOBS)) {
        Ok(list) => list,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(line_error(BLOBS, 0, format!("cannot read: {e}"))),
    };
    let names = list.map(|item| item.map(|i| i.file_name().to_string_lossy().into_owned()));
    let names: Result<Vec<String>, io::Error> = names.collect();
    let mut names = names.map_err(|e| line_error(BLOBS, 0, e.to_string()))?;
    names.sort();
    Ok(names)
}

fn check_blob(dir: &Path, name: &str) -> Result<(), LineError> {
    let file = format!("{BLOBS}/{name}");
    if record::sha256_hex(&read_bytes(dir, &file)?) != name {
        return Err(line_error(
            &file,
            0,
            "the name is not the SHA-256 of the content",
        ));
    }
    Ok(())
}

/// Check a whole journal directory. Every entry must be sealed.
pub fn check_dir(dir: &Path) -> Result<Report, LineError> {
    let keys = check_keys(&read(dir, KEYS)?)?;
    let entries = chain::check_journal(&read(dir, JOURNAL)?)?;
    let text = read(dir, SEALS)?;
    let seals = check_seals(&text, &entries, &keys)?;
    all_sealed(&seals, &entries)?;
    let blobs = check_blobs(dir)?;
    Ok(Report {
        keys,
        entries,
        seals,
        blobs,
    })
}
