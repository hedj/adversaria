# Tutorial: a first session

This tutorial takes about forty minutes (an estimate), in four parts, in
one session. docs/introduction.md says in a page what the notebook is
for; this is the same, done.

- A. A claim, checked (about ten minutes): you write a fact, record what
  a speaker said against it, and see the notebook catch the conflict.
  You question the claim, try adopting it, and try what follows if the
  speaker were right, without changing what you believe.
- B. Your documents: you import a draft, take in a co-author's revision
  change by change, and catch a quotation that no longer says what its
  source says.
- C. Boxes that compute: a small budget, written in sentences, that
  answers a question and checks itself every time a number changes.
- D. Views, and leaving: the budget as a chart that shows what breaks a
  rule, a sentence and a timeline computed from part A's dates, and why
  the record is signed.

The notes are about the Soviet Buran space shuttle, after the design's
user story (docs/design.md, § A user story). The key names are the
defaults; if you have changed them (docs/guide.md, § Configuration), use
yours.

## A. A claim, checked

### 1. Start

You need a desktop under X11 or Wayland; the notebook opens a window of
its own. Then, from the repository:

```bash
./dev-shell
```

```bash
make run JOURNAL=$HOME/adversaria-tutorial KEY=$HOME/.adversaria-tutorial.key
```

The first run makes a signing key and a new journal. The journal is
permanent: nothing you do is ever erased from it. That is why this tutorial
uses its own journal, which you can delete when you are done.

The window is the memex, the notebook. It has a top bar, your outline on
the left, and a second pane on the right. The outline says:

    (empty: type to write the first item)

### 2. A fact, and a note

*Why: a fact is a sentence the notebook can check against what others
say, and compute from. Everything else is a note for you.*

Type `The programme`, press Enter for a new item, and press Tab to put
it under the first. Type

`The date of Buran-flight is 1988. The label of Buran-flight is "Buran's flight".`

Press Enter and type an ordinary note:

`Buran flew once, unmanned, on 15 November 1988.`

Stop typing for a second. There is no Save: what you type is kept a
second after you stop. The outline shows:

    ▼ 1. The programme

The first item under it has turned cyan: the notebook has read it as two
facts. It reads Attempto Controlled English (ACE), English with one
meaning per sentence. `Buran-flight`, with a capital and a hyphen, is
the name of one thing; its label is how the notebook will write it in
words.

The note stays in the usual colour. It is not ACE (ACE has only the
present tense), so it stays prose, and a dim line at the cursor says
where the reading stopped:

    prose: not ACE, it stops at Buran <> flew once, unmanned, on 15 November 1988.

Nothing is wrong: most of a notebook is prose. Only short, plain
sentences such as the first become facts.

Press Up to put the cursor on the fact. The right pane shows, under
`READ AS`, what the notebook read, in Prolog, the language it reasons in:

    fact  date(buran_flight, 1988)
    fact  label(buran_flight, "Buran's flight")

You need not read Prolog. It is there so you can see that the notebook
understood you. Press Down to go back to the note.

### 3. A claim, and a rule that catches it

*Why: notes mix what you checked with what someone told you. Here the
notebook keeps them apart, and notices when they disagree.*

Press Enter and type what a speaker said at a talk, which you have not
checked:

`Talk-notes says that the date of Buran-flight is 1989.`

This is a claim: what a source, Talk-notes, says. It is kept apart from
what you believe until you adopt it. Now say what must never happen,
one thing with two dates. Press Enter and type

`Nothing has more than 1 date.`

Stop for a second. The notebook reads it as a rule, a constraint, and
says:

    constraint Nothing has more than 1 date kept: it fires on a planted violation

Before keeping a rule, the notebook tries it on a made-up case that
should break it (a "planted violation"). A rule that could never fire
would give false comfort, so such a rule is refused. This one fires, so
it is kept.

The top bar now says `1 disagreement between sources`, and under your
fact is a mark:

    ⚑ Nothing has more than 1 date, sources disagree: 1988 here, 1989 in 1.3

Items are addressed by number: 1.3 is the third item under item 1, the
claim. It is a disagreement, not an error, because 1989 is only a claim.
You wrote the rule once; it now checks every date in the notebook,
those you add later and those that arrive from documents and sources.

### 4. Question the claim, adopt it, take it back

*Why: before you rely on a claim, you want to know who else makes it,
and what it would break.*

Press Up to put the cursor on the claim, and press **F2** to question it.
The right pane answers five questions:

    INTERROGATE · WHAT IS CLAIMED HERE

It shows who else claims it, `who claims the contrary`, its source, the
source's record, and what adopting it would do:

    adopting it would raise 1 violation of Nothing has more than 1 date

Press **F3** to adopt the claim. It is now a belief, with its source kept.
The top bar says `1 constraint violation`, and the mark is loud:

    ⚑ Nothing has more than 1 date, violation: 1988 here, 1989 in 1.3

Two beliefs now conflict. Press **F4** to take the adoption back. The
notebook says the adoption is retracted, and the violation is gone.
Because the source is kept with every adoption, you can take back what
rests on a source if it proves wrong; the adoption and its retraction
both stay in the journal.

### 5. Suppose

*Why: when you do not yet know which source is right, try each without
changing what you believe.*

Hold **Super** (the Windows or Command key) and type
`the flight was in 1989`. Holding a key while you type is how the
notebook takes a command of one kind: here, a supposition. Release it.
The notebook reads your sentence against the facts and asks:

    Do you mean: Suppose that the date of Buran's flight is 1989.  ⏎ yes · Esc no

Press Enter. A second world opens beside the actual one, row for row:

    WORLD · SUPPOSE THE FLIGHT WAS IN 1989

In the supposed world your 1988 fact is set aside, so the rule holds;
the notebook says `set aside in this world: 1.1`, and the rule's row
says `0 violations · 1.1 retracted here`. Your actual world is
untouched. In part D you will compute a sentence from the dates, which
each world would show its own way.

Now run a command. Hold **Left Ctrl**, type `close`, and release: what
you type while Left Ctrl is held is a command, shown at the foot of the
screen. You are back in the actual world. The supposition stays in the
journal; the notebook says so: `it stays in the journal`. (`commit`
would instead make the supposed world the actual one.)

While Left Ctrl is held, `?` lists the commands that match what you have
typed, and Esc completes a command's name. A tap of Left Ctrl (press and
release with nothing typed) keeps it on without holding; Enter runs the
command.

## B. Your documents

### 6. Import a draft

*Why: a draft and its sources belong in one place, where the same rules
check both.*

Run the command

`import document docs/tutorial/draft/essay.md`

(the path is from the repository, where `make run` started; Word,
OpenDocument, LaTeX and HTML documents import the same way). The
notebook says `imported essay.md: 6 items; losses: none`, and the essay
is in your outline, its headings as levels:

    ▼ 2. How Buran’s flight software was reviewed

The last item says `Two teams reviewed the flight software.`

### 7. A co-author's revision

*Why: when a co-author sends a new version, you want to see each change
and choose, not just receive the new text.*

Your co-author has revised the essay. Import their version, under the
same file name:

`import document docs/tutorial/revised/essay.md`

The notebook compares it with what you imported before, and opens the
changes in a world of their own, one entry each:

    WORLD · REVISION OF ESSAY.MD

    #53  changed 2.2.1: “Two teams reviewed the flight software.” → “Three teams reviewed the flight software, one of them outside the programme.”
    #54  added: “Each team wrote its findings in the DRAKON language.”

Say your own sources name two teams. Reject that change by its number:
run `retract 53`. The foot of the screen says
`nothing depends on it · Enter retracts · Esc keeps it`; press Enter.
Then run `commit` to accept the rest. The essay keeps
`Two teams reviewed the flight software.` and gains
`Each team wrote its findings in the DRAKON language.` If your numbers
differ (they count every entry, so a typo adds one), use the numbers on
your screen.

### 8. Leap

*Why: an outline soon grows past a screen. Leap moves by what you
remember, not by arrowing.*

Hold **Right Alt** (Leap forward), type `fully automatic`, and release.
The cursor goes to the next item holding that text, past the end and
round to the top: the chief designer's words. Left Alt leaps backward;
`?` while holding lists every match.

### 9. A quotation checked

*Why: a quotation retyped, shortened or edited can stop saying what its
source says, and no one notices.*

Change the quotation as a careless edit might. With the cursor on it
(leap to `fully automatic` again if it has moved), press End, erase
`fully automatic.` with Backspace, and type `automatic.` Then run

`check quote`

The notebook compares the item with the document it came from, as it
was imported, and says `quote check: misquoted`. Put the words back:
leap to `landing was automatic`, press End, erase `automatic.` and type
`fully automatic.`, and run `check quote` again: `quote check: quoted`.

(`check source`, on an item that states a paper's DOI, asks Crossref
whether the paper exists, is what you claim, or has been retracted. It
needs the network, so this tutorial does not run it.)

## C. Boxes that compute

*Why: a spreadsheet computes, but does not check itself, and a what-if
overwrites the real numbers. Here rules are checked every time a number
changes, and a what-if sits beside the original.*

### 10. Boxes, rules and a question

Leap to `nothing has` (the rule of part A), press Enter for a new item,
and Shift-Tab to bring it to the top level. Type `Budget`. Press Enter,
then Tab to put the new item under Budget, and write one sentence per
item, pressing Enter between them:

- `The income is 2000.`
- `The rent is 1200.`
- `The food is 400.`
- `If the rent is X and the food is Y then the cost is X + Y.`
- `If the income is X and the cost is Y then the rest is X - Y.`
- `What is the rest?`

Stop for a second. Every item turns cyan. "The income", with no owner,
is a box: a value with a name. The cost and the rest are not boxes you
set; rules compute them. The question is answered on the line under it:

    → 400

### 11. Rules between the boxes

Spending must not exceed income, and a box holds one value. Press Enter
and type

`It is false that the cost is greater than the income.`

and stop for a second. As in step 3, it is tried on a made-up case
before it is kept:

    constraint It is false that the cost is greater than the income kept: it fires on a planted violation

Press Enter, type `Nothing has more than 1 value.` and stop. The
notebook says
`constraint Nothing has more than 1 value kept: it fires on a planted violation`.
It says of boxes what step 3 said of dates.

### 12. Change a value

To change a value, change its sentence. Leap to `food is 4`, press End,
erase `400.` with Backspace, type `900.`, and stop for a second. The old
food value is taken back and the new one read. The answer follows,
`→ -100`, and the top bar says `1 constraint violation`, marked under
the boxes involved. The notebook does not choose how to repair it (lower
the rent? the food?); that is yours. Leap to `food is 9`, and in the
same way make it `500.` Now `→ 300`, and the violation is gone.

### 13. What if

Hold Super, type `rent is 1600`, and release. The notebook asks:

    Do you mean: Suppose that the cell of rent is 1600.  ⏎ yes · Esc no

(`cell` is how the notebook keeps a box.) Press Enter. The two worlds are
shown side by side, box for box; in the supposed one the answer is
`→ -100`. The notebook says:

    supposed; set aside in this world: 2.2; left for you: 1 violation of It is false that the cost is greater than the income

The old rent is set aside, because a box holds one value. Income and
food stay: with them, the new rent breaks the cost rule, but any of the
three could be the one to change, so the notebook leaves that to you.

Run `close`; the actual budget still says `→ 300`.

## D. Views, and leaving

*Why: a chart drawn from your facts changes as they do, shows what
breaks a rule, and never draws a claim you have not adopted.*

### 14. A chart of the boxes

Leap to `what is the rest`, press Enter, and Shift-Tab to the top level.
Type `Spending chart`. Then run two commands, which draw it in layers, a
mark for each answer to a query:

- `layer bar(x(C), y(V)) where cell(C, V)`
- `layer rule(y(I)) where cell(income, I)`

The first draws a bar for each box, as high as its value; the second, a
line across the chart at the income. The chart is drawn under the item,
and the right pane says `VIEW · 2 LAYERS`. These are Prolog queries;
docs/guide.md, § 3.3 has recipes to copy for line charts, tables, Gantt
charts and flowcharts.

Leap to `food is 5` and make it `900.` again. The food bar rises, and
all three bars are drawn bold: the cost rule's violation rests on the
income, rent and food, and so does every bar. Leap to `food is 9` and
make it `500.` to put it back.

### 15. A sentence, a query and a timeline

Leap to `buran flew once, unmanned`, press Enter, and type a second date:

`The date of Energia-launch is 1987. The label of Energia-launch is "Energia's first launch".`

Stop for a second; it turns cyan. Press Enter and type a placeholder,
such as `gap`. Then run (all one line):

`derive The flight came {N:years} after Energia's first launch. where date(buran_flight, B), date(energia_launch, E), N is B - E`

The `where` part is a query: it finds the two dates and their
difference. The item now shows text computed from the facts:

    The flight came a year after Energia's first launch. [derived]

If a date changes, the sentence changes with it; in a supposition, each
world shows its own. To run a query by itself, run `query date(E, Y)`.
It finds every date the notebook believes, and says:

    2 answers: E = buran_flight, Y = 1988 · E = energia_launch, Y = 1987

Press Enter, type `Timeline of the programme`, then run:

`layer tick(x(Y), text(L)) where date(E, Y), label(E, L)`

Each event is a tick on a line of years, labelled in words:
`Energia's first launch` in 1987, then `Buran's flight` in 1988. The
talk notes' 1989 is not drawn: it is a claim, not a belief, and a view
draws beliefs.

### 16. Leave

Press **F10** to quit. What was pending is written first.

The sealer, started by `make run`, signs and dates every entry within a
fraction of a second. So you can show that a note existed, unchanged, on
a date: for a lab notebook, to show priority, or to trust your own
record. To check the whole journal, its chain, signatures and stored
files:

```bash
cargo run -q -p check -- $HOME/adversaria-tutorial
```

It prints `ok:` with counts of entries, seals, keys and blobs, or the
first fault it finds.

When you are done, delete `$HOME/adversaria-tutorial` and
`$HOME/.adversaria-tutorial.key` if you no longer want them.

## Where next

docs/guide.md describes every command, the files the notebook keeps, the
tools around it, and how to change the keys. Things this tutorial did not
show: links typed as `[[Buran flight]]`, `#tags`, splitting an item into
sentences, Wikidata as a public source to check dates against, and
results from a Python analysis, which are marked when their input data
changes (§ 4.1).

A test performs every step above and checks every screen text quoted
here (`crates/memex/tests/tutorial.rs`). If the notebook changes and this
tutorial does not, that test fails. Limits of this tutorial:

- The test performs the steps through the memex's own key events, not
  its window. A separate test (`tests/memex_in_a_window.py`) types into
  the window under Xvfb (X11) and a headless sway (Wayland). Neither is a
  real desktop or keyboard.
- The test pauses by calling what the window calls after a second without
  keys; it does not wait a second.
- In part C the boxes are an outline, not a grid, and only whole numbers
  can be supposed.
- Views are drawn in characters, on the window's grid; they are not yet
  images.
- The `check` command above is not performed by the tutorial test; the
  acceptance tests check journals made by the memex.
- Holding Super may be taken by your desktop. If it is, give Suppose
  another key (docs/guide.md, § Configuration).
