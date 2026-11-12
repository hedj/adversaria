"""Prolog terms as JSON, as docs/design.md § The journal format defines them.

| Prolog       | JSON                                   |
| integer      | a JSON integer, at most 2**53 - 1 in magnitude |
| float        | {"float": text}, text canonical (float_text) |
| string       | a JSON string                          |
| atom         | {"a": name}                            |
| variable     | {"v": n}                               |
| compound     | {"f": name, "args": [...]}             |

Floats are tagged because RFC 8785 writes 1.0 and 1 alike, and Prolog
tells them apart.
"""

import math
from dataclasses import dataclass

from jcs import MAX_SAFE_INTEGER, number


class TermError(ValueError):
    """A value that is not a term in the journal's encoding."""


@dataclass(frozen=True)
class Atom:
    name: str


@dataclass(frozen=True)
class Var:
    n: int


@dataclass(frozen=True)
class Compound:
    name: str
    args: tuple


def float_text(x):
    """The one canonical text of a float: ECMAScript's shortest form, with
    ".0" added when it would read as an integer; -0.0 keeps its sign."""
    if not math.isfinite(x):
        raise TermError(f"{x} is not a finite float")
    if x == 0:
        return "-0.0" if math.copysign(1.0, x) < 0 else "0.0"
    text = number(x)
    if "." not in text and "e" not in text:
        text += ".0"
    return text


def parse_float_text(text):
    """Read a float's text; refuse any text that is not canonical."""
    try:
        x = float(text)
    except ValueError:
        raise TermError(f"float text {text!r} is not a number") from None
    if float_text(x) != text:
        raise TermError(f"float text {text!r} is not canonical")
    return x


def encode(term):
    """Python value of a term -> JSON-ready value."""
    if isinstance(term, bool):
        raise TermError("a bool is not a term")
    if isinstance(term, int):
        if abs(term) > MAX_SAFE_INTEGER:
            raise TermError(f"integer {term} exceeds 2**53 - 1")
        return term
    if isinstance(term, float):
        return {"float": float_text(term)}
    if isinstance(term, str):
        return term
    if isinstance(term, Atom):
        return {"a": term.name}
    if isinstance(term, Var):
        if not isinstance(term.n, int) or isinstance(term.n, bool) or term.n < 0:
            raise TermError(f"variable number {term.n!r} is not a natural number")
        return {"v": term.n}
    if isinstance(term, Compound):
        if not term.args:
            raise TermError("a compound needs at least one argument")
        return {"f": term.name, "args": [encode(a) for a in term.args]}
    raise TermError(f"{type(term).__name__} is not a term")


def _keys(obj, *keys):
    if set(obj) != set(keys):
        raise TermError(f"term object has keys {sorted(obj)}, expected {list(keys)}")


def decode(value):
    """JSON value (as json.loads gives it) -> Python value of a term."""
    if isinstance(value, bool) or value is None:
        raise TermError(f"{value!r} is not a term")
    if isinstance(value, int):
        if abs(value) > MAX_SAFE_INTEGER:
            raise TermError(f"integer {value} exceeds 2**53 - 1")
        return value
    if isinstance(value, float):
        raise TermError(f"bare number {value!r}: floats are written {{\"float\": text}}")
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        if "float" in value:
            _keys(value, "float")
            if not isinstance(value["float"], str):
                raise TermError("float text must be a string")
            return parse_float_text(value["float"])
        if "a" in value:
            _keys(value, "a")
            if not isinstance(value["a"], str):
                raise TermError("atom name must be a string")
            return Atom(value["a"])
        if "v" in value:
            _keys(value, "v")
            n = value["v"]
            if isinstance(n, bool) or not isinstance(n, int) or n < 0:
                raise TermError(f"variable number {n!r} is not a natural number")
            return Var(n)
        if "f" in value:
            _keys(value, "f", "args")
            if not isinstance(value["f"], str) or not isinstance(value["args"], list) \
                    or not value["args"]:
                raise TermError("compound needs a string name and a non-empty args list")
            return Compound(value["f"], tuple(decode(a) for a in value["args"]))
    raise TermError(f"{value!r} is not a term")
