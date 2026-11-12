"""JSON Canonicalization Scheme (RFC 8785), for hashing journal records.

The journal format (docs/design.md § The journal format) hashes each record
serialised by RFC 8785. This module is that serialiser, with no dependencies,
so the reference checker and the sealer can share it.

Limits, by design of RFC 8785 itself:
* Numbers are IEEE 754 doubles. An integer beyond 2**53 - 1 in magnitude
  cannot be held exactly, so it is refused, not rounded.
* NaN and the infinities have no JSON form, so they are refused.
"""

import decimal
import math

MAX_SAFE_INTEGER = 2**53 - 1

_ESCAPES = {'"': '\\"', '\\': '\\\\', '\b': '\\b', '\f': '\\f',
            '\n': '\\n', '\r': '\\r', '\t': '\\t'}


class CanonicalError(ValueError):
    """A value that has no canonical JSON form."""


def number(x):
    """Serialise a number as ECMAScript's Number.prototype.toString does."""
    if isinstance(x, bool):
        raise CanonicalError("a bool is not a number")
    if isinstance(x, int):
        if abs(x) > MAX_SAFE_INTEGER:
            raise CanonicalError(f"integer {x} exceeds 2**53 - 1")
        return str(x)
    if not math.isfinite(x):
        raise CanonicalError(f"{x} has no JSON form")
    if x == 0:
        return "0"                      # also for -0.0
    if x < 0:
        return "-" + number(-x)
    # repr gives the shortest digits that read back to x, nearest to x.
    # x = digits × 10**(n - k), with no trailing zeros in digits.
    _, digit_tuple, exponent = decimal.Decimal(repr(x)).normalize().as_tuple()
    digits = "".join(map(str, digit_tuple))
    k = len(digits)
    n = k + exponent
    if k <= n <= 21:
        return digits + "0" * (n - k)
    if 0 < n <= 21:
        return digits[:n] + "." + digits[n:]
    if -6 < n <= 0:
        return "0." + "0" * (-n) + digits
    e = n - 1
    sign = "+" if e >= 0 else "-"
    head = digits[0] + ("." + digits[1:] if k > 1 else "")
    return f"{head}e{sign}{abs(e)}"


def string(s):
    """Serialise a string: escape only what JSON requires; keep the rest."""
    out = ['"']
    for ch in s:
        if ch in _ESCAPES:
            out.append(_ESCAPES[ch])
        elif ch < " ":
            out.append(f"\\u{ord(ch):04x}")
        else:
            out.append(ch)
    out.append('"')
    return "".join(out)


def _utf16_key(s):
    return s.encode("utf-16-be")


def dumps(value):
    """Return the canonical JSON text of value, as a str."""
    if value is None:
        return "null"
    if value is True:
        return "true"
    if value is False:
        return "false"
    if isinstance(value, (int, float)):
        return number(value)
    if isinstance(value, str):
        return string(value)
    if isinstance(value, (list, tuple)):
        return "[" + ",".join(dumps(v) for v in value) + "]"
    if isinstance(value, dict):
        for k in value:
            if not isinstance(k, str):
                raise CanonicalError(f"object key {k!r} is not a string")
        keys = sorted(value, key=_utf16_key)   # RFC 8785 § 3.2.3: UTF-16 code units
        return "{" + ",".join(string(k) + ":" + dumps(value[k]) for k in keys) + "}"
    raise CanonicalError(f"{type(value).__name__} has no JSON form")


def canonical_bytes(value):
    """Canonical JSON as UTF-8 bytes, ready to hash."""
    return dumps(value).encode("utf-8")
