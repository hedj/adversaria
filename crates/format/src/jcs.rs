//! JSON Canonicalization Scheme (RFC 8785), for hashing journal records.
//!
//! Limits, by design of RFC 8785 itself:
//! * Numbers are IEEE 754 doubles. An integer beyond 2^53 - 1 in magnitude
//!   cannot be held exactly, so it is refused, not rounded.
//! * NaN and the infinities have no JSON form, so they are refused.

use serde_json::Value;
use std::fmt;

pub const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;

/// A value that has no canonical JSON form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalError(pub String);

impl fmt::Display for CanonicalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CanonicalError {}

fn err<T>(msg: String) -> Result<T, CanonicalError> {
    Err(CanonicalError(msg))
}

/// Serialise an integer; refuse one a double cannot hold exactly.
pub fn integer(n: i128) -> Result<String, CanonicalError> {
    if n.unsigned_abs() > MAX_SAFE_INTEGER as u128 {
        return err(format!("integer {n} exceeds 2^53 - 1"));
    }
    Ok(n.to_string())
}

/// Serialise a double as ECMAScript's Number.prototype.toString does.
pub fn number(x: f64) -> Result<String, CanonicalError> {
    if !x.is_finite() {
        return err(format!("{x} has no JSON form"));
    }
    if x == 0.0 {
        return Ok("0".into()); // also for -0.0
    }
    if x < 0.0 {
        return Ok(format!("-{}", number(-x)?));
    }
    // Rust's {:e} gives the shortest digits that read back to x, as d.ddde±n.
    // x = digits × 10^(n - k), with no trailing zeros in digits.
    let sci = format!("{x:e}");
    let (mantissa, exp) = sci.split_once('e').expect("{:e} always writes an exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let mut digits = digits.trim_end_matches('0').to_string();
    let mut n = exp.parse::<i32>().expect("exponent is an integer") + 1;
    if let Some((even, even_n)) = even_on_tie(x, &digits) {
        (digits, n) = (even, even_n);
    }
    let digits = digits.as_str();
    let k = digits.len() as i32;
    Ok(if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat(-n as usize))
    } else {
        let e = n - 1;
        let sign = if e >= 0 { '+' } else { '-' };
        let head = if k > 1 {
            format!("{}.{}", &digits[..1], &digits[1..])
        } else {
            digits.to_string()
        };
        format!("{head}e{sign}{}", e.abs())
    })
}

/// ECMAScript breaks a tie between two shortest digit strings, equally near
/// x, by taking the even one (Number::toString, step 5); Rust's shortest
/// formatting does not. A tie means x's exact decimal value has one digit
/// more than the shortest form, and that digit is 5. Returns the even
/// candidate and its decimal-point position, if it differs and reads back to x.
fn even_on_tie(x: f64, shortest: &str) -> Option<(String, i32)> {
    // 800 places exceeds the longest exact expansion of a double, so this
    // text is exact, with no rounding.
    let exact = format!("{x:.800e}");
    let (mantissa, exp) = exact.split_once('e')?;
    let exact_digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let exact_digits = exact_digits.trim_end_matches('0');
    let k = shortest.len();
    if exact_digits.len() != k + 1 || !exact_digits.ends_with('5') {
        return None;
    }
    let n = exp.parse::<i32>().ok()? + 1;
    let lo = &exact_digits[..k];
    let (hi, hi_n) = increment(lo, n);
    let lo_even = (lo.as_bytes()[k - 1] - b'0').is_multiple_of(2);
    let (even, even_n) = if lo_even {
        (lo.to_string(), n)
    } else {
        (hi, hi_n)
    };
    let even = even.trim_end_matches('0').to_string();
    let reads_back = format!("0.{even}e{even_n}").parse::<f64>().ok()? == x;
    (even != shortest && reads_back).then_some((even, even_n))
}

/// Add one in the last place of a digit string; a carry out of the first
/// digit moves the decimal point one place right.
fn increment(digits: &str, n: i32) -> (String, i32) {
    let mut d: Vec<u8> = digits.bytes().collect();
    for i in (0..d.len()).rev() {
        if d[i] == b'9' {
            d[i] = b'0';
        } else {
            d[i] += 1;
            return (String::from_utf8(d).unwrap(), n);
        }
    }
    d.insert(0, b'1');
    (String::from_utf8(d).unwrap(), n + 1)
}

/// Serialise a string: escape only what JSON requires; keep the rest.
pub fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Return the canonical JSON text of a value.
pub fn dumps(value: &Value) -> Result<String, CanonicalError> {
    let mut out = String::new();
    write(value, &mut out)?;
    Ok(out)
}

/// Canonical JSON as UTF-8 bytes, ready to hash.
pub fn canonical_bytes(value: &Value) -> Result<Vec<u8>, CanonicalError> {
    dumps(value).map(String::into_bytes)
}

fn write(value: &Value, out: &mut String) -> Result<(), CanonicalError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(num) => {
            let text = if let Some(i) = num.as_i64() {
                integer(i.into())?
            } else if let Some(u) = num.as_u64() {
                integer(u.into())?
            } else {
                number(num.as_f64().expect("a JSON number is i64, u64 or f64"))?
            };
            out.push_str(&text);
        }
        Value::String(s) => out.push_str(&string(s)),
        Value::Array(items) => {
            out.push('[');
            for (i, v) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(v, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            // RFC 8785 § 3.2.3: sort keys by UTF-16 code units.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&string(k));
                out.push(':');
                write(&map[k.as_str()], out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use std::process::{Command, Stdio};

    /// RFC 8785 Appendix B: IEEE 754 bit pattern -> canonical text. All 24
    /// finite rows, checked against the RFC's text (rfc-editor.org,
    /// 12 November 2026); vectors_agree_with_node also checks each against Node.
    const VECTORS: [(u64, &str); 24] = [
        (0x0000000000000000, "0"),
        (0x8000000000000000, "0"),
        (0x0000000000000001, "5e-324"),
        (0x8000000000000001, "-5e-324"),
        (0x7fefffffffffffff, "1.7976931348623157e+308"),
        (0xffefffffffffffff, "-1.7976931348623157e+308"),
        (0x4340000000000000, "9007199254740992"),
        (0xc340000000000000, "-9007199254740992"),
        (0x4430000000000000, "295147905179352830000"),
        (0x44b52d02c7e14af5, "9.999999999999997e+22"),
        (0x44b52d02c7e14af6, "1e+23"),
        (0x44b52d02c7e14af7, "1.0000000000000001e+23"),
        (0x444b1ae4d6e2ef4e, "999999999999999700000"),
        (0x444b1ae4d6e2ef4f, "999999999999999900000"),
        (0x444b1ae4d6e2ef50, "1e+21"),
        (0x3eb0c6f7a0b5ed8c, "9.999999999999997e-7"),
        (0x3eb0c6f7a0b5ed8d, "0.000001"),
        (0x41b3de4355555553, "333333333.3333332"),
        (0x41b3de4355555554, "333333333.33333325"),
        (0x41b3de4355555555, "333333333.3333333"),
        (0x41b3de4355555556, "333333333.3333334"),
        (0x41b3de4355555557, "333333333.33333343"),
        (0xbecbf647612f3696, "-0.0000033333333333333333"),
        (0x43143ff3c1cb0959, "1424953923781206.2"),
    ];

    /// SplitMix64: a seeded generator, so random tests repeat exactly.
    pub(crate) struct Rng(pub u64);
    impl Rng {
        pub(crate) fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^ (z >> 31)
        }
        pub(crate) fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// Run a Node.js script with input on stdin. Node is the oracle: RFC 8785
    /// defines numbers as ECMAScript's Number.prototype.toString. A missing
    /// Node fails the test; it never skips it.
    fn node(script: &str, input: &str) -> String {
        let mut child = Command::new("node")
            .args(["-e", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("node must be installed: it is the tests' oracle");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "node failed");
        String::from_utf8(out.stdout).unwrap()
    }

    fn node_numbers(bits: &[u64]) -> Vec<String> {
        let input: Vec<String> = bits.iter().map(|b| format!("{b:016x}")).collect();
        let script = "const l=require('fs').readFileSync(0,'utf8').trim().split('\\n');\
            process.stdout.write(l.map(h=>String(Buffer.from(h,'hex').readDoubleBE(0))).join('\\n'))";
        node(script, &input.join("\n"))
            .split('\n')
            .map(String::from)
            .collect()
    }

    #[test]
    fn rfc_vectors() {
        // Catches a wrong rule for exponents, small or large magnitudes, or -0.
        for (bits, want) in VECTORS {
            assert_eq!(number(f64::from_bits(bits)).unwrap(), want, "{bits:016x}");
        }
    }

    #[test]
    fn vectors_agree_with_node() {
        // Catches a wrongly copied vector in the table above.
        let bits: Vec<u64> = VECTORS.iter().map(|v| v.0).collect();
        let want: Vec<&str> = VECTORS.iter().map(|v| v.1).collect();
        assert_eq!(node_numbers(&bits), want);
    }

    #[test]
    fn random_doubles_agree_with_node() {
        // Catches any difference from ECMAScript over random bit patterns:
        // 10,000 by default, or JCS_SAMPLES for a longer run.
        let samples: usize = std::env::var("JCS_SAMPLES").map_or(10_000, |v| v.parse().unwrap());
        let mut rng = Rng(8785);
        let mut bits = Vec::new();
        while bits.len() < samples {
            let b = rng.next();
            if f64::from_bits(b).is_finite() {
                bits.push(b);
            }
        }
        let want = node_numbers(&bits);
        let mismatches: Vec<_> = bits
            .iter()
            .zip(&want)
            .filter(|(b, w)| number(f64::from_bits(**b)).unwrap() != **w)
            .take(5)
            .collect();
        assert!(mismatches.is_empty(), "{mismatches:?}");
    }

    #[test]
    fn ties_go_to_even() {
        // Catches Rust's own tie-breaking leaking through. Exact values
        // 1424953923781206.25 and 163973701539428.625: Rust writes ...206.3
        // and ...428.63; ECMAScript takes the even digit.
        assert_eq!(
            number(f64::from_bits(0x43143ff3c1cb0959)).unwrap(),
            "1424953923781206.2"
        );
        assert_eq!(
            number(f64::from_bits(4819595160108321940)).unwrap(),
            "163973701539428.62"
        );
        assert_eq!(
            node_numbers(&[0x43143ff3c1cb0959, 4819595160108321940]),
            ["1424953923781206.2", "163973701539428.62"]
        );
    }

    #[test]
    fn increment_carries() {
        // Catches a carry that is dropped or that does not move the point.
        assert_eq!(increment("12", 3), ("13".into(), 3));
        assert_eq!(increment("19", 3), ("20".into(), 3));
        assert_eq!(increment("99", 3), ("100".into(), 4));
    }

    #[test]
    fn integers() {
        // Catches integers written with a fraction or exponent, or unsafe ones accepted.
        let m = MAX_SAFE_INTEGER as i128;
        assert_eq!(integer(0).unwrap(), "0");
        assert_eq!(integer(-17).unwrap(), "-17");
        assert_eq!(integer(m).unwrap(), "9007199254740991");
        assert_eq!(integer(-m).unwrap(), "-9007199254740991");
        for bad in [m + 1, -m - 1, 10i128.pow(30)] {
            assert!(integer(bad).is_err(), "{bad}");
        }
        assert!(dumps(&json!(u64::MAX)).is_err());
        assert!(dumps(&json!(i64::MIN)).is_err());
    }

    #[test]
    fn non_finite_refused() {
        // Catches NaN or infinity written as invalid JSON.
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(number(bad).is_err());
        }
    }

    #[test]
    fn integral_float_equals_integer() {
        // Records a property of RFC 8785: 1.0 and 1 serialise alike. A Prolog
        // float and integer differ, so terms tag floats (terms.rs). Fails if
        // the property changes.
        assert_eq!(dumps(&json!(1.0)).unwrap(), dumps(&json!(1)).unwrap());
        assert_eq!(dumps(&json!(1.0)).unwrap(), "1");
    }

    #[test]
    fn escapes() {
        // Catches over-escaping (of / or non-ASCII) or missed control characters.
        let cases = [
            ("", r#""""#),
            ("a\"b\\c", r#""a\"b\\c""#),
            ("\u{8}\u{c}\n\r\t", r#""\b\f\n\r\t""#),
            ("\u{0}\u{1f}", r#""\u0000\u001f""#),
            ("\u{7f}/é€😀", "\"\u{7f}/é€😀\""),
        ];
        for (s, want) in cases {
            assert_eq!(string(s), want, "{s:?}");
        }
    }

    #[test]
    fn strings_agree_with_json_parse() {
        // Catches output that a standard JSON reader decodes to another string.
        let s: String = (0u32..0x250)
            .filter_map(char::from_u32)
            .chain("😀\u{2028}\u{2029}".chars())
            .collect();
        let back: String = serde_json::from_str(&string(&s)).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn key_order_is_utf16() {
        // Catches sorting keys by code point instead of UTF-16 code unit.
        let keys = [
            "\u{20ac}",
            "\r",
            "\u{fb33}",
            "1",
            "\u{1f600}",
            "\u{80}",
            "\u{f6}",
        ];
        let obj: serde_json::Map<String, Value> =
            keys.iter().map(|k| (k.to_string(), json!(0))).collect();
        let text = dumps(&Value::Object(obj)).unwrap();
        let want = [
            "\r",
            "1",
            "\u{80}",
            "\u{f6}",
            "\u{20ac}",
            "\u{1f600}",
            "\u{fb33}",
        ];
        let want_text: Vec<String> = want.iter().map(|k| format!("{}:0", string(k))).collect();
        assert_eq!(text, format!("{{{}}}", want_text.join(",")));
    }

    #[test]
    fn key_order_agrees_with_node() {
        // Catches a difference from ECMAScript's default sort, which RFC 8785 cites.
        let mut rng = Rng(3);
        let mut keys = std::collections::BTreeSet::new();
        while keys.len() < 500 {
            let len = 1 + rng.below(3);
            let k: String = (0..len)
                .filter_map(|_| {
                    let c = if rng.below(2) == 0 {
                        0x20 + rng.below(0xd800 - 0x20)
                    } else {
                        0xe000 + rng.below(0x110000 - 0xe000)
                    };
                    char::from_u32(c as u32)
                })
                .collect();
            keys.insert(k);
        }
        let list: Vec<&String> = keys.iter().collect();
        let sorted = node(
            "const k=JSON.parse(require('fs').readFileSync(0,'utf8'));\
             process.stdout.write(JSON.stringify(k.sort()))",
            &serde_json::to_string(&list).unwrap(),
        );
        let want: Vec<String> = serde_json::from_str(&sorted).unwrap();
        let obj: serde_json::Map<String, Value> =
            keys.iter().map(|k| (k.clone(), json!(0))).collect();
        let got: Vec<String> = {
            // Read keys back in written order, without re-sorting.
            let text = dumps(&Value::Object(obj)).unwrap();
            let mut de = serde_json::Deserializer::from_str(&text);
            let ordered: OrderedKeys = serde::de::Deserialize::deserialize(&mut de).unwrap();
            ordered.0
        };
        assert_eq!(got, want);
    }

    /// Keys of a JSON object in the order they appear in the text.
    struct OrderedKeys(Vec<String>);
    impl<'de> serde::de::Deserialize<'de> for OrderedKeys {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;
            impl<'de> serde::de::Visitor<'de> for V {
                type Value = OrderedKeys;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("an object")
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut m: A,
                ) -> Result<OrderedKeys, A::Error> {
                    let mut keys = Vec::new();
                    while let Some((k, _)) = m.next_entry::<String, serde::de::IgnoredAny>()? {
                        keys.push(k);
                    }
                    Ok(OrderedKeys(keys))
                }
            }
            d.deserialize_map(V)
        }
    }

    #[test]
    fn nesting_and_empties() {
        // Catches whitespace, wrong literals, or nested objects left unsorted.
        let v = json!({"b": [], "a": {"z": null, "y": [true, false, {}]}, "": ""});
        assert_eq!(
            dumps(&v).unwrap(),
            r#"{"":"","a":{"y":[true,false,{}],"z":null},"b":[]}"#
        );
    }

    #[test]
    fn canonical_bytes_are_utf8() {
        // Catches hashing a text in some encoding other than UTF-8.
        assert_eq!(
            canonical_bytes(&json!({"é": 1})).unwrap(),
            "{\"é\":1}".as_bytes()
        );
    }
}
