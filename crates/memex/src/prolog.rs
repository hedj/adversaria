//! SWI-Prolog as the memex's child process, spoken to over the query
//! protocol on its standard input and output (docs/design.md § Query protocol).

use format::terms::Term;
use protocol::{Atoms, DONE_CANCELLED, DONE_ERROR, DONE_OK, Kind, Message, ProtoError};
use std::fmt;
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Protocol(ProtoError),
    /// DONE 1: the error term Prolog raised.
    Prolog(Term),
    /// The server sent what this client did not expect.
    Server(String),
    /// The connection is dead: a garbled message in either direction, or the
    /// server stopped. The child is killed, and every later call fails with
    /// this reason. Nothing restarts it.
    Dead(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Protocol(e) => write!(f, "protocol: {e}"),
            Error::Prolog(t) => write!(f, "Prolog error: {t:?}"),
            Error::Server(m) => f.write_str(m),
            Error::Dead(m) => write!(f, "the Prolog server is dead: {m}"),
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
    dead: Option<String>,
}

/// prolog/server.pl, unless ADVERSARIA_PROLOG names another directory.
fn server_path() -> PathBuf {
    let dir = std::env::var_os("ADVERSARIA_PROLOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../prolog")));
    dir.join("server.pl")
}

/// Start `swipl` on a server file; its errors go to this process's stderr.
fn spawn(server: &std::path::Path) -> io::Result<Child> {
    let mut swipl = Command::new("swipl");
    let piped = swipl
        .arg("-q")
        .arg(server)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    piped.stderr(Stdio::inherit()).spawn()
}

/// Read the server's messages in a thread of their own, until its output
/// ends or is garbled; they arrive on the channel returned.
fn listen(stdout: ChildStdout) -> Receiver<io::Result<Message>> {
    let mut stdout = BufReader::new(stdout);
    let (tx, replies) = channel();
    std::thread::spawn(move || {
        while let Some(m) = protocol::read_message(&mut stdout).transpose() {
            let last = m.is_err();
            if tx.send(m).is_err() || last {
                return;
            }
        }
    });
    replies
}

impl Prolog {
    /// Start `swipl` with the server; its errors go to this process's stderr.
    pub fn start() -> Result<Prolog, Error> {
        Prolog::start_with(&server_path())
    }

    /// Start `swipl` with another server file. For tests.
    pub fn start_with(server: &std::path::Path) -> Result<Prolog, Error> {
        let mut child = spawn(server)?;
        let stdin = BufWriter::new(child.stdin.take().unwrap());
        let replies = listen(child.stdout.take().unwrap());
        Ok(Prolog {
            child,
            stdin,
            replies,
            atoms: Atoms::default(),
            next_req: 1,
            dead: None,
        })
    }

    /// Kill the child, record why, and return the error every later call gets.
    fn die(&mut self, reason: String) -> Error {
        let _ = self.child.kill();
        let status = self
            .child
            .wait()
            .map(|s| format!(" ({s})"))
            .unwrap_or_default();
        let reason = format!("{reason}{status}");
        self.dead = Some(reason.clone());
        Error::Dead(reason)
    }

    fn alive(&self) -> Result<(), Error> {
        match &self.dead {
            Some(r) => Err(Error::Dead(r.clone())),
            None => Ok(()),
        }
    }

    fn new_req(&mut self) -> u32 {
        let r = self.next_req;
        self.next_req = self.next_req.wrapping_add(1).max(1);
        r
    }

    /// Send a message as it is. For tests of the server's handling of bad input.
    pub fn send(&mut self, kind: Kind, req: u32, body: Vec<u32>) -> Result<(), Error> {
        self.alive()?;
        let m = Message { kind, req, body };
        if m.body.len() >= 0xffff {
            return Err(Error::Server(format!(
                "message of {} cells is too long",
                m.body.len()
            )));
        }
        protocol::write_message(&mut self.stdin, &m)
            .map_err(|e| self.die(format!("cannot write to it: {e}")))
    }

    /// Send bytes as they are, framed or not. For tests of bad input.
    pub fn send_bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        use std::io::Write;
        self.alive()?;
        self.stdin
            .write_all(bytes)
            .and_then(|_| self.stdin.flush())
            .map_err(|e| self.die(format!("cannot write to it: {e}")))
    }

    /// The next message from the server, waiting at most `timeout`; None if
    /// none came. ATOM_IS is learned here as well as returned.
    pub fn receive(&mut self, timeout: Duration) -> Result<Option<Message>, Error> {
        // No alive() check: once dead, the channel is closed, and die()
        // gives the same reason again.
        match self.replies.recv_timeout(timeout) {
            Ok(Ok(m)) => {
                if m.kind == Kind::AtomIs {
                    self.learn(&m)?;
                }
                Ok(Some(m))
            }
            Ok(Err(e)) => Err(self.die(format!("garbled message from it: {e}"))),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(self.die("it stopped".into())),
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
            self.await_atom(req)?;
        }
        Ok(())
    }

    /// Wait for the ATOM_IS that answers request `req`; receiving it has
    /// learned the number.
    fn await_atom(&mut self, req: u32) -> Result<(), Error> {
        loop {
            let m = self.receive(Duration::from_secs(60))?;
            let m = m.ok_or_else(|| Error::Server("no ATOM_IS".into()))?;
            match (m.req == req, m.kind) {
                (true, Kind::AtomIs) => return Ok(()),
                (true, kind) => return Err(Error::Server(format!("ATOM got {kind:?}"))),
                (false, _) => {}
            }
        }
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
                Kind::Answer => answers.push(self.answer(&m, q.vars.len())?),
                Kind::Done => return Ok((answers, Some(self.done(&m)?))),
                k => return Err(Error::Server(format!("unexpected {k:?}"))),
            }
        }
        Ok((answers, None))
    }

    /// An answer's values: one for each of the query's variables.
    fn answer(&self, m: &Message, variables: usize) -> Result<Vec<Term>, Error> {
        let vals = protocol::decode(&m.body, &self.atoms)?;
        if vals.len() != variables {
            let n = vals.len();
            return Err(Error::Server(format!(
                "answer has {n} values for {variables} variables"
            )));
        }
        Ok(vals)
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
