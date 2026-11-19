# User guide

This guide describes the Adversaria prototype as built: the memex, which is
the notebook you use in a window, and the programs around it. For a first
session, follow docs/tutorial.md. For why it is built this way, see
docs/design.md; for what has been tested and what has not, docs/log.md.

Contents, four parts of four:

1. Concepts: the record, the outline, facts claims and beliefs,
   constraints and worlds.
2. Working: starting, the screen, keys and quasimodes, commands and
   addresses.
3. Reasoning: facts rules and constraints, worlds, views, suggestions and
   Wikidata.
4. Around the memex: documents and results, files and tools,
   configuration, troubleshooting and limits.

## 1. Concepts

### 1.1 The record

**Journal.** Everything you do is an entry in an append-only journal: a
file of JSON lines, each joined to the one before by a hash. Nothing is
ever overwritten or erased, except by `purge` (§ 4.2). There is no Save:
what you type is written a second after you stop, or when you do anything
else. Undo is an entry too.

**Seal.** A separate program, the sealer, holds a signing key and signs
the journal's entries, dated. The memex itself never sees the key. A sealed
entry can be shown to have existed, unchanged, by its date.

### 1.2 The outline

**Outline.** Your notes are an outline of items. Each item has text, may
have items under it, and may state facts.

**Quasimode.** A state that lasts only while you hold a key, such as
Command: hold the key, type a command, release to run it. You cannot forget
you are in it. See § 2.3.

### 1.3 Facts, claims and beliefs

**Fact, claim, belief.** A fact is a Prolog term an item states, such as
`date(buran_flight, 1988)`. A fact you state is a belief. A fact from a
source (a talk, a paper, Wikidata, an analysis) is a claim:
`claims(Source, Fact)`. A claim becomes a belief only when you adopt it,
and the adoption keeps its source.

### 1.4 Constraints and worlds

**Constraint.** A rule that must never be broken, written as a Prolog
denial: `false :- ...`. When beliefs break it, that is a **violation**. When
a claim and a belief, or two claims, would break it, that is a
**disagreement between sources**. Both are marked under the items involved.

**World.** The facts as they stand are the actual world. A supposition
opens a second world in which something else holds; the two are shown side
by side. Closing a world keeps it in the journal; committing it makes it
the actual world.

## 2. Working

### 2.1 Starting

The memex needs:

- a desktop under X11 or Wayland: the memex opens a window of its own and
  draws through Vulkan or OpenGL;
- SWI-Prolog (`swipl`) on the path, for reasoning;
- Pandoc, for importing and exporting documents;
- curl, for the source check.

`./dev-shell` gives a shell with all of these, Mesa's drivers included, and
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

### 2.2 The screen

- **Top bar:** the journal directory's name (`room`), the world (`world
  actual`, or `worlds actual | suppose: …`) and the number of journal
  entries (`journal`). On the right, a count of violations and
  disagreements; when there are none, which pane has focus (`pane 1 of
  2`). In a narrow window the right side takes a row of its own.
- **Left pane:** the outline. `▼` marks an item with items under it, `▶` a
  folded one (followed by how many items it hides), `•` an item with none.
  Top-level items show their number. Under an item, `⚑` lines are its
  marks: each constraint solution that rests on one of its facts. A
  violation is shown bold; a disagreement dim. An item that shows
  derived text ends in `[derived]`; one that states a claim ends in its
  source, in brackets. A result whose input files have changed says `input
  changed since:` and the files. Text the memex has read is cyan; under a
  question, `→` gives its answer. Under the item at the cursor, a dim line
  says why its text was not read, or which of its `[[links]]` name no item
  yet (§ 2.4, § 3.1).
- **Right pane:** what the selected item needs. By default its links; on an
  item with marks, the constraint, its rule, its solutions and what depends
  on them; on a view, its layers. Under the links or the constraint,
  `READ AS` shows what the item's text was read as, in Prolog, and its tags
  (§ 3.1). After `interrogate`, the pane shows the interrogation. `pane`
  sets it to the outline, links or suggestions.
- **Foot:** the message line, and, while a quasimode is held, what you have
  typed and what releasing will do.

A fading hint appears under an item with marks until you have used Suppose
four times.

### 2.3 Keys and quasimodes

The defaults are below. All of them can be changed (§ 4.3).

#### Editing

| Key | Does |
| --- | --- |
| a printable character | types into the item under the cursor |
| Enter | makes a new item after the cursor's item |
| Tab | makes the item a child of the one above it (`indent`) |
| Shift-Tab | makes the item a sibling of its parent (`outdent`) |
| Backspace, Delete | erase a character before or after the cursor |
| Up, Down | move to the item above or below |
| Left, Right, Home, End | move within the item's text |

#### Quasimodes

| Quasimode | Default keys | While held | On release |
| --- | --- | --- | --- |
| Leap forward | Right Alt (AltGr) | type a pattern | the cursor goes to the next item holding it |
| Leap back | Left Alt | type a pattern | the cursor goes to the previous item holding it |
| Command | Left Ctrl | type a command line | the command runs on the item under the cursor |
| Scope | Right Ctrl | Space widens the target: word, statement, branch | the target stays chosen for the next command |
| Suppose | Super (either) | type a statement; the line below says what may come next in controlled English, whether it is a sentence of it, and if it breaks, where and how to mend it | the memex offers its readings of it (§ 3.2) |

Within a quasimode:

- **Tap to latch.** Press and release the key with nothing typed between,
  and the quasimode stays on without holding. Enter, or a second tap, ends
  it. Backspace on an empty latched quasimode leaves it without acting.
- **Esc completes:** a command name in Command, a pattern in Leap. In
  Suppose it completes the word being typed when one word alone fits, or
  mends: it puts the repair the line below offers first in place.
- **`?` lists** the commands that match (Command), every match (Leap), or
  everything that may come next, each kind of word with a line of help
  (Suppose). In Suppose, `?` lists only at the start or after a space:
  typed straight after a word it ends a question ("Does John wait?").
  `?` again hides the list; what you typed stays.
- **Repeat a Leap:** a second tap of a latched, empty Leap repeats the last
  pattern.
- **Leap or make:** in Leap, Enter goes to the match, as release does; if
  no item holds the pattern, Enter makes a new item holding it, after the
  cursor's item, and puts the cursor at its end. Searching for a note and
  making one are one gesture. Releasing on no match makes nothing, so a
  mistyped pattern is only reported. The new item is read at the pause,
  as typed text is.
- **Focus lost while a key is held** (as by Alt+Tab): the release goes to
  another window, so the memex does not wait for it. A quasimode held with
  nothing typed is left without acting; Scope puts back the target it
  found. One held after you typed stays open, latched, with what you
  typed, and says so: Enter ends it, or leave it as any latched one.

Leap matches anywhere in an item's text, ignoring case, and wraps round
at either end of the outline.

Scope matters only to `delete` in this prototype: with the target at word,
`delete` removes the word at the cursor; at statement or branch, it deletes
the item and its branch. The target returns to statement after each
command.

#### Function keys and confirmations

F1 opens the command palette: Command, latched, with the list of
commands open. F10 runs `quit`. F2, F3 and F4 question a claim, adopt it
and take the adoption back, and F6 switches panes; each runs the command
of that name (§ 2.4), and § 4.3 lists every default.

**Confirmations.** Two commands ask before acting, at the foot of the
screen:

- `retract` lists the entries that depend on the one you retract. Enter
  retracts; Esc keeps it.
- Suppose offers readings of your statement: `Do you mean: …`. Enter
  accepts; Tab shows the next reading; Esc supposes nothing.

#### The mouse

The mouse is off unless you turn it on, with `enabled = true` in the
configuration (§ 4.3), or with `--mouse` for one run.
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

The mouse is off by default. That was decided while the memex ran in a
terminal, whose own text selection the mouse got in the way of; the window
has no text selection, and the default stands until the author changes it.

### 2.4 Commands and addresses

Type a command in the Command quasimode, or bind it to a key (§ 4.3). A
unique prefix is enough: `interr` runs `interrogate`. Words in parentheses
in the help, such as `(after)`, are guide words: you may type them or leave
them out. Unless the table says otherwise, a command acts on the **target**,
the item under the cursor.

Where a command takes an ADDRESS, any of these name an item:

- its number: `2`, or `1.4` for the fourth item under item 1;
- `#` and its entry number: `#28`;
- a name given by `name`;
- the start of its text, if only one item starts that way: `Energia`.

#### Editing the outline

| Command | Form | Does |
| --- | --- | --- |
| `new` | `new` | makes a new item after the target |
| `delete` | `delete` | deletes the target: a word, or an item and its branch (see Scope) |
| `indent` | `indent` | makes the target a child of the item above it |
| `outdent` | `outdent` | makes the target a sibling of its parent |
| `make` | `make` | makes an item for each of the target's `[[links]]` that names no item, at the foot of the top level, and links it; one `undo` takes it all back |
| `move` | `move after ADDRESS` | moves the target's branch to follow ADDRESS |
| `copy` | `copy after ADDRESS` | copies the target's text to a new item after ADDRESS |
| `split` | `split` | splits the target's text at each sentence end (`.`, `?` or `!`, a space, then a capital, a digit, a quote or a bracket; never after an abbreviation that comes before what it shortens, such as `e.g.`, `Dr.` or `p.`, or an initial such as `J.`; `etc.`, `et al.`, `Jr.`, `Sr.`, `ed.`, `eds.` and `St.` end one only before a capital letter): the target keeps the first sentence, with its links; each other becomes a new item after it, linked back as `split_from`, its characters kept from the target's text, so their history stays. Each part is read at the pause. One `undo` takes the whole split back |
| `transclude` | `transclude from ADDRESS` | makes a new item after the target that shows ADDRESS's text in place |
| `link` | `link [FROM] to TO [as TYPE]` | links two items; FROM defaults to the target, TYPE to `link`. `cites`, `supports` and `opposes` read well in the links pane |
| `name` | `name WORD` | names the target, so WORD addresses it; the name `title` also heads the outline |
| `follow` | `follow [N]` | goes to the other end of the target's Nth link (default 1) |
| `fold` | `fold` | hides the target's branch |
| `unfold` | `unfold` | shows the target's branch |
| `pane` | `pane outline`, `pane links`, `pane suggestions` | sets what the other pane shows |
| `switch` | `switch` | moves focus to the other pane |
| `help` | `help` | lists every command on the message line |
| `quit` | `quit` | writes what is pending and leaves |

**Tags.** A `#` at the start of a text or after a space, then a word,
tags the item: `Buran #reusable #Soviet-space` gives `item_tag(Item,
reusable)` and `item_tag(Item, soviet_space)` (lower case, hyphens made
underscores, as the reader writes names). Tags follow the text: there is
nothing to write or take back. Views, derived text and constraints read
them, as `constraint not both draft and final: false :- item_tag(I,
draft), item_tag(I, final).` does. `C#`, `#1`, a heading's `# ` and a web
address's `/#anchor` are not tags.

**Links typed in text.** Write `[[Buran flight]]` in an item's text and,
at the pause, the item links to the item named so, else to the first
whose text it is (ignoring case). A link that names no item makes
nothing: at the cursor a dim line says `not yet items: Buran flight ·
make makes them`, and `make` makes each at the foot of the top level and
links it; one `undo` takes that back. If you make such an item yourself
(by typing it, or with Leap and Enter), the link is made at the pause;
if you give an item the name with `name`, at once. A name moved to
another item takes its links with it; one `undo` takes a name and its
links back. Take the brackets out, and the link goes. The reader reads
the text without its brackets, so `The date of [[Buran-flight]] is
1988.` is still a fact.

#### Facts and definitions

| Command | Form | Does |
| --- | --- | --- |
| `fact` | `fact FACT` | the target states a belief |
| `claim` | `claim FACT from SOURCE` | the target states a claim: what SOURCE says |
| `rule` | `rule HEAD :- BODY.` | defines a predicate from others, usable wherever a goal is: in `derive`, `constraint`, `layer` and other rules. Refused if recursive, if a head variable is not bound by the body, or if the body calls what a definition may not |
| `ace` | `ace SENTENCE` | shows what APE, the parser of controlled English, makes of SENTENCE: its paraphrase, or where it fails and APE's advice. Each entity of the current world is a word for it, as are the names of one-place facts (adjectives) and two-place facts (nouns) and labels of one word (§ 3.2) |
| `set` | `set FACT` | states FACT and retracts the beliefs it directly contradicts (each that breaks a constraint with it and nothing else); the fact goes on the item that held the one it replaces, else on the target. A violation it leaves is named, not resolved |
| `constraint` | `constraint NAME: false :- BODY.` | adds a constraint, after checking it fires on a planted violation |
| `derive` | `derive TEMPLATE where GOAL` | the target shows text computed from facts (§ 3.1) |
| `layer` | `layer MARK where GOAL` | adds a layer to the target's view: one mark for each solution of GOAL (§ 3.3) |
| `check` | `check quote`, `check source` | checks the target's text against the document it came from, or its `doi` fact against Crossref |

#### Claims and suggestions

| Command | Form | Does |
| --- | --- | --- |
| `adopt` | `adopt` | makes the target's claims beliefs, keeping their source |
| `unadopt` | `unadopt` | retracts the target's last adoption |
| `interrogate` | `interrogate` | asks of the target's claim: who else claims it, who claims the contrary, its source, the source's record, and what adopting it would do |
| `accept` | `accept N` | adopts the Nth suggestion shown, with its evidence |
| `dismiss` | `dismiss N` | dismisses the Nth suggestion shown; it returns only if its evidence changes |
| `import` | `import wikidata FILE`, `import document FILE`, `import results FILE` | imports a Wikidata subset, a document, or an analysis's results (§ 3.4, § 4.1) |
| `export` | `export FILE [NUMBER]` | exports the outline, or the section at item NUMBER, as a document |

#### Worlds and history

| Command | Form | Does |
| --- | --- | --- |
| `suppose` | `suppose NAME` | starts a named world with nothing supposed in it (the Suppose quasimode is the usual way, § 3.2) |
| `close` | `close` | closes the supposition and returns to the actual world; it stays in the journal |
| `commit` | `commit` | makes the supposed world the actual one |
| `undo` | `undo` | undoes the last change on this branch |
| `query` | `query GOAL` | runs a Prolog goal over this world's beliefs, through rules, as derived text does: `query animal(X)` says `2 answers: X = nemo · X = dory`; a goal without variables is answered `yes` or `no`. At most 20 answers are shown. The goal may only read facts, compare and compute, as a definition may; a query writes nothing to the journal |
| `redo` | `redo` | redoes what undo undid, along the current branch |
| `retract` | `retract ENTRY` | reverses one earlier entry, by its number, keeping later work; lists what depends on it first |

**Undo and retract.** `undo` and `redo` move along the current branch of
the journal, as in any editor; undoing and then editing starts a new
branch, and the old one stays in the journal. To reverse one earlier
decision without losing what came after, `retract` it instead. The entry
number is shown in the interrogation pane, in the right pane, and in the
journal. A `set` writes each retraction and the new fact as separate
entries, so one `undo` reverses only the new fact; reversing a whole
`set` takes one `undo` per entry, or `set` again with the old value.

## 3. Reasoning

### 3.1 Facts, claims, constraints and derived text

**Write in controlled English, and the memex reads it.** When you pause
(a second without keys), each item you wrote whose text is ACE is read as
the statements it makes, and its text turns cyan. Nothing asks you to
confirm, and nothing is written twice:

```
The date of Buran-flight is 1988.
Talk-notes says that the date of Buran-flight is 1989.
The income is 2000.
The rent is 1200.
If the income is X and the rent is Y then the rest is X - Y.
It is false that the rest is smaller than 0.
```

These are a fact, a claim from a source, two boxes, a rule and a
constraint. With an item selected, the right pane shows, under `READ AS`,
each statement its text was read as, in Prolog, as the memex runs it
(`fact  date(buran_flight, 1988)`, `rule  rest(A):-cell(income, B), …`),
and its tags (`tag  item_tag(12, money)`). A text read before the memex
kept these forms shows none until its text changes. Change the text, and
what it said is taken back and the new text read; undo takes back a
sentence and what it said together. Text that is not ACE stays prose:
at the cursor, a dim line says where APE stopped (`prose: not ACE, it
stops at …`). A text is read whole or not at all: ACE the notebook cannot
state says `not read:` and why.

Ask in English, too: `What is an animal?`, `Who swims?`, `What is the
income?`, `Is Nemo an animal?`. After the pause, the answer is drawn
under the question (`→ dory, nemo`, `→ yes`, or `→ none`) and changes
as the facts do. `Which things are animals?` works too: ACE has no
plural after "are", so the memex asks it as `Which things are an
animal?`, which means the same; it does so for questions only. A thing
with a label is answered by it: with `The label of Nemo is "Nemo the
clownfish".`, the answer is `→ Nemo the clownfish, dory`. A thing, as in
"which things", is anything at all. "A fish is an animal." is ACE for
one fish with no name; write `Every fish is an animal.` for all fish, or
name one: `Nemo is a fish.` The table in docs/design.md (§ Reading
controlled English) lists what each kind of sentence is read as.

The commands below state the same things in Prolog.

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

A goal with no solution shows the template and `(no answer)`. A goal that
raises an error shows the template and what went wrong, such as `(cannot
answer: type_error(evaluable, foo / 0))`.

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

### 3.2 Worlds

Hold Suppose and type a statement in your own words, such as `the flight
was in 1989`. The memex reads it against the facts it knows and offers each
reading: `Do you mean: Suppose that the date of Buran's flight is 1989.`
Enter accepts; Tab shows the next reading; Esc cancels.

While you type, the line below checks the statement as controlled English
(ACE), with the words of your notebook, as if it ended with a full stop:

- `ACE:` and how APE, its parser, reads it back, when it is a sentence;
- `next:` and what may come next, when it is not yet one but can go on:
  your notebook's words first, then kinds of word (`a noun`, `a name`,
  `a verb`, ...), then words such as `the`, `of` and `is`. While you type
  a word, only those it begins are shown, and when one alone fits, Esc
  completes it (`Esc: buran_flight`);
- `not ACE:` when it cannot go on: the sentence with `<>` where parsing
  stopped, and up to three edits of one word that make it ACE or let it
  go on, your notebook's words first, such as `Esc: was → is` (ACE has
  only the present tense). Esc puts the first in place. With no such
  edit, it names the words ACE does not know.

When you mend free English into ACE and suppose it, the memex keeps the
pair: what you first typed, and the ACE. Later, when you type something
that matches an earlier free English but for a name or a number or two
(and the earlier ACE holds those words), the line below offers the
earlier ACE with your new words in place, `as before: … · Esc`, and Esc
puts it in place. The pairs are in the journal, with the suppositions.

A word is judged only once it ends, with a space or an end mark. To see
everything that may come next, not only the first twelve, type `?` (at
the start, or after a space): the notebook's words, each kind of word
with what it is (`a name — one thing by its name: John, buran_flight`),
and the words of ACE. A fact written as
Prolog, such as `cell(fuel, 80)`, is read exactly as you wrote it, even
for something the memex has no facts about. If there is no reading,
releasing starts a named world with nothing supposed; if what you typed
is believed already, it starts one with nothing changed, and says so.

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

### 3.3 Views

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
box gains `!`). With the mouse on (§ 2.3), a click on a mark goes to the
item that states its fact. In a supposition, each world draws its own
copy of every view, side by side. `retract` removes a layer, by its entry
number, shown in the right pane. A layer whose goal raises an error is
noted under the view, with its entry number and what went wrong, such as
`(layer 10 cannot answer: type_error(evaluable, foo / 0))`.

#### Marks

Each mark is written with its channels, `channel(Value)`:

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

#### Scales and frames

Scales follow from the values. Numbers and dates (`date(Y, M, D)`)
are positions on a line; anything else is a category, in standard order
(numbers first, then by text), the first at the top or left. All the
layers of a view share their scales, so a rule and the bars line up. The
marks choose how the view is drawn: any `node` or `edge` makes a graph,
laid out in layers from top to bottom (edges that close a loop go back
up round the right side); `cell` marks alone make a grid; anything else
makes a plot. Marks that do not fit are named under the view.

#### Recipes

Each is ordinary facts and rules, with layers over them.

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

Your own outline as a tree. The outline is offered to goals as four kinds
of fact, `item_text(Item, Text)`, `item_under(Item, Parent)`,
`item_tag(Item, Tag)` for each `#tag` in an item's text, and
`item_link(From, To, Type)`, one for each link, of any type, typed in text
or made by `link`:

```
layer node(id(I), text(T)) where item_text(I, T)
layer edge(from(P), to(C)) where item_under(C, P)
```

Links between items work the same way. `link 2 to 5 as supports` states
`supports(From, To)` between the two items, so with the node layer above,
`layer edge(from(A), to(B), text(supports)) where supports(A, B)` draws
an argument map. Every link at once, the graph of your notes:

```
layer node(id(I), text(T)) where item_text(I, T), (item_link(I, _, _) ; item_link(_, I, _))
layer edge(from(F), to(T)) where item_link(F, T, _)
```

#### Refused

A layer is refused, with the reason, if its mark or a
channel is unknown, a channel is missing or given twice, a variable in
the mark is not bound by the goal, or the goal calls what a definition
may not (§ 3.1).

### 3.4 Suggestions and Wikidata

The memex can read a subset of Wikidata, a public knowledge base, as
claims whose source is Wikidata. Make a subset with the `wikidata` tool
(§ 4.2), then:

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
- references, chosen only among works already in the store;
- links for unlinked mentions: an item whose text names another item's
  text, as whole words in any case, and does not link to it. Only short
  texts are taken as names (3 to 60 characters, six words at most, not
  ending in `.`, `?` or `!`). Accepting it writes the link.

`pane suggestions` shows them, at most five, ranked by evidence, each with
why it was offered. The pane never takes the keyboard: `accept N` adopts
the Nth, with its evidence; `dismiss N` hides it until its evidence
changes. While the pane has focus, suggestions keep their places.

## 4. Around the memex

### 4.1 Documents and results

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
added items, deleted items, and items moved under another heading (when a
heading changes its level, for one). `retract N` drops a change; `commit`
accepts the rest; `close` rejects them all. Your own items stay where you
put them.

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

### 4.2 Files and tools

#### Files

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

##### Purging

Purge is the one way to remove content: it replaces an entry's body with
its hash, so the chain still checks but the content is gone. It is a
library call in this prototype (`Journal::purge`), with no command.

#### Tools

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

### 4.3 Configuration

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
See § 2.3, The mouse.

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

### 4.4 Troubleshooting and limits

#### Troubleshooting

- **"memex: no window: …"**: the memex found no display. Run it in a
  desktop session under X11 or Wayland (`DISPLAY` or `WAYLAND_DISPLAY`
  set).
- **"memex: cannot draw: …"**: no Vulkan or OpenGL driver was found. Inside
  `./dev-shell` Mesa's are used. With NVIDIA's own driver outside NixOS,
  run the memex through nixGL (recalled, not tested).
- **A quasimode key does nothing:** your desktop may take it
  (Super often opens a launcher; Alt may open a menu). Rebind it (§ 4.3).
- **"sealer: the clock went back: it says …, before the last seal's date,
  …; no seal written"**: the system clock is set to a time before the last
  seal, and the sealer stopped rather than write a seal that `check` would
  refuse. Set the clock right, or wait until it passes that date, and start
  the sealer again; the entries written meanwhile are sealed then.
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
- **"the journal's last line was unfinished, as a crash leaves it":** the
  memex stopped, or the machine lost power, while it wrote an entry. The
  entry was never finished, so it was never kept: the memex keeps the line
  in `journal.jsonl.torn`, writes the journal without it, and opens the
  room. Only a last line with no newline, or with zero bytes in it, is set
  aside; any other fault is refused, as `check` would report it.
- **"… is open in another memex"**: a room is open in one memex at a time.
  Close the other, or wait for it to end.

#### Limits

This is a prototype, built for a one-month trial. Known limits:

- It has been run only under test: through its own key events, and by a
  script that types into its window under Xvfb (X11) and a headless sway
  (Wayland). Not on a real desktop or GPU, and not with a real keyboard
  under Wayland.
- The quasimode keys are provisional; they have not been tried on real
  keyboards.
- Images are not shown: views are drawn in characters.
- Re-importing a document does not detect a block moved past others; it
  shows as a deletion and an addition. A block that keeps its place among
  the others but falls under another heading is moved.
- Seal dates are not shown in the memex. (`check` refuses a seal dated
  before the seal before it.)
- Integers larger than 2^53 − 1 are refused in the journal.
- Only the Wikidata subset tool exists; the full subset is not built.
- Scope affects only `delete`.

docs/log.md records every result and fault found so far.
