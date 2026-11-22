# Concepts, a second proposal: how far, and the small changes first

A proposal for the author, 22 November 2026, after the fifth review
(docs/research/notes/ux-review-5-regularity.md). Nothing here is built.

## The answer

As built, a learner holds about 76 items by docs/concepts.md's counting
rule (an estimate). The fifth review's nine changes bring it to about 65.
If the interface is not sacred, about 20 is reachable with no capability
lost, and a first day needs about 9. Four structural moves do it (§ How
far). Before them, twelve small changes make the command grammar regular
(§ The small changes first). On their own they lower the count only a
little, to about 71. They are worth doing either way, and the larger
moves rest on them.

All counts are estimates under concepts.md's rule. One assumption carries
the most weight: an item recognised on the screen costs a learner little,
and one recalled costs much. The research supports it as far as memory
goes. Word users recognised about 135 commands but used 27 to 50 (measured,
self-report and logs; research/concept-budget.md). Recognition still
costs time (Hick's law), so menus must stay short and ranked. The journal
can test the assumption (§ Measuring).

## The small changes first

The grammar as it should read, in seven rules:

1. A command is a verb, then what it acts on. With nothing named, it acts
   on the target, the item at the cursor.
2. An object word narrows what the verb acts on (`adopt world`). Without
   one, what the target is decides.
3. A bare number is an item. A number after an object word is the number
   shown beside that object on the screen.
4. A guide word may always be typed, or left out. An argument's name is
   shown as a placeholder, `‹text›`, and is never typed.
5. Words in quotes are text.
6. What each verb takes is data: one table drives parsing, `?`, Tab and the
   guide, and a test holds them together.
7. Every offer can be adopted or dismissed. Every act that takes back what
   you hold asks first.

Where the memex breaks a rule today, and the small change that mends it
(each checked in the code; none loses a capability):

| # | Rule broken today | Small change | Capability | Cost |
| --- | --- | --- | --- | --- |
| 1 | 6: a verb declares at most one slot (`dispatch.rs`), so `?` after `adopt ` cannot list `world`, `fix`, `suggestion`, and Tab cannot complete them | each verb lists its object words; `?` lists them, Tab completes them, the guide's table is tested against them | kept; forms become recognisable | small |
| 2 | 4: the line shows `(to)` and `(after)`, which may be typed, beside `(what)`, `(text)`, `(goal)` and `(mark)`, which must not be; `as` is required in `link` and `show`, and `that` is shown for `suppose` | guide words in parentheses, argument names as `‹placeholders›`; every guide word may be typed or left out, and is needed only where two texts would run together | kept | small |
| 3 | 3: the suggestions' hint teaches `adopt N`, which runs only by falling through to the old `accept`; a revision's change is shown as `entry 52` and taken as `change 52`; the menu offers `#12` both as an entry and as an item | the hint says `adopt suggestion N`; a change is shown as `change 52`; the built-in pattern stops offering `go #12` | kept | small |
| 4 | 3: `export FILE NUMBER` takes only an item's number | `export FILE ADDRESS`: any address, a label or a text's start | kept, and gains labels | small |
| 5 | 5: `find violations` and `go next` reserve words, so the text "violations" or an item called "next" cannot be named | words in quotes are always text, for every argument | kept, and gains | small |
| 6 | 1: `go` falls back to text search, which Leap and `find` already do, so the same words mean an address in one place and a search in another | `go` takes an address or a way; when none matches, the foot offers `find WORDS` | kept: Leap and `find` search | small |
| 7 | 7: `adopt` takes claims, a suggestion, a fix and a world; `dismiss` takes a suggestion, a world and a change; `instead` is a manner, not an object | `adopt change N` (keep it, marked reviewed) and `dismiss fix` (hide it until the finding changes); `instead` becomes a choice in the confirmation of rule 7 | kept; dismiss gains | small to medium |
| 8 | 7: `adopt instead` retracts beliefs without asking, though `retract` asks | adopting a claim that contradicts beliefs asks: Enter beside them, Tab in place of them, Esc keeps it a claim | kept | small |
| 9 | 2: `import` needs `document`, `results` or `wikidata`, and `check` needs `quote` or `source`, though the file and the item already say which | the kind words narrow; without one, the file's extension or the item's provenance decides | kept | small |
| 10 | 1: `copy after ADDRESS` copies the target's text; `transclude from ADDRESS` copies the address's text after the target, keeping where its characters came from, and is not live (measured) | `copy [FROM] after TO`, FROM optional as in `link`, always keeping provenance; `transclude` becomes an old name | kept; a copy gains provenance | small to medium |
| 11 | 1: in `show 2 as chart` the address chooses where the view is drawn, while the chart is of every box | `show ADDRESS as chart` charts that item's facts, through an outline fact `item_states(Item, Fact)`; `show as chart` alone charts every box, as now | kept, and gains | medium |
| 12 | 1: the right-click shows what acts on each thing, but only to the mouse | a key, Menu by default, opens the same menu at the cursor | kept; every offer reachable from the keyboard | small |

Changes 1, 2 and 12 matter most. They make the grammar something the
memex can show, not something the learner must remember.

## How far

Four structural moves, each keeping every capability:

**1. Object first, then action.**
- **How:** one gesture acts on anything: the Menu key at the cursor, or a
  right-click under the pointer (change 12). It offers the verbs that
  apply to that thing, each with its key. A typed command stays the fast
  path, found by its name with `?`.
- **Effect:** the 17 verbs and 27 object words move from recall to
  recognition, and the menu teaches the keys, as it already names them
  (`adopt its claims  (F3)`).
- **Borrows:** the presentation-based interfaces (Symbolics Genera, CLIM);
  the path from menus to shortcuts that Grossman measured.
- **Held:** about −40: 19 verbs, 18 object words, and 3 address forms,
  since pointing fills an argument in.
- **Risk:** long menus. Keep each to about seven offers, ranked, the most
  likely first.

**2. Editing by habit.**
- **How:** an empty item deleted by Backspace; Ctrl+X, Ctrl+C and Ctrl+V on
  items, through the chords Ctrl+Z already uses (a cut item pasted is
  moved, and keeps its identity); drag to move.
- **Effect:** `new`, `delete`, `move`, `copy`, `undo` and `redo` need not
  be found even in the menu. The habits are held already.
- **Held:** no further change: move 1 already counts these verbs.

**3. Seven concepts in place of twelve.**
- **item.**
- **link**, which absorbs label. `#draft` is a link to `draft`; its
  backlinks are what `find #draft` lists, and "a label one item alone
  carries" becomes the item of that name. The targets can be virtual,
  so no item is made for each tag.
- **statement.**
- **offer**, which absorbs claim, suggestion, finding, document and
  revision. An offer is anything put to you for a decision, with its
  source: a talk, a paper, Wikidata, a co-author's version, or the
  memex itself (its suggestions, and its findings, which offer fixes).
  `import` reads a source; `adopt` and `dismiss` decide offers. This is
  CLAUDE.md's own rule made the whole model: everything you did not
  write is a claim to test.
- **world.**
- **history**, which absorbs entry and trail: every change and every
  jump is an entry; undo, redo and retract walk your changes, back and
  forward your jumps.
- **view.**

`find` stops being a concept. Leap finds text, and `?` while holding it
lists every match. The inspector of the notebook itself (the cursor
above the first item) lists everything open: offers, findings and
claims to decide.
- **Held:** −5.

**4. One reader for every line.**
- **How:** the Command line reads by form. A command word first runs the
  command; a question is answered once; a statement is supposed. "Do
  you mean …?", which Suppose already asks, serves every text that does
  not read, as a fix offered on the item: "not read: did you mean the rent
  is 1600? adopt fix".
- **Effect:** `suppose`, and `find` with a question, are no longer verbs to
  hold. The sentence forms are recognised from the readings offered.
- **Held:** −2: one statement form, and one rule of the command shape.
- **Risk:** a line that is both a command and a sentence. A command name
  first always wins, and the foot says which it read.

The result, by concepts.md's rule (estimates):

| Held in recall | Now | After the twelve small changes | After the four moves |
| --- | --- | --- | --- |
| concepts | 12 | 12 | 7 |
| verbs | 20 | 19 | 0 (recognised; typed by experts) |
| object words not concept names | 23 | 18 | 0 (recognised) |
| command shape and its rules | 2 | 2 (one regular rule set) | 1 |
| statement forms | 6 | 6 | 5 (Prolog in backticks is the tail) |
| address forms | 4 | 4 (one rule for numbers) | 1 (Leap; numbers are seen) |
| quasimodes, tap, and the act key | 3 | 4 | 4 |
| `?` and the in-quasimode keys | 4 | 4 | 1 (Enter, Esc and Tab are habits) |
| Shift with an arrow; right-click | 2 | 2 | 1 |
| **total** | **about 76** | **about 71** | **about 20** |

From 71 to 20: move 1 removes 40, move 3 removes 5, and move 4 removes 2.
The last 4 come from counting Enter, Esc, Tab and the right-click as the
habits they are, as concepts.md's rule allows.

A first day: item, statement, offer and world; typing, Enter and Tab;
Leap; the act key; and F3, which the screen shows. About 9.

## What it costs, and what could go wrong

- **Speed.** Experts lose none: every typed command and key stays, and the
  menu shows each key, so experts learn them from use.
- **Journal format.** Moves 1, 2 and 4 write the entries the commands
  write now. A label as a link needs no format change if its targets are
  virtual; making them items would be a revision. Change 11 adds an
  outline fact, `item_states/2`, which is computed and never stored.
- **Raskin.** The design keeps quasimodes and the presentations. A menu
  opened on an object is transient, not a mode, but it is a menu, which
  Raskin argued against for experts. Typed commands remain for them.
- **Order.** The twelve small changes stand alone. Move 1 depends on
  changes 1 and 12, move 3 on change 7, and move 4 on change 8.

## Measuring

The journal already records each command and each click as the action it
stands for. For each user, count:
- the share of actions started from the menu against those typed;
- the distinct verbs typed each week;
- the time from a verb's first use from the menu to its first use typed
  or by its key.

If recognition costs as little as assumed, the menu share starts high and
falls, typed and keyed use rise, and errors (refused commands) fall. If
the menu share stays high and slow, the long menus are costing more than
the counting rule allows.
