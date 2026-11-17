//! Results of reasoning over a world, and the pure helpers around them:
//! violations and disagreements, readings of a supposition, restatement in
//! English, and derived text (docs/design.md § Claims and beliefs, § Dataflow).

use crate::engine::Engine;
use crate::outline::{Constraint, Derive};
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

/// A fact a solution rests on, with its provenance.
pub type Sourced<'a> = (&'a Prov, &'a Term);

/// A finding of a constraint: its name, the solution, and whether it is
/// a violation (else sources disagree).
pub struct Finding<'a> {
    pub name: &'a str,
    pub sol: &'a Solution,
    pub violation: bool,
}

impl Finding<'_> {
    /// A mark of it: whether it is a violation, and a text after the
    /// constraint's name and the kind.
    pub fn mark(&self, text: &str) -> (bool, String) {
        let kind = if self.violation {
            "violation"
        } else {
            "sources disagree"
        };
        (self.violation, format!("{}, {kind}: {text}", self.name))
    }
}

impl Solution {
    /// The facts it rests on, each with its provenance.
    pub fn sourced(&self) -> impl Iterator<Item = Sourced<'_>> {
        self.prov.iter().zip(&self.facts)
    }

    /// Whether it rests on beliefs alone, no claim among them.
    fn on_beliefs(&self) -> bool {
        self.prov.iter().all(|p| matches!(p, Prov::Belief(_)))
    }

    /// Whether it rests on what an entry states.
    fn rests_on(&self, seq: u64) -> bool {
        self.prov.iter().any(|p| p.seq() == seq)
    }
}

/// A node's solutions: those resting on beliefs alone, and the others.
fn findings(node: &Term) -> (Vec<Solution>, Vec<Solution>) {
    solutions(node).into_iter().partition(|s| s.on_beliefs())
}

/// How many violations of a constraint there are, if there are any:
/// "1 violation of NAME", "2 violations of NAME".
pub(crate) fn violations_of(n: usize, name: &str) -> Option<String> {
    let s = if n == 1 { "" } else { "s" };
    (n > 0).then(|| format!("{n} violation{s} of {name}"))
}

/// How many of a node's solutions are violations an entry raises: resting
/// on beliefs alone, the entry's among them.
pub(crate) fn raised_by(node: &Term, seq: u64) -> usize {
    let found = solutions(node).into_iter();
    found.filter(|s| s.on_beliefs() && s.rests_on(seq)).count()
}

impl ConstraintEval {
    /// What a constraint's node found: the solutions resting on beliefs
    /// alone are violations, the others disagreements.
    pub(crate) fn of(seq: u64, constraint: Constraint, node: &Term) -> ConstraintEval {
        let (violations, disagreements) = findings(node);
        ConstraintEval {
            seq,
            constraint,
            violations,
            disagreements,
        }
    }

    /// Its findings: the violations, then the disagreements.
    pub fn findings(&self) -> impl Iterator<Item = Finding<'_>> {
        let name = self.constraint.name.as_str();
        let found = |violation| {
            move |sol| Finding {
                name,
                sol,
                violation,
            }
        };
        let violations = self.violations.iter().map(found(true));
        violations.chain(self.disagreements.iter().map(found(false)))
    }

    /// Read a constraint's node from an engine the world is loaded in.
    pub fn read(engine: &mut Engine, seq: u64, c: &Constraint) -> Result<Self, String> {
        let v = engine.node(seq).map_err(crate::engine::told)?;
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

impl ConstraintEval {
    /// A finding of this constraint, named: the constraint, and the facts
    /// it rests on, sorted.
    fn name_of(&self, s: &Solution) -> String {
        let mut facts: Vec<String> = s.facts.iter().map(show).collect();
        facts.sort();
        format!("{}: {}", self.constraint.name, facts.join(" · "))
    }
}

impl WorldEval {
    /// The entries whose facts are in violations: what rests on them is
    /// shown loud.
    pub fn violated_facts(&self) -> std::collections::BTreeSet<u64> {
        let violations = self.constraints.iter().flat_map(|c| &c.violations);
        violations
            .flat_map(|v| v.prov.iter().map(|p| p.seq()))
            .collect()
    }

    /// What the world shows: its violations, and its disagreements, each
    /// named, sorted.
    pub fn observed(&self) -> (Vec<String>, Vec<String>) {
        let violations = self.named(|c| &c.violations);
        (violations, self.named(|c| &c.disagreements))
    }

    /// The findings of one kind, each named, sorted.
    fn named(&self, kind: fn(&ConstraintEval) -> &Vec<Solution>) -> Vec<String> {
        let found = self.constraints.iter();
        let mut names: Vec<String> = found
            .flat_map(|c| kind(c).iter().map(|s| c.name_of(s)))
            .collect();
        names.sort();
        names
    }
}

/// What derived items and layers found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Nodes {
    /// Each derived item's answers.
    pub derived: BTreeMap<u64, Term>,
    /// Each node, of a derived item or a layer, that could not be read:
    /// what went wrong.
    pub failed: BTreeMap<u64, String>,
    /// Each layer's marks, by the layer's seq: the mark, and the seqs of
    /// the facts it rests on.
    pub layers: BTreeMap<u64, Vec<(Term, Vec<u64>)>>,
}

impl Nodes {
    /// A layer's marks, by the layer's seq: none if it has given none.
    pub fn marks(&self, layer: u64) -> &[(Term, Vec<u64>)] {
        self.layers.get(&layer).map_or(&[], Vec::as_slice)
    }
}

impl WorldEval {
    /// A derived item's text in this world, if its node has answered or
    /// failed.
    pub fn derived_text(&self, id: u64, d: &Derive) -> Option<String> {
        if let Some(why) = self.nodes.failed.get(&id) {
            return Some(format!("{} (cannot answer: {why})", d.template));
        }
        let answer = self.nodes.derived.get(&id)?;
        Some(fill(&d.template, &d.names, answer, &self.facts))
    }

    /// The facts stated by entries other than one that are this fact,
    /// claims of it included: each entry, and its fact as stated.
    pub fn stating(&self, fact: &Term, except: u64) -> Vec<(u64, &Term)> {
        let others = self
            .facts
            .iter()
            .filter(|(s, f)| *s != except && claimed(f) == fact);
        others.map(|(s, f)| (*s, f)).collect()
    }

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
    /// How an adoption records the suggestion it accepted.
    pub fn mark(&self) -> serde_json::Value {
        serde_json::json!({"kind": show(&self.kind), "evidence": self.evidence, "score": self.score})
    }

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

/// A term as text: a string bare, without its quotes.
pub(crate) fn evidence(t: &Term) -> String {
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

/// Whether a supposition typed in English states a belief already held:
/// a fact its words name, with a number the text gives, as a reading would
/// be if the number were not the fact's own.
pub fn believed_already(facts: &[(u64, Term)], text: &str) -> bool {
    let said = Said::read(text);
    facts.iter().any(|(_, f)| said.restates(facts, f))
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

    /// Whether the text names a fact, which is no claim, and gives one of
    /// its numbers as it is.
    fn restates(&self, facts: &[(u64, Term)], fact: &Term) -> bool {
        if is_meta(fact) {
            return false;
        }
        let Term::Compound(name, args) = fact else {
            return false;
        };
        (0..args.len()).any(|k| self.restates_at(facts, &Number { name, args, k }))
    }

    /// Whether the text gives a fact's number as it is, and names the fact.
    fn restates_at(&self, facts: &[(u64, Term)], at: &Number) -> bool {
        let held = self.numbers.iter().any(|n| at.is(*n));
        held && self.score(at.name, &at.others(facts)) > 0
    }

    /// A fact with a number changed to each other number of the text,
    /// scored by the words that name the fact; none if none does.
    fn changes(&self, facts: &[(u64, Term)], at: &Number) -> Vec<(usize, Term)> {
        let score = self.score(at.name, &at.others(facts));
        let numbers = self.numbers.iter().filter(|n| !at.is(**n));
        let readings = numbers.map(|n| (score, at.changed_to(*n)));
        readings.filter(|_| score > 0).collect()
    }

    /// How many of the text's words are words of a fact's name or of its
    /// other arguments: whole words, so "the" does not name "other".
    fn score(&self, name: &str, others: &str) -> usize {
        let named = words_of(&format!("{name} {others}").to_lowercase());
        self.words.iter().filter(|w| named.contains(w)).count()
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

/// A text's whole numbers, as written: a sign kept, a full stop that ends
/// a sentence dropped, and a number with a fraction, such as 1989.5, not
/// read as two.
fn numbers_of(text: &str) -> Vec<i64> {
    let tokens = text.split(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.'));
    let numbers = tokens.map(|t| t.trim_end_matches('.'));
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
    let Term::Compound(n, a) = t else {
        return show(t);
    };
    match a.as_slice() {
        [e, v] => attribute(facts, n, e, v),
        [e] => quality(facts, n, e),
        _ => show(t),
    }
}

/// A fact of two arguments, as an attribute: "the date of Buran's flight
/// is 1989".
fn attribute(facts: &[(u64, Term)], name: &str, e: &Term, v: &Term) -> String {
    let what = name.replace('_', " ");
    format!("the {what} of {} is {}", label(facts, e), show(v))
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

/// A count of years in words: "a year", "two years"; one not whole in
/// figures, "2.5 years".
fn years(v: &Term) -> String {
    match v {
        Term::Int(1) => "a year".to_string(),
        Term::Int(k) => format!("{} years", number_word(*k)),
        Term::Float(_) => format!("{} years", show(v)),
        v => show(v),
    }
}

/// The beliefs a new fact (entry `new`) directly contradicts, and the
/// violations it is part of that have no single repair. A violation that
/// rests on the new fact and exactly one other belief is a direct
/// contradiction: giving up that belief is the only way to keep the new
/// fact and the constraint. A violation resting on two or more other
/// beliefs could be repaired by giving up any of them; which one is the
/// user's choice, so it is left, counted by constraint, as "1 violation
/// of NAME".
pub(crate) fn direct_conflicts(
    constraints: &[ConstraintEval],
    new: u64,
) -> (Vec<u64>, Vec<String>) {
    let found: Vec<_> = constraints.iter().map(|c| conflicts_of(c, new)).collect();
    let mut direct: Vec<u64> = found.iter().flat_map(|(one, _)| one.clone()).collect();
    direct.sort();
    direct.dedup();
    (
        direct,
        found.into_iter().filter_map(|(_, left)| left).collect(),
    )
}

/// Of a constraint's violations that rest on a new fact: the beliefs that
/// those resting on one other belief contradict, and how many rest on
/// more, if any do.
fn conflicts_of(c: &ConstraintEval, new: u64) -> (Vec<u64>, Option<String>) {
    let rests = c.violations.iter().filter_map(|v| others_beside(v, new));
    let (one, many): (Vec<_>, Vec<_>) = rests.partition(|o| o.len() == 1);
    (one.concat(), violations_of(many.len(), &c.constraint.name))
}

/// The other beliefs a violation rests on, beside a new fact; none if it
/// does not rest on the new fact, or rests on it alone.
fn others_beside(v: &Solution, new: u64) -> Option<Vec<u64>> {
    let seqs = v.prov.iter().map(Prov::seq);
    if !seqs.clone().any(|s| s == new) {
        return None;
    }
    let mut others: Vec<u64> = seqs.filter(|s| *s != new).collect();
    others.sort();
    others.dedup();
    (!others.is_empty()).then_some(others)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A constraint, by name, violated once on each of these sets of
    /// beliefs.
    fn violated(name: &str, rests: &[&[u64]]) -> ConstraintEval {
        let constraint = Constraint {
            name: name.into(),
            rule: String::new(),
        };
        let violations = rests.iter().map(|r| on_beliefs(r)).collect();
        let disagreements = vec![];
        ConstraintEval {
            seq: 1,
            constraint,
            violations,
            disagreements,
        }
    }

    /// A solution resting on these beliefs.
    fn on_beliefs(seqs: &[u64]) -> Solution {
        let prov = seqs.iter().map(|s| Prov::Belief(*s)).collect();
        Solution {
            values: vec![],
            prov,
            facts: vec![],
        }
    }

    #[test]
    fn a_belief_named_twice_in_one_violation_is_contradicted_directly() {
        let c = violated("c", &[&[9, 3, 3]]);
        assert_eq!(direct_conflicts(&[c], 9), (vec![3], vec![]));
    }

    #[test]
    fn a_belief_contradicted_twice_is_named_once() {
        let c = violated("c", &[&[9, 3], &[3, 9]]);
        assert_eq!(direct_conflicts(&[c], 9).0, vec![3]);
    }

    #[test]
    fn a_violation_without_the_new_fact_is_none_of_its_conflicts() {
        let c = violated("c", &[&[1, 2]]);
        assert_eq!(direct_conflicts(&[c], 9), (vec![], vec![]));
    }

    #[test]
    fn a_violation_of_the_new_fact_alone_is_not_left() {
        let c = violated("c", &[&[9]]);
        assert_eq!(direct_conflicts(&[c], 9), (vec![], vec![]));
    }

    #[test]
    fn the_violations_left_are_counted_by_constraint() {
        let c = violated("one date", &[&[9, 1, 2], &[9, 3, 4]]);
        assert_eq!(direct_conflicts(&[c], 9).1, ["2 violations of one date"]);
    }

    fn date(e: &str, y: i64) -> Term {
        Term::Compound("date".into(), vec![Term::Atom(e.into()), Term::Int(y)])
    }

    fn lbl(e: &str, s: &str) -> Term {
        Term::Compound(
            "label".into(),
            vec![Term::Atom(e.into()), Term::Str(s.into())],
        )
    }

    /// A world in which each constraint, by name, has these findings, of
    /// these atoms: violations first, then disagreements.
    fn world(found: &[(&str, &[&str], &[&str])]) -> WorldEval {
        let constraints = found.iter().map(|(n, v, d)| finding(n, v, d)).collect();
        WorldEval {
            facts: vec![],
            constraints,
            nodes: Nodes::default(),
            at: 0,
        }
    }

    /// A constraint with a violation resting on some atoms, and a
    /// disagreement resting on others; none for no atoms.
    fn finding(name: &str, violated: &[&str], disagreed: &[&str]) -> ConstraintEval {
        let constraint = Constraint {
            name: name.into(),
            rule: String::new(),
        };
        let (violations, disagreements) = (resting(violated), resting(disagreed));
        ConstraintEval {
            seq: 1,
            constraint,
            violations,
            disagreements,
        }
    }

    /// A solution resting on facts that are these atoms; none for none.
    fn resting(atoms: &[&str]) -> Vec<Solution> {
        let facts: Vec<Term> = atoms.iter().map(|a| Term::Atom(a.to_string())).collect();
        let solution = Solution {
            values: vec![],
            prov: vec![],
            facts,
        };
        if atoms.is_empty() {
            vec![]
        } else {
            vec![solution]
        }
    }

    #[test]
    fn a_finding_is_named_by_its_constraint_and_its_facts_sorted() {
        let w = world(&[("c", &["b", "a"], &[])]);
        assert_eq!(w.observed(), (vec!["c: a · b".to_string()], vec![]));
    }

    #[test]
    fn the_findings_of_a_kind_are_sorted() {
        let w = world(&[("z", &["x"], &["y"]), ("a", &["x"], &["y"])]);
        let (v, d) = w.observed();
        assert_eq!(
            (v, d),
            (
                vec!["a: x".into(), "z: x".into()],
                vec!["a: y".into(), "z: y".into()]
            )
        );
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
    fn a_reading_takes_a_number_as_written() {
        // Catches "-5" read as 5, and "1989.5" as 1989 and 5: readings of
        // values the user did not write.
        let flight = |y| date("buran_flight", y);
        let facts = [
            (1, flight(1988)),
            (2, lbl("buran_flight", "Buran's flight")),
        ];
        let read = |text| readings(&facts, text);
        assert_eq!(read("the flight was in -5"), [flight(-5)]);
        assert_eq!(read("the flight was in 1989."), [flight(1989)]);
        assert!(read("the flight was in 1989.5").is_empty());
    }

    #[test]
    fn years_not_whole_are_still_years() {
        // Catches the unit dropped from a float: "It took 2.5." for 2.5
        // years, which says something else.
        let names = vec!["N".to_string()];
        let half = make_list(vec![make_list(vec![Term::Float(2.5)])]);
        assert_eq!(fill("{N:years}", &names, &half, &[]), "2.5 years");
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
    fn a_word_names_a_fact_only_as_a_whole_word() {
        // Catches a word found inside another: "the" in "other", "light"
        // in "flight", so "the cat is 5" read as other's date.
        let facts = vec![(2, date("other", 3)), (3, date("buran_flight", 1988))];
        assert_eq!(readings(&facts, "the cat is 5"), Vec::<Term>::new());
        assert_eq!(readings(&facts, "a light in 1990"), Vec::<Term>::new());
        assert!(!believed_already(&facts, "the cat is 3"));
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
