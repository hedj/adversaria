# Concepts: a budget, and the memex factored to fit it

A proposal for the author, 22 November 2026. The author took all seven
decisions (§ Decisions for the author), and they were built the same
day; § Status says what was built and where it differs.

## The answer

The memex asks a learner to hold about 231 separate mechanisms (the
fourth UX review's count, `research/notes/ux-review-4-mechanisms.md`).
The literature puts what one person holds in recall near **50 items**,
over months of daily use, and what one act may combine near **4**
(`research/concept-budget.md`). This proposal factors the memex into
**12 concepts and 17 verbs in one grammar**. Under the report's
counting rule a learner then holds about **49 items**, and needs **5
concepts on the first day**. Each of the memex's tasks is traced below
to the smaller set, and no capability is dropped; the rarer ones move
to a tail that is found by recognition (`?`, the list, the
right-click), not held in recall.

The budget is an estimate from measured figures, not a measurement of
this tool; the journal can test it (§ Measuring).

## The budget

| What is counted | Budget | Evidence (`research/concept-budget.md`) |
| --- | --- | --- |
| interacting elements in one act | at most 4 | Cowan's 3–5 chunks; Halford: five-way at chance (measured, in the lab) |
| purpose-level concepts | 10–15 | derived from the held vocabulary (estimate) |
| concepts before the first useful task | 4–5 | the interaction limit; Grossman's ceiling of 10 new mappings at once (estimate) |
| items held in recall, in all | at most about 50 | 27–50 commands per user in Word, AutoCAD, GIMP, Unix (measured, logs) |
| the recognised tail | no cap | Word users recognised about 135 (measured, self-report) |

The counting rule (the report's table): a concept counts 1; a verb or an
object word in a regular grammar counts 1 each, not 1 per pair; a command
shape counts 1; one name with two meanings counts more than 2; a mode
held in the head counts 1 in every act under it; an exception counts 1
and never becomes routine; one-way aliases cost about nothing; screen
regions and indicators the user sees are recognised, not recalled;
habits the user already has (editing keys, click and drag, Ctrl+Z)
cost nothing new.

## The user's purposes

From the introduction's three uses, design.md's user story and
CLAUDE.md:

1. Write and organise notes.
2. Find and get to things, and back.
3. Connect things.
4. Say what the notebook can reason about.
5. Keep what you hold apart from what a source says.
6. Notice conflicts and bad sources when they arrive.
7. Try "what if?" without changing what you hold.
8. Lose nothing, and prove the record.
9. See items as a chart, table or graph.
10. Exchange with documents and data outside.

## Twelve concepts

Each has one purpose, its own state and actions, and an operational
principle, in Jackson's sense. The concept's name is also its object
word in the grammar.

| # | Concept | Purpose | Actions | Operational principle |
| --- | --- | --- | --- | --- |
| 1 | **item** | 1 | `new`, `delete`, `move`, `copy`; typing; Enter, Tab, Shift+Tab | write an item, and it keeps its place and address until you move it |
| 2 | **link** | 3 | `link`; `[[text]]` in an item | link A to B, and each shows the other; a link of type `shows` puts B's text in A (transclusion) |
| 3 | **label** | 1, 2 | `#word` in an item's text | label items `#draft`, and `find #draft` lists them; a label one item alone holds addresses it (as `name` did) |
| 4 | **statement** | 4 | write a sentence in an item; a formal one in backticks | type `The rent is 1200.`, and it turns cyan: the notebook reads it; its meaning shows beside it |
| 5 | **claim** | 5 | `adopt`, `dismiss`, `retract` | what a source says stays its claim, checked but not believed, until you adopt it; a suggestion is a claim whose source is the memex |
| 6 | **finding** | 6 | `check`; `adopt` or `dismiss` a fix | write `Nothing has more than 1 date.` once, and every later fact that breaks it is marked under the items involved |
| 7 | **world** | 7 | `suppose`, `adopt`, `dismiss` | suppose something, and the two worlds stand side by side; adopt it and it becomes actual, dismiss it and the actual world is as it was |
| 8 | **entry** | 8 | `undo`, `redo`, `retract` | every change is an entry; any entry can be taken back later by a new one, and the sealer dates and signs each |
| 9 | **trail** | 2 | `go back`, `go forward` (F7, F8) | a jump (Leap, `go`, a click) can be gone back from, as in a browser |
| 10 | **find** | 2 | Leap; `find`; `go` | hold Leap and type, and the cursor goes to the match; `find` lists what matches a text, a label, a kind of finding or a question |
| 11 | **view** | 9 | `show … as …` | show an item's facts as a chart, a table or a graph, drawn from the world's facts |
| 12 | **document** | 10 | `import`, `export` | import a file as items and claims; import it again, and its changes open as a world, one entry each |

What this merges, by Jackson's moves:

- **Statements are written, not commanded.** `fact`, `claim`, `rule`,
  `constraint`, `ace`, `derive` and `query` each wrote one kind of
  statement by a command of its own. Each becomes a sentence in an item,
  which the reader already reads. Prolog that ACE cannot say is written
  in backticks: `` `false :- date(E, A), date(E, B), A \= B.` ``. A
  question asked without keeping it (`query`) is `find` with the
  question. (Redundancy removed: one purpose, seven concepts.)
- **A suggestion is a claim.** Its source is the memex, or Wikidata, and
  adopting and dismissing it are the claim's actions. `accept` goes.
  (Unify.)
- **Adopt and dismiss mean one thing everywhere: hold it, or set it
  aside.** For a claim, a suggestion, a fix of a finding, a supposed
  world (`commit`) and a revision's change. `commit` and `close` go, and
  so does `set`, which becomes the fix "adopt this, retracting that"
  offered on a disagreement. (One verb, one meaning, many objects.)
- **One list.** The matches pane, the suggestions pane, the list of a
  constraint's findings and a revision's changes are each `find` with a
  filter. (Jackson's Todo and Label: one query serves every "list all
  X".)
- **One inspector.** The right pane shows what is known about the target:
  its links (and backlinks), its findings, what its text was read as, its
  claims and who else says them (what `interrogate` showed), its view's
  layers. It never takes the keyboard. `pane`, `switch` and `interrogate`
  go; two outline panes side by side move to the tail.
- **One verb for going.** `go` takes an address, a link (`go link 2`), a
  relation (`go next`, `go parent`, …) or the trail (`go back`). `follow`,
  `back`, `forward`, `next`, `previous`, `first`, `last`, `parent` and
  `child` go as commands; their keys stay (Shift with an arrow, F7, F8).
- **Fixes are offered, not commanded.** `make` (items for `[[links]]` to
  nothing) and `split` (an item of several sentences) become fixes on the
  findings "not yet items" and "several statements in one item".
- **Labels address.** `name` goes; a label one item alone holds is its
  name (`#title` heads the outline).

## One grammar

One command shape: **verb, then objects, each after its guide word**,
as now: `link 3.2 (to) source 9 (as) cites`. The 17 verbs:

| Verb | Objects | Opposite |
| --- | --- | --- |
| `new` | item | `delete` |
| `delete` | item, word | `new` |
| `move` | item (to) place | |
| `copy` | item (to) place | |
| `link` | item (to) item (as) type | |
| `go` | address, link, relation, back, forward | |
| `find` | text, label, finding kind, question | |
| `adopt` | claim, suggestion, fix, world, change | `dismiss` |
| `dismiss` | the same | `adopt` |
| `retract` | entry (or, with none, the target's last) | `adopt` |
| `undo` | the last change | `redo` |
| `redo` | the last undone | `undo` |
| `suppose` | a statement | |
| `import` | document, results, Wikidata (the kind from the file) | `export` |
| `export` | outline, section (to) file | `import` |
| `check` | claim (its quote, its source) | |
| `show` | item (as) chart, table, graph | |

Addresses, three forms: an item's number (`1.2`), its text's start or
its label (`Energia`, `draft`), and an entry (`entry 12`). Bare numbers
always mean items, so `#` means a label alone (it now also meant an
entry).

Statements, five sentence forms and one formal form:

| Form | Example | Kind |
| --- | --- | --- |
| a fact | `The rent is 1200.` | belief |
| what a source says | `Talk-notes says that the date of Buran-flight is 1989.` | claim |
| if, then | `If the rent is X and the food is Y then the cost is X + Y.` | rule |
| what must never be | `Nothing has more than 1 date.` | constraint |
| a question | `What is the rest?` | answered under it, kept current |
| Prolog, in backticks | `` `cell(rent, 1200).` `` | as Prolog says |

## The input

Two quasimodes, as now: **Leap** (forward and back) and **Command**,
each held or tapped. Inside either: Enter runs, Esc cancels, Tab
completes, `?` lists. The Suppose key opens Command with `suppose `
typed. Function keys are shortcuts shown beside their commands in the
list (F1 the list, F3 adopt, F4 retract, F7 and F8 the trail), learnt by
seeing them. The mouse: a click selects or follows, a drag selects, and
a right-click offers what can act on what is under it.

## The count

Recalled items, by the counting rule:

| What | Items |
| --- | --- |
| concepts (their names are the object words too) | 12 |
| verbs | 17 |
| the command shape, and the guide-word rule | 2 |
| statement forms | 6 |
| address forms | 3 |
| quasimodes, and tap to latch | 3 |
| Enter, Esc, Tab and `?` in a quasimode | 4 |
| Shift with an arrow moves by structure | 1 |
| a right-click offers what acts on this | 1 |
| **total** | **49** |

Not counted, as the rule says: what the screen shows (the top bar, the
outline, the inspector, the foot, the marks, cyan for read text), habits
already held (typing, arrows, Backspace, click, drag, Ctrl+Z), the
shortcut keys (read beside their commands), and the tail.

The tail, found by recognition, held only by those who use it:

- the view grammar (10 marks, 10 channels, 3 frames: `show … as …`
  offers them, with `?` and examples);
- link types beyond `link`, `cites`, `supports`, `opposes`, `shows`;
- `purge`, configuration, plumbing patterns, two outline panes, the
  `wikidata` and `measures` tools, the sealer's commands.

**Day one**, five concepts: item, statement, claim, finding, world; and
about ten keys and commands: type, Enter, Tab, Leap, Command, `?`, Esc,
F3 (adopt), Ctrl+Z, `suppose`. That is the tutorial's first part.

**One act.** The busiest act, `link A (to) B (as) T`, combines a verb and
three arguments: 4, at the limit. Pointing supplies an argument from the
screen, so it need not be held. In a supposed world the world is shown
on every row (side by side) and in the top bar, so it is seen, not held.

## The 231, recounted

| The review's category | Count | Becomes | Held |
| --- | --- | --- | --- |
| quasimodes and their uses | 7 | Leap, Command; hold, tap | 3 |
| gestures in a quasimode, confirmations | 11 | Enter, Esc, Tab, `?`; confirmations ask with Enter and Esc | 4 |
| editing keys | 7 | habits | 0 |
| function keys | 8 | shortcuts, read beside their commands | 0 |
| commands and sub-forms | 57 | 17 verbs | 17 |
| syntax conventions | 8 | one shape, one guide-word rule | 2 |
| argument and address forms | 8 | three address forms | 3 |
| screen regions | 6 | four, seen | 0 |
| contents of the right pane | 10 | one inspector, one list | (concept 10) |
| indicators on items | 17 | seen | 0 |
| kinds of item text | 14 | six statement forms; the rest are views and imports | 6 |
| links | 10 | link, `[[ ]]`, type `shows` | (concept 2) |
| worlds | 6 | world; adopt, dismiss | (concept 7) |
| views | 28 | the tail | 0 |
| kinds of suggestion | 6 | claims whose source is the memex | 0 |
| mouse | 14 | click, drag, right-click (habits), pointing | 1 |
| plumbing | 10 | the right-click; patterns in the tail | 0 |
| configuration | 4 | the tail | 0 |
| concepts | — | 12 | 12 |
| Shift with an arrow | — | one rule | 1 |
| **total** | **231** | | **49** |

## Each task, composed

| Task (source) | With the smaller set |
| --- | --- |
| write, restructure notes | type; Enter, Tab, Shift+Tab; `move`, `copy`, `delete` |
| find a place (story 1) | hold Leap, type `dra`, release |
| list everything about a topic | `find #topic`, or `find buran` |
| connect two items | `link 3.2 (to) source 9 (as) cites`, or `[[Source 9]]` in the text |
| see what links here | the inspector, at the item |
| show another item's text in place | `link 4 (to) 2 (as) shows` |
| record a fact | type `The date of Buran-flight is 1988.` |
| record what a talk said (story 2) | type `Talk-notes says that the date of Buran-flight is 1989.` |
| add a rule, a constraint | type `If … then …`, `Nothing has more than 1 date.` |
| see a conflict when it arrives (story 2) | marked under both items; the inspector says why |
| question a claim | the inspector, at the claim: who else says it, what adopting breaks |
| adopt a claim, take it back | F3; F4 (or `retract`) |
| try the other date (story 3) | `suppose the flight was in 1989`, Enter at "do you mean?" |
| keep or drop the supposition | `adopt world`, `dismiss world` |
| state one fact and drop what it contradicts (`set`) | adopt the fix offered on the disagreement |
| ask a question once | `find What is the rest?` |
| keep an answer current | type `What is the rest?` in an item |
| undo; take back an old change | Ctrl+Z; `retract entry 12` |
| a co-author's revision (use 2) | `import d.docx` again: a world of changes; `adopt` or `dismiss` each, then the world |
| check a quote or a source | `check` at the claim |
| a budget model (use 3) | sentences in items; a question for the rest; a constraint for income |
| a chart of the budget (story 4) | `show 2 (as) chart` |
| suggestions (links, rules, references) | `find suggestions`; F3 or `dismiss` |
| Wikidata, an analysis's results | `import subset.pl`, `import results.json`: claims |
| export an essay | `export essay.md` |
| prove a note's date | the sealer, unchanged |
| go back after following a link | F7 |
| make items for `[[links]]` to nothing | adopt the fix on the finding "not yet items" |

No task is left without a route. The capabilities kept only in the tail
are listed above; none is removed.

## Each command of the guide, and where it goes

| Command | Becomes |
| --- | --- |
| `new`, `delete`, `move`, `copy`, `link`, `go`, `adopt`, `dismiss`, `retract`, `undo`, `redo`, `suppose`, `import`, `export`, `check` | verbs, as now |
| `indent`, `outdent` | Tab, Shift+Tab; `move` |
| `fold` | a click on an item's bullet, or its key; `fold` an alias |
| `transclude` | `link … (as) shows` |
| `name` | a label one item alone holds |
| `make`, `split` | fixes offered on findings |
| `follow`, `back`, `forward` | `go link N`, `go back`, `go forward`; F7, F8 |
| `next`, `previous`, `first`, `last`, `parent`, `child` | `go next` and so on; Shift with an arrow |
| `fact`, `claim`, `rule`, `constraint`, `ace`, `derive` | sentences in items; backticks for Prolog |
| `query` | `find` with a question |
| `set` | the fix "adopt this, retracting that" on a disagreement |
| `layer` | `show … (as) …`, the view grammar in the tail |
| `interrogate` | the inspector, at a claim |
| `accept` | `adopt` (a suggestion) |
| `commit`, `close` | `adopt world`, `dismiss world` |
| `pane`, `switch` | the inspector and `find`; two outline panes in the tail |
| `help` | `?` and F1, the list |
| `quit` | F10, or closing the window (a habit) |
| `purge` | the tail |

Every name in the left column stays as a one-way alias.

## What it changes

- **Commands.** 48 become 17 verbs. The old names stay as one-way aliases
  (`commit` runs `adopt world`), so tests, habits and the tutorial keep
  working while the guide teaches the 17. Aliases cost the learner about
  nothing and help finding (Furnas).
- **The journal format.** No change: the verbs write the entries the old
  commands wrote (`adopt world` writes the `world` view entry `commit`
  wrote). Old `name` entries are still read, as labels.
- **Renames that end collisions** (the review's): "rule" for definitions
  only (the view mark becomes `refline`, plumbing rules "patterns");
  "finding" for a line under an item, "mark" for a view's alone; boxes
  stored as `box(Name, V)` (a journal format revision); Leap up and down;
  `entry 12` for entries.

## Measuring

The journal records every command, so the budget can be tested rather
than assumed. For each user, count from the journal, not by asking:
distinct verbs per session and per week; the share of use taken by the
top ten; commands first used after a suggestion; and whether each is
used again after 1, 7 and 30 days. If the factoring works, a user's
vocabulary levels off near 30–50 while the tasks done keep growing.

## Decisions for the author

1. Statements only as sentences (and backticks), with the statement
   commands kept as aliases. This is the largest cut; it rests on the
   reader, which is built.
2. A suggestion as a claim whose source is the memex.
3. `adopt` and `dismiss` for worlds and revision changes.
4. One inspector without focus, and `find` for every list.
5. `go` for every movement; labels in place of names.
6. The view grammar in the tail, behind `show`.
7. The renames, including `box`, which needs a journal format revision.

Each is its own series of commits, with the guide and tutorial changed
in the same series, and the count above checked against the guide after
each.

## Status

Built on 22 November 2026, one commit for each change (ea1d81e to
72e34c5 on the branch, with one merge of main between). Counted from crates/memex/src/actions/dispatch.rs:
50 commands, of which 20 are listed and taught (the 17 verbs, and
`transclude`, `fold` and `switch`) and 30 run only as old names, typed
whole. docs/tutorial.md and docs/guide.md teach the listed ones; the
guide's last command table gives each old name and what replaces it.

Where it differs from the proposal above:

- `transclude` and `fold` stay listed: a link of type `shows` does not
  yet transclude, and neither a key nor a click on a bullet folds.
  `switch` stays listed, for two outlines side by side.
- `adopt` takes no revision change; changes are kept with `adopt world`,
  and one is rejected with `dismiss change N`.
- `set` is not a fix offered on a disagreement: `adopt instead`, at a
  disputed claim, adopts it and retracts what it contradicts, and a fact
  is changed by editing its sentence.
- `split` is not a fix of a finding: no finding names an item of several
  sentences, so a right-click on one offers the split, as before.
- `derive timeline` has no form in an item's text, which needs a blank;
  it stays as the old command's form.
- F2 is no longer bound: the inspector asks a claim's questions at the
  claim's item, with no key. The measures report, which counted
  `interrogate` as checking, now also counts a move whose view entry
  records the claim item the inspector shows (`inspects`). This changes
  what "checking" measures, from a key pressed to questions shown; the
  author may want to judge it.
- Two journal format revisions (docs/design.md): a view entry's `op` may
  be `find`, and it records `inspects`; and a box is stored as
  `box(Name, Value)`, an older journal's `cell(Name, Value)` read as one.

