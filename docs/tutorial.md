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
defaults; if you have changed them (docs/guide.md, § 4.3 Configuration),
use yours.

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

**If your desktop takes Super.** Some keys start a kind of input:
Left Ctrl a command, Alt a leap, and Super (the Windows or Command key)
a supposition. Many desktops open a launcher or an overview when Super
is pressed, and then the notebook never sees it. Before you start, press
Super once: if anything happens outside the window, give Suppose another
key. Quit the notebook (F10), make the directory for its configuration,

```bash
mkdir -p ~/.config/adversaria
```

and put these lines in `~/.config/adversaria/config.toml`:

```toml
[quasimodes]
suppose = ["RightControl"]
```

Right Ctrl has no other job, so it is free for this. Start the notebook again,
and use **Right Ctrl** wherever this tutorial says Super. (If you set
`XDG_CONFIG_HOME`, the file goes there instead; docs/guide.md,
§ 4.3 Configuration, says where else it may be, and lists every key
name.)

The window is the memex, the notebook. It has a top bar, your outline on
the left, and a second pane on the right. The outline says:

    Type to write the first item.

and, under it, the keys to know, which this tutorial takes in turn.

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

    ? sources disagree under constraint 1.4: 1988 here; 1989 in 1.3

Items are addressed by number, and each shows its address before its
text: 1.3 is the third item under item 1, the claim, and constraint 1.4
is the rule you just wrote. It is a disagreement, not an error, because 1989 is only a claim.
You wrote the rule once; it now checks every date in the notebook,
those you add later and those that arrive from documents and sources.

### 4. Question the claim, adopt it, take it back

*Why: before you rely on a claim, you want to know who else makes it,
and what it would break.*

Move the mouse pointer over the claim. It is underlined, and the foot
says what the mouse does there:

    click: go to 1.3 · right-click for: question its claim; adopt its claims

What the screen shows stands for your notes themselves, so the mouse
acts on them: a right-click offers what can be done with this claim.
The keys do the same. Press Up to put the cursor on the claim, and press
**F2** to question it. The right pane answers five questions:

    INTERROGATE · WHAT IS CLAIMED HERE

It shows who else claims it, `who claims the contrary`, its source, the
source's record, and what adopting it would do:

    adopting it would raise 1 violation of Nothing has more than 1 date

Press **F3** to adopt the claim. It is now a belief, with its source kept.
The top bar says `1 constraint violation`, and the mark is loud:

    ‼ breaks constraint 1.4: 1988 here; 1989 in 1.3

Two beliefs now conflict. Press **F4** to take the adoption back:
`retract`, the item's last entry. The foot asks first,
`Enter retracts · Esc keeps it`; press Enter. The notebook says the
adoption is retracted, and the violation is gone.
Because the source is kept with every adoption, you can take back what
rests on a source if it proves wrong; the adoption and its retraction
both stay in the journal.

### 5. Suppose

*Why: when you do not yet know which source is right, try each without
changing what you believe.*

Tap **Super** (the Windows or Command key; if your desktop takes it,
see "If your desktop takes Super" in step 1): press it and let go, with
nothing typed between. Super opens a command line with `suppose`
already typed: a supposition is a command, `suppose`, and the key saves
you typing it. A tap keeps the line open until you press Enter. The foot
of the screen says `COMMAND (latched)  suppose (that)`, and under it what
to do:

    type a statement · ? lists what may come next · Esc cancels · Enter, or tap again, offers its readings, or a world of that name if none

Type `the flight was in 1989`. The foot now says:

    not ACE: the flight <> was in 1989. · Tab: was → is (or was → wags, was → wars) · Enter, or tap again, offers its readings

That is not a mistake. `not ACE` means the sentence is not in the
controlled English the notebook reads exactly, which has no past
tense; Tab would mend it by changing a word. Leave it as it is, and
press Enter. The notebook reads your sentence against the facts
instead, finds the date it is about, and asks:

    Do you mean: Suppose that the date of Buran's flight is 1989.  Enter: yes · Esc: no

Press Enter. A second world opens beside the actual one, row for row:

    WORLD · SUPPOSE THE FLIGHT WAS IN 1989

and the top bar says `worlds actual | suppose: the flight was in 1989`.
The supposed world's first row is what it supposes, as the notebook
reads it: `supposed: date(buran_flight, 1989)`.

In the supposed world your 1988 fact is set aside, so the rule holds;
the notebook says `set aside in this world: 1.1`. Item 1.1 also states
the flight's label, which stays, so its row is not struck through; it is
marked `(1.1 set aside here: date(buran_flight, 1988))`, the fact set
aside, as the notebook reads it. Under the rule the
supposed world counts `0 violations, 0 disagreements`, then
`1.1 set aside here`; the actual world, beside it, still counts
`1 disagreement`. A count or a row that differs between the worlds is
bold and underlined. Your actual world is
untouched. In part D you will compute a sentence from the dates, which
each world would show its own way.

While a supposition is open, the foot says how to leave it:

    close returns to the actual world · commit makes this world the actual one

Now run a command. Tap **Left Ctrl**, type `close`, and press Enter:
what you type after the tap is a command, shown at the foot of the
screen. You are back in the actual world. The supposition stays in the
journal; the notebook says so: `it stays in the journal`. (`commit`
would instead make the supposed world the actual one.)

While Command is on, `?` lists the commands that match what you have
typed, and Tab completes a command's name; Esc, or Backspace on an
empty line, leaves without running anything. For a few letters you may instead
hold the key while you type and let go to finish. The foot says which you are in: `COMMAND (latched)`
after a tap, and while you hold the key before you press another,
`release now to type without holding`. Every "run the command" below
means either way.

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

Say your own sources name two teams. Reject that change: right-click it,
and the menu offers `reject this change`; or run, by its number,
`retract 53`. The foot of the screen says
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
As you type, the cursor goes to the next item holding what you have
typed so far, past the end and round to the top, and stays when you
let go: on the chief designer's words. Left Alt leaps backward; `?`
while holding lists every match, and `pane matches` keeps that list in
the right pane, numbered, with where the cursor is among them. If no
item holds what you typed, Enter makes an item of it; when an item holds
something a letter or two away, the first Enter only warns, and names
it.

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

### 10. A note linked, followed, and back

*Why: a note is worth more beside what it is about, and you want to get
there and back without losing your place.*

With the cursor on the chief designer's words, press End, then Enter,
for a new item after them, and type `Check this against [[How`. While
the text cursor is in an open `[[`, a dim line under the item lists the
items whose names fit what you have typed:

    [[ How Buran’s flight software was reviewed · Tab completes

Press Tab: the name is completed, and the link closed with `]]`. At the
pause the note links to the essay's heading. The right pane shows the
selected item's links, under its address:

    LINKS OF 2.1.3
    link →
      2. How Buran’s flight software was reviewed  1

Run `follow 1` (or click that line). The cursor goes to the heading,
and its pane lists your note among the items that link to it:

    link this ←
      2.1.3. Check this against [[How Buran’s flight software was reviewed]]  1

Press **F7** (back; a mouse's back button does the same): the cursor
returns to your note. F8 goes forward again. `follow`, Leap, `go` and a
click on a link are jumps, which back retraces; arrow keys are not.

## C. Boxes that compute

*Why: a spreadsheet computes, but does not check itself, and a what-if
overwrites the real numbers. Here rules are checked every time a number
changes, and a what-if sits beside the original.*

### 11. Boxes, rules and a question

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

### 12. Rules between the boxes

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

### 13. Change a value

To change a value, change its sentence. Leap to `food is 4`, press End,
erase `400.` with Backspace, type `900.`, and stop for a second. The old
food value is taken back and the new one read. The answer follows,
`→ -100`, and the top bar says `1 constraint violation`, marked under
the boxes involved. The notebook does not choose how to repair it (lower
the rent? the food?); that is yours. Leap to `food is 9`, and in the
same way make it `500.` Now `→ 300`, and the violation is gone.

### 14. What if

Tap Super, type `rent is 1600`, and press Enter. The notebook asks:

    Do you mean: Suppose that the cell of rent is 1600.  Enter: yes · Esc: no

(`cell` is how the notebook keeps a box.) Press Enter. The two worlds are
shown side by side, row for row. In the supposed one the old rent is
struck through and marked `(2.2 set aside here)`, the answer is
`→ -100`, and the cost rule counts `1 violation`. The notebook says:

    supposed; set aside in this world: 2.2; left for you: 1 violation of It is false that the cost is greater than the income

The old rent is set aside, because a box holds one value. Income and
food stay: with them, the new rent breaks the cost rule, but any of the
three could be the one to change, so the notebook leaves that to you.

Run `close`; the actual budget still says `→ 300`.

## D. Views, and leaving

*Why: a chart drawn from your facts changes as they do, shows what
breaks a rule, and never draws a claim you have not adopted.*

### 15. A chart of the boxes

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
the values above the three bars, `900`, `2000` and `1200`, turn red:
the cost rule's violation rests on the income, rent and food, and so
does every bar. Leap to `food is 9` and
make it `500.` to put it back.

### 16. A sentence, a query and a timeline

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

### 17. Leave

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
show: links made by command, with a type (`link to 2 as supports`),
moving by the outline's structure (Shift with an arrow: `next`,
`parent`), the list of a
Leap's matches (`pane matches`), `#tags`, splitting an item into
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
- The tutorial test does not run on a desktop, so it cannot tell whether
  yours takes Super; step 1 says how to tell, and what to do.
