//! Prolog terms as JSON, as docs/design.md § The journal format defines them.
//!
//! | Prolog   | JSON                                              |
//! | integer  | a JSON integer, at most 2^53 - 1 in magnitude     |
//! | float    | `{"float": text}`, text canonical ([`float_text`]) |
//! | string   | a JSON string                                     |
//! | atom     | `{"a": name}`                                     |
//! | variable | `{"v": n}`                                        |
//! | compound | `{"f": name, "args": [...]}`                      |
//!
//! Floats are tagged because RFC 8785 writes 1.0 and 1 alike, and Prolog
//! tells them apart.

use crate::jcs::{self, MAX_SAFE_INTEGER};
use serde_json::{Map, Value, json};
use std::fmt;

#[derive(Debug, Clone)]
pub enum Term {
    Int(i64),
    Float(f64),
    Str(String),
    Atom(String),
    Var(u64),
    Compound(String, Vec<Term>),
}

/// Equal and of the same kind throughout; floats compared by bit pattern,
/// so 0.0 and -0.0 differ, as they do in Prolog.
impl PartialEq for Term {
    fn eq(&self, other: &Self) -> bool {
        use Term::*;
        match (self, other) {
            (Int(a), Int(b)) => a == b,
            (Float(a), Float(b)) => a.to_bits() == b.to_bits(),
            (Str(a), Str(b)) | (Atom(a), Atom(b)) => a == b,
            (Var(a), Var(b)) => a == b,
            (Compound(f, a), Compound(g, b)) => f == g && a == b,
            _ => false,
        }
    }
}

/// A value that is not a term in the journal's encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermError(pub String);

impl fmt::Display for TermError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TermError {}

impl From<jcs::CanonicalError> for TermError {
    fn from(e: jcs::CanonicalError) -> Self {
        TermError(e.0)
    }
}

fn err<T>(msg: impl Into<String>) -> Result<T, TermError> {
    Err(TermError(msg.into()))
}

fn safe(n: i64) -> Result<i64, TermError> {
    if n.unsigned_abs() > MAX_SAFE_INTEGER as u64 {
        return err(format!("integer {n} exceeds 2^53 - 1"));
    }
    Ok(n)
}

/// The one canonical text of a float: ECMAScript's shortest form, with ".0"
/// added when it would read as an integer; -0.0 keeps its sign.
pub fn float_text(x: f64) -> Result<String, TermError> {
    if !x.is_finite() {
        return err(format!("{x} is not a finite float"));
    }
    if x == 0.0 {
        return Ok(if x.is_sign_negative() { "-0.0" } else { "0.0" }.into());
    }
    let text = jcs::number(x)?;
    let whole = !text.contains('.') && !text.contains('e');
    Ok(if whole { text + ".0" } else { text })
}

/// Read a float's text; refuse any text that is not canonical.
pub fn parse_float_text(text: &str) -> Result<f64, TermError> {
    let Ok(x) = text.parse::<f64>() else {
        return err(format!("float text {text:?} is not a number"));
    };
    if float_text(x).ok().as_deref() != Some(text) {
        return err(format!("float text {text:?} is not canonical"));
    }
    Ok(x)
}

/// A term -> its JSON value.
pub fn encode(term: &Term) -> Result<Value, TermError> {
    Ok(match term {
        Term::Int(n) => json!(safe(*n)?),
        Term::Float(x) => json!({"float": float_text(*x)?}),
        Term::Str(s) => json!(s),
        Term::Atom(name) => json!({"a": name}),
        Term::Var(n) => {
            if *n > MAX_SAFE_INTEGER as u64 {
                return err(format!("variable number {n} exceeds 2^53 - 1"));
            }
            json!({"v": n})
        }
        Term::Compound(name, args) => {
            if args.is_empty() {
                return err("a compound needs at least one argument");
            }
            let args: Result<Vec<Value>, TermError> = args.iter().map(encode).collect();
            json!({"f": name, "args": args?})
        }
    })
}

fn keys(obj: &Map<String, Value>, want: &[&str]) -> Result<(), TermError> {
    let mut have: Vec<&str> = obj.keys().map(String::as_str).collect();
    let mut want = want.to_vec();
    have.sort();
    want.sort();
    if have != want {
        return err(format!("term object has keys {have:?}, expected {want:?}"));
    }
    Ok(())
}

/// A JSON value -> the term it encodes.
pub fn decode(value: &Value) -> Result<Term, TermError> {
    match value {
        Value::Number(n) => integer(n),
        Value::String(s) => Ok(Term::Str(s.clone())),
        Value::Object(obj) => tagged(value, obj),
        _ => err(format!("{value} is not a term")),
    }
}

/// A bare number is an integer a double holds exactly.
fn integer(n: &serde_json::Number) -> Result<Term, TermError> {
    match n.as_i64() {
        Some(i) => Ok(Term::Int(safe(i)?)),
        None if n.is_u64() => err(format!("integer {n} exceeds 2^53 - 1")),
        None => err(format!(
            "bare number {n}: floats are written {{\"float\": text}}"
        )),
    }
}

/// An object is a term of the kind its key names.
fn tagged(value: &Value, obj: &Map<String, Value>) -> Result<Term, TermError> {
    let mut tags = ["float", "a", "v", "f"].into_iter();
    match tags.find(|k| obj.contains_key(*k)) {
        Some("float") => float(obj),
        Some("a") => atom(obj),
        Some("v") => variable(obj),
        Some("f") => compound(obj),
        _ => err(format!("{value} is not a term")),
    }
}

fn float(obj: &Map<String, Value>) -> Result<Term, TermError> {
    keys(obj, &["float"])?;
    match &obj["float"] {
        Value::String(text) => Ok(Term::Float(parse_float_text(text)?)),
        _ => err("float text must be a string"),
    }
}

fn atom(obj: &Map<String, Value>) -> Result<Term, TermError> {
    keys(obj, &["a"])?;
    match &obj["a"] {
        Value::String(name) => Ok(Term::Atom(name.clone())),
        _ => err("atom name must be a string"),
    }
}

fn variable(obj: &Map<String, Value>) -> Result<Term, TermError> {
    keys(obj, &["v"])?;
    let n = &obj["v"];
    match n.as_u64() {
        Some(v) if v <= MAX_SAFE_INTEGER as u64 => Ok(Term::Var(v)),
        _ => err(format!(
            "variable number {n} is not a natural number below 2^53"
        )),
    }
}

fn compound(obj: &Map<String, Value>) -> Result<Term, TermError> {
    keys(obj, &["f", "args"])?;
    match (&obj["f"], &obj["args"]) {
        (Value::String(name), Value::Array(args)) if !args.is_empty() => {
            let args: Result<Vec<Term>, TermError> = args.iter().map(decode).collect();
            Ok(Term::Compound(name.clone(), args?))
        }
        _ => err("compound needs a string name and a non-empty args list"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jcs::dumps;
    use crate::jcs::tests::Rng;

    /// Encode, write canonically, read back as a reader of the journal would.
    fn through_journal(t: &Term) -> Term {
        let text = dumps(&encode(t).unwrap()).unwrap();
        decode(&serde_json::from_str(&text).unwrap()).unwrap()
    }

    fn random_term(rng: &mut Rng, depth: u32) -> Term {
        let m = MAX_SAFE_INTEGER;
        match rng.below(if depth < 4 { 7 } else { 6 }) {
            0 => Term::Int(
                [0, 1, -1, m, -m, rng.below(2_000_000) as i64 - 1_000_000][rng.below(6) as usize],
            ),
            1 => loop {
                let x = f64::from_bits(rng.next());
                if x.is_finite() {
                    let pick = [x, rng.below(10) as f64 - 5.0, 0.5, -0.0];
                    break Term::Float(pick[rng.below(4) as usize]);
                }
            },
            2 => Term::Str(
                (0..rng.below(4))
                    .filter_map(|_| char::from_u32(rng.below(0x3000) as u32))
                    .collect(),
            ),
            3 => Term::Atom(["", "a", "[]", "'q'", "é", "foo bar"][rng.below(6) as usize].into()),
            4 => Term::Var(rng.below(100)),
            5 => Term::Atom("x".into()),
            _ => Term::Compound(
                ["f", ".", "supports", "€"][rng.below(4) as usize].into(),
                (0..1 + rng.below(3))
                    .map(|_| random_term(rng, depth + 1))
                    .collect(),
            ),
        }
    }

    #[test]
    fn canonical_texts() {
        // Catches a float written as an integer, or in a non-canonical form.
        let cases = [
            (1.0, "1.0"),
            (-1.0, "-1.0"),
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (0.5, "0.5"),
            (1e23, "1e+23"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000.0"),
            (1e-7, "1e-7"),
            (5e-324, "5e-324"),
            (9007199254740992.0, "9007199254740992.0"),
        ];
        for (x, want) in cases {
            assert_eq!(float_text(x).unwrap(), want, "{x}");
        }
    }

    #[test]
    fn non_finite_refused() {
        // Catches NaN or infinity entering the journal.
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(encode(&Term::Float(bad)).is_err());
        }
    }

    #[test]
    fn float_stays_float() {
        // Catches 1.0 coming back as the integer 1 (the reason floats are tagged).
        assert_eq!(through_journal(&Term::Float(1.0)), Term::Float(1.0));
        assert_eq!(through_journal(&Term::Int(1)), Term::Int(1));
        assert_ne!(Term::Float(1.0), Term::Int(1));
        assert_eq!(
            dumps(&encode(&Term::Float(1.0)).unwrap()).unwrap(),
            r#"{"float":"1.0"}"#
        );
    }

    #[test]
    fn negative_zero_keeps_sign() {
        // Catches -0.0 coming back as 0.0.
        assert_eq!(through_journal(&Term::Float(-0.0)), Term::Float(-0.0));
        assert_ne!(Term::Float(-0.0), Term::Float(0.0));
    }

    #[test]
    fn string_is_not_atom() {
        // Catches a Prolog string and an atom with the same text being confused.
        let s = Term::Str("abc".into());
        let a = Term::Atom("abc".into());
        assert_eq!(through_journal(&s), s);
        assert_eq!(through_journal(&a), a);
        assert_ne!(encode(&s).unwrap(), encode(&a).unwrap());
    }

    #[test]
    fn generated_terms() {
        // Catches any term that does not come back as itself, over 10,000 terms.
        let mut rng = Rng(2);
        for i in 0..10_000 {
            let t = random_term(&mut rng, 0);
            assert_eq!(through_journal(&t), t, "term {i}");
        }
    }

    #[test]
    fn encoding_is_deterministic() {
        // Catches two encodings of one term, which would give two hashes.
        let mut rng = Rng(5);
        for _ in 0..1000 {
            let t = random_term(&mut rng, 0);
            let a = dumps(&encode(&t).unwrap()).unwrap();
            let b = dumps(&encode(&through_journal(&t)).unwrap()).unwrap();
            assert_eq!(a, b);
        }
    }

    #[test]
    fn decode_refuses() {
        // Catches malformed or non-canonical JSON accepted as a term.
        let bad = [
            "null",
            "true",
            "1.5",
            "9007199254740992",
            "18446744073709551615",
            "1000000000000000000000000000000",
            "[]",
            "{}",
            r#"{"x":1}"#,
            r#"{"float":1.0}"#,
            r#"{"float":"1"}"#,
            r#"{"float":"1.00"}"#,
            r#"{"float":"1e23"}"#,
            r#"{"float":"+1.0"}"#,
            r#"{"float":"nan"}"#,
            r#"{"float":"NaN"}"#,
            r#"{"float":"inf"}"#,
            r#"{"float":""}"#,
            r#"{"v":9007199254740992}"#,
            r#"{"a":1}"#,
            r#"{"a":"x","extra":1}"#,
            r#"{"v":-1}"#,
            r#"{"v":"1"}"#,
            r#"{"v":true}"#,
            r#"{"v":1.5}"#,
            r#"{"f":"g"}"#,
            r#"{"f":"g","args":[]}"#,
            r#"{"f":1,"args":[1]}"#,
            r#"{"f":"g","args":[1],"a":"x"}"#,
            r#"{"f":"g","args":[1.5]}"#,
        ];
        for b in bad {
            let v: Value = serde_json::from_str(b).unwrap();
            assert!(decode(&v).is_err(), "accepted {b}");
        }
    }

    #[test]
    fn encode_refuses() {
        // Catches values with no term form being written.
        let m = MAX_SAFE_INTEGER;
        let bad = [
            Term::Int(m + 1),
            Term::Int(-m - 1),
            Term::Int(i64::MIN),
            Term::Var(m as u64 + 1),
            Term::Compound("f".into(), vec![]),
        ];
        for b in bad {
            assert!(encode(&b).is_err(), "{b:?}");
        }
    }
}
