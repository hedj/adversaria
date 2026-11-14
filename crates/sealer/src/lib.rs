//! The sealer: a separate process that alone holds the author's private key
//! and alone writes `seals.jsonl` (docs/design.md § The journal). For each
//! new entry of `journal.jsonl` it appends a seal: the date from the system
//! clock, the author's key fingerprint, and an Ed25519 signature over the
//! entry's hash. It signs nothing until the journal and its seals check.

use ed25519_dalek::{Signer, SigningKey};
use format::chain::{self, LineError};
use format::record::{self, JOURNAL, KEYS, KEYS_FORMAT, Key, SEALS, SEALS_FORMAT, Seal, Stamp};
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

/// A time as ISO 8601 UTC to the second, as 2026-11-12T09:30:00Z.
pub fn utc_date(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_secs() as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days since 1970-01-01 to a civil date (H. Hinnant, "chrono-Compatible
    // Low-Level Date Algorithms", civil_from_days).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Make a new key: write its secret to `key_path`, readable by its owner
/// only, and add its public half to the journal's `keys.jsonl`.
pub fn init(dir: &Path, key_path: &Path) -> Result<Key, Error> {
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|e| Error::Refused(format!("no randomness: {e}")))?;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(key_path)?;
    f.write_all((record::to_hex(&secret) + "\n").as_bytes())?;
    f.sync_all()?;
    let key = Key::new(&SigningKey::from_bytes(&secret).verifying_key().to_bytes())
        .map_err(Error::Refused)?;
    publish(dir, &key)?;
    Ok(key)
}

/// Add a public key to `keys.jsonl`, creating it if missing.
fn publish(dir: &Path, key: &Key) -> Result<(), Error> {
    fs::create_dir_all(dir)?;
    let path = dir.join(KEYS);
    let mut text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    if text.is_empty() {
        text = record::header(KEYS_FORMAT) + "\n";
        fs::write(&path, &text)?;
    }
    if check::check_keys(&text)?
        .iter()
        .any(|k| k.fingerprint == key.fingerprint)
    {
        return Ok(());
    }
    let mut f = OpenOptions::new().append(true).open(&path)?;
    f.write_all((key.to_line().map_err(Error::Refused)? + "\n").as_bytes())?;
    f.sync_all()?;
    Ok(())
}

/// Read the secret key, refusing a file that others can read.
pub fn load_key(key_path: &Path) -> Result<SigningKey, Error> {
    let mode = fs::metadata(key_path)?.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(Error::Refused(format!(
            "{}: readable by others (mode {:o}); it must be 600",
            key_path.display(),
            mode & 0o777
        )));
    }
    let text = fs::read_to_string(key_path)?;
    let secret = record::from_hex(text.trim_end_matches('\n'), 32)
        .ok_or_else(|| Error::Refused(format!("{}: not a key", key_path.display())))?;
    Ok(SigningKey::from_bytes(&secret.try_into().unwrap()))
}

/// Seal every complete entry not yet sealed; return how many were sealed.
/// A last line without its newline is still being written, so it waits.
pub fn seal_pending(
    dir: &Path,
    key: &SigningKey,
    clock: &dyn Fn() -> SystemTime,
) -> Result<usize, Error> {
    let text = fs::read_to_string(dir.join(JOURNAL))?;
    let complete = &text[..text.rfind('\n').map_or(0, |i| i + 1)];
    if complete.is_empty() {
        return Ok(0);
    }
    let entries = chain::check_journal(complete)?;
    let keys = check::check_keys(&fs::read_to_string(dir.join(KEYS))?)?;
    let me = Key::new(&key.verifying_key().to_bytes()).map_err(Error::Refused)?;
    if !keys.iter().any(|k| k.fingerprint == me.fingerprint) {
        return Err(Error::Refused(
            "this key is not in keys.jsonl; run sealer init".into(),
        ));
    }
    let seals_path = dir.join(SEALS);
    if !seals_path.exists() {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&seals_path)?;
        f.write_all((record::header(SEALS_FORMAT) + "\n").as_bytes())?;
        f.sync_all()?;
    }
    let seals = check::check_seals(&fs::read_to_string(&seals_path)?, &entries, &keys)?;
    let mut prev = seals.last().map(|s| s.hash.clone());
    let mut lines = String::new();
    for e in &entries[seals.len()..] {
        let hash = record::from_hex(&e.hash, 32).expect("checked");
        let sig = record::to_hex(&key.sign(&hash).to_bytes());
        let date = utc_date(clock());
        let stamp = Stamp::new(&date, &me.fingerprint, &sig);
        let s = Seal::new(e.id(), stamp, prev.as_deref()).map_err(Error::Refused)?;
        lines += &(s.to_line().map_err(Error::Refused)? + "\n");
        prev = Some(s.hash);
    }
    if !lines.is_empty() {
        let mut f = OpenOptions::new().append(true).open(&seals_path)?;
        f.write_all(lines.as_bytes())?;
        f.sync_data()?;
    }
    Ok(entries.len() - seals.len())
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
}
