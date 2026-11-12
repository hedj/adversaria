//! SWI-Prolog as the memex's child process, spoken to over the query
//! protocol on its standard input and output (docs/design.md § Query protocol).

use format::terms::Term;
use protocol::{Atoms, DONE_CANCELLED, DONE_ERROR, DONE_OK, Kind, Message, ProtoError};
use std::fmt;
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Protocol(ProtoError),
    /// DONE 1: the error term Prolog raised.
    Prolog(Term),
    /// The server stopped, or sent what this client did not expect.
    Server(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Protocol(e) => write!(f, "protocol: {e}"),
            Error::Prolog(t) => write!(f, "Prolog error: {t:?}"),
            Error::Server(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<ProtoError> for Error {
    fn from(e: ProtoError) -> Self {
        Error::Protocol(e)
    }
}

/// How a query's answers ended, when they have.
#[derive(Debug, Clone, PartialEq)]
pub enum End {
    /// No more answers.
    Exhausted,
    Cancelled,
}

/// An open query: its request number, and the ids of the goal's variables in
/// the order each answer gives their values.
#[derive(Debug, Clone)]
pub struct Query {
    pub req: u32,
    pub vars: Vec<u64>,
}

pub struct Prolog {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    replies: Receiver<io::Result<Message>>,
    atoms: Atoms,
    next_req: u32,
}

/// prolog/server.pl, unless ADVERSARIA_PROLOG names another directory.
fn server_path() -> PathBuf {
    let dir = std::env::var_os("ADVERSARIA_PROLOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../prolog")));
    dir.join("server.pl")
}

impl Prolog {
    /// Start `swipl` with the server; its errors go to this process's stderr.
    pub fn start() -> Result<Prolog, Error> {
        let mut child = Command::new("swipl")
            .arg("-q")
            .arg(server_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdin = BufWriter::new(child.stdin.take().unwrap());
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let (tx, replies) = channel();
        std::thread::spawn(move || {
            loop {
                match protocol::read_message(&mut stdout) {
                    Ok(Some(m)) => {
                        if tx.send(Ok(m)).is_err() {
                            return;
                        }
                    }
                    Ok(None) => return,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                }
            }
        });
        Ok(Prolog {
            child,
            stdin,
            replies,
            atoms: Atoms::default(),
            next_req: 1,
        })
    }

    fn new_req(&mut self) -> u32 {
        let r = self.next_req;
        self.next_req = self.next_req.wrapping_add(1).max(1);
        r
    }

    /// Send a message as it is. For tests of the server's handling of bad input.
    pub fn send(&mut self, kind: Kind, req: u32, body: Vec<u32>) -> Result<(), Error> {
        protocol::write_message(&mut self.stdin, &Message { kind, req, body })?;
        Ok(())
    }

    /// Send bytes as they are, framed or not. For tests of bad input.
    pub fn send_bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        use std::io::Write;
        self.stdin.write_all(bytes)?;
        self.stdin.flush()?;
        Ok(())
    }

    /// The next message from the server, waiting at most `timeout`; None if
    /// none came. ATOM_IS is learned here as well as returned.
    pub fn receive(&mut self, timeout: Duration) -> Result<Option<Message>, Error> {
        match self.replies.recv_timeout(timeout) {
            Ok(Ok(m)) => {
                if m.kind == Kind::AtomIs {
                    self.learn(&m)?;
                }
                Ok(Some(m))
            }
            Ok(Err(e)) => Err(e.into()),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                Err(Error::Server("the Prolog server stopped".into()))
            }
        }
    }

    fn learn(&mut self, m: &Message) -> Result<(), Error> {
        let Some((&number, rest)) = m.body.split_first() else {
            return Err(Error::Server("empty ATOM_IS".into()));
        };
        let name = match protocol::decode(rest, &self.atoms)?.as_slice() {
            [Term::Str(s)] if number >> 29 == 2 => s.clone(),
            _ => return Err(Error::Server("malformed ATOM_IS".into())),
        };
        self.atoms.learn(number & ((1 << 29) - 1), &name);
        Ok(())
    }

    /// The next message for request `req`, skipping ATOM_IS and the late
    /// replies of requests already finished.
    fn reply(&mut self, req: u32) -> Result<Message, Error> {
        loop {
            let m = self
                .receive(Duration::from_secs(60))?
                .ok_or_else(|| Error::Server("no reply in 60 s".into()))?;
            if m.req == req && m.kind != Kind::AtomIs {
                return Ok(m);
            }
        }
    }

    /// Give every atom in the terms a number the server agrees on.
    pub fn intern(&mut self, terms: &[Term]) -> Result<(), Error> {
        for name in self.atoms.missing(terms) {
            let req = self.new_req();
            let (body, _) = protocol::encode(&[Term::Str(name.clone())], &self.atoms)?;
            self.send(Kind::Atom, req, body)?;
            loop {
                let m = self
                    .receive(Duration::from_secs(60))?
                    .ok_or_else(|| Error::Server("no ATOM_IS".into()))?;
                if m.req == req && m.kind == Kind::AtomIs {
                    break;
                }
                if m.req == req {
                    return Err(Error::Server(format!("ATOM got {:?}", m.kind)));
                }
            }
        }
        Ok(())
    }

    /// Start a query. Nothing is computed until `next` asks.
    pub fn query(&mut self, goal: &Term) -> Result<Query, Error> {
        self.intern(std::slice::from_ref(goal))?;
        let (body, vars) = protocol::encode(std::slice::from_ref(goal), &self.atoms)?;
        let req = self.new_req();
        self.send(Kind::Query, req, body)?;
        Ok(Query { req, vars })
    }

    /// Ask for up to n answers. Returns the answers (each a value per query
    /// variable) and, if the answers have ended, how.
    pub fn next(&mut self, q: &Query, n: u32) -> Result<(Vec<Vec<Term>>, Option<End>), Error> {
        let count =
            protocol::int_cell(n as i64).ok_or_else(|| Error::Server("n is too large".into()))?;
        self.send(Kind::Next, q.req, vec![count])?;
        let mut answers = Vec::new();
        while answers.len() < n as usize {
            let m = self.reply(q.req)?;
            match m.kind {
                Kind::Answer => {
                    let vals = protocol::decode(&m.body, &self.atoms)?;
                    if vals.len() != q.vars.len() {
                        return Err(Error::Server(format!(
                            "answer has {} values for {} variables",
                            vals.len(),
                            q.vars.len()
                        )));
                    }
                    answers.push(vals);
                }
                Kind::Done => return Ok((answers, Some(self.done(&m)?))),
                k => return Err(Error::Server(format!("unexpected {k:?}"))),
            }
        }
        Ok((answers, None))
    }

    fn done(&self, m: &Message) -> Result<End, Error> {
        let terms = protocol::decode(&m.body, &self.atoms)?;
        match terms.as_slice() {
            [Term::Int(DONE_OK)] => Ok(End::Exhausted),
            [Term::Int(DONE_CANCELLED)] => Ok(End::Cancelled),
            [Term::Int(DONE_ERROR), e] => Err(Error::Prolog(e.clone())),
            _ => Err(Error::Server(format!("malformed DONE {terms:?}"))),
        }
    }

    /// Drop a query and its choice points.
    pub fn cancel(&mut self, q: &Query) -> Result<End, Error> {
        self.send(Kind::Cancel, q.req, vec![])?;
        let m = self.reply(q.req)?;
        self.done(&m)
    }

    /// All answers to a goal, asking `batch` at a time.
    pub fn all(&mut self, goal: &Term, batch: u32) -> Result<Vec<Vec<Term>>, Error> {
        let q = self.query(goal)?;
        let mut out = Vec::new();
        loop {
            let (mut a, end) = self.next(&q, batch)?;
            out.append(&mut a);
            if end.is_some() {
                return Ok(out);
            }
        }
    }

    fn fact_message(&mut self, kind: Kind, fact: &Term, seq: u64) -> Result<(), Error> {
        let seq = Term::Int(i64::try_from(seq).map_err(|_| Error::Server("seq too large".into()))?);
        self.intern(std::slice::from_ref(fact))?;
        let (body, _) = protocol::encode(&[fact.clone(), seq], &self.atoms)?;
        let req = self.new_req();
        self.send(kind, req, body)?;
        let m = self.reply(req)?;
        match self.done(&m)? {
            End::Exhausted => Ok(()),
            End::Cancelled => Err(Error::Server("fact cancelled".into())),
        }
    }

    /// Add a fact, recording the journal entry it came from.
    pub fn assert_fact(&mut self, fact: &Term, seq: u64) -> Result<(), Error> {
        self.fact_message(Kind::Assert, fact, seq)
    }

    /// Remove the fact that journal entry `seq` added.
    pub fn retract_fact(&mut self, fact: &Term, seq: u64) -> Result<(), Error> {
        self.fact_message(Kind::Retract, fact, seq)
    }
}

impl Drop for Prolog {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
