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

/// A random term: atoms, integers small and large, floats, strings,
/// variables, lists and nested compounds.
pub fn gen_term(rng: &mut Rng, depth: u32) -> Term {
    let pick = |rng: &mut Rng, n: usize| rng.below(n as u64) as usize;
    match rng.below(if depth >= 4 { 7 } else { 9 }) {
        0 => Term::Atom(NAMES[pick(rng, NAMES.len())].into()),
        1 => Term::Int(
            [
                0,
                1,
                -1,
                268435455,
                -268435456,
                268435456,
                -268435457,
                i64::MAX,
                i64::MIN,
            ][pick(rng, 9)],
        ),
        2 => Term::Int(rng.below(2_000_001) as i64 - 1_000_000),
        3 => loop {
            let x = f64::from_bits(rng.next());
            if x.is_finite() {
                break Term::Float([x, 0.0, -0.0, 1.0, 0.1, 1e23, 5e-324][pick(rng, 7)]);
            }
        },
        4 => Term::Str(
            (0..rng.below(7))
                .filter_map(|_| char::from_u32(1 + rng.below(0x2fff) as u32))
                .collect(),
        ),
        5 => Term::Var(rng.below(5)),
        6 => Term::Str(["", "a\nb", "😀"][pick(rng, 3)].into()),
        7 => Term::Compound(
            NAMES[pick(rng, NAMES.len())].into(),
            (0..1 + rng.below(4))
                .map(|_| gen_term(rng, depth + 1))
                .collect(),
        ),
        _ => {
            let n = rng.below(4);
            (0..n).fold(Term::Atom("[]".into()), |tail, _| {
                Term::Compound(".".into(), vec![gen_term(rng, depth + 1), tail])
            })
        }
    }
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
    // suggestions overflowed a test thread). Run in a thread with a small
    // stack so a recursive decoder fails here, not by chance elsewhere.
    let atoms = Atoms::default();
    let n = 200_000;
    let mut cells = Vec::with_capacity(2 * n + 1);
    for i in 0..n {
        cells.push(cell(STRUCT, (2 << 21) | 1));
        cells.push(int_cell(i as i64 % 1000).unwrap());
    }
    cells.push(cell(ATOM, 0));
    let len = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let t = decode(&cells, &atoms).unwrap();
            let mut count = 0;
            let mut at = &t[0];
            while let Term::Compound(_, a) = at {
                count += 1;
                at = &a[1];
            }
            std::mem::forget(t); // dropping a long list recurses too (a known limit)
            count
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(len, n);
}

/// Cells for f(f(...f([])...)), `depth` compounds deep.
fn nested(depth: usize, a: &Atoms) -> Vec<u32> {
    let f = a.number("f").unwrap();
    let mut cells = vec![cell(STRUCT, (1 << 21) | f); depth];
    cells.push(cell(ATOM, 0));
    cells
}

#[test]
fn nesting_is_limited_and_refused_without_recursing_to_overflow() {
    // Catches a decoder that recurses until the stack runs out on a deeply
    // nested term from Prolog (found: depth 1,000 aborted a test thread),
    // and a limit that also counts a list's spine, so long lists fail.
    let a = atoms_for_tests();
    let r = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let ok = decode(&nested(MAX_DEPTH, &a), &a).is_ok();
            let over = decode(&nested(MAX_DEPTH + 1, &a), &a).unwrap_err().0;
            let far = decode(&nested(1_000_000, &a), &a).unwrap_err().0;
            // A list of 10,000 lists of one element: spine not counted.
            let mut cells = Vec::new();
            for _ in 0..10_000 {
                cells.push(cell(STRUCT, (2 << 21) | 1));
                cells.extend([cell(STRUCT, (2 << 21) | 1), cell(INT, 7), cell(ATOM, 0)]);
            }
            cells.push(cell(ATOM, 0));
            let t = decode(&cells, &a);
            let lists = t.is_ok();
            std::mem::forget(t); // a long list's drop recurses (a known limit)
            (ok, over, far, lists)
        })
        .unwrap()
        .join()
        .unwrap();
    assert!(r.0, "depth {MAX_DEPTH} is allowed");
    assert!(r.1.contains("nested deeper than 256"), "{}", r.1);
    assert!(r.2.contains("nested deeper than 256"), "{}", r.2);
    assert!(r.3, "a long list of short lists decodes");
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
