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
        let t = renumber(t, &mut Vec::new());
        let a = parts(&t, "suggestion", 4)?;
        let (kind, facts) = (a[0].clone(), list(&a[1]));
        let (evidence, score) = (evidence(&a[2]), score(&a[3]));
        Some(Suggestion {
            kind,
            facts,
            evidence,
            score,
        })
    }

    /// Known by what it proposes and its evidence: a dismissed suggestion
    /// comes back only if its evidence changes.
    pub fn key(&self) -> String {
        format!("{} | {}", self.identity(), self.evidence)
    }

    /// What it proposes, without its evidence: its place in a frozen list
    /// holds while its evidence grows.
    pub fn identity(&self) -> String {
        format!("{} | {}", show(&self.kind), shown(&self.facts))
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
        let fact = self.facts.first().cloned();
        let fact = fact.unwrap_or(Term::Atom("[]".into()));
        match (self.kind_name(), &self.kind) {
            ("link", _) => format!("{} is Wikidata {}", arg(&fact, 0), arg(&fact, 1)),
            ("mapping", _) => format!("{} is Wikidata's {}", arg(&fact, 0), arg(&fact, 1)),
            ("constraint", Term::Compound(_, x)) if x.len() == 3 => constraint_words(&x[0], &x[2]),
            ("rule", kind) => format!("rule {} :- {}", arg(kind, 0), arg(kind, 1)),
            ("reference", kind) => {
                format!(
                    "cite {} for the claims about {}",
                    arg(kind, 1),
                    arg(kind, 0)
                )
            }
            (_, kind) => show(kind),
        }
    }
}

/// A term with its variables numbered from 0 in order of first appearance;
/// `seen` holds the variables met so far.
fn renumber(t: &Term, seen: &mut Vec<u64>) -> Term {
    match t {
        Term::Var(v) => Term::Var(place(seen, *v)),
        Term::Compound(f, a) => Term::Compound(f.clone(), renumber_all(a, seen)),
        t => t.clone(),
    }
}

/// Terms renumbered in order, sharing what has been seen.
fn renumber_all(ts: &[Term], seen: &mut Vec<u64>) -> Vec<Term> {
    ts.iter().map(|t| renumber(t, seen)).collect()
}

/// A variable's place among those seen, where it is put if it is new.
fn place(seen: &mut Vec<u64>, v: u64) -> u64 {
    let known = seen.iter().position(|x| *x == v);
    let at = known.unwrap_or_else(|| {
        seen.push(v);
        seen.len() - 1
    });
    at as u64
}

fn evidence(t: &Term) -> String {
    match t {
        Term::Str(s) => s.clone(),
        t => show(t),
    }
}

fn score(t: &Term) -> i64 {
    match t {
        Term::Int(n) => *n,
        _ => 0,
    }
}

/// An argument of a compound term, shown; nothing if it has none there.
fn arg(t: &Term, i: usize) -> String {
    match t {
        Term::Compound(_, x) => x.get(i).map(show).unwrap_or_default(),
        _ => String::new(),
    }
}

fn constraint_words(name: &Term, body: &Term) -> String {
    let name = match name {
        Term::Str(n) | Term::Atom(n) => n.clone(),
        t => show(t),
    };
    format!("constraint {name}: false :- {}", show(body))
}

/// The arguments of a compound term of a name and an arity, if the term
/// is one.
fn parts<'a>(t: &'a Term, name: &str, arity: usize) -> Option<&'a [Term]> {
    match t {
        Term::Compound(f, a) if f == name && a.len() == arity => Some(a),
        _ => None,
    }
}

/// A Prolog list as a vector.
pub fn list(t: &Term) -> Vec<Term> {
    let mut out = Vec::new();
    let mut at = t;
    while let Some(cell) = parts(at, ".", 2) {
        out.push(cell[0].clone());
        at = &cell[1];
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

/// A layer's value, as (mark, seqs of the facts it rests on) pairs.
pub fn marks(v: &Term) -> Vec<(Term, Vec<u64>)> {
    list(v).iter().filter_map(mark).collect()
}

/// Mark-Seqs: a mark, and the entries whose facts it rests on.
fn mark(pair: &Term) -> Option<(Term, Vec<u64>)> {
    let a = parts(pair, "-", 2)?;
    let seqs = list(&a[1]).into_iter().filter_map(|s| match s {
        Term::Int(n) if n > 0 => Some(n as u64),
        _ => None,
    });
    Some((a[0].clone(), seqs.collect()))
}

/// A constraint node's value: Vars-Prov pairs. Solutions that rest on the
/// same facts (the same pair found both ways round) count once.
pub fn solutions(v: &Term) -> Vec<Solution> {
    let mut out: Vec<Solution> = Vec::new();
    for s in list(v).iter().filter_map(solution) {
        if s.rests_on_two() && !out.iter().any(|o| o.key() == s.key()) {
            out.push(s);
        }
    }
    out
}

/// Vars-Prov: a solution's values, and what each rests on.
fn solution(pair: &Term) -> Option<Solution> {
    let a = parts(pair, "-", 2)?;
    let (prov, facts) = list(&a[1]).iter().filter_map(rests_on).unzip();
    let values = list(&a[0]);
    Some(Solution {
        values,
        prov,
        facts,
    })
}

/// belief(Seq, Fact) or claim(Source, Seq, Fact): where a fact came from,
/// and the fact.
fn rests_on(p: &Term) -> Option<(Prov, Term)> {
    if let Some([Term::Int(s), fact]) = parts(p, "belief", 2) {
        return Some((Prov::Belief(*s as u64), fact.clone()));
    }
    match parts(p, "claim", 3)? {
        [Term::Str(src) | Term::Atom(src), Term::Int(s), fact] => {
            Some((Prov::Claim(src.clone(), *s as u64), fact.clone()))
        }
        _ => None,
    }
}

impl Solution {
    /// A solution is known by the facts it rests on.
    fn key(&self) -> Vec<String> {
        let rests = self.prov.iter().zip(&self.facts);
        let mut k: Vec<String> = rests
            .map(|(p, f)| format!("{}:{}", p.seq(), show(f)))
            .collect();
        k.sort();
        k.dedup();
        k
    }

    /// Whether it rests on two facts: one fact matched twice is not two
    /// facts disagreeing.
    fn rests_on_two(&self) -> bool {
        self.key().len() >= 2 || self.prov.len() < 2
    }
}

/// The operators that are written between their two arguments.
const INFIX: [&str; 12] = [
    "\\=", "=", "is", "<", ">", "=<", ">=", "+", "-", "*", "/", ":-",
];

/// A term as Prolog would print it, near enough for reading.
pub fn show(t: &Term) -> String {
    match t {
        Term::Int(i) => i.to_string(),
        Term::Float(x) => x.to_string(),
        Term::Str(s) => format!("\"{s}\""),
        Term::Atom(a) => a.clone(),
        Term::Var(v) => format!("_{v}"),
        Term::Compound(f, _) if f == "." => format!("[{}]", shown(&list(t))),
        Term::Compound(f, a) if f == "," && a.len() == 2 => shown(a),
        Term::Compound(f, a) if a.len() == 2 && INFIX.contains(&f.as_str()) => {
            format!("{} {f} {}", show(&a[0]), show(&a[1]))
        }
        Term::Compound(f, a) => format!("{f}({})", shown(a)),
    }
}

/// Terms shown, with commas between.
fn shown(terms: &[Term]) -> String {
    terms.iter().map(show).collect::<Vec<_>>().join(", ")
}

/// The label a world gives an entity (`label(E, "text")`), or its own name.
pub fn label(facts: &[(u64, Term)], e: &Term) -> String {
    let given = facts.iter().find_map(|(_, f)| given_label(f, e));
    given.unwrap_or_else(|| show(e).replace('_', " "))
}

/// The text of a fact label(E, "text"), if it is one about this entity.
fn given_label(fact: &Term, e: &Term) -> Option<String> {
    match parts(fact, "label", 2)? {
        [of, Term::Str(s)] if of == e => Some(s.clone()),
        _ => None,
    }
}

fn is_meta(f: &Term) -> bool {
    matches!(f, Term::Compound(n, _) if n == "claims" || n == "node_def" || n == "label")
}

/// The readings of a supposition typed in English: facts of the world with
/// one number changed to a number in the text, ranked by how many of the
/// text's words name the fact's other arguments. No model and no parser: a
/// "do you mean?" built from the world's own facts, until APE is used.
pub fn readings(facts: &[(u64, Term)], text: &str) -> Vec<Term> {
    let said = Said::read(text);
    let mut scored: Vec<(usize, Term)> = Vec::new();
    for reading in facts.iter().flat_map(|(_, f)| said.readings_of(facts, f)) {
        if !scored.iter().any(|(_, t)| *t == reading.1) {
            scored.push(reading);
        }
    }
    scored.sort_by_key(|s| std::cmp::Reverse(s.0));
    scored.into_iter().map(|(_, t)| t).collect()
}

/// What a typed text says, for matching against facts: its words of
/// three letters or more, and its numbers.
struct Said {
    words: Vec<String>,
    numbers: Vec<i64>,
}

impl Said {
    fn read(text: &str) -> Said {
        let lower = text.to_lowercase();
        Said {
            words: words_of(&lower),
            numbers: numbers_of(&lower),
        }
    }

    /// The readings a fact gives, each with its score: the fact with one
    /// of its numbers changed to one of the text's, where a word of the
    /// text names the fact.
    fn readings_of(&self, facts: &[(u64, Term)], fact: &Term) -> Vec<(usize, Term)> {
        let fact = claimed(fact);
        let Term::Compound(name, args) = fact else {
            return vec![];
        };
        if is_meta(fact) {
            return vec![];
        }
        let numbered = (0..args.len()).filter(|k| matches!(args[*k], Term::Int(_)));
        let changes = |k| self.changes(facts, &Number { name, args, k });
        numbered.flat_map(changes).collect()
    }

    /// A fact with a number changed to each other number of the text,
    /// scored by the words that name the fact; none if none does.
    fn changes(&self, facts: &[(u64, Term)], at: &Number) -> Vec<(usize, Term)> {
        let score = self.score(at.name, &at.others(facts));
        let numbers = self.numbers.iter().filter(|n| !at.is(**n));
        let readings = numbers.map(|n| (score, at.changed_to(*n)));
        readings.filter(|_| score > 0).collect()
    }

    /// How many of the text's words are in a fact's name or its other
    /// arguments.
    fn score(&self, name: &str, others: &str) -> usize {
        let names = |w: &&String| others.contains(w.as_str()) || name.contains(w.as_str());
        self.words.iter().filter(names).count()
    }
}

/// The fact a claim makes, or the fact itself.
fn claimed(fact: &Term) -> &Term {
    match parts(fact, "claims", 2) {
        Some(a) => &a[1],
        None => fact,
    }
}

/// A text's words of three letters or more.
fn words_of(text: &str) -> Vec<String> {
    let words = text.split(|c: char| !c.is_alphanumeric());
    words.filter(|w| w.len() >= 3).map(String::from).collect()
}

/// A text's numbers.
fn numbers_of(text: &str) -> Vec<i64> {
    let numbers = text.split(|c: char| !c.is_ascii_digit());
    numbers.filter_map(|n| n.parse().ok()).collect()
}

/// A number in a fact, by its place: the fact's name, its arguments, and
/// which of them is the number.
struct Number<'a> {
    name: &'a str,
    args: &'a [Term],
    k: usize,
}

impl Number<'_> {
    fn is(&self, n: i64) -> bool {
        self.args[self.k] == Term::Int(n)
    }

    /// The fact's other arguments, each shown and by its label, in small
    /// letters.
    fn others(&self, facts: &[(u64, Term)]) -> String {
        let rest = self.args.iter().enumerate().filter(|(j, _)| *j != self.k);
        let words: Vec<String> = rest
            .map(|(_, a)| format!("{} {}", show(a), label(facts, a)))
            .collect();
        words.join(" ").to_lowercase().replace('_', " ")
    }

    /// The fact with this number changed to another.
    fn changed_to(&self, n: i64) -> Term {
        let mut args = self.args.to_vec();
        args[self.k] = Term::Int(n);
        Term::Compound(self.name.to_string(), args)
    }
}

/// A fact restated in English, by a generic frame: "the date of Buran's
/// flight is 1989".
pub fn restate(facts: &[(u64, Term)], t: &Term) -> String {
    match t {
        Term::Compound(n, a) if a.len() == 2 => attribute(facts, n, a),
        Term::Compound(n, a) if a.len() == 1 => quality(facts, n, &a[0]),
        t => show(t),
    }
}

/// A fact of two arguments, as an attribute: "the date of Buran's flight
/// is 1989".
fn attribute(facts: &[(u64, Term)], name: &str, a: &[Term]) -> String {
    let what = name.replace('_', " ");
    format!("the {what} of {} is {}", label(facts, &a[0]), show(&a[1]))
}

/// A fact of one argument, as a quality: "Buran is reusable".
fn quality(facts: &[(u64, Term)], name: &str, e: &Term) -> String {
    format!("{} is {}", label(facts, e), name.replace('_', " "))
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
        return timeline(&rows, facts);
    }
    match rows.first() {
        Some(first) => filled(template, names, &list(first)),
        None => format!("{template} (no answer)"),
    }
}

/// Rows of an entity and a year, as "year label" in order of the years.
fn timeline(rows: &[Term], facts: &[(u64, Term)]) -> String {
    let mut points: Vec<(i64, String)> = rows.iter().filter_map(|r| point(r, facts)).collect();
    points.sort();
    let shown: Vec<String> = points.iter().map(|(y, l)| format!("{y} {l}")).collect();
    shown.join(" ── ")
}

/// A row of an entity and a year, as the year and the entity's label.
fn point(row: &Term, facts: &[(u64, Term)]) -> Option<(i64, String)> {
    match list(row).as_slice() {
        [e, Term::Int(y)] => Some((*y, label(facts, e))),
        _ => None,
    }
}

/// A template with each name's value put in.
fn filled(template: &str, names: &[String], values: &[Term]) -> String {
    let mut out = template.to_string();
    for (n, v) in names.iter().zip(values) {
        out = out.replace(&format!("{{{n}:years}}"), &years(v));
        out = out.replace(&format!("{{{n}}}"), &show(v));
    }
    out
}

/// A count of years in words: "a year", "two years".
fn years(v: &Term) -> String {
    match v {
        Term::Int(1) => "a year".to_string(),
        Term::Int(k) => format!("{} years", number_word(*k)),
        v => show(v),
    }
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
    fn a_numbers_own_value_does_not_name_its_fact() {
        // Catches a fact scored by the number that is to change: "1988"
        // in the text is not a word that names date(buran, 1988).
        let facts = vec![(2, date("buran", 1988))];
        assert_eq!(readings(&facts, "1988 to 1989"), Vec::<Term>::new());
    }

    #[test]
    fn a_word_of_three_letters_names_a_fact() {
        // Catches the shortest words dropped: "ant" names date(ant, 1).
        let facts = vec![(2, date("ant", 1))];
        assert_eq!(readings(&facts, "the ant 2"), [date("ant", 2)]);
    }

    #[test]
    fn a_fact_of_two_arguments_is_restated_as_an_attribute() {
        // Catches the underscores of a name left in its English.
        let t = Term::Compound(
            "launch_site".into(),
            vec![Term::Atom("buran".into()), Term::Atom("baikonur".into())],
        );
        assert_eq!(restate(&[], &t), "the launch site of buran is baikonur");
    }

    #[test]
    fn a_fact_of_one_argument_is_restated_as_a_quality() {
        // Catches the quality put before what has it, or its underscores
        // left in.
        let t = Term::Compound("partly_reusable".into(), vec![Term::Atom("buran".into())]);
        assert_eq!(restate(&[], &t), "buran is partly reusable");
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

    #[test]
    fn a_suggestions_variables_are_numbered_from_its_own_first() {
        // Catches a suggestion whose key depends on its place in the answer.
        let kind = |v| Term::Compound("rule".into(), vec![Term::Var(v), Term::Var(v)]);
        let args = vec![
            kind(7),
            make_list(vec![]),
            Term::Str("e".into()),
            Term::Int(1),
        ];
        let s = Suggestion::from_term(&Term::Compound("suggestion".into(), args)).unwrap();
        assert_eq!(s.kind, kind(0));
    }

    #[test]
    fn a_reading_two_facts_give_is_offered_once() {
        // Catches the same reading listed for a belief and for a claim of it.
        let claim = Term::Compound(
            "claims".into(),
            vec![Term::Str("talk".into()), date("flight", 1988)],
        );
        let facts = vec![(2, date("flight", 1988)), (3, claim)];
        assert_eq!(
            readings(&facts, "the flight was in 1989"),
            [date("flight", 1989)]
        );
    }

    #[test]
    fn readings_are_ranked_by_the_words_that_name_them() {
        // Catches readings left in the order of the facts.
        let facts = vec![(2, date("launch", 1987)), (3, date("buran_flight", 1988))];
        let r = readings(&facts, "the buran flight or launch, 1990");
        assert_eq!(r, [date("buran_flight", 1990), date("launch", 1990)]);
    }

    #[test]
    fn a_mark_rests_on_entries_only() {
        // Catches 0, which stands for no entry, kept among a mark's entries.
        let seqs = make_list(vec![Term::Int(0), Term::Int(3)]);
        let pair = Term::Compound("-".into(), vec![Term::Atom("dot".into()), seqs]);
        assert_eq!(
            marks(&make_list(vec![pair])),
            [(Term::Atom("dot".into()), vec![3])]
        );
    }
}
