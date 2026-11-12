# Implementation plan: the Adversaria prototype, stage A

This plan lets a session that sees only this repository build the first stage of the prototype. Read `CLAUDE.md` first, then this plan, then the sections of `docs/design.md` that each milestone names.

## What stage A is for

Stage A is an experiment, not a product. It must answer four questions with numbers from its own journal:

1. Do constraints find real errors in the user's notes?
2. Does the user adopt suggestions, and keep them, or retract them later?
3. Does the share of clerical actions (searching, following links) fall?
4. Is the dated, chained record used and trusted?

Build what answers these. Everything else waits for stage B (listed at the end). If a milestone grows past its estimate by a quarter, stop and report to the author before going on.

## Sources of truth

* `docs/design.md` is the design. Section names below refer to it. Where this plan and the design disagree, the design wins; report the disagreement.
* `docs/translator.md` covers the English–Prolog translator. Stage A uses none of its model; it may use APE (see milestone 0).
* The design was exported from a living document. From now on, record changes in `docs/design.md` itself, in the same commit as the code that needs them.
* Stage A runs on a PC. The sealer, a separate process, dates and signs the journal (see milestone 1).

## Decisions already made (do not reopen)

From `docs/design.md` § Decisions and the sections named:

* The PC prototype: a terminal program in Rust (ratatui on crossterm), with SWI-Prolog as a child process, split at the query protocol over a pipe; no web interface (§ The PC prototype, § Query protocol). The PC prototype is the version for others. (Changed by the author on 12 November 2026, from a browser client.)
* All the project's own code outside Prolog is Rust, the reference checker included. The memex requires a terminal with kitty's keyboard protocol (key releases) and graphics protocol (images).
* The journal: append-only, hash-chained, dated and signed by the sealer, a separate process that alone holds the key and writes the seals; open format (§ The journal format).
* Imported content is a claim, never a fact; adopting and interrogating take one click or key (§ Claims and beliefs).
* Retract, don't rewind; undo is a tree; a world is a branch (§ The journal).
* Quasimodes, with latching and a command palette beside them (§ Quasimodes, not modes).
* Constraints are dataflow nodes; disagreements among claims are shown apart from violations of beliefs (§ Dataflow, § Claims and beliefs).
* Suggestions come first from Wikidata's property constraints, mined rules and the citation graph; no trained model in stage A (§ Claims and beliefs, `docs/translator.md` § Suggesting facts, rules and constraints).
* Documents cross as Pandoc's JSON document tree (§ Documents in and out).

## Decisions for the author before or during milestone 0

Ask; do not choose silently.

1. **Licence.** The Clex lexicon is GPL-3.0 and APE is LGPL-3.0 (`docs/translator.md`). Using them may constrain the project's licence.
2. **Numbers in hashed records.** RFC 8785 serialises floating-point numbers in a way that is easy to get wrong. Proposal: in format version 1, write every non-integer as a decimal string, so canonical JSON needs no float rules. This changes `docs/design.md` § The journal format.
3. **Which Wikidata subset.** The design's subsets run from about 63 MB (D) to 9.5 GB (A). Proposal: start with D, or a subset of a few thousand items around the author's own field, fetched through the Wikidata query service.

## Environment

`./dev-shell` (`nix-shell` with `shell.nix`), or `nix develop` with flakes enabled (see `flake.nix`), provides all of these, pinned by `flake.lock`, with s(CASP) 1.1.4 at `$SCASP_HOME`. The session needs: Rust (stable, with Cargo); SWI-Prolog 9 or later, with incremental tabling; Pandoc 3; Git; Node.js, only as the tests' oracle for RFC 8785 numbers (ECMAScript defines them). To use the memex: a terminal with kitty's keyboard and graphics protocols. Optional: s(CASP) (from source; its pack server has failed before), APE with Clex. Network access may be limited by the session's environment; if Wikidata, Crossref or package registries are blocked, report which host was blocked and stop that task.

## Layout

```
Cargo.toml     the Rust workspace
crates/format/ Rust library: canonical JSON, terms, journal records and hashes
crates/memex/  Rust program: terminal interface, store, journal writer, Prolog child process
crates/sealer/ Rust program: the sealer (dates and signatures)
crates/check/  Rust program: the reference checker
crates/tools/  Rust programs: importers, measures report, Wikidata subset builder
prolog/        SWI-Prolog: facts, rules, constraints, dataflow, the protocol's other end
tests/         spikes and end-to-end tests; unit tests live beside the Rust code
              every test says what fault it catches
docs/          design.md, translator.md, plan.md, and a log of measured results
```

One command runs every test (`make test`). One command starts the system (`make run`).

## Milestones

Estimates assume Claude writes the code and the author reviews it; they are estimates, not measurements (§ Estimate for the PC prototype). Each milestone ends with: all tests pass, each acceptance check below shown with its output, the measured sizes (lines of code by language, test count) recorded in `docs/log.md`, and one commit per change.

### Milestone 0: toolchain and spikes (about 1 week)

Read: § Dataflow (incremental tabling), § Open questions.

1. Check each tool and record its version.
2. **Spike: incremental tabling.** The design relies on SWI-Prolog recomputing only what a change invalidates, and says this is "recalled from memory and not checked". Write a 20-line test: two tabled predicates over dynamic facts, change one fact, and show which answers are recomputed. Record the result. If it does not behave as the design assumes, stop and report.
3. **Spike: s(CASP).** Install it and reproduce the forall bug (issue #13) and the workaround `--forall=sasp`. Record the result.
4. Settle the three author decisions above.

Acceptance: `docs/log.md` holds the versions and both spike results with their output.

### Milestone 1: the journal (about 1–2 weeks)

Read: § The journal, § The journal format, § Signed entries and exchanging journals (signing only).

1. The memex (Rust): append entries to `journal.jsonl` with `seq`, `parent`, `prev`, `kind`, `body`, `body_hash`, `hash`, hashed as § The journal format says.
2. Sealer: a separate process that watches `journal.jsonl`, and for each new entry appends a seal record to `seals.jsonl` with the date from the system clock, the author's key fingerprint and an Ed25519 signature. The memex never writes `seals.jsonl` and never holds a key; a test enforces this (no code path in `crates/memex/` or `prolog/` writes `seals.jsonl` or reads the private key). The user's private key lives in a file only the sealer reads.
3. `keys.jsonl` holds the public key.
4. Purge: replace one entry's `body` with null, keep `body_hash`, write a purge entry.
5. Reference checker (`crates/check/`): verifies both chains, every signature and every blob's name, and needs nothing else.

Acceptance:

* The checker passes on a journal of 10,000 generated entries, and fails, naming the line, when any one byte of any record is changed (test every field kind at least once).
* A purged entry still verifies.
* Replaying the journal rebuilds the same store twice (same facts, in the same order).

### Milestone 2: store, terms and the query protocol (about 1 week)

Read: § Query protocol, § The journal format (terms).

1. Terms in JSON as the format defines them; facts as replayed from the journal.
2. The query protocol's tagged cells: an encoder and decoder in Prolog and in Rust; messages QUERY, NEXT, CANCEL, ASSERT, RETRACT, ATOM, ANSWER, DONE, ATOM_IS, over a pipe to SWI-Prolog as a child process.
3. Answers only on NEXT; CANCEL drops the query.

Acceptance:

* Round trip: every term in a generated set of 10,000 (atoms, integers, strings, variables, nested compounds) encodes and decodes to itself, both directions, both languages.
* A query with 1,000 answers sends none until NEXT, and exactly n per NEXT n.

### Milestone 3: the outline and the quasimodes (about 2–3 weeks)

Read: § A user story (scenes 1 and 4), § The outline model, § Quasimodes, not modes, § Views, references and anchors, § Response time.

1. Outline: items, levels, editing, links, transclusion shown in place.
2. Leap (forward and back) with recognition on Escape and guide words; Command with noise words and `?` help; Scope; Suppose (milestone 4 gives it meaning). Each quasimode can be held or latched; a command palette offers every command by name.
3. Undo and redo along the current branch; retraction of any earlier entry, listing what depends on it before confirming.
4. Two panes side by side.
5. At start, ask the terminal whether it supports kitty's keyboard and graphics protocols; if not, refuse to start and name what is missing.

Acceptance:

* The user story's scene 1 works as written.
* Keystroke to visible change within 0.1 s for an outline of 10,000 items, measured and logged (§ Response time).
* In a terminal that lacks either protocol, the memex refuses to start and says which one is missing.
* Every edit, undo and retraction is a journal entry, and replay reproduces the screen state.

### Milestone 4: claims, constraints and worlds (about 2–3 weeks)

Read: § Claims and beliefs, § Testing claims (the quote and provenance checks only), § Dataflow, § The journal (worlds).

1. `claims(Source, P)` for everything imported; beliefs are what the user wrote or adopted.
2. Adopt and retract with one click; interrogate with one key: who else claims it, who claims the contrary, its provenance, the source's record, and a preview of adopting it as a world.
3. Constraints as dataflow nodes over beliefs; disagreements among claims shown apart, less loudly.
4. Suppose starts a named branch; two worlds open side by side; a world is a branch of the undo tree.
5. Each constraint the user adopts is tried against a planted violation (§ Testing claims, last row).

Acceptance:

* The user story's scenes 2 and 3 work as written, except the restatement by APE, which is optional here.
* Recomputation after a change touches only the nodes that depend on it (show the count, from milestone 0's spike method).

### Milestone 5: Wikidata and suggestions without a model (about 2 weeks)

Read: § Sources and references, § Claims and beliefs, `docs/translator.md` § A knowledge-base chatbot (the Wikidata subset table), § Suggesting facts, rules and constraints.

1. `tools/wikidata.py` builds the chosen subset as facts, with labels; every Wikidata statement is a claim whose source is Wikidata.
2. Entity links suggested from the label index; Wikidata's property constraints offered as constraints; rules mined from the facts (AMIE, run offline, or a simpler miner with support and confidence); references suggested from the citation graph and main subject, only among works in the store.
3. The suggestion pane, under its rules: never takes the keyboard, never reorders under the pointer, ranked by evidence, few in view, dismissals recorded.

Acceptance:

* A planted wrong date in the user's notes is reported against Wikidata.
* No suggested reference names a work outside the store (test with a store that lacks the obvious answer).

### Milestone 6: documents and results in and out (about 1–2 weeks)

Read: § Documents in and out, § Importing results, § Uncertainty (measured values only), § Testing claims (the source check).

1. Import and export through Pandoc's JSON tree, mapped onto the outline as § Documents in and out says; export records the hashes of its source entries.
2. Re-importing a revised document opens a world, and a table lists each change to adopt or retract.
3. Results import from an exchange file with W3C PROV provenance; values as `measured(Value, StandardUncertainty, Unit)`; a small Python helper writes the file (Python because the user's analyses call it; it is not the project's own code).
4. The quote check and the source check (DOI through Crossref).

Acceptance:

* Markdown and Word round trips preserve headings, items, citations and mathematics (as LaTeX); losses are listed, not hidden.
* A changed input file marks the results that depended on it.

### Milestone 7: measurement and use (about 1 week, then a month of use)

Read: § The journal (counts of actions), and the four questions above.

1. `tools/measures.py` reports, from the journal alone: actions by kind; suggestions offered, adopted, dismissed and later retracted; constraint violations and how many led to a correction; disagreements found.
2. The author uses the system for real work for a month. The report is run weekly and logged.

Acceptance: the report runs on any valid journal, and its counts match a hand count on a small journal.

## Stage B (after the author reviews stage A's numbers)

Each is specified in `docs/design.md`; build only what stage A's numbers justify.

* Falsifiability of formal claims, with its seven tests (§ Falsifiability of formal claims). It needs s(CASP) and declared observation vocabularies.
* The history pane in full, and A/B comparison in full (§ The journal, § A/B comparison).
* Monte Carlo, intervals, weighted worlds and fuzzy degrees (§ Uncertainty).
* Diagrams beyond argument maps (§ Diagrams).
* Mail, calendars and the web (§ Mail, calendars and the web).
* Signed bundles for exchanging journals, and read-only HTML pages (§ Signed entries and exchanging journals, § Documents in and out).
* Scholarly PDF through GROBID.
* The translator's model (`docs/translator.md`).

## How to report

At the end of each milestone, write to `docs/log.md`: what was built, the acceptance checks with their output, measured sizes, what took longer than estimated and why, and any change made to `docs/design.md`. Say plainly what failed or was skipped.
