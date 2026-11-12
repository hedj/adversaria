"""Tests for tools/terms.py, the journal's JSON encoding of Prolog terms.

Fault caught by each test is in its docstring.
"""

import json
import os
import random
import struct
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "..", "tools"))
import jcs  # noqa: E402
from terms import (Atom, Compound, TermError, Var, decode, encode,  # noqa: E402
                   float_text)


def same(a, b):
    """Equal and of the same type throughout; floats compared by bit pattern.
    (Python's == says 1 == 1.0 and 0.0 == -0.0, which would hide the faults.)"""
    if type(a) is not type(b):
        return False
    if isinstance(a, float):
        return struct.pack(">d", a) == struct.pack(">d", b)
    if isinstance(a, Compound):
        return a.name == b.name and len(a.args) == len(b.args) and \
            all(same(x, y) for x, y in zip(a.args, b.args))
    return a == b


def through_journal(term):
    """Encode, write canonically, read back as a reader of the journal would."""
    return decode(json.loads(jcs.dumps(encode(term))))


def random_term(rng, depth=0):
    kind = rng.randrange(7 if depth < 4 else 6)
    if kind == 0:
        return rng.choice([0, 1, -1, jcs.MAX_SAFE_INTEGER, -jcs.MAX_SAFE_INTEGER,
                           rng.randrange(-10**6, 10**6)])
    if kind == 1:
        while True:
            x = struct.unpack(">d", struct.pack(">Q", rng.getrandbits(64)))[0]
            if x == x and abs(x) != float("inf"):
                return rng.choice([x, float(rng.randrange(-5, 5)), 0.5, -0.0])
    if kind == 2:
        return "".join(chr(rng.randrange(0, 0x3000)) for _ in range(rng.randrange(4)))
    if kind == 3:
        return Atom(rng.choice(["", "a", "[]", "'q'", "é", "foo bar"]))
    if kind == 4:
        return Var(rng.randrange(100))
    if kind == 5:
        return Atom("x")
    return Compound(rng.choice(["f", ".", "supports", "€"]),
                    tuple(random_term(rng, depth + 1) for _ in range(rng.randrange(1, 4))))


class FloatText(unittest.TestCase):
    def test_canonical_texts(self):
        """Catches a float written as an integer, or in a non-canonical form."""
        cases = [(1.0, "1.0"), (-1.0, "-1.0"), (0.0, "0.0"), (-0.0, "-0.0"),
                 (0.5, "0.5"), (1e23, "1e+23"), (1e21, "1e+21"),
                 (1e20, "100000000000000000000.0"), (1e-7, "1e-7"),
                 (5e-324, "5e-324"), (2.0**53, "9007199254740992.0")]
        for x, want in cases:
            with self.subTest(x=x):
                self.assertEqual(float_text(x), want)

    def test_non_finite_refused(self):
        """Catches NaN or infinity entering the journal."""
        for bad in (float("nan"), float("inf"), float("-inf")):
            with self.subTest(bad=bad), self.assertRaises(TermError):
                encode(bad)


class RoundTrip(unittest.TestCase):
    def test_float_stays_float(self):
        """Catches 1.0 coming back as the integer 1 (the reason floats are tagged)."""
        self.assertTrue(same(through_journal(1.0), 1.0))
        self.assertTrue(same(through_journal(1), 1))
        self.assertNotEqual(jcs.dumps(encode(1.0)), jcs.dumps(encode(1)))

    def test_negative_zero_keeps_sign(self):
        """Catches -0.0 coming back as 0.0."""
        self.assertTrue(same(through_journal(-0.0), -0.0))

    def test_string_is_not_atom(self):
        """Catches a Prolog string and an atom with the same text being confused."""
        self.assertTrue(same(through_journal("abc"), "abc"))
        self.assertTrue(same(through_journal(Atom("abc")), Atom("abc")))
        self.assertNotEqual(jcs.dumps(encode("abc")), jcs.dumps(encode(Atom("abc"))))

    def test_generated_terms(self):
        """Catches any term that does not come back as itself, over 10,000 terms."""
        rng = random.Random(2)
        for i in range(10_000):
            t = random_term(rng)
            with self.subTest(i=i):
                self.assertTrue(same(through_journal(t), t), repr(t))

    def test_encoding_is_deterministic(self):
        """Catches two encodings of one term, which would give two hashes."""
        rng = random.Random(5)
        for _ in range(1000):
            t = random_term(rng)
            self.assertEqual(jcs.dumps(encode(t)),
                             jcs.dumps(encode(through_journal(t))))


class Malformed(unittest.TestCase):
    def test_decode_refuses(self):
        """Catches malformed or non-canonical JSON accepted as a term."""
        bad = [None, True, 1.5, 2**53, [], {}, {"x": 1},
               {"float": 1.0}, {"float": "1"}, {"float": "1.00"}, {"float": "1e23"},
               {"float": "+1.0"}, {"float": "nan"}, {"float": "inf"}, {"float": ""},
               {"a": 1}, {"a": "x", "extra": 1},
               {"v": -1}, {"v": "1"}, {"v": True},
               {"f": "g"}, {"f": "g", "args": []}, {"f": 1, "args": [1]},
               {"f": "g", "args": [1], "a": "x"}, {"f": "g", "args": [1.5]}]
        for b in bad:
            with self.subTest(b=b), self.assertRaises(TermError):
                decode(b)

    def test_encode_refuses(self):
        """Catches values with no term form being written."""
        for b in (True, None, 2**53, -(2**53), Var(-1), Compound("f", ()), [1], b"x"):
            with self.subTest(b=b), self.assertRaises(TermError):
                encode(b)


if __name__ == "__main__":
    unittest.main()
