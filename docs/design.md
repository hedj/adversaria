# bcw-3 memex: working design

11 November 2026. Exported from the living Claude Doc of the same name.

## Purpose

The project is named **Adversaria**, after the notebooks in which scholars kept their observations, excerpts and drafts. A web search found no clashing software (a search cannot prove absence; no trademark registry was searched). Its prototype moves to a repository of its own; this document still calls the system “the memex” where it describes its design.

The memex is a cognitive augment for the one user of bcw-3, in the sense of Bush's memex (1945) and Engelbart's NLS (1968). It holds the user's own material, links it, finds it fast and shows it many ways. Reasoning over it in English comes later, through the translator in [Complex Koopman Reservoir: a local English–Prolog translator](https://claude.ai/artifact/K7iZLMRqHUyunZdX6wSyLS).

This is a working document, kept alongside the design conversation. Figures are estimates unless marked as measured; history is recalled from memory unless a source is linked.

The target hardware is bcw-3's core with bcw-1's peripherals, as the author has decided: its external memory, graphics and display path, storage, input and network (see Hardware, after bcw-1), plus the WAM engine on a second ECP5-85F. The book itself so far gives only the 65,536-byte pool (`pool.pool`).

## A user story

A morning's work on an essay about how Buran's flight software was reviewed. The screens are mock-ups, not software: they are drawn to bcw-1's display, 1280×720 with 16 intensities of one ramp and one tint. The flight dates are real; the talk notes, sources 7 and 9, and the counts of actions are invented for the story.

### 1. Finding your place: Leap

You hold Leap and type “dra”. Three matches appear as you type, with the guide line beneath. You release Leap and land on the DRAKON section; nothing was opened, saved or chosen from a menu. The right pane shows the selected item's links: what it supports, what cites it, and where it is transcluded.

![Mock-up: Leap held, “dra” typed, three matches listed above the guide line](design/s1.png)

### 2. The memex notices a conflict: a constraint

You record a claim from a conference talk, attributed to its speaker: “the 1989 flight was the only one”. It is a claim, not a belief of yours, so the memex reports a disagreement between sources, not a broken fact: your source says 1988, the talk says 1989, and both items get a tinted mark naming the other. The right pane shows the rule, a Prolog denial, the disagreement, and what depends on the date. A fading hint suggests trying the other date with Suppose.

![Mock-up: two items tinted by the constraint one date per event, with the rule and its violation in the right pane](design/s2.png)

### 3. Trying the other date: Suppose, and “do you mean?”

You hold Suppose and type “the flight was in 1989”. The translator restates it in controlled English, “Suppose that the date of Buran's flight is 1989”, and you press Enter. A second world opens beside the actual one, joined line by line. In it, the derived sentence changes from “a year” to “two years” after Energia's first launch, the timeline's point moves, and the constraint has no violation, because your source's date is retracted there. Your actual world is untouched. You decide the talk notes were wrong, and close the supposition.

![Mock-up: the actual world and the supposed world side by side, with the restatement in the guide line](design/s3.png)

### 4. Building the argument: a diagram, Command, and where the work went

You switch the section to an argument map. The thesis and its supports run straight down one vertical; objections sit to the right in a notched shape, and sources are rounded. One objection is tinted: it cites no source. You hold Command and type “link 3.2 source 9”; the noise word “(to)” fills itself in, and on release the edge appears without moving a pinned node. On the right, the journal's count of today's 482 actions shows searching as the largest share, mostly for dates: Licklider's point, measured on your own work.

![Mock-up: an argument map with the main path on one vertical, and the journal's count of actions by kind](design/s4.png)

## Glossary

| Term | Meaning |
| --- | --- |
| Room | a program of the user, run by the executive with its own region and capabilities (`executive.room`) |
| Journal | the append-only record of every change; described below |
| World | a branch of the journal's undo tree, after VPRI's Worlds: it reads through to its parent and keeps its own changes, so suppositions, undone work and versions do not disturb the original; Suppose names one |
| Quasimode | a state kept only while a key is held, after Raskin; described under The outline model |
| Controlled English | a subset of English with one meaning per sentence; here ACE |
| ACE | Attempto Controlled English, a controlled English that its parser turns into logic |
| APE | the Attempto Parsing Engine: parses ACE and restates a parse in ACE |
| s(CASP) | a Prolog-based reasoner used here for counterfactual questions |
| WAM engine | planned hardware, the only place Prolog runs: a second ECP5-85F with 1 GB of DDR3 and two 512 GB A2-class SD cards (index and data), joined to bcw-3 by a dedicated 32-bit link at 100 MHz, running Prolog compiled for the Warren Abstract Machine; see Timing estimates |
| Model accelerator | planned hardware that runs a small neural model proposing ACE for free English |
| Predictive editor | an editor that offers only the words the grammar accepts next |
| Hot layer | the part of the store kept in the 16 MB external memory; see Storage |
| Storage accelerator | bcw-1's device for bulk storage: SPI flash, reached by handles to extents (`12_STORAGE.md` §1.1) |
| Mechanism IDs | N (NLS/Augment), X (Xanadu), J (notebooks), L (live simulation); listed under Mechanisms considered |

## Hardware, after bcw-1

bcw-1's design documents (repository hedj/bcw, `docs/`) specify the peripherals bcw-3 will have. bcw-1 states the same purpose as this memex: "a cognitive augment: an instrument with the immediacy and predictability of a physical tool" (`02_SYSTEM_OVERVIEW.md` §1). Most of what it specifies supports the design; two points conflicted with it, and the author has resolved both.

| Device | What bcw-1 specifies | What it means for the memex |
| --- | --- | --- |
| External memory | 16 MB of 32-bit asynchronous static RAM, about 420 MB/s usable; "no worker can address \[it\] at all. Everything moves between them by block transfer" (`04_MEMORY_ACCELERATOR.md`) | the 16 MB hot layer is confirmed, but it is a staging store: a process reaches it only by transfers, through the mover |
| Pool and regions | 64 kB pool, 48 kB of it for workers; "A process shall fit its region whole", and a larger one "does not run" | **Resolved:** the memex software and its working data fit a region and move blocks in and out of external memory; Prolog runs only on the WAM engine |
| Display path | a beam-racing compositor with no framebuffer: 1280×720 at 85 Hz, a change ahead of the beam visible "within one scanline" (15.69 µs); tiles placed by the supervisor; 4-bit intensity surfaces through one 16-entry RGB ramp; vector content as monotonic strokes; one tint colour for marked strokes (`07_SURFACE_ENGINE.md`, `08_GRAPHICS_DISPLAY.md`) | display latency is negligible against the 0.1 s limit, so computation sets it. Panes are tiles, text comes from glyph surfaces or strokes, and the lines joining matching spans, charts and diagrams are strokes. Highlighting has 16 intensities and one tint, not a palette |
| Mover | one operation: a masked, strided rectangle copy between extents, also used for transfers to external memory | moves journal blocks, index blocks and glyphs between external memory, surfaces and a region |
| Bulk storage | SPI flash on 6 pins, quad-rate reads after boot, 256-byte pages; only the supervisor erases; the filesystem runs on a worker (`12_STORAGE.md`) | **Resolved:** SPI flash is small, but the append-only journal suits it (pages are programmed in order, erasure is rare). The knowledge base, with Wikidata subsets of about 0.8–9.5 GB, lives on the WAM engine's 512 GB cards |
| Input | two USB ports, keyboard and mouse at boot protocol, polled every 1 ms; events go to the supervisor, which routes them (`13_HID.md`) | each report gives the keys held, so key presses and releases are known and quasimodes work; routing by the supervisor keeps keystrokes private between rooms |
| Network | 100BASE-TX full duplex; the protocol stack runs on a worker; about 49 Mbit/s guaranteed lossless (`10_ETHERNET.md`) | capture and import over the network, parsed in a confined worker |
| Time | a worker has no readable clock, only "suspend me and resume me no sooner than δ" (`01_ARCHITECTURE_RATIONALE.md` §4.5) | consistent with this design: sequence numbers, not timestamps. bcw-1 keeps wall time in the supervisor, a 32-bit counter from a 32.768 kHz crystal corrected by network time (`15_SYSTEM_PERIPHERALS.md` §2.1); no worker reads it |

Prolog runs only on the WAM engine (see Glossary). The engine holds the link facts, the knowledge base and the worlds, and answers queries. bcw-3 holds the journal, the outline and the display; its memex software stays small enough for a region and asks the engine through a handle. The engine joins bcw-3 by a dedicated link, not the network: 32 bits wide at 100 MHz, 400 MB/s raw. bcw-1 budgets 50 MB/s for worker transfers through the mover, within a peak demand of 212 MB/s on the 420 MB/s external memory (`04_MEMORY_ACCELERATOR.md` §3.3), so the link is not the slower path for a block. bcw-3 runs at 95 MHz, so the link crosses clock domains, and it takes about 32 data pins plus a clock and handshake on each FPGA.

The engine carries 1 GB of DDR3 and two 512 GB A2-class SD cards, each read in 4-bit mode by its own controller, extended to use its card's command queue and cache, on which A2's random-read rate depends (recalled, not checked). The controller is ZipCPU's open SDIO controller (github.com/ZipCPU/sdspi): 1,438 Xilinx 6-input LUTs without DMA and 2,708 with it, as its source reports; its count in ECP5 LUT4s is not synthesised. The DDR3 holds the working set. The index card holds every fact in three orderings: subject–relation–object, relation–object–subject and object–subject–relation. Each of the seven patterns of bound arguments in a triple is a prefix of one of them, so every lookup costs a bounded number of reads whatever it binds. A fact of more than three arguments is stored as triples. The data card holds labels, strings, item text and large values. Index reads and data reads proceed at once, at an estimated 100–400 random reads on each card within 0.1 s. That bounds the lookups, not the search: a query that backtracks over many answers still costs a read for each.

Each change to a fact writes all three orderings, so changes are buffered in the DDR3 and applied to the index card lazily, when no reads wait: an SD card can pause for tens of milliseconds while it reorganises (recalled, not checked), and a read must not wait behind a write. A lookup consults the buffer as well as the card. The buffer has a bound; when it is full, the engine writes back even if reads wait. After each batch of index writes completes, the engine records on the index card the journal sequence number it has applied up to. The DDR3 loses the buffer at power-off, so at start the engine asks bcw-3 over the link for the journal entries after that mark, and bcw-3 reads them from SPI flash.

## Features

The core is a store of items with permanent addresses, typed links and named trails through them, fast search, views, and an append-only journal of every change. NLS had the addresses and the journal, and Bush's memex had the trails. bcw-3 adds three constraints: no clock in a process that writes, sharing only through capabilities, and storage reached through bcw-1's storage accelerator. Priorities 1–3 order the work within step 1 of the order of work; priority 4 needs steps 2 and later.

| Feature | From | What it needs on bcw-3 | Priority |
| --- | --- | --- | --- |
| Items with permanent addresses, down to a paragraph or statement | NLS: every statement had an address | addresses in the append-only journal, which never change (adopted: X3, N2) | 1 |
| An append-only journal of every change, which can be cited | NLS's Journal | each edit a committed world (Worlds); entries ordered by sequence number, because the memex holds no clock: a process that has had one keeps only reading capabilities, and the memex must write (`system.no-clock`) | 1 |
| Typed links from any item or span to any other | Bush's associative linking; NLS links | a plain link table (from, to, type) that native code follows in step 1; from step 2 the WAM engine also holds links as facts (`supports(A, B)`, `cites(A, B)`), so Prolog can follow them | 1 |
| Search by words and by structure | NLS content filters | an inverted index over text and the link table, in native code, answering within the 0.1 s limit (L6), which serves `design.latency`; Prolog over links from step 2 | 1 |
| Views: the same structure as outline, levels or filtered | NLS view specs | views as queries over the store, rendered without copying; content filters in Prolog from step 2 | 2 |
| Several panes side by side | Bush's twin screens; NLS windows | two or more views at once, for comparing and linking | 2 |
| Named trails: ordered paths through items | Bush: trails named, recalled and copied for a colleague | a trail is a named list of links | 2 |
| Annotation: marginal notes attached to a span | Bush's marginal notes; NLS comments | anchors to character ranges that survive edits | 2 |
| Sharing trails | Bush | passed by capability in a request through the executive, so that a room's answers still depend only on its own calls (\`system.answers\`, \`executive.no-interference\`), and revocable (`design.revocation`) | 3 |
| Capture beyond typing | Bush: dictation and a camera | import over bcw-1's Ethernet, parsed by a confined worker; mail, calendars and web pages as well (see Mail, calendars and the web) | 3 |
| An interface for a trained user | Engelbart's chord keyset | keyboard commands and typed controlled English, with the predictive editor to teach it | 3 |
| Reasoning over the store | beyond Bush and Engelbart | items and links as facts, so "what if" and "do you mean?" work on the user's own knowledge | 4 |

## The journal

One append-only journal carries five adopted mechanisms at once: permanent span addresses (X3), every version kept with branches (X4), named statements (N2), who changed what (N8), and a log of computation (J4). Nothing in it is ever overwritten, so each follows from one rule.

1. Every change is an entry with a sequence number, its author (the user or a room), and its operation: text appended, structure changed, a name set, a link made, or a query run with its answer, unless the room has switched off the journalling of queries.
2. Text is appended once and never rewritten. The address of a span is its position in the journal, so it never changes and is never reused (X3).
3. A statement is a list of spans plus its place in the outline. An edit writes a new list as a new entry; the old one stays, so every version is kept. A branch is a world sprouted from any entry (X4).
4. A name is an entry mapping it to a statement, so links that use names survive restructuring (N2).
5. Each statement's latest entry names its author and sequence number, so "who changed this, and since which entry" is a query. The memex holds no clock (see Features): the supervisor dates each entry, and the memex never reads the dates (N8, `system.no-clock`).
6. A query and its answer are journal entries, recording the world they ran in, so any result can be run again on the same state (J4).

Two more follow without extra mechanism. Annotation anchors on span addresses survive edits, because surviving characters keep their addresses. Transclusion (X1) becomes a list of spans that points into another statement's text; it is adopted under References.

The cost is growth: the journal is never shortened, so span lists need compacting, and a true deletion, for privacy or by law, is a purge: it removes an entry's content and keeps its hash, so the chain still checks (see The journal format).

The journal also measures the user's own work, as Licklider's 1957 study of himself did: he found about 85% of his "thinking" time went to getting into a position to think, by searching, plotting and transforming ("Man-Computer Symbiosis", 1960, §3.1). A view over the journal counts the user's actions by kind (searching, following links, transforming, writing, deciding), so the memex shows where the work goes and which clerical steps to remove next. It counts actions, not seconds: entries carry sequence numbers, not timestamps, and no worker reads a clock (`system.no-clock`).

Undo has two forms, after the literature on selective undo (Berlage 1994; Prakash and Knister 1994; Yoon and Myers 2015, where selective undo made programmers about twice as fast). Plain undo and redo move along the current branch, as everyone expects. To reverse one earlier decision, such as an adoption, the user **retracts** it instead of rewinding to it: a retraction is a new entry that reverses the old one and keeps everything done since, and before it is confirmed the dataflow graph lists what depended on it. Retracting is unadopting, generalised to any entry, so it adds no concept. Every adoption of a suggestion is an entry marked as one, with the suggestion's evidence, so an adoption made without checking can be found and retracted later, the user can see the path of their decisions, and, since entries are facts, a query searches them (“every suggestion I adopted this week”, “what did I decide about Buran's date”).

Undoing and then editing starts a new branch instead of discarding the old one, so undo and redo form a tree, as in Emacs and Vim; the journal stays append-only, and the tree is a view over it. A history pane shows the tree; it is described below. A world and a branch of the tree are one concept: each is an alternative state of the store chosen by a decision, and undoing is a decision too. Branches come only from Suppose and from undoing and then editing, and each is named by its first change, which the user may rename. A branch always keeps its own state, so moving between worlds asks nothing and loses nothing, as in Gitless (Perez De Rosso and Jackson 2013). The actual world is the branch the user works on; closing a supposition leaves its branch in the tree, to be found, compared and reopened; and worlds open side by side, where one edit can apply to all of them, after Parallel Paths and subjunctive interfaces (Terry and Mynatt 2004; Lunzer and Hornbæk 2008). Because retraction keeps most work on one branch, most users meet the tree only in the history pane.

The journal is a scientific record, so its entries are dated and chained. These are obligations, not preferences: research-integrity codes, funders and journals expect a record that is attributable, unaltered and dated, as a bound lab notebook's dated pages are.

1. **Hash-chained.** Each entry holds the SHA-256 hash of the entry it follows, so the journal is a chain of hashes along each branch and a tree of them across branches, as Git's commits are. Changing or removing any entry breaks every hash after it, which anyone can check from the journal alone.
2. **Dated by the supervisor.** The memex holds no clock (`system.no-clock`), so it cannot date its own entries. bcw-1's supervisor keeps wall time, a 32.768 kHz counter set from the network; it stamps each entry with the date as the entry commits, in a log of seals of its own whose records are hash-chained too, each binding an entry's hash to a date and to its author's signature. The supervisor keeps the dates, and the memex never reads them, so `system.no-clock` holds: the supervisor draws each date in a tile beside its entry, which the memex cannot read, and answers questions by date (“what did I decide last week”) in its own tile. Choosing an entry there moves the memex to it as a user action, so the memex learns which entry the user chose, never when it was written. On a PC the prototype reads the system clock instead.
3. **Anchored outside.** From time to time the hash at the head of each branch can be sent to an external timestamping service (RFC 3161), which proves that the record existed by that date without revealing its content.

### The journal format

The journal must outlive the memex, so its format is open, plain and documented, and readable with common tools. One format serves for storage and export; the query protocol's tagged cells are only how terms cross the link.

| Part | Specification |
| --- | --- |
| Files | each room keeps a directory: `journal.jsonl`, written only by the memex; `seals.jsonl`, written only by the supervisor; `keys.jsonl`, the public keys of the authors whose entries the journal holds; and `blobs/`, holding images, figures and imported files, each named by the SHA-256 of its content |
| Lines | JSON Lines: UTF-8 text, strings in Unicode NFC, one JSON object per line, never rewritten except by a purge, which replaces one entry's `body` with null. The first line of each file is a header: `{"format": "memex-journal", "version": 1}`, or `"memex-seals"` or `"memex-keys"` |
| An entry | `seq`, its sequence number; `parent`, the `seq` of the entry it follows, null for the first; `prev`, the hash of that entry; `kind`, one of edit, link, adopt, retract, suppose, name, import, purge, undo, redo; `body`, whose fields the kind defines; `hash`, the entry's own hash |
| A term | a JSON integer or number; a JSON string for a Prolog string; `{"a": name}` for an atom; `{"v": n}` for a variable; `{"f": name, "args": [...]}` for a compound term |
| Kinds that carry more | adopt carries the suggestion and its evidence (support, exceptions, violations); retract names the entry it reverses; import carries its provenance as W3C PROV-JSON and refers to its files in `blobs/` by hash |
| A seal record | `seq` and `hash` of the entry; `date` in ISO 8601 UTC to the second; `author`, the fingerprint of the author's key; `sig`, the author's Ed25519 signature (RFC 8032) over the entry's hash; `prev`, the hash of the previous seal record; and its own `hash` |
| The hash | SHA-256, in lowercase hexadecimal, of the record without its `hash` field and with its `body` replaced by the SHA-256 of the body, serialised by the JSON Canonicalization Scheme (RFC 8785). The entry keeps that `body_hash`, so a purged body leaves every hash checkable, and a purge entry names what it removed and why |
| Checking | a reference checker of an estimated 100–150 lines of Python is published with the format; it verifies both chains, every signature against `keys.jsonl`, and every blob's name, and needs nothing from the memex |

Text costs space: a JSON entry takes several times the bytes of its tagged cells (an estimate), which counts against SPI flash's capacity, still an open question. The format's version number allows later kinds and fields; a reader ignores fields it does not know.

The history pane draws the undo tree by the rules for diagrams:

1. **The current branch is the main path.** It runs straight down one vertical, newest at the foot; branches left by undoing sit to its right, as DRAKON puts exceptions to the right.
2. **Entries are grouped.** A run of edits to one item becomes one node, named by the item, so the pane stays within the size limits for diagrams; a node opens on request to its entries.
3. **Kinds are told apart by shape.** An adoption of a suggestion, a supposition, an import and an ordinary edit each have their own shape, and an adoption shows the evidence it was offered with.
4. **A query filters it.** The pane can show only adoptions, only one item's history, or the result of any query over the journal; Leap moves through it like any other view.
5. **Pointing previews, choosing compares.** Pointing at a node previews the store as it stood there; choosing it opens that state in a second pane, side by side with the present, never animated between them. Choosing an earlier decision offers to retract it and keep all later work; moving the current state to the node instead is an ordinary undo or redo, itself a journal entry.

## Dataflow

The memex is a dataflow graph over the journal, in the spreadsheet model. This one mechanism replaces five candidates (L1–L5), so a reader holds one concept instead of five.

| Former candidate | Its place in the graph |
| --- | --- |
| L1 Recomputation by dependency | the engine: a change recomputes only its dependents |
| L2 Reactive documents, with executable content (J1, J2, J6) | an item holds prose, runnable Prolog queries (each a node, kept apart from the engine that runs it), their live results, and controls, so the reader changes assumptions in place. The notebook weakness of hidden state from cells run out of order cannot arise, because every node is a pure query over a committed world |
| L3 Ladder of abstraction | the graph evaluated over a range of one input, then one case selected |
| L4 Parameters as controls | source nodes that the user sets with a slider, menu or checkbox |
| L5 Named scenarios compared | one instance of the graph per named world, with a live comparison |

1. Nodes are items, parameters and queries. A query is pure Prolog over one world.
2. Edges are the facts and parameters a query read, recorded when it runs.
3. A new journal entry marks every direct and indirect dependent as dirty, and only those are recomputed, in dependency order. A cycle is an error.
4. A control is a source node in an item's text. Each value it takes is a world with one fact changed. While a control is dragged, those worlds are not journalled; the value at release is one journal entry.
5. A sweep evaluates the graph over a range of one parameter as sibling worlds; pointing at one result selects that world.
6. A scenario is a named world. A table comparing scenarios is a query across worlds, so it stays live; Excel's scenario reports, by contrast, are not recalculated.

It holds three properties by structure. Nodes are pure queries over committed worlds, so there is no hidden state. The memex holds no clock and has no volatile function, so the same inputs always give the same values (`system.no-clock`, `system.answers`). Any value can be rebuilt from the journal, so caching values is an optimisation, not a source of truth.

SWI-Prolog's incremental tabling, recalled from memory and not checked, records which facts each tabled answer depends on and recomputes only what a change invalidates: the engine of rule 3 above, built in. s(CASP) is not tabled, so how its counterfactual queries join the graph is open.

Constraints are nodes too, so the memex turns up flaws without being asked, as Licklider wanted computers to do in trial-and-error thinking. A constraint is a Prolog denial over one world, such as two items that claim the same date for different events, or a claim that cites a retracted source. Its node's value is the list of violations, recomputed on every change like any other node, and each violation shows beside the items it names. Constraints run on the WAM engine, so they arrive with step 2. The user need not write them all: the translator's model suggests constraints, facts and rules, each shown with the violations or exceptions it would raise, and each entering the journal only when accepted (see the translator document). The model runs all the time and keeps a pane of suggestions; one click adopts a suggestion and another unadopts it, each an ordinary journal entry under universal undo.

### Response time (adopted L6)

The graph must update within the limits that keep interaction direct (Nielsen, 1993, updated 2014, citing Miller 1968 and Card, Robertson and Mackinlay 1991):

| Interaction | Limit | Why | Meets it |
| --- | --- | --- | --- |
| Navigating and editing the outline, a Leap, following a link | 0.1 s | "the limit for having the user feel that the system is reacting instantaneously" | native code on bcw-3, in software: an index lookup and a redisplay (estimate) |
| Dragging a control, changing a view that runs a Prolog filter | 0.1 s | the limit for direct manipulation | the WAM engine, for updates that stay within about 100 ms; a simulation update is estimated at 10–300 ms, if the engine supports attributed variables, so only small models stay live; software on bcw-3 does not |
| A query, a parse with its restatement, a "do you mean?" proposal | 1 s | keeps "the user's flow of thought" | the WAM engine: a parse and its restatement in about 5–13 ms; with a model proposal, a round in about 0.4–1.2 s |
| A long simulation or search | 10 s | keeps attention; beyond it, show progress and allow interruption | most work, with a progress indicator |

### Timing estimates

Every Prolog timing in either document comes from this table; the translator document gives the model's own timings.

| Operation | Measured on a 4-core Xeon (SWI-Prolog) | Software on one bcw-3 thread (estimate) | WAM engine (estimate) |
| --- | --- | --- | --- |
| APE parse of one sentence | about 2.5 ms (tokenise 0.96, parse 1.52, anaphora 0.03) | about 1–15 s | about 3–8 ms, plus tokenising in native code on bcw-3 |
| APE restatement | about 1.1 ms | about 1–15 s | about 2–6 ms |
| Parse and restatement together | about 3.6 ms | about 2–30 s | about 5–13 ms |
| s(CASP) counterfactual or simulation query (small tests) | 2–66 ms | seconds | about 10–300 ms, if the engine supports attributed variables |
| Model accelerator: one proposal of about 100 bytes | — | — | about 0.4 s, on the accelerator |
| One "do you mean?" round with the model, 1–3 candidates | — | — | about 0.4–1.2 s |

The engine's estimates follow one rule: the Xeon's time for the Prolog work, multiplied by 2–5. Tokenising, about 38% of APE's time in the profile, moves to native code on bcw-3 and is left out: a parse is 1.55 ms of Prolog on the Xeon, so 3–8 ms; a restatement 1.1 ms, so 2–6 ms. The software estimates scale by bcw-3's speed of one instruction per 8 cycles at 95 MHz per thread.

Animation is paced by the compositor, which takes each tile's descriptor at the start of its pass (bcw-1), not by the process, so the memex never reads a clock (`system.no-clock`).

Sources: [Response Times: The 3 Important Limits](https://www.nngroup.com/articles/response-times-3-important-limits/), [Observable runtime](https://github.com/observablehq/runtime), [Explorable Explanations](http://worrydream.com/ExplorableExplanations/), [Up and Down the Ladder of Abstraction](http://worrydream.com/LadderOfAbstraction/), [Manipulate](https://reference.wolfram.com/language/ref/Manipulate.html), [Excel recalculation](https://learn.microsoft.com/en-us/office/client-developer/excel/excel-recalculation), [Introduction to What-If Analysis](https://support.microsoft.com/en-us/office/introduction-to-what-if-analysis-22bffa5f-e891-4acc-bf7a-e4645c446fb4).

## Views, references and anchors

Four adopted mechanisms: three merge candidates that did the same job in different systems, and workspaces (J5) stands alone. All three rest on the journal's permanent addresses, and every view is a dataflow node, so it stays live.

| Mechanism | Merged from | How it works here |
| --- | --- | --- |
| Views | N5 view specs, N6 freezing, J3 synchronised views, X5 side-by-side versions | A view is a query over the store: an outline level, a truncation, and a content filter written in Prolog (from step 2, once the WAM engine is added). Views open in panes, and a pane can be frozen in place. Two panes can show versions or worlds side by side, with lines joining spans that share journal addresses. |
| References | X1 transclusion, J6 code blocks called as functions, J3 mirrored output | Content is defined once and used elsewhere by reference: a span (transclusion), a query called with arguments, or a result shown in another place. A reference is resolved through permanent addresses and shows its origin, so the same content appears "knowably" in more than one place. |
| Anchors | N3 markers, X2 links that cannot break | A named point or span is anchored on journal addresses. Links attach to anchors, can be followed from either end, and stay attached through edits; only deleted characters drop out. |
| Workspaces | J5 saved workspaces | The open panes, their views and their layout form an item in the journal, kept as the user works with no Save. A workspace is restored or switched by name, and any earlier state of it can be reopened from the journal. |

NLS reported that a skilled user changes view settings "several times in as many seconds", so views must meet the 0.1 s limit (L6). Xanadu never shipped as a product, and Nelson's paper "Xanalogical structure" (2000, listed in the sources) admits "difficulty delivering product": only the parts that the append-only journal makes cheap are adopted.

## A/B comparison

Science compares: two states that differ in one thing, seen under the same questions. Worlds give the states; the interface makes comparing them direct. Gleicher et al. found that every design for visual comparison is built from three parts: juxtaposition, superposition and explicit encoding of the differences (“Visual comparison for information visualization”, 2011). The mechanisms below are proposed.

1. **A comparison is an item.** It names its worlds (A and B, or more) and the views to compare. Like any item it is kept in the journal, linked and searched, so a comparison made last month reopens as it was.
2. **One variable at a time.** The comparison states what differs between its worlds, computed from their own changes (“A and B differ in: the date of Buran's flight”), and marks it with the tint when more than one thing differs.
3. **Juxtapose.** Each world in its own pane, side by side, rows aligned by item and joined by lines where they correspond. Charts in every pane share one scale, since position on a common scale is judged best (Cleveland and McGill).
4. **Superpose.** For two worlds, one chart or diagram holds both: A in the intensity ramp and B in the one tint, which bcw-1's display supplies.
5. **Encode the difference.** A table of only what differs: facts added and removed, derived values with their change, constraint violations gained and lost.
6. **Linked panes.** Scrolling, selecting and Leap move every pane together. An edit applies to all the worlds unless it is the variable under test, after subjunctive interfaces (Lunzer and Hornbæk 2008).
7. **More than two.** A supposition over a range (“suppose the date is 1987 to 1990”) makes a family of worlds, shown as small multiples on one scale; up to about four panes side by side, the span of working memory, and more as small multiples.
8. **A result can become a claim.** The outcome of a comparison can be adopted as an item, citing the comparison as its evidence.

## Claims and beliefs

Nothing that comes from outside is trusted as a fact. An imported document, a web page, a mail, a collaborator's entry, a Wikidata statement, an analysis's result and a model's suggestion all enter as claims: `claims(Source, P)`, the source with its provenance, never `P` itself. The user's own beliefs are what the user has written or adopted. This answers the reviewers' point that knowledge workers deal mostly in who claimed what.

1. **Constraints run on beliefs.** A constraint broken by the user's beliefs is a violation. A constraint broken among claims, or between a claim and a belief, is a disagreement between sources, shown apart and less loudly: it is information, not an error.
2. **Adopting is one click.** Adopting a claim writes an adopt entry that names the claim and its source, so the belief keeps its evidence; retracting is one click too. A suggestion is a claim whose source is the model, so suggestions and claims share one mechanism.
3. **Interrogating is one key.** On any claim, a single key opens questions answered at once from the store: who else claims it, and who claims the contrary; what follows if it is adopted, previewed as a world; where it came from, with its provenance; how this source's earlier claims fared, adopted or retracted; and what the model can find to support or oppose it, among works already in the store.
4. **Sources build a record.** Each adoption or retraction of a source's claim is a fact about that source, so how reliable a source has been is a query, not an impression.

### Testing claims

A claim is tested by the cheapest check that could refute it. These checks come from the author's “Popper” project, which routes each claim of a physics paper to such a judge; it has 105 passing tests but no measured rate of catching errors yet. The mechanisms below are proposed.

| Check | How | Rulings |
| --- | --- | --- |
| **The quote** | every imported claim keeps the verbatim span it came from and the hash of the source; the check ignores spaces, hyphens and case | quoted, or misquoted when the span is not in the source |
| **The source** | the memex fetches the source itself by its identifier (a DOI through Crossref, or an address), never trusting a copy that came with the claim, and records the hash of what it read | quoted, misattributed, not found, unavailable, or retracted |
| **The provenance** | a claim counts as evidence for a belief only if its source rules quoted; a source's support for its own claims, such as a paper's own data, counts as weak | gated, with the reason shown |
| **The weakest link** | every premise of a derived belief names its origin: a source, a constant, or an assumption with its reason. The belief takes the weakest ruling of its premises and lists every assumption it rests on | the weakest ruling, with the assumptions |
| **What is left open** | a relation that depends on a parameter the source does not state is tested over that parameter's range, as a family of worlds | holds, fails, or underdetermined, with the range over which each holds |
| **Independence** | corroboration counts only from sources independent by provenance: not the same author, data or upstream source | independent, or not |
| **Gaps shown** | each kind of claim routes to the checks that can test it; a kind that no check covers, such as an interpretation, shows as unchecked, never as passed | unchecked |
| **The checks themselves** | every constraint the user adopts is tried against a planted violation, to show that it fires | fires, or broken |

The rulings are facts about claims, so they are queried, shown and compared like any others, and each records the hash of what it examined. Uncertain values follow the Uncertainty section, which keeps a bound and a standard deviation apart as Popper's judges do.

### Falsifiability of formal claims

A claim written in Prolog, by hand or through APE, can be measured for how it could be refuted, after Popper's *Logic of Scientific Discovery*. This section is written for an implementer. The measures are proposed; none is built or measured.

**Notation.** H is the program of the user's beliefs: rules, facts and constraints (denials, `false :- ...`). C is the claim, a goal. An observation is a ground atom of an observable predicate. O is a finite set of observations, and k is its size.

**1. Declare the observation vocabulary first.** Prolog's closed world makes every unproved atom false, so without open predicates every claim looks decided. Each predicate is therefore declared, as an adopted fact, to be one of:

- *observable*: the world supplies its facts, as `observable(p/2)`;
- *theoretical*: open, but not observed directly, as `theoretical(q/1)`;
- *closed*: defined by H alone.

Each argument of an open predicate has a domain: a finite list of values, an integer range, or the reals, as `domain(p/2, 1, colour)` with `colour` enumerated. The declarations are claims about the world in their own right, and the rulings below hold only relative to them. A tool generates one `#abducible` line of s(CASP) for each open predicate.

**2. Restrict to one model where possible.** A program without negation through a cycle (a *stratified* program) has one model, so “C fails” means the same in every model. The measures below assume H is stratified, which a static check of the predicate dependency graph establishes. For an unstratified H, s(CASP) finds a model in which C fails, not that C fails in all, and the ruling must say “in some model”.

**3. The measures.**

| Measure | Definition | Procedure | Result |
| --- | --- | --- | --- |
| Compatible | some O consistent with H lets C hold | query `?- C.` with open predicates abducible | compatible, incompatible, or unknown within the budget |
| Independent | some O over the open predicates, observable or theoretical, consistent with H, makes C fail | query `?- not C.` with all open predicates abducible; s(CASP) returns the abduced atoms in its justification. Confirm by adding them as facts, turning abduction off, and checking that `?- C.` fails | independent, or entailed by H (the claim restates its premises) |
| Refutable | the same, with O over observable predicates only | as above, with only observable predicates abducible | refutable, or not empirical relative to the vocabulary (only theoretical facts could refute it) |
| Characteristic number k | the size of the smallest refuting O; Popper's dimension is k − 1 | iterative deepening on n = 1, 2, …: enumerate sets of n observations, only over atoms that C depends on through the dependency graph; for each, add it to H, reject it if a denial fires, and test that C fails, with SWI-Prolog's tabling for speed. Stop at the first falsifier or at the budget | k = n with its falsifier; or k > n, exhausted, for finite domains; or unknown above n |
| Numeric dimension | for a law with real parameters, the number p of parameters a fit can adjust; then k = p + 1 | outside Prolog, in the analysis tools: sample points, and take the numerical rank of the law's Jacobian with respect to its parameters | p and k, marked as a numerical estimate |
| Needed premises | each premise h of H that C's justification uses matters | for each such h: recompute compatibility and k with H without h | needed, or idle |
| Comparative strength | claim A is at least as falsifiable as B when every falsifier of B also refutes A, which holds when A entails B under H | query `?- A, not B.` with abduction; no model means A entails B | stronger, weaker, equivalent, or incomparable |
| Immunisation | adopting hypothesis h rescues claim C from a falsifier found before | every falsifier found is kept as a fact; when h is adopted, re-test C against the stored falsifiers under H with h | the falsifiers that h defeats, shown beside the adoption |

**4. Records.** Each result is a fact, `falsifiability(Claim, Measure, Result, Bound, Budget, Engine)`, citing the hash of the part of H it used, so the dataflow graph recomputes it when that part changes. A falsifier opens as a world, “suppose these observations”, in which the user sees the claim fail. Searches run in the background with a budget, outside the 0.1 s limit; a result arrives when it is ready, and a bound is always shown as a bound.

**5. Engine notes.** Run s(CASP) with `--forall=sasp` until its issue #13 is fixed. Use SWI-Prolog with tabling for the enumerations of stratified programs; reserve s(CASP) for abduction and for unstratified programs. The general problem is undecidable, since Prolog is Turing-complete, so every procedure is bounded by its domain and its budget.

**6. Tests to build first.** Each has a known answer and a planted fault.

1. A claim that restates a fact of H is ruled entailed, not independent.
2. “All swans are white”, with `swan/1` and `black/1` observable over a domain of three birds, has k = 2: one bird that is a swan and black. Making `black/1` closed and empty makes the claim not refutable.
3. A claim that mentions only theoretical predicates is independent but not refutable.
4. A premise that the claim's justification never uses is ruled idle.
5. A straight line through points with small integer coordinates has k = 3; adding a third parameter makes k = 4.
6. After the black swan is observed, adopting “black birds are not swans” is flagged as defeating that falsifier.
7. Each test is also run with its procedure broken on purpose, as Popper's judges are, to show that the test fails.

## Importing results

The memex is not a tool for analysing data; Python, R and Julia are. It takes their results in as facts that carry their provenance, so a claim built on a number can be traced to the data and the code that made it. The mechanisms below are proposed.

1. **One exchange file per run.** The analysis writes its results as rows (CSV or JSON) and a provenance record in the W3C PROV model: the script and its version-control commit, the command, the tool and library versions, and a content hash of each input file. A small helper library in each language writes it, so the analysis changes by one call.
2. **An import is a journal entry.** It adds the results as facts, each citing the import; the history pane shows it in the shape for imports. A figure comes in as an image, shown in the display's 16 intensities, beside the facts it plots.
3. **Values carry units.** A result declares its unit, and constraints check units and ranges as they check anything else: a probability above 1, or a length added to a time, is a violation.
4. **Re-running is comparing.** Importing a new run of the same analysis starts a world beside the old one, so the A/B comparison shows which results changed and which claims depend on them; adopting the new run is a decision like any other, and retracting it restores the old.
5. **Reproducible by construction.** The provenance record holds what is needed to run the analysis again; the memex marks a result whose inputs no longer match their hashes.

On a PC the helper writes the file where the memex reads it. On bcw-3 the file arrives over the network and is parsed by a confined worker, as other captured material is.

## Uncertainty

A measured value without its uncertainty is half a fact. The memex holds four kinds of not-knowing, and keeps them apart, because each is a different question: how far off a number may be, how likely each alternative is, which alternatives exist, and how far a vague word applies. The mechanisms below are proposed; they follow the Guide to the Expression of Uncertainty in Measurement (GUM, JCGM 100:2008) and its supplement on Monte Carlo (JCGM 101:2008), the standards that metrology uses.

| Kind | Held as | Propagated by | Answers |
| --- | --- | --- | --- |
| A bound | `interval(Low, High, Unit)` | interval arithmetic, relational as in CLP(BNR), which SWI-Prolog offers as a pack (recalled) | guaranteed bounds; wide, because intervals forget that two inputs are the same quantity |
| A measurement | `measured(Value, StandardUncertainty, Unit)`, with degrees of freedom and covariances where known | the GUM's first-order law of propagation, when the model is nearly linear | a value and its standard uncertainty, as journals expect |
| A distribution | `distribution(Kind, Parameters, Unit)`, or samples in a blob | Monte Carlo, after the GUM's supplement: a fixed number of samples from a seed kept in the journal, so a run repeats exactly | intervals and probabilities for models far from linear |
| Competing alternatives | worlds, each with a weight | weighted worlds, as probabilistic logic programming weighs facts (ProbLog; recalled) | the probability of a claim across alternatives |
| A vague term | a membership function for a predicate such as “early” or “large” | fuzzy logic: degrees of truth combined by its connectives | how far a vague claim holds; never read as a probability |

1. **One rule chooses the method.** Bounds alone give interval arithmetic; measurements through a nearly linear model give the GUM's law; anything else numerical gives Monte Carlo; discrete alternatives give weighted worlds; vague terms give fuzzy degrees. The choice is shown with each result, so the reader knows what an interval means.
2. **Correlation is kept.** Inputs that share a cause carry their covariance, or come from one sampled source, since ignoring correlation is the commonest error in propagating uncertainty.
3. **Constraints become three-valued.** A constraint over uncertain values is satisfied for every value, violated for every value, or possibly violated, the last with its probability where one is known. A possible violation shows in outline, a certain one filled, both in the tint.
4. **Charts show it.** Error bars and intervals on a common scale; a distribution as a small density curve in strokes; in an A/B comparison, the overlap of two worlds' distributions.
5. **Imports carry it.** Results from Python, R or Julia bring their uncertainties and covariances, or their samples as a blob.

Large Monte Carlo runs belong in the analysis tools, whose samples are then imported; the WAM engine and bcw-3 run small models only.

## Sources and references

Reference management needs no tool of its own: a source is an item, and what is known of it is facts. Wikidata already describes tens of millions of scholarly works (its WikiCite effort; recalled, not counted), with properties that serve as the schema. The property numbers below are recalled and not checked.

| Need | Facts |
| --- | --- |
| What the source is | Wikidata's own: DOI (P356), author (P50), title, published in (P1433), publication date (P577) |
| What cites what | cites work (P2860), between sources; `cites(Item, Source)` from the user's items |
| Whether it still stands | is retracted by (P5824), and the retraction data that Crossref took over from Retraction Watch and opened (2023, recalled) |
| The file | a PDF in `blobs/`, named by its hash, linked from the source's item |

This makes questions about sources ordinary queries: which claims rest on a single source; which cite a retracted work; which sources of a section cite each other. The constraint “a claim cites a retracted source” is the one the user story already shows. Sources the knowledge base lacks come in by DOI from Wikidata or Crossref over the network, or from a library in BibTeX or CSL-JSON, each as an import with provenance. For a paper, the memex writes CSL-JSON, which Pandoc and the common reference managers read. The translator's model also suggests references for a claim in its pane of suggestions, choosing only among works already in the store, so it cannot invent one; each suggestion shows why it was offered (see the translator document). The Wikidata subsets may leave out scholarly works to save space; which subset keeps them is an open question.

## Documents in and out

A scientist's work arrives as documents and leaves as documents: drafts from co-authors in Word or LaTeX, papers as PDF, and manuscripts, reports and theses to journals and examiners. Import and export are therefore a primary feature, not an accessory. The mechanisms below are proposed.

1. **One boundary format.** Every document crosses into the memex as Pandoc's document tree in JSON, and leaves the same way. Pandoc converts between that tree and Markdown, LaTeX, Word, OpenDocument, HTML, EPUB and JATS, the XML that journals use. The memex reads and writes only the tree: on a PC it runs Pandoc itself; bcw-3 cannot run Pandoc, so the conversion happens on another machine and a confined worker parses only the JSON.
2. **A document maps onto the outline.** Headings become outline levels; paragraphs, lists and block quotes become items; citations become `cites` facts against sources; mathematics stays as LaTeX in its own items; figures and attached files go to `blobs/`; tables become items holding rows. Comments become annotations anchored to their spans.
3. **Re-import is comparison.** When a co-author returns a revised draft, importing it starts a world beside the current one. The A/B comparison shows each change, and the user adopts or retracts them one by one; tracked changes in a Word file arrive as suggestions in the pane. This is a merge that leaves a record: who changed what, and what the user decided.
4. **Export is a view.** A section of the outline, in one world, is exported as a manuscript: Markdown or LaTeX through Pandoc, with citations written as CSL-JSON and formatted to the journal's style. An exported document records the hashes of the entries it was made from, so a published text traces back to the record.
5. **Scholarly PDF is the hard case.** A PDF holds layout, not structure. GROBID, which extracts the structure of scholarly papers (title, sections, references) as TEI XML, converts it on another machine; the result is imported with its provenance, and references are matched to works by DOI. Imperfect extraction is expected, so the import is a world to be checked before adoption.
6. **Read-only pages for colleagues.** A trail, an argument map or a section exports as plain HTML, with the hashes of its entries, for readers who do not run the memex; they can read it, and check it against a shared bundle, but not change it.

Round trips lose what the outline does not hold: page layout, fonts, and a journal's template. These belong to the export's style, not to the record.

## Mail, calendars and the web

Most of a knowledge worker's week passes in mail, meetings and the web, so these come in through the same import path as documents, each as claims with provenance. The mechanisms below are proposed.

1. **Mail** from mbox or `.eml` files: each message an item, its sender the source, its thread a trail, its attachments blobs.
2. **Calendars** from iCalendar (`.ics`) files: each event an item with its people and its date; a deadline is a fact.
3. **Web pages**, clipped as their text and an image of the page, with the address and the hash of what was fetched.
4. **Due dates are watched by the supervisor.** The memex holds no clock, so it cannot tell when a deadline nears; the supervisor, which keeps time, shows what is due and overdue in its own tile, as it shows dates, and choosing an item there moves the memex to it as a user action.

The memex does not send mail or keep a calendar: it imports them to be thought about, and leaves sending and scheduling to the tools that do them.

## Signed entries and exchanging journals

Collaborators can share the record itself, not only documents made from it. Real-time co-editing stays out: each room has one user, and the book's isolation rules (`system.no-interference`) forbid state shared between processes, so all sharing goes through export and import. The mechanisms below are proposed.

1. **Every entry is signed.** The supervisor holds each user's private key and, as it seals an entry, signs the entry's hash with Ed25519 (RFC 8032) beside the date. The memex never holds a key, so a room cannot sign for anyone. On a PC the prototype keeps the key in a file of its own. A signature says who wrote an entry; the hash chain says it is unaltered; the date says when.
2. **An entry is known by its hash.** Sequence numbers are local to a journal; across journals an entry is named by its hash, as Git names commits, so the same entry held by two people is recognised as one.
3. **A bundle carries a branch.** Exporting for a collaborator writes a bundle: the entries of chosen branches or sections, their seal records, the public keys of their authors, and the blobs they refer to. Sensitive blobs are left out, or sent encrypted to the recipient's key.
4. **Import checks, then offers a world.** On import the memex verifies every hash, chain and signature, and refuses a bundle that fails. A verified bundle arrives as a world, rooted where it shares history with the user's journal, or as a separate root where it does not. The A/B comparison shows what the collaborator changed.
5. **Adoption keeps authorship.** Adopting a collaborator's entry writes an adopt entry in the user's journal that names the original by its hash and author, so the record always shows who wrote what and who accepted it. Retracting works as for any adoption.
6. **Trust is a fact.** The keys the user trusts, and for what, are facts in the journal; a constraint flags entries signed by an unknown key.

On a PC a bundle travels as a file, by any means. On bcw-3 it arrives over the network and a confined worker parses it, as other imports do. Bundles answer asynchronous collaboration; editing together in real time is ruled out by the isolation rules, not merely missing.

## Diagrams

The memex supports diagrams first-rate, with the keyboard and mouse and no pen: pens are awkward for language, and most input here is language. Studies of diagramming by pen and by mouse conflict (Apte and Kimura 1993; Forsberg et al. 2008), so a diagram earns its quality from structure and layout, not from freehand drawing.

1. **A diagram is a view.** Its nodes are items and its edges are links, held as facts, so one structure shows as an outline or as a diagram, and queries, constraints and the dataflow graph see it unchanged. There is no separate drawing format.
2. **The memex lays it out.** Layered layout for directed graphs, trees for hierarchies, and charts for data. The user pins a node by dragging it with the mouse; the pin is a fact in the journal, and layout keeps every pinned node (see Requirements and mechanisms below).
3. **Editing is ordinary editing.** A new node is a new item, typed; a new edge is a link, made with the Command quasimode and recognition on Escape. Each edit is a journal entry, so undo, versions and side-by-side comparison of two versions apply to diagrams as to text.
4. **Diagrams are live.** A diagram over a query redraws when the facts change, and each constraint's violations show on the nodes they name.
5. **It draws with bcw-1's compositor.** Node labels come from glyph surfaces, and edges, arrowheads and chart lines are monotonic strokes, a curve split where it turns back. Emphasis is by intensity and the one tint.

### Requirements and mechanisms

The requirements come from the literature on diagrams; the mechanisms are proposed. The evidence column says how firmly each requirement stands.

| Requirement | Source | Evidence | Proposed mechanism |
| --- | --- | --- | --- |
| Minimise edge crossings; they matter far more than bends or symmetry | Purchase 1997 | strong; symmetry inconclusive | layout's first objective is fewest crossings (layered layout with median ordering of each layer), then straight paths, then bends; symmetry is not an objective |
| Keep paths continuous and straight, and nodes on them few-branched | Ware, Purchase, Colpoys and McGill 2002 | measured; weights not confirmed | long edges run straight through their layers; the path the user follows (a trail, or the main path of a flowchart) lies on one vertical line |
| Put related things together, so the reader need not search | Larkin and Simon 1987 | foundational theory | every label sits on or beside its node, with no legend; an item's annotations and constraint violations show beside it; lines join a node to its item in an outline pane |
| One symbol for each concept, easy to tell apart, suggestive of its meaning, few in number, with text beside the graphics | Moody 2009 | design theory; four of its nine principles confirmed, five from memory | a fixed set of node shapes and line styles, one for each kind of item or link, told apart by shape and line style rather than colour (bcw-1 gives 16 intensities and one tint); every symbol carries a text label |
| Manage complexity by splitting into linked diagrams | Moody 2009; DRAKON's silhouette | design theory and practice | a diagram over a size limit splits along the outline hierarchy into linked sub-diagrams, joined by Leap, at the size limits below |
| Fit the notation to the task | Moody 2009 (cognitive fit) | design theory | five diagram kinds, each with its own layout: flowchart, argument map (supports and opposes), tree, chart, and adjacency matrix |
| Charts: position on a common scale is judged most accurately, then length, angle and area, with colour last | Cleveland and McGill 1984; replicated by Heer and Bostock | strong | dot and bar charts on a common baseline; no pie charts; no quantity encoded in intensity or tint |
| Flowcharts: vertical flow, right angles only, no crossings, the main path straight down, exceptions to the right, branches ordered left to right | DRAKON | practitioner rules, untested alone; agrees with Purchase on crossings | the flowchart kind follows DRAKON's rules: main path on one vertical, worse outcomes further right, right-angle routing |
| Avoid meaning carried by layout alone, viscosity and hidden dependencies | Green and Petre 1996 | analytic framework | position carries no meaning the facts lack, so no edit forces a manual re-layout; dependencies are links, shown on request |
| Keep a changing diagram stable | Archambault and Purchase 2011–2013 | weak: helped navigation only; side-by-side views beat animation | pins and minimal change when the user navigates a diagram; versions compared side by side, not animated |

These limits bound a diagram's size; none is the often-quoted 7 ± 2, which misreads Miller.

| Limit | Value | Source | Mechanism |
| --- | --- | --- | --- |
| Local: elements related in one reading step | about 4 chunks (3–5) | Cowan 2001, 2010: the span of working memory | along the path a reader follows, a node branches to at most about 4; more branches are grouped under one node that opens on request |
| Whole diagram, node-link | about 50 nodes when dense, about 100 when sparse | Yoghourdjian et al. 2020: shortest-path tasks, 25–175 nodes, with EEG | a diagram over the limit splits into linked sub-diagrams; the limits are starting values, to be measured on the memex's own diagrams |
| Whole diagram, other tasks | above about 20 nodes, an adjacency matrix beats a node-link diagram on most tasks except finding paths | Ghoniem, Fekete and Castagliola 2004; weaker at 50 nodes in a replication | a large graph read for anything but paths can also be shown as an adjacency matrix |

Time and errors understate load: the same performance can cost different mental effort (Huang, Eades and Hong 2009), so a measurement of these limits records effort as well. Sources: [Cowan 2001](https://memory.psych.missouri.edu/assets/doc/articles/2001/cowan-bbs-2001.pdf), [Cowan 2010](https://pmc.ncbi.nlm.nih.gov/articles/PMC2864034), [Yoghourdjian et al. 2020](https://arxiv.org/pdf/2008.07944), [Ghoniem et al. 2004](https://ieeexplore.ieee.org/document/1382886), [Huang, Eades and Hong 2009](https://dblp.org/rec/journals/ivs/HuangEH09).

Sources: [Purchase 1997](https://link.springer.com/doi/10.1007/3-540-63938-1_67), [Ware et al. 2002](https://research.monash.edu/en/publications/cognitive-measurements-of-graph-aesthetics/), [Larkin and Simon 1987](https://mechanism.ucsd.edu/bill/teaching/F12/cs200/Readings/larkin.whyadiagramissometimesworth.1987.pdf), [Moody 2009](https://ieeexplore.ieee.org/document/5353439), [Cleveland and McGill, summary](https://www.cs.ubc.ca/~tmm/courses/533-09/slides/percep.pdf), [DRAKON rules](https://drakon-editor.sourceforge.net/language.html), [Green and Petre 1996](https://www.academia.edu/3050347/Usability_Analysis_of_Visual_Programming_Environments_a_cognitive_dimensions_framework), [Archambault and Purchase 2011](https://research.monash.edu/en/publications/animation-small-multiples-and-the-effect-of-mental-map-preservati/), [Archambault and Purchase, navigation](https://eprints.gla.ac.uk/66915).

## The outline model

The store is an explicit outline that the user moves through and edits by its parts, as in NLS (N1, N4, N7), with onscreen aids that address NLS's known weakness: it "was not designed to be easy to learn", made "heavy use of program modes" and used "cryptic mnemonic codes" (Wikipedia, NLS).

| Part | Adopted from | How it works here |
| --- | --- | --- |
| Explicit hierarchy | N1 | Text is held as statements at outline levels; the structure is data, not formatting. NLS found that "after an initial period of negative reaction … one comes to prefer it". |
| Structural navigation | N4 | Jump to the successor, predecessor, head or tail of a list, the statement above, or the first one below. |
| Editing by entity | N7 | Delete, Insert, Replace, Move and Copy act on a character, word, statement, branch or group, chosen as the target by a quasimode, then named by a command (below); "DW" survives as an abbreviation that recognition completes. |

Onscreen aids to learning (this design's own proposals, not tested):

1. Visible structure: indentation and level marks always shown, with names and statement boundaries on request, as a view.
2. Recognition, after TOPS-20: Escape completes a command or name as far as it is unambiguous, so "DW" becomes "Delete Word" on screen; full names are accepted too.
3. A command bar for the current selection, listing the operations valid for its kind of entity, each with its short form, so the bar teaches the shortcuts.
4. No modes at all, only quasimodes (below): the selected target and any command in progress are always on screen.
5. A preview before every operation: its target is highlighted, and a Move or Copy shows its destination.
6. Safe experiment: every edit is a journal entry, so any of them can be undone by returning to an earlier entry.
7. Navigation hints: the structural neighbours of the current statement are labelled with their keys.
8. Hints that fade: aids the user no longer needs grow quieter, so learning does not cost expert speed.
9. Guide words, after TOPS-20: a completed field is followed by a parenthesised noise word naming the next argument, as in "Copy (from) report (to) archive". Each guide word names one slot of the verb's frame, so the fixed argument order is shown, not memorised.
10. Help on "?", after TOPS-20: typing "?" anywhere in a command, or in controlled English, lists what may come next with a line of help, without losing what was typed. It is the predictive editor, on demand.

TOPS-20's command parser (the COMND monitor call) typed guide words in parentheses after an Escape, filled in a default, and answered "?" with a help string or the list of possible completions ([COMND guide](https://pdp-10.trailing-edge.com/decuslib20-01/01/decus/20-0002/comnd.mss.html), [Pascal COMND interface](https://pdp-10.trailing-edge.com/decuslib10-07/01/43,50433/pascmd.mac.html), [question marks help](https://pdp-10.trailing-edge.com/BB-PBQUC-BM_1990/01/help/question-marks.hlp.html), [recognition help](https://pdp-10.trailing-edge.com/BB-PBQUC-BM_1990/01/help/recognition.hlp.html); taken from search excerpts of these archived pages).

### Quasimodes, not modes

The interface has no modes, only quasimodes, after Jef Raskin: a state that lasts "only through some constant action", such as a held key, so the user "cannot forget that they are in that mode". Raskin called modes "a significant source of errors, confusion, unnecessary restrictions, and complexity". Quasimodes choose the target of a command; the command then acts on it, noun before verb.

| Quasimode | While held | On release | Origin |
| --- | --- | --- | --- |
| Leap forward, Leap back | type a pattern; the cursor moves to the next or previous match as each letter is typed | the cursor stays; leaping from one end of a span to the other selects it | Raskin's LEAP, meant to "turn cursor positioning into a reflex" |
| Scope | each tap widens the target by structure: word, statement, branch, group; the target is highlighted | the target stays selected | this design, replacing NLS's entity letters (N7) |
| Command | type a command name, with recognition, guide words and help on "?"; the effect is previewed on the target | the command runs on the target, as one journal entry that can be undone | Raskin's command quasimode, with TOPS-20's aids |
| Suppose | commands act in a sprouted world, not in reality | the differences are shown; a Commit command makes them real, and anything else sets the world aside, kept in the journal so that it can be reopened | this design: the shell's dry run and counterfactuals |

Raskin's further rules, adopted:

- Monotony: one command for each atomic task, so that it becomes habit. An abbreviation and the full name are the same command, which recognition shows identically. Leap moves by content and the structural keys by structure, so they serve different tasks.
- No Save and universal undo: every action is a journal entry, so nothing is lost and no warning dialogs are needed. Archy had the same: "This eliminates the need for, and the concept of, saving a document."
- A command applies wherever its target is, with no separate applications.

Limits:

- Holding Leap while typing "has been criticized as uncomfortable" (Wikipedia, Archy); the held keys should fall under the thumbs, as on the Canon Cat's two large Leap keys.
- Quasimodes need key-down and key-up events. bcw-1's USB keyboard input, at boot protocol, reports the keys held, so presses and releases are known (`13_HID.md`).
- The zooming interface of THE and Archy is not adopted; Archy's prototype "did not include code for zooming".

Conventional shortcuts sit beside the quasimodes, for laptops without thumb keys and for accessibility: each quasimode can be latched by tapping its key instead of holding it, and a command palette offers every command by name with recognition. Holding remains the fast path; nothing needs it.

Sources: [The Humane Interface](https://en.wikipedia.org/wiki/The_Humane_Interface), [Archy](<https://en.wikipedia.org/wiki/Archy_(software)>), [Mode (user interface)](<https://en.wikipedia.org/wiki/Mode_(user_interface)>).

## Storage

Bulk storage is bcw-1's SPI flash, its boot medium (`12_STORAGE.md` §2.1), so it is present from step 1. It holds the journal and the items; its capacity is open, and SPI flash is small. A process reaches external memory only through the mover and bulk storage only through the storage accelerator, and a slow device costs more turns still, so what the user is working on stays in the 16 MB hot layer. The knowledge base lives on the WAM engine, not here.

*Diagram, kept in the living doc only: storage tiers: pool, external memory, SPI flash, WAM engine.*

- Indexes and link tables are B-trees or sorted runs in whole blocks: a search costs one or two reads of bulk storage after the cached top levels.
- The journal is append-only, so writes are sequential, nothing is overwritten, and old versions stay citable at no cost.
- Each room gets its own area of bulk storage, through its own capability, so one room's data or access pattern cannot reach another's answers (`system.answers`).
- Opening an item or following a trail loads its neighbourhood (linked items, annotations) into the hot layer.
- Plain text is small: 100,000 pages of about 5 KB is about 0.5 GB, plus an index of about 30–100%.

## Mechanisms considered

Every candidate from NLS/Augment, Xanadu, notebook environments and live simulation has been settled. Adopted: X3, X4, N2, N8 and J4 under The journal; L1–L6, with J1, J2 and J6, under Dataflow; N3, N5, N6, X1, X2, X5, J3 and J5 under Views, references and anchors; N1, N4 and N7 under The outline model. J7 is implicit in bcw-3's treatment of hostile input. N9, N10 and X6 were dropped. The sources are listed below.

| ID | Mechanism | Source | Outcome |
| --- | --- | --- | --- |
| N1 | Explicit statement hierarchy | NLS | adopted: The outline model |
| N2 | Named statements and links | NLS | adopted: The journal |
| N3 | Markers | NLS | adopted: anchors |
| N4 | Structural navigation | NLS | adopted: The outline model |
| N5 | View specs | NLS | adopted: views |
| N6 | Freezing | NLS | adopted: views |
| N7 | Editing by entity | NLS | adopted: The outline model |
| N8 | Who changed what, and when | NLS | adopted: The journal |
| N9 | Bootstrapping | NLS | dropped |
| N10 | Shared-screen conferencing | NLS | dropped |
| X1 | Transclusion | Xanadu | adopted: references |
| X2 | Links that cannot break | Xanadu | adopted: anchors |
| X3 | Permanent addresses for every span | Xanadu | adopted: The journal |
| X4 | Every version kept, with branches | Xanadu | adopted: The journal |
| X5 | Side-by-side versions | Xanadu | adopted: views |
| X6 | Transcopyright | Xanadu | dropped |
| J1 | Cells mixing prose, code and output | JupyterLab | adopted: Dataflow, reactive documents |
| J2 | Document separate from the engine | JupyterLab | adopted: Dataflow, reactive documents |
| J3 | Synchronised views and mirrored output | JupyterLab | adopted: views and references |
| J4 | A log of computation | JupyterLab | adopted: The journal |
| J5 | Saved workspaces | JupyterLab | adopted: workspaces |
| J6 | Code blocks as functions | Org-mode Babel | adopted: references; Dataflow |
| J7 | Untrusted output until trusted | JupyterLab | implicit in bcw-3 |
| L1 | Recomputation by dependency | Excel, Observable | adopted: Dataflow |
| L2 | Reactive documents | Bret Victor | adopted: Dataflow |
| L3 | Up and down the ladder of abstraction | Bret Victor | adopted: Dataflow |
| L4 | Parameters as controls | Mathematica | adopted: Dataflow |
| L5 | Named scenarios compared | Excel | adopted: Dataflow |
| L6 | Response-time limits | Nielsen | adopted: Response time |

Sources: [NLS (Wikipedia)](<https://en.wikipedia.org/wiki/NLS_(computer_system)>), [The Mother of All Demos](https://en.wikipedia.org/wiki/The_Mother_of_All_Demos), [About NLS/Augment](https://www.dougengelbart.org/content/view/155/), [Engelbart 1968 paper](http://dougengelbart.org/content/view/140/), [Augmenting Human Intellect](https://www.dougengelbart.org/content/view/138/), [Project Xanadu](https://en.wikipedia.org/wiki/Project_Xanadu), [Transclusion](https://en.wikipedia.org/wiki/Transclusion), [Xanalogical structure](https://xanadu.com.au/ted/XUsurvey/xuDation.html), [Transcopyright](https://xanadu.com/xuTco.html), [Enfilade](<https://en.wikipedia.org/wiki/Enfilade_(Xanadu)>), [JupyterLab overview](https://jupyterlab.readthedocs.io/en/latest/getting_started/overview.html), [JupyterLab notebooks](https://jupyterlab.readthedocs.io/en/latest/user/notebook.html), [JupyterLab documents and kernels](https://jupyterlab.readthedocs.io/en/latest/user/documents_kernels.html), [Project Jupyter](https://en.wikipedia.org/wiki/Project_Jupyter), [Org-mode Babel](https://orgmode.org/worg/org-contrib/babel/intro.html).

## Order of work

1. Build the memex as software on bcw-3, in worker regions with the 16 MB external memory as a staging store and the journal on SPI flash: the journal, the outline with its quasimodes, links, search, views, panes, trails and annotation. Navigation, editing and following links run in native code within 0.1 s; Prolog filters, queries and the dataflow graph over them wait for step 2.
2. Add the WAM engine (see Glossary), so that Prolog runs, and live updates, queries, parses and restatements meet their limits (see Timing estimates).
3. Add "do you mean?" without a model: the predictive editor, APE's repair hints, and a memory of confirmed pairs.
4. Add the model accelerator last, as a fallback for free English that the memory cannot match, if logged use shows such input is common.

SPI flash, bcw-1's boot medium, is present from step 1 and holds the journal; its capacity is an open question.

## Parallel paths

Steps 1 and 2 of the order of work run as four parallel paths. They meet at one interface, the query protocol below, which is fixed first.

| Path | What it delivers | Depends on | Main risk |
| --- | --- | --- | --- |
| 1. The WAM engine | the board, the core, storage, the link and replay | the protocol; the engine's instruction set and built-ins | its size: it is a machine of its own |
| 2. The memex on a PC, over SWI-Prolog | the memex in use early: journal, outline, quasimodes, dataflow, s(CASP); its client and its Prolog server split at the protocol | the protocol; the engine's subset of Prolog | SWI-only features (tabling, incremental tabling, CLP(Q), strings, garbage collection) turn path 3 into a rewrite; timings on a PC say nothing of bcw-3 |
| 3. Porting the memex to the WAM engine | path 2's Prolog running on path 1 | paths 1 and 2 | attributed variables for s(CASP); incremental tabling, on which the dataflow design relies |
| 4. bcw-3 user interface | the outline, panes, glyphs and strokes on bcw-1's compositor, within a 48 kB region | bcw-1's peripherals on bcw-3; the protocol, for queries | the region's size; 16 intensities and one tint for all highlighting |

Path 2's client stands in for path 4, and its SWI-Prolog server for path 1, so each half can be replaced on its own. Path 2 is also the version for others: a PC is what other people and their organisations can run, while bcw-3 is the author's own instrument. One rule binds path 2: its Prolog uses only the engine's subset, written down as the subset grows.

### Estimate for path 2

These are estimates by judgement, not measurements. They assume a browser client talking to SWI-Prolog over the query protocol, APE and s(CASP) as they are, no trained model, and one experienced developer; the second column has Claude writing the code while the author directs and reviews.

| Part | Weeks, by hand | Weeks, with Claude | Why |
| --- | --- | --- | --- |
| Query protocol, journal, panes, count of actions, emulation of bcw-1's display | 6–10 | 1–2 | well specified and routine |
| Outline editor: items, transclusion, links | 3–5 | 1–2 | editing behaviour needs many rounds of trial |
| Quasimodes: Leap, Command with noise words, recognition, ?, Scope, Suppose | 2–3 | 1–2 | their feel is judged by hand; browsers report key releases, but the system takes some keys |
| Dataflow, constraints and worlds on SWI's incremental tabling | 4–8 | 2–4 | the most uncertain part: incremental tabling is unchecked, and s(CASP) is not tabled |
| Diagrams: layered layout, DRAKON flowcharts, argument maps, matrices, charts, pins | 4–8 | 2–3 | layout code compresses; DRAKON's routing and the look need review |
| “Do you mean?” through APE's restatement and Clex | 1–2 | 0.5–1 | APE already runs |
| Undo tree, retraction and the history pane | 2–4 | 1–2 | the journal already holds the entries; the pane follows the rules for diagrams |
| A/B comparison | 3–5 | 1–2 | linked panes, superposition and the difference table |
| Importing results with provenance | 2–3 | 1–2 | helpers for Python, R and Julia, and the reader |
| Hash chains, dates and the open journal format | 1–2 | 0.5–1 | standard parts: SHA-256, canonical JSON, JSON Lines |
| Documents in and out through Pandoc's tree | 2–4 | 1–2 | mapping the tree to the outline both ways; re-import reuses A/B comparison |
| Uncertainty: intervals, GUM propagation, Monte Carlo, weighted worlds, fuzzy degrees | 3–6 | 1–3 | existing libraries for intervals and sampling; the choice of method and the three-valued constraints are new |
| Signed entries and journal exchange | 2–4 | 1–2 | standard signatures; bundles reuse import, worlds and A/B comparison |
| Integration and daily use | 2–4 | 1–2 | the author's use sets the pace |
| **Total** | **36–68** | **13–28** | overlapping parts shorten the second column |

A scripted demonstration of the user story's four scenes takes about 4–6 weeks by hand, or 1–2 with Claude. Published evidence on speed-ups from AI is mixed (recalled, not checked): about 56% faster on one small, well-defined task with Copilot (Peng et al. 2023), and about 19% slower for experienced developers in large codebases they knew (METR 2025). This prototype is new code with a written design, nearer the first case, so the second column assumes the gain falls mainly on writing code, not on the loops of design and feel. The prototype answers whether the interaction works and how much of SWI-Prolog the memex uses; it says nothing of timing on bcw-3 or the WAM engine.

## Query protocol

Text, Prolog terms written and read as ASCII, costs no bandwidth that matters: the 400 MB/s link carries a 100-byte query in 0.25 µs. Its cost is code: a term reader and writer, with operators, quoting and escapes, both in a 48 kB region on bcw-3 and in the engine. The protocol sends terms as the engine's own 32-bit tagged cells, in prefix order, so each side decodes in one pass of a small loop and the engine copies the cells straight into its heap. Erlang's external term format and SWI-Prolog's `fast_term_serialized` apply the same idea in bytes (recalled, not checked).

A message is a header word, a request number, and cells. The header holds the type (8 bits), the room (8 bits) and the length in words (16 bits). bcw-3's end of the link writes the room, not the process, so no process can name another room.

| Cell tag (3 bits) | Value (29 bits) | Followed by |
| --- | --- | --- |
| VAR | the variable's number within the message, from 0 | nothing |
| INT | a signed integer | nothing |
| ATOM | the atom's number | nothing |
| STRUCT | arity (8 bits) and the functor's atom number (21 bits) | its arguments, in prefix order |
| BLOB | length in bytes | the bytes, four to a word: strings, large integers, floats |

Lists are `'.'/2` structures, and three tags are spare. `supports(X, item42)` takes 3 cells (12 bytes) against 19 bytes of text, so the saving is in code, not bandwidth.

| Message | Direction | Body |
| --- | --- | --- |
| QUERY | to the engine | the goal |
| NEXT | to the engine | n: send up to n more answers |
| CANCEL | to the engine | nothing; the engine drops the query's choice points |
| ASSERT, RETRACT | to the engine | the fact, and its journal sequence number |
| ATOM | to the engine | a name, as a BLOB |
| JOURNAL | to the engine | journal entries, answering FETCH |
| ANSWER | to bcw-3 | one term, with one argument for each VAR of the query |
| DONE | to bcw-3 | no more answers, failure, or an error term |
| ATOM\_IS | to bcw-3 | the atom's number |
| FETCH | to bcw-3 | a journal sequence number: send the entries after it |

Answers come only when NEXT asks for them, so the engine backtracks lazily and no buffer on either side grows without bound. Atom numbers are kept per room, so the number an atom gets tells one room nothing of what another has interned. Path 2 sends the same messages over a pipe between its client and SWI-Prolog, with an encoder and decoder in Prolog of an estimated 100–200 lines.

## Decisions

| Decision | Reason |
| --- | --- |
| Name the project Adversaria, with the prototype in a repository of its own | the author's decision; “memex” is taken, and the prototype shares no code, tools or audience with bcw-3 |
| Measure the falsifiability of formal claims: compatible, independent, refutable, characteristic number, needed premises, comparative strength, immunisation; relative to a declared observation vocabulary | the author's decision; Popper's measures, carried from Lean to Prolog through s(CASP)'s abduction |
| Test claims by the cheapest check that could refute them: quote, source, provenance, weakest link, open parameters, independence, gaps shown, planted faults | the author's decision, adopting what transfers from the Popper project's judges of paper claims |
| Imported content is a claim, never a fact; adopting and interrogating a claim take one click or key. The PC prototype is the version for others; purges keep the chain checkable; mail, calendars and the web are imported; quasimodes can be latched and a command palette offers every command; trails and maps export as read-only HTML | the author's decisions, after an outside review of the design's fit for general knowledge workers |
| Entries are signed by the supervisor with the author's key; journals are exchanged as verified bundles that arrive as worlds | the author's decision; collaborators share the record itself, with authorship kept, while the isolation rules still forbid shared state |
| Support uncertainty: intervals, the GUM's propagation, Monte Carlo, weighted worlds and fuzzy degrees, kept apart and chosen by one rule | the author's decision; a measured value without its uncertainty is half a fact |
| Import and export of documents are a primary feature, through Pandoc's document tree; re-import is A/B comparison | the author's decision; a scientist's work arrives and leaves as documents |
| The model suggests references, chosen only from works in the store | the author's decision; finding sources is clerical work, and choosing among existing works rules out invented references |
| References are facts: a source is an item described by Wikidata's properties; import from BibTeX and CSL-JSON, export to CSL-JSON | the author's decision; no separate reference manager, and questions about sources become queries and constraints |
| The journal's format is open: JSON Lines with canonical JSON (RFC 8785) hashed by SHA-256, a separate log of seals (dates and signatures) by the supervisor, and blobs named by their hashes | the author's decision; a scientific record must outlive the tools that made it |
| The supervisor keeps the dates; the memex never reads them | the author's decision; it keeps `system.no-clock` without an exception, while the record is still dated |
| Journal entries are hash-chained and dated by the supervisor | the author's decision; a scientific record must be unaltered and dated, and the memex itself holds no clock |
| Import results from analysis tools as facts with provenance | the author's decision; the memex sits beside the scientist's analysis tools, not in their place |
| The interface supports A/B comparison of worlds: juxtaposed, superposed, and as a table of differences | the author's decision; science compares states that differ in one thing, and worlds already hold those states |
| Retract, don't rewind: an earlier decision is reversed by a new entry on the current branch; branches come only from Suppose and from undoing then editing | the author's decision, after the literature on selective undo; it keeps most work on one branch and adds no concept, since retracting generalises unadopting |
| A world is a branch of the undo tree; Suppose names one | the author's decision: undoing is a decision too, so one concept of alternative state serves both |
| A history pane shows the undo tree over the journal | the author's decision; the user tracks, searches and reverses decisions, adoptions above all |
| Undo and redo form a tree over the journal; adoptions of suggestions are marked and searchable | the author's decision; it limits the risk of adopting without checking, and keeps the user's decisions traceable and searchable |
| The model runs all the time and offers a pane of suggestions, adopted and unadopted by a click | the author's decision; adoption is a journal entry, so undo, worlds and dataflow apply unchanged |
| The translator's model also suggests facts, rules and constraints for the user to accept | the author's decision; the main risk is that the user will not write facts and constraints, and Wikidata covers only the public world |
| First-rate support for diagrams, with keyboard and mouse; no pen | the author's decision; pens are awkward for language input, and a diagram as a view over items and links reuses the journal, queries and dataflow |
| Count the user's actions by kind from the journal; run constraints as dataflow nodes whose values are their violations | the author's decision, after Licklider's "Man-Computer Symbiosis" (1960): measure where the work goes, and let the machine expose flaws during formulation |
| Adopt the query protocol: terms as the engine's 32-bit tagged cells in prefix order, answers on demand, rooms written by bcw-3's end of the link, atom numbers per room | the author's decision; one decoding loop on each side instead of a term reader and writer in a 48 kB region |
| Steps 1 and 2 run as four parallel paths: the WAM engine; the memex on a PC over SWI-Prolog; the port to the engine; bcw-3's user interface | the author's decision; the memex is in use before the engine exists, and the paths meet at the query protocol |
| Buffer index updates in DDR3 and apply them lazily | the author's decision; consultations are expected to outnumber changes by far (a guess of 1,000–100,000 to 1, to be measured), and a card's write pauses must not delay reads |
| Index every fact on the index card in three orderings (SPO, POS, OSP) | the author's decision to index every pattern of bound arguments; three orderings cover all seven patterns of a triple by prefix, so each lookup's worst case is bounded, and they fill a small part of the 512 GB card |
| The WAM engine's SD controllers support the card's command queue and cache | the author's decision; A2's random-read rate depends on them (recalled, not checked) |
| The WAM engine carries 1 GB of DDR3 and two 512 GB A2-class SD cards, one for the index and one for the data, each with its own controller | the author's decision; the knowledge base and its indexes live there. An SD card replaced an NVMe SSD, whose PCIe root port and NVMe host would take an estimated 12–30% of the FPGA, need the transceiver variant, and are closed vendor IP |
| The WAM engine joins bcw-3 by a dedicated 32-bit link at 100 MHz | the author's decision; queries do not share the 100BASE-TX network |
| Prolog runs only on the WAM engine, a second ECP5-85F (see Glossary) | bcw-1 requires a process to fit its region whole, and no worker addresses external memory; the knowledge base outgrows SPI flash |
| Target bcw-1's peripherals: 16 MB static RAM, the beam-racing compositor and mover, SPI flash storage, USB input, 100BASE-TX | the author's decision; bcw-1 states the same purpose, a cognitive augment |
| Adopt J5: workspaces as items in the journal | a layout is just another view of the store, so the journal keeps it with no Save |
| No modes: quasimodes (Leap, Scope, Command, Suppose) choose the target, then a command acts on it | a held key cannot be forgotten, so mode errors cannot occur; after Raskin |
| Add TOPS-20's recognition on Escape, guide words, and help on "?" to the outline model's aids | they teach commands and argument order in place; guide words name the slots of the verb frames |
| Adopt N1, N4 and N7 as the outline model, with onscreen aids to learning | NLS's structure gave experts speed; the aids address its known difficulty of learning |
| Adopt views, references and anchors (merging N3, N5, N6, X1, X2, X5, J3); merge executable content (J1, J2, J6) into reactive documents; drop N9, N10, X6; J7 is implicit | each group did one job in several systems; the dropped ones concern practice, collaboration or licensing, not mechanism |
| Move the WAM engine to step 2 of the order of work | live interaction (L6) needs it; software on bcw-3 cannot meet 0.1 s |
| Adopt L6: response-time limits of 0.1 s, 1 s and 10 s on the dataflow graph | live interaction depends on them; they set what the WAM engine must achieve |
| Adopt dataflow over the journal, merging L1–L5 | five candidates are one mechanism seen from different sides |
| Adopt the mechanisms an append-only journal gives almost free: X3, X4, N2, N8, J4 | one rule (nothing overwritten) yields all five |
| bcw-3's bulk storage is bcw-1's SPI flash, holding the journal from step 1 | it is bcw-1's boot medium, so it is present from the start; it is small, so the knowledge base lives on the WAM engine |
| The memex comes first, as software on bcw-3 | most of the augmentation needs no new hardware |
| "Do you mean?" starts without a model: predictive editor, repair hints, memory of pairs | they teach the controlled English, and cost almost nothing |
| The model accelerator is a late fallback | its value depends on how much free English the cheaper methods miss |
| The translator is English ↔ Prolog only | Prolog runs on the WAM engine; Lean runs on neither machine |
| "What if" works by counterfactual revision in Worlds, with s(CASP) run with `--forall=sasp` | its default algorithm has an open bug, s(CASP) issue #13 (github.com/SWI-Prolog/sCASP/issues/13), reproduced in the translator document's trial |

## Open questions

- [ ] Which SPI flash device, of what capacity and read rate? It sets how large the store can be, and how many lookups a search can afford.
- [ ] How fast does the journal grow in real use, and how are span lists compacted?
- [ ] How is the bulk storage partitioned between rooms, so that one room's answers depend only on its own calls (`system.answers`)?
- [ ] Does SWI-Prolog's incremental tabling work as recalled, and how do s(CASP)'s untabled queries join the dataflow graph?
- [ ] At what granularity are dependencies recorded (fact, predicate, item), and what does recording cost?
- [ ] Which keys of a USB keyboard fall under the thumbs for Leap, Scope, Command and Suppose? (bcw-1's boot-protocol input reports the keys held, so presses and releases are known.)
- [ ] Which parts enter the book, its proofs and its audit: the WAM engine, APE, s(CASP), the memex software? APE and s(CASP) alone are about 39K lines of Prolog (measured), about 49 audit days against the 40-day budget (`design.audit-budget`).
- [ ] What does command queue and cache support add to each SD controller's logic, and how many random reads a second does an A2-class card then give? The class promises at least 4,000 IOPS; measured cards fell short, perhaps because their hosts did not use the queue.
- [ ] The WAM engine serves every room. For a room's answers to depend only on its own calls, and its rate on nothing another room does (`system.answers`, `system.no-timing-channel`), the engine needs per-room partitions of facts, worlds and the write buffer, and link time in fixed slots per room. Or does the engine serve one room only?
- [ ] How large may the WAM engine's write buffer grow before it forces write-back, and how long can a forced write-back delay reads?
- [ ] Which SD bus mode does the engine drive: 3.3 V high speed at about 25 MB/s, or UHS-I at 1.8 V? A2's promised rates may depend on it.
- [ ] How does the link frame messages in fixed time slots for each room?
- [ ] Where does diagram layout run, in a worker region on bcw-3 or on the WAM engine, and how large a diagram must it lay out within 0.1 s? How many strokes on one scanline can bcw-1's compositor sustain (its span-less fetch is still open)?
- [ ] Which timestamping service, how often, and over which link does the head hash leave bcw-3?
