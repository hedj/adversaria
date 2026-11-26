//! The constraint pane: a rule an item's findings come from, in words and
//! as Prolog, with the predicates it reads, whether each is stated, and the
//! facts of each finding, what they share and how they differ
//! (docs/guide.md § 1.1).

use super::*;

/// The predicates a goal or body calls, by name, and through the world's
/// rules, those their bodies call.
fn predicates(e: &WorldEval, t: &Term, out: &mut Vec<String>) {
    let (f, a) = match t {
        Term::Compound(f, a) => (f, a.as_slice()),
        Term::Atom(f) => (f, &[][..]),
        _ => return,
    };
    if let Some(goals) = goals_within(f, a) {
        return goals.iter().for_each(|x| predicates(e, x, out));
    }
    if out.contains(f) {
        return;
    }
    out.push(f.clone());
    rule_bodies(e, f).for_each(|b| predicates(e, b, out));
}

/// Whether a fact of the world, believed or claimed, is of a predicate,
/// by name: what a goal of it may read. A builtin is never one.
fn is_stated(e: &WorldEval, name: &str) -> bool {
    e.facts.iter().any(|(_, f)| head_name(inner(f)) == name)
}

/// The bodies of the world's rules for a predicate, by name.
fn rule_bodies<'a>(e: &'a WorldEval, name: &'a str) -> impl Iterator<Item = &'a Term> {
    e.facts.iter().filter_map(move |(_, f)| match f {
        Term::Compound(n, a) if n == "rule_def" && a.len() == 3 && head_name(&a[1]) == name => {
            Some(&a[2])
        }
        _ => None,
    })
}

/// The name of a rule's head or of a fact: its predicate.
fn head_name(t: &Term) -> &str {
    match t {
        Term::Compound(n, _) | Term::Atom(n) => n,
        _ => "",
    }
}

/// The goals a control construct calls, as body_literal in
/// prolog/dataflow.pl finds them; none if it is not one.
fn goals_within<'a>(f: &str, a: &'a [Term]) -> Option<&'a [Term]> {
    match (f, a.len()) {
        ("," | ";" | "->" | "*->" | "forall", 2) | ("\\+", 1) => Some(a),
        ("findall", 3 | 4) => Some(&a[1..2]),
        _ => None,
    }
}

fn node_def(e: &WorldEval, id: u64) -> Option<&Term> {
    e.facts.iter().find_map(|(_, f)| match f {
        Term::Compound(n, a) if n == "node_def" && a.len() == 2 && a[0] == Term::Int(id as i64) => {
            Some(&a[1])
        }
        _ => None,
    })
}

/// A constraint's heading, then its rule, set in.
fn constraint_head(c: &ConstraintEval) -> Vec<Line<'static>> {
    let heading = format!("CONSTRAINT · {}", c.constraint.name.to_uppercase());
    let rule = rule_lines(&c.constraint.rule);
    let rule = rule.into_iter().map(|l| Line::raw(format!("  {l}")));
    let head = [Line::styled(heading, dim()), Line::raw("")];
    head.into_iter()
        .chain(rule)
        .chain([Line::raw("")])
        .collect()
}

/// The constraints of a world, drawn against the outline.
pub(super) struct ConstraintPane<'a> {
    pub(super) outline: &'a Outline,
    pub(super) e: &'a WorldEval,
}

impl ConstraintPane<'_> {
    /// The right pane for an item with marks: each constraint with a
    /// finding that rests on one of its facts.
    pub(super) fn of(&self, item: u64) -> Vec<Presented> {
        let touching = self.e.constraints.iter().filter(|c| self.touches(c, item));
        touching.flat_map(|c| self.constraint(c)).collect()
    }

    /// Whether a constraint has a finding that rests on one of an item's
    /// facts.
    fn touches(&self, c: &ConstraintEval, item: u64) -> bool {
        let found = c.violations.iter().chain(&c.disagreements);
        let mut prov = found.flat_map(|s| &s.prov);
        prov.any(|p| self.outline.states(item, p.seq()))
    }

    /// A constraint: its heading and rule, its findings, and what depends
    /// on the facts it reads.
    fn constraint(&self, c: &ConstraintEval) -> Vec<Presented> {
        let mut head = plain(constraint_head(c));
        head[0].1 = Some(self.presented(c));
        [head, plain(self.found(c)), plain(self.dependents(c))].concat()
    }

    /// A constraint, presented in the outline pane, with the item whose
    /// text states it.
    fn presented(&self, c: &ConstraintEval) -> Presentation {
        let item = stating_rule(self.outline, &c.constraint.name);
        Presentation::Constraint {
            pane: 0,
            entry: c.seq,
            item,
            under: None,
        }
    }

    /// A constraint's violations, loud, then its disagreements.
    fn found(&self, c: &ConstraintEval) -> Vec<Line<'static>> {
        let violations = self.kind("violations", &c.violations, violation());
        let disagreements = "disagreements between sources";
        let disagreements = self.kind(disagreements, &c.disagreements, disagreement());
        [violations, disagreements].concat()
    }

    /// Findings of one kind, if there are any: a heading that says when
    /// they were recomputed, then a row for each.
    fn kind(&self, title: &str, found: &[Solution], style: Style) -> Vec<Line<'static>> {
        if found.is_empty() {
            return vec![];
        }
        let heading = title.to_string();
        let rows = found.iter().map(|s| Line::styled(self.row(s), style));
        let heading = std::iter::once(Line::styled(heading, dim()));
        heading.chain(rows).chain([Line::raw("")]).collect()
    }

    /// A finding's row: what its facts share, then how each differs and
    /// where it comes from; for one fact, the fact.
    fn row(&self, sol: &Solution) -> String {
        if let [f] = sol.facts.as_slice() {
            let from = where_from(self.outline, &sol.prov[0]);
            return format!("{} ({from})", reason::show(inner(f)));
        }
        let parts = self.parts(sol);
        match shared_key(&sol.facts).as_str() {
            "" => parts,
            key => format!("{key}: {parts}"),
        }
    }

    /// Each fact of a finding, how it differs and where it comes from.
    fn parts(&self, sol: &Solution) -> String {
        let parts: Vec<String> = (0..sol.facts.len()).map(|i| self.part(sol, i)).collect();
        parts.join(" · ")
    }

    /// A fact of a finding, by its place: how it differs from the first
    /// other fact, and where it comes from.
    fn part(&self, sol: &Solution, i: usize) -> String {
        let from = where_from(self.outline, &sol.prov[i]);
        format!("{} ({from})", differing(&sol.facts, i))
    }

    /// What depends on the facts a constraint reads, if anything: the
    /// derived items that read one of its predicates.
    fn dependents(&self, c: &ConstraintEval) -> Vec<Line<'static>> {
        let deps = self.derived_reading(&self.fact_reads(c.seq));
        if deps.is_empty() {
            return vec![];
        }
        let heading = std::iter::once(Line::styled("depends on these facts", dim()));
        let rows = deps.into_iter().map(|d| self.dependent(d));
        heading.chain(rows).chain([Line::raw("")]).collect()
    }

    /// The predicates a definition reads that the world's facts state.
    fn fact_reads(&self, id: u64) -> Vec<String> {
        let mut reads = definition_reads(self.e, id);
        reads.retain(|p| is_stated(self.e, p));
        reads
    }

    /// The derived items that read one of these predicates, in outline
    /// order.
    fn derived_reading(&self, reads: &[String]) -> Vec<u64> {
        let reading = |d: &u64| {
            definition_reads(self.e, *d)
                .iter()
                .any(|p| reads.contains(p))
        };
        let mut deps: Vec<u64> = self
            .outline
            .derived
            .keys()
            .copied()
            .filter(reading)
            .collect();
        let order = self.outline.in_order();
        deps.sort_by_key(|d| order.iter().position(|x| x == d));
        deps
    }

    /// A derived item that depends on the facts: its number, and its text,
    /// cut at 40 characters.
    fn dependent(&self, d: u64) -> Line<'static> {
        let text = self.derived_text(d);
        let short: String = text.chars().take(40).collect();
        let more = if text.chars().count() > 40 {
            " …"
        } else {
            ""
        };
        Line::raw(format!("  {} {short}{more}", self.outline.number(d)))
    }

    /// A derived item's text in the world, or as the outline holds it.
    fn derived_text(&self, d: u64) -> String {
        let derive = self.outline.derived.get(&d);
        let text = derive.and_then(|t| self.e.derived_text(d, t));
        text.unwrap_or_else(|| self.outline.text(d))
    }
}

/// What a finding's facts share: each argument of the first that every
/// fact has in the same place.
fn shared_key(facts: &[Term]) -> String {
    let shared: Vec<String> = shared_args(facts).map(reason::show).collect();
    shared.join(", ")
}

/// The arguments of the first fact that every fact has in the same place.
fn shared_args(facts: &[Term]) -> impl Iterator<Item = &Term> {
    let n = facts.first().map_or(0, arity);
    (0..n).filter_map(move |k| shared_at(facts, k))
}

/// The first fact's argument at a place, if every fact has it there.
fn shared_at(facts: &[Term], k: usize) -> Option<&Term> {
    let first = arg_of(facts.first()?, k)?;
    facts
        .iter()
        .all(|f| arg_of(f, k) == Some(first))
        .then_some(first)
}

/// How many arguments a fact has.
fn arity(f: &Term) -> usize {
    match inner(f) {
        Term::Compound(_, a) => a.len(),
        _ => 0,
    }
}

/// A fact's argument at a place.
fn arg_of(f: &Term, k: usize) -> Option<&Term> {
    match inner(f) {
        Term::Compound(_, a) => a.get(k),
        _ => None,
    }
}

/// How the fact at a place differs from the first other fact, or from
/// itself if it is alone.
fn differing(facts: &[Term], at: usize) -> String {
    let others = facts.iter().enumerate().filter(|(i, _)| *i != at);
    let other = others.map(|(_, f)| f).next().unwrap_or(&facts[at]);
    difference(&facts[at], other).0
}

/// What a definition reads: the predicates its goal calls.
fn definition_reads(e: &WorldEval, id: u64) -> Vec<String> {
    let mut reads = vec![];
    if let Some(goal) = definition_goal(e, id) {
        predicates(e, goal, &mut reads);
    }
    reads
}

/// A definition's goal: the last argument of its node,
/// constraint(Name, Vars, Body) or query(Vars, Goal).
fn definition_goal(e: &WorldEval, id: u64) -> Option<&Term> {
    match node_def(e, id)? {
        Term::Compound(_, a) => a.last(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_that_reaches_itself_is_read_once() {
        // Catches an endless walk of a journal's recursive rules, which
        // the rule command refuses but a journal may hold.
        let call = |n: &str| Term::Atom(n.into());
        let rule = |h, b| Term::Compound("rule_def".into(), vec![Term::Int(1), call(h), call(b)]);
        let e = WorldEval {
            facts: vec![(1, rule("a", "b")), (2, rule("b", "a"))],
            ..WorldEval::default()
        };
        let mut reads = vec![];
        predicates(&e, &call("a"), &mut reads);
        assert_eq!(reads, ["a", "b"]);
    }
}
