//! The interrogation of a claim item, in the inspector: who else claims what
//! it claims, who claims the contrary, its source's record, and what adopting
//! it would do (docs/guide.md § 1.1, Claims).

use super::*;
use format::record::EntryKind;

/// The right pane at a claim item, or one interrogated: each of the
/// item's claims, then what adopting them would do. The hint follows,
/// drawn by draw_interrogation, where a click can find it.
pub(super) fn interrogation(app: &App, item: u64) -> Vec<Presented> {
    let Some(e) = app.reason.eval[0].as_ref() else {
        return plain(vec![Line::raw("not yet evaluated")]);
    };
    let mut lines = interrogation_head(&app.notebook.outline, item);
    lines.extend(claims_asked(app, e, item));
    lines.extend(plain(adoption_lines(app, item).to_vec()));
    lines
}

/// Each claim an item states, and what is asked of it.
fn claims_asked(app: &App, e: &WorldEval, item: u64) -> Vec<Presented> {
    let claims = app.claims_shown(item).into_iter();
    claims.flat_map(|c| claim_lines(app, e, item, c)).collect()
}

/// The heading of a claim item's questions, and the item.
fn interrogation_head(outline: &Outline, item: u64) -> Vec<Presented> {
    let named = format!("CLAIMS OF {} · WHO SAYS THEM", outline.number(item));
    let heading = Line::styled(named, dim());
    plain(vec![
        heading,
        Line::raw(""),
        Line::raw(outline.text(item)),
        Line::raw(""),
    ])
}

/// A claim interrogated: the claim, in an offer's rows, then who else
/// claims it, who claims the contrary, and its source's record.
fn claim_lines(app: &App, e: &WorldEval, item: u64, claim: (u64, String, Term)) -> Vec<Presented> {
    let mut lines = claim_row(app, item, &claim);
    lines.extend(plain(claim_questions(app, e, &claim)));
    lines
}

/// A claim an item states, in an offer's rows: dim, and said so, once
/// adopted.
fn claim_row(app: &App, item: u64, claim: &(u64, String, Term)) -> Vec<Presented> {
    let (_, source, fact) = claim;
    let open = app.claim_open(item, source.clone(), fact);
    let adopted = app.notebook.not_adopted(vec![claim.clone()]).is_empty();
    offers::offer_row(&open, adopted.then_some("adopted"))
}

/// What is asked of a claim: who else claims it, who claims the
/// contrary, and its source's record.
fn claim_questions(app: &App, e: &WorldEval, claim: &(u64, String, Term)) -> Vec<Line<'static>> {
    let asking = Asking::of(app, e, claim);
    let mut lines = section("who else claims it", asking.claimants(), "no one else");
    lines.extend(section(
        "who claims the contrary",
        asking.contrary(),
        "no one",
    ));
    lines.extend(section(
        "the source's record",
        vec![record(app, &claim.1)],
        "",
    ));
    lines
}

/// A claim being interrogated: the outline it is in, what reasoning
/// found, and the claim: its entry, its source and its fact.
struct Asking<'a> {
    outline: &'a Outline,
    eval: &'a WorldEval,
    claim: &'a (u64, String, Term),
}

impl<'a> Asking<'a> {
    fn of(app: &'a App, eval: &'a WorldEval, claim: &'a (u64, String, Term)) -> Self {
        let outline = &app.notebook.outline;
        Asking {
            outline,
            eval,
            claim,
        }
    }

    /// A fact restated in words, by the world's labels.
    fn restated(&self, f: &Term) -> String {
        reason::restate(&self.eval.facts, inner(f))
    }

    /// Who else states the claim's fact: each other entry, by its place,
    /// with its source, or as a belief.
    fn claimants(&self) -> Vec<String> {
        let (seq, _, p) = self.claim;
        let others = self.eval.stating(p, *seq);
        others
            .into_iter()
            .map(|(s, f)| claimant(self.outline, s, f))
            .collect()
    }

    /// Who claims the contrary: each other fact of each disagreement that
    /// rests on the claim, constraint by constraint. A violation rests on
    /// beliefs alone, never on a claim.
    fn contrary(&self) -> Vec<String> {
        let found = self.eval.constraints.iter().flat_map(disagreements);
        let resting = found.filter(|(_, sol)| sol.prov.iter().any(|q| q.seq() == self.claim.0));
        resting
            .flat_map(|(name, sol)| self.against(name, sol))
            .collect()
    }

    /// The facts of a finding other than the claim's, each a row of who
    /// claims the contrary.
    fn against(&self, name: &str, sol: &Solution) -> Vec<String> {
        let others = sol.sourced().filter(|(q, _)| q.seq() != self.claim.0);
        others.map(|(q, f)| self.contrary_row(name, q, f)).collect()
    }

    /// A row of who claims the contrary: where the fact comes from, the
    /// fact restated, and the constraint it breaks.
    fn contrary_row(&self, name: &str, q: &Prov, f: &Term) -> String {
        let said = self.restated(f);
        let from = where_from(self.outline, q);
        format!("  {from}: {said} ({name})")
    }
}

/// A constraint's disagreements, by its name.
fn disagreements(c: &ConstraintEval) -> impl Iterator<Item = (&str, &Solution)> {
    let name = c.constraint.name.as_str();
    c.disagreements.iter().map(move |sol| (name, sol))
}

/// A section of an interrogation: its heading, then its rows, or, if it
/// has none, what says so.
fn section(heading: &str, rows: Vec<String>, none: &str) -> Vec<Line<'static>> {
    let rows = if rows.is_empty() {
        vec![format!("  {none}")]
    } else {
        rows
    };
    let heading = Line::styled(heading.to_string(), dim());
    std::iter::once(heading)
        .chain(rows.into_iter().map(Line::raw))
        .collect()
}

/// An entry that states a fact: where it is, and its source, or that it
/// is a belief.
fn claimant(outline: &Outline, seq: u64, f: &Term) -> String {
    let at = place(outline, seq);
    match f {
        Term::Compound(n, a) if n == "claims" => format!("  {at} ({})", reason::evidence(&a[0])),
        _ => format!("  {at} (a belief)"),
    }
}

/// A source's record: how many of its claims were adopted, and how many
/// of those adoptions were retracted since, anywhere in the journal.
fn record(app: &App, src: &str) -> String {
    let entries = app.journal().entries();
    let adopted = adoptions_from(entries, src);
    let retracted = retracted_among(entries, &adopted);
    format!(
        "  {} of its claims adopted, {retracted} of them retracted since",
        adopted.len()
    )
}

/// How many of these entries a retraction names, anywhere in the journal.
fn retracted_among(entries: &[format::record::Entry], seqs: &[u64]) -> usize {
    let retracted = |s: &&u64| entries.iter().any(|r| retracts(r, **s));
    seqs.iter().filter(retracted).count()
}

/// The entries that adopt a claim from a source, by number.
fn adoptions_from(entries: &[format::record::Entry], src: &str) -> Vec<u64> {
    let adopting = entries.iter().filter(|x| adopts_from(x, src));
    adopting.map(|x| x.seq).collect()
}

/// Whether an entry adopts a claim from a source.
fn adopts_from(e: &format::record::Entry, src: &str) -> bool {
    e.kind() == EntryKind::Adopt && e.body().is_some_and(|b| b["source"] == src)
}

/// Whether an entry retracts another, by its number.
fn retracts(e: &format::record::Entry, target: u64) -> bool {
    e.kind() == EntryKind::Retract && e.body().and_then(|b| b["target"].as_u64()) == Some(target)
}

/// What adopting the interrogated claims would do, then the blank line
/// before the hint.
fn adoption_lines(app: &App, item: u64) -> [Line<'static>; 3] {
    let preview = match &app.reason.preview {
        Some((of, said)) if *of == item => said.clone(),
        _ => "not yet computed".into(),
    };
    [
        Line::styled("if adopted", dim()),
        Line::raw(format!("  {preview}")),
        Line::raw(""),
    ]
}
