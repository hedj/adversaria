"""Tests for tools/jcs.py, the RFC 8785 serialiser that journal hashes rest on.

Fault caught by each test is in its docstring. Node.js is the oracle for
numbers: RFC 8785 defines number serialisation as ECMAScript's
Number.prototype.toString, which Node implements.
"""

import json
import os
import random
import struct
import subprocess
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "..", "tools"))
import jcs  # noqa: E402

# RFC 8785 Appendix B: IEEE 754 bit pattern -> canonical text. All 24 finite
# rows, checked against the RFC's text (rfc-editor.org, 12 November 2026);
# test_vectors_agree_with_node also checks each against Node.
VECTORS = [
    ("0000000000000000", "0"),
    ("8000000000000000", "0"),
    ("0000000000000001", "5e-324"),
    ("8000000000000001", "-5e-324"),
    ("7fefffffffffffff", "1.7976931348623157e+308"),
    ("ffefffffffffffff", "-1.7976931348623157e+308"),
    ("4340000000000000", "9007199254740992"),
    ("c340000000000000", "-9007199254740992"),
    ("4430000000000000", "295147905179352830000"),
    ("44b52d02c7e14af5", "9.999999999999997e+22"),
    ("44b52d02c7e14af6", "1e+23"),
    ("44b52d02c7e14af7", "1.0000000000000001e+23"),
    ("444b1ae4d6e2ef4e", "999999999999999700000"),
    ("444b1ae4d6e2ef4f", "999999999999999900000"),
    ("444b1ae4d6e2ef50", "1e+21"),
    ("3eb0c6f7a0b5ed8c", "9.999999999999997e-7"),
    ("3eb0c6f7a0b5ed8d", "0.000001"),
    ("41b3de4355555553", "333333333.3333332"),
    ("41b3de4355555554", "333333333.33333325"),
    ("41b3de4355555555", "333333333.3333333"),
    ("41b3de4355555556", "333333333.3333334"),
    ("41b3de4355555557", "333333333.33333343"),
    ("becbf647612f3696", "-0.0000033333333333333333"),
    ("43143ff3c1cb0959", "1424953923781206.2"),
]

NODE_NUMBERS = r"""
const lines = require('fs').readFileSync(0, 'utf8').trim().split('\n');
const out = lines.map(h => String(Buffer.from(h, 'hex').readDoubleBE(0)));
process.stdout.write(out.join('\n'));
"""


def from_hex(h):
    return struct.unpack(">d", bytes.fromhex(h))[0]


def node_numbers(hexes):
    r = subprocess.run(["node", "-e", NODE_NUMBERS], input="\n".join(hexes),
                       capture_output=True, text=True, check=True)
    return r.stdout.split("\n")


class Numbers(unittest.TestCase):
    def test_rfc_vectors(self):
        """Catches a wrong rule for exponents, small or large magnitudes, or -0."""
        for h, want in VECTORS:
            with self.subTest(h=h):
                self.assertEqual(jcs.number(from_hex(h)), want)

    def test_vectors_agree_with_node(self):
        """Catches a wrongly recalled vector above (the oracle checks the table)."""
        got = node_numbers([h for h, _ in VECTORS])
        self.assertEqual(got, [want for _, want in VECTORS])

    def test_random_doubles_agree_with_node(self):
        """Catches any difference from ECMAScript over 10,000 random bit patterns."""
        rng = random.Random(8785)
        hexes = []
        while len(hexes) < 10_000:
            h = f"{rng.getrandbits(64):016x}"
            if jcs.math.isfinite(from_hex(h)):
                hexes.append(h)
        want = node_numbers(hexes)
        got = [jcs.number(from_hex(h)) for h in hexes]
        mismatches = [(h, g, w) for h, g, w in zip(hexes, got, want) if g != w]
        self.assertEqual(mismatches[:5], [], f"{len(mismatches)} mismatches")

    def test_integers(self):
        """Catches integers written with a fraction or exponent, or unsafe ones accepted."""
        self.assertEqual(jcs.number(0), "0")
        self.assertEqual(jcs.number(-17), "-17")
        self.assertEqual(jcs.number(2**53 - 1), "9007199254740991")
        self.assertEqual(jcs.number(-(2**53 - 1)), "-9007199254740991")
        for bad in (2**53, -(2**53), 10**30):
            with self.subTest(bad=bad), self.assertRaises(jcs.CanonicalError):
                jcs.number(bad)

    def test_non_finite_refused(self):
        """Catches NaN or infinity written as invalid JSON."""
        for bad in (float("nan"), float("inf"), float("-inf")):
            with self.subTest(bad=bad), self.assertRaises(jcs.CanonicalError):
                jcs.number(bad)

    def test_integral_float_equals_integer(self):
        """Records a property of RFC 8785: 1.0 and 1 serialise alike.

        A Prolog float 1.0 and integer 1 are different terms, so the journal
        cannot tell them apart after a round trip. This is reported to the
        author; the test fails if the property ever changes.
        """
        self.assertEqual(jcs.dumps(1.0), jcs.dumps(1))
        self.assertEqual(jcs.dumps(1.0), "1")


class Strings(unittest.TestCase):
    def test_escapes(self):
        """Catches over-escaping (e.g. of / or non-ASCII) or missed control characters."""
        cases = {
            "": '""',
            'a"b\\c': '"a\\"b\\\\c"',
            "\b\f\n\r\t": '"\\b\\f\\n\\r\\t"',
            "\x00\x1f": '"\\u0000\\u001f"',
            "\x7f/é€😀": '"\x7f/é€😀"',
        }
        for s, want in cases.items():
            with self.subTest(s=s):
                self.assertEqual(jcs.string(s), want)

    def test_strings_agree_with_json_parse(self):
        """Catches output that a standard JSON reader decodes to another string."""
        s = "".join(chr(c) for c in range(0, 0x250)) + "😀  "
        self.assertEqual(json.loads(jcs.string(s)), s)


class Structure(unittest.TestCase):
    def test_key_order_is_utf16(self):
        """Catches sorting keys by code point instead of UTF-16 code unit (RFC 8785 § 3.2.3)."""
        keys = ["€", "\r", "דּ", "1", "\U0001f600", "\u0080", "ö"]
        obj = {k: i for i, k in enumerate(keys)}
        got = [k for k in json.loads(jcs.dumps(obj))]
        self.assertEqual(got, ["\r", "1", "\u0080", "ö", "€",
                               "\U0001f600", "דּ"])

    def test_key_order_agrees_with_node(self):
        """Catches a difference from ECMAScript's default sort, which RFC 8785 cites."""
        rng = random.Random(3)
        keys = list({"".join(chr(rng.choice([rng.randrange(0x20, 0xd800),
                                              rng.randrange(0xe000, 0x110000)]))
                             for _ in range(rng.randrange(1, 4)))
                     for _ in range(500)})
        r = subprocess.run(["node", "-e",
                            "const k=JSON.parse(require('fs').readFileSync(0,'utf8'));"
                            "process.stdout.write(JSON.stringify(k.sort()))"],
                           input=json.dumps(keys), capture_output=True, text=True,
                           check=True)
        want = json.loads(r.stdout)
        got = list(json.loads(jcs.dumps({k: 0 for k in keys})))
        self.assertEqual(got, want)

    def test_nesting_and_empties(self):
        """Catches whitespace, wrong literals, or nested objects left unsorted."""
        value = {"b": [], "a": {"z": None, "y": [True, False, {}]}, "": ""}
        self.assertEqual(jcs.dumps(value),
                         '{"":"","a":{"y":[true,false,{}],"z":null},"b":[]}')

    def test_refusals(self):
        """Catches non-JSON values or non-string keys passed through silently."""
        for bad in ({1: "x"}, {"a": {1, 2}}, b"bytes", object()):
            with self.subTest(bad=bad), self.assertRaises(jcs.CanonicalError):
                jcs.dumps(bad)

    def test_canonical_bytes_are_utf8(self):
        """Catches hashing a text in some encoding other than UTF-8."""
        self.assertEqual(jcs.canonical_bytes({"é": 1}), '{"é":1}'.encode("utf-8"))


if __name__ == "__main__":
    unittest.main()
