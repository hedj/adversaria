//! Results of reasoning over a world, and the pure helpers around them:
//! violations and disagreements, readings of a supposition, restatement in
//! English, and derived text (docs/design.md § Claims and beliefs, § Dataflow).

use format::terms::Term;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Prov {
    Belief(u64),
    Claim(String, u64),
}

impl Prov {
    pub fn seq(&self) -> u64 {
        match self {
            Prov::Belief(s) | Prov::Claim(_, s) => *s,
        }
    }
}

/// One way a constraint's body holds: the values of its variables, and the
/// facts it rests on.
#[derive(Debug, Clone, PartialEq)]
pub struct Solution {
    pub values: Vec<Term>,
    pub prov: Vec<Prov>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintEval {
    pub seq: u64,
    pub name: String,
    pub rule: String,
    /// Resting on beliefs alone: broken facts.
    pub violations: Vec<Solution>,
    /// Resting on a claim: sources that disagree.
    pub disagreements: Vec<Solution>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldEval {
    pub facts: Vec<(u64, Term)>,
    pub constraints: Vec<ConstraintEval>,
    /// Each derived item's answers.
    pub derived: BTreeMap<u64, Term>,
    /// The journal's length when this was computed.
    pub at: usize,
}

impl WorldEval {
    pub fn fact(&self, seq: u64) -> Option<&Term> {
        self.facts.iter().find(|(s, _)| *s == seq).map(|(_, t)| t)
    }
}

/// A Prolog list as a vector.
pub fn list(t: &Term) -> Vec<Term> {
    let mut out = Vec::new();
    let mut at = t;
    while let Term::Compound(f, a) = at {
        if f != "." || a.len() != 2 {
            break;
        }
        out.push(a[0].clone());
        at = &a[1];
    }
    out
}

pub fn make_list(items: Vec<Term>) -> Term {
    items
        .into_iter()
        .rev()
        .fold(Term::Atom("[]".into()), |t, h| {
            Term::Compound(".".into(), vec![h, t])
        })
}

/// A constraint node's value: Vars-Prov pairs. Solutions that rest on the
/// same facts (the same pair found both ways round) count once.
pub fn solutions(v: &Term) -> Vec<Solution> {
    let mut out: Vec<Solution> = Vec::new();
    for pair in list(v) {
        let Term::Compound(f, a) = &pair else {
            continue;
        };
        if f != "-" || a.len() != 2 {
            continue;
        }
        let prov: Vec<Prov> = list(&a[1])
            .iter()
            .filter_map(|p| match p {
                Term::Compound(f, x) if f == "belief" => match &x[0] {
                    Term::Int(s) => Some(Prov::Belief(*s as u64)),
                    _ => None,
                },
                Term::Compound(f, x) if f == "claim" => match (&x[0], &x[1]) {
                    (Term::Str(src), Term::Int(s)) => Some(Prov::Claim(src.clone(), *s as u64)),
                    (Term::Atom(src), Term::Int(s)) => Some(Prov::Claim(src.clone(), *s as u64)),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let mut key: Vec<u64> = prov.iter().map(Prov::seq).collect();
        key.sort();
        key.dedup();
        if key.len() < 2 && prov.len() >= 2 {
            continue; // one fact matched twice: not two facts disagreeing
        }
        if out.iter().any(|s| {
            let mut k: Vec<u64> = s.prov.iter().map(Prov::seq).collect();
            k.sort();
            k.dedup();
            k == key
        }) {
            continue;
        }
        out.push(Solution {
            values: list(&a[0]),
            prov,
        });
    }
    out
}

/// A term as Prolog would print it, near enough for reading.
pub fn show(t: &Term) -> String {
    match t {
        Term::Int(i) => i.to_string(),
        Term::Float(x) => x.to_string(),
        Term::Str(s) => format!("\"{s}\""),
        Term::Atom(a) => a.clone(),
        Term::Var(v) => format!("_{v}"),
        Term::Compound(f, _) if f == "." => format!(
            "[{}]",
            list(t).iter().map(show).collect::<Vec<_>>().join(", ")
        ),
        Term::Compound(f, a) => {
            format!("{f}({})", a.iter().map(show).collect::<Vec<_>>().join(", "))
        }
    }
}

/// The label a world gives an entity (`label(E, "text")`), or its own name.
pub fn label(facts: &[(u64, Term)], e: &Term) -> String {
    for (_, f) in facts {
        if let Term::Compound(n, a) = f
            && n == "label"
            && a.len() == 2
            && &a[0] == e
            && let Term::Str(s) = &a[1]
        {
            return s.clone();
        }
    }
    show(e).replace('_', " ")
}

fn is_meta(f: &Term) -> bool {
    matches!(f, Term::Compound(n, _) if n == "claims" || n == "node_def" || n == "label")
}

/// The readings of a supposition typed in English: facts of the world with
/// one number changed to a number in the text, ranked by how many of the
/// text's words name the fact's other arguments. No model and no parser: a
/// "do you mean?" built from the world's own facts, until APE is used.
pub fn readings(facts: &[(u64, Term)], text: &str) -> Vec<Term> {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
        .collect();
    let numbers: Vec<i64> = lower
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|n| n.parse().ok())
        .collect();
    let mut scored: Vec<(usize, Term)> = Vec::new();
    let mut seen: Vec<Term> = Vec::new();
    for (_, f) in facts {
        let f = match f {
            Term::Compound(n, a) if n == "claims" && a.len() == 2 => &a[1],
            f => f,
        };
        if is_meta(f) {
            continue;
        }
        let Term::Compound(name, args) = f else {
            continue;
        };
        for (k, arg) in args.iter().enumerate() {
            let Term::Int(old) = arg else { continue };
            for n in &numbers {
                if n == old {
                    continue;
                }
                let others: String = args
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != k)
                    .map(|(_, a)| format!("{} {}", show(a), label(facts, a)))
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_lowercase()
                    .replace('_', " ");
                let score = words
                    .iter()
                    .filter(|w| others.contains(*w) || name.contains(*w))
                    .count();
                if score == 0 {
                    continue;
                }
                let mut a2 = args.clone();
                a2[k] = Term::Int(*n);
                let t = Term::Compound(name.clone(), a2);
                if !seen.contains(&t) {
                    seen.push(t.clone());
                    scored.push((score, t));
                }
            }
        }
    }
    scored.sort_by_key(|s| std::cmp::Reverse(s.0));
    scored.into_iter().map(|(_, t)| t).collect()
}

/// A fact restated in English, by a generic frame: "the date of Buran's
/// flight is 1989".
pub fn restate(facts: &[(u64, Term)], t: &Term) -> String {
    match t {
        Term::Compound(n, a) if a.len() == 2 => format!(
            "the {} of {} is {}",
            n.replace('_', " "),
            label(facts, &a[0]),
            show(&a[1])
        ),
        Term::Compound(n, a) if a.len() == 1 => {
            format!("{} is {}", label(facts, &a[0]), n.replace('_', " "))
        }
        t => show(t),
    }
}

fn number_word(n: i64) -> String {
    let w = [
        "no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    if (0..=10).contains(&n) {
        w[n as usize].into()
    } else {
        n.to_string()
    }
}

/// A derived item's text: its template with each {Name} filled from the
/// first answer; {Name:years} reads "a year", "two years", ...
pub fn fill(template: &str, names: &[String], answers: &Term, facts: &[(u64, Term)]) -> String {
    let rows = list(answers);
    if template == "timeline" {
        let mut points: Vec<(i64, String)> = rows
            .iter()
            .filter_map(|r| match list(r).as_slice() {
                [e, Term::Int(y)] => Some((*y, label(facts, e))),
                _ => None,
            })
            .collect();
        points.sort();
        return points
            .iter()
            .map(|(y, l)| format!("{y} {l}"))
            .collect::<Vec<_>>()
            .join(" ── ");
    }
    let Some(first) = rows.first().map(list) else {
        return format!("{template} (no answer)");
    };
    let mut out = template.to_string();
    for (i, n) in names.iter().enumerate() {
        let Some(v) = first.get(i) else { continue };
        let years = match v {
            Term::Int(1) => "a year".to_string(),
            Term::Int(k) => format!("{} years", number_word(*k)),
            v => show(v),
        };
        out = out
            .replace(&format!("{{{n}:years}}"), &years)
            .replace(&format!("{{{n}}}"), &show(v));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(e: &str, y: i64) -> Term {
        Term::Compound("date".into(), vec![Term::Atom(e.into()), Term::Int(y)])
    }

    fn lbl(e: &str, s: &str) -> Term {
        Term::Compound(
            "label".into(),
            vec![Term::Atom(e.into()), Term::Str(s.into())],
        )
    }

    #[test]
    fn readings_come_from_the_world() {
        // Catches a reading that changes the wrong fact, or none at all.
        let facts = vec![
            (2, date("buran_flight", 1988)),
            (3, date("energia_launch", 1987)),
            (4, lbl("buran_flight", "Buran's flight")),
        ];
        let r = readings(&facts, "the flight was in 1989");
        assert_eq!(r.first(), Some(&date("buran_flight", 1989)));
        assert_eq!(restate(&facts, &r[0]), "the date of Buran's flight is 1989");
        assert!(
            readings(&facts, "the flight was memorable").is_empty(),
            "no number, no reading"
        );
        assert!(
            readings(&facts, "nothing here 1990").is_empty(),
            "no word names a fact"
        );
    }

    #[test]
    fn templates() {
        // Catches "1 years", or a value not filled in.
        let names = vec!["N".to_string()];
        let one = make_list(vec![make_list(vec![Term::Int(1)])]);
        let two = make_list(vec![make_list(vec![Term::Int(2)])]);
        let t = "The flight came {N:years} after Energia's first launch.";
        assert_eq!(
            fill(t, &names, &one, &[]),
            "The flight came a year after Energia's first launch."
        );
        assert_eq!(
            fill(t, &names, &two, &[]),
            "The flight came two years after Energia's first launch."
        );
        let tl = make_list(vec![
            make_list(vec![Term::Atom("b".into()), Term::Int(1988)]),
            make_list(vec![Term::Atom("a".into()), Term::Int(1987)]),
        ]);
        assert_eq!(
            fill("timeline", &[], &tl, &[(9, lbl("b", "Buran"))]),
            "1987 a ── 1988 Buran"
        );
    }

    #[test]
    fn symmetric_solutions_count_once() {
        // Catches one disagreement counted twice (found both ways round).
        let p = |a: Term, b: Term| {
            Term::Compound("-".into(), vec![make_list(vec![]), make_list(vec![a, b])])
        };
        let b = |s| Term::Compound("belief".into(), vec![Term::Int(s)]);
        let c = |s| Term::Compound("claim".into(), vec![Term::Str("talk".into()), Term::Int(s)]);
        let v = make_list(vec![p(b(2), c(4)), p(c(4), b(2)), p(b(2), b(2))]);
        let s = solutions(&v);
        assert_eq!(s.len(), 1);
        assert_eq!(
            s[0].prov,
            vec![Prov::Belief(2), Prov::Claim("talk".into(), 4)]
        );
    }
}
