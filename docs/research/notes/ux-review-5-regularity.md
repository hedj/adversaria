## Verdict

The seven decisions moved most of the memex from recall to recognition, but the learner still holds about 76 items by docs/concepts.md's own counting rule, against a plan of 49 and a budget of about 50 (an estimate; the rule's boundaries are mine). Almost all of the excess is in one place: the taught forms use 27 object words, and 23 of them are not concept names (measured from the guide's § 2.4). Three more listed commands sit beyond the 17 verbs. The irregularities that remain are of five kinds:

- `adopt` and `dismiss` are not defined over the same objects;
- one act asks before it retracts, and another does not;
- `find` and `go` reserve words with no way to escape them;
- the inspector composes its sections three different ways;
- several commands ask for a word the memex could read from what they act on.

Nine suggestions below make the forms regular and the mechanisms general. None drops a capability, and three add one. Together they would bring the held count to about 65, or about 57 if the eight ways of `go` are counted with the keys that run them (estimates). No file but this one was changed.

## The count

**Method.** As in the fourth review (ux-review-4-mechanisms.md): one mechanism is one thing with its own name or behaviour that a learner must recognise or recall. I counted from `crates/memex/src/actions/dispatch.rs` (`COMMANDS`, `GROUPS`, `ALIASES`), `src/config.rs`, `src/plumb.rs`, `src/present.rs`, `src/view.rs` and the guide. Old names are not counted: they run only when typed whole, and are not listed.

| Category | Count | What it covers (how known) |
| --- | --- | --- |
| Quasimodes and ways to use them | 6 | Leap down, Leap up, Command, the Suppose key; hold, tap (checked) |
| Gestures in a quasimode, and confirmations | 12 | Enter, Esc, Tab, `?`, Backspace on empty, the "release now" window, Leap again, Leap-or-make, the near-match warning, Suppose's readings, the retract confirmation, `?` typed after `find` and a word (checked) |
| Editing keys | 7 | guide § 2.3 (checked) |
| Function keys, Shift bindings, chords | 16 | 7 F-keys and 6 Shift bindings (measured, config.rs: 13 defaults); Ctrl+Z, Ctrl+Y, Ctrl+F (checked) |
| Commands and their object words | 47 | 20 listed (measured), 27 object words in their taught forms (measured) |
| Syntax conventions | 10 | unique prefix; `to`/`from`/`after` optional; `as` required; other guide words left out; `where`; backticks; `{N}` blanks; a final `?`; `#` labels; quotes for strings (checked) |
| Address forms | 7 | number, text's start, label, `entry N`; and after an object word, `suggestion N`, `link N`, `change N` (checked) |
| Screen regions | 6 | top bar, outline, inspector or list, foot, menu, dim hint line |
| Inspector contents | 9 | links, findings, READ AS, a claim's questions, a view's layers, four kinds of list (checked) |
| Indicators on items | 17 | as in the fourth review, with "adopt fix makes them" for links to nothing |
| Kinds of item text, and statement forms | 18 | 12 kinds; 6 forms in backticks (checked) |
| Links | 10 | as before, a label in place of a name (checked) |
| Worlds | 6 | as before |
| Views | 30 | 3 kinds of `show`, 10 marks, 10 channels, 3 frames, 4 outline predicates (measured, view.rs) |
| Kinds of suggestion | 6 | § 3.4 |
| Mouse | 14 | 6 gestures, 8 kinds of presentation (measured, present.rs) |
| Plumbing | 14 | the idea and 13 built-in patterns (measured, plumb.rs) |
| Configuration sections | 4 | checked |
| **Total** | **about 239** | an estimate: the fourth review's 231, with nothing removed, and `find`, `show`, labels and Prolog in items added |

The total was never the measure; what a learner must hold is. By docs/concepts.md's counting rule:

| Held in recall | Planned | As built |
| --- | --- | --- |
| concepts | 12 | 12 |
| verbs | 17 | 20 (`transclude`, `fold`, `switch` still listed) |
| command shape, guide-word rule | 2 | 2 |
| statement forms | 6 | 6 |
| address forms | 3 | 4 (a number after an object word is a fourth) |
| quasimodes, tap | 3 | 3 |
| Enter, Esc, Tab, `?` | 4 | 4 |
| Shift with an arrow; right-click | 2 | 2 |
| object words that are not concept names | 0 (assumed) | 23 |
| **total** | **49** | **about 76** (estimate) |

The 23 object words (measured): `back`, `forward`, `next`, `previous`, `first`, `last`, `parent`, `child` (go); `violations`, `disagreements`, `suggestions` (find); `chart`, `table`, `graph` (show); `quote`, `source` (check); `results`, `wikidata` (import); `instead`, `fix`, `suggestion` (adopt); `change` (dismiss); `word` (delete). The proposal assumed the object words would be the concept names; they are not.

## The irregular spots

1. **`adopt` and `dismiss` take different objects** (checked, claims.rs). `adopt` takes claims, `instead`, `suggestion N`, `fix` and `world`. `dismiss` takes `suggestion N`, `world` and `change N`. There is no `adopt change N`, no `dismiss fix`, and no dismissing a claim in an item. `instead` is not an object but a manner. `dismiss change N` runs the same code as `retract entry N` (`cmd_retract`), so the word `change` is a second name for one act.
2. **One act retracts without asking** (checked). `retract`, F4 and `dismiss change N` list what depends on the entry and wait for Enter. `adopt instead` retracts the beliefs a claim contradicts at once (`adopt_claims`: `retract_all`, with no confirmation).
3. **Reserved words cannot be escaped** (checked, found.rs and editing.rs). `find violations`, `find disagreements` and `find suggestions` are lists, so the text "violations" cannot be found. `go back`, `go next` and `go link 2` are moves, so an item whose text starts "next" or "link" is reached only by its number.
4. **Numbers mean three things after an object word, and two when bare** (checked).
   - The suggestions list's own hint teaches `adopt N adopts · dismiss N dismisses` (ui.rs). That is a form the guide does not teach: it works only because a bare number falls through to the old `accept`.
   - `suggestion N` and `link N` are positions in a list, while `change N` is an entry's number.
   - The right-click on `#12` offers both `retract entry 12` and `go #12`: `#` is an entry in one and an item in the other (plumbing tests).
5. **The inspector composes its sections three ways** (checked, ui.rs `right_panel`, `draw_claimed`).
   - An item with findings shows its constraint and READ AS, but not its links.
   - A claim item shows its findings, READ AS and its questions, but not its links.
   - A view shows its layers only: no READ AS, no links.
   So the links of a disputed claim, the item most worth tracing, are hidden.
6. **The menu makes statements the text does not show** (checked, plumb.rs). Three of the 13 built-in patterns run old names: `fact $0`, `set $0` and `ace $0`. The first two state a fact on the item without writing it, which decision 1 ("statements are written, not commanded") set out to end.
7. **`show ADDRESS as chart` does not chart the item's facts** (checked, layers.rs). docs/concepts.md says `show` shows "an item's facts as a chart". As built, the address only chooses which item draws the view, and the chart is of every box in the world.
8. **`transclude` is `copy` with its arguments reversed, and it is not live** (measured: a transcluded item kept "Energia rocket" after its original became "Energia rocket launched 1987").
   - `copy after ADDRESS` copies the target's text, with new characters.
   - `transclude from ADDRESS` copies the address's text, sharing its characters, so it keeps their origin.
   The guide says it "shows ADDRESS's text in place", which suggests it follows the original. The proposal's `link … (as) shows` would be live, which is a different capability.
9. **Commands ask for what they could read** (checked, sources.rs).
   - `import` needs `document`, `results` or `wikidata`, though the file's extension says which; plumbing already chooses by extension. On any failure its usage message names only the Wikidata form.
   - `check` needs `quote` or `source`, though the item says which it has: a document it came from, or a `doi` fact.
   - `export FILE NUMBER` takes an item's number, where every other command takes any address.
10. **Headings follow no pattern** (checked): `LINKS OF 2.1`, `CLAIMS OF 2 · WHO SAYS THEM`, `CONSTRAINT · NOTHING HAS …`, `VIEW · 2 LAYERS`, `MATCHES OF “x” · 3`, `LABELLED #draft · 2`, `MARKED BY A VIOLATION · 2`, `SUGGESTIONS · 3 OF 5`.
11. **Only Suppose mends free English** (checked). Suppose reads "the flight was in 1989" against the facts and asks "Do you mean …?". An item, or a question to `find`, must be exact ACE, or it is prose.

These hold up and should stay: the one command shape; `go` (to an address) beside `find` (by content); old names that run but are not taught; statements written in items; the inspector that takes no keys.

## Suggestions

**1. Make adopt and dismiss total over offers; retract takes back any adoption.**
- **Rule:** an offer is a claim (in an item, or a suggestion, which is a claim of the memex), a fix of a finding, a change of a revision, or a supposed world. `adopt OFFER` holds it, `dismiss OFFER` sets it aside, and `retract` takes back any adoption.
- **How:** add `adopt change N` (keeps it, and marks it reviewed) and `dismiss fix` (hides the fix until the finding changes). A dismissed claim in an item shows dim and asks no questions. Replace `instead` with the confirmation of suggestion 2.
- **Capabilities:** all kept; `dismiss` gains claims and fixes. `dismiss change N` and `retract entry N` stay one act, under one taught name: `dismiss change N` for an offer, `retract entry N` for anything.
- **Net:** no change in held items (suggestion 2 removes `instead`); one table of which objects each verb takes, with no gaps.
- **Cost:** medium. **Risk:** low.

**2. One confirmation rule: whatever retracts asks first.**
- **How:** `adopt` on a claim that contradicts beliefs asks: `Enter adopts it beside 1.1 (a violation) · Tab adopts it in place of 1.1 · Esc keeps it a claim`. It uses the same three keys as Suppose's readings.
- **Replaces:** `adopt instead`, which becomes an old name.
- **Capabilities:** kept. A claim that contradicts nothing is adopted at once by F3, as now. The tutorial's step 4 needs one Enter more.
- **Net:** −1 held, and one exception gone.
- **Cost:** low to medium. **Risk:** an extra keystroke on disputed claims, which is where a pause is wanted.

**3. `find` lists by concept: findings and claims.**
- **How:**
  - `find findings` lists every item a finding marks, grouped by kind: violations, disagreements, links to nothing, results whose input changed, text not read.
  - `find claims` lists the claims awaiting a decision: those in items not yet adopted, and the suggestions.
  - Words in quotes are always text, for `find` and `go`: `find "violations"`, `go "next steps"`.
- **Replaces:** `violations`, `disagreements`, `suggestions` as taught words. They stay as narrowing forms, listed by `?`.
- **Capabilities:** kept, and three gained: the other kinds of finding become findable, open claims form one list, and reserved words can be escaped.
- **Net:** −3 held (the replacements are concept names).
- **Cost:** low to medium. **Risk:** low.

**4. Read the kind from what is acted on.**
- **How:** `import FILE [as ADDRESS]` chooses document, results or Wikidata by the file, as plumbing does. `check` checks all the target can be checked against: its quotation against the document it came from, its `doi` against Crossref. The kind words stay, as narrowing forms. Fix `import`'s usage message to name every form.
- **Capabilities:** kept.
- **Net:** −4 held (`results`, `wikidata`, `quote`, `source`; `document` is a concept name).
- **Cost:** low. **Risk:** a file whose extension misleads; the kind word still settles it.

**5. One rule for numbers.**
- **Rule:** a bare number is an item; a number after an object word is the number shown beside that object.
- **How:**
  - The suggestions hint reads `adopt suggestion N · dismiss suggestion N`, as the guide teaches.
  - The built-in "go to" pattern stops offering `go #12`.
  - `export FILE ADDRESS` takes any address: `export essay.md draft`.
  - The revision's list shows each change's number as `change 52`, the number `dismiss change 52` takes.
- **Capabilities:** kept. `export` gains labels and text starts.
- **Net:** one rule in place of three exceptions.
- **Cost:** low. **Risk:** low.

**6. The inspector has one order of sections.**
- **How:** each section is shown when it has something, always in this order:
  1. findings;
  2. the claims' questions;
  3. the view's layers;
  4. links and mentions;
  5. READ AS.
  One heading pattern, `WHAT OF ADDRESS · COUNT`: `FINDINGS OF 1.1 · 1`, `CLAIMS OF 1.3 · 1`, `LINKS OF 1.3 · 2`. A list's heading says how to leave it (`find alone returns`).
- **Capabilities:** kept; gained are the links of items with findings or claims, and READ AS for views.
- **Net:** one rule in place of three; headings recognised, not learnt.
- **Cost:** low to medium. **Risk:** long panes; the order puts what needs a decision first.

**7. Every statement goes through the text; fixes do what `set` did.**
- **How:**
  - The right-click's "state it here" appends the fact to the item's text in backticks, where it is read as any statement. It no longer runs `fact`.
  - A violation between a belief just read and an older one offers a fix: `adopt fix` retracts the older one (decision 3's "set as a fix", which was not built).
  - "read it as ACE" shows the reading at the foot, as typing in an item does.
- **Replaces:** the three menu patterns that run old names.
- **Capabilities:** kept; the statement becomes visible and editable, and undo takes it back with the text.
- **Net:** no held change; one path for statements.
- **Cost:** medium. **Risk:** an item's text grows by what the menu adds.

**8. `show` charts an item's facts.**
- **How:** the outline offers a fact linking each statement to its item (`item_states(Item, Fact)`, beside `item_text/2` and `item_tag/2`). Then:
  - `show 2 as chart` charts the boxes stated in item 2's branch;
  - `show #budget as chart`, those of the items labelled `#budget`;
  - `show as chart` alone, every box, as now.
- **Capabilities:** kept, and gained: views of part of the notebook, without writing the layer by hand.
- **Net:** no held change; it does what decision 6 said.
- **Cost:** medium. **Risk:** low.

**9. Bring the listed commands to the 17 verbs.**
- **How:**
  - **`transclude`:** becomes `copy [FROM] after TO`, which always keeps where its characters came from, as transclusion does. `transclude from 3` is then `copy 3 after here`, with the same optional FROM that `link` has. The guide should say it is a copy, not a live view; a live view would be a new capability (`link … as shows`).
  - **`fold`:** a click on an item's bullet, a presentation the outline already draws.
  - **`switch`:** `show ADDRESS as outline` opens a second outline at an item, and a click gives it the keys, as a click already does. `pane`, `switch` and F6 become old names.
- **Capabilities:** kept. Two outlines stay; a copy gains provenance.
- **Net:** −3 held; the list is the 17 verbs.
- **Cost:** medium. **Risk:** a copy that keeps provenance where a user wanted fresh text. Nothing found relies on fresh characters; a `check quote` on a copy would then pass, which is right.

**Smaller, by renaming or wording alone:**
- **Old names teach:** when one runs, the foot says so, as in `commit ran as adopt world`.
- **Mending everywhere:** Suppose's "Do you mean …?" could serve an item's text and a question to `find`, as a fix on a "not read" finding that rewrites the text in ACE. This is medium cost, and it gives one mending mechanism for all input.
- **The eight ways of `go`:** these are the largest block of object words, and each has a key. List them under `go` in `?` with their keys, so they are read, not recalled.

## Ranked summary

| # | Suggestion | Held items removed (estimate) | Capabilities | Effect on learnability | Cost |
| --- | --- | --- | --- | --- | --- |
| 4 | Read the kind from what is acted on | 4 | kept | high: no word for what the memex can see | low |
| 3 | `find findings`, `find claims`, quotes for text | 3 | kept, three gained | high: lists named by concepts | low–med |
| 6 | One order of sections in the inspector | 0 (3 rules become 1) | kept, links and READ AS gained | high: nothing hidden by context | low–med |
| 5 | One rule for numbers | 0 (3 exceptions become 1 rule) | kept, `export` gains addresses | medium | low |
| 2 | Whatever retracts asks first | 1 | kept | medium: one exception gone | low–med |
| 1 | adopt and dismiss total over offers | 0 | kept, dismiss gains claims and fixes | medium: one table, no gaps | med |
| 9 | The listed commands become the 17 verbs | 3 | kept, copy gains provenance | medium | med |
| 7 | Every statement through the text; `set` as a fix | 0 | kept, statements visible | medium: one path | med |
| 8 | `show` charts an item's facts | 0 | kept, gained | low for learning, high for use | med |

With all nine, the held count falls from about 76 to about 65 (4 + 3 + 1 + 3 = 11 items). If the eight ways of `go` are counted with their keys, as concepts.md counts shortcuts, it is about 57. Both are estimates under concepts.md's rule.

How each claim was checked:
- **Counts:** commands, groups and old names by a script over dispatch.rs; object words by a script over the guide's § 2.4 forms; bindings in config.rs; patterns in plumb.rs; presentations in present.rs; marks and channels in view.rs.
- **Behaviours:** read in the code at the places named above. The transclusion was run: a probe test, deleted after.
- **Not run:** the suggestions themselves, and any timing. The held counts are estimates under concepts.md's rule, whose boundaries are a judgement.
