# Seven concepts: move 3 in full

A proposal for the author, 23 November 2026. It describes move 3 of
docs/concepts-next.md § How far, the last of the four moves; moves 1, 2
and 4 are built. The author took the five recommendations of § Decisions
for the author (design.md records them), and changes 1 to 3 of § What it
costs are being built; changes 4 and 5 wait for the measures.

**Built so far.** Change 1, the notebook's inspector, with two
departures from § What it needs built. It lists no revision's changes:
while a revision is open its changes fill the screen, and the notebook
is not drawn, so a row there could never be seen. It lists no fix for a
`[[link]]` to nothing, since decision 3 makes such a link a virtual
page, not an offer. A click on an offer does what a click on its thing
does: it goes to the item or the rule, and it adopts a suggestion.
Change 2, one row for an offer: the notebook's inspector, the
suggestions list, a claim's questions and a revision's changes draw an
offer in the same two rows, what it is and offers, then where it came
from. One decided stays where it was listed, dim, ending in how it was
decided: `(adopted)` for a claim, `(retracted)` for a change. A claim's
entry number, which its questions showed, is gone: nothing used it.

## The answer

Move 3 replaces the guide's twelve concepts with seven: **item, link,
statement, offer, world, history, view**. Seven leave recall (label,
claim, finding, document, entry, trail and find) and two come (offer and
history), so the learner holds five fewer: 12 − 7 + 2 = 7.

- **Label into link.** `#draft` becomes a link to a page named `draft`,
  as `[[draft]]` is. A page with no item of its own is virtual: it is
  made of its backlinks.
- **Claim, suggestion, finding, document and revision into offer.** An
  offer is anything put to you for a decision, with its source. You
  adopt it or dismiss it.
- **Entry and trail into history.** Every change and every jump is an
  entry already. Undo, redo and retract walk the changes; back and
  forward walk the jumps.
- **Find stops being a concept.** Each of its uses belongs to another
  concept already, or to one new list: the inspector of the notebook
  itself.

The count falls from about 29 to about 24 by concepts.md's rule (an
estimate), and a first day needs four concepts: item, statement, offer
and world. The journal's format need not change, if the author takes the
first decision below as recommended. The work is three new lists on the
screen, one change in how labels resolve, and a change of words through
the guide, the tutorial, the screen and the tests.

The main risk is in "offer". It joins concepts whose purposes differ: a
source's claim to test, a rule's finding, a co-author's version. If their
operational principles differ too, the merge is what Jackson calls
overloading, and it would cost the learner more than it saves (§ What
could go wrong). A middle path keeps claim and finding apart, with nine
concepts.

## The seven

Each has one purpose, its own state and actions, and an operational
principle, in Jackson's sense, as concepts.md's twelve do. The purposes
are concepts.md's ten.

| Concept | Purposes | Absorbs | Actions | Operational principle |
| --- | --- | --- | --- | --- |
| **item** | 1 write and organise | | typing, Enter, Tab, Shift+Tab; Backspace, Ctrl+X, Ctrl+C, Ctrl+V; `new`, `delete`, `move`, `copy` | write an item, and it keeps its place and its address until you move it |
| **link** | 3 connect; 1 group; 2 get to | label | `[[name]]` or `#name` in an item's text; `link` | link an item to a page, and each shows the other; a page is an item, or, with none of that name, the list of what links to it |
| **statement** | 4 say what to reason with | | write a sentence in an item, or Prolog in backticks; a question typed as it is | write `The rent is 1200.`, and it turns cyan: the notebook reads it, and its meaning shows beside it |
| **offer** | 5 keep a source apart; 6 notice conflicts; 10 exchange | claim, suggestion, finding, document, revision | `import`; `adopt`, `dismiss`; `check` | something is put to you, with its source, and stays apart from what you hold until you adopt it; dismiss it and it is set aside, its source kept |
| **world** | 7 what if | | a statement typed as it is, or `suppose`; `adopt world`, `dismiss world` | suppose something, and the two worlds stand side by side; adopt it and it is actual, dismiss it and the actual world is as it was |
| **history** | 8 lose nothing; 2 get back | entry, trail | Ctrl+Z, Ctrl+Y, `retract`; F7, F8 | every change and every jump is an entry, dated and signed; walk back along the changes to undo, along the jumps to go back |
| **view** | 9 see as a chart | | `show` | show an item's facts as a chart, a table or a graph, drawn from the world's facts |

## Label into link

**Now** (checked in the code):

- A label is a word after `#` in an item's text (outline.rs, `tags`). It
  is not stored: it is read from the text, and the facts
  `item_tag(Item, Tag)` are computed from it (world.rs), never written.
- `find #draft` lists the items that carry it (found.rs, `Found::Label`).
- A label that one item alone carries addresses that item, as `name`
  did (outline.rs, `by_label`; concepts.md, decision 5). So `go draft`
  and `[[draft]]` reach the one item whose text has `#draft`, and
  `#title` heads the outline.
- A `[[link]]` is read from the text too. Once the name resolves to an
  item, a `link` entry records it (`"inline": true`). Until then it is
  unresolved, and the item shows `not yet items: … adopt fix makes
  them`.

So a label and a link already work alike, from text, but one points
from the item and the other to it: `#draft` says that this item is a
draft, and `[[draft]]` says that this item points at the draft.

**After:** `#draft` and `[[draft]]` are two spellings of one link, to the
page named `draft`, as Roam and Logseq treat them (recalled, not
checked; Obsidian keeps tags apart from links).

- A page is the item of that name, if there is one: the item whose text
  is `draft`. With none, the page is virtual: it has no item and no
  entry, and it is made of its backlinks. `#title` still heads the
  outline with the one item that carries it.
- The inspector of a page, real or virtual, lists what links to it, as
  the inspector lists an item's backlinks now. So `find #draft` becomes
  `go draft`: going to the page shows the items that carry it.
- A virtual page with one backlink is reached as that item, so `go draft`
  still goes to the one item that carries `#draft`, as decision 5 has
  it. With two or more, it shows the page.
- `item_tag(Item, Tag)` stays, computed as now, so that the rules in old
  journals still hold. Beside it, a label is an `item_link(Item, Page,
  label)` fact, computed too, so a rule may use either.

**What the learner holds:** one concept, link, with two spellings. `#` is
for a word in running text and `[[ ]]` for a name of several words.

**What it needs built:**

- resolving `#name` and `[[name]]` to the same page;
- the virtual page: an address, a row in the inspector, a presentation a
  right-click acts on;
- `item_link(…, label)` facts beside `item_tag`;
- the guide's Link and Label ideas made one, and the tutorial's labels.

**The journal:** no change, if pages are virtual (decision 1). Making
each page an item would write a `create` entry for each label the first
time it is used, which is a revision of the format, with a version and a
reason (CLAUDE.md, Rigour).

## Claim, suggestion, finding, document and revision into offer

**Now** (checked in the code):

- **Claim.** A fact with a source: an item's text such as `Talk-notes
  says that the date of Buran-flight is 1989.`, or facts imported with a
  source, kept as `claims(Source, P)`. `adopt` (F3) writes an `adopt`
  entry, which `retract` takes back. The inspector shows `CLAIMS OF … ·
  WHO SAYS THEM`.
- **Suggestion.** A claim whose source is the memex (concepts.md,
  decision 2), computed at each evaluation, of six kinds: a link to
  Wikidata, a mapping, a constraint, a rule, a mention to link, a
  reference to cite (reason.rs, `Suggestion`). `find suggestions` lists
  them; `adopt suggestion N` and `dismiss suggestion N` decide them.
- **Finding.** What a constraint finds, computed at each evaluation and
  marked under the items involved: a violation (beliefs that break a
  rule) or a disagreement (a claim against a belief or another claim).
  A claim's disagreement offers `adopt instead`. Beside the findings, the
  memex's own reading offers fixes too: `adopt fix` makes what an item's
  `[[links]]` name, or, since move 4, mends a text that does not read;
  the right-click offers `split` for an item of several sentences. A
  `view` entry records the findings' counts when they change, for the
  measures.
- **Document.** A file imported as items, its losses counted, kept as a
  blob by its hash; `export` writes the outline back out.
- **Revision.** A document imported again: a world of changes, one entry
  each. `dismiss change N` rejects one; `adopt world` keeps the rest.

`adopt` and `dismiss` already mean one thing for all five (concepts.md,
decision 3): hold it, or set it aside. What the learner still holds is
five names, each with its own place on the screen.

**After:** one concept, offer. Its sources are a talk, a paper,
Wikidata, a co-author's version, a file, or the memex itself, from your
rules and from its own reading:

| An offer | Its source | Adopting it | Dismissing it |
| --- | --- | --- | --- |
| a claim | whoever says it | makes it a belief, its source kept | leaves it a claim, checked, not held |
| a suggestion | the memex | adds what it proposes | sets it aside until its evidence changes |
| a finding's fix, or a fix of the memex's reading | your rule, or the memex's reading | makes the fix (instead, make, mend, split) | leaves the finding or the text as it is |
| a document's items | the file | imports them as items, their claims as claims | imports nothing |
| a revision's change | the co-author's version | keeps it | rejects it |

The words claim, suggestion and finding may stay on the screen as kinds
of offer, named for their source (decision 2). The concept the learner
holds is one: something is put to you, with its source, until you
decide.

A document is a source, not a concept of its own: `import` reads a
source, and `export` stays a verb. A finding that offers no fix is still
an offer, of the choice between the beliefs it names: which to set
aside. `set`, an old command, was that choice made by hand, and `adopt
instead` makes it for a claim already.

**What it needs built:**

- **The notebook's inspector.** With the cursor above the first item,
  the inspector lists every open offer: claims not adopted, suggestions,
  findings and their fixes, the changes of an open revision, and the
  item just written, if its text has a repair (only that item keeps
  one). Each has its source, and adopt and
  dismiss on a click or a right-click. It answers "what is waiting for
  me?" in one place, as the suggestions list, `find violations` and
  `find disagreements` answer it now in three. The cursor needs a way
  there: `go parent` from an item at the top level (it was refused
  before change 1), and a click on the outline's title.
- **One row for an offer.** The suggestions list, the claim's questions
  and a revision's changes each draw their rows their own way now. One
  presentation of an offer, with its source, its adopt and its dismiss,
  serves all of them.
- **A document's first import as offers** (decision 4): either kept as
  now, imported at once, or put to you as an offer of its items, as a
  revision is.
- the guide's Claim, Finding and Document ideas made one, and the
  tutorial's part A and part B told as offers.

**The journal:** no change. An offer is a view over entries that exist:
claims, adoptions, suggestions dismissed, revision worlds and their
changes, and over findings, which are computed.

## Entry and trail into history

**Now** (checked in the code):

- Every change is an entry in the journal, numbered, dated and signed.
  `undo` and `redo` (Ctrl+Z, Ctrl+Y) walk the changes of this branch;
  `retract entry 12` takes back an old one and keeps what came after.
- Every jump is an entry too: a `view` entry with where it came from and
  where it went. The trail is replayed from those entries
  (session.rs, `Trail::apply`), so a reopened notebook keeps it. `go
  back` and `go forward` (F7, F8) walk it.
- Entry numbers appear where something presents an entry: a layer, a
  change, a constraint, a fact, a retraction's question. No list shows
  the entries themselves.

**After:** one concept, history, with one operational principle: what you
did is a sequence, and you can walk back along it. It has two strands,
and the keys stay apart: Ctrl+Z and Ctrl+Y for the changes, F7 and F8
for the jumps. `retract` takes back one change from the past.

**What it needs built:** a list of the history, so that an entry's number
is something seen, not recalled: its latest changes and jumps, newest
first, each with its number, what it did, and, on a right-click, retract
or go back to it. The notebook's inspector can end with it.

**The journal:** no change. Jumps are entries already.

## Find stops being a concept

**Now**, `find` lists in the inspector's place what matches a text, a
label, a kind of finding, the suggestions, or a question, and alone it
brings back the item's links.

**After**, each use belongs to another concept:

| `find` now | After move 3 | Built? |
| --- | --- | --- |
| a text | Leap, and `?` while holding lists every match | yes |
| a label (`find #draft`) | the page: `go draft` | no: the page's inspector |
| violations, disagreements, suggestions | the notebook's inspector | no |
| a question | typed as it is (move 4) | yes |
| alone: the item's links | the inspector, which shows them by default | yes |

`find` stays as a verb, for those who know it, and the right-click and
plumbing still offer it. It is no longer an idea the learner must hold
to get to things.

## What the learner holds after

| Held in recall | Now (concepts-next.md) | After move 3 |
| --- | --- | --- |
| concepts | 12 | 7 |
| everything else | about 17 | about 17 |
| **total** | **about 29** | **about 24** |

All estimates, under concepts.md's rule. The fall is the five concepts.
Nothing else in the count moves, since the verbs, the object words and
the address forms were made recognised by moves 1, 2 and 4.

A first day: item, statement, offer and world; typing, Enter and Tab;
Leap; the right-click; F3, which the screen shows. About 9, as
concepts-next.md had it, with one concept fewer than the five the
tutorial's part A teaches now (claim and finding become offer).

## What it costs

In a recommended order, one change a commit. The sizes are estimates.

| Change | Size | Depends on |
| --- | --- | --- |
| 1. The notebook's inspector: every open offer, each with its source, adopt and dismiss; `go parent` from the top level, and a click on the title, go there | medium | |
| 2. One presentation of an offer, used by the suggestions, a claim's questions and a revision's changes | medium | 1 |
| 3. History: a list of the latest changes and jumps, at the foot of the notebook's inspector | small to medium | 1 |
| 4. Labels as links: `#name` and `[[name]]` to one page; virtual pages, their inspector and presentation; `item_link(…, label)` beside `item_tag` | medium to large | |
| 5. The guide's seven ideas; the tutorial's parts A and B told with offers; the screen's words, as decision 2 settles | large | 1 to 4 |

**Words to change** (measured, by counting each word and its plural in
each file; many uses in the code are names that would stay):

| Word | The guide | The tutorial | The memex's code | Its tests |
| --- | --- | --- | --- | --- |
| claim | 48 | 24 | 208 | 156 |
| suggestion | 32 | 0 | 158 | 68 |
| finding | 20 | 8 | 64 | 19 |
| label | 27 | 9 | 181 | 64 |
| entry | 35 | 6 | 452 | 90 |
| trail | 5 | 1 | 28 | 23 |
| document | 22 | 7 | 93 | 44 |
| revision | 9 | 3 | 34 | 23 |

Under decision 2's recommendation, few of the screen's words change:
the concept is new, and the kinds keep their names. Tests that pin a
heading, such as `CLAIMS OF … · WHO SAYS THEM`, change only where a
heading does.

**The journal:** unchanged under decision 1's recommendation. Old
journals replay as they are: the labels in their text become links when
read, and `item_tag` still holds for their rules.

## What could go wrong

- **Offer may be overloaded.** Jackson's concept design asks that each
  concept serve one purpose, and calls a concept that serves two
  overloaded; one of his kinds is false convergence, two concepts taken
  for one because they look alike (The Essence of Software, 2021;
  recalled, not checked). Offer serves three of the ten purposes. The
  case for the merge is that its operational principle is one: put to
  you, with a source, until you decide. The case against is that a claim
  is tested against the world, a finding is computed from your own rule,
  and a revision is a world of its own, and that the learner must still
  learn each one's source to decide well. **Middle path:** offer absorbs
  suggestion, document and revision, which already decide alike; claim
  and finding stay. Nine concepts, about 26 (an estimate).
- **A label no longer names the item that carries it.** Under the link
  model, `#draft` on an item points away from it, to the page. The rule
  that a page with one backlink is reached as that item keeps `go draft`
  working; a second item labelled `#draft` turns the address into a
  page, as it turns it into nothing now.
- **A typo makes a page.** If `[[name]]` to nothing becomes a virtual page,
  as `#name` is, the hint `not yet items: … adopt fix makes them` has no
  reason to stay, and a misspelt link makes a page of its own, quietly
  (decision 3).
- **Undo and back are one concept and two keys.** A learner told that
  history is one sequence may expect Ctrl+Z to undo a jump. Browsers keep
  the two apart, and so do the keys here; the concept joins only what
  they have in common.
- **The notebook's inspector may be long.** In a notebook with many
  claims it lists many offers. It needs the ranking and the limit the
  right-click's menu lacks: the most specific first, and the rest
  counted, not listed.
- **The measures.** The guide's sections that merge (Link and Label;
  Claim, Finding and Document) cite more of each other's terms, and the
  referent gate may put them over 6. Two of the guide's sections are
  over it now, accepted: Pointing at an argument (8) and Plumbing (12).

## Decisions for the author

1. **Virtual pages or real ones?** A label's page with no item of its
   own is virtual (no entry, no format change), or each label makes an
   item the first time it is used (a revision of the format).
   Recommended: virtual. Making one an item stays possible by hand.
2. **The words on the screen.** The screen says "offer" everywhere, or
   keeps claim, suggestion and finding as the kinds of offer, named for
   their source. Recommended: keep the kinds. The concept is what the
   learner holds; the kinds tell the learner where each came from, which
   deciding needs.
3. **`[[name]]` to nothing.** It becomes a virtual page, as `#name` does,
   and the hint goes; or it stays unresolved, with its hint and `adopt
   fix`. Recommended: a virtual page, with the hint kept only while the
   text cursor is in the link, so a typo is seen as it is typed.
4. **A document's first import.** Imported at once, as now, or put to
   you as an offer of its items, as a revision's changes are.
   Recommended: at once. An import is asked for, so a second question
   would only slow it; its claims are offers already.
5. **Seven, or the middle path of nine?** Recommended: build changes 1
   to 3 first. They serve either choice, and the journal can measure
   whether learners treat a claim and a finding as one kind of thing
   before the words are changed.

## Measuring

The journal records each adoption and dismissal with what it decided. If
offer is one concept to the learner:

- the time from a learner's first adopt of one kind of offer to the
  first adopt of another kind falls, compared with today's journals;
- dismissals are spread across the kinds, not kept to suggestions;
- refused commands about the old words (`find suggestions`, `find
  violations`) fall once the notebook's inspector lists them.

If the middle path is right, the first measure stays flat between claims
and findings while it falls between suggestions, documents and
revisions.
