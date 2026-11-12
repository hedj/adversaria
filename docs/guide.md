# User guide

This guide describes the Adversaria prototype as built: the memex, which is
the notebook you use in a terminal, and the programs around it. For a first
session, follow docs/tutorial.md. For why it is built this way, see
docs/design.md; for what has been tested and what has not, docs/log.md.

Contents:

1. Concepts
2. Starting
3. The screen
4. Keys and quasimodes
5. Commands
6. Addresses
7. Facts, claims, constraints and derived text
8. Worlds
9. Views
10. Suggestions and Wikidata
11. Documents and results
12. Files
13. Tools
14. Configuration
15. Troubleshooting
16. Limits

## 1. Concepts

**Journal.** Everything you do is an entry in an append-only journal: a
file of JSON lines, each joined to the one before by a hash. Nothing is
ever overwritten or erased, except by `purge` (§ 12). There is no Save:
what you type is written a second after you stop, or when you do anything
else. Undo is an entry too.

**Seal.** A separate program, the sealer, holds a signing key and signs
the journal's entries, dated. The memex itself never sees the key. A sealed
entry can be shown to have existed, unchanged, by its date.

**Outline.** Your notes are an outline of items. Each item has text, may
have items under it, and may state facts.

**Fact, claim, belief.** A fact is a Prolog term an item states, such as
`date(buran_flight, 1988)`. A fact you state is a belief. A fact from a
source (a talk, a paper, Wikidata, an analysis) is a claim:
`claims(Source, Fact)`. A claim becomes a belief only when you adopt it,
and the adoption keeps its source.

**Constraint.** A rule that must never be broken, written as a Prolog
denial: `false :- ...`. When beliefs break it, that is a **violation**. When
a claim and a belief, or two claims, would break it, that is a
**disagreement between sources**. Both are marked under the items involved.

**World.** The facts as they stand are the actual world. A supposition
opens a second world in which something else holds; the two are shown side
by side. Closing a world keeps it in the journal; committing it makes it
the actual world.

**Quasimode.** A state that lasts only while you hold a key, such as
Command: hold the key, type a command, release to run it. You cannot forget
you are in it. See § 4.

## 2. Starting

The memex needs:

- a terminal with the kitty keyboard protocol (for key releases) and the
  kitty graphics protocol: kitty, WezTerm or Ghostty. The memex asks the
  terminal at start and refuses to start, naming what is missing, if either
  protocol is absent;
- SWI-Prolog (`swipl`) on the path, for reasoning;
- Pandoc, for importing and exporting documents;
- curl, for the source check.

`./dev-shell` gives a shell with all of these (except the terminal), and
needs no experimental Nix features. It runs `nix-shell` with the pinned
nixpkgs on the search path, because `nix-shell` looks for an interactive
bash in `<nixpkgs>`; plain `nix-shell` works too, but without nixpkgs on
your `NIX_PATH` it prints an error and uses your system's bash. With
flakes enabled, `nix develop` gives the same shell. All use the versions
pinned in `flake.lock`; the shell is defined in `nix/devshell.nix`.

To start the sealer and the memex together:

```bash
make run
```

This uses the journal directory `~/adversaria-journal` and the key file
`~/.adversaria.key`, making both on first use. Give others with
`JOURNAL=` and `KEY=`:

```bash
make run JOURNAL=$HOME/notes KEY=$HOME/.notes.key
```

Or run the memex alone, on a journal directory, which it makes if needed:

```bash
cargo run -q --release -p memex -- [--config FILE] [--mouse | --no-mouse] <journal directory>
```

Without the sealer, entries are written but not sealed until a sealer runs.

## 3. The screen

- **Top bar:** the journal directory's name (`room`), the world (`world
  actual`, or `worlds actual | suppose: …`), the number of journal entries,
  and a count of violations and disagreements. On the right, which pane has
  focus.
- **Left pane:** the outline. `▼` marks an item with items under it, `▶` a
  folded one (followed by how many items it hides), `•` an item with none.
  Top-level items show their number. Under an item, `⚑` lines are its
  marks: each constraint solution that rests on one of its facts. A
  violation is shown bold; a disagreement dim. An item that shows
  derived text ends in `[derived]`; one that states a claim ends in its
  source, in brackets. A result whose input files have changed says `input
  changed since:` and the files.
- **Right pane:** what the selected item needs. By default its links; on an
  item with marks, the constraint, its rule, its solutions and what depends
  on them; after `interrogate`, the interrogation. `pane` sets it to the
  outline, links or suggestions.
- **Foot:** the message line, and, while a quasimode is held, what you have
  typed and what releasing will do.

A fading hint appears under an item with marks until you have used Suppose
four times.

## 4. Keys and quasimodes

The defaults are below. All of them can be changed (§ 14).

### Editing

| Key | Does |
| --- | --- |
| a printable character | types into the item under the cursor |
| Enter | makes a new item after the cursor's item |
| Tab | makes the item a child of the one above it (`indent`) |
| Shift-Tab | makes the item a sibling of its parent (`outdent`) |
| Backspace, Delete | erase a character before or after the cursor |
| Up, Down | move to the item above or below |
| Left, Right, Home, End | move within the item's text |

### Quasimodes

| Quasimode | Default keys | While held | On release |
| --- | --- | --- | --- |
| Leap forward | Right Alt (AltGr) | type a pattern | the cursor goes to the next item holding it |
| Leap back | Left Alt | type a pattern | the cursor goes to the previous item holding it |
| Command | Left Ctrl | type a command line | the command runs on the item under the cursor |
| Scope | Right Ctrl | Space widens the target: word, statement, branch | the target stays chosen for the next command |
| Suppose | Super (either) | type a statement | the memex offers its readings of it (§ 8) |

Within a quasimode:

- **Tap to latch.** Press and release the key with nothing typed between,
  and the quasimode stays on without holding. Enter, or a second tap, ends
  it. Backspace on an empty latched quasimode leaves it without acting.
- **Esc completes:** a command name in Command, a pattern in Leap.
- **`?` lists** the commands that match (Command) or every match (Leap).
- **Repeat a Leap:** a second tap of a latched, empty Leap repeats the last
  pattern.

Leap matches anywhere in an item's text, ignoring case, and wraps round
at either end of the outline.

Scope matters only to `delete` in this prototype: with the target at word,
`delete` removes the word at the cursor; at statement or branch, it deletes
the item and its branch. The target returns to statement after each
command.

### Bound keys

| Key | Runs |
| --- | --- |
| F1 | the command palette: Command, latched, with the list open |
| F2 | `interrogate` |
| F3 | `adopt` |
| F4 | `unadopt` |
| F6 | `switch` |
| F10 | `quit` |

### Confirmations

Two commands ask before acting, at the foot of the screen:

- `retract` lists the entries that depend on the one you retract. Enter
  retracts; Esc keeps it.
- Suppose offers readings of your statement: `Do you mean: …`. Enter
  accepts; Tab shows the next reading; Esc supposes nothing.

### The mouse

The mouse is off unless you turn it on, with `enabled = true` under
`[mouse]` in the configuration (§ 14), or with `--mouse` for one run.
`--no-mouse` turns it off for one run whatever the file says. Giving both
flags is an error.

With it on, a left click:

- on an item's line in an outline pane moves the cursor there (and focus
  to that pane), as the arrow keys would;
- on `F3 adopts` or `F4 retracts an adoption` under an interrogation runs
  `adopt` or `unadopt` (the hint names whichever keys you have bound);
- on a suggestion, anywhere in its lines, runs `accept` for it.

A click does nothing while a quasimode or a confirmation is open, and
nothing on a mark line, a heading or empty space. Other buttons, dragging
and the wheel do nothing. A click is recorded in the journal as the
keyboard action it stands for.

While the memex has the mouse, the terminal's own text selection usually
needs Shift held (recalled, not tested). That is why it is off by default.

## 5. Commands

Type a command in the Command quasimode, or bind it to a key (§ 14). A
unique prefix is enough: `interr` runs `interrogate`. Words in parentheses
in the help, such as `(after)`, are guide words: you may type them or leave
them out. Unless the table says otherwise, a command acts on the **target**,
the item under the cursor.

| Command | Form | Does |
| --- | --- | --- |
| `accept` | `accept N` | adopts the Nth suggestion shown, with its evidence |
| `adopt` | `adopt` | makes the target's claims beliefs, keeping their source |
| `check` | `check quote`, `check source` | checks the target's text against the document it came from, or its `doi` fact against Crossref |
| `claim` | `claim FACT from SOURCE` | the target states a claim: what SOURCE says |
| `close` | `close` | closes the supposition and returns to the actual world; it stays in the journal |
| `commit` | `commit` | makes the supposed world the actual one |
| `constraint` | `constraint NAME: false :- BODY.` | adds a constraint, after checking it fires on a planted violation |
| `copy` | `copy after ADDRESS` | copies the target's text to a new item after ADDRESS |
| `delete` | `delete` | deletes the target: a word, or an item and its branch (see Scope) |
| `derive` | `derive TEMPLATE where GOAL` | the target shows text computed from facts (§ 7) |
| `dismiss` | `dismiss N` | dismisses the Nth suggestion shown; it returns only if its evidence changes |
| `export` | `export FILE [NUMBER]` | exports the outline, or the section at item NUMBER, as a document |
| `fact` | `fact FACT` | the target states a belief |
| `fold` | `fold` | hides the target's branch |
| `follow` | `follow [N]` | goes to the other end of the target's Nth link (default 1) |
| `help` | `help` | lists every command on the message line |
| `import` | `import wikidata FILE`, `import document FILE`, `import results FILE` | imports a Wikidata subset, a document, or an analysis's results (§ 10, § 11) |
| `indent` | `indent` | makes the target a child of the item above it |
| `interrogate` | `interrogate` | asks of the target's claim: who else claims it, who claims the contrary, its source, the source's record, and what adopting it would do |
| `layer` | `layer MARK where GOAL` | adds a layer to the target's view: one mark for each solution of GOAL (§ 9) |
| `link` | `link [FROM] to TO [as TYPE]` | links two items; FROM defaults to the target, TYPE to `link`. `cites`, `supports` and `opposes` read well in the links pane |
| `move` | `move after ADDRESS` | moves the target's branch to follow ADDRESS |
| `name` | `name WORD` | names the target, so WORD addresses it; the name `title` also heads the outline |
| `new` | `new` | makes a new item after the target |
| `outdent` | `outdent` | makes the target a sibling of its parent |
| `pane` | `pane outline`, `pane links`, `pane suggestions` | sets what the other pane shows |
| `quit` | `quit` | writes what is pending and leaves |
| `redo` | `redo` | redoes what undo undid, along the current branch |
| `retract` | `retract ENTRY` | reverses one earlier entry, by its number, keeping later work; lists what depends on it first |
| `rule` | `rule HEAD :- BODY.` | defines a predicate from others, usable wherever a goal is: in `derive`, `constraint`, `layer` and other rules. Refused if recursive, if a head variable is not bound by the body, or if the body calls what a definition may not |
| `set` | `set FACT` | states FACT and retracts the beliefs it directly contradicts (each that breaks a constraint with it and nothing else); the fact goes on the item that held the one it replaces, else on the target. A violation it leaves is named, not resolved |
| `suppose` | `suppose NAME` | starts a named world with nothing supposed in it (the Suppose quasimode is the usual way, § 8) |
| `switch` | `switch` | moves focus to the other pane |
| `transclude` | `transclude from ADDRESS` | makes a new item after the target that shows ADDRESS's text in place |
| `unadopt` | `unadopt` | retracts the target's last adoption |
| `undo` | `undo` | undoes the last change on this branch |
| `unfold` | `unfold` | shows the target's branch |

**Undo and retract.** `undo` and `redo` move along the current branch of
the journal, as in any editor; undoing and then editing starts a new
branch, and the old one stays in the journal. To reverse one earlier
decision without losing what came after, `retract` it instead. The entry
number is shown in the interrogation pane, in the right pane, and in the
journal.

## 6. Addresses

Where a command takes an ADDRESS, any of these name an item:

- its number: `2`, or `1.4` for the fourth item under item 1;
- `#` and its entry number: `#28`;
- a name given by `name`;
- the start of its text, if only one item starts that way: `Energia`.

## 7. Facts, claims, constraints and derived text

Facts are Prolog terms: an atom or a compound, with no variables. Strings
go in double quotes. A trailing full stop is optional.

```
fact date(buran_flight, 1988)
fact label(buran_flight, "Buran's flight")
claim date(buran_flight, 1989) from my notes, conference talk
```

A `label(Entity, "words")` fact lets the memex write facts about Entity in
words, in marks, interrogation and Suppose's readings.

**Constraints** are denials. The name comes before the colon:

```
constraint one date per event: false :- date(E, D1), date(E, D2), D1 \= D2.
```

Before keeping it, the memex plants facts that should break it and checks
that it fires. A constraint that cannot fire is refused, with the reason.
Constraints are recomputed whenever the journal moves; only the parts that
depend on what changed are recomputed.

**Derived text.** `derive` makes the target show text computed from the
facts. In the template, `{Var}` is a variable of the goal, and `{N:years}`
writes a number of years in words ("a year", "two years"):

```
derive The flight came {N:years} after Energia's first launch. where date(buran_flight, B), date(energia_launch, E), N is B - E
```

`derive timeline where date(E, Y)` draws a timeline of the solutions, using
labels where there are any.

**Rules.** A computation used in several places can be written once, as
a rule, and then used like a fact:

```
rule spent(S) :- cell(rent, R), cell(food, F), S is R + F.
derive Spent: {S} where spent(S)
constraint within income: false :- spent(S), cell(income, I), S > I.
```

A rule may not be recursive, directly or through other rules (the
reasoner proves rules by plain recursion, which would not end); every
variable in its head must be bound by its body; and its body may call
only what a definition may (below). A violation found through a rule
rests on the facts under the rule, so its marks appear on the items you
would change.

**What a definition may call.** A constraint, a derived goal and an
adopted rule may only read facts, compare and compute. Besides your facts,
they may use:

- comparison and unification: `=`, `\=`, `==`, `\==`, `@<`, `@>`, `@=<`,
  `@>=`, `compare/3`, `=@=`, `\=@=`;
- arithmetic: `is`, `=:=`, `=\=`, `<`, `>`, `=<`, `>=`, `succ/2`, `plus/3`,
  `between/3`;
- type tests: `var`, `nonvar`, `atom`, `number`, `integer`, `float`,
  `string`, `atomic`, `compound`, `callable`, `is_list`, `ground`;
- terms: `functor/3`, `arg/3`, `=..`, `copy_term/2`;
- atoms and strings: `atom_length/2`, `atom_concat/3`, `sub_atom/5`,
  `atom_chars/2`, `atom_codes/2`, `char_code/2`, `atom_number/2`,
  `atom_string/2`, `number_codes/2`, `number_string/2`, `upcase_atom/2`,
  `downcase_atom/2`, `atomic_list_concat/2,3`, `string_concat/3`,
  `string_length/2`, `sub_string/5`, `string_chars/2`, `string_codes/2`,
  `string_code/3`, `string_lower/2`, `string_upper/2`, `split_string/4`;
- lists: `length/2`, `msort/2`, `sort/2,4`, `keysort/2`, `memberchk/2`;
- `true`, `fail`, `false`, and `\+`, `;`, `->`, `*->`, `findall/3,4`,
  `forall/2` around goals that are themselves allowed.

Anything else, such as `shell/1`, `open/3`, `assertz/1`, `format/2` or
`call/1`, is refused when the constraint or derived text is defined, with
what is not allowed. The reasoner checks again each time it calls one, so
a definition that arrives some other way cannot act either. `fact`,
`claim` and `adopt` refuse `node_def/2` and `rule_def/3`, the forms
definitions are stored in.

## 8. Worlds

Hold Suppose and type a statement in your own words, such as `the flight
was in 1989`. The memex reads it against the facts it knows and offers each
reading: `Do you mean: Suppose that the date of Buran's flight is 1989.`
Enter accepts; Tab shows the next reading; Esc cancels. If there is no
reading, releasing starts a named world with nothing supposed.

In the supposed world, each belief the supposition directly contradicts
is set aside, in that world only: the supposition wins. A belief is
directly contradicted when it and the supposition, and nothing else, break
a constraint (two dates for one event; two values for one cell). A
violation that rests on the supposition and two or more other beliefs is
left in place, and the message names it ("left for you: 1 violation of
…"): any of those beliefs could be given up, and which one is your
choice. The two worlds are shown row for row: derived text, timelines and
constraints, with what changed.

`close` returns to the actual world and keeps the supposition in the
journal. `commit` makes the supposed world the actual one.

## 9. Views

A view is an item that draws something: a chart, a timeline, a grid, a
flowchart, a tree. There are no chart types. A view is made of layers,
and each layer is a mark drawn once for each solution of a goal:

```
layer bar(x(C), y(V)) where cell(C, V)
```

This draws one bar for each `cell(C, V)` fact, at category C, V high.
Run `layer` with the cursor on the item that is to be the view; each
`layer` adds a layer. A layer without `where` is drawn once, as a
reference line: `layer rule(y(1500))`.

The view is drawn under its item and changes as the facts do. With the
cursor on it, the right pane lists its layers, with how many marks each
draws. A mark that rests on a fact in a violation is drawn bold (a grid
box gains `!`). With the mouse on (§ 4), a click on a mark goes to the
item that states its fact. In a supposition, each world draws its own
copy of every view, side by side. `retract` removes a layer, by its entry
number, shown in the right pane.

**Marks.** Each is written with its channels, `channel(Value)`:

| Mark | Channels: required; optional | Draws |
| --- | --- | --- |
| `dot` | x, y; text | a point, labelled |
| `bar` | x, y; text | a bar from zero: upright if x is a category, across if y is |
| `line` | x, y; series | points joined in order of x, one line and one symbol per series |
| `rule` | x or y | a line across the plot at that value |
| `text` | x, y, text | a label at a point |
| `tick` | x; y, text | a mark on the axis with its label above: a timeline. With y, on that lane |
| `span` | x1, x2; y, text | an interval from x1 to x2, on lane y: a Gantt chart |
| `cell` | x, y; text | a box in a grid: a table, a spreadsheet |
| `node` | id; text, shape | a box in a graph; shape `box`, `round` or `decision` |
| `edge` | from, to; text | an arrow from one node to another, by id |

**Scales** follow from the values. Numbers and dates (`date(Y, M, D)`)
are positions on a line; anything else is a category, in standard order
(numbers first, then by text), the first at the top or left. All the
layers of a view share their scales, so a rule and the bars line up. The
marks choose how the view is drawn: any `node` or `edge` makes a graph,
laid out in layers from top to bottom (edges that close a loop go back
up round the right side); `cell` marks alone make a grid; anything else
makes a plot. Marks that do not fit are named under the view.

**Recipes.** Each is ordinary facts and rules, with layers over them.

A bar chart with a limit:

```
layer bar(x(C), y(V)) where cell(C, V)
layer rule(y(I)) where cell(income, I)
```

A spreadsheet: boxes at places in a grid, one computed by a rule.

```
fact at(income, b, 1)
fact at(rent, b, 2)
fact at(left, b, 4)
rule value(B, V) :- cell(B, V).
rule value(left, L) :- cell(income, I), cell(rent, R), L is I - R.
layer cell(x(a), y(R), text(B)) where at(B, _, R)
layer cell(x(C), y(R), text(V)) where at(B, C, R), value(B, V)
```

A line chart, one line per series: `layer line(x(M), y(V), series(S))
where spend(S, M, V)`.

A timeline: `layer tick(x(Y), text(L)) where date(E, Y), label(E, L)`.

A Gantt chart: `layer span(y(T), x1(A), x2(B), text(T)) where task(T, A, B)`.

A flowchart:

```
layer node(id(S), text(T)) where step(S, T)
layer node(id(S), text(T), shape(decision)) where decision(S, T)
layer edge(from(A), to(B), text(L)) where next(A, B, L)
```

Your own outline as a tree. The outline is offered to goals as two kinds
of fact, `item_text(Item, Text)` and `item_under(Item, Parent)`:

```
layer node(id(I), text(T)) where item_text(I, T)
layer edge(from(P), to(C)) where item_under(C, P)
```

Links between items work the same way. `link 2 to 5 as supports` states
`supports(From, To)` between the two items, so with the node layer above,
`layer edge(from(A), to(B), text(supports)) where supports(A, B)` draws
an argument map.

**Refused.** A layer is refused, with the reason, if its mark or a
channel is unknown, a channel is missing or given twice, a variable in
the mark is not bound by the goal, or the goal calls what a definition
may not (§ 7).

## 10. Suggestions and Wikidata

The memex can read a subset of Wikidata, a public knowledge base, as
claims whose source is Wikidata. Make a subset with the `wikidata` tool
(§ 13), then:

```
import wikidata subset.pl
```

The file is copied into the journal's `blobs/` directory under its hash,
with its provenance (derived from Wikidata, under CC0). It must hold only
the facts the tool writes, with no variables: `wd(Item, Property, Value)`,
where Value is an atom, a number, a string or `date(Y, M, D)`;
`wd_label(Entity, "label")`; and `wd_constraint(Property, Kind)`. Any
other term is refused, named, and nothing is imported.

To tie your facts to Wikidata, two kinds of belief are needed:
`same_as(Entity, Q)` says your entity is Wikidata's item Q, and
`wd_property(Predicate, P)` says your predicate is Wikidata's property P.
Then Wikidata's statements become claims about your entities, and
constraints check them like any other.

The memex suggests these and more, without any language model:

- entity links, from Wikidata's labels;
- property mappings, where a linked entity's date sits beside yours;
- Wikidata's single-value constraints for mapped properties, each checked
  against a planted violation;
- rules found in your beliefs, such as `Q(X, Y) :- P(X, Y)`, with their
  exceptions;
- references, chosen only among works already in the store.

`pane suggestions` shows them, at most five, ranked by evidence, each with
why it was offered. The pane never takes the keyboard: `accept N` adopts
the Nth, with its evidence; `dismiss N` hides it until its evidence
changes. While the pane has focus, suggestions keep their places.

## 11. Documents and results

**Import a document** (anything Pandoc reads: Markdown, Word, OpenDocument,
LaTeX, HTML):

```
import document draft.docx
```

Headings become outline levels; paragraphs, list items, quotes, code,
display mathematics and tables become items; citations `[@key]` become
`cites` facts. What the outline cannot hold (footnotes, images, raw
blocks, definition lists, some attributes) is counted and reported, not
hidden.

**Import it again** after a co-author has revised it, under the same file
name, and the memex compares the new version with what it imported before.
It opens a supposed world with each change as one entry: changed text,
added items, deleted items. `retract N` drops a change; `commit` accepts
the rest; `close` rejects them all.

**Export** writes the outline, or one section, through Pandoc. The format
comes from the file's extension: `.md`, `.docx`, `.odt`, `.tex`, `.html`.

```
export paper.docx
export section.md 2
```

The document's metadata lists the hashes of the journal entries its text
came from.

An export holds words, structure, mathematics, citations and links, and
nothing that acts. Text that reads as an image (`![…](…)`), as raw HTML
or LaTeX (`<script>`, `\input{…}`), is left out, and a link whose target
is not `http`, `https`, `mailto` or `doi` keeps its text but loses its
target. Imports are read by Pandoc with `--sandbox`, so a document cannot
pull in another file (LaTeX `\input`, `\include`).

**Check a quote:** `check quote` on an imported item says whether its text
still appears in the document it came from, ignoring spaces, hyphens and
case.

**Check a source:** `check source` on an item with a `doi(Entity,
"10.…")` fact asks Crossref for that DOI and rules: found, misattributed
(the title differs from the one claimed), not found, unavailable, or
retracted.

**Import results** from an analysis. In Python, at the end of the
analysis:

```python
from memex_results import write_results
write_results("results.json",
              results=[("mean_delay", 1.52, 0.07, "s")],
              inputs=["data.csv"])
```

(`helpers/python/memex_results.py`). Then `import results results.json`.
Each value arrives as a claim whose source is the analysis, with its
standard uncertainty, unit and provenance: the script, its commit, the
command, and a hash of each input file. If an input file changes later,
the result is marked `input changed since:`. Only regular files are
read for this; anything else, such as a pipe or a device, counts as
changed.

## 12. Files

A journal directory holds:

| File | Holds |
| --- | --- |
| `journal.jsonl` | the entries, one JSON object per line, hash-chained |
| `seals.jsonl` | the sealer's signatures, dated |
| `keys.jsonl` | the public keys that signed seals |
| `blobs/` | imported files, each named by its SHA-256 hash |

The format is version 1, described in docs/design.md, § The journal
format. Text is stored in Unicode NFC.

The signing key is a separate file, readable only by you (mode 600). Keep
it out of the journal directory. If it is lost, the journal and its old
seals still check, because `keys.jsonl` holds the public half; `sealer init`
with a new key file adds a new key for new seals. (Read from the code; this
recovery has not been tested.)

**Purge** is the one way to remove content: it replaces an entry's body
with its hash, so the chain still checks but the content is gone. It is a
library call in this prototype (`Journal::purge`), with no command.

## 13. Tools

All run from the repository with `cargo run -q -p <crate> --`, or from
`target/release/` after `cargo build --release`.

| Program | Usage | Does |
| --- | --- | --- |
| `sealer` | `sealer init DIR KEYFILE` | makes a signing key and records its public half in DIR |
| | `sealer run DIR KEYFILE [--once] [--interval-ms N]` | seals new entries, every 200 ms by default; `--once` seals and exits |
| `check` | `check DIR` | checks a journal: the chain, every seal's signature, and the blobs. Prints `ok:` and counts, or the first fault; exits 1 on a fault |
| `wikidata` | `wikidata --items Q2548,Q15180 --out FILE` | writes those items of Wikidata as Prolog facts |
| | `wikidata --min-sitelinks K --limit N --out FILE` | the same for up to N items with at least K site links |
| `measures` | `measures DIR` | the measures report for the experiment, from the journal alone |
| `memex` | `memex [--config FILE] [--mouse \| --no-mouse] DIR` | the notebook; the flags override the configuration's `[mouse]` |
| | `memex --print-config` | writes the default configuration |

`make measures JOURNAL=…` runs the measures report on a journal. The
`wikidata` tool needs network access to query.wikidata.org.

## 14. Configuration

The memex reads a TOML file, the first of:

1. the file given by `--config FILE`;
2. the file named by `$ADVERSARIA_CONFIG`;
3. `$XDG_CONFIG_HOME/adversaria/config.toml`;
4. `~/.config/adversaria/config.toml`.

If the file is absent, the defaults apply. (A file given by `--config` must
exist.) To start, write out the defaults and edit them:

```bash
mkdir -p ~/.config/adversaria
```

```bash
cargo run -q -p memex -- --print-config > ~/.config/adversaria/config.toml
```

The same file is in the repository as docs/config.example.toml.

It has three sections. Anything you leave out keeps its default.

```toml
[quasimodes]
leap_forward = ["RightAlt", "IsoLevel3Shift"]
leap_back = ["LeftAlt"]
command = ["LeftControl"]
scope = ["RightControl"]
suppose = ["LeftSuper", "RightSuper"]

[keys]
F1 = "palette"
F2 = "interrogate"
F3 = "adopt"
F4 = "unadopt"
F6 = "switch"
F10 = "quit"

[mouse]
enabled = false
```

**`[quasimodes]`** gives each quasimode a list of keys (or one key, as a
string). Naming a quasimode replaces its default keys; each needs at least
one.

**`[keys]`** binds a key to a command line, exactly as you would type it in
Command, such as `"pane suggestions"` or `"export draft.md"`; or to
`"palette"`. A key named here replaces its default; `""` unbinds it.

**`[mouse]`** has one setting, `enabled`: `true` or `false` (the default).
See § 4, The mouse.

**Key names:**

- modifiers, for quasimodes only: `LeftAlt`, `RightAlt`, `LeftControl`,
  `RightControl`, `LeftSuper`, `RightSuper`, `LeftHyper`, `RightHyper`,
  `LeftMeta`, `RightMeta`, `IsoLevel3Shift` (AltGr on many layouts),
  `IsoLevel5Shift`;
- for quasimodes or bindings: `F1` to `F35`, `Insert`, `PageUp`,
  `PageDown`, `Menu`, `Pause`, `ScrollLock`, `PrintScreen`, `CapsLock`,
  `NumLock`.

The editing keys (Enter, Tab, Backspace, Delete, Esc, the arrows, Home,
End), Shift and printable characters cannot be configured.

The memex refuses to start on a file it would have to guess at, and says
which file and why: a section, quasimode, key or command it does not know,
an editing key, a quasimode with no keys, one key given two jobs, or a
`[mouse]` setting other than `enabled = true` or `false`. To move a key
from a binding to a quasimode, unbind it as well:

```toml
[quasimodes]
scope = ["F1"]

[keys]
F1 = ""
F12 = "palette"
```

Examples:

- A laptop whose desktop takes Super: `suppose = ["RightControl"]` and
  `scope = ["Menu"]`.
- Keys only, no modifiers: `leap_forward = ["F11"]`, `leap_back = ["F12"]`,
  `command = ["Insert"]`. Tap them to latch if holding a function key is
  awkward.

The hints on screen (such as "F3 adopts") follow your bindings.

## 15. Troubleshooting

- **"this terminal lacks the kitty keyboard protocol …"**: use kitty,
  WezTerm or Ghostty. Run it outside tmux or screen, which are not known
  to pass key releases and graphics through (recalled, not tested).
- **"the terminal did not answer a device attributes request within 2 s"**:
  the memex is not attached to a terminal, or the terminal is very slow.
- **A quasimode key does nothing:** your desktop or terminal may take it
  (Super often opens a launcher; Alt may open a menu). Rebind it (§ 14).
- **"memex: configuration: …"**: the file and the reason are named; fix the
  line it names. `memex --print-config` shows a file that works.
- **"no command …"**: `?` in Command lists the commands; `help` lists them
  all.
- **"not a Prolog fact" / "not a Prolog rule"**: check brackets, commas and
  quotes. Variables start with a capital letter; atoms with a small one.
- **A constraint is "not kept"**: it did not fire on the planted violation,
  so it could never fire. Check that the body can be true.
- **A message about Prolog, or no marks where you expect them:** SWI-Prolog
  must be on the path (`swipl --version`). `./dev-shell` provides it.
- **Plain `nix-shell` prints an error about `<nixpkgs>` and "uses bash
  from your environment":** nixpkgs is not on your `NIX_PATH`, or is there
  as a flake, which needs flakes enabled. `nix-shell` only wanted an
  interactive bash from it, and uses your own instead; the shell is
  otherwise complete. `./dev-shell` avoids the message.
- **`check` reports a fault:** it names the line and the fault. Do not edit
  the journal by hand; it is hash-chained, and any change breaks the chain
  from that line on.

## 16. Limits

This is a prototype, built for a one-month trial. Known limits:

- It has not been run in a real kitty, WezTerm or Ghostty. Tests drive it
  through its own key events and through a script that plays a kitty
  terminal.
- The quasimode keys are provisional; they have not been tried on real
  keyboards.
- Images are not shown, though the graphics protocol is required.
- Re-importing a document does not detect moved blocks; a move shows as a
  deletion and an addition.
- Seal dates are not shown in the memex, and `check` does not check that
  they are in order.
- Integers larger than 2^53 − 1 are refused in the journal.
- Only the Wikidata subset tool exists; the full subset is not built.
- Scope affects only `delete`.
- `set` writes each retraction and the new fact as separate journal
  entries, so one `undo` reverses only the new fact; reversing a whole
  `set` takes one `undo` per entry, or `set` again with the old value.

docs/log.md records every result and fault found so far.
