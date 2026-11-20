# Introduction: what Adversaria is for

Notes mix what you checked with what someone told you. A date from a
talk sits beside a date from a book, and nothing says which is which.
Months later the two disagree in your essay, and no one noticed. A
co-author changes a paragraph, and you see only the new version. A
budget is right until one number changes and the total quietly goes
wrong.

Adversaria is a notebook for that problem. It keeps what *you* hold true
apart from what a source *says*, notices when they conflict, and lets
you try "what if the other source is right?" without losing anything.

It is a prototype, built to find out whether this helps. Nothing below
is proven useful yet: answering that is what the prototype is for.

## Three uses

**An essay with sources.** You write, in your outline, that Buran flew
in 1988. A speaker said 1989; you write that too, as what the speaker
says: `Talk-notes says that the date of Buran-flight is 1989.` You add
one rule, `Nothing has more than 1 date.` The notebook now marks both
items: the sources disagree. It does not decide for you. You can
question the claim (who else says it, what would adopting it break), or
suppose the speaker is right and see, side by side, what changes in
your notes. The tutorial does exactly this, in its first part.

**A co-author's revision.** You import a draft, written in Word or
Markdown. Your co-author sends a new version, and you import it again.
The notebook opens their changes as a separate version of your notes,
one change per entry: text changed, paragraphs added, removed, or moved
under another heading. You drop the changes you reject and keep the
rest. Your own notes stay where you put them. Later, `check quote` tells
you whether a passage you quoted still appears in the document, and
`check source` whether a paper you cite has been retracted or is not
what you think it is.

**A budget, or any small model.** "The income is 2000." "The rent is
1200." "If the rent is X and the food is Y then the cost is X + Y." These
are notes and a working model at once. A rule such as "It is false that
the cost is greater than the income." is checked every time a number
changes, and marks what breaks it. Ask "What is the rest?" and the
answer, under the question, follows the numbers. Suppose the rent rises,
and the old and the new budget are shown side by side; your real budget
does not change.

## What makes it different

- **You write in plain English, and it reads.** Short sentences in
  Attempto Controlled English, a precise subset of English, become facts,
  rules and questions. They turn cyan when read. Everything else is
  simply prose, and most of a notebook is.
- **Claims are kept apart from beliefs.** What a source says stays its
  claim, with its source, until you adopt it; and an adoption can be
  taken back.
- **Rules are checked, and checked to be useful.** Before keeping a rule,
  the notebook tries it on a made-up case that should break it. A rule
  that could never fire would give false comfort, so it is refused.
- **Nothing is lost.** Every change is written to a permanent journal.
  Undo, supposing, and rejecting are entries too, so you can go back to
  any point.
- **The record can be proven.** A separate program signs and dates each
  entry. You can show that a note existed, unchanged, on a date: for a
  lab notebook, for priority, or simply to trust your own record.
- **Results stay tied to their data.** Numbers from an analysis in
  Python arrive as claims, with the script, its commit and a hash of
  each input. If the data changes, the result is marked.

## What it is not, yet

It is a prototype, driven mostly from the keyboard, and run so far only
in tests (docs/log.md says which). The mouse selects text and, with a
right-click, offers commands that act on it. Views are drawn in characters,
not images. The words "memex" (the notebook's window, after Vannevar
Bush's 1945 design for a personal knowledge machine) and Prolog (the
logic language it reasons in) appear in the other documents; you need
neither to begin.

## Where next

- docs/tutorial.md: a first session, about forty minutes (an estimate).
- docs/guide.md: every command, the files kept, the tools, and the
  configuration.
- docs/design.md: why it is built this way.
