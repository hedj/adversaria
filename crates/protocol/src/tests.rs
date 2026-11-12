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
