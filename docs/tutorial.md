# Tutorial: a first session

This tutorial takes about twenty minutes. You will write a short outline,
state facts, add a claim from a source, write a constraint that catches the
conflict, question the claim, adopt it and take it back, and try it in a
supposed world. It follows the design's user story (docs/design.md, § A user
story): notes on the Soviet Buran space shuttle.

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
nix-shell
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

## What next

docs/guide.md describes every command, the files the memex keeps, the tools
around it, and how to change the keys.

## Limits

- The test performs these steps through the memex's own key events, not a
  real terminal. A separate test (`tests/memex_in_a_pty.py`) drives the
  program through a pseudo-terminal with kitty's key encoding. Neither is a
  real kitty, WezTerm or Ghostty, and the memex has not yet been run in one.
- Step 9's `check` command is not performed by the tutorial test; the
  acceptance tests check journals made by the memex.
- Holding Super may be taken by your desktop. If it is, give Suppose another
  key (docs/guide.md, § Configuration).
