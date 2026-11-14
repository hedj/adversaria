//! The records of a journal directory, one per line (docs/design.md
//! § The journal format), and the checks a single line can make on itself.
//!
//! Every line is canonical JSON (RFC 8785), so changing any byte of a line
//! either breaks its canonical form or changes a value its own `hash` covers.
//! A record's hash is the SHA-256, in lowercase hexadecimal, of the canonical
//! JSON of the record without its `hash` field, and, for an entry, without its
//! `body` too: the entry's `body_hash` stands in for the body, so a purged
//! body (null) leaves the hash checkable. Fields a reader does not know are
//! kept, and covered by the hash.

use crate::jcs;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use unicode_normalization::{UnicodeNormalization, is_nfc};

pub const JOURNAL: &str = "journal.jsonl";
pub const SEALS: &str = "seals.jsonl";
pub const KEYS: &str = "keys.jsonl";
pub const BLOBS: &str = "blobs";

pub const VERSION: u64 = 1;

/// The kinds of entry in format version 1.
/// `view` (a change to what is shown: cursor, folds, panes, a Leap) was added
/// to version 1 before any journal was kept, for milestone 3.
pub const KINDS: [&str; 11] = [
    "edit", "link", "adopt", "retract", "suppose", "name", "import", "purge", "undo", "redo",
    "view",
];

/// The header line of each file: `{"format":"memex-journal","version":1}`.
pub fn header(format: &str) -> String {
    jcs::dumps(&json!({"format": format, "version": VERSION})).expect("a header is canonical")
}

pub const JOURNAL_FORMAT: &str = "memex-journal";
pub const SEALS_FORMAT: &str = "memex-seals";
pub const KEYS_FORMAT: &str = "memex-keys";

pub fn sha256_hex(bytes: &[u8]) -> String {
    to_hex(&Sha256::digest(bytes))
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Lowercase hexadecimal of exactly `n` bytes -> the bytes.
pub fn from_hex(s: &str, n: usize) -> Option<Vec<u8>> {
    if s.len() != 2 * n
        || !s
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return None;
    }
    (0..n)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}

fn is_hash(v: &Value) -> bool {
    v.as_str().is_some_and(|s| from_hex(s, 32).is_some())
}

/// Hash of a record: SHA-256 of its canonical JSON without `hash` (and `body`).
fn record_hash(obj: &Map<String, Value>, drop_body: bool) -> String {
    let mut m = obj.clone();
    m.remove("hash");
    if drop_body {
        m.remove("body");
    }
    sha256_hex(&jcs::canonical_bytes(&Value::Object(m)).expect("a parsed line is canonical"))
}

/// The hash of an entry's body.
pub fn body_hash(body: &Value) -> Result<String, String> {
    Ok(sha256_hex(&jcs::canonical_bytes(body).map_err(|e| e.0)?))
}

/// The value with every string and object key in Unicode NFC, as the
/// format requires. Refuses an object whose keys become equal once
/// normalised, rather than drop one of them.
pub fn nfc(v: &Value) -> Result<Value, String> {
    Ok(match v {
        Value::String(s) => Value::String(s.nfc().collect()),
        Value::Array(a) => Value::Array(a.iter().map(nfc).collect::<Result<_, _>>()?),
        Value::Object(m) => {
            let mut out = Map::new();
            for (k, x) in m {
                let k2: String = k.nfc().collect();
                if out.insert(k2.clone(), nfc(x)?).is_some() {
                    return Err(format!("two keys are both {k2:?} in Unicode NFC"));
                }
            }
            Value::Object(out)
        }
        other => other.clone(),
    })
}

/// Whether every string and object key in the value is in Unicode NFC.
pub fn all_nfc(v: &Value) -> bool {
    match v {
        Value::String(s) => is_nfc(s),
        Value::Array(a) => a.iter().all(all_nfc),
        Value::Object(m) => m.iter().all(|(k, x)| is_nfc(k) && all_nfc(x)),
        _ => true,
    }
}

/// The fields of one line, which is the canonical JSON of an object, read
/// by key: each reading checks its field and says what is wrong with it.
struct Fields(Map<String, Value>);

/// What a hash must be, for an error.
const HASH: &str = "64 lowercase hex digits";

impl Fields {
    fn read(line: &str) -> Result<Fields, String> {
        let v: Value = serde_json::from_str(line).map_err(|e| format!("not JSON: {e}"))?;
        check_canonical(&v, line)?;
        match v {
            Value::Object(m) => Ok(Fields(m)),
            _ => Err("not a JSON object".into()),
        }
    }

    fn need(&self, key: &str) -> Result<&Value, String> {
        self.0
            .get(key)
            .ok_or_else(|| format!("missing field {key:?}"))
    }

    fn natural(&self, key: &str) -> Result<u64, String> {
        natural(self.need(key)?, key)
    }

    /// A field's text, or the empty text if it is not text.
    fn text(&self, key: &str) -> Result<&str, String> {
        Ok(self.need(key)?.as_str().unwrap_or(""))
    }

    /// A field that is `bytes` bytes in lowercase hexadecimal; if not, it
    /// "must be" what `what` says.
    fn hex(&self, key: &str, bytes: usize, what: &str) -> Result<String, String> {
        let text = self.text(key)?;
        let found = from_hex(text, bytes).map(|_| text.to_string());
        found.ok_or_else(|| format!("{key} must be {what}"))
    }

    /// The record's own hash, which must be the hash of the record.
    fn own_hash(&self, drop_body: bool) -> Result<String, String> {
        let hash = self.hex("hash", 32, HASH)?;
        if hash != record_hash(&self.0, drop_body) {
            return Err("hash does not match the record".into());
        }
        Ok(hash)
    }

    // An entry's fields.

    fn parent(&self) -> Result<Option<u64>, String> {
        match self.need("parent")? {
            Value::Null => Ok(None),
            v => natural(v, "parent").map(Some),
        }
    }

    /// The parent's hash: there exactly when there is a parent.
    fn prev(&self, parent: Option<u64>) -> Result<Option<String>, String> {
        match (self.need("prev")?, parent) {
            (Value::Null, None) => Ok(None),
            (v, Some(_)) if is_hash(v) => Ok(v.as_str().map(String::from)),
            _ => Err("prev must be null exactly when parent is, else a hash".into()),
        }
    }

    fn kind(&self) -> Result<String, String> {
        let kind = self.text("kind")?;
        if !KINDS.contains(&kind) {
            return Err(format!("unknown kind {}", self.0["kind"]));
        }
        Ok(kind.to_string())
    }

    /// The body: an object that its body_hash is the hash of, or none
    /// once purged.
    fn body(&self) -> Result<Option<Value>, String> {
        match self.need("body")? {
            Value::Null => Ok(None),
            b @ Value::Object(_) => self.checked_body(b).map(Some),
            _ => Err("body must be an object, or null when purged".into()),
        }
    }

    fn checked_body(&self, body: &Value) -> Result<Value, String> {
        if body_hash(body)? != self.hex("body_hash", 32, HASH)? {
            return Err("body_hash does not match the body".into());
        }
        Ok(body.clone())
    }

    // A seal's fields.

    fn date(&self) -> Result<String, String> {
        let date = self.text("date")?;
        if !is_date(date) {
            return Err("date must be ISO 8601 UTC to the second, as 2026-11-12T09:30:00Z".into());
        }
        Ok(date.to_string())
    }

    /// The hash of the seal before: none for the first.
    fn prev_seal(&self) -> Result<Option<String>, String> {
        match self.need("prev")? {
            Value::Null => Ok(None),
            v if is_hash(v) => Ok(v.as_str().map(String::from)),
            _ => Err("prev must be null or a hash".into()),
        }
    }

    // A key's fields.

    /// The public key, which its fingerprint must be the hash of.
    fn key(&self) -> Result<String, String> {
        let key = self.hex("key", 32, HASH)?;
        let bytes = from_hex(&key, 32).unwrap_or_default();
        if self.text("fingerprint")? != sha256_hex(&bytes) {
            return Err("fingerprint is not the SHA-256 of the key".into());
        }
        Ok(key)
    }
}

/// A line must be the canonical JSON of its value, with every string in
/// Unicode NFC.
fn check_canonical(v: &Value, line: &str) -> Result<(), String> {
    if jcs::dumps(v).map_err(|e| e.0)? != line {
        return Err("not canonical JSON (RFC 8785), or a key is repeated".into());
    }
    if !all_nfc(v) {
        return Err("a string is not in Unicode NFC".into());
    }
    Ok(())
}

fn natural(v: &Value, what: &str) -> Result<u64, String> {
    match v.as_u64() {
        Some(n) if n >= 1 && n <= jcs::MAX_SAFE_INTEGER as u64 => Ok(n),
        _ => Err(format!("{what} must be an integer from 1 to 2^53 - 1")),
    }
}

/// The canonical JSON of a record's fields, as one line.
fn line_of(fields: Value) -> Result<String, String> {
    jcs::dumps(&fields).map_err(|e| e.0)
}

/// The hash of a record made of these fields.
fn hash_of(fields: &Value, drop_body: bool) -> String {
    let empty = Map::new();
    record_hash(fields.as_object().unwrap_or(&empty), drop_body)
}

/// Check a header line against the format it should name.
pub fn check_header(line: &str, format: &str) -> Result<(), String> {
    if line == header(format) {
        return Ok(());
    }
    let f = Fields::read(line)?;
    match (f.0.get("format"), f.0.get("version")) {
        (Some(f), _) if f != format => Err(format!("header names format {f}, expected {format:?}")),
        (_, Some(v)) if v != VERSION => Err(format!("format version {v} is not supported")),
        _ => Err(format!("header must be exactly {}", header(format))),
    }
}

/// One entry of `journal.jsonl`.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub seq: u64,
    pub parent: Option<u64>,
    pub prev: Option<String>,
    pub kind: String,
    /// None once purged.
    pub body: Option<Value>,
    pub body_hash: String,
    pub hash: String,
}

impl Entry {
    /// Make a new entry; the body must be an object.
    pub fn new(
        seq: u64,
        parent: Option<(u64, &str)>,
        kind: &str,
        body: Value,
    ) -> Result<Entry, String> {
        check_new(kind, &body)?;
        let mut entry = Entry {
            seq,
            parent: parent.map(|p| p.0),
            prev: parent.map(|p| p.1.to_string()),
            kind: kind.into(),
            body_hash: body_hash(&body)?,
            body: Some(body),
            hash: String::new(),
        };
        entry.hash = hash_of(&entry.fields(), true);
        Entry::parse(&entry.to_line()?)
    }

    /// The entry's fields, as the format names them.
    fn fields(&self) -> Value {
        json!({
            "seq": self.seq, "parent": self.parent, "prev": self.prev, "kind": self.kind,
            "body": self.body, "body_hash": self.body_hash, "hash": self.hash,
        })
    }

    /// The entry as one line of canonical JSON, without its newline.
    pub fn to_line(&self) -> Result<String, String> {
        line_of(self.fields())
    }

    /// The same entry with its body purged.
    pub fn purged(&self) -> Entry {
        Entry {
            body: None,
            ..self.clone()
        }
    }

    /// Parse a line and check everything the line can check on itself.
    pub fn parse(line: &str) -> Result<Entry, String> {
        let f = Fields::read(line)?;
        let seq = f.natural("seq")?;
        let parent = f.parent()?;
        let prev = f.prev(parent)?;
        if parent.is_some_and(|p| p >= seq) {
            return Err("parent must come before the entry".into());
        }
        Ok(Entry {
            seq,
            parent,
            prev,
            kind: f.kind()?,
            body_hash: f.hex("body_hash", 32, HASH)?,
            body: f.body()?,
            hash: f.own_hash(true)?,
        })
    }
}

/// What a new entry must be: of a known kind, with an object for its
/// body, in Unicode NFC.
fn check_new(kind: &str, body: &Value) -> Result<(), String> {
    if !KINDS.contains(&kind) {
        return Err(format!("unknown kind {kind:?}"));
    }
    if !body.is_object() {
        return Err("a body must be a JSON object".into());
    }
    if !all_nfc(body) {
        return Err(
            "a string in the body is not in Unicode NFC; normalise it with record::nfc".into(),
        );
    }
    Ok(())
}

/// One record of `seals.jsonl`. The sealer signs the 32 bytes of the entry's hash.
#[derive(Debug, Clone, PartialEq)]
pub struct Seal {
    pub seq: u64,
    pub entry_hash: String,
    pub date: String,
    pub author: String,
    pub sig: String,
    pub prev: Option<String>,
    pub hash: String,
}

impl Seal {
    /// Make a seal record and its hash.
    pub fn new(
        seq: u64,
        entry_hash: &str,
        date: &str,
        author: &str,
        sig: &str,
        prev: Option<&str>,
    ) -> Result<Seal, String> {
        let mut fields = json!({
            "seq": seq, "entry_hash": entry_hash, "date": date,
            "author": author, "sig": sig, "prev": prev,
        });
        fields["hash"] = json!(hash_of(&fields, false));
        Seal::parse(&line_of(fields)?)
    }

    pub fn to_line(&self) -> Result<String, String> {
        line_of(json!({
            "seq": self.seq, "entry_hash": self.entry_hash, "date": self.date,
            "author": self.author, "sig": self.sig, "prev": self.prev, "hash": self.hash,
        }))
    }

    pub fn parse(line: &str) -> Result<Seal, String> {
        let f = Fields::read(line)?;
        Ok(Seal {
            seq: f.natural("seq")?,
            entry_hash: f.hex("entry_hash", 32, HASH)?,
            date: f.date()?,
            author: f.hex("author", 32, "a key fingerprint, 64 lowercase hex digits")?,
            sig: f.hex("sig", 64, "128 lowercase hex digits")?,
            prev: f.prev_seal()?,
            hash: f.own_hash(false)?,
        })
    }
}

/// ISO 8601 UTC to the second, with a valid calendar date and time.
pub fn is_date(s: &str) -> bool {
    let parts = s.strip_suffix('Z').and_then(|s| s.split_once('T'));
    parts.is_some_and(|(date, time)| valid_date(date) && valid_time(time))
}

fn valid_date(text: &str) -> bool {
    match numbers(text, "0000-00-00").as_deref() {
        Some([y, mo, d]) => (1..=12).contains(mo) && (1..=days_in(*y, *mo)).contains(d),
        _ => false,
    }
}

fn valid_time(text: &str) -> bool {
    match numbers(text, "00:00:00").as_deref() {
        Some([h, mi, se]) => *h < 24 && *mi < 60 && *se < 60,
        _ => false,
    }
}

/// The numbers in a text that has a form's shape: a digit wherever the
/// form has 0, and the form's own character elsewhere.
fn numbers(text: &str, form: &str) -> Option<Vec<u32>> {
    let fits = |(c, f): (u8, u8)| {
        if f == b'0' {
            c.is_ascii_digit()
        } else {
            c == f
        }
    };
    if text.len() != form.len() || !text.bytes().zip(form.bytes()).all(fits) {
        return None;
    }
    let runs = text.split(|c: char| !c.is_ascii_digit());
    Some(runs.filter_map(|n| n.parse().ok()).collect())
}

fn days_in(year: u32, month: u32) -> u32 {
    let divides = |n: u32| year.is_multiple_of(n);
    let leap = (divides(4) && !divides(100)) || divides(400);
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// One record of `keys.jsonl`: an author's Ed25519 public key.
#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    /// The public key, 32 bytes in hex.
    pub key: String,
    /// SHA-256 of the 32 key bytes, in hex.
    pub fingerprint: String,
    pub hash: String,
}

impl Key {
    pub fn new(public: &[u8; 32]) -> Result<Key, String> {
        let (key, fingerprint) = (to_hex(public), sha256_hex(public));
        let mut fields = json!({"alg": "ed25519", "key": key, "fingerprint": fingerprint});
        fields["hash"] = json!(hash_of(&fields, false));
        Key::parse(&line_of(fields)?)
    }

    pub fn to_line(&self) -> Result<String, String> {
        line_of(json!({
            "alg": "ed25519", "key": self.key, "fingerprint": self.fingerprint, "hash": self.hash,
        }))
    }

    pub fn bytes(&self) -> [u8; 32] {
        from_hex(&self.key, 32).unwrap().try_into().unwrap()
    }

    pub fn parse(line: &str) -> Result<Key, String> {
        let f = Fields::read(line)?;
        if f.need("alg")? != "ed25519" {
            return Err("alg must be \"ed25519\"".into());
        }
        Ok(Key {
            key: f.key()?,
            fingerprint: f.text("fingerprint")?.to_string(),
            hash: f.own_hash(false)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_round_trip() {
        // Catches an entry whose line does not parse back to itself.
        let e = Entry::new(1, None, "edit", json!({"text": "Buran flew in 1988"})).unwrap();
        assert_eq!(Entry::parse(&e.to_line().unwrap()).unwrap(), e);
        let f = Entry::new(2, Some((1, &e.hash)), "link", json!({"from": 1, "to": 2})).unwrap();
        assert_eq!(Entry::parse(&f.to_line().unwrap()).unwrap(), f);
    }

    #[test]
    fn purged_entry_keeps_its_hash() {
        // Catches a hash that covers the body itself, so a purge breaks the chain.
        let e = Entry::new(1, None, "edit", json!({"text": "secret"})).unwrap();
        let p = Entry::parse(&e.purged().to_line().unwrap()).unwrap();
        assert_eq!(p.hash, e.hash);
        assert_eq!(p.body, None);
    }

    #[test]
    fn hash_covers_every_field_but_body() {
        // Catches a field left out of the hash, so it could change unnoticed.
        let e = Entry::new(3, Some((2, &"ab".repeat(32))), "edit", json!({"x": 1})).unwrap();
        let line = e.to_line().unwrap();
        for (from, to) in [
            ("\"seq\":3", "\"seq\":4"),
            ("\"parent\":2", "\"parent\":1"),
            ("\"kind\":\"edit\"", "\"kind\":\"link\""),
            (
                &format!("\"prev\":\"{}", "ab".repeat(32))[..],
                &format!("\"prev\":\"{}", "cd".repeat(32))[..],
            ),
        ] {
            let bad = line.replacen(from, to, 1);
            assert_ne!(bad, line, "{from}");
            assert_eq!(
                Entry::parse(&bad).unwrap_err(),
                "hash does not match the record",
                "{from}"
            );
        }
    }

    #[test]
    fn unknown_fields_are_kept_and_hashed() {
        // Catches a reader that rejects later fields, or ignores them in the hash.
        let e = Entry::new(1, None, "edit", json!({})).unwrap();
        let mut m: Map<String, Value> = serde_json::from_str(&e.to_line().unwrap()).unwrap();
        m.insert("zzz".into(), json!("later"));
        let h = record_hash(&m, true);
        m.insert("hash".into(), json!(h));
        let line = jcs::dumps(&Value::Object(m.clone())).unwrap();
        assert!(Entry::parse(&line).is_ok());
        m.insert("zzz".into(), json!("altered"));
        assert!(Entry::parse(&jcs::dumps(&Value::Object(m)).unwrap()).is_err());
    }

    #[test]
    fn entry_refusals() {
        // Catches malformed entries accepted.
        let e = Entry::new(2, Some((1, &"ab".repeat(32))), "edit", json!({"x": 1})).unwrap();
        let line = e.to_line().unwrap();
        let cases = [
            line.replace(":", ": "),                                      // not canonical
            line.replacen("\"body\":{\"x\":1}", "\"body\":{\"x\":2}", 1), // body altered
            line.replacen("\"body\":{\"x\":1}", "\"body\":[1]", 1),       // body not an object
            line.replacen("\"parent\":1", "\"parent\":null", 1),          // prev without parent
            "[]".into(),
            "".into(),
            "{".into(),
        ];
        for c in cases {
            assert!(Entry::parse(&c).is_err(), "accepted {c}");
        }
        assert!(Entry::new(1, None, "frobnicate", json!({})).is_err());
        assert!(Entry::new(1, None, "edit", json!([1])).is_err());
        assert!(Entry::new(1, None, "edit", json!(null)).is_err());
        assert!(Entry::new(1, Some((1, &"ab".repeat(32))), "edit", json!({})).is_err());
    }

    #[test]
    fn nfc_known_cases() {
        // Catches text left decomposed, or composed wrongly.
        let cases = [
            ("e\u{301}", "\u{e9}"),                 // e + acute -> é
            ("\u{212b}", "\u{c5}"),                 // Angstrom sign -> Å (a singleton)
            ("\u{1100}\u{1161}", "\u{ac00}"),       // Hangul jamo -> syllable
            ("a\u{307}\u{323}", "\u{1ea1}\u{307}"), // marks reordered, then composed
            ("plain ASCII", "plain ASCII"),
            ("", ""),
        ];
        for (from, want) in cases {
            assert_eq!(nfc(&json!(from)).unwrap(), json!(want), "{from:?}");
            assert!(all_nfc(&json!(want)));
        }
        assert!(!all_nfc(&json!({"a": ["e\u{301}"]})));
        assert!(!all_nfc(&json!({"e\u{301}": 1})));
    }

    #[test]
    fn nfc_agrees_with_node() {
        // Catches any difference from an independent NFC (Node's ICU, Unicode
        // 17.0; the crate has 18.0) over 10,000 strings of letters, combining
        // marks and Hangul jamo.
        let mut rng = crate::jcs::tests::Rng(15);
        let pools: [(u32, u32); 5] = [
            (0x41, 0x7a),
            (0x300, 0x36f),
            (0x1100, 0x11ff),
            (0xc0, 0x24f),
            (0x1e00, 0x1eff),
        ];
        let strings: Vec<String> = (0..10_000)
            .map(|_| {
                (0..1 + rng.below(6))
                    .filter_map(|_| {
                        let (lo, hi) = pools[rng.below(5) as usize];
                        char::from_u32(lo + rng.below((hi - lo + 1) as u64) as u32)
                    })
                    .collect()
            })
            .collect();
        let want: Vec<String> = serde_json::from_str(&crate::jcs::tests::node(
            "const s=JSON.parse(require('fs').readFileSync(0,'utf8'));\
             process.stdout.write(JSON.stringify(s.map(x=>x.normalize('NFC'))))",
            &serde_json::to_string(&strings).unwrap(),
        ))
        .unwrap();
        let changed = strings.iter().zip(&want).filter(|(s, w)| s != w).count();
        // 663 of the 10,000 need normalising (counted); the floor keeps the sample meaningful.
        assert!(
            changed > 500,
            "too few strings needed normalising: {changed}"
        );
        for (s, w) in strings.iter().zip(&want) {
            assert_eq!(nfc(&json!(s)).unwrap(), json!(w), "{s:?}");
            assert_eq!(all_nfc(&json!(s)), s == w, "{s:?}");
        }
    }

    #[test]
    fn colliding_keys_refused() {
        // Catches two keys merged silently when they normalise to one.
        assert!(nfc(&json!({"\u{e9}": 1, "e\u{301}": 2})).is_err());
    }

    #[test]
    fn non_nfc_line_refused() {
        // Catches a line accepted with text not in NFC, though its hashes are right.
        assert!(Entry::new(1, None, "edit", json!({"t": "e\u{301}"})).is_err());
        let body = json!({"t": "e\u{301}"});
        let mut m = Map::new();
        m.insert("seq".into(), json!(1));
        m.insert("parent".into(), Value::Null);
        m.insert("prev".into(), Value::Null);
        m.insert("kind".into(), json!("edit"));
        m.insert("body_hash".into(), json!(body_hash(&body).unwrap()));
        let h = record_hash(&m, true);
        m.insert("body".into(), body);
        m.insert("hash".into(), json!(h));
        let line = jcs::dumps(&Value::Object(m)).unwrap();
        assert_eq!(
            Entry::parse(&line).unwrap_err(),
            "a string is not in Unicode NFC"
        );
        assert!(
            Entry::parse(&line.replace("e\u{301}", "\u{e9}")).is_err(),
            "hash still covers the text"
        );
    }

    /// The line of a record made of these fields, with its hash added: a
    /// record that is wrong only in what a test puts in its fields.
    fn hashed_line(mut fields: Value, drop_body: bool) -> String {
        fields["hash"] = json!(hash_of(&fields, drop_body));
        line_of(fields).unwrap()
    }

    #[test]
    fn a_first_entry_with_a_parents_hash_is_refused() {
        // Catches prev accepted where there is no parent.
        let body = json!({});
        let fields = json!({
            "seq": 1, "parent": null, "prev": sha256_hex(b"no entry"), "kind": "edit",
            "body": body, "body_hash": body_hash(&body).unwrap(),
        });
        assert_eq!(
            Entry::parse(&hashed_line(fields, true)),
            Err("prev must be null exactly when parent is, else a hash".into())
        );
    }

    #[test]
    fn a_key_with_another_keys_fingerprint_is_refused() {
        // Catches a fingerprint that is not checked against its key.
        let fields = json!({
            "alg": "ed25519", "key": to_hex(&[7u8; 32]), "fingerprint": sha256_hex(&[8u8; 32]),
        });
        assert_eq!(
            Key::parse(&hashed_line(fields, false)),
            Err("fingerprint is not the SHA-256 of the key".into())
        );
    }

    #[test]
    fn dates() {
        // Catches malformed or impossible dates accepted, or valid ones refused.
        for good in [
            "2026-11-12T09:30:00Z",
            "2024-02-29T23:59:59Z",
            "2000-02-29T00:00:00Z",
        ] {
            assert!(is_date(good), "{good}");
        }
        for bad in [
            "2026-11-12T09:30:00",
            "2026-11-12 09:30:00Z",
            "2026-13-01T00:00:00Z",
            "2026-02-29T00:00:00Z",
            "1900-02-29T00:00:00Z",
            "2026-11-12T24:00:00Z",
            "2026-11-12T09:30:60Z",
            "2026-11-12T09:30:00.5Z",
            "",
            "2026-1a-12T09:30:00Z",
            "2026/11/12T09:30:00Z",
            "2026-11-12T09.30.00Z",
        ] {
            assert!(!is_date(bad), "{bad}");
        }
    }

    #[test]
    fn headers() {
        // Catches a wrong format name or version accepted.
        assert_eq!(
            header(JOURNAL_FORMAT),
            r#"{"format":"memex-journal","version":1}"#
        );
        assert!(check_header(&header(JOURNAL_FORMAT), JOURNAL_FORMAT).is_ok());
        assert!(check_header(&header(SEALS_FORMAT), JOURNAL_FORMAT).is_err());
        assert!(check_header(r#"{"format":"memex-journal","version":2}"#, JOURNAL_FORMAT).is_err());
        assert!(
            check_header(
                r#"{"format":"memex-journal","version":1,"x":1}"#,
                JOURNAL_FORMAT
            )
            .is_err()
        );
    }

    #[test]
    fn key_and_seal_round_trip() {
        // Catches key or seal records that do not parse back to themselves.
        let k = Key::new(&[7u8; 32]).unwrap();
        assert_eq!(Key::parse(&k.to_line().unwrap()).unwrap(), k);
        let s = Seal::new(
            1,
            &"ab".repeat(32),
            "2026-11-12T09:30:00Z",
            &k.fingerprint,
            &"0".repeat(128),
            None,
        )
        .unwrap();
        assert_eq!(Seal::parse(&s.to_line().unwrap()).unwrap(), s);
    }
}
