# Log of measured results

Each entry says what was built, what was measured, and what failed or was skipped.

## Milestone 0: toolchain and spikes (12 November 2026, done)

### Tool versions (measured)

| Tool | Needed | Found | How |
| --- | --- | --- | --- |
| SWI-Prolog | 9 or later, with `library(http/websocket)` and incremental tabling | 9.0.4 | installed in this session from Ubuntu 24.04's `swi-prolog-nox` package; the websocket library loads; tabling checked below |
| Python | 3.11 or later | 3.13.18 | already installed |
| Node.js | 20 or later | 22.23.4 | already installed |
| npm | (with Node.js) | 10.9.9 | already installed |
| Pandoc | 3 | 3.1.3 | already installed |
| Git | any | 2.43.0 | already installed |
| GNU Make | (for `make test`) | 4.3 | already installed |
| s(CASP) | optional | 1.1.4 (tag `V1.1.4`, commit 51ae396) | cloned from GitHub with the author's permission; loaded from the checkout's `prolog/` directory, because `pack_install` contacts the pack server, which answered `426 Upgrade Required` |
| APE with Clex | optional | not installed | not attempted; waits for the licence decision |

The session runs in a cloud container that is rebuilt from scratch. SWI-Prolog is not in its base image, so each new session must install it again until the environment's setup script does so.

### Spike: incremental tabling (measured, passed)

Test: `tests/spikes/incremental_tabling.pl`, run by `make test`. It tables four predicates over incremental dynamic facts, counts each evaluation of a tabled body, changes one fact at a time, and checks both the answers and the counts.

Output:

```
first query             p=[1,2] q=[10,20] s=[2] r(k1)=[1] r(k2)=[2]
                        evals: p=1 q=1 s=1 r(k1)=1 r(k2)=1
repeat query            p=[1,2] q=[10,20] s=[2] r(k1)=[1] r(k2)=[2]
                        evals: p=1 q=1 s=1 r(k1)=1 r(k2)=1
assertz(a(3))           p=[1,2,3] q=[10,20] s=[2,3] r(k1)=[1] r(k2)=[2]
                        evals: p=2 q=1 s=2 r(k1)=1 r(k2)=1
retract(b(10))          p=[1,2,3] q=[20] s=[2,3] r(k1)=[1] r(k2)=[2]
                        evals: p=2 q=2 s=2 r(k1)=1 r(k2)=1
assertz(c(k1,9))        p=[1,2,3] q=[20] s=[2,3] r(k1)=[1,9] r(k2)=[2]
                        evals: p=2 q=2 s=2 r(k1)=2 r(k2)=1
answers correct; only invalidated tables recomputed
```

Findings:

1. A repeated query recomputes nothing.
2. A change to `a/1` re-evaluates `p` and `s` (which reads `p`), and not `q`. Dependencies through another tabled predicate are followed.
3. Invalidation is finer than the predicate. `r(k1, _)` and `r(k2, _)` are separate tables over the same predicate `c/2`; adding `c(k1, 9)` re-evaluated only `r(k1, _)`.
4. A table that called `c(_, nothing)` was not invalidated by adding `c(k1, 9)`. This suggests that SWI-Prolog invalidates by whether the changed clause unifies with the call a table made. It is one observation, not a tested rule.

So the design's assumption in § Dataflow holds for these cases. This answers part of the open question on granularity: dependencies can be recorded per call variant, with no code of ours. It does not measure the cost of recording them; that waits for milestone 4.

Checks of the test itself (break it on purpose):

* With `as incremental` removed from the tables, the answers go stale and the test fails: `FAILED: answers(p,[1,2,3])`, exit 1.
* With `r/2` made to depend on every `c/2` fact (`once(c(_, _))` added to its body), the change re-evaluates `r(k2, _)` and the test fails: `FAILED: ev(r_evals_k2,1)`, exit 1.

A fault in the first draft of the test, found and fixed: `flag/3` keys a compound term by its name and arity only, so `r_evals(k1)` and `r_evals(k2)` shared one counter. The keys are now atoms.

### Spike: s(CASP) and issue #13 (measured, reproduced)

Test: `tests/spikes/scasp_forall.pl`, run by `make spike-scasp SCASP=<checkout>`. It is not part of `make test`, because s(CASP) is optional before stage B. The program is the case from `docs/translator.md`: `bad :- p(X, Y), q(X, Y).` with the facts `p(a,x)`, `p(b,y)` and `q(a,x)`. `bad` holds, so `?- not bad.` must have no model.

| `--forall=` | Result | Correct? |
| --- | --- | --- |
| `all` (the default) | a model, resting on `not p(a,y)` and `not q(b,y)` | no: the bug |
| `all_c` | the same model | no |
| `prev` | No models (0.008 s) | yes |
| `sasp` | No models (0.009 s) | yes |

The default algorithm checks `not p(a,y)` and not every value of Y, as the translator document says. The workaround `--forall=sasp` works on SWI-Prolog 9.0.4.

Output of `make spike-scasp`:

```
--forall=all: not bad -> model (expected model)
--forall=sasp: not bad -> no_model (expected no_model)
```

The check fails (exit 1) when `q(a, x)` is removed, because `sasp` then finds a model. It will also fail if a later s(CASP) fixes the default algorithm; that is meant, since the workaround could then go.

A fault in my first run, found and fixed: without `-s1` and with standard input open, s(CASP) printed its first answer and waited for input, so `all` and `all_c` looked like a 60-second hang.

### Disagreement between the plan and the design

The design (§ Query protocol, last paragraph) says the prototype sends the protocol's messages "over a pipe between its client and SWI-Prolog". The plan (milestone 2) says "over a websocket", and asks for SWI-Prolog's websocket library. A browser client cannot open a pipe, so the plan's choice looks right. Resolved by the author on 12 November 2026: a websocket. `docs/design.md` now says so.

### Decisions by the author (12 November 2026)

1. **Licence:** GPL-3.0. It allows APE and Clex to ship with the project.
2. **Numbers in hashed records:** JSON numbers, with RFC 8785's serialisation in full; the plan's proposal of decimal strings is not taken. Milestone 1 must implement RFC 8785's number rules in the server and in the checker, and test them.
3. **Wikidata subset:** D, the pocket base (about 100K items × 20 facts, about 63 MB; an estimate).
4. **s(CASP):** installed from source, with permission; spike run above.

### The Nix environment (measured, 12 November 2026)

`flake.nix` gives a dev shell with every tool the plan names, pinned by `flake.lock`: nixpkgs from the `nixos-26.05` channel (revision `ac588ef1c914`), and s(CASP) at tag `V1.1.4` (commit `51ae396`), exported as `SCASP_HOME`. nixpkgs comes from `channels.nixos.org`, not GitHub, because this session's network refused GitHub's API and tarball hosts (HTTP 403).

Inside `nix develop -i` (no tools from outside Nix):

| Tool | Version |
| --- | --- |
| SWI-Prolog | 9.2.9 (websocket library loads) |
| Python | 3.13.17 |
| Node.js / npm | 22.23.4 / 10.9.9 |
| Pandoc | 3.7.0.2 |
| Git | 2.54.0 |
| GNU Make | 4.4.1 |

`make test` exits 0 (spike and 22 tests); `make spike-scasp` exits 0. `nix flake check` passes on x86_64-linux; the other three systems are declared but not checked.

Costs and faults met:

* SWI-Prolog's closure is 1.3 GiB, mostly Java and GTK, which we do not use. nixpkgs offers `withJava` and other switches, but a changed build is not in the binary cache, so it would compile from source. Left as is.
* A first version fetched s(CASP) with `fetchgit` inside the build sandbox; it failed on this session's TLS proxy (self-signed certificate in chain). As a flake input, Nix fetches it outside the sandbox, as it does nixpkgs.
* s(CASP)'s own Makefile ran during the build and needed `swipl`; the build step is skipped, since only the source is copied.

## Change of plans (12 November 2026)

The author changed the prototype's form. Recorded in `docs/design.md` (§ The PC prototype, § Query protocol, § Quasimodes, not modes, § Decisions) and `docs/plan.md`.

* **A terminal program in Rust**, drawn with ratatui on crossterm. No web interface. This replaces the browser client, and the websocket chosen earlier the same day; the query protocol goes back to a pipe, now to SWI-Prolog as a child process.
* **SWI-Prolog stays** the reasoning engine: facts, rules, constraints, incremental tabling, s(CASP) and APE.
* **All the project's own code outside Prolog is Rust**, the reference checker included. The Python modules `tools/jcs.py` and `tools/terms.py` are to be ported and removed. A cost, stated when the choice was offered: the checker is no longer an independent implementation of the format.
* **A terminal with kitty's keyboard and graphics protocols is required.** Checked in crossterm 0.29's source: on Unix, key-release events arrive only when `KeyboardEnhancementFlags::REPORT_EVENT_TYPES` is pushed, which needs the kitty keyboard protocol; on Windows they always arrive. Which terminals support both protocols is recalled, not checked (kitty and Ghostty; WezTerm with gaps; foot has the keyboard protocol but uses sixel for images), and must be checked before milestone 3.

The design's estimate for the PC prototype was made for a browser client; it is marked as not yet revised.

## Milestone 1, first part: canonical JSON and terms, in Rust (12 November 2026)

`crates/format` replaces `tools/jcs.py` and `tools/terms.py`, which are removed. It holds the RFC 8785 serialiser and the term encoding, with floats tagged as `{"float": text}`. One dependency, `serde_json` (with `float_roundtrip`), to parse journal lines; `serde` only in tests.

A fault found in the port, by the test against Node: Rust's shortest float formatting breaks ties differently from ECMAScript. For an exact value such as 1424953923781206.25, both …206.2 and …206.3 are shortest and equally near; ECMAScript takes the even digit (Number::toString, step 5) and Rust took …206.3. 5 of the first 10,000 random doubles were such ties. The fix finds a tie from the exact decimal expansion and takes the even candidate when it reads back to the same double.

Checked:

* `make test` exits 0 inside `nix develop -i` (Rust 1.95.0): the tabling spike, 23 Rust tests, `cargo clippy -D warnings` and `cargo fmt --check`.
* Numbers match all 24 finite vectors of RFC 8785 Appendix B, and match Node on 1,000,000 random doubles (one run, `JCS_SAMPLES=1000000`, 31 s); the suite runs 10,000.
* Six deliberate breaks each made tests fail: tie rule removed; keys sorted by code point; one control character left unescaped; floats written as bare numbers; the sign of -0.0 dropped; the canonical check on reading removed.

## Milestone 1: the journal (12 November 2026, done, with two gaps below)

Built:

* `crates/format`: records (entry, seal, key, headers), each checking itself, and the chain check across `journal.jsonl` (sequence, parents, `prev` hashes, purges). SHA-256 only; no signature code. New dependency: `sha2`.
* `crates/memex`: the journal writer (append; branch from any earlier entry; purge, as a purge entry then the target's body set to null through a temporary file and a rename) and replay.
* `crates/sealer`: a separate program. `sealer init` makes a key (secret in a file of mode 600; public half in `keys.jsonl`); `sealer run` polls `journal.jsonl` and seals each complete new entry with the date, the key's fingerprint and an Ed25519 signature over the entry's hash. It checks the journal and the existing seals before it signs, refuses a key file others can read, and leaves a line without its newline for the next round. New dependencies: `ed25519-dalek` (signing), `getrandom` (key generation).
* `crates/check`: the reference checker, a library and a program (`check <dir>`: exit 0, 1 naming file and line, or 2 for usage). It checks `keys.jsonl`, then `journal.jsonl`, then `seals.jsonl`, then `blobs/`, each line before the next, so the first error names the first wrong line.
* `docs/design.md` § The journal format now states version 1 exactly. Two changes from the exported design: a seal's entry hash is named `entry_hash` (the design named two fields `hash`), and an entry's hash leaves out `body`, for which `body_hash` stands (the design said the body is "replaced by" its hash, while also keeping `body_hash`).

Acceptance checks (all in `make test`; output from this session):

* 10,000 generated entries (with branches, every kind but purge, 20 purges, Unicode text, tagged floats, one blob) check: "10,000 entries: written and sealed in 9.5 s, checked in 2.3 s" (unoptimised build of our crates).
* Every byte of every line of `keys.jsonl`, `journal.jsonl` and `seals.jsonl` of a 12-entry journal was changed in turn (XOR 1): "10306 single-byte changes, each refused at its own line". This covers every field of every record kind. A byte that breaks UTF-8, and a changed blob, are each named too.
* A purged entry still verifies, sealed before the purge, with the purge entry sealed after.
* Replay twice gives the same store. This is weak: until milestone 2 defines facts, the store is the list of entries.

Further tests, added because the acceptance checks alone did not catch some faults:

* A seal with a forged signature but a recomputed hash and re-chained successors is refused: "the signature does not verify". A journal rewritten consistently from entry 2 on, every hash recomputed, is refused by the seals: "entry_hash is not the hash of entry 2". Without these two, removing the signature check made no test fail.
* An unsealed entry is named. Without this test, switching off that rule made no test fail; found by breaking the code on purpose.
* `tests/memex_holds_no_key.sh`: the memex's dependency tree holds no signing or randomness crate, and no line of `crates/memex/src` or `prolog/` names the seals file or a key (comments skipped). It fails when `ed25519-dalek` is added to the memex, or when a memex source names `seals.jsonl`. Its first version failed on correct code (a doc comment, and the word "secret" as test data); fixed. Limit: it cannot see a path built at run time.
* The sealer program, run as a separate process with a 5 ms poll while the writer appended 200 entries, sealed all 200, and the result checks.

Deliberate breaks, each made a test fail: signature check off; `entry_hash` comparison off; unsealed-entry rule off; `prev` comparison off; purge rule ("body still here") off; own-hash comparison off; canonical-form check off; key-file permission check off.

Gaps, not done:

* **Unicode NFC.** The format says strings are in NFC. Nothing checks or produces NFC yet. *Closed the same day*: the author chose that the writer normalises text (see below).
* **Dates are not checked for order.** A seal's date comes from the system clock, which can go backwards; the checker does not compare dates.

Time: the plan estimated 1–2 weeks with Claude; this took one session. It is not comparable to the estimate's assumption of a person reviewing each step.

## Unicode NFC (12 November 2026)

The author chose that the writer normalises text. Now the memex normalises every string and object key of an entry's body to NFC before hashing and writing it, and refuses an object whose keys become equal once normalised. Every record parser refuses a line with a string or key not in NFC, so the checker catches text written otherwise. New dependency: `unicode-normalization` 0.1.26 (Unicode 18.0), in `crates/format`.

Checked:

* Known cases: e with a combining acute to é; the Angstrom sign to Å; Hangul jamo to a syllable; two combining marks put in canonical order, then composed.
* 10,000 random strings of letters, combining marks and Hangul jamo, against Node's `String.prototype.normalize("NFC")` (ICU 78.3, Unicode 17.0): all agree. 663 of them needed normalising (counted).
* A line whose hashes are right but whose text is not NFC is refused: "a string is not in Unicode NFC".
* Four deliberate breaks each made a test fail: the parser's NFC check off; the writer's normalisation removed; normalisation returning text unchanged; colliding keys merged silently.
* `make test` exits 0, here and in `nix develop -i`.

## Milestone 2: store, terms and the query protocol (12 November 2026, done)

Built:

* `prolog/protocol.pl`: the protocol's Prolog end. Framing, the atom table, and terms to cells and back.
* `prolog/server.pl`: the server, run by the memex as `swipl -q prolog/server.pl`. Each query is a SWI-Prolog engine, so it computes an answer only when NEXT asks. ASSERT and RETRACT record each fact with the journal entry it came from; a fact's predicate is declared dynamic and incremental, ready for milestone 4's tabling.
* `crates/protocol`: the Rust end of the codec, with no dependencies beyond `crates/format`'s terms.
* `crates/memex/src/prolog.rs`: the client. It starts SWI-Prolog as a child process and reads its replies on a thread, so a caller can wait with a timeout.
* `crates/memex/src/store.rs`: the store. A world is the path from a head entry back to the root; its facts are the entries' `"facts"`, less those a `retract` names. `load` asserts them into Prolog in order.
* `docs/design.md` § Query protocol now has a subsection with every point the design left open (byte order, numbering, typed BLOBs, ATOM\_IS with names, DONE's status, error handling), and § The journal format has the `facts` and `retract` bodies.

Acceptance checks (all in `make test`; output from this session):

* Round trip, both directions, both languages:
  * Rust only: 10,000 generated terms (atoms, integers small and large, floats, strings, variables, lists, nested compounds) encode and decode to themselves.
  * Prolog only: 10,000 generated terms, the same.
  * Rust to Prolog to Rust: "10,000 Rust -> Prolog -> Rust round trips in 2.08 s". Prolog decodes each term, unifies a variable with it, and encodes the answer back.
  * Prolog to Rust to Prolog: "10,000 Prolog -> Rust -> Prolog round trips in 20.7 s". Prolog generates each term from a seed, Rust decodes and re-encodes it, and Prolog checks it is the term it made. (Slower because each round trip is two queries, and Prolog reseeds and regenerates.)
* Answers only on NEXT. A query with 1,000 answers sends no message within 300 ms of QUERY, and Prolog has computed none (a flag the goal bumps per answer reads 0). NEXT 1, 10 and 989 each bring exactly that many ANSWERs, nothing more within 300 ms, and the flag reads 1, 11 and 1,000. The next NEXT brings DONE with status 0 and no answer.
* CANCEL: acknowledged with DONE 2; a NEXT after it gets an error.

Faults found by the tests and fixed:

* SWI-Prolog's list cell is `'[|]'/2`. A Rust term `'[|]'(a, b)` came back as the list `[a|b]`, which is a different term; SWI-Prolog has no separate form for the original. Found by the Rust to Prolog to Rust test at term 220. Such a STRUCT is now refused with `list_cell_clash`.
* The Prolog generator wrote `-(2^100)` and `2^200` as unevaluated expressions, so no large integer was ever sent. Now it evaluates them, and stays within 64 bits for the tests that cross to Rust. A Prolog-only test covers integers beyond 64 bits, and a cross test shows Rust refusing 2^100: "does not fit 64 bits".
* The DONE status was first written as a raw word 0, which decodes as VAR 0, not the integer 0. Caught in review before the first run; it is now an INT cell.
* In SWI-Prolog 7 and later, `[]` is not an atom, and `'.'(a, b)` in source is dict notation. Both broke the first Prolog tests; handled.

Deliberate breaks, each made a test fail: INT range off by one, in Prolog and in Rust; answers computed eagerly (`findall` before the engine); one answer too many per NEXT; the `[|]`/2 refusal removed; the dense-variable check removed (Rust); standard output not redirected. The last was not caught at first: the noise test printed only to the current output, which a separate line already redirects. The test now also prints to `user_output` by name, and fails without the redirect.

`make test` exits 0 on SWI-Prolog 9.0.4 (this container) and 9.2.9 (`nix develop -i`).

Not done, or weak:

* The server runs every goal it is sent, with the user's full rights. The only client is the memex itself, so this is by design, but it is a fact to know.
* A malformed header stops the server, and the client reports that it stopped; nothing restarts the server yet. *Settled the same day*: the author ruled that both ends die (see below).
* Rust holds integers in 64 bits, so a wider integer from Prolog is refused, not carried.

## Dying on a garbled header (12 November 2026)

The author ruled: die on a garbled message header. Before, the server printed an error and ended with status 0, as if all were well; a header cut short after its first byte was taken for a clean end of input.

Now:

* The server says why ("dying on a garbled message") and exits with status 1, for a non-zero reserved field, an unknown type, no request number, a message cut short, or a header cut short. A clean end of input still exits 0. A well-framed request with a bad body still gets DONE 1, and the server carries on.
* The client, on a garbled message from the server or on the server stopping, kills the child and records the reason (with the exit status). Every later call fails with `Error::Dead` and that reason. Nothing restarts it. The client is a library, so it returns the error rather than ending the process; the memex program will exit on it.
* An over-long message from the client is refused before it is sent, and does not kill the connection: it is the client's own mistake, not a stream out of step.

Checked: three new tests (the server's exit status and message for five kinds of garbled header, and 0 for a clean end; the client dying with the server; the client dying on garbage from a fake server). Three deliberate breaks each made one fail: the server stopping with status 0; the client treating a stopped server as an ordinary error; the client passing a garbled reply through. A fourth break, removing the dead check in `receive`, made none fail: it was redundant with the check in `send`, and gave the same reason, so it was removed. `make test` exits 0 here and in `nix develop -i`.

Two faults met on the way: a fake server written as `initialization((Goal1, Goal2), main)` did not run its goal, while the same goal as a named `main` did; I did not find why. And the first fake test waited out its 60-second timeout before failing, because nothing arrived.

## Milestone 3: the outline and the quasimodes (12 November 2026, done, with limits below)

Built, in `crates/memex`:

* `outline.rs`: items in a tree, known by the seq of the entry that made them. Text is appended once and an item's text is a list of spans, so surviving characters keep their addresses, and a span into another item's text is a transclusion, shown in place with its origin. Also links, names, outline numbers ("3.2").
* `app.rs`: the interface state, apart from the terminal. Key events go in; journal entries come out. Every change of state goes through `apply(entry)`, live and in replay, which is what makes replay reproduce the screen. It holds:
  * the quasimodes: Leap forward and back, Command, Scope and Suppose, each held or latched by a tap;
  * Leap's recognition on Escape, `?` lists every match, and "Leap again repeats";
  * Command's recognition, its guide words (`link 3.2 (to) source 9`) and `?` help, with 20 commands and a palette on F1;
  * undo and redo along the current branch, an edit after undo starting a new branch;
  * retraction, which lists the entries that depend on the one retracted and waits for Enter;
  * two panes: an outline, and the selected item's links.
* `ui.rs`: drawing, after the scene 1 mock-up. `term.rs` and the `memex` program: the terminal check, the keys, the event loop. A second without keys writes what is pending.
* `make run` starts the sealer in the background and the memex in the terminal. **Not exercised**: it needs an interactive terminal.

The format gained the `view` kind and the outline's bodies; `docs/design.md` records them, with the provisional keys.

Acceptance checks (all in `make test`):

* **Scene 1 as written.** Hold Leap and type "dra": the Leap box shows "LEAP · 3 MATCHES" and the three matches with their paths, and the guide line reads "release Leap to go to the first match · Esc completes · ? lists every match · Leap again repeats". Nothing is written while Leap is held. Release: the cursor is on DRAKON, the journal has exactly one new entry (a `view` entry, `op` leap, pattern "dra"), and the right pane shows "supports →", "cited by ←" and "transcluded in ←" with their items.
* **Keystroke to visible change**, on 10,000 items, measured as handling a key and drawing the whole screen into a 160 x 50 test terminal. 39 keystrokes: arrows, typing, a Leap of 11 letters, Enter.
  * Optimised build: mean 2.0 ms, worst 4.7 ms; the journal of 10,001 entries replays in 0.17 s.
  * Unoptimised (`make test`): mean 18.8 ms, worst 43 ms; replay 0.9 s.
  * Writing to a real terminal is not included.
* **Replay reproduces the screen.** A session of every kind of action (typing, branches, links, a transclusion, a name, a fold, undo, redo, Scope, a word deleted, a retraction, a supposition, a pane switch) replays to the same screen, cell for cell, but for the message line, which is feedback, not state. A second test compares the whole state after each of 17 steps.
* `tests/memex_in_a_pty.py` runs the real program in a pseudo-terminal, with the script playing the terminal:
  * Answered as a terminal without the protocols, it refuses to start, names both protocols, and writes nothing.
  * Answered as kitty, it reads keys in the kitty encoding: typed text reaches the journal, a held Right Alt makes one Leap, ended by its release, and F10 quits with status 0.

Faults found and fixed:

* Replaying 10,000 entries took 16.6 s (unoptimised): `apply` computed the whole outline order before every entry. It now does so only when an item may vanish. 0.9 s.
* Retracting an item also removed an unrelated item written after it, whose placement named it as its preceding sibling. Now an item whose sibling is gone goes first among its siblings; only a missing parent makes an edit fail.
* The program panicked on its first frame in a pseudo-terminal, which starts at 0 x 0: an underflow when the pane has no rows. Found by the pseudo-terminal test; a test now draws at six small sizes.
* Leap's match list printed an empty ". ›" for top-level items; it walked past the top of the outline.
* The no-key test (milestone 1) matched `ev.key` in Rust code as a key file name; narrowed to a quoted name ending in `.key`, and shown still to fail on one.

Deliberate breaks, each made a test fail: Leap writing no entry; undo's parent wrong; retraction without confirmation; placement failing without its sibling; release not ending a held quasimode; drawing without the row check; Right Alt unmapped (the pseudo-terminal test). One break was not caught at first: typing that did not record the text cursor. The screen comparison cannot see the terminal's cursor, and a later view entry hid the difference. The step-by-step replay test catches it.

Terminals (from kitty's own protocol pages, read by a search agent; not tested here):

* Listed as implementing both the keyboard and the graphics protocol: kitty, WezTerm, Ghostty, iTerm2. Konsole is listed for graphics only, and foot for the keyboard protocol only.
* No page opened confirms that any terminal but kitty sends key releases, which the quasimodes need. Release events are flag 0b10 of the keyboard protocol.

Limits:

* The quasimode keys are provisional and untried on real keyboards. Right Alt is AltGr on many layouts, and the operating system may take Super.
* The memex has run only under a script playing a terminal, never in a real kitty, WezTerm or Ghostty.
* Images (the graphics protocol) are checked for at start but not yet used.
* Time: the plan estimated 2–3 weeks; this took part of one session, without the author's review of the interface's feel, which the estimate counted.

## Milestone 4: claims, constraints and worlds (12 November 2026, done, with limits below)

Built:

* `prolog/dataflow.pl`: the dataflow graph. A node is defined by a `node_def` fact in the journal; `node/2` computes it, tabled incrementally and shared; each evaluation is counted. A constraint's body is matched against beliefs and claims, so each solution says whether it is a violation (beliefs only) or a disagreement between sources (a claim in it). `plant_check/3` tries a constraint against a planted violation.
* `crates/memex/src/engine.rs`: one SWI-Prolog process per world shown, its facts synced by difference, so only changed facts invalidate tables. Terms the user types are read by Prolog's own reader.
* `crates/memex/src/reason.rs`: solutions, readings of a supposition, restatement through a generic frame and `label` facts, derived text ("a year", "two years"), the text timeline.
* In the interface:
  * the commands `fact`, `claim`, `constraint`, `derive`, `adopt` (F3), `unadopt` (F4), `interrogate` (F2), `close` and `commit`;
  * marks under items, and the constraint panel;
  * the "do you mean?" step after Suppose, then revision;
  * two worlds side by side, and the fading hint.

Acceptance checks (all in `make test`):

* **Scene 2 as written**, the restatement aside:
  * The talk's claim against the source's date is reported as "1 disagreement between sources", with no violation.
  * Both items are marked, naming each other ("1988 here, 1989 in 2.1"), and the marks are dimmed.
  * The right pane shows "CONSTRAINT · ONE DATE PER EVENT", the rule one literal per line, "disagreements between sources · recomputed at journal 42", "buran_flight: 1988 (1.1) · 1989 (2.1)", and "depends on these facts" with the derived sentence and the timeline.
  * The fading hint shows.
* **Scene 3 as written**, the restatement by APE aside:
  * Suppose "the flight was in 1989" offers "Do you mean: Suppose that the date of Buran's flight is 1989."; Enter opens the second world.
  * Side by side: "a year" against "two years", the timeline's 1988 against 1989, "0 violations, 1 disagreement" against "0 violations · 1.1 retracted here".
  * The actual world's facts are unchanged; the supposition is a branch off the actual head. `close` returns to the actual world and keeps the branch.
* **Recomputation touches only the nodes that depend on a change**, counted per node as in milestone 0's spike. After one new date fact for a third event:
  * "one date per event" +1 and the timeline +1, which read every date;
  * the sentence +0, which reads only the dates of Buran's flight and Energia's launch;
  * "one capital per country" +0.
* Also:
  * Adopting the claim (F3) turns the disagreement into a violation, shown loudly; retracting the adoption (F4) turns it back.
  * Interrogation (F2) answers who else claims it, who claims the contrary, its source and entry, the source's record, and "adopting it would raise 1 violation of one date per event".
  * A constraint that cannot fire (`D \= D`) is refused and nothing is written.
  * A replay gives the same outline, view, supposition, reasoning and screen.

Faults found and fixed:

* **Tabling cached nothing in the server**: every query re-evaluated every node, even with nothing changed. Each query runs in its own engine, and SWI-Prolog's tables were private to it. Declaring them `shared` fixed it. A test then showed a capital fact still re-evaluates the capital constraint, and finds its violation, and undoing it does too: the tables are shared and still invalidated.
* My first expectation for the recomputation test was wrong. I expected the derived sentence to be re-evaluated by any new date, but it reads two specific dates, and SWI-Prolog tracks call variants. The test now expects 0 for it.
* The rule was split at every comma, including inside `date(E, D1)`; now only at top-level commas.
* Long lines in the right pane were cut off; the panels now wrap.

Deliberate breaks, each made a test fail: private tables; no revision after a supposition; violations and disagreements swapped; the planted check skipped; claims ignored by constraints; adoption writing no facts.

`make test` exits 0 here (SWI-Prolog 9.0.4) and in `nix develop -i` (9.2.9).

Limits:

* Without APE, Suppose understands only "change one number of a known fact". The restatement uses a generic frame, not a verb-frame lexicon.
* Revision sets aside every belief in a violation with the supposition; it does not look for a minimal set, as the translator document notes s(CASP) also does not.
* The timeline is text, not an image; the graphics protocol is still unused.
* The memex cannot show seal dates in provenance: the no-key rule forbids its sources to name the seals file. The rule should allow reading it, by a check that the memex opens it read-only. Not done.
* Every engine is a separate SWI-Prolog process, about 100 ms to start (estimate, not measured).

## Milestone 5: Wikidata and suggestions without a model (12 November 2026, done, with limits below)

Built:

* `crates/tools`, the `wikidata` program. It writes a subset of Wikidata as Prolog facts: the items' truthy statements without external identifiers, English labels, and the properties' constraints. It queries the query service through `curl`, under a generic user agent that names neither the user nor the project.
* `tests/fixtures/wikidata-buran.pl`: 386 facts of real Wikidata (CC0), made by `wikidata --items Q2548,Q4273185,Q1010251,Q80811` in 2.7 s: Buran the spaceplane, a village also called Buran, the Buran programme, and Sputnik 1.
* `import wikidata <file>`: the file into `blobs/` under its hash, an `import` entry with its provenance, and loading into each world whose path holds it.
* In `prolog/dataflow.pl`:
  * Wikidata's statements become claims through adopted `same_as` and `wd_property` facts;
  * suggestions of five kinds: entity links, property mappings, Wikidata's single-value constraints, mined rules, and references among stored works;
  * adopted rules (`rule_def`) take part in reasoning.
* The suggestion pane: `pane suggestions`, `accept N`, `dismiss N`. An adoption is an entry marked as a suggestion, with its evidence.

Acceptance checks (all in `make test`):

* **A planted wrong date is reported against Wikidata.** The notes say Buran flew in 1989, with the user's constraint one date per event.
  * After the import, the first suggestion is "buran_flight is Wikidata Q2548" (the spaceplane, not the village), with the evidence "Wikidata dates it within five years of yours".
  * Accepting it, then "date is Wikidata's P606" (first flight), gives "1 disagreement between sources" and the mark "one date per event, sources disagree: 1989 here, 1988 in Wikidata".
* **No suggested reference names a work outside the store.** Synthetic works (`QTEST1` to `QTEST3`): `QTEST3`, the obvious answer, is about Buran and cited by both others, but it is not in the store. The only reference suggested is "cite paper_a for the claims about buran_flight", and nothing names `QTEST3`. Accepting it makes a `cites` link from the claim's item to the paper's item.
* Also:
  * on real data, Wikidata's single-value constraint on P619 is offered for Sputnik 1's launch, tried against a planted violation, kept, and not offered again;
  * a mined rule is offered with "support 2, confidence 1.00, 0 exceptions", and once adopted it derives a new fact ("Vostochny is in russia");
  * the pane never takes the keyboard, shows at most five, ranks by score, keeps the order of those that remain while it has focus, and keeps a dismissal through a replay.

Faults found and fixed:

* **Stack overflow in a test thread.** The protocol's decoder recursed once per list element, and a list of a few hundred suggestions overflowed a 2 MB stack (unoptimised). The decoder now walks a list's spine in a loop. A new test decodes a list of 200,000 elements on a 256 KB stack, and overflows with the old decoder.
* **Duplicate suggestions with unstable keys.** Each rule was suggested once per fact naming its predicates, with variable numbers depending on its place in the list, so dismissals and the frozen order could not hold. Now: distinct predicates, duplicates removed up to variable renaming, and variables numbered within each suggestion.
* **Sputnik 1 was linked to Q1393751** ("Sputnik", the programme), not Q80811 ("Sputnik 1"). A label was matched only against single words of the name. It is now matched against the whole name too, which scores higher.
* **The frozen order broke as evidence grew.** A suggestion's key included its evidence. Its place now uses what it proposes alone; dismissals still use the evidence too.
* **My pane test was wrong:** it required every shown suggestion to stay, but a rule whose confidence falls below 0.8 rightly leaves. It now checks that those that remain keep their order.
* **The user agent.** In milestone 5's first probes I sent "adversaria" in the user agent of about six requests to Wikidata and Crossref, against the rule that project details stay private. The tool's user agent is now generic.
* **Rate limiting.** Wikimedia's API rate-limited this machine (HTTP 429) during a check; I did not retry it, and checked the identifiers through the query service instead.

Deliberate breaks, each made a test fail: any scholarly article counted as in the store; no Wikidata claims; the order never frozen; dismissals ignored; the whole-name label match removed; and, in the protocol, the recursive list decoder.

Checked against Wikidata's own labels (one query to the query service): P31 instance of, P50 author, P356 DOI, P577 publication date, P606 first flight, P619 UTC date of spacecraft launch, P921 main subject, P1433 published in, P2302 property constraint, P2860 cites work, P5824 retracted by, Q13442814 scholarly article, Q19474404 single-value constraint, Q21502410 distinct-values constraint.

Limits:

* **Subset D itself is not built.** The tool can make it (`wikidata --min-sitelinks K --limit 100000 --out …`), but I did not run 100,000 items' worth of queries against the public service from here. K is not yet calibrated to give about 100,000 items, and the query that would count them may time out.
* Matching is by English labels only. Dates use only the year, and their precision is not kept.
* One Wikidata import is loaded at a time.
* The reference test uses synthetic works: Wikidata has no scholarly article about Buran.
* A long list still recurses when dropped (Rust's default drop), and the other term walkers still recurse. Only the decoder walks lists in a loop.

## Milestone 6: documents and results in and out (12 November 2026, done, with limits below)

Built:

* `crates/memex/src/docs.rs`: Pandoc's JSON tree to outline blocks and back, losses counted by kind, longest-common-subsequence alignment, and the quote check's text.
* In the interface:
  * `import document <file>`, and a revised file opening a world with a table of changes;
  * `export <file> [item]`, with `source-entries` in the document's metadata;
  * `import results <file>`, and the "input changed since" mark;
  * `check quote` and `check source`.
* `helpers/python/memex_results.py`, which an analysis calls to write its results file.
* The dev shell gains `curl`. The source check and the `wikidata` tool need it, and its absence was found when `make test` failed in `nix develop -i`.

Acceptance checks (all in `make test`):

* **Markdown round trip.** A document with two headings, emphasis, a link, a citation, inline and display mathematics, nested and ordered lists, a quote, code, a table, a footnote and an image:
  * the import reports "losses: footnote 1, image 1", and keeps them in the import entry;
  * the citation becomes a `cites` fact, and the display formula stays LaTeX (`\int_0^1 x \, dx = \frac{1}{2}`);
  * the export, read back, has the same 16 blocks (style, depth, text), with no further loss.
* **Word round trip.** Markdown to `.docx` by Pandoc, imported, exported to `.docx` by the memex, imported again: the same blocks.
  * Against the Markdown original: the same styles and depths, the same headings, the same citation (recovered from text, since Word carries it as text), and the formula still LaTeX.
  * Losses: "footnote 1". The image never reached the Word file, because Pandoc could not find `chart.png`.
* **A changed input file marks its results.** The Python helper writes `results.json` with `data.csv`'s hash; the import makes "mean_delay = 1.55 ± 0.05 s" with a `measured(1.55, 0.05, "s")` claim. After `data.csv` is edited, the item shows "⚑ input changed since: data.csv".
* Also:
  * a revised document opens a world listing "changed 1.1", "deleted 1.2" and "added"; retracting the deletion and committing keeps the rest;
  * the quote check rules `quoted`, then `misquoted` after the text is edited;
  * the source check, against a local server standing in for Crossref, rules `found` (title alike but for case and hyphens), `misattributed`, `retracted`, `not_found`, and, with the server stopped, `unavailable`.

Checked against the real Crossref (two requests, generic user agent): 10.1038/nature12373 has `title` and no retraction; the retracted Wakefield 1998 paper lists `{"type": "retraction"}` in `updated-by`, the field the check reads.

Faults found and fixed:

* Every Markdown heading counted as a loss, because Pandoc always gives headings an identifier. Now only an identifier other than the one Pandoc would make from the text counts, or classes and attributes. The rule for that identifier is approximated from memory of Pandoc's documentation.
* Text left with a trailing space where an image was removed differed after the round trip; block text is now trimmed (code aside).
* My source-check test expected a typographic apostrophe (’ against ') to be ignored; the design's rule ignores only spaces, hyphens and case, so the test was wrong.
* `curl` was missing from the dev shell (above).

Deliberate breaks, each made a test fail: citation keys not recovered from text; footnotes not counted as losses; the stale check reading no hashes; case not ignored; retractions undetected; a revision written without its world.

`make test` exits 0 here (Pandoc 3.1.3) and in `nix develop -i` (Pandoc 3.7.0.2).

Limits:

* Lost kinds are counted, not kept. A table's caption and cell structure beyond " | ", footnotes and images are not carried. Images do not go to `blobs/` yet.
* Re-import does not see moves: a block that moved is a deletion and an addition. A deleted item whose children survive is not deleted.
* The quote check compares plain text through Pandoc, so formatting differences do not count; the source check reads titles and retractions, not abstracts, so it never rules a quote `quoted` from the source itself.
* Results take values with standard uncertainties only; the other kinds in § Uncertainty wait for stage B.

## Milestone 7: measurement (12 November 2026, the report built; the month of use is the author's)

Built:

* The memex records what reasoning shows. A `view` entry, op `observed`, holds the current world's violations and disagreements whenever they change; op `offer` holds the suggestions shown in the pane for the first time. Without these, offered suggestions and violations could not be counted from the journal alone.
* `crates/tools/src/measures.rs` and the `measures` program. It checks the journal first, then reports actions by kind, the clerical share, suggestions (offered, adopted, dismissed, adopted then retracted), constraint violations (seen, corrected) and disagreements. Every entry is counted once: an action, an observation, or unclassified. `make measures JOURNAL=…` runs it.

Acceptance checks (in `make test`):

* **Counts match a hand count on a small journal.** A session of 22 entries, printed and read by hand, its kinds and ops written into the test literally:
  * writing 9 (entries 2, 4, 6, 7, 8, 13–16); navigating 4 (3, 5, 17, 22); searching 1 (10); deciding 4 (11, 19, 20, 21);
  * observations 3 (9, 12, 18), and entry 1, the memex's own, unclassified;
  * 2 suggestions offered, 1 adopted, 1 dismissed, 1 adopted then retracted;
  * 1 violation seen and corrected; 0 disagreements;
  * the clerical share "5 of 18, 27.8%".
  
  The report gives exactly these.
* **Runs on any valid journal.** An empty journal gives zeros. 10,000 entries of every kind with arbitrary bodies are each counted once: actions + observations + unclassified = 10,000. The program reports on a valid journal (exit 0), and refuses one changed by one byte (exit 1).
* A unit test: a violation that disappears only in a supposed world is not counted as corrected.

Faults found and fixed: my first invariant assumed entry 1 is always the memex's own first entry, but in a generated journal it is any kind. Now the report counts unclassified entries openly, and the invariant includes them.

Deliberate breaks, each made a test fail: corrections counted in any world; every retraction counted as an adoption retracted; Leap counted as navigating; dismissals not counted; observations not recorded.

`make test` exits 0 here and in `nix develop -i`.

**Not done: the month of use.** The plan's second item is the author using the memex for real work for a month, with the report run weekly and logged here. That is the author's to do; nothing in this log is a measurement of real use. Things to know before it starts:

* The memex has never run in a real terminal; only a script has played kitty. The quasimode keys are provisional.
* Subset D is not built (see milestone 5). The month can start with the import of a small subset, or none.
* The kinds of action are this report's own classification, written in `measures.rs`; the design names them (searching, following links, transforming, writing, deciding) but does not define them.

## Stage A: where it stands (12 November 2026)

Milestones 0 to 6 are built and pass their acceptance checks. Milestone 7's report is built and checked; the month of use waits for the author. The questions stage A asks get their answers from that month's journal, and nothing here answers them yet:

1. Do constraints find real errors in the user's notes?
2. Does the user adopt suggestions, and keep them?
3. Does the share of clerical actions fall?
4. Is the dated, chained record used and trusted?

### Sizes (counted, after milestone 7)

| Part | Lines | Of which tests |
| --- | --- | --- |
| `crates/memex` (with its tests) | 8,792 | 2,397 |
| `crates/format` | 1,827 | 860 |
| `crates/protocol` | 639 | 236 |
| `crates/tools` | 641 | 100 |
| `crates/sealer` | 384 | 124 |
| `crates/check` | 234 | 57 |
| `crates/acceptance` | 1,147 | 1,146 |
| `prolog/` (server, protocol, dataflow) | 737 | 0 |
| `prolog/tests/` | 141 | 141 |
| `helpers/python/memex_results.py` | 74 | 0 |
| Tests in Python and shell | 189 | 189 |

Tests: 132 Rust tests, 18 Prolog tests, the pseudo-terminal script (7 checks), the tabling spike and the no-key script, in `make test`.
