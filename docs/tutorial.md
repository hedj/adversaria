# Tutorial: a first session

This tutorial takes about forty minutes (an estimate). In part 1 you
will write a short outline, state facts, add a claim from a source, write
a constraint that catches the conflict, question the claim, adopt it and
take it back, and try it in a supposed world. Part 1 follows the design's
user story (docs/design.md, § A user story): notes on the Soviet Buran
space shuttle. In part 2 you will build a small budget that computes
itself from boxes and rules, and in part 3 draw it as a chart and a
table, and part 1's dates as a timeline.

A test performs every step below and checks every screen text quoted here
(`crates/memex/tests/tutorial.rs`). If the memex changes and this tutorial
does not, that test fails. The test drives the memex directly, not through a
terminal; see "Limits" at the end.

The key names below are the defaults. If you have changed them (docs/guide.md,
§ Configuration), use yours.

## 1. Start

You need a terminal with the kitty keyboard and graphics protocols: kitty,
WezTerm or Ghostty. The memex refuses to start in any other, and says which
protocol is missing. Then, from the repository:

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

## 2. Write an outline

Type `The programme`, then press Enter for a new item, and type
`Buran flew once, unmanned, on 15 November 1988.`

Press Tab. The Buran item moves under the first, and the outline shows:

    ▼ 1. The programme
      • Buran flew once, unmanned, on 15 November 1988.

Only top-level items show their number. Items under them are addressed by
path: the first item under 1 is 1.1, the second 1.2, and so on.

Everything you type is written to the journal, a second after you stop or
when you do something else. There is no Save.

## 3. State facts

A fact is a Prolog term that the item states. Facts are what constraints and
derived values read.

Hold **Left Ctrl**. This is the Command quasimode: while you hold it, what
you type is a command, shown at the foot of the screen. Type
`fact date(buran_flight, 1988)` and release Left Ctrl. Releasing runs the
command on the item under the cursor.

Do the same with `fact label(buran_flight, "Buran's flight")`. A label lets
the memex write the fact in words.

Press Enter, type `Energia, its launcher, first flew in May 1987.`, and give
it two facts in the same way:

- `fact date(energia_launch, 1987)`
- `fact label(energia_launch, "Energia's first launch")`

While Left Ctrl is held: `?` lists the commands that match what you have
typed; Esc completes the command name; Backspace erases. A tap of Left Ctrl
(press and release with nothing typed) latches the quasimode, so you can
type without holding it; Enter or a second tap runs the command.

## 4. Derive a sentence

Press Enter and type a placeholder, such as `gap`. Then run this command
(all one line):

`derive The flight came {N:years} after Energia's first launch. where date(buran_flight, B), date(energia_launch, E), N is B - E`

The item now shows text computed from the facts:

    The flight came a year after Energia's first launch. [derived]

If a date changes, the sentence changes with it.

## 5. A claim, and a constraint

Press Enter and type `Talk notes: the 1989 flight was the only one.` This is
what a speaker said, and you have not checked it. Record it as a claim, with
its source:

`claim date(buran_flight, 1989) from my notes, conference talk`

A claim is not a belief. It is something a source says, kept apart from what
you believe until you adopt it.

Now say what must never happen: one event with two dates.

`constraint one date per event: false :- date(E, D1), date(E, D2), D1 \= D2.`

Before keeping a constraint, the memex plants a violation and checks that
the constraint fires on it. It says:

    constraint one date per event kept: it fires on a planted violation

The top bar now says `1 disagreement between sources`, and under the Buran
item is a mark:

    ⚑ one date per event, sources disagree: 1988 here, 1989 in 1.4

It is a disagreement, not a violation, because the 1989 date is only a claim.

## 6. Leap

Leap moves the cursor by searching, not by arrowing. Hold **Right Alt**
(Leap forward), type `buran`, and release. The cursor goes to the next item
holding that text. The right pane shows the constraint behind the item's
mark:

    CONSTRAINT · ONE DATE PER EVENT

Left Alt leaps backward. Esc completes the pattern; `?` lists every match.
Tapping the Leap key twice repeats the last Leap.

## 7. Question the claim, adopt it, take it back

Leap to `talk`. Press **F2** to interrogate the claim. The right pane
answers five questions:

    INTERROGATE · WHAT IS CLAIMED HERE

It shows who else claims it, `who claims the contrary`, its source, the
source's record, and what adopting it would do:

    adopting it would raise 1 violation of one date per event

Press **F3** to adopt the claim. It is now a belief, with its source kept.
The top bar says `1 constraint violation`, and the mark is loud:

    ⚑ one date per event, violation: 1988 here, 1989 in 1.4

Press **F4** to take the adoption back. The memex says the adoption is
retracted, and the violation is gone. The adoption and its retraction both
stay in the journal.

## 8. Suppose

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
`set aside in this world: 1.1`. The actual world is untouched.

Run the command `close` to return to the actual world. The supposition
stays in the journal; the memex says so: `it stays in the journal`.
(`commit` would instead make the supposed world the actual one.)

## 9. Leave

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

## Part 2: boxes that compute

The memex has no spreadsheet. But items can hold values, derived text can
compute from values, and constraints can relate them, and that is most of
what a spreadsheet does. This part builds a small household budget that
way. Nothing in it is special to budgets or spreadsheets: `cell` below is
a name chosen here, not something the memex knows.

Start the memex again in the same journal, with the same `make run`
command as in step 1. Part 1's outline is still there.

## 10. A box that holds a value

Leap to `talk` (the last item of part 1), press Enter for a new item and
Shift-Tab to bring it to the top level, and type `Budget`. Press Enter,
type `income`, and press Tab to put it under Budget.

Give the item a value, as a fact, and make it show that value:

- `fact cell(income, 2000)`
- `derive Income: {V} where cell(income, V)`

The item now reads:

    Income: 2000 [derived]

The text you typed (`income`) stays in the journal; the item shows the
derived text instead, and will follow the value when it changes.

## 11. More boxes, and one computed from them

Make two more boxes the same way. Press Enter, type `rent`, then:

- `fact cell(rent, 1200)`
- `derive Rent: {V} where cell(rent, V)`

Press Enter, type `food`, then:

- `fact cell(food, 400)`
- `derive Food: {V} where cell(food, V)`

Now a box computed from the others. Press Enter, type `left`, then:

`derive Left over: {L} where cell(income, I), cell(rent, R), cell(food, F), L is I - R - F`

It reads:

    Left over: 400 [derived]

## 12. Rules between the boxes

Two constraints. The first says a box holds one value; the second, that
spending must not exceed income:

`constraint one value per cell: false :- cell(C, A), cell(C, B), A \= B.`

`constraint spending within income: false :- cell(income, I), cell(rent, R), cell(food, F), R + F > I.`

Each is tried against a planted violation before it is kept, as in step
5:

    constraint spending within income kept: it fires on a planted violation

## 13. Change a value

`set` states a fact and takes back the facts it directly contradicts:
here, because of the rule "one value per cell", the old food value. It
does not matter where the cursor is; the new value goes on the Food box.

`set cell(food, 900)`

The memex says:

    set; replaced the fact at 2.3; left for you: 1 violation of spending within income

Every box that depends on food is recomputed at once: the screen shows
`Food: 900 [derived]` and `Left over: -100 [derived]`, and the top bar
says `1 constraint violation`, marked under the boxes involved. The memex
does not choose how to repair it (lower the rent? the food?); that is left
to you. Put food back within income:

`set cell(food, 500)`

Now `Left over: 300 [derived]`, and the violation is gone.

## 14. What if

To see what a change would do without making it, suppose it, as in step
8. Hold Super, type `rent is 1600`, and release. The memex asks:

    Do you mean: Suppose that the cell of rent is 1600.  ⏎ yes · Esc no

Press Enter. The two worlds are shown side by side, box for box: in the
supposed one, the old rent is set aside and `Left over: -100`. The memex
says:

    supposed; set aside in this world: 2.2; left for you: 1 violation of spending within income

Only the old rent is set aside. Income and food stay: with them, the new
rent breaks "spending within income", but any of the three could be the
one to change, so the memex leaves that violation for you rather than
choosing.

Run `close` to go back; the actual budget still says
`Left over: 300 [derived]`. (`commit` would have made the new rent real.)

## Part 3: views

A view is an item that draws something, made of layers. A layer is a
mark, such as a bar or a box, drawn once for each solution of a goal. As
before, nothing here is a chart type the memex knows: a chart, a table
and a timeline are marks over your own facts.

## 15. A chart of the boxes

Leap to `left over`, press Enter, and Shift-Tab to the top level. Type
`Spending chart`. Then add two layers to it:

- `layer bar(x(C), y(V)) where cell(C, V)`
- `layer rule(y(I)) where cell(income, I)`

The first draws a bar for each box, at the box's name and as high as its
value; the second, a line across the chart at the income. The chart is
drawn under the item, and the right pane says `VIEW · 2 LAYERS`, with how
many marks each layer draws.

Run `set cell(food, 900)` again. The food bar rises, and all three bars
are drawn bold: the violation of "spending within income" rests on the
income, rent and food facts, and so does every bar. Run `set cell(food, 500)` to
put it back. With the mouse on (docs/guide.md, § The mouse), a click on
a bar goes to the box that holds its value.

## 16. The same boxes as a grid

Press Enter and type `Budget table`. Then:

`layer cell(x(value), y(C), text(V)) where cell(C, V)`

A `cell` mark is a box in a grid, at column x and row y. The view is a
table, rows in order:

    food   │   500
    income │  2000

A spreadsheet is the same idea with more columns, and with boxes whose
values come from rules (docs/guide.md, § 9).

## 17. A timeline from part 1

Part 1's facts are still here. Press Enter, type
`Timeline of the programme`, then:

`layer tick(x(Y), text(L)) where date(E, Y), label(E, L)`

Each event is a tick on a line of years, labelled from its `label` fact:
`Energia's first launch` in 1987, then `Buran's flight` in 1988. The talk
notes' 1989 is not drawn: it is a claim, not a belief, and a view draws
beliefs.

docs/guide.md, § 9 has more: line charts, Gantt charts, flowcharts, and
your own outline drawn as a tree.

## What next

docs/guide.md describes every command, the files the memex keeps, the tools
around it, and how to change the keys.

## Limits

- The test performs these steps through the memex's own key events, not a
  real terminal. A separate test (`tests/memex_in_a_pty.py`) drives the
  program through a pseudo-terminal with kitty's key encoding. Neither is a
  real kitty, WezTerm or Ghostty, and the memex has not yet been run in one.
- In part 2, the boxes are an outline, not a grid, and only whole numbers
  can be supposed.
- Views are drawn in characters, in the terminal; they are not yet images.
- Step 9's `check` command is not performed by the tutorial test; the
  acceptance tests check journals made by the memex.
- Holding Super may be taken by your desktop. If it is, give Suppose another
  key (docs/guide.md, § Configuration).
