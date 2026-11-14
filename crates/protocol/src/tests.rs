use super::*;

/// SplitMix64, so generated terms repeat exactly.
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// One of a list, at random.
    pub fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }
}

pub const NAMES: [&str; 10] = [
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

/// A random term: atoms, integers small and large, floats, strings,
/// variables, lists and nested compounds.
pub fn gen_term(rng: &mut Rng, depth: u32) -> Term {
    match rng.below(if depth >= 4 { 7 } else { 9 }) {
        0 => Term::Atom(rng.pick(&NAMES).into()),
        1 => Term::Int(rng.pick(&INTS)),
        2 => Term::Int(rng.below(2_000_001) as i64 - 1_000_000),
        3 => Term::Float(gen_float(rng)),
        4 => Term::Str(gen_text(rng)),
        5 => Term::Var(rng.below(5)),
        6 => Term::Str(rng.pick(&["", "a\nb", "😀"]).into()),
        7 => Term::Compound(rng.pick(&NAMES).into(), gen_args(rng, depth)),
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

/// One to four terms, one level deeper.
fn gen_args(rng: &mut Rng, depth: u32) -> Vec<Term> {
    let n = 1 + rng.below(4);
    (0..n).map(|_| gen_term(rng, depth + 1)).collect()
}

/// A list of up to three terms, one level deeper, built from its end.
fn gen_list(rng: &mut Rng, depth: u32) -> Term {
    let n = rng.below(4);
    (0..n).fold(Term::Atom("[]".into()), |tail, _| {
        Term::Compound(".".into(), vec![gen_term(rng, depth + 1), tail])
    })
}

pub fn atoms_for_tests() -> Atoms {
    let mut a = Atoms::default();
    for (i, n) in NAMES.iter().enumerate() {
        if a.number(n).is_none() {
            a.learn(100 + i as u32, n);
        }
    }
    a
}

#[test]
fn ten_thousand_round_trips() {
    // Catches any term that does not come back as itself (up to renaming
    // its variables, which the protocol numbers by first appearance).
    let atoms = atoms_for_tests();
    let mut rng = Rng(7);
    for i in 0..10_000 {
        let t = gen_term(&mut rng, 0);
        let (cells, _) = encode(std::slice::from_ref(&t), &atoms).unwrap();
        let back = decode(&cells, &atoms).unwrap();
        assert_eq!(back.len(), 1);
        assert!(
            variant(&back[0], &t),
            "term {i}: {t:?} came back as {:?}",
            back[0]
        );
    }
}

#[test]
fn shared_variables_and_their_order() {
    // Catches one variable sent as two, or the answer order lost.
    let atoms = atoms_for_tests();
    let t = Term::Compound("f".into(), vec![Term::Var(9), Term::Var(4), Term::Var(9)]);
    let (cells, vars) = encode(&[t, Term::Var(4)], &atoms).unwrap();
    assert_eq!(vars, vec![9, 4]);
    let back = decode(&cells, &atoms).unwrap();
    assert_eq!(
        back[0],
        Term::Compound("f".into(), vec![Term::Var(0), Term::Var(1), Term::Var(0)])
    );
    assert_eq!(back[1], Term::Var(1));
}

#[test]
fn int_boundaries() {
    // Catches the 29-bit INT range off by one: its edges stay INT, beyond is BLOB.
    let a = Atoms::default();
    for (i, tag) in [
        (268435455, INT),
        (-268435456, INT),
        (268435456, BLOB),
        (-268435457, BLOB),
    ] {
        let (c, _) = encode(&[Term::Int(i)], &a).unwrap();
        assert_eq!(c[0] >> 29, tag, "{i}");
        assert_eq!(decode(&c, &a).unwrap(), vec![Term::Int(i)]);
    }
}

#[test]
fn unknown_atoms_are_listed_not_guessed() {
    // Catches an atom encoded before the server has numbered it.
    let a = Atoms::default();
    let t = Term::Compound(
        "p".into(),
        vec![
            Term::Atom("q".into()),
            Term::Atom("[]".into()),
            Term::Atom("p".into()),
        ],
    );
    assert_eq!(
        a.missing(std::slice::from_ref(&t)),
        vec!["q".to_string(), "p".to_string()]
    );
    assert!(encode(&[t], &a).is_err());
}

#[test]
fn message_round_trip() {
    // Catches a framing error: header fields, byte order or length.
    let m = Message {
        kind: Kind::Answer,
        req: 4_000_000_000,
        body: vec![1, 0xdeadbeef],
    };
    let mut bytes = Vec::new();
    write_message(&mut bytes, &m).unwrap();
    assert_eq!(&bytes[..4], &[7, 0, 0, 3]);
    assert_eq!(read_message(&mut bytes.as_slice()).unwrap(), Some(m));
    assert_eq!(read_message(&mut [].as_slice()).unwrap(), None);
}

#[test]
fn malformed_messages_refused() {
    // Catches a bad header, or a message cut short, accepted.
    for bytes in [
        vec![7, 1, 0, 1, 0, 0, 0, 0],
        vec![99, 0, 0, 1, 0, 0, 0, 0],
        vec![7, 0, 0, 0],
        vec![7, 0, 0, 2, 0, 0, 0, 0],
    ] {
        assert!(read_message(&mut bytes.as_slice()).is_err(), "{bytes:?}");
    }
}

#[test]
fn malformed_cells_refused() {
    // Catches a truncated term, an unknown tag, a short blob, or variables
    // that skip a number, accepted.
    let a = atoms_for_tests();
    let f = (STRUCT << 29) | (1 << 21) | 108; // f/1 with no argument
    for cells in [
        vec![f],
        vec![0x6000_0001],
        vec![(BLOB << 29) | 5, 0x7300_0000],
        vec![1],
        vec![(BLOB << 29) | 2, 0x7841_0000],
    ] {
        assert!(decode(&cells, &a).is_err(), "{cells:x?}");
    }
}

#[test]
fn a_long_list_decodes_without_deep_recursion() {
    // Catches a decoder that recurses once per list element: a long list
    // from Prolog would overflow the stack (found when a few hundred
    // suggestions overflowed a test thread). The stack here is small, so a
    // recursive decoder fails.
    let n = 200_000;
    let mut cells = Vec::with_capacity(2 * n + 1);
    for i in 0..n {
        cells.push(cell(STRUCT, (2 << 21) | 1));
        cells.push(int_cell(i as i64 % 1000).unwrap());
    }
    cells.push(cell(ATOM, 0));
    let len = on_a_stack_of(256 * 1024, move || {
        let t = decode(&cells, &Atoms::default()).unwrap();
        let len = list_length(&t[0]);
        std::mem::forget(t); // dropping a long list recurses too (a known limit)
        len
    });
    assert_eq!(len, n);
}

/// The number of cells along a list's spine.
fn list_length(list: &Term) -> usize {
    let mut count = 0;
    let mut at = list;
    while let Term::Compound(_, a) = at {
        count += 1;
        at = &a[1];
    }
    count
}

/// Cells for f(f(...f([])...)), `depth` compounds deep.
fn nested(depth: usize, a: &Atoms) -> Vec<u32> {
    let f = a.number("f").unwrap();
    let mut cells = vec![cell(STRUCT, (1 << 21) | f); depth];
    cells.push(cell(ATOM, 0));
    cells
}

/// What a function gives, run in a thread with a stack of this many bytes,
/// so that one that recurses too deep fails here, not by chance elsewhere.
fn on_a_stack_of<T: Send + 'static>(bytes: usize, f: impl FnOnce() -> T + Send + 'static) -> T {
    let thread = std::thread::Builder::new().stack_size(bytes);
    thread.spawn(f).unwrap().join().unwrap()
}

/// The error of decoding a term nested this deep, on a stack of 2 MiB.
fn nested_refusal(depth: usize) -> String {
    let a = atoms_for_tests();
    on_a_stack_of(2 << 20, move || {
        decode(&nested(depth, &a), &a).unwrap_err().0
    })
}

// The next four catch a decoder that recurses until the stack runs out on a
// deeply nested term from Prolog (found: depth 1,000 aborted a test thread),
// and a limit that also counts a list's spine, so long lists fail.

#[test]
fn nesting_to_the_limit_is_allowed() {
    let a = atoms_for_tests();
    let ok = on_a_stack_of(2 << 20, move || decode(&nested(MAX_DEPTH, &a), &a).is_ok());
    assert!(ok, "depth {MAX_DEPTH} is allowed");
}

#[test]
fn nesting_one_past_the_limit_is_refused() {
    let r = nested_refusal(MAX_DEPTH + 1);
    assert!(r.contains("nested deeper than 256"), "{r}");
}

#[test]
fn nesting_a_million_deep_is_refused_without_recursing_to_overflow() {
    let r = nested_refusal(1_000_000);
    assert!(r.contains("nested deeper than 256"), "{r}");
}

#[test]
fn a_long_list_of_short_lists_decodes() {
    // A list of 10,000 lists of one element: the spine is not counted.
    let mut cells = Vec::new();
    for _ in 0..10_000 {
        cells.push(cell(STRUCT, (2 << 21) | 1));
        cells.extend([cell(STRUCT, (2 << 21) | 1), cell(INT, 7), cell(ATOM, 0)]);
    }
    cells.push(cell(ATOM, 0));
    let a = atoms_for_tests();
    let decodes = on_a_stack_of(2 << 20, move || {
        let t = decode(&cells, &a);
        let ok = t.is_ok();
        std::mem::forget(t); // a long list's drop recurses (a known limit)
        ok
    });
    assert!(decodes, "a long list of short lists decodes");
}

#[test]
fn a_blob_whose_padding_is_not_zero_is_refused() {
    // Catches bytes hidden after a blob's text: "A" as a string, then a 1.
    let cells = [(BLOB << 29) | 2, 0x7341_0001];
    let refused = decode(&cells, &atoms_for_tests()).unwrap_err().0;
    assert_eq!(refused, "blob is empty or its padding is not zero");
}

#[test]
fn a_lists_tail_is_no_deeper_than_the_list() {
    // Catches the tail counted a level below its list: after one element,
    // a tail nested to the limit must still decode.
    let a = atoms_for_tests();
    let mut cells = vec![CONS, cell(INT, 7)];
    cells.extend(nested(MAX_DEPTH, &a));
    let decoded = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || decode(&cells, &a).is_ok());
    assert!(decoded.unwrap().join().unwrap());
}

#[test]
fn a_renaming_must_hold_both_ways() {
    // Catches two variables renamed to one: f(X, Y) is no variant of f(Z, Z).
    let f = |x, y| Term::Compound("f".into(), vec![Term::Var(x), Term::Var(y)]);
    let pairs = [(f(0, 1), f(5, 5)), (f(5, 5), f(0, 1)), (f(0, 1), f(8, 9))];
    let variants = pairs.map(|(a, b)| variant(&a, &b));
    assert_eq!(variants, [false, false, true]);
}

#[test]
fn compounds_of_two_names_are_not_variants() {
    // Catches compounds compared by their arguments alone: f(X) is no
    // variant of g(X).
    let one = |name: &str| Term::Compound(name.into(), vec![Term::Var(0)]);
    assert!(!variant(&one("f"), &one("g")));
}

#[test]
fn a_message_longer_than_its_header_can_count_is_refused() {
    // Catches a body of 65,535 words written with a length that wraps.
    let message = |words| Message {
        kind: Kind::Query,
        req: 1,
        body: vec![0; words],
    };
    let written = |words| write_message(&mut Vec::new(), &message(words)).is_ok();
    assert_eq!((written(0xfffe), written(0xffff)), (true, false));
}
