//! Milestone 2's acceptance checks (docs/plan.md, Milestone 2), against a
//! real SWI-Prolog child process.

use format::terms::Term;
use memex::prolog::{End, Error, Prolog};
use protocol::Kind;
use std::time::{Duration, Instant};

fn a(n: &str) -> Term {
    Term::Atom(n.into())
}
fn c(f: &str, args: Vec<Term>) -> Term {
    Term::Compound(f.into(), args)
}
fn s(x: &str) -> Term {
    Term::Str(x.into())
}
fn i(n: i64) -> Term {
    Term::Int(n)
}
fn v(n: u64) -> Term {
    Term::Var(n)
}
fn and(x: Term, y: Term) -> Term {
    c(",", vec![x, y])
}

/// SplitMix64, so generated terms repeat exactly.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// One of a list, at random.
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }
}

const NAMES: [&str; 10] = [
    "a",
    "foo",
    "Bar baz",
    "é€😀",
    "[]",
    "[|]",
    "",
    " ",
    "f",
    "supports",
];

/// Integers at the edges of the protocol's sizes.
const INTS: [i64; 9] = [
    0,
    1,
    -1,
    268435455,
    -268435456,
    268435456,
    -268435457,
    i64::MAX,
    i64::MIN,
];

/// Atoms, integers small and large, floats, strings, variables 0-4, lists
/// and nested compounds.
fn gen_term(rng: &mut Rng, depth: u32) -> Term {
    match rng.below(if depth >= 4 { 7 } else { 9 }) {
        0 => a(rng.pick(&NAMES)),
        1 => i(rng.pick(&INTS)),
        2 => i(rng.below(2_000_001) as i64 - 1_000_000),
        3 => Term::Float(gen_float(rng)),
        4 => Term::Str(gen_text(rng)),
        5 => v(rng.below(5)),
        6 => s(rng.pick(&["", "a\nb", "😀"])),
        7 => gen_compound(rng, depth),
        _ => gen_list(rng, depth),
    }
}

/// A finite float: the one drawn, or one of six that are hard to print.
fn gen_float(rng: &mut Rng) -> f64 {
    loop {
        let x = f64::from_bits(rng.next());
        if x.is_finite() {
            return rng.pick(&[x, 0.0, -0.0, 1.0, 0.1, 1e23, 5e-324]);
        }
    }
}

/// Up to six characters, from the first 12,287 after NUL.
fn gen_text(rng: &mut Rng) -> String {
    let n = rng.below(7);
    (0..n)
        .filter_map(|_| char::from_u32(1 + rng.below(0x2fff) as u32))
        .collect()
}

/// A compound of one to four terms, one level deeper. '[|]'/2 is
/// SWI-Prolog's list cell, so it cannot cross (tested below).
fn gen_compound(rng: &mut Rng, depth: u32) -> Term {
    let n = 1 + rng.below(4);
    let args: Vec<Term> = (0..n).map(|_| gen_term(rng, depth + 1)).collect();
    let crosses = |name: &&str| !(*name == "[|]" && args.len() == 2);
    let names: Vec<&str> = NAMES.iter().copied().filter(crosses).collect();
    c(rng.pick(&names), args)
}

/// A list of up to three terms, one level deeper, built from its end.
fn gen_list(rng: &mut Rng, depth: u32) -> Term {
    let n = rng.below(4);
    (0..n).fold(a("[]"), |tail, _| {
        c(".", vec![gen_term(rng, depth + 1), tail])
    })
}

#[test]
fn rust_to_prolog_to_rust() {
    // Plan: every one of 10,000 generated terms "encodes and decodes to itself,
    // both directions, both languages". Rust encodes, Prolog decodes, unifies
    // X with it, and encodes X back; Rust decodes and compares.
    let mut p = Prolog::start().unwrap();
    let mut rng = Rng(11);
    let t0 = Instant::now();
    for k in 0..10_000 {
        let t = gen_term(&mut rng, 0);
        let goal = c("=", vec![v(1000), t.clone()]);
        let q = p.query(&goal).unwrap();
        let (ans, end) = p.next(&q, 2).unwrap();
        assert_eq!((ans.len(), end), (1, Some(End::Exhausted)), "term {k}");
        // Compare X and the goal's own variables, under one renaming.
        let want = c(
            "t",
            q.vars
                .iter()
                .map(|id| if *id == 1000 { t.clone() } else { v(*id) })
                .collect(),
        );
        assert!(
            protocol::variant(&c("t", ans[0].clone()), &want),
            "term {k}: {t:?} came back as {:?}",
            ans[0]
        );
    }
    eprintln!(
        "10,000 Rust -> Prolog -> Rust round trips in {:?}",
        t0.elapsed()
    );
}

#[test]
fn integers_beyond_64_bits_refused_in_rust() {
    // Catches an integer too large for Rust's Term silently truncated.
    let mut p = Prolog::start().unwrap();
    let e = p
        .all(&c("is", vec![v(0), c("**", vec![i(2), i(100)])]), 1)
        .unwrap_err();
    assert!(
        matches!(&e, Error::Protocol(m) if m.0.contains("64 bits")),
        "{e:?}"
    );
    let max = p
        .all(
            &c(
                "is",
                vec![v(0), c("-", vec![c("**", vec![i(2), i(63)]), i(1)])],
            ),
            1,
        )
        .unwrap();
    assert_eq!(max, vec![vec![i(i64::MAX)]]);
}

#[test]
fn list_cell_clash_refused() {
    // Catches '[|]'(a, b) from Rust turning silently into the list [a|b].
    let mut p = Prolog::start().unwrap();
    let e = p
        .all(&c("=", vec![v(0), c("[|]", vec![a("a"), a("b")])]), 1)
        .unwrap_err();
    assert!(
        matches!(&e, Error::Prolog(Term::Compound(f, args)) if f == "protocol_error" && args[0] == a("list_cell_clash")),
        "{e:?}"
    );
    assert_eq!(
        p.all(&c("=", vec![v(0), a("[|]")]), 1).unwrap(),
        vec![vec![a("[|]")]],
        "the plain atom crosses"
    );
}

#[test]
fn prolog_to_rust_to_prolog() {
    // The other direction: Prolog generates and encodes a term, Rust decodes
    // and re-encodes it, and Prolog checks it is the term it made.
    let mut p = Prolog::start().unwrap();
    let generator = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../prolog/tests/gen_terms.pl"
    );
    assert_eq!(
        p.all(&c("use_module", vec![s(generator)]), 1)
            .unwrap()
            .len(),
        1
    );
    let t0 = Instant::now();
    for seed in 0..10_000 {
        let ans = p.all(&c("seeded_term", vec![i(seed), v(0)]), 2).unwrap();
        assert_eq!(ans.len(), 1, "seed {seed}");
        let back = p
            .all(&c("same_as_seeded", vec![i(seed), ans[0][0].clone()]), 2)
            .unwrap();
        assert_eq!(
            back.len(),
            1,
            "seed {seed}: {:?} did not come back as itself",
            ans[0][0]
        );
    }
    eprintln!(
        "10,000 Prolog -> Rust -> Prolog round trips in {:?}",
        t0.elapsed()
    );
}

/// The count of answers Prolog has computed for the lazy query below.
fn produced(p: &mut Prolog) -> i64 {
    match p
        .all(&c("flag", vec![a("m2_produced"), v(0), v(0)]), 1)
        .unwrap()
        .as_slice()
    {
        [vals] => match vals[0] {
            Term::Int(n) => n,
            ref t => panic!("{t:?}"),
        },
        x => panic!("{x:?}"),
    }
}

#[test]
fn answers_only_on_next() {
    // Plan: "A query with 1,000 answers sends none until NEXT, and exactly n
    // per NEXT n." Counted on both sides: messages received, and answers
    // Prolog computed (a flag the goal bumps per answer).
    let mut p = Prolog::start().unwrap();
    let quiet = Duration::from_millis(300);
    let goal = and(
        c("between", vec![i(1), i(1000), v(0)]),
        c(
            "flag",
            vec![a("m2_produced"), v(1), c("+", vec![v(1), i(1)])],
        ),
    );
    let q = p.query(&goal).unwrap();
    assert!(
        p.receive(quiet).unwrap().is_none(),
        "a message came before NEXT"
    );
    assert_eq!(produced(&mut p), 0, "Prolog computed before NEXT");
    let mut total = 0;
    for n in [1u32, 10, 989] {
        let (ans, end) = p.next(&q, n).unwrap();
        assert_eq!((ans.len(), end), (n as usize, None));
        assert!(
            p.receive(quiet).unwrap().is_none(),
            "more than {n} came for NEXT {n}"
        );
        total += n as i64;
        assert_eq!(produced(&mut p), total);
        assert_eq!(ans.last().unwrap()[0], i(total));
    }
    // All 1,000 are out; the next NEXT finds there are no more.
    let (ans, end) = p.next(&q, 5).unwrap();
    assert_eq!((ans.len(), end), (0, Some(End::Exhausted)));
}

#[test]
fn cancel_drops_the_query() {
    // Catches CANCEL that leaves the query alive, or is not acknowledged.
    let mut p = Prolog::start().unwrap();
    let q = p.query(&c("between", vec![i(1), a("inf"), v(0)])).unwrap();
    assert_eq!(p.next(&q, 3).unwrap().0.len(), 3);
    assert_eq!(p.cancel(&q).unwrap(), End::Cancelled);
    assert!(
        matches!(p.next(&q, 1), Err(Error::Prolog(_))),
        "NEXT after CANCEL found the query"
    );
}

#[test]
fn errors_come_back_and_the_server_carries_on() {
    // Catches an error that kills the server, or is lost; and a goal that
    // prints corrupting the protocol stream.
    let mut p = Prolog::start().unwrap();
    let e = p
        .all(&c("no_such_predicate_m2", vec![v(0)]), 1)
        .unwrap_err();
    assert!(
        matches!(&e, Error::Prolog(Term::Compound(f, _)) if f == "error"),
        "{e:?}"
    );
    // An error term holding a stream cannot be encoded; it comes back as text.
    let e = p
        .all(
            &and(c("current_output", vec![v(0)]), c("throw", vec![v(0)])),
            1,
        )
        .unwrap_err();
    assert!(
        matches!(&e, Error::Prolog(Term::Str(t)) if t.contains("stream")),
        "{e:?}"
    );
    // Printing, to the current output and to user_output by name.
    let noisy = p
        .all(&c("format", vec![s("noise on current output~n")]), 1)
        .unwrap();
    assert_eq!(noisy.len(), 1);
    let noisy = p
        .all(
            &c(
                "format",
                vec![a("user_output"), s("noise on user_output~n"), a("[]")],
            ),
            1,
        )
        .unwrap();
    assert_eq!(noisy.len(), 1);
    assert_eq!(
        p.all(&c("=", vec![v(0), i(42)]), 1).unwrap(),
        vec![vec![i(42)]]
    );
}

#[test]
fn facts_assert_and_retract() {
    // Catches a fact asserted without its seq, retracted by the wrong seq,
    // or invisible to queries.
    let mut p = Prolog::start().unwrap();
    let fact = c("date", vec![a("buran"), i(1988)]);
    p.assert_fact(&fact, 7).unwrap();
    assert_eq!(
        p.all(&c("date", vec![a("buran"), v(0)]), 5).unwrap(),
        vec![vec![i(1988)]]
    );
    assert!(
        p.retract_fact(&fact, 8).is_err(),
        "retracted by the wrong seq"
    );
    p.retract_fact(&fact, 7).unwrap();
    assert!(
        p.all(&c("date", vec![a("buran"), v(0)]), 5)
            .unwrap()
            .is_empty()
    );
    assert!(
        p.assert_fact(&c("atom", vec![a("x")]), 9).is_err(),
        "asserted over a built-in"
    );
}

#[test]
fn wrong_requests_get_an_error_and_the_server_carries_on() {
    // Catches a well-framed but wrong request killing the server, or passing
    // without an error.
    let mut p = Prolog::start().unwrap();
    p.send(Kind::Answer, 900, vec![]).unwrap(); // a reply type sent as a request
    p.send(Kind::Next, 901, vec![0xffff_ffff]).unwrap(); // an unknown tag in the body
    let mut errors = 0;
    while let Some(m) = p.receive(Duration::from_millis(500)).unwrap() {
        assert!(m.kind == Kind::Done || m.kind == Kind::AtomIs, "{m:?}");
        if m.kind == Kind::Done {
            errors += 1;
        }
    }
    assert_eq!(errors, 2);
    assert_eq!(p.all(&a("true"), 1).unwrap().len(), 1);
}

/// Run the server as a bare process, feed it bytes, close its input, and
/// return its exit code and what it wrote to stderr.
fn server_given(bytes: &[u8]) -> (Option<i32>, String) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let server = concat!(env!("CARGO_MANIFEST_DIR"), "/../../prolog/server.pl");
    let mut child = Command::new("swipl")
        .args(["-q", server])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn the_server_dies_on_a_garbled_header() {
    // The author's rule: a garbled header is fatal. Catches the server
    // carrying on, or stopping as if all were well (exit 0).
    for (what, bytes) in [
        ("reserved field not zero", vec![7, 1, 0, 1, 0, 0, 0, 0]),
        ("unknown type", vec![99, 0, 0, 1, 0, 0, 0, 0]),
        ("no request number", vec![1, 0, 0, 0]),
        ("cut short", vec![1, 0, 0, 3, 0, 0, 0, 1]),
        ("header cut short", vec![1, 0]),
    ] {
        let (code, err) = server_given(&bytes);
        assert_eq!(code, Some(1), "{what}: {err}");
        assert!(err.contains("dying on a garbled message"), "{what}: {err}");
    }
    let (code, err) = server_given(&[]);
    assert_eq!(code, Some(0), "a clean end of input is not an error: {err}");
}

#[test]
fn bytes_sent_as_they_are_reach_the_server_at_once() {
    // Catches bytes held in the client's buffer: with nothing sent after
    // them, the garbled header must still reach the server and kill it.
    let mut p = Prolog::start().unwrap();
    p.send_bytes(&[7, 1, 0, 1, 0, 0, 0, 0]).unwrap();
    let e = p.receive(Duration::from_secs(10)).unwrap_err();
    assert!(matches!(&e, Error::Dead(_)), "{e:?}");
}

#[test]
fn a_dead_servers_reason_says_how_it_ended() {
    // Catches the server's exit status lost from the reason given.
    let mut p = Prolog::start().unwrap();
    p.send_bytes(&[7, 1, 0, 1, 0, 0, 0, 0]).unwrap();
    let e = p.all(&a("true"), 1).unwrap_err();
    assert!(e.to_string().contains("(exit status: "), "{e}");
}

#[test]
fn the_client_dies_with_the_server() {
    // Catches a client that hangs, or keeps calling a dead server, after a
    // garbled header kills it.
    let mut p = Prolog::start().unwrap();
    p.send_bytes(&[7, 1, 0, 1, 0, 0, 0, 0]).unwrap();
    let t = Instant::now();
    let e = p.all(&a("true"), 1).unwrap_err();
    assert!(
        matches!(&e, Error::Dead(r) if r.contains("stopped")),
        "{e:?}"
    );
    assert!(t.elapsed() < Duration::from_secs(10));
    let again = p.all(&a("true"), 1).unwrap_err();
    assert_eq!(
        again.to_string(),
        e.to_string(),
        "a later call must fail with the same reason"
    );
    assert!(matches!(p.send_bytes(&[0]), Err(Error::Dead(_))));
}

#[test]
fn the_client_dies_on_a_garbled_reply() {
    // Catches a client that reads on past a garbled message from the server.
    let dir = std::env::temp_dir().join(format!("adversaria-m2-garbled-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fake = dir.join("server.pl");
    // A named main: the same goal given inline to initialization/2 did not run (not explained).
    std::fs::write(
        &fake,
        ":- initialization(main, main).\nmain :- format(\"garbage!\"), flush_output, read(_).\n",
    )
    .unwrap();
    let mut p = Prolog::start_with(&fake).unwrap();
    let e = p.all(&a("true"), 1).unwrap_err();
    assert!(
        matches!(&e, Error::Dead(r) if r.contains("garbled message from it")),
        "{e:?}"
    );
    assert!(matches!(p.all(&a("true"), 1), Err(Error::Dead(_))));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_store_replays_into_prolog_the_same_twice() {
    // Plan, milestone 2 item 1: "facts as replayed from the journal"; and the
    // milestone 1 check, now with real facts: replaying twice gives the same
    // facts in the same order. Catches facts from another branch, a
    // retraction not applied, or the order changed on loading.
    use memex::journal::Journal;
    use serde_json::json;
    let dir = std::env::temp_dir().join(format!("adversaria-m2-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut j = Journal::open(&dir).unwrap();
    j.durable = false;
    let fact = |e: &str, y: i64| format::terms::encode(&c("date", vec![a(e), i(y)])).unwrap();
    j.append(
        "edit",
        json!({"facts": [fact("buran", 1988), fact("energia", 1987)]}),
    )
    .unwrap();
    j.append("import", json!({"facts": [fact("talk", 1989)]}))
        .unwrap();
    j.append_after(
        Some(1),
        "suppose",
        json!({"facts": [fact("other_branch", 1)]}),
    )
    .unwrap();
    j.append_after(Some(2), "retract", json!({"target": 2}))
        .unwrap();
    j.append("adopt", json!({"facts": [fact("buran_landing", 1988)]}))
        .unwrap();
    let head = j.last().unwrap().seq;
    let facts = memex::store::facts(j.entries(), head).unwrap();
    let listed: Vec<Vec<Term>> = facts
        .iter()
        .map(|(s, f)| vec![i(*s as i64), f.clone()])
        .collect();
    let mut runs = Vec::new();
    for _ in 0..2 {
        let mut p = Prolog::start().unwrap();
        memex::store::load(&mut p, &facts).unwrap();
        runs.push(p.all(&c("memex_fact", vec![v(0), v(1)]), 10).unwrap());
        assert_eq!(
            p.all(&c("date", vec![a("buran"), v(0)]), 5).unwrap(),
            vec![vec![i(1988)]]
        );
        assert!(
            p.all(&c("date", vec![a("talk"), v(0)]), 5)
                .unwrap()
                .is_empty(),
            "retracted fact present"
        );
        assert!(
            p.all(&c("date", vec![a("other_branch"), v(0)]), 5)
                .unwrap()
                .is_empty(),
            "other branch present"
        );
    }
    assert_eq!(runs[0], listed);
    assert_eq!(runs[0], runs[1]);
    assert_eq!(listed.len(), 3);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A client of a server that sends these bytes at once, whatever it is asked.
fn client_of_a_server_that_says(name: &str, bytes: &[u8]) -> Prolog {
    let dir = std::env::temp_dir().join(format!("adversaria-m2-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let program = format!(
        ":- initialization(main, main).\nmain :- set_stream(user_output, type(binary)), forall(member(B, {bytes:?}), put_byte(B)), flush_output, read(_).\n"
    );
    std::fs::write(dir.join("server.pl"), program).unwrap();
    Prolog::start_with(&dir.join("server.pl")).unwrap()
}

#[test]
fn an_answer_of_the_wrong_width_is_refused() {
    // Catches an answer taken with more or fewer values than the query has
    // variables: here ANSWER for request 1 with two integers, for one variable.
    let answer = [7, 0, 0, 3, 0, 0, 0, 1, 32, 0, 0, 1, 32, 0, 0, 1];
    let mut p = client_of_a_server_that_says("width", &answer);
    let q = p.query(&Term::Var(0)).unwrap();
    let refused = p.next(&q, 1).unwrap_err().to_string();
    assert_eq!(refused, "answer has 2 values for 1 variables");
}

#[test]
fn a_wrong_reply_to_an_atom_request_is_refused() {
    // Catches a client that waits on, or goes on, when the server answers
    // ATOM with something other than ATOM_IS: here DONE for request 1.
    let mut p = client_of_a_server_that_says("atom", &[8, 0, 0, 1, 0, 0, 0, 1]);
    let refused = p
        .intern(&[a("unknown_to_the_server")])
        .unwrap_err()
        .to_string();
    assert_eq!(refused, "ATOM got Done");
}
