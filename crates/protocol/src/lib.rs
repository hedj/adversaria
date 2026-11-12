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
    let len = m.body.len() + 1;
    if len > 0xffff {
        return Err(proto_io(ProtoError(format!(
            "message of {len} words is too long"
        ))));
    }
    let mut bytes = Vec::with_capacity(4 * (len + 1));
    for word in [((m.kind as u32) << 24) | len as u32, m.req]
        .iter()
        .chain(&m.body)
    {
        bytes.extend_from_slice(&word.to_be_bytes());
    }
    w.write_all(&bytes)?;
    w.flush()
}

/// Read one message; None at a clean end of input.
pub fn read_message(r: &mut impl Read) -> io::Result<Option<Message>> {
    let mut h = [0u8; 4];
    match r.read_exact(&mut h) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let header = u32::from_be_bytes(h);
    let reserved = (header >> 16) & 0xff;
    let len = (header & 0xffff) as usize;
    if reserved != 0 {
        return Err(proto_io(ProtoError(format!(
            "reserved field is {reserved}, not 0"
        ))));
    }
    let Some(kind) = Kind::from_code(header >> 24) else {
        return Err(proto_io(ProtoError(format!(
            "unknown message type {}",
            header >> 24
        ))));
    };
    if len == 0 {
        return Err(proto_io(ProtoError("message has no request number".into())));
    }
    let mut buf = vec![0u8; 4 * len];
    r.read_exact(&mut buf)?;
    let words: Vec<u32> = buf
        .chunks(4)
        .map(|c| u32::from_be_bytes(c.try_into().unwrap()))
        .collect();
    Ok(Some(Message {
        kind,
        req: words[0],
        body: words[1..].to_vec(),
    }))
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
        let mut out = Vec::new();
        fn walk(t: &Term, a: &Atoms, out: &mut Vec<String>) {
            let name = match t {
                Term::Atom(n) => Some(n),
                Term::Compound(n, args) => {
                    args.iter().for_each(|x| walk(x, a, out));
                    Some(n)
                }
                _ => None,
            };
            if let Some(n) = name
                && a.number(n).is_none()
                && !out.contains(n)
            {
                out.push(n.clone());
            }
        }
        terms.iter().for_each(|t| walk(t, self, &mut out));
        out
    }
}

const VAR: u32 = 0;
const INT: u32 = 1;
const ATOM: u32 = 2;
const STRUCT: u32 = 3;
const BLOB: u32 = 4;
const VALUE: u32 = (1 << 29) - 1;

fn cell(tag: u32, value: u32) -> u32 {
    (tag << 29) | value
}

pub fn int_cell(i: i64) -> Option<u32> {
    (-(1i64 << 28)..(1i64 << 28))
        .contains(&i)
        .then(|| cell(INT, (i as u32) & VALUE))
}

/// Encode terms as one cell sequence. The terms' variables are numbered in
/// order of first appearance; the result lists their original ids in that
/// order, so answers can be matched to them.
pub fn encode(terms: &[Term], atoms: &Atoms) -> Result<(Vec<u32>, Vec<u64>), ProtoError> {
    let mut cells = Vec::new();
    let mut vars = Vec::new();
    for t in terms {
        encode_term(t, atoms, &mut cells, &mut vars)?;
    }
    Ok((cells, vars))
}

fn encode_term(
    t: &Term,
    atoms: &Atoms,
    out: &mut Vec<u32>,
    vars: &mut Vec<u64>,
) -> Result<(), ProtoError> {
    let number = |n: &str| {
        atoms
            .number(n)
            .ok_or_else(|| ProtoError(format!("atom {n:?} has no number yet")))
    };
    match t {
        Term::Var(v) => {
            let k = vars.iter().position(|x| x == v).unwrap_or_else(|| {
                vars.push(*v);
                vars.len() - 1
            });
            out.push(cell(VAR, k as u32));
        }
        Term::Int(i) => match int_cell(*i) {
            Some(c) => out.push(c),
            None => blob(b'i', i.to_string().as_bytes(), out)?,
        },
        Term::Float(x) => {
            if !x.is_finite() {
                return err(format!("{x} has no cell form"));
            }
            // Shortest digits that read back exactly; Prolog reads "1e23" as a float.
            blob(b'f', format!("{x:e}").as_bytes(), out)?
        }
        Term::Str(s) => blob(b's', s.as_bytes(), out)?,
        Term::Atom(n) => out.push(cell(ATOM, number(n)?)),
        Term::Compound(n, args) => {
            if args.is_empty() || args.len() > 255 {
                return err(format!("{n}/{}: arity must be 1 to 255", args.len()));
            }
            let no = number(n)?;
            if no >= 1 << 21 {
                return err(format!("functor {n:?} has number {no}, beyond 2^21"));
            }
            out.push(cell(STRUCT, ((args.len() as u32) << 21) | no));
            for a in args {
                encode_term(a, atoms, out, vars)?;
            }
        }
    }
    Ok(())
}

fn blob(kind: u8, data: &[u8], out: &mut Vec<u32>) -> Result<(), ProtoError> {
    let n = data.len() + 1;
    if n >= 1 << 29 {
        return err("blob too long");
    }
    out.push(cell(BLOB, n as u32));
    let mut bytes = Vec::with_capacity(n + 3);
    bytes.push(kind);
    bytes.extend_from_slice(data);
    bytes.resize(n.div_ceil(4) * 4, 0);
    out.extend(
        bytes
            .chunks(4)
            .map(|c| u32::from_be_bytes(c.try_into().unwrap())),
    );
    Ok(())
}

/// Decode a cell sequence into terms. VAR n becomes Term::Var(n); the
/// numbers must be dense from 0, in order of first appearance.
pub fn decode(cells: &[u32], atoms: &Atoms) -> Result<Vec<Term>, ProtoError> {
    let mut pos = 0;
    let mut terms = Vec::new();
    let mut seen = 0u64;
    while pos < cells.len() {
        terms.push(decode_term(cells, &mut pos, atoms, &mut seen)?);
    }
    Ok(terms)
}

fn decode_term(
    cells: &[u32],
    pos: &mut usize,
    atoms: &Atoms,
    seen: &mut u64,
) -> Result<Term, ProtoError> {
    let Some(&c) = cells.get(*pos) else {
        return err("term is cut short");
    };
    *pos += 1;
    let v = c & VALUE;
    let name = |no: u32| {
        atoms
            .name(no)
            .map(String::from)
            .ok_or_else(|| ProtoError(format!("unknown atom number {no}")))
    };
    Ok(match c >> 29 {
        VAR => {
            let k = v as u64;
            if k > *seen {
                return err(format!("variable {k} before variable {seen}"));
            }
            if k == *seen {
                *seen += 1;
            }
            Term::Var(k)
        }
        INT => Term::Int(if v >= 1 << 28 {
            v as i64 - (1 << 29)
        } else {
            v as i64
        }),
        ATOM => Term::Atom(name(v)?),
        STRUCT => {
            let arity = (v >> 21) as usize;
            if arity == 0 {
                return err("a struct of arity 0");
            }
            let fno = v & ((1 << 21) - 1);
            if arity == 2 && fno == 1 {
                // A list: walk its spine in a loop, so a long list does not
                // recurse once per element.
                let mut heads = vec![decode_term(cells, pos, atoms, seen)?];
                while let Some(&c) = cells.get(*pos) {
                    if c >> 29 == STRUCT && (c & VALUE) >> 21 == 2 && c & ((1 << 21) - 1) == 1 {
                        *pos += 1;
                        heads.push(decode_term(cells, pos, atoms, seen)?);
                    } else {
                        break;
                    }
                }
                let tail = decode_term(cells, pos, atoms, seen)?;
                return Ok(heads
                    .into_iter()
                    .rev()
                    .fold(tail, |t, h| Term::Compound(".".into(), vec![h, t])));
            }
            let f = name(fno)?;
            let args = (0..arity)
                .map(|_| decode_term(cells, pos, atoms, seen))
                .collect::<Result<_, _>>()?;
            Term::Compound(f, args)
        }
        BLOB => {
            let n = v as usize;
            let words = n.div_ceil(4);
            let Some(ws) = cells.get(*pos..*pos + words) else {
                return err("blob is cut short");
            };
            *pos += words;
            let bytes: Vec<u8> = ws.iter().flat_map(|w| w.to_be_bytes()).collect();
            if n == 0 || bytes[n..].iter().any(|b| *b != 0) {
                return err("blob is empty or its padding is not zero");
            }
            let text = std::str::from_utf8(&bytes[1..n])
                .map_err(|_| ProtoError("blob is not UTF-8".into()))?;
            match bytes[0] {
                b's' => Term::Str(text.into()),
                b'i' => {
                    Term::Int(text.parse().map_err(|_| {
                        ProtoError(format!("integer {text:?} does not fit 64 bits"))
                    })?)
                }
                b'f' => {
                    let x: f64 = text
                        .parse()
                        .map_err(|_| ProtoError(format!("bad float text {text:?}")))?;
                    if !x.is_finite() {
                        return err(format!("float text {text:?} is not finite"));
                    }
                    Term::Float(x)
                }
                k => return err(format!("unknown blob kind {k}")),
            }
        }
        t => return err(format!("unknown tag {t}")),
    })
}

/// Equal up to a consistent renaming of variables.
pub fn variant(a: &Term, b: &Term) -> bool {
    fn go(a: &Term, b: &Term, ab: &mut HashMap<u64, u64>, ba: &mut HashMap<u64, u64>) -> bool {
        match (a, b) {
            (Term::Var(x), Term::Var(y)) => {
                *ab.entry(*x).or_insert(*y) == *y && *ba.entry(*y).or_insert(*x) == *x
            }
            (Term::Compound(f, xs), Term::Compound(g, ys)) => {
                f == g && xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| go(x, y, ab, ba))
            }
            _ => a == b,
        }
    }
    go(a, b, &mut HashMap::new(), &mut HashMap::new())
}

#[cfg(test)]
mod tests;
