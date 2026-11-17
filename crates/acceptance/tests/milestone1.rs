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
    write_entries(&dir, n, purges);
    add_blob(&dir, b"a figure");
    let k = sealer::load_key(&key).unwrap();
    sealer::seal_pending(&dir, &k, &ticking_clock()).unwrap();
    (dir, key)
}

/// n entries, of every kind, with branches; then the purges.
fn write_entries(dir: &Path, n: u64, purges: u64) {
    let mut j = Journal::open(dir).unwrap();
    j.durable = false;
    let mut rng = Rng(n);
    j.append("edit", body(&mut rng, 1)).unwrap();
    while (j.entries().len() as u64) < n - purges {
        append_one(&mut j, &mut rng);
    }
    for i in 0..purges {
        j.purge(2 + 3 * i, "test purge").unwrap();
    }
}

/// An entry of a kind at random. One in ten starts a branch from an
/// earlier entry.
fn append_one(j: &mut Journal, rng: &mut Rng) {
    let entry = random_entry(rng, j.last().unwrap().seq);
    let (parent, kind, body) = entry;
    j.append_after(Some(parent), kind, body).unwrap();
}

/// An entry to append after the last: its parent, usually the last but
/// one time in ten any earlier entry, its kind, and its body.
fn random_entry(rng: &mut Rng, last: u64) -> (u64, &'static str, Value) {
    let parent = if rng.below(10) == 0 {
        1 + rng.below(last)
    } else {
        last
    };
    let kind = KINDS[rng.below(KINDS.len() as u64) as usize];
    (parent, kind, body(rng, last + 1))
}

/// A blob, named by the SHA-256 of its content.
fn add_blob(dir: &Path, blob: &[u8]) {
    fs::create_dir_all(dir.join(BLOBS)).unwrap();
    fs::write(dir.join(BLOBS).join(sha256_hex(blob)), blob).unwrap();
}

/// The first blob of a directory's, as the system lists them.
fn first_blob(dir: &Path) -> PathBuf {
    let mut blobs = fs::read_dir(dir.join(BLOBS)).unwrap();
    blobs.next().unwrap().unwrap().path()
}

/// Change a file's first byte.
fn flip_first_byte(file: &Path) {
    let mut b = fs::read(file).unwrap();
    b[0] ^= 1;
    fs::write(file, b).unwrap();
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
    assert_eq!(sizes(&r), (10_000, 10_000, 1, 1));
    assert_eq!(r.entries.iter().filter(|e| e.body().is_none()).count(), 20);
    assert!(r.entries.iter().any(branches), "no branch was made");
    cleanup(&dir, &key);
}

/// How many entries, seals, keys and blobs a checked directory holds.
fn sizes(r: &check::Report) -> (usize, usize, usize, usize) {
    (r.entries.len(), r.seals.len(), r.keys.len(), r.blobs)
}

/// Whether an entry starts a branch: its parent is not the one before it.
fn branches(e: &format::record::Entry) -> bool {
    e.parent_seq() != Some(e.seq - 1) && e.seq > 1
}

/// Check the three files as text, as check_dir does after reading them.
fn check_texts(keys: &str, journal: &str, seals: &str) -> Result<(), LineError> {
    let ks = check::check_keys(keys)?;
    let es = chain::check_journal(journal)?;
    let ss = check::check_seals(seals, &es, &ks)?;
    check::all_sealed(&ss, &es)
}

#[test]
fn any_changed_byte_fails_naming_its_line() {
    // Plan: the checker "fails, naming the line, when any one byte of any
    // record is changed (test every field kind at least once)". Every byte of
    // every line of all three files is changed in turn, so every field is.
    let (dir, key) = generate("bytes", 12, 1);
    let texts = FILES.map(|f| fs::read_to_string(dir.join(f)).unwrap());
    check_texts(&texts[0], &texts[1], &texts[2]).unwrap();
    let tried: usize = (0..3).map(|which| changes_refused(&texts, which)).sum();
    eprintln!("{tried} single-byte changes, each refused at its own line");
    assert!(tried > 5_000);
    cleanup(&dir, &key);
}

/// The three files of a journal directory, in the order checked.
const FILES: [&str; 3] = [KEYS, JOURNAL, SEALS];

/// Each byte of one of the three texts changed in turn: each change is
/// refused at the line it is on. How many changes left UTF-8 to check.
fn changes_refused(texts: &[String; 3], which: usize) -> usize {
    let file = FILES[which];
    let changes = changed_texts(texts, which);
    changes.iter().for_each(|c| c.assert_refused(file));
    changes.len()
}

/// The three texts with one byte changed, the byte, and its line.
struct Change {
    at: usize,
    line: usize,
    texts: [String; 3],
}

impl Change {
    /// The change is refused, at its line.
    fn assert_refused(&self, file: &str) {
        let e = check_all(&self.texts)
            .expect_err(&format!("{file} byte {} changed, still checks", self.at));
        assert_eq!(
            (e.file.as_str(), e.line),
            (file, self.line),
            "{file} byte {}: {e}",
            self.at
        );
    }
}

/// Each change of a byte of one text that leaves it UTF-8 (one that does
/// not, check_dir refuses when reading; tested below).
fn changed_texts(texts: &[String; 3], which: usize) -> Vec<Change> {
    (0..texts[which].len())
        .filter_map(|i| change_at(texts, which, i))
        .collect()
}

fn change_at(texts: &[String; 3], which: usize, i: usize) -> Option<Change> {
    let t = with_byte_changed(texts, which, i)?;
    let line = line_of(&texts[which], i);
    Some(Change {
        at: i,
        line,
        texts: t,
    })
}

/// Check the three texts, in order.
fn check_all(t: &[String; 3]) -> Result<(), LineError> {
    check_texts(&t[0], &t[1], &t[2])
}

/// The three texts with byte i of one changed, if that is still UTF-8.
fn with_byte_changed(texts: &[String; 3], which: usize, i: usize) -> Option<[String; 3]> {
    let mut b = texts[which].as_bytes().to_vec();
    b[i] ^= 0x01;
    let mut t = texts.clone();
    t[which] = String::from_utf8(b).ok()?;
    Some(t)
}

/// The line, from 1, that byte i of a text is on.
fn line_of(text: &str, i: usize) -> usize {
    1 + text.as_bytes()[..i].iter().filter(|c| **c == b'\n').count()
}

#[test]
fn bytes_that_break_utf8_are_named_by_their_line() {
    // Catches a non-UTF-8 line passing, or blamed elsewhere.
    let (dir, key) = generate("utf8", 5, 0);
    let p = dir.join(JOURNAL);
    let mut bad = fs::read(&p).unwrap();
    let at = start_of_line(&bad, 3) + 4;
    bad[at] = 0xff;
    fs::write(&p, &bad).unwrap();
    let e = check::check_dir(&dir).unwrap_err();
    assert_eq!((e.file.as_str(), e.line), (JOURNAL, 3), "{e}");
    cleanup(&dir, &key);
}

/// Where line n, from 1, begins.
fn start_of_line(bytes: &[u8], n: usize) -> usize {
    let ends = bytes.iter().enumerate().filter(|(_, c)| **c == b'\n');
    ends.map(|(i, _)| i).nth(n - 2).unwrap() + 1
}

#[test]
fn a_changed_blob_is_named() {
    // Catches a changed blob passing, or blamed elsewhere.
    let (dir, key) = generate("blob", 5, 0);
    flip_first_byte(&first_blob(&dir));
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

// The next three catch the sealer using an exposed key, signing a broken
// journal, or signing with a key not in keys.jsonl.

#[test]
fn the_sealer_refuses_a_key_others_can_read() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, key) = generate("refuse-key", 5, 0);
    fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(sealer::load_key(&key).is_err());
    cleanup(&dir, &key);
}

#[test]
fn the_sealer_refuses_a_broken_journal_and_writes_no_seal() {
    let (dir, key) = generate("refuse-broken", 5, 0);
    let k = sealer::load_key(&key).unwrap();
    let p = dir.join(JOURNAL);
    let good = fs::read_to_string(&p).unwrap();
    fs::write(&p, good.replacen("\"item\":3", "\"item\":4", 1)).unwrap();
    let seals_before = fs::read_to_string(dir.join(SEALS)).unwrap();
    assert!(sealer::seal_pending(&dir, &k, &ticking_clock()).is_err());
    assert_eq!(fs::read_to_string(dir.join(SEALS)).unwrap(), seals_before);
    cleanup(&dir, &key);
}

#[test]
fn the_sealer_refuses_a_key_not_in_the_journals_keys() {
    let (dir, key) = generate("refuse-other", 5, 0);
    let (elsewhere, other) = (
        temp_dir("refuse-elsewhere"),
        dir.with_extension("other-key"),
    );
    let _ = fs::remove_file(&other);
    sealer::init(&elsewhere, &other).unwrap();
    let ok = sealer::load_key(&other).unwrap();
    assert!(sealer::seal_pending(&dir, &ok, &ticking_clock()).is_err());
    cleanup(&elsewhere, &other);
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

/// A seal's line.
fn seal_line(s: &format::record::Seal) -> String {
    s.to_line().unwrap()
}

/// The seals' lines with seal n's signature changed in its last digit.
fn forged_at(lines: &[String], n: usize) -> Vec<String> {
    let mut out = lines.to_vec();
    out[n] = forged(&lines[n]);
    out
}

/// A seal's line with its signature's last digit changed.
fn forged(line: &str) -> String {
    let s = format::record::Seal::parse(line).unwrap();
    let stamp = format::record::Stamp {
        sig: flipped(&s.stamp.sig),
        ..s.stamp
    };
    seal_line(&format::record::Seal::new(s.entry, stamp, s.prev.as_deref()).unwrap())
}

/// A signature with its last hex digit changed.
fn flipped(sig: &str) -> String {
    let last = if sig.ends_with('0') { "1" } else { "0" };
    format!("{}{last}", &sig[..127])
}

/// The seals' lines after line n made again, each following the one
/// before, so only what was changed is wrong.
fn rechained(mut lines: Vec<String>, n: usize) -> Vec<String> {
    for i in n + 1..lines.len() {
        let prev = hash_of(&lines[i - 1]);
        lines[i] = following(&lines[i], &prev);
    }
    lines
}

/// The hash of a seal's line.
fn hash_of(line: &str) -> String {
    format::record::Seal::parse(line).unwrap().hash
}

fn read_lines(p: &Path) -> Vec<String> {
    fs::read_to_string(p)
        .unwrap()
        .lines()
        .map(String::from)
        .collect()
}

fn write_lines(p: &Path, lines: &[String]) {
    fs::write(p, lines.join("\n") + "\n").unwrap();
}

#[test]
fn forged_seal_refused() {
    // Catches seals checked only by their own hash: a forger who recomputes
    // the hash still cannot make the signature.
    let (dir, key) = generate("forge", 4, 0);
    let p = dir.join(SEALS);
    let forged = forged_at(&read_lines(&p), 2);
    write_lines(&p, &rechained(forged, 2));
    let want = (SEALS.into(), 3, "the signature does not verify".into());
    assert_eq!(first_error(&dir), want);
    cleanup(&dir, &key);
}

/// The entries with entry 2's text rewritten, and every hash after it
/// made again, so that the chain checks.
fn rewritten(old: &[format::record::Entry]) -> Vec<format::record::Entry> {
    let mut new: Vec<format::record::Entry> = vec![old[0].clone()];
    for e in &old[1..] {
        let again = rewrite_one(e, &new);
        new.push(again);
    }
    new
}

/// An entry made again after those made so far.
fn rewrite_one(e: &format::record::Entry, new: &[format::record::Entry]) -> format::record::Entry {
    let b = body_rewritten(e);
    let parent = parent_in(new, e);
    format::record::Entry::new(e.seq, parent, e.kind(), b).unwrap()
}

/// An entry's body, its text rewritten if it is entry 2.
fn body_rewritten(e: &format::record::Entry) -> Value {
    let mut b = e.body().cloned().unwrap();
    if e.seq == 2 {
        b["text"] = json!("rewritten");
    }
    b
}

/// An entry's parent among these, as Entry::new names a parent.
fn parent_in<'a>(
    new: &'a [format::record::Entry],
    e: &format::record::Entry,
) -> Option<(u64, &'a str)> {
    let p = &new[e.parent_seq()? as usize - 1];
    Some((p.seq, p.hash.as_str()))
}

/// The journal's text, rewritten from entry 2.
fn rewritten_text(p: &Path) -> String {
    let old = chain::check_journal(&fs::read_to_string(p).unwrap()).unwrap();
    journal_text(&rewritten(&old))
}

/// A journal's text, of these entries.
fn journal_text(entries: &[format::record::Entry]) -> String {
    let mut text = format::record::header(format::record::JOURNAL_FORMAT) + "\n";
    for e in entries {
        text += &(e.to_line().unwrap() + "\n");
    }
    text
}

#[test]
fn consistently_rewritten_journal_refused() {
    // Catches a journal rewritten from entry 2 on, every hash recomputed so
    // the chain checks: only the seals can show it.
    let (dir, key) = generate("rewrite", 6, 0);
    let p = dir.join(JOURNAL);
    let text = rewritten_text(&p);
    chain::check_journal(&text).expect("the rewritten chain checks on its own");
    fs::write(&p, text).unwrap();
    let want = (
        SEALS.into(),
        3,
        "entry_hash is not the hash of entry 2".into(),
    );
    assert_eq!(first_error(&dir), want);
    cleanup(&dir, &key);
}

#[test]
fn unsealed_entry_is_named() {
    // Catches a journal accepted while an entry has no seal (no date, no signature).
    let (dir, key) = generate("unsealed", 4, 0);
    let mut j = Journal::open(&dir).unwrap();
    j.append("edit", json!({"text": "late"})).unwrap();
    let want = (JOURNAL.into(), 6, "entry 5 is not sealed".into());
    assert_eq!(first_error(&dir), want);
    cleanup(&dir, &key);
}

/// The first error of a checked directory: its file, line and message.
fn first_error(dir: &Path) -> (String, usize, String) {
    let e = check::check_dir(dir).unwrap_err();
    (e.file, e.line, e.msg)
}

#[test]
fn a_seal_that_does_not_follow_the_seal_before_is_refused() {
    // Catches a seals file whose chain is not checked: a seal rewritten to
    // follow some other seal still has its true signature.
    let (dir, key) = generate("seal-prev", 3, 0);
    let p = dir.join(SEALS);
    let mut lines = read_lines(&p);
    lines[2] = following(&lines[2], &sha256_hex(b"no seal"));
    write_lines(&p, &lines);
    let want = "prev is not the hash of the seal before";
    assert_eq!(first_error(&dir), (SEALS.into(), 3, want.into()));
    cleanup(&dir, &key);
}

/// A seal's line, made again to follow the seal of another hash.
fn following(line: &str, prev: &str) -> String {
    let s = format::record::Seal::parse(line).unwrap();
    seal_line(&format::record::Seal::new(s.entry, s.stamp, Some(prev)).unwrap())
}

#[test]
fn a_key_listed_twice_is_refused() {
    // Catches keys.jsonl accepted with the same key on two lines.
    let (dir, key) = generate("key-twice", 1, 0);
    let p = dir.join(KEYS);
    let text = fs::read_to_string(&p).unwrap();
    let again = text.lines().nth(1).unwrap();
    fs::write(&p, format!("{text}{again}\n")).unwrap();
    let want = "this key is listed twice";
    assert_eq!(first_error(&dir), (KEYS.into(), 3, want.into()));
    cleanup(&dir, &key);
}
