# User guide

This guide describes the Adversaria prototype as built: the memex, which is
the notebook you use in a window, and the programs around it. For what it
is for, in a page, read docs/introduction.md; for a first session, follow
docs/tutorial.md. For why it is built this way, see
docs/design.md; for what has been tested and what has not, docs/log.md.

Contents, four parts of four:

1. Concepts: eleven ideas, the record, facts claims and beliefs,
   constraints and worlds, and one command shape with seventeen verbs.
2. Working: starting, the screen, keys and quasimodes, commands and
   addresses.
3. Reasoning: facts rules and constraints, worlds, views, suggestions and
   Wikidata.
4. Around the memex: documents and results, files and tools,
   configuration, troubleshooting and limits.

## 1. Concepts

The notebook is built from eleven ideas, and you act on them with
seventeen verbs, each in one command shape. Five of the ideas are enough
for a first day; they are marked *first day*, and the tutorial's part A
teaches them (docs/tutorial.md). The rest you meet when you need them:
`?` in Command lists what may come next, and a right-click offers what
can act on what is under the pointer.

### 1.1 Eleven ideas

#### Item (first day)

For writing and organising notes. Type; Enter makes a new item, Tab and Shift+Tab indent and outdent; `new`, `delete`, `move`, `copy`.

#### Statement (first day)

For saying what the notebook can reason with. Write a sentence in an item: `The rent is 1200.` It turns cyan when it is read; the inspector shows what it was read as. What controlled English cannot say is written in Prolog between backticks.

#### Claim (first day)

For keeping what a source says apart from what you hold. Write `Talk-notes says that the date of Buran-flight is 1989.`; `adopt` it (F3) to hold it, `retract` the adoption (F4); `dismiss` a suggestion, which is a claim the notebook makes.

#### Finding (first day)

For noticing a conflict when it arrives. Write a constraint once, `Nothing has more than 1 date.`; each break is a finding, a line under the items involved; `adopt fix` where a finding offers a fix.

#### World (first day)

For trying "what if?" without changing what you hold. `suppose` (the Super key); the two worlds stand side by side; `adopt world` makes the supposed one actual, `dismiss world` drops it.

#### Link

For connecting items, and grouping them. `[[Buran flight]]` in an item's text links it to the page of that name, and `#draft`, a label, is the same link, for a word in running text. A page is the item of its name; with none it is made of what links to it. `link to 9 as cites` links by command, with a kind. The inspector shows an item's links both ways, and its pages; `find #draft` lists what links to a page, and a page one item alone links to addresses it: `go draft`.

#### Entry

For losing nothing, and proving the record. Every change is an entry in the journal (§ 1.2); `undo` and `redo` (Ctrl+Z, Ctrl+Y); `retract entry 12` takes back an old one, keeping what came after. The notebook's inspector ends with the latest, each by its number.

#### Trail

For getting back after a jump. `go back` (F7) and `go forward` (F8), as in a browser.

#### Find

For getting to things. Leap: hold Alt and type, and the cursor goes to the match; `find` lists what matches a text, a label, a kind of finding or a question; `go` goes to an address.

#### View

For seeing facts as a chart, a table or a graph. `show as chart`, drawn from the world's facts; behind it, the marks a view is made of (§ 3.3).

#### Document

For exchanging with files outside. `import`, `export`; a co-author's revision opens as a world of changes, one entry each (§ 4.1).

The pane beside the outline is the **inspector**: it shows what is known
about the item the cursor is on (its links, its findings, what its text
was read as, its claims and who else makes them, its view's layers),
and it never takes the keys. `find` shows a list in its place.

### 1.2 The record

**Journal.** So that nothing is lost, and you can always go back,
everything you do is an entry in an append-only journal: a file of JSON
lines, each joined by a hash to its parent, the entry it follows on its
branch (after an undo or in a supposed world, not always the line before
it). Nothing is ever overwritten or erased, except by `purge` (§ 4.2).
There is no Save: what you type is written a second after you stop, or
when you do anything else. Undo is an entry too.

**Seal.** So that you can show a note existed, unchanged, on a date (for
a lab notebook, for priority, or to trust your own record), a separate
program, the sealer, holds a signing key and signs the journal's entries,
dated. The memex itself never sees the key.

### 1.3 Facts, claims and beliefs

Notes mix what you checked with what someone told you; these keep them
apart. A fact is a statement the notebook can check and compute from,
written as a short English sentence (`The date of Buran-flight is
1988.`) and kept as a Prolog term (`date(buran_flight, 1988)`). A fact
you state is a belief. A fact from a source (a talk, a paper, Wikidata,
an analysis, or the notebook's own suggestions) is a claim
(`Talk-notes says that the date of Buran-flight is 1989.`). A claim
becomes a belief only when you adopt it, and the adoption keeps its
source, so what rests on a source can be found, and taken back if the
source proves wrong.

The sentences the notebook reads, and Prolog in backticks for what they
cannot say:

| Form | Example | Kind |
| --- | --- | --- |
| a fact | `The rent is 1200.` | a belief |
| what a source says | `Talk-notes says that the date of Buran-flight is 1989.` | a claim |
| if, then | `If the rent is X and the food is Y then the cost is X + Y.` | a rule |
| what must never be | `Nothing has more than 1 date.` | a constraint |
| a question | `What is the rest?` | answered under its item, and kept current |
| Prolog, in backticks | `` `box(rent, 1200).` `` | as Prolog says: a fact, a `claims(Source, Fact)`, a rule `:-`, a constraint `false :- …`, a question `?- …`; with `{N}` in the text around it, the text filled from the goal |

### 1.4 Constraints and worlds

**Constraint.** So that a conflict is found when it arrives, not months
later, a constraint says what must never be: `Nothing has more than 1
date.`, or in Prolog a denial, `false :- ...`. Written once, it checks
every fact, now and later. When beliefs break it, that is a
**violation**. When a claim and a belief, or two claims, would break it,
that is a **disagreement between sources**. Each is a finding, shown
under the items involved. Before it is kept, a constraint is tried on a
made-up case that should break it (a planted violation): one that could
never fire would give false comfort, so it is refused.

**World.** So that you can try "what if?" without changing what you
believe, the facts as they stand are the actual world, and a supposition
opens a second world in which something else holds; the two are shown
side by side. `dismiss world` leaves it, and it stays in the journal;
`adopt world` makes it the actual world. A co-author's revision of a
document opens in a world too, one change per entry (§ 4.1).

### 1.5 One command shape

A command is a verb, then what it acts on, each part after its guide
word: `link 3.2 (to) source 9 (as) cites`. The memex shows the guide
words in parentheses as you type; § 2.4 says which you may type. Unless
it names an item, a command acts on the **target**, the item the cursor
is on. Typed alone, a command takes what is selected (§ 2.3, Selecting)
for its first part, if that part is of its kind: `delete` deletes the
items or the characters selected, and `find`, `go`, `suppose` and
`import` take the text selected. The cursor's item is no text, so it
fills none of those. A command that acts on one item refuses when
several are selected. The seventeen verbs:

| Verb | Acts on | Its opposite |
| --- | --- | --- |
| `new` | an item | `delete` |
| `delete` | the items or text selected, an item, or the word at the text cursor | `new` |
| `move` | an item, after a place | |
| `copy` | an item, after a place | |
| `link` | an item, to an item, as a type | |
| `go` | an address, a link, the trail, or a way along the outline | |
| `find` | a text, a label, a kind of finding, or a question | |
| `adopt` | claims, a suggestion, a fix or a world | `dismiss` |
| `dismiss` | a suggestion, a world, or a change of a revision | `adopt` |
| `retract` | an entry, or the target's last | |
| `undo` | the last change | `redo` |
| `redo` | the last change undone | `undo` |
| `suppose` | a statement | |
| `import` | a document, results, or a Wikidata subset | `export` |
| `export` | the outline, or a section, to a file | `import` |
| `check` | a quotation, or a source | |
| `show` | an item, as a chart, a table or a graph | |

Three more are listed: `transclude` and `fold`, until what replaces them
is built (a link of type `shows`, a click on a bullet), and `switch`,
which moves between two outlines side by side (`pane outline` puts a
second one beside the first). Every older command name still runs,
typed whole; § 2.4 lists them with what replaces each.

An **address** names an item in three forms: its number (`1.2`), the
start of its text or a page's name (`Energia`, `draft`), or,
for an entry, `entry 12`.

Type a verb alone, and the memex fills in what it can. The foot shows
each part it will fill, with its guide word, its kind and its default:
`move (after) ‹item› [3]` moves the target after item 3, the item you
last jumped from. A part with no default now is asked for: Command stays
open, latched, with the verb typed, and the message says why there is no
default. Type the part or point at it (§ 2.3), then press Enter; Esc
leaves the command. § 2.4, A verb alone, lists the defaults.

## 2. Working

### 2.1 Starting

The memex needs:

- a desktop under X11 or Wayland: the memex opens a window of its own and
  draws through Vulkan or OpenGL;
- SWI-Prolog (`swipl`) on the path, for reasoning;
- APE, the parser of controlled English, found through `APE_HOME`, for
  reading text and Suppose; without it they say "APE is not
  installed";
- Pandoc, for importing and exporting documents;
- curl, for the source check.

`./dev-shell` gives a shell with all of these, Mesa's drivers included, with
`APE_HOME` set, and needs no experimental Nix features. It runs `nix-shell`
with the pinned nixpkgs on the search path, because `nix-shell` looks for an
interactive bash in `<nixpkgs>`; plain `nix-shell` works too, but without
nixpkgs on your `NIX_PATH` it prints an error and uses your system's bash.
With flakes enabled, `nix develop` gives the same shell. All use the
versions pinned in `flake.lock`; the shell is defined in `nix/devshell.nix`.

To start the sealer and the memex together:

```bash
make run
```

This uses the journal directory `~/adversaria-journal` and the key file
`~/.adversaria.key`, making both on first use. Give others with
`JOURNAL=` and `KEY=`:

```bash
make run JOURNAL=$HOME/notes KEY=$HOME/.notes.key
```

`make run` passes the memex no flags; for `--mouse` or `--config`, run it
alone, on a journal directory, which it makes if needed:

```bash
cargo run -q --release -p memex -- [--config FILE] [--mouse | --no-mouse] <journal directory>
```

Without the sealer, entries are written but not sealed until a sealer runs.

### 2.2 The screen

- **Top bar:** the notebook's name (its journal directory's,
  `notebook …`) and the world (`world actual`, or `worlds actual |
  suppose: …`). On the right, a count of violations and disagreements.
  In a narrow window the right side takes a row of its own. Only an
  outline takes the keys; the outline with focus has its heading
  bright, and the pane beside it, which shows links or a list, has its
  heading dim.
- **Left pane:** the outline. When it runs past the pane, the pane's
  last row says how many items lie above and below (`↑ 6 items above ·
  ↓ continues below`); the outline scrolls to keep the cursor in view.
  The cursor's item is tinted blue-grey across the pane, with a brighter
  blue bar beside it in the margin. `▼` marks an item with items under it, `▶` a
  folded one (followed by how many items it hides), `•` an item with none.
  Each item shows its address (`1.`, `1.3.`), as the foot, the menus
  and the commands quote it. Under an item, `‼` and `?` lines
  are its findings: one for each constraint its facts break or disagree under,
  naming the constraint (`constraint 1.4` when an item states it, else its
  name in quotes), the item's own fact, and each other fact in the
  conflict with where it is: `‼ breaks constraint 2.7: income 2000 here; rent
  1200 in 2.2; food 900 in 2.3`. A
  violation (beliefs that break a rule) is red and marked `‼`; a
  disagreement between sources (a claim against a belief or another
  claim) is amber and marked `?`. The item that states the rule has a
  bullet of the same colour, and the top bar's counts are coloured so. An item that shows
  derived text ends in `[derived]`; one that states a claim ends in its
  source, in brackets. A result whose input files have changed says `input
  changed since:` and the files. Text the memex has read is cyan; under a
  question, `→` gives its answer. Under the item at the cursor, a dim line
  says why its text was not read (or, for a sentence APE can mend, `not
  read: did you mean “…”? adopt fix mends it`), or, while the text cursor
  is in a `[[link]]` that names no item, which of its links name none yet
  (§ 2.4, § 3.1).
- **Right pane, the inspector:** what is known about the item the cursor
  is on. By default its links,
  under `LINKS OF` and the item's address, or `no links`, each with the
  text of the item at its other end, grouped by kind: those from it as
  `cites →`, those into it as `cited by ←`, `supported by ←` or `opposed
  by ←`, and a kind of your own as `precedes this ←`; under `pages →`, the
  page of each of its labels and of each `[[link]]` that names no item
  (`#draft → 1. Draft`, or `#urgent · no item · linked from 2`); and, on
  an item that is a page, under `labelled by ←`, the items whose labels
  name it (§ 2.4, Labels). Then, under `mentioned in, not
  linked ←`, the items whose text names this one's as whole words, in any
  case, without linking to it (only a text that reads as a name, as for
  unlinked-mention suggestions, § 3.4), each a click away; on an item with
  findings, the constraint, its rule, its solutions and what depends on them;
  on a view, its layers. Under the links or the constraint, `READ AS` and
  the address show what the item's text was read as, in Prolog, and its
  tags (§ 3.1). On an item that states a claim, the pane then shows each
  claim in the two rows of any offer (`claim  2: …`, then, dim, `from` its
  source; dim, ending `(adopted)`, once adopted), and asks of it who else
  claims it, who claims the contrary, the source's record, and what
  adopting it would do (`CLAIMS OF 2 · WHO SAYS THEM`). The inspector follows the cursor and never takes the keys.
  `find` shows a list in its place, and `find` alone shows the inspector
  again (§ 2.4). With the cursor above the first item, on the notebook
  itself, the inspector lists what waits for you to decide, as the next
  part tells.
- **Foot:** the message line, and, while a quasimode is on (held or
  latched), what you have typed and how it ends.

With the cursor on an item with findings, the foot suggests a what-if
(`Try a what-if: tap Super to suppose a statement, …`), dim, until you have used Suppose
four times.

#### The notebook

Above the first item is the notebook itself, and there the inspector
lists what waits for you to decide (`THE NOTEBOOK · 3 OPEN OFFERS`), the
most pressing first: each finding, from the rule it comes from; each
claim not adopted, from its source; a text you just wrote that the memex
can mend; then the memex's suggestions. Each takes two rows: what it is
and what it offers (`claim  2: the date of Buran-flight is 1989`), then,
dim, where it came from (`from Talk-notes`). An offer takes the same two
rows wherever it is listed. A click on one goes to its
item or its rule, or adopts a suggestion, as a click on the thing itself
does; a right-click offers what decides it, such as `adopt` or
`dismiss`. The first twelve are listed, and the rest are counted.
`go parent` from an item at the top level goes there (Ctrl+Left), and so
does a click on the outline's title. A revision's changes are not among
them: while a revision is open they fill the screen (§ 4.1).

Below the offers, under `HISTORY · NEWEST FIRST`, are the latest eight
steps of this branch: each change, retraction, undo and redo, and each
jump, by its entry's number (`entry 17  linked 1: “Alder” to 2:
“Birch”`). The reading of a text is no step of its own: it follows the
text's edit. A right-click on a change offers to retract it, asking
first; a change retracted is dim, ending `(retracted)`. A right-click on
a jump offers to go back to where it went.

### 2.3 Keys and quasimodes

The defaults are below. All of them can be changed (§ 4.3).

#### Editing

| Key | Does |
| --- | --- |
| a printable character | types into the item under the cursor |
| Enter | makes a new item after the cursor's item |
| Tab | makes the item the last child of the item before it at its level; in an open `[[`, completes the link (§ 2.4) |
| Shift-Tab | makes the item a sibling of its parent |
| Backspace, Delete | erase a character before or after the cursor; with text selected, erase it. Backspace in an empty item with nothing under it deletes the item, and the cursor goes to the end of the item above |
| Up, Down | move to the item above or below |
| Left, Right, Home, End | move within the item's text |
| Shift with an arrow, Home or End | moves as the key does, and selects from where the cursor was |

#### Selecting

Shift with an arrow, Home or End selects, as in a desktop program: from
where the cursor was to where it goes, drawn reversed. Within one item
it selects characters: Backspace or Delete erases them, and a character
typed replaces them. Up or Down with Shift selects whole items, from one
to the cursor's; Backspace and Delete then refuse, so a key that erases
a character does not delete a branch. A move without Shift, a click or
a command ends the selection.

A command typed alone takes the selection for its first part (§ 1.5):
select three items and type `delete`, or select a word and type `find`.

A Leap selects too: leap to one end, then hold a Leap key, type the
other end, and press the other Leap key. The items from where that Leap
began to the item it reached are selected. A drag of the mouse selects
the text it covers (§ 2.3, The mouse), until a click without a drag, or
a key, ends it; Backspace and Delete leave it, since they erase only text
selected with Shift in one item.

#### Moving by structure

Ctrl with an arrow, Home or End moves by the outline's structure, as
`go next` and the other ways of § 2.4, Finding and moving, do: over the
items under one, not line by line. Ctrl is the Command quasimode's key:
held with nothing typed, an arrow, Home or End moves, and you may move
again before you let go.

| Key | Does |
| --- | --- |
| Ctrl with Up, Down | move to the previous or next item at the cursor's level (`go previous`, `go next`) |
| Ctrl with Left, Right | move to the item the cursor's is under, or the first under it (`go parent`, `go child`); from the top level, to the notebook above the first item (§ 2.2) |
| Ctrl with Home, End | move to the first or last item at the cursor's level (`go first`, `go last`) |

#### Quasimodes

A quasimode is a kind of input that a key starts: searching,
commanding and supposing each have one. Use it either way:

- **Tap, type, Enter.** Tap the key (press and let go, with nothing
  typed between), type, and press Enter. The foot names the quasimode,
  `(latched)`, until Enter ends it. This suits a command line or a
  sentence, typed with both hands free.
- **Hold, type, let go.** Hold the key while you type, and let go to
  finish. This suits a few letters, such as a Leap. You cannot forget
  you are in a quasimode while you hold its key, as you can with a mode.
  Until you press another key, a release latches it, so the foot says
  `release now to type without holding`, and then what to do while you
  hold it: for Command, `or type and release Command to run on the
  target`.

The table says what each does while it is on, held or latched, and
when it ends.

| Quasimode | Default keys | While on | When it ends |
| --- | --- | --- | --- |
| Leap down | Right Alt (AltGr) | type a pattern: as each letter is typed, the cursor goes to the next item down holding it, counted from where the Leap began, and stays at the last match if a letter matches nothing | the cursor stays where it is; back returns to where the Leap began. Erasing the pattern puts the cursor back |
| Leap up | Left Alt | type a pattern: as Leap down, to the item above holding it | as Leap down |
| Command | Left Ctrl | type a command line | the command runs on the item under the cursor |
| Suppose | Super (either) | opens Command with `suppose ` typed: type a statement; the line below says what may come next in controlled English, whether it is a sentence of it, and if it breaks, where and how to mend it | the memex offers its readings of it (§ 3.2) |

Within a quasimode:

- **Tap to latch.** Press and release the key with nothing typed between,
  and the quasimode stays on without holding. Enter, or a second tap, ends
  it. Backspace on an empty latched quasimode leaves it without acting.
- **Tab completes:** a command name in Command, a pattern in Leap. In a
  sentence (a Command line that starts `suppose `, or `ace `, an old name) it
  completes the word being typed when one word alone fits, or mends: it
  puts the repair the line below offers first in place.
- **Esc cancels,** as everywhere: the quasimode ends and nothing acts; a
  Leap puts the cursor back where it began.
- **`?` lists** the commands that match (Command; once a command's name
  and a space are typed, its form and help instead), every match (Leap), or
  everything that may come next, each kind of word with a line of help
  (a sentence). In a sentence, `?` lists only at the start or after a space:
  typed straight after a word it ends a question ("Does John wait?").
  `?` again hides the list; what you typed stays. In Command, `?` inside
  quotes is typed; in Leap, `?` always lists, so Leap cannot search for it.
  Command lists the commands under the five headings of § 2.4, by name
  within each, with what each does and, at the right, its key, if one is
  bound to it (F3 for `adopt`). Where they do not all fit, it lists each
  group's names alone, with their keys; typing a command's first letters
  narrows the list to those it fits.
- **Repeat a Leap:** a second tap of a latched, empty Leap, or Enter on
  it, repeats the last pattern.
- **Leap or make:** in Leap, Enter goes to the match, as release does; if
  no item holds the pattern, Enter makes a new item holding it, after the
  cursor's item, and puts the cursor at its end. Searching for a note and
  making one are one gesture. Releasing on no match makes nothing, so a
  mistyped pattern is only reported. If an item holds something a few
  edits from the pattern (one for four to seven letters, two for more;
  none for fewer), the first Enter makes nothing and warns: `no item
  holds “Korolov”; 2 holds “Korolev” · Enter again makes “Korolov”, or
  change the pattern`. The new item is read at the pause,
  as typed text is.
- **Focus lost while a key is held** (as by Alt+Tab): the release goes to
  another window, so the memex does not wait for it. A quasimode held with
  nothing typed is left without acting. One held after you typed stays open, latched, with what you
  typed, and says so: Enter ends it, or leave it as any latched one.

Leap matches anywhere in an item's text, ignoring case, and wraps round
at either end of the outline. It unfolds a folded branch to reach a match.

#### Ctrl chords

Command held for one letter, then released, is a
chord, as in any desktop program: Ctrl+Z undoes, Ctrl+Y or
Ctrl+Shift+Z redoes, Ctrl+F opens Leap, latched. Ctrl+C copies and
Ctrl+X cuts the text selected, or else the items selected or the
cursor's, each top-most one with its branch; Ctrl+V puts text at the
text cursor, and items after the cursor's item. Items cut are drawn
dim until Ctrl+V moves them, so each keeps its identity and its
entries, as a spreadsheet's cut does; items copied are new items of
their text, as `copy` makes, and may be pasted again. Text dragged with
the mouse can be copied, not cut. Any other single
letter runs nothing and says so, so a chord typed from habit (Ctrl+N)
does not run a command (`new`) by surprise. A latched Command still
takes one letter: tap Left Ctrl, type `n`, press Enter.

#### Function keys and confirmations

F1 opens the command palette: Command, latched, with the list of
commands open. The Menu key, or Shift+F10, opens at the cursor's item
the menu a right-click on it opens (§ 2.3, Right-click), so the keyboard
reaches every offer. F10 quits, writing what is pending first. F3 and F4 adopt
a claim and take back the target's own last entry, an adoption, a fact
or a layer (`adopt`, `retract`), F6 switches between two outline panes,
and F7 and F8 go back and forward along the trail of jumps (`go back`,
`go forward`); § 4.3 lists every default.

**Confirmations.** Two commands ask before acting, at the foot of the
screen:

- `retract` lists the entries that depend on the one you retract. Enter
  retracts; Esc keeps it.
- Suppose offers readings of your statement: `Do you mean: …`. Enter
  accepts; Tab shows the next reading; Esc supposes nothing.

#### The mouse

The mouse is on unless you turn it off, with `enabled = false` in the
configuration (§ 4.3), or with `--no-mouse` for one run; `--mouse` turns
it on for one run whatever the file says. Giving both flags is an error.

What the screen draws that stands for something of yours is a
*presentation* of it, after the Symbolics Lisp machine's Dynamic Windows
(docs/design.md, Presentations): the mouse acts on the thing itself, not
on the characters. Presented are:

| What is drawn | Presents | A click |
| --- | --- | --- |
| an item's line in an outline pane | the item | moves the cursor to it, in the outline pane it is drawn in, and focus there, as the arrow keys would |
| the outline's title | the notebook | moves the cursor above the first item, where the inspector lists what waits for you to decide (§ 2.2) |
| an offer, in the notebook's inspector | what it is about: the item, the constraint or the suggestion | does what a click on that thing does |
| a step of the history, in the notebook's inspector | its entry | nothing: a right-click offers to retract it, or to go back to where a jump went |
| a link, a mention or a match, in the inspector or a list | the item at its other end | jumps to it in the left pane (§ 2.4) |
| a page, under `pages →` in the inspector | its item, or, with none, the list of what links to it | jumps to its item, or lists them (`find #draft`) |
| a finding under an item (`‼ breaks …`, `? sources disagree …`), and the heading of the constraint in the inspector | the constraint | goes to the item that states the rule, if one does |
| a mark of a view: a bar, a point, a box | the fact it draws | goes to the item that states the fact (§ 3.3) |
| a layer, in a view's inspector | the layer | goes to its view |
| a change, in a revision's pane | the change | nothing: a right-click offers what to do |
| a suggestion, anywhere in its lines | the suggestion | adopts it (`adopt suggestion N`) |
| `F3 adopts` or `F4 retracts` under a claim's questions | the command | runs `adopt` or `retract` (the hint names whichever keys you have bound) |

**Pointing.** The presentation under the pointer is underlined, all its
rows, when a click or a right-click on it would do something now; and the foot says what
each button does there (`click: go to 2 · right-click for: question its
claim; adopt its claims`), on its first row, so the message stays.

**Selecting.** Drag with the left button to select text: it is drawn
reversed, from where the button went down to where it is, in reading
order, within the pane where the drag began. A click without a drag
clears it.

#### Right-click: what can act on this?

A right-click on a presentation
offers the commands that apply to what it presents, as it is now:

| Right-click on | Offers |
| --- | --- |
| an item (the cursor goes to it first) | question its claim and adopt its claims, if it states any; take back its adoption, if it has one; split it, if it has two sentences or more; hide or show its branch, if it has one |
| a finding under an item (the cursor goes to that item first) | what the item offers, as above, since the finding is about it; then go to the item that states the rule; retract the constraint; then your verdict on the finding: a real error, the rule is wrong, the text was misread, or known already (`verdict`, § 2.4) |
| a mark of a view | go to the item that states its fact |
| a layer | retract it; go to its view |
| a revision's change | reject it (`dismiss change N`, which asks first); keep the rest (`adopt world`) |
| a suggestion | adopt it; dismiss it |

After them come the commands plumbing offers (below) for the word under
the pointer (without the punctuation around it), if it is in an item or
in text that presents nothing, or for the text selected, whatever is
under it. Last, on an item's text or a link, come the verbs that apply
to that item, the cursor on it: link it, move it, copy it, a new item
after it, show it as a chart (if it is no view yet), show another item's
text after it, make what its `[[links]]` name (if one names no item),
check its quotation or its source (if it has one), suppose what it
states (if it is read), find what carries each of its labels, retract
its last entry (if it has one, and not already offered), and delete it.
Each runs as the verb typed alone does (§ 2.4, A verb alone): its words
name the default it takes, or end in `…` where it asks. A verb with a
key of habit names it, as `copy it after itself  (Ctrl+C, Ctrl+V)`.
Over an item, the foot's `right-click for:` ends with `the verbs that
apply to it`. The offers appear in a
menu titled with what it acts on, such as `act on “flight”` or the
item's text, just below the pointer (above it near the foot), so the
row clicked stays in view. The Menu key, or Shift+F10, opens the same
menu for the cursor's item, below it. An offer that a key runs names the key, as
`adopt its claims  (F3)`. Up and Down choose, Enter or a number runs
one, Esc closes it; a click on an offer runs it, and a click anywhere
else closes the menu. If nothing can act on it, the foot says so.

#### Pointing at an argument

While Command waits for an argument (its
line ends in a space), a click on a presentation of the kind the
argument takes types it in for you, with a space after, and only those
are underlined: an item, by its address, for `link`, `copy`, `move`,
`transclude` and `go` (a link, a fact, a rule or a view stands for its
item); an entry, by its number, for `retract` (a layer, a change, a
constraint, a fact or a step of the history); a suggestion, by its number, for `adopt` and
`dismiss`. Tap Left Ctrl, type `link to `, and click the item to link
to: the line reads `link (from 1) (to) 2`, the from being the target,
which you did not type, and Enter runs it. The foot says what
a click would type (`click: type “2” into the line`).

#### Plumbing

What a right-click offers for text is plumbing, after Plan 9's plumber (Rob Pike, "Plumbing and Other
Utilities"). Patterns say what can act on a text: each has a regular
expression that the whole text must match, and a command line to offer.
Plan 9 runs the first that matches; here every pattern that matches is
offered, in order, each command once. The built-in patterns, most
specific first:

| The text | Offered |
| --- | --- |
| an address that names an item other than the cursor's: `1.3`, `draft` | `go` to it |
| `entry` and a number, or `#` and a number | `retract entry` it |
| the path of a file that exists, ending in `.md`, `.docx`, `.odt`, `.tex`, `.html` (and others Pandoc reads), `.json` or `.pl` | `import document`, `import results` or `import wikidata` |
| a Prolog goal, such as `date(E, Y)` | `find` its answers (`` find `date(E, Y)` ``), state it here as a fact, or state it and set aside what it contradicts |
| a Prolog fact, or two or more words without brackets | `suppose` it, as the Suppose key does |
| two or more words, without brackets | read them as ACE (`ace`, an old command), which says why if it cannot |
| text that names an item other than the cursor's | `link to` it from here, or `transclude` it: show its text here, in place |
| any text | `find` the items that hold it |

The other commands act on the item, not on text (`adopt`, `delete`,
`show` and the rest), so plumbing does not offer them: the menu offers
them after plumbing's, as the verbs that apply to the item (§ 2.3,
Right-click).

Add your own with `[[plumb]]` in the configuration (§ 4.3); yours are
offered before the built-in ones. Each has a `name` (the menu's words), a
`match` (the regular expression, in Rust's syntax), a `run` (a command
line, as typed in Command), and, if it needs one, an `if`: `"file"` (the
text is a file that exists, from where the memex started) or `"item"`
(the text names an item other than the cursor's, since going to it,
linking it to itself or showing it in its own place does nothing). In
`name` and `run`, `$0` is the whole text and
`$1` to `$9` (or `${1}`) the expression's groups:

```toml
[[plumb]]
name = "show Wikidata's statements about $0"
match = 'Q\d+'
run = "query wd('$0', P, V)"
```

A pattern whose `match` is not a regular expression, a command that
does not exist, or an unknown setting is refused when the memex starts,
as any configuration it would have to guess at is.

A click does nothing while a quasimode or a confirmation is open, and
nothing on a finding's line, a heading or empty space. The middle button and
the wheel do nothing. A click is recorded in the journal as the keyboard
action it stands for; a selection and an open menu are not recorded, but
the command an offer runs is.

### 2.4 Commands and addresses

Type a command in the Command quasimode, or bind it to a key (§ 4.3). A
unique prefix is enough: `supp` runs `suppose`. After a verb and a
space, `?` lists the words its first part may be, each with what it
means (`world — the supposed world, as the actual one` after `adopt`),
and Tab completes one that alone starts as you typed.

**One reader for every line.** A line of two words or more whose first
word names no command is read by its form. A question, ending in `?`,
is answered once, as `find` answers it. A statement is supposed, its
readings offered (`Do you mean: …`), as the Suppose key does. A line
with no reading is refused and kept open, so that Tab can mend it and a
mistyped command makes no world; to make a world named by a text with
no reading, type `suppose` before it. A command's whole name, or a start
of two letters or more, always wins, so a sentence that starts "A" is no
`adopt`. The foot says how the line will be read before you end it. Words in parentheses
in the help, such as `(after)`, are guide words, which say what each part
is. Only `to`, `from` and `after`, before an address, may be typed or
left out (`move after 2` or `move 2`), and `link` needs `as` before a
type. Any other guide word you type is read as part of what follows it,
so leave those out. Unless the table
says otherwise, a command acts on the **target**, the item under the
cursor.

#### A verb alone

| Verb alone | Takes | With none now |
| --- | --- | --- |
| `move` | (after) the item you last jumped from | asks for it |
| `copy` | (after) the target itself | |
| `link` | (to) the item you last jumped from, (as) `link` | asks for the item |
| `go` | `back` | asks where |
| `show` | (as) `chart` | |
| `suppose` | (that) the target's text, if the memex reads it as statements | asks for a statement |
| `check` | (which) `quote` or `source`, whichever the target has | asks, if it has both or neither |
| `retract` | (entry) the target's last entry that can be retracted | asks for an entry |
| `import`, `export` | | asks for the file |
| `transclude` | | asks for the item whose text to show |

The item you last jumped from is never the target itself. Other verbs
alone do what their rows below say.

Where a command takes an ADDRESS, any of these name an item:

- its number: `2`, or `1.4` for the fourth item under item 1;
- `#` and the number of the entry that made it, `#28`, an old form, kept;
- a name given by `name`, an old command (§ 2.4, Old names);
- the name of a page, with its `#` or without: `draft` or `#draft`
  (§ 2.4, Labels): the page's item, if it has one, else the one item
  that links to the page, by `#draft` or by `[[draft]]`. A page two or
  more items link to addresses none of them; `go draft` lists them. The
  item labelled `#title` heads the outline;
- the start of its text, if only one item starts that way: `Energia`.

#### Editing the outline

| Command | Form | Does |
| --- | --- | --- |
| `new` | `new` | makes a new item after the target |
| `delete` | `delete`, `delete word` | deletes the target and its branch, and with them what they state: their facts and claims, the beliefs adopted from their claims, their layers and derived values, and the links to and from them; `undo` puts it all back. `delete word` deletes only the word at the text cursor |
| `move` | `move after ADDRESS` | moves the target's branch to follow ADDRESS |
| `copy` | `copy after ADDRESS` | copies the target's text to a new item after ADDRESS |
| `link` | `link [FROM] to TO [as TYPE]` | links two items; FROM defaults to the target, TYPE to `link`. `cites`, `supports` and `opposes` read well in the inspector |
| `transclude` | `transclude from ADDRESS` | makes a new item after the target that shows ADDRESS's text in place |
| `fold` | `fold` | hides the target's branch, or shows it if it is hidden; a right-click on an item with a branch offers it |
| `switch` | `switch` (F6) | moves focus to the other pane if it is an outline (`pane outline` puts a second outline there); the inspector and a list take no keys, so `switch` there says so (`only an outline takes keys; the other pane shows links`) |

#### Finding and moving

| Command | Form | Does |
| --- | --- | --- |
| `find` | `find TEXT`, `find #LABEL`, `find violations`, `find disagreements`, `find suggestions`, `find QUESTION?`, `` find `GOAL` ``, `find` | lists in the inspector's place the items that hold TEXT, ignoring case (as Leap matches), that link to the label's page (by `#draft` or `[[draft]]`, § 2.4, Labels), or that a violation or a disagreement marks, in the outline's order, numbered, with where the cursor is among them (`LINKED TO #draft · 2 · at 1 of 2`); a click on one jumps to it, so `go back` returns, and the list stays as you edit. `find suggestions` lists the suggestions. A question, in English or as a Prolog goal between backticks, is answered once over this world's beliefs, through rules: `` find `animal(X)` `` says `2 answers: X = nemo · X = dory`, a goal without variables `yes` or `no`, and an English question its answer alone (`1 answer: Answer = 1600`). At most 20 answers are shown; the goal may only read facts, compare and compute, as a definition may; nothing is written. `find` alone shows the inspector again |
| `go` | `go TEXT`, `go link N`, `go back`, `go forward`, `go next`, `go previous`, `go first`, `go last`, `go parent`, `go child` | goes to the item TEXT addresses; else, if two or more items link to the page TEXT names, lists them, as `find #TEXT` does; else goes to the next item that holds TEXT, ignoring case, as Leap does; a right-click offers it (§ 2.3). `go link N` goes to the other end of the target's Nth link (default 1). `go back` (F7, or the mouse's back button) goes back to where the last jump came from: `go`, Leap, or a click on a link, a mention or a match; `go forward` (F8) goes forward again, and a new jump empties the way forward, as in a browser. Each jump's view entry records where it came from, so a reopened notebook keeps the trail; an item since deleted is passed over. `go next`, `go previous`, `go first`, `go last`, `go parent` and `go child` move by the outline's structure, after NLS (Ctrl with an arrow, Home or End, § 2.3): `go next` passes over the items under the target, `go parent` from the top level goes to the notebook, whose inspector lists what waits for you to decide (§ 2.2), and `go child` shows a folded branch. These are moves, as arrow keys are, not jumps. The words of the ways are kept for them: an item whose text is `next` is reached by its number |

#### Labels

A `#` at the start of a text or after a space, then a word,
tags the item: `Buran #reusable #Soviet-space` gives `item_tag(Item,
reusable)` and `item_tag(Item, soviet_space)` (lower case, hyphens made
underscores, as the reader writes names). Tags follow the text: there is
nothing to write or take back.

A label is a link, to the page of its name: `#draft` and `[[draft]]`
link to one page, `draft`, and so do `[[Draft]]` and, for a name of two
words, `#buran_flight`, `[[Buran flight]]` and `[[buran-flight]]` (case
aside, a space and a hyphen are an underscore). A page is the item named
so, or else the first whose text is its name; with neither, the page is
virtual, made of the items that link to it. `find #draft` lists them, by
either spelling (`LINKED TO #draft · 3`). An item's inspector lists its
pages under `pages →`, and a page's item lists the items that label it
under `labelled by ←`; that one is looked for only when the item's text
reads as a name, as for mentions (§ 3.4). Each label also gives
`item_link(Item, Page, label)`, and a `[[link]]` to a virtual page
`item_link(Item, Page, link)`, with the page by its item's number, else
by its name: `item_link(4, draft, label)`. Views, derived text and constraints read
them, as `constraint not both draft and final: false :- item_tag(I,
draft), item_tag(I, final).` does. `C#`, `#1`, a heading's `# ` and a web
address's `/#anchor` are not tags.

#### Links typed in text

Write `[[Buran flight]]` in an item's text and,
at the pause, the item links to the page's item: the item named so, else
the first whose text it is (ignoring case, with a space and a hyphen
alike). While the text cursor is after an open
`[[`, a dim line under the item lists the names that fit what you have
typed (`[[ Energia · Energy launch · Tab completes`): the names that
`name` gave, and the texts short enough to be names (3 to 60 characters, six
words at most, not ending in `.`, `?` or `!`). Tab completes as far as
they agree, and, when one fits, closes the link with `]]`. A link that names no item makes
nothing: it links to a page of its own (§ 2.4, Labels). While the text
cursor is in it, a dim line says `not yet items: Buran flight · adopt fix
makes them`, so that a typo is seen as it is typed, and `adopt fix` (or `make`, its old name) makes
each at the foot of the top level and links it; one `undo` takes that back. If you make such an item yourself
(by typing it, or with Leap and Enter), the link is made at the pause, and one `undo` takes the text and its
links back together. A name given by `name` links at once; a name moved
to another item takes its links with it. Take the brackets out, and the link goes. The reader reads
the text without its brackets, so `The date of [[Buran-flight]] is
1988.` is still a fact.

#### Views and checks

| Command | Form | Does |
| --- | --- | --- |
| `show` | `show as chart`, `show as table`, `show as graph`, `show ADDRESS as chart`, `show MARK where GOAL` | makes the target, or the item ADDRESS names, a view of the world's facts (§ 3.3): a bar for each box (`box` fact), a table of them, or the graph of the items and their links. With a mark, it adds that layer: one mark for each solution of GOAL |
| `check` | `check quote`, `check source` | checks the target's text against the document it came from, or its `doi` fact against Crossref |
| `verdict` | `verdict error`, `verdict rule`, `verdict misread`, `verdict known`, each with `entry N` or without | records what a finding marked under the target is worth: a real error; the rule is wrong; the text was misread; known already. `entry N` names the rule's entry; without it, the one rule with a finding under the target. It changes nothing shown; the journal keeps it, so the measures can tell findings that caught errors from those that did not. A right-click on a finding offers each |

#### Claims and suggestions

| Command | Form | Does |
| --- | --- | --- |
| `adopt` | `adopt` (F3), `adopt instead`, `adopt suggestion N`, `adopt fix`, `adopt world` | `adopt` makes the target's claims beliefs, keeping their source, one adoption for each claim; `adopt instead` also retracts the beliefs they directly contradict (each that breaks a constraint with them and nothing else); `adopt suggestion N`, the Nth suggestion shown, with its evidence; `adopt fix`, the fix of a finding: the items the target's `[[links]]` name, made at the foot of the top level and linked, one `undo` taking it all back; or, if it has none to make and its text, a sentence, was not read, its text mended as APE would mend it (`not read: did you mean “…”?` under it); `adopt world`, the supposed world as the actual one |
| `dismiss` | `dismiss suggestion N`, `dismiss world`, `dismiss change N` | `dismiss suggestion N` sets the Nth suggestion aside; it returns only if its evidence changes. `dismiss world` closes the supposition and returns to the actual world; it stays in the journal. `dismiss change N` rejects one change of a revision (§ 4.1), asking first |
| `import` | `import FILE [as ADDRESS]`; or the kind first: `import wikidata FILE`, `import document FILE [as ADDRESS]`, `import results FILE` | imports a file of the kind its name ends in: a Wikidata subset (`.pl`), an analysis's results (`.json`), or else a document (with `as`, as a revision of the document the item at ADDRESS came from) (§ 3.4, § 4.1). Alone, it asks for the file |
| `export` | `export FILE [NUMBER]` | exports the outline, or the section at item NUMBER, as a document |

#### Worlds and history

| Command | Form | Does |
| --- | --- | --- |
| `suppose` | `suppose [that] STATEMENT` | supposes STATEMENT as the Suppose key does (§ 3.2): asks which of its readings against the facts you mean, or, with none, starts a world of that name with nothing supposed |
| `undo` | `undo` (Ctrl+Z) | undoes the last change on this branch |
| `redo` | `redo` (Ctrl+Y) | redoes what undo undid, along the current branch |
| `retract` | `retract [entry N]` (F4) | reverses one earlier entry, by its number (`retract entry 12`; `retract 12` and `retract #12`, the old forms, still work), keeping later work; lists what depends on it first, and Enter retracts. With no number, the target's own last entry that can be retracted: an adoption of its claims, a fact given it by command (not one its text was read as, which its text keeps), or a layer of its view. An item with two claims has two adoptions |

**Undo and retract.** `undo` and `redo` move along the current branch of
the journal, as in any editor; undoing and then editing starts a new
branch, and the old one stays in the journal. To reverse one earlier
decision without losing what came after, `retract` it instead. An
entry's number is shown in the inspector, offered by a right-click, and
in the journal. `adopt instead` writes each retraction and the adoption
as separate entries, so one `undo` reverses only the last of them.

#### Old names

Each of these still runs, typed whole (a prefix finds only the commands
above), so habits, journals and scripts keep working. None is listed by
`?`.

| Old name | Form | Now |
| --- | --- | --- |
| `fact` | `fact FACT` | write the fact in an item, as a sentence or in Prolog between backticks. `fact` states it on the target |
| `claim` | `claim FACT from SOURCE` | write `SOURCE says that …` in an item, or `` `claims("SOURCE", FACT).` ``. `claim` states it on the target |
| `rule` | `rule HEAD :- BODY.` | write `If … then …` in an item, or the rule between backticks. A rule defines a predicate from others, usable wherever a goal is; it is refused if recursive, if a head variable is not bound by the body, or if the body calls what a definition may not |
| `constraint` | `constraint NAME: false :- BODY.` | write the sentence in an item (`Nothing has more than 1 date.`), or `` `false :- BODY.` `` |
| `derive` | `derive TEMPLATE where GOAL` | write the sentence with its blanks and, between backticks, the goal that fills them, in the item (§ 3.1) |
| `query` | `query GOAL` | `` find `GOAL` `` |
| `ace` | `ace SENTENCE` | shows what APE, the parser of controlled English, makes of SENTENCE: its paraphrase, or where it fails and APE's advice. Each entity of the current world is a word for it, as are the names of one-place facts (adjectives) and two-place facts (nouns) and labels of one word (§ 3.2). Typing in an item shows the same, at the cursor |
| `set` | `set FACT` | edit the sentence that states the fact: the old value is taken back as the new one is read; or, at a disputed claim, `adopt instead`. `set` states FACT and retracts the beliefs it directly contradicts; the fact goes on the item that held the one it replaces, else on the target. A violation it leaves is named, not resolved |
| `accept` | `accept N` | `adopt suggestion N` |
| `interrogate` | `interrogate` | the inspector asks a claim's questions at any item that states one (§ 2.2). `interrogate` holds the target in the inspector while the cursor moves elsewhere, until a supposition is left |
| `commit` | `commit` | `adopt world` |
| `close` | `close` | `dismiss world` |
| `pane` | `pane outline`, `pane links`, `pane suggestions`, `pane matches` | `find` and `find suggestions` for the lists, `find` alone for the inspector; `pane outline` puts a second outline beside the first |
| `name` | `name WORD` | a label the item alone carries (§ 2.4, addresses). `name` still names the target, so WORD addresses it, before any label; the name `title` heads the outline, as the label `#title` does |
| `make` | `make` | `adopt fix` |
| `split` | `split` | a right-click on an item of several sentences offers it. It splits the target's text at each sentence end (`.`, `?` or `!`, a space, then a capital, a digit, a quote or a bracket; never after an abbreviation that comes before what it shortens, such as `e.g.`, `Dr.` or `p.`, or an initial such as `J.`; `etc.`, `et al.`, `Jr.`, `Sr.`, `ed.`, `eds.` and `St.` end one only before a capital letter): the target keeps the first sentence, with its links; each other becomes a new item after it, linked back as `split_from`, its characters kept from the target's text, so their history stays. Each part is read at the pause. One `undo` takes the whole split back |
| `layer` | `layer MARK where GOAL` | `show MARK where GOAL` |
| `indent` | `indent` | Tab: makes the target the last child of the item before it at its own level |
| `outdent` | `outdent` | Shift+Tab: makes the target a sibling of its parent |
| `help` | `help` | F1, or `?` in Command: the list of every command |
| `quit` | `quit` | F10, or closing the window: writes what is pending and leaves |
| `follow` | `follow [N]` | `go link N` |
| `back` | `back` | `go back` (F7) |
| `forward` | `forward` | `go forward` (F8) |
| `next` | `next` | `go next` (Ctrl+Down) |
| `previous` | `previous` | `go previous` (Ctrl+Up) |
| `first` | `first` | `go first` (Ctrl+Home) |
| `last` | `last` | `go last` (Ctrl+End) |
| `parent` | `parent` | `go parent` (Ctrl+Left) |
| `child` | `child` | `go child` (Ctrl+Right) |

## 3. Reasoning

### 3.1 Facts, claims, constraints and derived text

**Write in controlled English, and the memex reads it.** When you pause
(a second without keys), each item you wrote whose text is ACE is read as
the statements it makes, and its text turns cyan. Nothing asks you to
confirm, and nothing is written twice:

```
The date of Buran-flight is 1988.
Talk-notes says that the date of Buran-flight is 1989.
The income is 2000.
The rent is 1200.
If the income is X and the rent is Y then the rest is X - Y.
It is false that the rest is smaller than 0.
```

These are a fact, a claim from a source, two boxes, a rule and a
constraint. A box is a value with a name, such as the income;
`Nothing has more than 1 value.` says that each box holds one, and
`Nothing has more than 1 date.` the same of dates. With an item selected, the inspector shows, under `READ AS`,
each statement its text was read as, in Prolog, as the memex runs it
(`fact  date(buran_flight, 1988)`, `rule  rest(A):-box(income, B), …`),
and its tags (`tag  item_tag(12, money)`). A text read before the memex
kept these forms shows none until its text changes. Change the text, and
what it said is taken back and the new text read; undo takes back a
sentence and what it said together. Text that is not ACE stays prose:
at the cursor, if it reads as a sentence (it ends in `.`, `?` or `!`),
a dim line says where APE stopped (`prose: not ACE, it
stops at …`). A text is read whole or not at all: ACE the notebook cannot
state says `not read:` and why.

Ask in English, too: `What is an animal?`, `Who swims?`, `What is the
income?`, `Is Nemo an animal?`. After the pause, the answer is drawn
under the question (`→ dory, nemo`, `→ yes`, or `→ none`) and changes
as the facts do. `Which things are animals?` works too: ACE has no
plural after "are", so the memex asks it as `Which things are an
animal?`, which means the same; it does so for questions only. A thing
with a label is answered by it: with `The label of Nemo is "Nemo the
clownfish".`, the answer is `→ Nemo the clownfish, dory`. A thing, as in
"which things", is anything at all. "A fish is an animal." is ACE for
one fish with no name; write `Every fish is an animal.` for all fish, or
name one: `Nemo is a fish.` The table in docs/design.md (§ Reading
controlled English) lists what each kind of sentence is read as.

What controlled English cannot say, write in Prolog between backticks,
in an item's text; the prose around it is read as any prose is. Each
item below is one line of your outline:

```
The flight's date: `date(buran_flight, 1988).`
`label(buran_flight, "Buran's flight").`
`claims("my notes, conference talk", date(buran_flight, 1989)).`
```

A fact is a Prolog term: an atom or a compound, with no variables.
Strings go in double quotes. A trailing full stop is optional.
`claims(Source, Fact)` is what Source says. A `label(Entity, "words")`
fact lets the memex write facts about Entity in words: in the
inspector, Suppose's readings, timelines and answers to questions.
Findings, and the marks of views, show terms as they are.

**Constraints** are denials, `false :- …`. The prose before them is
their name:

```
One date per event: `false :- date(E, D1), date(E, D2), D1 \= D2.`
```

Before keeping it, the memex plants facts that should break it and checks
that it fires. A constraint that cannot fire is refused, with the reason;
so is one with more than seven variables, which the check cannot try.
Constraints are recomputed whenever the journal moves; only the parts that
depend on what changed are recomputed.

**Derived text.** An item can show text computed from the facts: write
the text with its blanks, and between backticks the goal that fills
them. In the text, `{Var}` is a variable of the goal, and `{N:years}`
writes a number of years in words ("a year", "two years"). The text is
filled from the goal's first solution only:

```
The flight came {N:years} after Energia's first launch. `date(buran_flight, B), date(energia_launch, E), N is B - E`
```

`derive timeline where date(E, Y)`, a form of the old command `derive`
that a text with blanks cannot write, draws a timeline of the solutions, using
labels where there are any: one line, such as `1987 Energia's first launch
── 1988 Buran's flight`. Each solution must give exactly two values, an
entity and then a whole-number year, in the order the goal first names
them; any other is left out, so `date(E, Y), label(E, L)` draws nothing.

A goal with no solution shows the template and `(no answer)`. A goal that
raises an error shows the template and what went wrong, such as `(cannot
answer: type_error(evaluable, foo / 0))`.

**Rules.** A computation used in several places can be written once, as
a rule, and then used like a fact:

```
`spent(S) :- box(rent, R), box(food, F), S is R + F.`
Spent: {S} `spent(S)`
Within income: `false :- spent(S), box(income, I), S > I.`
```

A rule may not be recursive, directly or through other rules (the
reasoner proves rules by plain recursion, which would not end); every
variable in its head must be bound by its body; and its body may call
only what a definition may (below). A violation found through a rule
rests on the facts under the rule, so its findings appear on the items
you would change.

**What a definition may call.** A constraint, a derived goal and an
adopted rule may only read facts, compare and compute. Besides your facts,
they may use the list below. A fact whose name and number of arguments
are those of a predicate of the reasoner or of a Prolog library, such as
`member/2`, `reverse/2`, `node/2` or `count/1`, can be stated but not
used in a definition: the definition is refused, naming it.

- comparison and unification: `=`, `\=`, `==`, `\==`, `@<`, `@>`, `@=<`,
  `@>=`, `compare/3`, `=@=`, `\=@=`;
- arithmetic: `is`, `=:=`, `=\=`, `<`, `>`, `=<`, `>=`, `succ/2`, `plus/3`,
  `between/3`;
- type tests: `var`, `nonvar`, `atom`, `number`, `integer`, `float`,
  `string`, `atomic`, `compound`, `callable`, `is_list`, `ground`;
- terms: `functor/3`, `arg/3`, `=..`, `copy_term/2`;
- atoms and strings: `atom_length/2`, `atom_concat/3`, `sub_atom/5`,
  `atom_chars/2`, `atom_codes/2`, `char_code/2`, `atom_number/2`,
  `atom_string/2`, `number_codes/2`, `number_string/2`, `upcase_atom/2`,
  `downcase_atom/2`, `atomic_list_concat/2,3`, `string_concat/3`,
  `string_length/2`, `sub_string/5`, `string_chars/2`, `string_codes/2`,
  `string_code/3`, `string_lower/2`, `string_upper/2`, `split_string/4`;
- lists: `length/2`, `msort/2`, `sort/2,4`, `keysort/2`, `memberchk/2`;
- `true`, `fail`, `false`, and `\+`, `;`, `->`, `*->`, `findall/3,4`,
  `forall/2` around goals that are themselves allowed.

Anything else, such as `shell/1`, `open/3`, `assertz/1`, `format/2` or
`call/1`, is refused when the constraint or derived text is defined, with
what is not allowed. The reasoner checks again each time it calls one, so
a definition that arrives some other way cannot act either. A statement,
and `adopt`, refuse `node_def/2` and `rule_def/3`, the forms definitions
are stored in.

### 3.2 Worlds

When you do not yet know which source is right, or what a change would
do, suppose it: the notebook shows what follows beside what you believe,
and changes nothing you believe.

Tap Suppose (or hold it): it opens Command with `suppose ` typed, so
`suppose` is the command and the key saves typing it. Type a statement
in your own words, such as
`the flight was in 1989`, and press Enter (or let go). The memex reads it against the facts it knows and offers each
reading: `Do you mean: Suppose that the date of Buran's flight is 1989.`
Enter accepts; Tab shows the next reading; Esc cancels. A reading is a
fact of the world with one whole number changed to a number you typed,
ranked by how many of your words name its other parts; so a statement
with no number, or about something with no fact yet, has no reading
unless you type it as Prolog.

While you type, the line below checks the statement as controlled English
(ACE), with the words of your notebook, as if it ended with a full stop.
It does the same for any Command line that starts `ace `, an old
command:

- `ACE:` and how APE, its parser, reads it back, when it is a sentence;
- `next:` and what may come next, when it is not yet one but can go on:
  your notebook's words first, then kinds of word (`a noun`, `a name`,
  `a verb`, ...), then words such as `the`, `of` and `is`. While you type
  a word, only the words it begins are shown (with the kinds of word), and
  when one alone fits, Tab completes it (`Tab: buran_flight`);
- `not ACE:` when it cannot go on: the sentence with `<>` where parsing
  stopped, and up to three edits of one word that make it ACE or let it
  go on, your notebook's words first, such as `Tab: was → is` (ACE has
  only the present tense). Tab puts the first in place. With no such
  edit, it names the words ACE does not know.

When you mend free English into ACE and suppose it, the memex keeps the
pair: what you first typed, and the ACE. Later, when you type something that
matches an earlier free English word for word but for one or two words,
fewer than half of them (and the earlier ACE holds those words), the line
below offers the earlier ACE with your new words in place, `as before: … ·
Tab`, and Tab puts it in place. The pairs are in the journal, with the
suppositions.

A word is judged only once it ends, with a space or an end mark. To see
everything that may come next, not only the first twelve, type `?` (at
the start, or after a space): the notebook's words, each kind of word
with what it is (`a name — one thing by its name: John, buran_flight`),
and the words of ACE. A fact written as
Prolog, such as `box(fuel, 80)`, is read exactly as you wrote it, even
for something the memex has no facts about. The command `suppose
STATEMENT` does the same as the key. If there is no reading, ending
the quasimode starts a named world with nothing supposed; if what you typed
is believed already, it starts one with nothing changed, and says so.

In the supposed world, each belief the supposition directly contradicts
is set aside, in that world only: the supposition wins. A belief is
directly contradicted when it and the supposition, and nothing else, break
a constraint (two dates for one event; two values for one box). A
violation that rests on the supposition and two or more other beliefs is
left in place, and the message names it ("left for you: 1 violation of
…"): any of those beliefs could be given up, and which one is your
choice. The two worlds are shown row for row: in the supposed world,
first what it supposes, each fact as the notebook reads it ("supposed:
date(buran_flight, 1989)"), beside blank rows; then the whole outline,
set in by level, each item with its address, then each constraint and
each view. In the supposed world a row
whose facts are all set aside is struck through and says so, with its
place ("(2.2 set aside here)"); a row that states more keeps its text and
names the facts set aside ("(1.1 set aside here: date(buran_flight,
1988))"); a row whose text differs is bold and underlined, with a dashed
trail on the actual side. Each constraint gives both counts in both
worlds, "0 violations, 1 disagreement", a count above zero in its
colour, and in the supposed world a count that differs is bold and
underlined; under it, what it set aside.

`dismiss world` returns to the actual world and keeps the supposition in
the journal. `adopt world` makes the supposed world the actual one. While a
supposition is open, the foot says so, above any message.

### 3.3 Views

A view draws your facts, and changes as they do; it draws only beliefs,
never a claim you have not adopted, and shows in red what breaks a
constraint.

A view is an item that draws something: a chart, a timeline, a grid, a
flowchart, a tree. `show as chart` makes the item at the cursor a bar
chart of the boxes (`The rent is 1200.` states `box(rent,
1200)`); `show as table` a table of them; `show as graph` the graph of
your items and their links. Each writes the layers of a recipe below.

Behind `show` is the grammar views are made in. A view is made of
layers, and each layer is a mark drawn once for each solution of a goal:

```
show bar(x(C), y(V)) where box(C, V)
```

This draws one bar for each `box(C, V)` fact, at category C, V high:
it is what `show as chart` writes. Run `show` with the cursor on the
item that is to be the view; each `show` with a mark adds a layer
(`layer`, its old name, does the same). A layer without `where` is drawn once, as a
reference line: `show refline(y(1500))` (`rule`, its old name, still draws one).

The view is drawn under its item and changes as the facts do. With the
cursor on it, the inspector lists its layers, with how many marks each
draws. A mark that rests on a fact in a violation is drawn red; a bar
keeps its plain fill, and its value above it is red, since each number
may be right where only their sum breaks a rule (a grid box gains `!`).
A chart keeps room for its bars' values: a row above the tallest
upright bar, and columns after the longest bar that lies across. A
chart whose plotting area is under six rows high keeps no such row, since giving one up would draw bars of different values the
same height; there the tallest bar's value is not drawn. With the mouse on (§ 2.3), a click on a mark goes to the
item that states its fact. In a supposition, each world draws its own
copy of every view, side by side. `retract` removes a layer, by its entry
number, shown in the inspector. A layer whose goal raises an error is
noted under the view, with its entry number and what went wrong, such as
`(layer 10 cannot answer: type_error(evaluable, foo / 0))`.

#### Marks

Each mark is written with its channels, `channel(Value)`:

| Mark | Channels: required; optional | Draws |
| --- | --- | --- |
| `dot` | x, y; text | a point, labelled |
| `bar` | x, y; text | a bar from zero: upright if x is a category, across if y is. An upright bar of a value not below zero is drawn to the eighth of a cell (`▁` to `▇` at its top), so its height is in proportion; zero draws none |
| `line` | x, y; series | points joined in order of x, one line and one symbol per series |
| `refline` | x or y | a reference line across the plot at that value (`rule` until 22 November 2026, still read) |
| `text` | x, y, text | a label at a point |
| `tick` | x; y, text | a mark on the axis with its label above: a timeline. With y, on that lane |
| `span` | x1, x2; y, text | an interval from x1 to x2, on lane y: a Gantt chart |
| `cell` | x, y; text | a cell in a grid: a table, a spreadsheet |
| `node` | id; text, shape | a box in a graph; shape `box`, `round` or `decision` |
| `edge` | from, to; text | an arrow from one node to another, by id |

#### Scales and frames

Scales follow from the values. Numbers and dates (`date(Y, M, D)`)
are positions on a line; anything else is a category, in standard order
(numbers first, then by text), the first at the top or left. All the
layers of a view share their scales, so a rule and the bars line up. The
marks choose how the view is drawn: any `node` or `edge` makes a graph,
laid out in layers from top to bottom (edges that close a loop go back
up round the right side); `cell` marks alone make a grid; anything else
makes a plot. Marks that do not fit are named under the view.

#### Recipes

Each is ordinary facts and rules, with layers over them. A line in
backticks is an item of your outline; a line that starts `show` is a
command, run with the cursor on the item that is to be the view.

A bar chart with a limit:

```
show bar(x(C), y(V)) where box(C, V)
show refline(y(I)) where box(income, I)
```

A spreadsheet: boxes at places in a grid, one computed by a rule.

```
`at(income, b, 1).`
`at(rent, b, 2).`
`at(left, b, 4).`
`value(B, V) :- box(B, V).`
`value(left, L) :- box(income, I), box(rent, R), L is I - R.`
show cell(x(a), y(R), text(B)) where at(B, _, R)
show cell(x(C), y(R), text(V)) where at(B, C, R), value(B, V)
```

A line chart, one line per series: `show line(x(M), y(V), series(S))
where spend(S, M, V)`.

A timeline: `show tick(x(Y), text(L)) where date(E, Y), label(E, L)`.

A Gantt chart: `show span(y(T), x1(A), x2(B), text(T)) where task(T, A, B)`.

A flowchart:

```
show node(id(S), text(T)) where step(S, T)
show node(id(S), text(T), shape(decision)) where decision(S, T)
show edge(from(A), to(B), text(L)) where next(A, B, L)
```

Your own outline as a tree. The outline is offered to goals as four kinds
of fact, `item_text(Item, Text)`, `item_under(Item, Parent)`,
`item_tag(Item, Tag)` for each `#tag` in an item's text, and
`item_link(From, To, Type)`, one for each link, of any type, typed in text
or made by `link`:

```
show node(id(I), text(T)) where item_text(I, T)
show edge(from(P), to(C)) where item_under(C, P)
```

Links between items work the same way. `link 2 to 5 as supports` states
`supports(From, To)` between the two items, so with the node layer above,
`show edge(from(A), to(B), text(supports)) where supports(A, B)` draws
an argument map. Every link at once, the graph of your notes:

```
show node(id(I), text(T)) where item_text(I, T), (item_link(I, _, _) ; item_link(_, I, _))
show edge(from(F), to(T)) where item_link(F, T, _)
```

#### Refused

A layer is refused, with the reason, if its mark or a
channel is unknown, a channel is missing or given twice, a variable in
the mark is not bound by the goal, or the goal calls what a definition
may not (§ 3.1).

### 3.4 Suggestions and Wikidata

Wikidata, a public knowledge base, is a second source to check your
facts against: its dates and statements come in as claims, and your
constraints compare them with yours. The memex also suggests links,
rules and references from what you have written, each with why.

The memex can read a subset of Wikidata, a public knowledge base, as
claims whose source is Wikidata. Make a subset with the `wikidata` tool
(§ 4.2), then:

```
import wikidata subset.pl
```

The file is copied into the journal's `blobs/` directory under its hash,
with its provenance (derived from Wikidata, under CC0). It must hold only
the facts the tool writes, with no variables: `wd(Item, Property, Value)`,
where Value is an atom, a number, a string or `date(Y, M, D)`;
`wd_label(Entity, "label")`; and `wd_constraint(Property, Kind)`. Any
other term is refused, named, and nothing is imported.

To tie your facts to Wikidata, two kinds of belief are needed:
`same_as(Entity, Q)` says your entity is Wikidata's item Q, and
`wd_property(Predicate, P)` says your predicate is Wikidata's property P.
Then Wikidata's statements become claims about your entities, and
constraints check them like any other. A world reads one subset: the last
import of one that is not retracted replaces those before it.

The memex suggests these and more, without any language model:

- entity links, from Wikidata's labels;
- property mappings, where a linked entity's date sits beside yours;
- Wikidata's single-value constraints for mapped properties, each checked
  against a planted violation;
- rules found in your beliefs, such as `Q(X, Y) :- P(X, Y)`, with their
  exceptions;
- references, chosen only among works already in the store;
- links for unlinked mentions: an item whose text names another item's
  text, as whole words in any case, and does not link to it. Only short
  texts are taken as names (3 to 60 characters, six words at most, not
  ending in `.`, `?` or `!`). Accepting it writes the link.

`find suggestions` lists them in the inspector's place, at most five,
ranked by evidence, each in the two rows of any offer: what it offers
(`suggestion 1  link …`), then, dim, `from the memex:` and why it was
offered. A suggestion is a claim
whose source is the memex: `adopt suggestion N` adopts the Nth, with its
evidence, and `dismiss suggestion N` sets it aside until its evidence
changes. While they are listed, suggestions keep their places, so the
number that adopts one stays its number.

## 4. Around the memex

### 4.1 Documents and results

Your drafts, your co-authors' revisions and your analyses' results come
in so that the same rules check them, each change can be chosen, and a
result is marked when its data changes.

**Import a document** in Markdown, Word, OpenDocument, LaTeX or HTML, by
its extension (`.md` or `.markdown`, `.docx`, `.odt`, `.tex`, `.html` or
`.htm`; any other is refused):

```
import document draft.docx
```

Headings become outline levels; paragraphs, list items, quotes, code,
display mathematics and tables become items; citations `[@key]` become
`cites` facts. What the outline cannot hold (footnotes, images, raw
blocks, definition lists, some attributes) is counted and reported, not
hidden.

**Import it again** after a co-author has revised it, under the same file
name, and the memex compares the new version with what it imported before.
If the revision has another name, say which document it revises by an
item of it: `import document essay-v2.md as 2` compares it with the
imports of the document item 2 came from, and records it as a version of
that document, so a later import under either name finds the same
history.
It opens a supposed world with each change as one entry: changed text,
added items, deleted items, and items moved under another heading (when a
heading changes its level, for one). Each change takes the two rows of
any offer: `change 52  changed 2.2.1: …`, then, dim, `from the revision
of essay.md`; one dropped stays, dim, ending `(retracted)`. While the
changes fill the screen, the outline is not drawn, and keys that would
edit it or move in it do nothing; the foot says how to leave.
`dismiss change N` drops a change;
`adopt world` keeps the rest; `dismiss world` drops them all. Your own items stay where you
put them.

**Export** writes the outline, or one section, through Pandoc. The format
comes from the file's extension: `.md`, `.docx`, `.odt`, `.tex`, `.html`.

```
export paper.docx
export section.md 2
```

The document's metadata lists the hashes of the journal entries its text
came from.

An export holds words, structure, mathematics, citations and links, and
nothing that acts. Text that reads as an image (`![…](…)`), as raw HTML or
LaTeX (`<script>`, `\input{…}`), is left out, and a link whose target has a
scheme other than `http`, `https`, `mailto` or `doi` keeps its text but
loses its target; a relative link, such as `#methods` or `notes.html`, keeps
it. An export replaces a file of the same name without asking. Imports are
read by Pandoc with `--sandbox`, so a document cannot pull in another file
(LaTeX `\input`, `\include`).

**Check a quote:** `check quote` on an imported item says whether its text
still appears in the document it came from, as it was imported (the copy
in `blobs/`, not the file as it is now), ignoring spaces, hyphens and
case: `quoted` or `misquoted`.

**Check a source:** `check source` on an item with a `doi(Entity,
"10.…")` fact asks Crossref for that DOI, through `curl`, and rules:
`found`, `misattributed` (the title differs from the item's
`title(Entity, "…")` fact; with no such fact, a DOI that resolves is
found), `not_found`, `unavailable`, or `retracted`.

**Import results** from an analysis. In Python, at the end of the
analysis:

```python
from memex_results import write_results
write_results("results.json",
              results=[("mean_delay", 1.52, 0.07, "s")],
              inputs=["data.csv"])
```

(`helpers/python/memex_results.py`). The script's name is taken from the
command line unless you give `script="…"`; a relative input path is read
from the results file's directory. Then `import results results.json`. The
values go under a new last heading, `Results of SCRIPT@COMMIT`
(`@uncommitted` outside git), one item each, written `mean_delay = 1.52 ±
0.07 s`. Each value arrives as a claim whose source is the analysis, with
its standard uncertainty, unit and provenance: the script, its commit, the
command, and a hash of each input file. If an input file changes later, the
result is marked `input changed since:`. Only regular files are read for
this; anything else, such as a pipe or a device, counts as changed.

### 4.2 Files and tools

#### Files

A journal directory holds:

| File | Holds |
| --- | --- |
| `journal.jsonl` | the entries, one JSON object per line, hash-chained |
| `seals.jsonl` | the sealer's signatures, dated |
| `keys.jsonl` | the public keys that signed seals |
| `blobs/` | imported files, each named by its SHA-256 hash |

The format is version 1, described in docs/design.md, § The journal
format. Text is stored in Unicode NFC.

The signing key is a separate file, readable only by you (mode 600; the
sealer refuses a key that gives the group or others any permission). Keep
it out of the journal directory. If it is lost, the journal and its old
seals still check, because `keys.jsonl` holds the public half; `sealer init`
with a new key file adds a new key for new seals. (Read from the code; this
recovery has not been tested.)

##### Purging

Purge is the one way to remove content: it replaces an entry's body with
null and keeps the entry's hashes, so the chain still checks but the
content is gone from the journal. A file the entry imported stays in
`blobs/`. It is a library call in this prototype (`Journal::purge`), with
no command.

#### Tools

All run from the repository with `cargo run -q -p <crate> --`, where the
crate is `sealer`, `check` or `memex`; `wikidata` and `measures` are
programs of the `tools` crate: `cargo run -q -p tools --bin wikidata --`.
Or run them from `target/release/` after `cargo build --release`.

| Program | Usage | Does |
| --- | --- | --- |
| `sealer` | `sealer init DIR KEYFILE` | makes a signing key and records its public half in DIR |
| | `sealer run DIR KEYFILE [--once] [--interval-ms N]` | seals new entries, every 200 ms by default, and waits for a journal not yet made; `--once` seals and exits |
| `check` | `check DIR` | checks a journal: the chain, every seal's signature, that every entry is sealed, and the blobs. Prints `ok:` and counts, or the first fault; exits 1 on a fault, so a journal written while no sealer ran fails until one has |
| `wikidata` | `wikidata --items Q2548,Q15180 --out FILE` | writes those items of Wikidata as Prolog facts |
| | `wikidata --min-sitelinks K [--limit N] --out FILE` | the same for up to N items (100 000 if not given) with at least K site links |
| `measures` | `measures DIR` | the measures report for the prototype's trial, from the journal alone: the violations shown and later corrected, the suggestions adopted and those later retracted, and your actions by kind, with the share that was clerical (searching, following links, navigating) |
| `memex` | `memex [--config FILE] [--mouse \| --no-mouse] DIR` | the notebook; the flags override the configuration's `[mouse]` |
| | `memex --print-config` | writes the default configuration |

`make measures JOURNAL=…` runs the measures report on a journal. The
`wikidata` tool needs network access to query.wikidata.org.

### 4.3 Configuration

The memex reads one TOML file, named by the first of these that is given:

1. `--config FILE`;
2. `$ADVERSARIA_CONFIG`;
3. `$XDG_CONFIG_HOME/adversaria/config.toml`, when `XDG_CONFIG_HOME` is
   set;
4. `~/.config/adversaria/config.toml`.

If the file named is absent, the defaults apply; the memex does not look
further, even if a later one exists. (A file given by
`--config` must exist.) To start, write out the defaults and edit them:

```bash
mkdir -p ~/.config/adversaria
```

```bash
cargo run -q -p memex -- --print-config > ~/.config/adversaria/config.toml
```

The same file is in the repository as docs/config.example.toml.

It has four sections. Anything you leave out keeps its default. The
defaults are below; `[[plumb]]`, which has no defaults here, is in § 2.3.

```toml
[quasimodes]
leap_down = ["RightAlt", "IsoLevel3Shift"]
leap_up = ["LeftAlt"]
command = ["LeftControl"]
suppose = ["LeftSuper", "RightSuper"]

[keys]
F1 = "palette"
F3 = "adopt"
F4 = "retract"
F6 = "switch"
F7 = "go back"
F8 = "go forward"
F10 = "quit"

[mouse]
enabled = true
```

**`[quasimodes]`** gives each quasimode a list of keys (or one key, as a
string). Naming a quasimode replaces its default keys; each needs at least
one.

**`[keys]`** binds a key to a command line, exactly as you would type it in
Command, such as `"pane suggestions"` or `"export draft.md"`; or to
`"palette"`. A key named here replaces its default; `""` unbinds it.

**`[mouse]`** has one setting, `enabled`: `true` (the default) or `false`.
See § 2.3, The mouse.

**`[[plumb]]`** adds a plumbing pattern, offered on a right-click before the
built-in ones; there may be any number, in order. See § 2.3, Plumbing.

**Key names:**

- modifiers, for quasimodes only: `LeftAlt`, `RightAlt`, `LeftControl`,
  `RightControl`, `LeftSuper`, `RightSuper`, `LeftHyper`, `RightHyper`,
  `LeftMeta`, `RightMeta`, `IsoLevel3Shift` (AltGr on many layouts),
  `IsoLevel5Shift`;
- for quasimodes or bindings: `F1` to `F35`, `Insert`, `PageUp`,
  `PageDown`, `Menu`, `Pause`, `ScrollLock`, `PrintScreen`, `CapsLock`,
  `NumLock`. Unbound, `Menu`, which Shift+F10 also is, opens the
  right-click's menu at the cursor;
- for bindings only: `ShiftUp`, `ShiftDown`, `ShiftLeft`, `ShiftRight`,
  `ShiftHome`, `ShiftEnd`, an arrow, Home or End with Shift held. A
  quasimode cannot use one: it is two keys, and if Shift is let go first,
  the quasimode would not see its key's release. Unbound, one selects
  (§ 2.3, Selecting).

The editing keys (Enter, Tab, Backspace, Delete, Esc, the arrows, Home,
End), Shift alone and printable characters cannot be configured.

The memex refuses to start on a file it would have to guess at, and says
which file and why: a section, quasimode, key or command it does not know,
an editing key, a quasimode with no keys, one key given two jobs, or a
`[mouse]` setting other than `enabled = true` or `false`. To move a key
from a binding to a quasimode, unbind it as well:

```toml
[quasimodes]
leap_up = ["F1"]

[keys]
F1 = ""
F12 = "palette"
```

Examples:

- A laptop whose desktop takes Super: `suppose = ["RightControl"]`
  (Right Ctrl has no job by default).
- Keys only, no modifiers: `leap_down = ["F11"]`, `leap_up = ["F12"]`,
  `command = ["Insert"]`. Tap them to latch if holding a function key is
  awkward.

The hints on screen (such as "F3 adopts") follow your bindings.

### 4.4 Troubleshooting and limits

#### Troubleshooting

- **"memex: no window: …"**: the memex found no display. Run it in a
  desktop session under X11 or Wayland (`DISPLAY` or `WAYLAND_DISPLAY`
  set).
- **"memex: cannot draw: …"**: no Vulkan or OpenGL driver was found. Inside
  `./dev-shell` Mesa's are used. With NVIDIA's own driver outside NixOS,
  run the memex through nixGL (recalled, not tested).
- **A quasimode key does nothing:** your desktop may take it
  (Super often opens a launcher; Alt may open a menu). Rebind it (§ 4.3).
- **"sealer: the clock went back: it says …, before the last seal's date,
  …; no seal written"**: the system clock is set to a time before the last
  seal, and the sealer stopped rather than write a seal that `check` would
  refuse. Set the clock right, or wait until it passes that date, and start
  the sealer again; the entries written meanwhile are sealed then.
- **"memex: configuration: …"**: the file and the reason are named; fix the
  line it names. `memex --print-config` shows a file that works.
- **"no command …"**: `?` in Command lists the commands that match what you
  typed; F1 lists them all.
- **"not a Prolog fact" / "not a Prolog rule"**: check brackets, commas and
  quotes. Variables start with a capital letter; atoms with a small one.
- **A constraint is "not kept"**: it did not fire on the planted violation,
  so it could never fire. Check that the body can be true.
- **A message about Prolog, or no findings where you expect them:** SWI-Prolog
  must be on the path (`swipl --version`). `./dev-shell` provides it.
- **Plain `nix-shell` prints an error about `<nixpkgs>` and "uses bash
  from your environment":** nixpkgs is not on your `NIX_PATH`, or is there
  as a flake, which needs flakes enabled. `nix-shell` only wanted an
  interactive bash from it, and uses your own instead; the shell is
  otherwise complete. `./dev-shell` avoids the message.
- **`check` reports a fault:** it names the line and the fault. Do not edit
  the journal by hand; it is hash-chained, and any change breaks the chain
  from that line on.
- **"the journal's last line was unfinished, as a crash leaves it":** the
  memex stopped, or the machine lost power, while it wrote an entry. The
  entry was never finished, so it was never kept: the memex keeps the line
  in `journal.jsonl.torn`, writes the journal without it, and opens the
  room. Only a last line with no newline, or with zero bytes in it, is set
  aside; any other fault is refused, as `check` would report it.
- **"… is open in another memex"**: a room is open in one memex at a time.
  Close the other, or wait for it to end.

#### Limits

This is a prototype, built for a one-month trial. Known limits:

- It has been run only under test: through its own key events, and by a
  script that types into its window under Xvfb (X11) and a headless sway
  (Wayland). Not on a real desktop or GPU, and not with a real keyboard
  under Wayland.
- The quasimode keys are provisional; they have not been tried on real
  keyboards.
- Images are not shown: views are drawn in characters.
- Re-importing a document does not detect a block moved past others; it
  shows as a deletion and an addition. A block that keeps its place among
  the others but falls under another heading is moved.
- Seal dates are not shown in the memex. (`check` refuses a seal dated
  before the seal before it.)
- Integers larger than 2^53 − 1 are refused in the journal.
- Only the Wikidata subset tool exists; the full subset is not built.
- A mouse's back and forward buttons are not driven by the window test;
  `go back` and `go forward`, and F7 and F8, are.
- The inspector looks for an item's unlinked mentions in every item's
  text each time it is drawn: on the 10,000-item timing test this added
  about 1–2 ms a keystroke, measured on items whose text is too long to
  be a name; on an item whose text is a name it was not timed alone.

docs/log.md records every result and fault found so far.
