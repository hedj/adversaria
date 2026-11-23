# Concepts, a second proposal: how far, and the small changes first

A proposal for the author, 22 November 2026, after the fifth review
(docs/research/notes/ux-review-5-regularity.md). The grammar is built,
and so are the lists of each verb's words, most of move 2 (editing by
habit), move 1 (the right-click's verbs) and move 4 (one reader for
every line) (§ Built, and the count now); move 3 is not.

## The answer

As built, a learner holds about 76 items by docs/concepts.md's counting
rule (an estimate). The fifth review's nine changes bring it to about 65.
If the interface is not sacred, about 20 is reachable with no capability
lost, and a first day needs about 9. The author's rule for the grammar
(§ The grammar) does most of it: a command is a verb and nouns, the
first noun is the selection, and the rest are prompted for with
defaults. The proposal expected that to take the count to about 48. As
built, with each verb's words listed where they are used, it is about
53, just over the budget of about 50 (§ Built, and the count now). Four
structural moves take it the rest of the way (§ How far). Twelve
small changes, written before the rule, are kept below; the rule makes
most of them follow (§ The small changes, without the grammar).

All counts are estimates under concepts.md's rule. One assumption carries
the most weight: an item recognised on the screen costs a learner little,
and one recalled costs much. The research supports it as far as memory
goes. Word users recognised about 135 commands but used 27 to 50 (measured,
self-report and logs; research/concept-budget.md). Recognition still
costs time (Hick's law), so menus must stay short and ranked. The journal
can test the assumption (§ Measuring).

## Built, and the count now

The author took both recommendations of § The grammar (the cursor's item
is a selection of its kind; Shift selects and Ctrl moves by structure).
Built on 22 November 2026, one change a commit:

| What | Commit |
| --- | --- |
| Shift with an arrow selects; Ctrl with one moves by structure | dd8ad27 |
| a Leap (the other Leap key) and a drag of the mouse select | 4b7e484 |
| a verb alone takes its nouns' defaults, or asks for a noun with none | 3793314, c0e839a |
| typed alone, a command takes the selection for its first noun | a3a28f5 |
| `import` reads the kind from the file's ending | 1782194 |
| `?` after a verb lists the words its first noun may be; Tab completes | 3b9e805 |
| the tutorial teaches the grammar where it is first used | 5a68a1f |
| move 2, editing by habit: Backspace deletes an empty item; Ctrl+X, Ctrl+C and Ctrl+V | ea2ead5 |
| move 1: the right-click offers every verb that applies to an item; the Menu key and Shift+F10 open it at the cursor | 9329041, f4ca719; plumbing's item test, 6657e87 |
| move 4: a Command line with no command first is read by its form; an item that does not read offers APE's repair, and `adopt fix` takes it | fada7b5, 6cde612 |

Where the build differs from § The grammar:

- **Defaults only for a verb alone.** Typed with words, a command runs as
  before, so every line typed in full, every key bound and every old
  journal is unchanged. A latched Enter does not step through the
  prompts one by one: it takes every default and stops at the first noun
  with none.
- **Numbers are not yet read by kind.** `retract 12` is entry 12 and
  `adopt 3` suggestion 3 because those verbs read them so. The address
  form "a number after an object word" stays.
- **Not built:** `retract`'s prompt as its confirmation (it still asks in
  a step of its own); `adopt`'s (in place of) prompt (`adopt instead`
  stays, now listed); `go` as a place only (it still looks for text);
  `copy` and `transclude` as one verb; `show 2 as chart` charting what
  item 2 states; `export` taking an item. `export` alone asks for its
  file rather than default to one, since a default name could overwrite
  a file.
- **Several items selected:** only `delete` takes them, and each is
  deleted by an entry of its own, since the journal has no entry that
  deletes several; undo takes them back one at a time, and the message
  says so. Any other verb that acts on one item refuses.
- **Found, not fixed:** Ctrl held while a letter is pressed twice (Ctrl+Z,
  Z) types `zz`, which is no command. It was so before the build.

The count now, by concepts.md's rule (estimates; the boundaries are
mine, as in the fifth review):

| Held in recall | Before the build | Now |
| --- | --- | --- |
| concepts | 12 | 12 |
| verbs | 20 | 20 |
| object words not concept names | 23 | 0: each is listed by `?` after its verb, with what it means, and Tab completes it |
| command shape and its rules | 2 | 2: the shape (the first noun is the selection; the rest are asked for, with defaults) and the guide-word rule |
| statement forms | 6 | 6 |
| address forms | 4 | 4 |
| quasimodes and tap | 3 | 3 |
| `?` and the in-quasimode keys | 4 | 4 |
| keys with an arrow; right-click | 2 | 2: Shift selects, a habit; Ctrl with an arrow moves by structure; right-click |
| **total** | **about 76** | **about 53**; with move 2, **about 48** |

Move 2 makes five verbs habits: `new` (Enter, and Backspace takes an
empty item back), `move` (Ctrl+X, Ctrl+V), `copy` (Ctrl+C, Ctrl+V),
`undo` and `redo` (Ctrl+Z, Ctrl+Y). `delete` stays a verb to recall,
since an item with text, or a branch, still needs it; drag to move is
not built. The count falls from about 53 to about 48, within the budget
of about 50.

Move 1 makes the verbs recognised: the right-click on an item offers
`link`, `move`, `copy`, `new`, `show`, `transclude`, `adopt fix`,
`check`, `suppose`, `find` by label, `retract` and `delete`, where each
applies, beside what the item offers already (its claims adopted, its
adoption taken back, split, fold). Plumbing offers `go`, `find` and
`import` on the text pointed at; a suggestion's, a change's and a
world's presentations offer `adopt` and `dismiss`; `undo` and `redo` are
habits since move 2, and `switch` has its key beside it in the list.
Only `export` is offered nowhere, so one verb stays in recall. With the
proposal's other assumption for this move, that pointing fills an
argument, three address forms leave too. The count falls from about 48
to about 31 (48 − 14 − 3), an estimate that rests on both assumptions.

Move 4 lets a statement or a question be typed as it is, with no verb:
one rule of the command shape (a sentence needs `suppose` or `find`
before it) and one statement form (the question asked by `find`) leave
recall, as the proposal counted them. The count falls from about 31 to
about 29. A line with no reading is refused rather than made the name of
a world, so `suppose` before a text is still how a world is named.

All 23 of the fall to 53 come from the lists, and rest on one assumption: that
a list shown by `?` after the verb is recognition, as the fifth review's
first suggestion assumed. The grammar itself removes nothing by the rule.
It changes how a noun is given, not what must be recalled; its gain is
fewer words typed and fewer refusals, which the journal can measure
(§ Measuring). The budget is about 50.

### The rest of the way

From about 29 to about 20, each move keeping every capability (§ How far
gives each in full). The rows for moves 2, 1 and 4 are what was
proposed; all three are built, move 2 but for drag to move, and move 1
leaving `export` in recall:

| Move | Held after it | Cost | What it needs |
| --- | --- | --- | --- |
| 2. Editing by habit: Backspace deletes an empty item; Ctrl+X, Ctrl+C and Ctrl+V on items and text; drag to move | about 47 as proposed, with `new`, `delete`, `move`, `copy`, `undo` and `redo` habits; about 48 as built, since `delete` stays | small to medium | three chords on the path Ctrl+Z uses, and a clipboard of the session's. A cut marks its items until they are pasted, as a spreadsheet's cut does, so a moved item keeps its identity and its entries |
| 1. The right-click offers every verb that applies to what is under the pointer, ranked, each with its key, beside its object's commands and plumbing's, which stay; the Menu key, the keyboard's own right-click, opens the same menu at the cursor | about 30: the other 15 verbs, `delete` among them, and 3 address forms, since pointing fills an argument | large | for each verb, a test of whether it applies and a rank; the menu itself exists (the right-click's) |
| 4. One reader for every line | about 29 | medium | a line that is no command but reads as a statement is supposed, a question is answered, and the foot says which, before Enter. Risk: a mistyped command read as a statement, so only a line with a reading is taken |
| 3. Seven concepts | about 24 | large | labels as links and the five kinds of offer change the guide's vocabulary throughout; labels as links need a decision on the journal's meaning |
| Enter, Esc, Tab and the right-click counted as habits | about 20 | none | a decision on the counting rule, for the author |

Recommended order: 2, 1 and 4 (built), then 3. Move 2 is the cheapest,
and costs nothing to learn, since the keys are habits already. Move 1
removes the most. Move 3 changes what the learner is told the notebook
is, so it is the author's to choose, and it is best done last, when the
other moves have shown which concepts users reach for.

## The grammar

The author's rule, 22 November 2026:

> A command is a verb and nouns. The first noun is replaced with the
> target if one is selected (via mouse-selection, key-selection, or
> quasimode). Following arguments are prompted for, with defaults.

Unfilled arguments are prompted for. So every command line works the same
way:

1. **Select, or not.** The selection is what you last chose: a range
   dragged with the mouse, a range selected with the keys, a span leapt
   over (Leap to one end, then the other), or a row of the inspector
   clicked. If it is of the first noun's kind, it is the first noun.
2. **Type the verb.** The memex prompts for each noun still unfilled, one
   at a time, on the Command line: `link 2.1 (to) ‹item› [1.3]`. The guide
   word is in brackets, the noun's kind in angle brackets, and its default
   in square brackets.
3. **Answer each prompt.** Type it, point at something of its kind (as
   pointing at an argument does now), press Tab to complete, or `?` to list
   the candidates. Enter takes the default; Esc abandons the command.

A noun's kind decides how what you type is read. At an entry's prompt,
`12` is entry 12; at an item's prompt, item 12; at a text's prompt, every
word is text. So a number needs no rule of its own, and no word needs
quoting.

Each listed verb, its nouns in order, and their defaults:

| Verb | First noun, from the selection | Prompted, with its default |
| --- | --- | --- |
| `new` | a place (an item) | after the selection; with none, at the end |
| `delete` | an item, or a text range | — |
| `move` | an item, or a text range | (after) a place: the item last jumped from |
| `copy` | an item, or a text range, keeping where it came from | (after) a place: after itself |
| `link` | an item | (to) an item: the item last jumped from; (as) a type: `link` |
| `go` | a place: a clicked row, a selected address, or `next`, `parent` and the other ways | with nothing selected: back along the trail |
| `find` | a text, a label or a question: the selected words | the last thing found |
| `adopt` | an offer: a claim's item, a suggestion's row, a change's row, a fix's line, a world's heading | if it contradicts beliefs, (in place of) those beliefs: none, so it is adopted beside them |
| `dismiss` | an offer, as for `adopt` | — |
| `retract` | an entry: a presented fact, layer, change, constraint or adoption | the target's last that can be retracted; the prompt lists what depends on it, so it is the confirmation |
| `undo`, `redo` | — | — |
| `suppose` | a statement: the selected one, to edit | the target's statement |
| `import` | a file: a selected path | (as revision of) an item, asked only if the file was imported before: that import |
| `export` | an item: its section | (to) a file: the item's text, with `.md` |
| `check` | an item | what to check, asked only if both apply: both |
| `show` | an item | (as) a kind: `chart`; or, in the tail, a mark (where) a goal |
| `fold` | an item | — |

What it makes unnecessary, from the twelve small changes below:

- **Object words.** `world`, `fix`, `suggestion`, `change`, `entry`,
  `link`, `back`, `instead`, `word` and the kind words of `import`,
  `check` and `find` become kinds of noun. A noun is filled from the
  selection, or recognised from the prompt's list. Changes 3, 5, 7 and 9
  follow from this.
- **Guide-word rules.** The prompt shows each guide word, and you never
  type one. Change 2 is half done by this; the other half is showing
  kinds as placeholders.
- **A separate confirmation.** `retract`'s prompt shows what it will take
  back, and Enter takes the default; `adopt`'s prompt asks whether to
  adopt in place of what a claim contradicts. Change 8 follows.
- **`go` as a search.** `go`'s noun is a place; text is `find`'s noun.
  Change 6 follows.
- **The first noun is the selection.** `copy` and `transclude` become one
  verb (change 10); `show 2 as chart` charts what item 2 states
  (change 11); `export` takes an item (change 4).

What remains needed: the verbs' nouns, kinds and defaults as one table in
the code (change 1, now the core of the mechanism), and the Menu key as
the keyboard's route to the right-click's menu (change 12).

The count, by concepts.md's rule (estimates):

- The object words leave recall. The prompt shows each noun's kind, its
  default and its candidates.
- The command shape becomes one rule, and the address forms one.
- So the count falls from about 76 to about 48, within the budget of
  about 50, with the twelve concepts and the verbs as they are.
- The four moves of § How far then take it to about 20.

Two consequences are for the author to decide:

1. **Is the cursor's item a selection?** If it is, the item verbs never
   prompt for their first noun, as the target works now. `go`, `find` and
   `suppose`, whose first noun is a place or a text, then need the rule
   that a selection fills a noun only if it is of the noun's kind. The
   cursor's item is not a place to go to, nor a text to find. Recommended:
   yes, with that rule, which the presentations already follow when you
   point at an argument.
2. **Which keys select?** Shift with an arrow is how most programs extend
   a selection, but here it moves by the outline's structure (`go next`
   and the rest).
   - (a) Shift with an arrow selects; the structural moves go to Ctrl
     with an arrow, through the chords Ctrl+Z already uses. Ctrl+Up and
     Ctrl+Down move by paragraph in most editors, near enough to "the
     next item at this level".
   - (b) The structural moves keep Shift; keys select only by leaping
     over a span.
   Recommended: (a), since selecting with Shift is a habit and costs
   nothing to learn.

## The small changes, without the grammar

Before the author set the rule above, I proposed the grammar as seven rules, and twelve changes where the memex breaks them. With the rule, most of them follow from it; they are kept here for the reasons each gives:

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
- **How:** one gesture acts on anything: the right-click, which exists
  and is held already. Its menu keeps both kinds of offer it has now:
  the commands for the object presented under the pointer (for an item:
  question, adopt, take back, split, fold), and plumbing's, after Plan
  9's plumber, for the text selected or the word under the pointer (go
  to an address, import a file named, answer a Prolog goal, and the
  user's own patterns). The move adds every verb that applies to the
  object, each with its key, beside those. The Menu key, which PC
  keyboards have for this job, opens the same menu at the cursor, so the
  keyboard reaches every offer too (change 12). A typed command stays the fast
  path, found by its name with `?`.
- **Effect:** the 17 verbs and 27 object words move from recall to
  recognition, and the menu teaches the keys, as it already names them
  (`adopt its claims  (F3)`).
- **Borrows:** the presentation-based interfaces (Symbolics Genera, CLIM);
  the path from menus to shortcuts that Grossman measured.
- **Held:** about −40: 19 verbs, 18 object words, and 3 address forms,
  since pointing fills an argument in.
- **Risk:** long menus. Keep each to about seven offers, ranked, the most
  likely first: plumbing's matches, which are the most specific to what
  was pointed at, before the verbs that apply to any item.

**2. Editing by habit.**
- **How:** an empty item deleted by Backspace; Ctrl+X, Ctrl+C and Ctrl+V on
  items, through the chords Ctrl+Z already uses (a cut item pasted is
  moved, and keeps its identity); drag to move.
- **Effect:** `new`, `delete`, `move`, `copy`, `undo` and `redo` need not
  be found even in the menu. The habits are held already.
- **Held:** no further change: move 1 already counts these verbs.

**3. Seven concepts in place of twelve.** (docs/concepts-seven.md
describes it in full: what each merge absorbs, how it maps onto the code
as built, what it costs, what could go wrong, and five decisions.)
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

The act key in the last two columns is no new thing to hold: the
right-click is the gesture, counted in its own row, and the Menu key is
the keyboard's right-click. Without it the totals are about 70 and
about 19.

From 71 to 20: move 1 removes 40, move 3 removes 5, and move 4 removes 2.
The last 4 come from counting Enter, Esc, Tab and the right-click as the
habits they are, as concepts.md's rule allows. With the grammar (§ The
grammar), the start is 48: the object words and address forms are gone
already, so move 1 removes the 19 verbs, and the sum is 48 − 19 − 5 − 2 −
4 = 18, about 20 again. As built, the start is about 53, since numbers
are not yet read by kind; § Built, and the count now, tables the rest of
the way from there, to about 20.

A first day: item, statement, offer and world; typing, Enter and Tab;
Leap; the right-click; and F3, which the screen shows. About 9.

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
