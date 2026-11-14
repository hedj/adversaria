//! The query protocol's tagged cells (docs/design.md § Query protocol): the
//! Rust end. prolog/protocol.pl is the other end; its module comment states
//! the layout, which this file follows.

use format::terms::Term;
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Query = 1,
    Next = 2,
    Cancel = 3,
    Assert = 4,
    Retract = 5,
    Atom = 6,
    Answer = 7,
    Done = 8,
    AtomIs = 9,
}

impl Kind {
    fn from_code(c: u32) -> Option<Kind> {
        use Kind::*;
        [
            Query, Next, Cancel, Assert, Retract, Atom, Answer, Done, AtomIs,
        ]
        .into_iter()
        .find(|k| *k as u32 == c)
    }
}

/// DONE's status.
/// The deepest a term may nest, in compounds: each argument is one level
/// below its compound, and a list's elements one level below the list,
/// however long the list. Deeper terms are refused at both ends, so a
/// decoder never recurses further than this.
pub const MAX_DEPTH: usize = 256;

pub const DONE_OK: i64 = 0;
pub const DONE_ERROR: i64 = 1;
pub const DONE_CANCELLED: i64 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub kind: Kind,
    pub req: u32,
    pub body: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtoError(pub String);

impl fmt::Display for ProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProtoError {}

fn err<T>(m: impl Into<String>) -> Result<T, ProtoError> {
    Err(ProtoError(m.into()))
}

fn proto_io(e: ProtoError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}

/// Write one message: header, request number, body; big-endian words.
pub fn write_message(w: &mut impl Write, m: &Message) -> io::Result<()> {
    let bytes = m.bytes().map_err(proto_io)?;
    w.write_all(&bytes)?;
    w.flush()
}

impl Message {
    /// The message as bytes: its header, its request number and its body.
    fn bytes(&self) -> Result<Vec<u8>, ProtoError> {
        let start = [self.header()?, self.req].into_iter();
        let words = start.chain(self.body.iter().copied());
        Ok(words.flat_map(u32::to_be_bytes).collect())
    }

    /// The header word: the kind, and how many words follow it.
    fn header(&self) -> Result<u32, ProtoError> {
        let len = self.body.len() + 1;
        if len > 0xffff {
            return err(format!("message of {len} words is too long"));
        }
        Ok(((self.kind as u32) << 24) | len as u32)
    }
}

/// Read one message; None at a clean end of input.
pub fn read_message(r: &mut impl Read) -> io::Result<Option<Message>> {
    let Some(header) = read_header(r)? else {
        return Ok(None);
    };
    let (kind, len) = parse_header(header).map_err(proto_io)?;
    let words = read_words(r, len)?;
    let (req, body) = (words[0], words[1..].to_vec());
    Ok(Some(Message { kind, req, body }))
}

/// The next header word; None if the input ends before it.
fn read_header(r: &mut impl Read) -> io::Result<Option<u32>> {
    let mut h = [0u8; 4];
    match r.read_exact(&mut h) {
        Ok(()) => Ok(Some(u32::from_be_bytes(h))),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(e),
    }
}

/// A header's kind, and how many words follow it: at least one, the
/// request number.
fn parse_header(header: u32) -> Result<(Kind, usize), ProtoError> {
    let (code, reserved, len) = (header >> 24, (header >> 16) & 0xff, header & 0xffff);
    if reserved != 0 {
        return err(format!("reserved field is {reserved}, not 0"));
    }
    let Some(kind) = Kind::from_code(code) else {
        return err(format!("unknown message type {code}"));
    };
    if len == 0 {
        return err("message has no request number");
    }
    Ok((kind, len as usize))
}

fn read_words(r: &mut impl Read, len: usize) -> io::Result<Vec<u32>> {
    let mut buf = vec![0u8; 4 * len];
    r.read_exact(&mut buf)?;
    Ok(be_words(&buf))
}

/// Bytes, four at a time, as big-endian words.
fn be_words(bytes: &[u8]) -> Vec<u32> {
    let word = |c: &[u8]| u32::from_be_bytes(c.try_into().unwrap());
    bytes.chunks(4).map(word).collect()
}

/// The atom numbers both ends agree on. 0 and 1 are fixed: `[]` and `.`.
#[derive(Debug, Clone)]
pub struct Atoms {
    by_name: HashMap<String, u32>,
    by_number: HashMap<u32, String>,
}

impl Default for Atoms {
    fn default() -> Self {
        let mut a = Atoms {
            by_name: HashMap::new(),
            by_number: HashMap::new(),
        };
        a.learn(0, "[]");
        a.learn(1, ".");
        a
    }
}

impl Atoms {
    /// Record the server's number for a name (from ATOM_IS).
    pub fn learn(&mut self, number: u32, name: &str) {
        self.by_name.insert(name.to_string(), number);
        self.by_number.insert(number, name.to_string());
    }

    pub fn number(&self, name: &str) -> Option<u32> {
        self.by_name.get(name).copied()
    }

    pub fn name(&self, number: u32) -> Option<&str> {
        self.by_number.get(&number).map(String::as_str)
    }

    /// The names in these terms that have no number yet, each once.
    pub fn missing(&self, terms: &[Term]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for name in terms.iter().flat_map(names) {
            if self.number(name).is_none() && !out.contains(name) {
                out.push(name.clone());
            }
        }
        out
    }
}

/// The names a term uses, its atoms and functors: a compound's own name
/// after those of its arguments.
fn names(t: &Term) -> Vec<&String> {
    match t {
        Term::Atom(n) => vec![n],
        Term::Compound(n, args) => args.iter().flat_map(names).chain([n]).collect(),
        _ => vec![],
    }
}

const VAR: u32 = 0;
const INT: u32 = 1;
const ATOM: u32 = 2;
const STRUCT: u32 = 3;
const BLOB: u32 = 4;
const VALUE: u32 = (1 << 29) - 1;
/// The part of a struct cell's value that numbers its functor.
const FUNCTOR: u32 = (1 << 21) - 1;
/// The struct cell of a list: './2', whose name has the fixed number 1.
const CONS: u32 = (STRUCT << 29) | (2 << 21) | 1;

fn cell(tag: u32, value: u32) -> u32 {
    (tag << 29) | value
}

pub fn int_cell(i: i64) -> Option<u32> {
    (-(1i64 << 28)..(1i64 << 28))
        .contains(&i)
        .then(|| cell(INT, (i as u32) & VALUE))
}

/// The integer of an INT cell's value: 29 bits, two's complement.
fn signed(v: u32) -> i64 {
    let v = v as i64;
    if v >= 1 << 28 { v - (1 << 29) } else { v }
}

/// Encode terms as one cell sequence. The terms' variables are numbered in
/// order of first appearance; the result lists their original ids in that
/// order, so answers can be matched to them.
pub fn encode(terms: &[Term], atoms: &Atoms) -> Result<(Vec<u32>, Vec<u64>), ProtoError> {
    let mut e = Encoder::new(atoms);
    terms.iter().try_for_each(|t| e.term(t))?;
    Ok((e.cells, e.vars))
}

/// Terms being encoded: the cells written, and the variables met, in
/// order of first appearance.
struct Encoder<'a> {
    atoms: &'a Atoms,
    cells: Vec<u32>,
    vars: Vec<u64>,
}

impl<'a> Encoder<'a> {
    fn new(atoms: &'a Atoms) -> Self {
        let (cells, vars) = (Vec::new(), Vec::new());
        Encoder { atoms, cells, vars }
    }

    fn term(&mut self, t: &Term) -> Result<(), ProtoError> {
        match t {
            Term::Var(v) => self.var(*v),
            Term::Int(i) => self.int(*i),
            Term::Float(x) => self.float(*x),
            Term::Str(s) => self.blob(b's', s.as_bytes()),
            Term::Atom(n) => self.atom(n),
            Term::Compound(n, args) => self.compound(n, args),
        }
    }

    fn var(&mut self, v: u64) -> Result<(), ProtoError> {
        let k = self.var_number(v);
        self.cells.push(cell(VAR, k as u32));
        Ok(())
    }

    /// A variable's number: its place in order of first appearance.
    fn var_number(&mut self, v: u64) -> usize {
        let known = self.vars.iter().position(|x| *x == v);
        known.unwrap_or_else(|| {
            self.vars.push(v);
            self.vars.len() - 1
        })
    }

    /// An integer: in a cell if it fits 29 bits, else as its digits.
    fn int(&mut self, i: i64) -> Result<(), ProtoError> {
        match int_cell(i) {
            Some(c) => self.cells.push(c),
            None => self.blob(b'i', i.to_string().as_bytes())?,
        }
        Ok(())
    }

    fn float(&mut self, x: f64) -> Result<(), ProtoError> {
        if !x.is_finite() {
            return err(format!("{x} has no cell form"));
        }
        // Shortest digits that read back exactly; Prolog reads "1e23" as a float.
        self.blob(b'f', format!("{x:e}").as_bytes())
    }

    fn atom(&mut self, n: &str) -> Result<(), ProtoError> {
        let no = self.number(n)?;
        self.cells.push(cell(ATOM, no));
        Ok(())
    }

    fn number(&self, n: &str) -> Result<u32, ProtoError> {
        let no = self.atoms.number(n);
        no.ok_or_else(|| ProtoError(format!("atom {n:?} has no number yet")))
    }

    fn compound(&mut self, n: &str, args: &[Term]) -> Result<(), ProtoError> {
        let functor = self.functor(n, args.len())?;
        self.cells.push(functor);
        args.iter().try_for_each(|a| self.term(a))
    }

    /// The cell that opens a compound: its arity and its name's number.
    fn functor(&self, n: &str, arity: usize) -> Result<u32, ProtoError> {
        if arity == 0 || arity > 255 {
            return err(format!("{n}/{arity}: arity must be 1 to 255"));
        }
        let no = self.number(n)?;
        if no > FUNCTOR {
            return err(format!("functor {n:?} has number {no}, beyond 2^21"));
        }
        Ok(cell(STRUCT, ((arity as u32) << 21) | no))
    }

    /// A blob: a cell with its length in bytes, then its kind and its
    /// data in words, padded with zeros.
    fn blob(&mut self, kind: u8, data: &[u8]) -> Result<(), ProtoError> {
        let n = data.len() + 1;
        if n >= 1 << 29 {
            return err("blob too long");
        }
        self.cells.push(cell(BLOB, n as u32));
        self.cells.extend(blob_words(kind, data));
        Ok(())
    }
}

fn blob_words(kind: u8, data: &[u8]) -> Vec<u32> {
    let mut bytes = vec![kind];
    bytes.extend_from_slice(data);
    bytes.resize(bytes.len().div_ceil(4) * 4, 0);
    be_words(&bytes)
}

/// Decode a cell sequence into terms. VAR n becomes Term::Var(n); the
/// numbers must be dense from 0, in order of first appearance.
pub fn decode(cells: &[u32], atoms: &Atoms) -> Result<Vec<Term>, ProtoError> {
    let mut d = Decoder::new(cells, atoms);
    let mut terms = Vec::new();
    while d.pos < cells.len() {
        terms.push(d.term(0)?);
    }
    Ok(terms)
}

/// Cells being decoded: where the next cell is, and how many variables
/// have been met.
struct Decoder<'a> {
    cells: &'a [u32],
    atoms: &'a Atoms,
    pos: usize,
    seen: u64,
}

impl<'a> Decoder<'a> {
    fn new(cells: &'a [u32], atoms: &'a Atoms) -> Self {
        let (pos, seen) = (0, 0);
        Decoder {
            cells,
            atoms,
            pos,
            seen,
        }
    }

    /// The next term, `depth` compounds deep.
    fn term(&mut self, depth: usize) -> Result<Term, ProtoError> {
        let Some(c) = self.take(1).map(|c| c[0]) else {
            return err("term is cut short");
        };
        let v = c & VALUE;
        match c >> 29 {
            VAR => self.var(v as u64),
            INT => Ok(Term::Int(signed(v))),
            ATOM => self.name(v).map(Term::Atom),
            STRUCT => self.structure(v, depth),
            BLOB => self.blob(v as usize),
            t => err(format!("unknown tag {t}")),
        }
    }

    /// The next `count` cells, stepped over; None if there are fewer.
    fn take(&mut self, count: usize) -> Option<&'a [u32]> {
        let cells = self.cells.get(self.pos..self.pos + count)?;
        self.pos += count;
        Some(cells)
    }

    /// A variable: one met before, or the next new one.
    fn var(&mut self, k: u64) -> Result<Term, ProtoError> {
        if k > self.seen {
            return err(format!("variable {k} before variable {}", self.seen));
        }
        if k == self.seen {
            self.seen += 1;
        }
        Ok(Term::Var(k))
    }

    fn name(&self, no: u32) -> Result<String, ProtoError> {
        let name = self.atoms.name(no).map(String::from);
        name.ok_or_else(|| ProtoError(format!("unknown atom number {no}")))
    }

    /// The term a struct cell opens: a list, or a compound.
    fn structure(&mut self, v: u32, depth: usize) -> Result<Term, ProtoError> {
        let arity = (v >> 21) as usize;
        if arity == 0 {
            return err("a struct of arity 0");
        }
        if depth >= MAX_DEPTH {
            return err(format!("a term nested deeper than {MAX_DEPTH}"));
        }
        if cell(STRUCT, v) == CONS {
            return self.list(depth);
        }
        self.compound(v & FUNCTOR, arity, depth)
    }

    fn compound(&mut self, functor: u32, arity: usize, depth: usize) -> Result<Term, ProtoError> {
        let name = self.name(functor)?;
        let args = (0..arity).map(|_| self.term(depth + 1));
        Ok(Term::Compound(name, args.collect::<Result<_, _>>()?))
    }

    /// A list, after its first cons cell. Its spine is walked in a loop,
    /// so a long list does not recurse once for each element.
    fn list(&mut self, depth: usize) -> Result<Term, ProtoError> {
        let mut heads = vec![self.term(depth + 1)?];
        while self.take_cons() {
            heads.push(self.term(depth + 1)?);
        }
        let tail = self.term(depth)?;
        let cons = |t, h| Term::Compound(".".into(), vec![h, t]);
        Ok(heads.into_iter().rev().fold(tail, cons))
    }

    /// Step over a cons cell, if one is next.
    fn take_cons(&mut self) -> bool {
        let next = self.cells.get(self.pos) == Some(&CONS);
        self.pos += next as usize;
        next
    }

    /// A blob of n bytes: its kind, then its text.
    fn blob(&mut self, n: usize) -> Result<Term, ProtoError> {
        let bytes = self.blob_bytes(n)?;
        let text = std::str::from_utf8(&bytes[1..]);
        let text = text.map_err(|_| ProtoError("blob is not UTF-8".into()))?;
        blob_term(bytes[0], text)
    }

    /// A blob's n bytes, from whole words whose padding is zero.
    fn blob_bytes(&mut self, n: usize) -> Result<Vec<u8>, ProtoError> {
        let Some(words) = self.take(n.div_ceil(4)) else {
            return err("blob is cut short");
        };
        let mut bytes: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        if n == 0 || bytes[n..].iter().any(|b| *b != 0) {
            return err("blob is empty or its padding is not zero");
        }
        bytes.truncate(n);
        Ok(bytes)
    }
}

/// The term a blob holds, by its kind: a string, an integer or a float.
fn blob_term(kind: u8, text: &str) -> Result<Term, ProtoError> {
    match kind {
        b's' => Ok(Term::Str(text.into())),
        b'i' => blob_int(text),
        b'f' => blob_float(text),
        k => err(format!("unknown blob kind {k}")),
    }
}

fn blob_int(text: &str) -> Result<Term, ProtoError> {
    let i = text.parse().map(Term::Int);
    i.map_err(|_| ProtoError(format!("integer {text:?} does not fit 64 bits")))
}

fn blob_float(text: &str) -> Result<Term, ProtoError> {
    let Ok(x) = text.parse::<f64>() else {
        return err(format!("bad float text {text:?}"));
    };
    if !x.is_finite() {
        return err(format!("float text {text:?} is not finite"));
    }
    Ok(Term::Float(x))
}

/// Equal up to a consistent renaming of variables.
pub fn variant(a: &Term, b: &Term) -> bool {
    Renaming::default().matches(a, b)
}

/// A renaming of one term's variables to another's, and back.
#[derive(Default)]
struct Renaming {
    there: HashMap<u64, u64>,
    back: HashMap<u64, u64>,
}

impl Renaming {
    fn matches(&mut self, a: &Term, b: &Term) -> bool {
        match (a, b) {
            (Term::Var(x), Term::Var(y)) => self.pairs(*x, *y),
            (Term::Compound(f, xs), Term::Compound(g, ys)) => f == g && self.all_match(xs, ys),
            _ => a == b,
        }
    }

    /// Whether x may be renamed to y: each is paired with the other only.
    fn pairs(&mut self, x: u64, y: u64) -> bool {
        let there = *self.there.entry(x).or_insert(y);
        there == y && *self.back.entry(y).or_insert(x) == x
    }

    fn all_match(&mut self, xs: &[Term], ys: &[Term]) -> bool {
        xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| self.matches(x, y))
    }
}

#[cfg(test)]
mod tests;
