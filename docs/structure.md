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

Code has a second count, which is now its measure: what a reader holds
at each statement of a function, read in order. It is the count the book
in hedj/bcw3 makes of its blocks, chosen by the user on 13 November 2026
in place of the count above, which is still reported beside it. Its
rules are at the top of `crates/tools/src/held.rs`:

- A function is read as its signature, then each statement; a branch or
  a loop as its head, then each arm and each inner statement.
- At a statement the reader holds each local and each project thing it
  names, and each local that an earlier statement named and a later one
  names again.
- One field or method of a struct is the struct; two or more are a thing
  each. A method's own type is its topic and is free, with one piece of
  it. A local of a declared type is its type's instance, one thing.

Both counts are frozen as of commit 7fb5e2a. The first was changed six
times during stage 2, each change tested and stated in its commit, but a
measure changed while it judges the work is hard to trust. A count
changes again only to fix a fault that a failing test shows, stated as
such, or by the user's decision.

What the measure cannot see, so its numbers are bounds, not facts:
method calls are resolved without types, so a call such as `.find()`
may be std or the project's (reported as a lower and an upper bound);
172 macros
have arguments that are not expressions; concepts written in plain words,
not as a backticked term, are not found in prose (an undercount); and a
count of distinct referents overstates the load of a function whose steps
are independent, until the steps are named functions.

### The target

At most 4 everywhere: for documents and the module tree by the first
count, and for code at every statement by the second. Where plain code
holds more, it is kept plain, listed and reported, not reshaped for the
count (the book's rule for honest measures).

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

By the second count, at commit 7fb5e2a, before any work towards it:

| Units | Hold more than 4 at a statement | Highest |
| --- | --- | --- |
| Rust functions | 304 of 859; upper bound 321 | 33, `plot` in `view.rs` |
| Rust test functions | 124 of 227 | 14 |

By crate: memex 221, tools 50, format 27, protocol 10, sealer 7,
check 6.

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

How the code is to be written, after the book in hedj/bcw3
(`next/book/framework.rst`), from the bottom up: the record, the
outline, reasoning, then the screen.

1. Shapes are small first: a struct has about four fields, an enum about
   four cases. The functions follow from the shapes.
2. An operation lives on the struct that owns the data it changes, and
   touches one or two of its fields.
3. A step only chooses: one line for each case, each a call to a named
   operation. A dispatch over many cases is a table.
4. A function binds few names and uses each soon after binding it.
5. What has no present use goes; no helper for fewer than three uses,
   and no function that only passes its work on.
6. A test checks one thing, through helpers named for the question they
   ask, as `quasi(config, "RightAlt")`.

The proof so far: `config.rs` was restructured by these rules (commit
e3276ca). It had 5 functions over 4 (the largest 13); it has none at the
lower bound, and three whose upper bounds of 5 to 7 come from calls each
checked by hand to be std's. Its behaviour and checks are unchanged.
