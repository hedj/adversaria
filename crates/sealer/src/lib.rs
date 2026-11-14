//! The sealer: a separate process that alone holds the author's private key
//! and alone writes `seals.jsonl` (docs/design.md § The journal). For each
//! new entry of `journal.jsonl` it appends a seal: the date from the system
//! clock, the author's key fingerprint, and an Ed25519 signature over the
//! entry's hash. It signs nothing until the journal and its seals check.

use ed25519_dalek::{Signer, SigningKey};
use format::chain::{self, LineError};
use format::record::{
    self, Entry, JOURNAL, KEYS, KEYS_FORMAT, Key, SEALS, SEALS_FORMAT, Seal, Stamp,
};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Format(LineError),
    Refused(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Format(e) => write!(f, "{e}"),
            Error::Refused(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<LineError> for Error {
    fn from(e: LineError) -> Self {
        Error::Format(e)
    }
}

impl From<String> for Error {
    fn from(m: String) -> Self {
        Error::Refused(m)
    }
}

/// A time as ISO 8601 UTC to the second, as 2026-11-12T09:30:00Z.
pub fn utc_date(t: SystemTime) -> String {
    let since = t
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970");
    let secs = since.as_secs() as i64;
    let date = civil_date(secs.div_euclid(86_400));
    let time = clock_time(secs.rem_euclid(86_400));
    format!("{date}T{time}Z")
}

/// The seconds of a day as HH:MM:SS.
fn clock_time(secs: i64) -> String {
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    format!("{h:02}:{m:02}:{s:02}")
}

/// The days since 1970-01-01 as a civil date, YYYY-MM-DD (H. Hinnant,
/// "chrono-Compatible Low-Level Date Algorithms", civil_from_days).
fn civil_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

// Files.

/// Append text to a file and sync it; no text opens nothing.
fn append(path: &Path, text: &str) -> io::Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    let mut f = OpenOptions::new().append(true).open(path)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()
}

/// Start a file, which must not be there yet, with its header.
fn start(path: &Path, format: &str) -> io::Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    f.write_all((record::header(format) + "\n").as_bytes())?;
    f.sync_all()
}

// Keys.

/// The public record of a secret key.
fn public(key: &SigningKey) -> Result<Key, Error> {
    Ok(Key::new(&key.verifying_key().to_bytes())?)
}

fn listed(keys: &[Key], key: &Key) -> bool {
    keys.iter().any(|k| k.fingerprint == key.fingerprint)
}

/// Make a new key: write its secret to `key_path`, readable by its owner
/// only, and add its public half to the journal's `keys.jsonl`.
pub fn init(dir: &Path, key_path: &Path) -> Result<Key, Error> {
    let secret = new_secret()?;
    write_secret(key_path, &secret)?;
    let key = public(&SigningKey::from_bytes(&secret))?;
    publish(dir, &key)?;
    Ok(key)
}

fn new_secret() -> Result<[u8; 32], Error> {
    let mut secret = [0u8; 32];
    let filled = getrandom::fill(&mut secret);
    filled.map_err(|e| format!("no randomness: {e}"))?;
    Ok(secret)
}

/// Write a secret to a file that must not be there yet, for its owner
/// alone to read.
fn write_secret(path: &Path, secret: &[u8; 32]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    let mut f = options
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all((record::to_hex(secret) + "\n").as_bytes())?;
    f.sync_all()
}

/// Add a public key to `keys.jsonl`, creating it if missing.
fn publish(dir: &Path, key: &Key) -> Result<(), Error> {
    fs::create_dir_all(dir)?;
    let path = dir.join(KEYS);
    let keys = check::check_keys(&keys_text(&path)?)?;
    if listed(&keys, key) {
        return Ok(());
    }
    Ok(append(&path, &(key.to_line()? + "\n"))?)
}

/// The text of `keys.jsonl`; a missing or empty file is given its header.
fn keys_text(path: &Path) -> io::Result<String> {
    let text = match fs::read_to_string(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        other => other?,
    };
    if !text.is_empty() {
        return Ok(text);
    }
    let header = record::header(KEYS_FORMAT) + "\n";
    fs::write(path, &header)?;
    Ok(header)
}

/// Read the secret key, refusing a file that others can read.
pub fn load_key(key_path: &Path) -> Result<SigningKey, Error> {
    check_private(key_path)?;
    let text = fs::read_to_string(key_path)?;
    let secret = record::from_hex(text.trim_end_matches('\n'), 32);
    let secret = secret.ok_or_else(|| format!("{}: not a key", key_path.display()))?;
    Ok(SigningKey::from_bytes(&secret.try_into().unwrap()))
}

fn check_private(key_path: &Path) -> Result<(), Error> {
    let mode = fs::metadata(key_path)?.permissions().mode() & 0o777;
    if mode & 0o077 == 0 {
        return Ok(());
    }
    let path = key_path.display();
    Err(format!("{path}: readable by others (mode {mode:o}); it must be 600").into())
}

// Sealing.

/// Seal every complete entry not yet sealed; return how many were sealed.
/// A last line without its newline is still being written, so it waits.
pub fn seal_pending(
    dir: &Path,
    key: &SigningKey,
    clock: &dyn Fn() -> SystemTime,
) -> Result<usize, Error> {
    let author = Author::new(key, clock)?;
    let Some(journal) = Checked::read(dir)? else {
        return Ok(0);
    };
    author.check_listed(&journal.keys)?;
    let sealed = journal.seals(dir)?;
    let lines = author.seal_after(&sealed, &journal.entries)?;
    append(&dir.join(SEALS), &lines.concat())?;
    Ok(lines.len())
}

/// A journal's complete entries and its keys, checked.
struct Checked {
    entries: Vec<Entry>,
    keys: Vec<Key>,
}

impl Checked {
    /// None if the journal has no complete line yet.
    fn read(dir: &Path) -> Result<Option<Checked>, Error> {
        let text = fs::read_to_string(dir.join(JOURNAL))?;
        let complete = complete_lines(&text);
        if complete.is_empty() {
            return Ok(None);
        }
        let entries = chain::check_journal(complete)?;
        let keys = check::check_keys(&fs::read_to_string(dir.join(KEYS))?)?;
        Ok(Some(Checked { entries, keys }))
    }

    /// The seals so far, checked against the entries and the keys; the
    /// file is started if it is not there.
    fn seals(&self, dir: &Path) -> Result<Vec<Seal>, Error> {
        let path = dir.join(SEALS);
        if !path.exists() {
            start(&path, SEALS_FORMAT)?;
        }
        let text = fs::read_to_string(&path)?;
        Ok(check::check_seals(&text, &self.entries, &self.keys)?)
    }
}

/// A text up to its last newline: a last line without one is still being
/// written.
fn complete_lines(text: &str) -> &str {
    &text[..text.rfind('\n').map_or(0, |i| i + 1)]
}

/// Who seals: the secret key, its public record, and the clock.
struct Author<'a> {
    key: &'a SigningKey,
    record: Key,
    clock: &'a dyn Fn() -> SystemTime,
}

impl<'a> Author<'a> {
    fn new(key: &'a SigningKey, clock: &'a dyn Fn() -> SystemTime) -> Result<Self, Error> {
        let record = public(key)?;
        Ok(Author { key, record, clock })
    }

    fn check_listed(&self, keys: &[Key]) -> Result<(), Error> {
        if listed(keys, &self.record) {
            return Ok(());
        }
        Err(Error::Refused(
            "this key is not in keys.jsonl; run sealer init".into(),
        ))
    }

    /// A seal's line for each entry after those sealed, each seal
    /// following the one before it.
    fn seal_after(&self, sealed: &[Seal], entries: &[Entry]) -> Result<Vec<String>, Error> {
        let mut prev = sealed.last().map(|s| s.hash.clone());
        let mut lines = Vec::new();
        for e in &entries[sealed.len()..] {
            let seal = Seal::new(e.id(), self.stamp(&e.hash), prev.as_deref())?;
            lines.push(seal.to_line()? + "\n");
            prev = Some(seal.hash);
        }
        Ok(lines)
    }

    /// The stamp for an entry's hash: this author's signature of it, and
    /// the date now.
    fn stamp(&self, hash: &str) -> Stamp {
        let hash = record::from_hex(hash, 32).expect("checked");
        let sig = record::to_hex(&self.key.sign(&hash).to_bytes());
        let date = utc_date((self.clock)());
        Stamp::new(&date, &self.record.fingerprint, &sig)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn dates() {
        // Catches a wrong calendar conversion, at leap days and centuries.
        let at = |s: u64| utc_date(UNIX_EPOCH + Duration::from_secs(s));
        assert_eq!(at(0), "1970-01-01T00:00:00Z");
        assert_eq!(at(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(at(1_709_251_199), "2024-02-29T23:59:59Z");
        assert_eq!(at(1_794_000_000), "2026-11-06T21:20:00Z"); // checked with Python datetime
        assert_eq!(at(4_102_444_800), "2100-01-01T00:00:00Z");
        for s in (0..5_000_000_000u64).step_by(86_399 * 37) {
            assert!(record::is_date(&at(s)), "{}", at(s));
        }
    }

    #[test]
    fn a_key_is_published_once() {
        // Catches a key added to keys.jsonl again when it is already there.
        let dir = std::env::temp_dir().join(format!("adversaria-publish-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let key = public(&SigningKey::from_bytes(&[3u8; 32])).unwrap();
        publish(&dir, &key).unwrap();
        publish(&dir, &key).unwrap();
        let lines = fs::read_to_string(dir.join(KEYS)).unwrap().lines().count();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(lines, 2, "a header and one key");
    }
}
