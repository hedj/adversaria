//! The journal's record format pinned by known answers (docs/design.md,
//! The journal format): an entry's and a seal's hash, worked out here from
//! literal canonical JSON, not from the code that writes them, so that a
//! change to the hash rule fails even when writer and reader change
//! together, as every journal already written would then fail to check.

use format::record::{Entry, EntryId, Seal, Stamp, nfc, sha256_hex};

/// The first entry of every journal.
fn init() -> Entry {
    Entry::new(1, None, "edit", serde_json::json!({"op": "init"})).unwrap()
}

#[test]
fn an_entry_s_body_hash_is_the_sha_256_of_its_canonical_body() {
    let literal = r#"{"op":"init"}"#;
    assert_eq!(init().content.hash, sha256_hex(literal.as_bytes()));
}

#[test]
fn an_entry_s_hash_is_the_sha_256_of_its_fields_without_body_or_hash() {
    // Catches a change to which fields the hash covers, or their order.
    let e = init();
    let bh = sha256_hex(br#"{"op":"init"}"#);
    let literal =
        format!(r#"{{"body_hash":"{bh}","kind":"edit","parent":null,"prev":null,"seq":1}}"#);
    assert_eq!(e.hash, sha256_hex(literal.as_bytes()));
}

#[test]
fn an_entry_after_another_names_its_number_and_hash() {
    let first = init();
    let typed =
        serde_json::json!({"op": "text", "item": 2, "add": "Cafe\u{301}", "spans": [[0, 0, 4]]});
    let e = Entry::new(2, Some((1, &first.hash)), "edit", nfc(&typed).unwrap()).unwrap();
    let bh = sha256_hex(r#"{"add":"Café","item":2,"op":"text","spans":[[0,0,4]]}"#.as_bytes());
    assert_eq!(
        e.content.hash, bh,
        "the body in Unicode NFC, its keys sorted"
    );
    let h = &first.hash;
    let literal =
        format!(r#"{{"body_hash":"{bh}","kind":"edit","parent":1,"prev":"{h}","seq":2}}"#);
    assert_eq!(e.hash, sha256_hex(literal.as_bytes()));
}

/// A seal's author's fingerprint and signature: any 32 and 64 bytes, in
/// hexadecimal, as the format checks no more than that on its own.
const AUTHOR: &str = "abababababababababababababababababababababababababababababababab";
const SIG: &str = "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
const DATE: &str = "2026-11-24T09:30:00Z";

/// The first seal, of an entry.
fn sealed(e: &Entry) -> Seal {
    let stamp = Stamp::new(DATE, AUTHOR, SIG);
    Seal::new(EntryId::new(e.seq, &e.hash), stamp, None).unwrap()
}

#[test]
fn a_seal_s_hash_is_the_sha_256_of_its_fields_without_its_hash() {
    let e = init();
    let h = &e.hash;
    let literal = format!(
        r#"{{"author":"{AUTHOR}","date":"{DATE}","entry_hash":"{h}","prev":null,"seq":1,"sig":"{SIG}"}}"#
    );
    assert_eq!(sealed(&e).hash, sha256_hex(literal.as_bytes()));
}

#[test]
fn the_first_entry_s_line_is_this_exact_text() {
    // Catches any change to the line's fields, their names or their order.
    let bh = sha256_hex(br#"{"op":"init"}"#);
    let e = init();
    let h = &e.hash;
    let line = format!(
        r#"{{"body":{{"op":"init"}},"body_hash":"{bh}","hash":"{h}","kind":"edit","parent":null,"prev":null,"seq":1}}"#
    );
    assert_eq!(e.to_line().unwrap(), line);
}
