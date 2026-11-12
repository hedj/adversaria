# Structure

How the documentation and the code are to be arranged so that no part asks
a reader to hold more than four things in mind at once. Proposed
12 November 2026. Built so far: the measure, the baseline, the
documentation's structure and one module of code.

## The measure

Working memory holds about four chunks, three to five (Cowan 2001, 2010).
Element interactivity is the number of elements that must be processed
together (Sweller). Applying both to the graph of referents in code and
text is the author's idea. `make interactivity` measures it, by these
rules, written out at the top of `crates/tools/src/interactivity.rs`:

- A unit is a Rust function or module, a Prolog predicate, or a Markdown
  section. Its interactivity is the number of distinct things defined
  elsewhere in the project that it refers to; the language and standard
  libraries are taken as known.
- A unit with parts (a module's files, a section's subsections) has as
  many children as parts. The limit is 4 for both.
- A section refers forward when it uses something defined later in its
  own document; reading in order then needs a cycle.
- Things learned together count as one chunk: terms defined in a section
  that defines at most four; an enum with its variants; a struct of at
  most four fields with its fields. A field is attributed to its struct
  by the declared types along a chain from `self` or a typed parameter,
  else by name. Each count is also reported with each term or field
  apart.

The measure is frozen as of commit a9ed10c. It was changed six times
during stage 2, each change tested and stated in its commit, but a
measure changed while it judges the work is hard to trust. It changes
again only to fix a fault that a failing test shows, stated as such.

What the measure cannot see, so its numbers are bounds, not facts:
method calls are resolved without types, so a call such as `.find()`
may be std or the project's (reported as a lower and an upper bound);
172 macros
have arguments that are not expressions; concepts written in plain words,
not as a backticked term, are not found in prose (an undercount); and a
count of distinct referents overstates the load of a function whose steps
are independent, until the steps are named functions.

### The target

Documents and the module tree: at most 4 everywhere. Functions: none
over 8. Between 5 and 8 is accepted where splitting further would only
add functions that pass their work on, which the measure cannot see as
a cost: a reader follows each one. This replaces the first target of 4
for every function, after stage 2 showed what meeting it takes (see
Where it stands, below).

## Where it stands

Counted by `make interactivity` on the code before `config.rs` was
restructured (commit 33d19b4):

| Units | Over 4 | Largest | Other |
| --- | --- | --- | --- |
| Rust functions | 168 of 550 (30%); upper bound 209 | 83, `App::command` | median 3 |
| Rust test functions | 119 of 222 (53%) | 27 | |
| Rust modules | 2 of 32 | 9, `app.rs` | `memex` has 13 children |
| Prolog predicates | 6 of 93 | 10, `term_cells/6` | |
| Markdown sections | 20 of 158 | 12 | 10 with over 4 children; 22 refer forward |

After the documents were restructured (commit 129f016):

| Units | Over 4 | Largest | Other |
| --- | --- | --- | --- |
| Markdown sections | 0 of 235; 1 counting each term apart | 4 | none with over 4 children; 16 refer forward |

During stage 2 (commits a852bb9 to e05909e), measured with the frozen
measure at e05909e:

| Units | Over 4 | Over 8 | Largest |
| --- | --- | --- | --- |
| Rust functions | 161 of 771 | 70 | 41, `draw_outline` |
| Rust test functions | 122 of 227 | 67 | 25 |
| Prolog predicates | 6 of 93 | 1 | 10, `term_cells/6` |

What stage 2 did: the commands dispatch through their table, and each
of the 33 handlers, in `actions/`, is at most 4 (`App::command` had 83).
Evaluation is in `world.rs` and `evaluation.rs`, at most 4 each. Five
behaviours that no test checked, found by breaking code on purpose, now
have tests. What it showed: under the target of 4, every struct must
have at most four fields and most functions make one to three calls.
Splitting App's 25 fields into four nested parts made the count worse
(each level of nesting is a struct to know) and was reverted.

The one section over 4 counting each term apart is the log's Milestone 1
entry (7); its terms come from sections that each define at most four,
and count as one chunk per section. The 16 forward references are listed
at the end of `make interactivity`; 8 are in the guide (6 of them in part 2,
where the screen, keys and commands each mention the others), 4 in the
design and 4 in the plan.

Most of the load is in three files: `app.rs` (59 functions over 4),
`ui.rs` (23) and `view.rs` (13). The documents' widest nodes are the
design (25 sections at one level), the tutorial (21 steps and parts) and
the guide (16 sections).

## The documentation's structure

A tree of four at every level, read top down. The aim is that a section
cites only its ancestors or what comes before it, so that reading in
order needs no cycle; 16 sections do not yet meet it (above).

```
README
├── Start: the tutorial
│   ├── A. First notes: start, outline, facts, derived text
│   ├── B. Checking claims: claim and constraint, Leap, interrogate, Suppose
│   ├── C. Boxes and rules: boxes, rules, set, what if
│   └── D. Views: chart, grid, timeline, leaving
├── Use: the guide
│   ├── Concepts: journal and seals, outline, facts claims beliefs, constraints and worlds
│   ├── Working: screen, keys and quasimodes, commands, addresses
│   ├── Reasoning: rules and constraints, worlds, views, suggestions
│   └── Around it: documents and results, files and tools, configuration, trouble and limits
├── Understand: the design
│   ├── Purpose: purpose, user story, glossary, decisions
│   ├── The record: journal, format, seals, input that could act
│   ├── Reasoning: dataflow, claims and beliefs, testing claims, the translator
│   │   (docs/translator.md: aims, the model, conversation, reasoning with it)
│   └── Interface: outline model, quasimodes, views and diagrams, the PC prototype
└── Record: plan, log, this document
    ├── The plan: stage A milestones, in four groups
    ├── The log: stage A, in four groups, then later work by theme
    └── Structure: this document
```

Tables are not divided by the measure, which counts sections, not rows.
The table of commands and the Decisions table are each split into four
tables by topic; their rows, alike in form, are reference to look up, not
to hold.

## The code's structure

A tree of four, and an order: each part uses only parts beside or below
it, so the modules form an acyclic graph.

```
workspace
├── record: format (jcs, terms, record, chain), check, sealer
├── reasoning: protocol; prolog (vetting, nodes, wikidata, suggestions)
├── memex
│   ├── state: journal, outline, store and replay, configuration
│   ├── reasoner: client, engine, readings and solutions
│   ├── actions: dispatch, editing, knowledge, worlds and history
│   └── screen: terminal, panels, views
└── tools: wikidata, measures, interactivity; acceptance tests
```

The rules that keep each function within the target:

1. A long function becomes a short tree of named steps (the steps are
   named for what they do), but not a chain of steps that only pass
   their work on.
2. A dispatch is a table, not a long match: `App::command`, with 83
   referents, becomes a lookup in the table of commands, each command's
   handler a function in its group under actions.
3. State lives in the smallest part that needs it, and a method lives on
   the part whose fields it uses, so it reads its own fields, not many.
   Nesting parts only to group fields does not help: App's 25 fields in
   four nested parts measured worse, and the split was reverted.
4. A test checks one thing, through helpers named for the question they
   ask, as `quasi(config, "RightAlt")`.

The proof so far: `config.rs` was restructured by these rules (commit
e3276ca). It had 5 functions over 4 (the largest 13); it has none at the
lower bound, and three whose upper bounds of 5 to 7 come from calls each
checked by hand to be std's. Its behaviour and checks are unchanged.
