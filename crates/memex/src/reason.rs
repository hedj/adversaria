//! Results of reasoning over a world, and the pure helpers around them:
//! violations and disagreements, readings of a supposition, restatement in
//! English, and derived text (docs/design.md § Claims and beliefs, § Dataflow).

use crate::engine::Engine;
use crate::outline::Constraint;
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
    /// The fact each provenance rests on, in the same order.
    pub facts: Vec<Term>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintEval {
    pub seq: u64,
    /// Its name and rule, as the outline keeps them.
    pub constraint: Constraint,
    /// Resting on beliefs alone: broken facts.
    pub violations: Vec<Solution>,
    /// Resting on a claim: sources that disagree.
    pub disagreements: Vec<Solution>,
}

impl ConstraintEval {
    /// Read a constraint's node from an engine the world is loaded in.
    pub fn read(engine: &mut Engine, seq: u64, c: &Constraint) -> Result<Self, String> {
        let v = engine.node(seq).map_err(|e| e.to_string())?;
        let (violations, disagreements) = split_solutions(&v);
        Ok(Self {
            seq,
            constraint: c.clone(),
            violations,
            disagreements,
        })
    }
}

/// A constraint node's solutions: those resting on beliefs alone
/// (violations), and those resting on a claim (disagreements).
pub fn split_solutions(v: &Term) -> (Vec<Solution>, Vec<Solution>) {
    solutions(v).into_iter().partition(rests_on_beliefs)
}

fn rests_on_beliefs(s: &Solution) -> bool {
    s.prov.iter().all(|p| matches!(p, Prov::Belief(_)))
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldEval {
    pub facts: Vec<(u64, Term)>,
    pub constraints: Vec<ConstraintEval>,
    /// What the other nodes found: derived text and layers.
    pub nodes: Nodes,
    /// The journal's length when this was computed.
    pub at: usize,
}

/// What derived items and layers found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Nodes {
    /// Each derived item's answers.
    pub derived: BTreeMap<u64, Term>,
    /// Each layer's marks, by the layer's seq: the mark, and the seqs of
    /// the facts it rests on.
    pub layers: BTreeMap<u64, Vec<(Term, Vec<u64>)>>,
}

impl WorldEval {
    pub fn fact(&self, seq: u64) -> Option<&Term> {
        self.facts.iter().find(|(s, _)| *s == seq).map(|(_, t)| t)
    }
}

/// A suggestion without a model: what it proposes, the facts adopting it
/// adds, its evidence, and a score by which suggestions are ranked.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// link, mapping, constraint(Name, Vars, Body), rule(Head, Body) or
    /// reference(Entity, Work).
    pub kind: Term,
    pub facts: Vec<Term>,
    pub evidence: String,
    pub score: i64,
}

impl Suggestion {
    pub fn from_term(t: &Term) -> Option<Suggestion> {
        // Variables are numbered within the whole answer; number them from 0
        // within this suggestion, so its key does not depend on its place.
        let mut seen: Vec<u64> = Vec::new();
        fn renumber(t: &Term, seen: &mut Vec<u64>) -> Term {
            match t {
                Term::Var(v) => {
                    let i = seen.iter().position(|x| x == v).unwrap_or_else(|| {
                        seen.push(*v);
                        seen.len() - 1
                    });
                    Term::Var(i as u64)
                }
                Term::Compound(f, a) => {
                    Term::Compound(f.clone(), a.iter().map(|x| renumber(x, seen)).collect())
                }
                t => t.clone(),
            }
        }
        let t = &renumber(t, &mut seen);
        match t {
            Term::Compound(f, a) if f == "suggestion" && a.len() == 4 => Some(Suggestion {
                kind: a[0].clone(),
                facts: list(&a[1]),
                evidence: match &a[2] {
                    Term::Str(s) => s.clone(),
                    t => show(t),
                },
                score: match &a[3] {
                    Term::Int(n) => *n,
                    _ => 0,
                },
            }),
            _ => None,
        }
    }

    /// Known by what it proposes and its evidence: a dismissed suggestion
    /// comes back only if its evidence changes.
    pub fn key(&self) -> String {
        format!(
            "{} | {} | {}",
            show(&self.kind),
            self.facts.iter().map(show).collect::<Vec<_>>().join(", "),
            self.evidence
        )
    }

    /// What it proposes, without its evidence: its place in a frozen list
    /// holds while its evidence grows.
    pub fn identity(&self) -> String {
        format!(
            "{} | {}",
            show(&self.kind),
            self.facts.iter().map(show).collect::<Vec<_>>().join(", ")
        )
    }

    pub fn kind_name(&self) -> &str {
        match &self.kind {
            Term::Atom(a) => a,
            Term::Compound(f, _) => f,
            _ => "?",
        }
    }

    /// The suggestion in words.
    pub fn describe(&self) -> String {
        let arg = |t: &Term, i: usize| match t {
            Term::Compound(_, x) => x.get(i).map(show).unwrap_or_default(),
            _ => String::new(),
        };
        let fact = self
            .facts
            .first()
            .cloned()
            .unwrap_or(Term::Atom("[]".into()));
        match (self.kind_name(), &self.kind) {
            ("link", _) => format!("{} is Wikidata {}", arg(&fact, 0), arg(&fact, 1)),
            ("mapping", _) => format!("{} is Wikidata's {}", arg(&fact, 0), arg(&fact, 1)),
            ("constraint", Term::Compound(_, x)) if x.len() == 3 => {
                let name = match &x[0] {
                    Term::Str(n) | Term::Atom(n) => n.clone(),
                    t => show(t),
                };
                format!("constraint {name}: false :- {}", show(&x[2]))
            }
            ("rule", _) => format!("rule {} :- {}", arg(&self.kind, 0), arg(&self.kind, 1)),
            ("reference", _) => format!(
                "cite {} for the claims about {}",
                arg(&self.kind, 1),
                arg(&self.kind, 0)
            ),
            _ => show(&self.kind),
        }
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
/// A layer's value, as (mark, seqs of the facts it rests on) pairs.
pub fn marks(v: &Term) -> Vec<(Term, Vec<u64>)> {
    list(v)
        .into_iter()
        .filter_map(|p| match p {
            Term::Compound(f, a) if f == "-" && a.len() == 2 => Some((
                a[0].clone(),
                list(&a[1])
                    .into_iter()
                    .filter_map(|s| match s {
                        Term::Int(n) if n > 0 => Some(n as u64),
                        _ => None,
                    })
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

pub fn solutions(v: &Term) -> Vec<Solution> {
    let mut out: Vec<Solution> = Vec::new();
    for pair in list(v) {
        let Term::Compound(f, a) = &pair else {
            continue;
        };
        if f != "-" || a.len() != 2 {
            continue;
        }
        // belief(Seq, Fact) or claim(Source, Seq, Fact).
        let parsed: Vec<(Prov, Term)> = list(&a[1])
            .iter()
            .filter_map(|p| match p {
                Term::Compound(f, x) if f == "belief" && x.len() == 2 => match &x[0] {
                    Term::Int(s) => Some((Prov::Belief(*s as u64), x[1].clone())),
                    _ => None,
                },
                Term::Compound(f, x) if f == "claim" && x.len() == 3 => match (&x[0], &x[1]) {
                    (Term::Str(src) | Term::Atom(src), Term::Int(s)) => {
                        Some((Prov::Claim(src.clone(), *s as u64), x[2].clone()))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let prov: Vec<Prov> = parsed.iter().map(|p| p.0.clone()).collect();
        let facts: Vec<Term> = parsed.into_iter().map(|p| p.1).collect();
        // A solution is known by the facts it rests on.
        let key = |prov: &[Prov], facts: &[Term]| {
            let mut k: Vec<String> = prov
                .iter()
                .zip(facts)
                .map(|(p, f)| format!("{}:{}", p.seq(), show(f)))
                .collect();
            k.sort();
            k.dedup();
            k
        };
        let k = key(&prov, &facts);
        if k.len() < 2 && prov.len() >= 2 {
            continue; // one fact matched twice: not two facts disagreeing
        }
        if out.iter().any(|s| key(&s.prov, &s.facts) == k) {
            continue;
        }
        out.push(Solution {
            values: list(&a[0]),
            prov,
            facts,
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
        Term::Compound(f, a) if f == "," && a.len() == 2 => {
            format!("{}, {}", show(&a[0]), show(&a[1]))
        }
        Term::Compound(f, a)
            if a.len() == 2
                && [
                    "\\=", "=", "is", "<", ">", "=<", ">=", "+", "-", "*", "/", ":-",
                ]
                .contains(&f.as_str()) =>
        {
            format!("{} {f} {}", show(&a[0]), show(&a[1]))
        }
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
        let b = |s| Term::Compound("belief".into(), vec![Term::Int(s), date("x", 1)]);
        let c = |s| {
            Term::Compound(
                "claim".into(),
                vec![Term::Str("talk".into()), Term::Int(s), date("x", 2)],
            )
        };
        let v = make_list(vec![p(b(2), c(4)), p(c(4), b(2)), p(b(2), b(2))]);
        let s = solutions(&v);
        assert_eq!(s.len(), 1);
        assert_eq!(
            s[0].prov,
            vec![Prov::Belief(2), Prov::Claim("talk".into(), 4)]
        );
    }
}
