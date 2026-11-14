//! Milestone 1's acceptance checks (docs/plan.md, Milestone 1).

use format::chain::{self, LineError};
use format::record::{BLOBS, JOURNAL, KEYS, SEALS, sha256_hex};
use format::terms::{self, Term};
use memex::journal::Journal;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("adversaria-m1-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// A clock that ticks one second per call, so dates are reproducible.
fn ticking_clock() -> impl Fn() -> SystemTime {
    let t = std::cell::Cell::new(1_794_000_000u64);
    move || {
        t.set(t.get() + 1);
        UNIX_EPOCH + Duration::from_secs(t.get())
    }
}

/// SplitMix64, so generated journals repeat exactly.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const KINDS: [&str; 9] = [
    "edit", "link", "adopt", "retract", "suppose", "name", "import", "undo", "redo",
];

fn body(rng: &mut Rng, n: u64) -> Value {
    let text: String = (0..rng.below(12))
        .filter_map(|_| char::from_u32(0x20 + rng.below(0x2fff) as u32))
        .collect();
    let term = Term::Compound(
        "date".into(),
        vec![
            Term::Atom(format!("event{n}")),
            Term::Int(1988),
            Term::Float(rng.below(1000) as f64 / 8.0),
        ],
    );
    json!({"item": n, "text": text, "fact": terms::encode(&term).unwrap()})
}

/// Write a journal of `n` entries, with branches and purges, then seal it.
/// Returns the directory and the key file.
fn generate(name: &str, n: u64, purges: u64) -> (PathBuf, PathBuf) {
    let dir = temp_dir(name);
    let key = dir.with_extension("key");
    let _ = fs::remove_file(&key);
    sealer::init(&dir, &key).unwrap();
    let mut j = Journal::open(&dir).unwrap();
    j.durable = false;
    let mut rng = Rng(n);
    j.append("edit", body(&mut rng, 1)).unwrap();
    while (j.entries().len() as u64) < n - purges {
        let last = j.last().unwrap().seq;
        // One entry in ten starts a branch from an earlier entry.
        let parent = if rng.below(10) == 0 {
            1 + rng.below(last)
        } else {
            last
        };
        let kind = KINDS[rng.below(KINDS.len() as u64) as usize];
        j.append_after(Some(parent), kind, body(&mut rng, last + 1))
            .unwrap();
    }
    for i in 0..purges {
        j.purge(2 + 3 * i, "test purge").unwrap();
    }
    fs::create_dir_all(dir.join(BLOBS)).unwrap();
    let blob = b"a figure";
    fs::write(dir.join(BLOBS).join(sha256_hex(blob)), blob).unwrap();
    let k = sealer::load_key(&key).unwrap();
    sealer::seal_pending(&dir, &k, &ticking_clock()).unwrap();
    (dir, key)
}

fn cleanup(dir: &Path, key: &Path) {
    fs::remove_dir_all(dir).unwrap();
    fs::remove_file(key).unwrap();
}

#[test]
fn ten_thousand_entries_check() {
    // Plan: "The checker passes on a journal of 10,000 generated entries."
    let t = Instant::now();
    let (dir, key) = generate("10k", 10_000, 20);
    let made = t.elapsed();
    let t = Instant::now();
    let r = check::check_dir(&dir).unwrap();
    eprintln!(
        "10,000 entries: written and sealed in {made:?}, checked in {:?}",
        t.elapsed()
    );
    assert_eq!(
        (r.entries.len(), r.seals.len(), r.keys.len(), r.blobs),
        (10_000, 10_000, 1, 1)
    );
    assert_eq!(r.entries.iter().filter(|e| e.body().is_none()).count(), 20);
    assert!(
        r.entries
            .iter()
            .any(|e| e.parent_seq() != Some(e.seq - 1) && e.seq > 1),
        "no branch was made"
    );
    cleanup(&dir, &key);
}

/// Check the three files as text, as check_dir does after reading them.
fn check_texts(keys: &str, journal: &str, seals: &str) -> Result<(), LineError> {
    let ks = check::check_keys(keys)?;
    let es = chain::check_journal(journal)?;
    check::check_seals(seals, &es, &ks, true)?;
    Ok(())
}

#[test]
fn any_changed_byte_fails_naming_its_line() {
    // Plan: the checker "fails, naming the line, when any one byte of any
    // record is changed (test every field kind at least once)". Every byte of
    // every line of all three files is changed in turn, so every field is.
    let (dir, key) = generate("bytes", 12, 1);
    let read = |f: &str| fs::read_to_string(dir.join(f)).unwrap();
    let texts = [read(KEYS), read(JOURNAL), read(SEALS)];
    check_texts(&texts[0], &texts[1], &texts[2]).unwrap();
    let mut tried = 0;
    for (which, file) in [KEYS, JOURNAL, SEALS].iter().enumerate() {
        let bytes = texts[which].as_bytes();
        for i in 0..bytes.len() {
            let mut b = bytes.to_vec();
            b[i] ^= 0x01;
            let line = 1 + bytes[..i].iter().filter(|c| **c == b'\n').count();
            let Ok(changed) = String::from_utf8(b) else {
                continue; // not UTF-8: check_dir refuses it when reading; tested below
            };
            let mut t = texts.clone();
            t[which] = changed;
            let e = check_texts(&t[0], &t[1], &t[2])
                .expect_err(&format!("{file} byte {i} changed, still checks"));
            assert_eq!(
                (e.file.as_str(), e.line),
                (*file, line),
                "{file} byte {i}: {e}"
            );
            tried += 1;
        }
    }
    eprintln!("{tried} single-byte changes, each refused at its own line");
    assert!(tried > 5_000);
    cleanup(&dir, &key);
}

#[test]
fn bytes_that_break_utf8_or_blobs_are_named() {
    // Catches a non-UTF-8 line or a changed blob passing, or blamed elsewhere.
    let (dir, key) = generate("utf8", 5, 0);
    let p = dir.join(JOURNAL);
    let good = fs::read(&p).unwrap();
    let line3 = good
        .iter()
        .enumerate()
        .filter(|(_, c)| **c == b'\n')
        .nth(1)
        .unwrap()
        .0
        + 5;
    let mut bad = good.clone();
    bad[line3] = 0xff;
    fs::write(&p, &bad).unwrap();
    let e = check::check_dir(&dir).unwrap_err();
    assert_eq!((e.file.as_str(), e.line), (JOURNAL, 3), "{e}");
    fs::write(&p, &good).unwrap();

    let blob = fs::read_dir(dir.join(BLOBS))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut b = fs::read(&blob).unwrap();
    b[0] ^= 1;
    fs::write(&blob, b).unwrap();
    let e = check::check_dir(&dir).unwrap_err();
    assert!(e.file.starts_with("blobs/") && e.line == 0, "{e}");
    cleanup(&dir, &key);
}

#[test]
fn purged_entry_still_verifies() {
    // Plan: "A purged entry still verifies", after it was sealed.
    let dir = temp_dir("purge");
    let key = dir.with_extension("key");
    let _ = fs::remove_file(&key);
    sealer::init(&dir, &key).unwrap();
    let k = sealer::load_key(&key).unwrap();
    let clock = ticking_clock();
    let mut j = Journal::open(&dir).unwrap();
    j.append("edit", json!({"text": "a private note"})).unwrap();
    j.append("edit", json!({"text": "a public note"})).unwrap();
    assert_eq!(sealer::seal_pending(&dir, &k, &clock).unwrap(), 2);
    j.purge(1, "privacy").unwrap();
    assert_eq!(sealer::seal_pending(&dir, &k, &clock).unwrap(), 1);
    let r = check::check_dir(&dir).unwrap();
    assert_eq!(r.entries[0].body(), None);
    assert!(
        !fs::read_to_string(dir.join(JOURNAL))
            .unwrap()
            .contains("private")
    );
    cleanup(&dir, &key);
}

#[test]
fn replay_rebuilds_the_same_store_twice() {
    // Plan: "Replaying the journal rebuilds the same store twice (same facts,
    // in the same order)." The store is still the list of entries (replay.rs).
    let (dir, key) = generate("replay", 500, 3);
    let a = memex::replay::replay(&dir).unwrap();
    let b = memex::replay::replay(&dir).unwrap();
    assert_eq!(a.len(), 500);
    assert_eq!(a, b);
    cleanup(&dir, &key);
}

#[test]
fn sealer_refusals() {
    // Catches the sealer signing a broken journal, using an exposed key, or
    // signing with a key not in keys.jsonl.
    use std::os::unix::fs::PermissionsExt;
    let (dir, key) = generate("refuse", 5, 0);
    let k = sealer::load_key(&key).unwrap();

    fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(sealer::load_key(&key).is_err());
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).unwrap();

    let p = dir.join(JOURNAL);
    let good = fs::read_to_string(&p).unwrap();
    fs::write(&p, good.replacen("\"item\":3", "\"item\":4", 1)).unwrap();
    let seals_before = fs::read_to_string(dir.join(SEALS)).unwrap();
    assert!(sealer::seal_pending(&dir, &k, &ticking_clock()).is_err());
    assert_eq!(fs::read_to_string(dir.join(SEALS)).unwrap(), seals_before);
    fs::write(&p, &good).unwrap();

    let other = dir.with_extension("other-key");
    let _ = fs::remove_file(&other);
    let elsewhere = temp_dir("refuse-elsewhere");
    sealer::init(&elsewhere, &other).unwrap();
    let ok = sealer::load_key(&other).unwrap();
    assert!(sealer::seal_pending(&dir, &ok, &ticking_clock()).is_err());
    fs::remove_file(&other).unwrap();
    fs::remove_dir_all(&elsewhere).unwrap();
    cleanup(&dir, &key);
}

#[test]
fn torn_last_line_waits() {
    // Catches the sealer signing a line the memex is still writing.
    let (dir, key) = generate("torn", 3, 0);
    let k = sealer::load_key(&key).unwrap();
    let p = dir.join(JOURNAL);
    let mut text = fs::read_to_string(&p).unwrap();
    text.push_str("{\"seq\":4,\"par");
    fs::write(&p, &text).unwrap();
    assert_eq!(sealer::seal_pending(&dir, &k, &ticking_clock()).unwrap(), 0);
    cleanup(&dir, &key);
}

#[test]
fn forged_seal_refused() {
    // Catches seals checked only by their own hash: a forger who recomputes
    // the hash still cannot make the signature.
    let (dir, key) = generate("forge", 4, 0);
    let p = dir.join(SEALS);
    let text = fs::read_to_string(&p).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let s = format::record::Seal::parse(lines[2]).unwrap();
    let sig = format!(
        "{}{}",
        &s.stamp.sig[..127],
        if s.stamp.sig.ends_with('0') { "1" } else { "0" }
    );
    let stamp = format::record::Stamp { sig, ..s.stamp };
    let forged = format::record::Seal::new(s.entry, stamp, s.prev.as_deref()).unwrap();
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    out[2] = forged.to_line().unwrap();
    // Re-chain the seals after it, so only the signature is wrong.
    for i in 3..out.len() {
        let t = format::record::Seal::parse(&out[i]).unwrap();
        let prev = format::record::Seal::parse(&out[i - 1]).unwrap().hash;
        out[i] = format::record::Seal::new(t.entry, t.stamp, Some(&prev))
            .unwrap()
            .to_line()
            .unwrap();
    }
    fs::write(&p, out.join("\n") + "\n").unwrap();
    let e = check::check_dir(&dir).unwrap_err();
    assert_eq!(
        (e.file.as_str(), e.line, e.msg.as_str()),
        (SEALS, 3, "the signature does not verify")
    );
    cleanup(&dir, &key);
}

#[test]
fn consistently_rewritten_journal_refused() {
    // Catches a journal rewritten from entry 2 on, every hash recomputed so
    // the chain checks: only the seals can show it.
    let (dir, key) = generate("rewrite", 6, 0);
    let p = dir.join(JOURNAL);
    let old = chain::check_journal(&fs::read_to_string(&p).unwrap()).unwrap();
    let mut new: Vec<format::record::Entry> = vec![old[0].clone()];
    for e in &old[1..] {
        let mut b = e.body().cloned().unwrap();
        if e.seq == 2 {
            b["text"] = json!("rewritten");
        }
        let parent = e
            .parent_seq()
            .map(|p| (p, new[p as usize - 1].hash.clone()));
        new.push(
            format::record::Entry::new(
                e.seq,
                parent.as_ref().map(|(p, h)| (*p, h.as_str())),
                e.kind(),
                b,
            )
            .unwrap(),
        );
    }
    let mut text = format::record::header(format::record::JOURNAL_FORMAT) + "\n";
    for e in &new {
        text += &(e.to_line().unwrap() + "\n");
    }
    chain::check_journal(&text).expect("the rewritten chain checks on its own");
    fs::write(&p, text).unwrap();
    let e = check::check_dir(&dir).unwrap_err();
    assert_eq!(
        (e.file.as_str(), e.line, e.msg.as_str()),
        (SEALS, 3, "entry_hash is not the hash of entry 2")
    );
    cleanup(&dir, &key);
}

#[test]
fn unsealed_entry_is_named() {
    // Catches a journal accepted while an entry has no seal (no date, no signature).
    let (dir, key) = generate("unsealed", 4, 0);
    Journal::open(&dir)
        .unwrap()
        .append("edit", json!({"text": "late"}))
        .unwrap();
    let e = check::check_dir(&dir).unwrap_err();
    assert_eq!(
        (e.file.as_str(), e.line, e.msg.as_str()),
        (JOURNAL, 6, "entry 5 is not sealed")
    );
    cleanup(&dir, &key);
}
