# Tutorial: a first session

This tutorial takes about forty minutes (an estimate), in four parts of
four steps each, in one session:

- A. First notes: an outline, facts written in controlled English, a
  sentence computed from them and a query, after the design's user story (docs/design.md, § A user story): notes
  on the Soviet Buran space shuttle.
- B. Checking a claim: a claim from a source, a constraint that catches
  the conflict, and the claim questioned, adopted, taken back and tried
  in a supposed world.
- C. Boxes that compute: a small budget, from boxes and rules, and a
  question about it, all in sentences.
- D. Views, and leaving: the budget as a chart and a table, part A's
  dates as a timeline, and how to leave.

A test performs every step below and checks every screen text quoted here
(`crates/memex/tests/tutorial.rs`). If the memex changes and this tutorial
does not, that test fails. The test drives the memex directly, not through
its window; see the limits in the last step.

The key names below are the defaults. If you have changed them (docs/guide.md,
§ Configuration), use yours.

## A. First notes

### 1. Start

You need a desktop under X11 or Wayland; the memex opens a window of its
own. Then, from the repository:

```bash
./dev-shell
```

```bash
make run JOURNAL=$HOME/adversaria-tutorial KEY=$HOME/.adversaria-tutorial.key
```

The first run makes a signing key and a new journal. The journal is
permanent: nothing you do is ever erased from it. That is why this tutorial
uses its own journal, which you can delete when you are done.

The screen has a top bar, the outline on the left, and a second pane on the
right. The outline says:

    (empty: type to write the first item)

### 2. Write an outline

Type `The programme`, then press Enter for a new item, and type
`Buran flew once, unmanned, on 15 November 1988.`

Press Tab. The Buran item moves under the first. Stop typing for a
second, and the outline shows:

    ▼ 1. The programme
      • Buran flew once, unmanned, on 15 November 1988.
          prose: not ACE, it stops at Buran <> flew once, unmanned, on 15 November 1988.

Only top-level items show their number. Items under them are addressed by
path: the first item under 1 is 1.1, the second 1.2, and so on.

Everything you type is written to the journal, a second after you stop or
when you do something else. There is no Save.

The dim line is the memex reading what you wrote. It reads Attempto
Controlled English (ACE): English with one meaning per sentence, which its
parser, APE, turns into logic. This sentence is not ACE (ACE has only the
present tense), so it stays prose, a note for you; the line shows where
APE stopped. It is shown only at the cursor, and nothing is wrong: most of
an outline is prose.

### 3. Write facts

A fact is a statement the memex can reason with. Facts are what
constraints and derived values read. Write them in ACE, and the memex
reads them; you never write them a second time in another language.

Press Enter and type

`The date of Buran-flight is 1988. The label of Buran-flight is "Buran's flight".`

Press Enter, and type

`The date of Energia-launch is 1987. The label of Energia-launch is "Energia's first launch".`

Stop for a second. Both items turn cyan: the memex has read each as two
facts. Cyan text has a formal meaning to the memex; text in the usual
colour is prose. A name written with a capital and hyphens, such as
`Buran-flight`, is one thing; "the date of Buran-flight is 1988" says
that its date is 1988. A label lets the memex write facts about the thing
in words.

The right pane shows, under `READ AS`, what the item at the cursor was
read as, in Prolog, the language the memex reasons in:

    fact  date(energia_launch, 1987)
    fact  label(energia_launch, "Energia's first launch")

If you change a cyan sentence, what it said is taken back and the new
text read. A text is read whole or not at all, so it never says less to
the memex than to you.

### 4. Derive a sentence

Press Enter and type a placeholder, such as `gap`. Then give the memex a
command. Hold **Left Ctrl**. This is the Command quasimode: while you hold
it, what you type is a command, shown at the foot of the screen. Type this
(all one line) and release Left Ctrl; releasing runs the command on the
item under the cursor:

`derive The flight came {N:years} after Energia's first launch. where date(buran_flight, B), date(energia_launch, E), N is B - E`

The `where` part is a Prolog query, the memex's own language: it finds the
two dates and their difference. The item now shows text computed from the
facts:

    The flight came a year after Energia's first launch. [derived]

If a date changes, the sentence changes with it.

To run a query by itself, run the command `query date(E, Y)`. It finds
every date the memex believes, and says:

    2 answers: E = buran_flight, Y = 1988 · E = energia_launch, Y = 1987

A query only reads; it writes nothing to the journal.

While Left Ctrl is held: `?` lists the commands that match what you have
typed; Esc completes the command name; Backspace erases. A tap of Left Ctrl
(press and release with nothing typed) latches the quasimode, so you can
type without holding it; Enter or a second tap runs the command.

## B. Checking a claim

### 5. A claim, and a constraint

Press Enter and type what a speaker said, which you have not checked:

`Talk-notes says that the date of Buran-flight is 1989.`

The memex reads it as a claim, with its source, Talk-notes. A claim is not
a belief. It is something a source says, kept apart from what you believe
until you adopt it.

Now say what must never happen: one thing with two dates. Press Enter and
type

`Nothing has more than 1 date.`

and stop for a second. The memex reads it as a constraint. Before keeping a
constraint, the memex plants a violation and checks that the constraint
fires on it. It says:

    constraint Nothing has more than 1 date kept: it fires on a planted violation

The top bar now says `1 disagreement between sources`, and under the Buran
item is a mark:

    ⚑ Nothing has more than 1 date, sources disagree: 1988 here, 1989 in 1.5

It is a disagreement, not a violation, because the 1989 date is only a claim.

### 6. Leap

Leap moves the cursor by searching, not by arrowing. Hold **Right Alt**
(Leap forward), type `date of buran`, and release. The cursor goes to the
next item holding that text, past the end and round to the top. The right
pane shows the constraint behind the item's mark:

    CONSTRAINT · NOTHING HAS MORE THAN 1 DATE

Left Alt leaps backward. Esc completes the pattern; `?` lists every match.
Tapping the Leap key twice repeats the last Leap.

### 7. Question the claim, adopt it, take it back

Leap to `talk`. Press **F2** to interrogate the claim. The right pane
answers five questions:

    INTERROGATE · WHAT IS CLAIMED HERE

It shows who else claims it, `who claims the contrary`, its source, the
source's record, and what adopting it would do:

    adopting it would raise 1 violation of Nothing has more than 1 date

Press **F3** to adopt the claim. It is now a belief, with its source kept.
The top bar says `1 constraint violation`, and the mark is loud:

    ⚑ Nothing has more than 1 date, violation: 1988 here, 1989 in 1.5

Press **F4** to take the adoption back. The memex says the adoption is
retracted, and the violation is gone. The adoption and its retraction both
stay in the journal.

### 8. Suppose

To see what follows if the speaker were right, without changing what you
believe, suppose it. Hold **Super** (the Windows or Command key), type
`the flight was in 1989`, and release. The memex reads your sentence against
the facts and asks:

    Do you mean: Suppose that the date of Buran's flight is 1989.  ⏎ yes · Esc no

Press Enter. A second world opens beside the actual one, row for row:

    WORLD · SUPPOSE THE FLIGHT WAS IN 1989

In the supposed world the derived sentence says
`The flight came two years after Energia's first launch.` The 1988 fact is
set aside there, so the constraint holds; the memex says
`set aside in this world: 1.2`. The actual world is untouched.

Run the command `close` to return to the actual world. The supposition
stays in the journal; the memex says so: `it stays in the journal`.
(`commit` would instead make the supposed world the actual one.)

## C. Boxes that compute

The memex has no spreadsheet. But items can state values, rules can
compute from them, and constraints can relate them, and that is most of
what a spreadsheet does. This part builds a small household budget that
way, in sentences.

### 9. Boxes that hold values

Leap to `nothing has` (the last item of part B), press Enter for a new
item and Shift-Tab to bring it to the top level, and type `Budget`. Press
Enter, then Tab to put the new item under Budget, and write one box per
item, pressing Enter between them:

- `The income is 2000.`
- `The rent is 1200.`
- `The food is 400.`

"The income", with no owner, is a box: a value with a name. Now two rules,
one per item:

- `If the rent is X and the food is Y then the cost is X + Y.`
- `If the income is X and the cost is Y then the rest is X - Y.`

Stop for a second. Every one of these items turns cyan, and the memex says
`rule rest/1 kept`. A rule's value is not a box you set: the cost is
computed from the rent and the food, and the rest from the income and the
cost.

To show the rest, press Enter, type `left`, and run the command:

`derive Left over: {L} where rest(L)`

It reads:

    Left over: 400 [derived]

You can also ask in English. Press Enter and type `What is the rest?`.
After the pause, the question turns cyan, and the line under it gives
the answer:

    → 400

The answer changes as the boxes do.

### 10. Rules between the boxes

Spending must not exceed income. Press Enter and type

`It is false that the cost is greater than the income.`

Stop for a second. As in step 5, it is tried against a planted violation
before it is kept:

    constraint It is false that the cost is greater than the income kept: it fires on a planted violation

One more constraint, which says that a box holds one value. It is about
every box at once, and ACE has no sentence for that, so give it as a
command, in Prolog:

`constraint one value per box: false :- cell(B, X), cell(B, Y), X \= Y.`

`cell(B, X)` is how the memex keeps "the B is X". It says:

    constraint one value per box kept: it fires on a planted violation

### 11. Change a value

To change a value, change its sentence. Leap to `food is 4`, press End,
erase `400.` with Backspace, type `900.`, and stop for a second. The old
food value is taken back and the new one read. Every box that depends on
food is recomputed: the screen shows `Left over: -100 [derived]`, and the
top bar says `1 constraint violation`, marked under the boxes involved.
The memex does not choose how to repair it (lower the rent? the food?);
that is left to you. Put food back within income: leap to `food is 9`,
and in the same way make it `500.`

Now `Left over: 300 [derived]`, and the violation is gone.

### 12. What if

To see what a change would do without making it, suppose it, as in step
8. Hold Super, type `rent is 1600`, and release. The memex asks:

    Do you mean: Suppose that the cell of rent is 1600.  ⏎ yes · Esc no

Press Enter. The two worlds are shown side by side, box for box: in the
supposed one, the old rent is set aside and `Left over: -100`. The memex
says:

    supposed; set aside in this world: 2.2; left for you: 1 violation of It is false that the cost is greater than the income

Only the old rent is set aside, by "one value per box". Income and food
stay: with them, the new rent breaks the cost constraint, but any of the
three could be the one to change, so the memex leaves that violation for
you rather than choosing.

Run `close` to go back; the actual budget still says
`Left over: 300 [derived]`. (`commit` would have made the new rent real.)

## D. Views, and leaving

A view is an item that draws something, made of layers. A layer is a
mark, such as a bar or a box, drawn once for each solution of a goal. As
before, nothing here is a chart type the memex knows: a chart, a table
and a timeline are marks over your own facts.

### 13. A chart of the boxes

Leap to `left over`, press Enter, and Shift-Tab to the top level. Type
`Spending chart`. Then add two layers to it:

- `layer bar(x(C), y(V)) where cell(C, V)`
- `layer rule(y(I)) where cell(income, I)`

The first draws a bar for each box, at the box's name and as high as its
value; the second, a line across the chart at the income. The chart is
drawn under the item, and the right pane says `VIEW · 2 LAYERS`, with how
many marks each layer draws.

Leap to `food is 5` and make it `900.` again. The food bar rises, and
all three bars are drawn bold: the violation of the cost constraint rests
on the income, rent and food facts, and so does every bar. Leap to
`food is 9` and make it `500.` to put it back. With the mouse on (docs/guide.md, § The mouse), a click on
a bar goes to the box that holds its value.

### 14. The same boxes as a grid

Press Enter and type `Budget table`. Then:

`layer cell(x(value), y(C), text(V)) where cell(C, V)`

A `cell` mark is a box in a grid, at column x and row y. The view is a
table, rows in order:

    food   │   500
    income │  2000

A spreadsheet is the same idea with more columns, and with boxes whose
values come from rules (docs/guide.md, § 3.3).

### 15. A timeline from part A

Part A's facts are still here. Press Enter, type
`Timeline of the programme`, then:

`layer tick(x(Y), text(L)) where date(E, Y), label(E, L)`

Each event is a tick on a line of years, labelled from its `label` fact:
`Energia's first launch` in 1987, then `Buran's flight` in 1988. The talk
notes' 1989 is not drawn: it is a claim, not a belief, and a view draws
beliefs.

docs/guide.md, § 3.3 has more: line charts, Gantt charts, flowcharts, and
your own outline drawn as a tree.

### 16. Leave

Press **F10** to quit. What was pending is written first.

The sealer, started by `make run`, signs new entries within a fraction of a
second (it looks every 200 ms). To check the whole journal, its hash chain,
signatures and stored files:

```bash
cargo run -q -p check -- $HOME/adversaria-tutorial
```

It prints `ok:` with counts of entries, seals, keys and blobs, or the first
fault it finds.

When you are done, delete `$HOME/adversaria-tutorial` and
`$HOME/.adversaria-tutorial.key` if you no longer want them.

docs/guide.md describes every command, the files the memex keeps, the
tools around it, and how to change the keys.

Limits of this tutorial:

- The test performs these steps through the memex's own key events, not
  its window. A separate test (`tests/memex_in_a_window.py`) types into the
  window under Xvfb (X11) and a headless sway (Wayland). Neither is a real
  desktop or keyboard.
- In part C, the boxes are an outline, not a grid, and only whole numbers
  can be supposed.
- The test pauses by calling what the window calls after a second without
  keys; it does not wait a second.
- Views are drawn in characters, on the window's grid; they are not yet
  images.
- The `check` command above is not performed by the tutorial test; the
  acceptance tests check journals made by the memex.
- Holding Super may be taken by your desktop. If it is, give Suppose another
  key (docs/guide.md, § Configuration).
