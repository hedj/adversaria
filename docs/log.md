# Log of measured results

Each entry says what was built, what was measured, and what failed or was skipped.

The plan these results answer to is docs/plan.md; how the code and the
documents are arranged, and why, is docs/structure.md.

## 1. Stage A: the record

### Milestone 0: toolchain and spikes (12 November 2026, done)

#### Tools and the environment

##### Tool versions (measured)

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
| APE with Clex | needed to teach controlled English | 6.7, commit 5f4d535 (21 April 2024), with its small Clex lexicon | pinned in Nix on 17 November 2026 (flake input `ape`, built by `nix/devshell.nix`, `APE_HOME` in the shell); a commit after the last tag (6.7.180715), which still set SWI-Prolog's removed `float_format` flag. Checked: it builds in Nix's sandbox, paraphrases "John likes Mary.", and names where "Buran flighted in 1989." failed, with a repair. "The date of Buran's flight is 1989." is not ACE with the small lexicon: `flight` is not in it. Clex, the large lexicon (97,541 lines, commit 20960a5, 5 November 2018, GPL-3.0), is pinned too (flake input `clex`) and replaces the small one before the grammar is compiled, as its README says. With it the sentence parses ("A date of a flight of Buran is 1989."), and "The flight was in 1989." fails at `was`, which ACE does not allow (past tense). Loading APE with Clex and parsing four sentences took 1.1 s (measured once) |

The session runs in a cloud container that is rebuilt from scratch. SWI-Prolog is not in its base image, so each new session must install it again until the environment's setup script does so.

##### The Nix environment (measured, 12 November 2026)

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

#### The spikes

##### Spike: incremental tabling (measured, passed)

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

##### Spike: s(CASP) and issue #13 (measured, reproduced)

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

#### Decisions

##### Disagreement between the plan and the design

The design (§ Query protocol, last paragraph) says the prototype sends the protocol's messages "over a pipe between its client and SWI-Prolog". The plan (milestone 2) says "over a websocket", and asks for SWI-Prolog's websocket library. A browser client cannot open a pipe, so the plan's choice looks right. Resolved by the author on 12 November 2026: a websocket. `docs/design.md` now says so.

##### Decisions by the author (12 November 2026)

1. **Licence:** GPL-3.0. It allows APE and Clex to ship with the project.
2. **Numbers in hashed records:** JSON numbers, with RFC 8785's serialisation in full; the plan's proposal of decimal strings is not taken. Milestone 1 must implement RFC 8785's number rules in the server and in the checker, and test them.
3. **Wikidata subset:** D, the pocket base (about 100K items × 20 facts, about 63 MB; an estimate).
4. **s(CASP):** installed from source, with permission; spike run above.

### Change of plans (12 November 2026)

The author changed the prototype's form. Recorded in `docs/design.md` § Decisions, whose rows name the sections changed, and `docs/plan.md`.

* **A terminal program in Rust**, drawn with ratatui on crossterm. No web interface. This replaces the browser client, and the websocket chosen earlier the same day; the query protocol goes back to a pipe, now to SWI-Prolog as a child process.
* **SWI-Prolog stays** the reasoning engine: facts, rules, constraints, incremental tabling, s(CASP) and APE.
* **All the project's own code outside Prolog is Rust**, the reference checker included. The Python modules `tools/jcs.py` and `tools/terms.py` are to be ported and removed. A cost, stated when the choice was offered: the checker is no longer an independent implementation of the format.
* **A terminal with kitty's keyboard and graphics protocols is required.** Checked in crossterm 0.29's source: on Unix, key-release events arrive only when `KeyboardEnhancementFlags::REPORT_EVENT_TYPES` is pushed, which needs the kitty keyboard protocol; on Windows they always arrive. Which terminals support both protocols is recalled, not checked (kitty and Ghostty; WezTerm with gaps; foot has the keyboard protocol but uses sixel for images), and must be checked before milestone 3.

The design's estimate for the PC prototype was made for a browser client; it is marked as not yet revised.

### Milestone 1: canonical JSON, the journal and NFC

#### Milestone 1, first part: canonical JSON and terms, in Rust (12 November 2026)

`crates/format` replaces `tools/jcs.py` and `tools/terms.py`, which are removed. It holds the RFC 8785 serialiser and the term encoding, with floats tagged as `{"float": text}`. One dependency, `serde_json` (with `float_roundtrip`), to parse journal lines; `serde` only in tests.

A fault found in the port, by the test against Node: Rust's shortest float formatting breaks ties differently from ECMAScript. For an exact value such as 1424953923781206.25, both …206.2 and …206.3 are shortest and equally near; ECMAScript takes the even digit (Number::toString, step 5) and Rust took …206.3. 5 of the first 10,000 random doubles were such ties. The fix finds a tie from the exact decimal expansion and takes the even candidate when it reads back to the same double.

Checked:

* `make test` exits 0 inside `nix develop -i` (Rust 1.95.0): the tabling spike, 23 Rust tests, `cargo clippy -D warnings` and `cargo fmt --check`.
* Numbers match all 24 finite vectors of RFC 8785 Appendix B, and match Node on 1,000,000 random doubles (one run, `JCS_SAMPLES=1000000`, 31 s); the suite runs 10,000.
* Six deliberate breaks each made tests fail: tie rule removed; keys sorted by code point; one control character left unescaped; floats written as bare numbers; the sign of -0.0 dropped; the canonical check on reading removed.

#### Milestone 1: the journal (12 November 2026, done, with two gaps below)

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
* **Dates are not checked for order.** A seal's date comes from the system clock, which can go backwards; the checker does not compare dates. *Closed on 18 November 2026* (see § Seal dates in order).

Time: the plan estimated 1–2 weeks with Claude; this took one session. It is not comparable to the estimate's assumption of a person reviewing each step.

#### Unicode NFC (12 November 2026)

The author chose that the writer normalises text. Now the memex normalises every string and object key of an entry's body to NFC before hashing and writing it, and refuses an object whose keys become equal once normalised. Every record parser refuses a line with a string or key not in NFC, so the checker catches text written otherwise. New dependency: `unicode-normalization` 0.1.26 (Unicode 18.0), in `crates/format`.

Checked:

* Known cases: e with a combining acute to é; the Angstrom sign to Å; Hangul jamo to a syllable; two combining marks put in canonical order, then composed.
* 10,000 random strings of letters, combining marks and Hangul jamo, against Node's `String.prototype.normalize("NFC")` (ICU 78.3, Unicode 17.0): all agree. 663 of them needed normalising (counted).
* A line whose hashes are right but whose text is not NFC is refused: "a string is not in Unicode NFC".
* Four deliberate breaks each made a test fail: the parser's NFC check off; the writer's normalisation removed; normalisation returning text unchanged; colliding keys merged silently.
* `make test` exits 0, here and in `nix develop -i`.

### Milestone 2: store, terms and the query protocol

#### Milestone 2: store, terms and the query protocol (12 November 2026, done)

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

#### Dying on a garbled header (12 November 2026)

The author ruled: die on a garbled message header. Before, the server printed an error and ended with status 0, as if all were well; a header cut short after its first byte was taken for a clean end of input.

Now:

* The server says why ("dying on a garbled message") and exits with status 1, for a non-zero reserved field, an unknown type, no request number, a message cut short, or a header cut short. A clean end of input still exits 0. A well-framed request with a bad body still gets DONE 1, and the server carries on.
* The client, on a garbled message from the server or on the server stopping, kills the child and records the reason (with the exit status). Every later call fails with `Error::Dead` and that reason. Nothing restarts it. The client is a library, so it returns the error rather than ending the process; the memex program will exit on it.
* An over-long message from the client is refused before it is sent, and does not kill the connection: it is the client's own mistake, not a stream out of step.

Checked: three new tests (the server's exit status and message for five kinds of garbled header, and 0 for a clean end; the client dying with the server; the client dying on garbage from a fake server). Three deliberate breaks each made one fail: the server stopping with status 0; the client treating a stopped server as an ordinary error; the client passing a garbled reply through. A fourth break, removing the dead check in `receive`, made none fail: it was redundant with the check in `send`, and gave the same reason, so it was removed. `make test` exits 0 here and in `nix develop -i`.

Two faults met on the way: a fake server written as `initialization((Goal1, Goal2), main)` did not run its goal, while the same goal as a named `main` did; I did not find why. And the first fake test waited out its 60-second timeout before failing, because nothing arrived.

## 2. Stage A: the notebook

### Milestone 3: the outline and the quasimodes (12 November 2026, done, with limits below)

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

### Milestone 4: claims, constraints and worlds (12 November 2026, done, with limits below)

Built:

* `prolog/dataflow.pl`: the dataflow graph. A node is defined by a `node_def` fact in the journal; `node/2` computes it, tabled incrementally and shared; each evaluation is counted. A constraint's body is matched against beliefs and claims, so each solution says whether it is a violation (beliefs only) or a disagreement between sources (a claim in it). `plant_check/3` tries a constraint against a planted violation.
* `crates/memex/src/engine.rs`: one SWI-Prolog process per world shown, its facts synced by difference, so only changed facts invalidate tables. Terms the user types are read by Prolog's own reader.
* `crates/memex/src/reason.rs`: solutions, readings of a supposition, restatement through a generic frame and `label` facts, derived text ("a year", "two years"), the text timeline.
* In the interface:
  * nine commands for facts, claims and worlds (listed in docs/guide.md, § 2.4), three of them on F2, F3 and F4;
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

### Milestones 5 and 6: sources

#### Milestone 5: Wikidata and suggestions without a model (12 November 2026, done, with limits below)

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

#### Milestone 6: documents and results in and out (12 November 2026, done, with limits below)

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

### Milestone 7: measurement (12 November 2026, the report built; the month of use is the author's)

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

## 3. After stage A

### Manuals and settings

#### Manual and key configuration (12 November 2026)

The author asked where the user manual was. There was none. Now:

* **Configuration** (`crates/memex/src/config.rs`). A TOML file names the keys of each quasimode and binds keys to command lines. It is read from `--config FILE`, `$ADVERSARIA_CONFIG`, `$XDG_CONFIG_HOME/adversaria/config.toml` or `~/.config/adversaria/config.toml`; a missing file means the defaults, which are the keys used so far. A named entry replaces its default; `""` unbinds a key. `memex --print-config` writes the defaults; `docs/config.example.toml` is that output, and a test keeps them equal. A file with an unknown section, quasimode, key or command, an editing key, an empty key list or one key with two jobs is refused, naming the file and the reason, before the terminal is touched. Insert, PageUp, PageDown, Menu, Pause and the lock keys can now be bound. Dependency: `toml` 1.2.1.
* **Tutorial** (`docs/tutorial.md`): the design's Buran story as a first session. `crates/memex/tests/tutorial.rs` performs it and checks each screen text it quotes, and that each command and text it uses appears in the tutorial.
* **Guide** (`docs/guide.md`). `crates/memex/tests/guide.rs` checks that it has a row for every command and no others, lists exactly the key names the configuration knows, and gives examples the configuration accepts.
* **README.md** points to these.

##### Faults found

Faults found and fixed on the way:

* The F-key path discarded the mode a command set, so a bound `retract` would have lost its confirmation. A bound key now keeps it.
* Hints named fixed keys ("F3 adopts · F4 retracts an adoption"; "switch (F6) to edit") that a configuration can move. They now name the bound key, or "command adopt" when none is bound.
* A configuration giving only `[quasimodes]` first dropped every default binding; a configuration test caught it before commit.
* An empty key list left a quasimode unreachable; it is now refused.
* Two help texts were stale (suppose, import).

##### Checked

Checked: `make test` passes here (SWI-Prolog 9.0.4) and in `nix develop -i` (SWI-Prolog 9.2.9): 143 Rust tests, 18 Prolog tests (12 and 6 sub-tests), 14 pseudo-terminal checks. The pseudo-terminal script now also runs the binary with a configuration that makes F12 Leap forward, unbinds F10 and binds Insert to quit, and with a wrong file (exit status 2, nothing written). Each new test was broken on purpose once and failed.

Not checked: the tutorial's `make run` and `check` steps, and every key in a real terminal. The quasimode defaults are still provisional; the configuration is how to change them after trying them.

#### The mouse (12 November 2026)

The memex had no mouse support, though the plan asks for adopt and retract "with one click" (milestone 4) and the design for suggestions adopted by a click. The milestone 4 entry above called that milestone done without noting the gap.

* `[mouse] enabled` in the configuration, default false; `--mouse` and `--no-mouse` override it for one run, and both together are refused.
* When on, a left click on an item's line moves the cursor there; on the interrogation hint's two halves, runs adopt or unadopt; on a suggestion, accepts it. A click does nothing while a quasimode or confirmation is open. A click is journaled as the keyboard action it stands for, so replay agrees.
* Where things were drawn comes from the drawing itself (`ui::Hits`). Wrapped suggestion heights use ratatui's `Paragraph::line_count`, behind its `unstable-rendered-line-info` feature: an API ratatui may change.

Checked: `make test` passes (147 Rust tests, 18 Prolog tests, 21 pseudo-terminal checks). The pseudo-terminal script sends a real SGR mouse click to the binary. Each new test was broken on purpose once and failed.

Not done: the wheel, dragging, right clicks, clicks in the worlds and revision views, and diagrams (not built). Not checked: any real terminal, or whether text selection with Shift works while the memex has the mouse.

#### Nix without experimental features (12 November 2026)

`nix-shell` now gives the development shell with no experimental Nix features: `shell.nix` reads the pins from `flake.lock` and fetches them with `builtins.fetchTarball` and `builtins.fetchGit`; the shell is defined once, in `nix/devshell.nix`, which `flake.nix` also uses. Checked with experimental features switched off: `shell.nix` gives the same derivation as the flake, and `make test` passes inside it. Not checked: a machine whose Nix has never had flakes enabled, and macOS. Then the author ran `nix-shell` with no nixpkgs on `NIX_PATH` and got an error from `(import <nixpkgs> {}).bashInteractive`: `nix-shell` looks up an interactive bash there, outside `shell.nix`, and fell back to the system's bash. `./dev-shell` resolves the pinned nixpkgs first and passes it with `-I`; checked with experimental features off and `NIX_PATH` unset: no error, bash from the pinned nixpkgs, and `make test` passes.

### Input that could act (12 November 2026)

The author pointed out that imports (Pandoc, Wikidata, Crossref, results files) and the pipe to SWI-Prolog could carry injection or unexpected terms. I audited each by trying it. Found and fixed, each with a test that failed first (`crates/memex/tests/hostile.rs` unless named):

1. **Pandoc read other files.** `import document paper.tex` with `\input{…}` copied the named file into the outline and journal. Fixed with `--sandbox`. (`.rst` `include::` and `.org` `#+INCLUDE` do the same in Pandoc, but the memex does not read those formats.)
2. **Export turned text into markup.** `<script>` ran in an HTML export, `\input{…}` became raw LaTeX, `javascript:` links were live, and `![x](/path)` embedded a local file in a .docx (seen in `word/media/`). Fixed by filtering Pandoc's tree. Pandoc 3.1.3's `--sandbox` cannot write .docx (it cannot find its own data files); 3.7's can. So the writing step is not sandboxed, and has no images left to read.
3. **Crossref got patterns.** A DOI with `[1-5]` made five requests (against a local server); `[1-1000000]` would make a million. Fixed by encoding and `--globoff`.
4. **Wikidata files were loosely checked.** `wd_label(X, "anything")` was accepted; values could be any term. Directives and quasi-quotations were already refused. Fixed: only the tool's ground shapes, checked before the import entry is written.
5. **Deep terms crashed the memex.** A term 1,000 deep from the server overflowed the client's stack. Fixed by a 256-level limit at both ends (`crates/protocol/src/tests.rs`, `prolog/tests/test_protocol.pl`).
6. **Definitions could run anything.** Typing `constraint c: false :- date(E, Y), shell('touch …')` ran the command during the planted check, before anything was kept; a `node_def` sent to the server ran it on evaluation; a claimed `node_def` would have run on adoption; a fact `(p :- …)` would have been stored as a rule. Fixed in three layers (reasoner allow-list, server storing only plain user facts, memex refusals).
7. **A results file could block the memex.** An input that was a named pipe blocked it forever; `/dev/zero` would exhaust memory. Fixed: only regular files are hashed.

Not a hole: Rust builds every Prolog goal as a term, never as source text, and strings cross the pipe as length-prefixed blobs, so no data is ever parsed as code on the way. Results and document imports write only fixed shapes of fact.

Also noticed, not fixed: a fact nested more than about 60 deep is refused with "not JSON: recursion limit exceeded", which is safe but unclear; refusal messages for Wikidata files show the Prolog error term raw.

Checked: `make test` passes (161 Rust tests, 19 Prolog tests, 21 pseudo-terminal checks). Each fix was broken on purpose once and its test failed. Open items are listed in docs/design.md, § Input that could act.

### Computing and drawing

#### Boxes that compute (12 November 2026)

The author asked whether a user could build a spreadsheet from what exists: boxes with values, declared relationships, and updates as values change. A trial budget (items with `cell/2` facts, derived text, two constraints) showed it could, with three faults: changing a value took `retract` with an entry number the screen does not show beside the fact, and the new fact attached to whatever item the cursor was on; and a supposition set aside every belief in a violation with it, so supposing food is 900 removed income and rent too, and `commit` made that real. Fixed: a supposition now sets aside only what it directly contradicts (a violation with it and one other belief), naming the rest; and `set` states a fact and retracts what it directly contradicts, on the replaced fact's item. Nothing spreadsheet-specific was added: a value replaces another only because the user's constraint "one value per cell" says so. The tutorial has a part 2 that builds the budget, checked by its test.

Limits: the boxes are an outline, not a grid; only integers can be supposed; one `undo` reverses only the fact part of a `set`; marks list each violation under every item it involves, which is noisy for a rule over three boxes; the constraint panel shows a stray leading colon (": income, 2000 …"), not yet investigated.

#### Views (12 November 2026)

The author asked for graphs, charts, spreadsheets, flowcharts and timelines built from simple elements bound to Prolog queries, after reading about the grammar of graphics, D3 and Observable. Built: `rule`; `layer MARK where GOAL` with ten marks; drawing as a plot, a grid or a layered graph; views drawn under their items, live, loud on violations, clickable, and drawn in both worlds of a supposition; the outline offered as facts. docs/design.md, § Views, records the sources and what each idea became.

Faults found on the way, each fixed with a test that failed first:

* rules did not work inside negation, disjunction or findall;
* `?` could not be typed inside a quoted string in a command;
* the planted check refused constraints on negative values (`L < 0`);
* a violation through a rule rested on the rule, not the facts under it, so the facts' marks and views were not loud;
* the outline was drawn by item, so tall views would push the cursor off screen; drawing every item instead made milestone 3's 10,000-item test fail at 117 ms per keystroke (limit 100); drawing only items near the cursor brought it to 44 ms;
* in the renderer: a crowded timeline lost a label; lanes read bottom up; an edge label lay under a later line; a cycle shared a route with a forward edge; two edges merged at one port.

Checked: `make test` passes (187 Rust tests and the tutorial's three parts, 19 Prolog tests, 21 pseudo-terminal checks). A gallery test builds each kind through the memex's commands. Not checked: any of it in a real terminal; views wider than the screen are cut, not scrolled.

### Teaching the controlled English

The design teaches ACE while the user types: APE's repair hints, a
predictive editor, `?` in Suppose, and a memory of confirmed pairs
(`docs/translator.md` § "Do you mean?"). Before 17 November 2026 none
of it was built, and Suppose's "Do you mean?" used no parser. The author
asked for all four; APE and Clex are pinned in Nix (environment table
above).

#### APE in the memex (17 November 2026)

* `prolog/ace.pl`: `ace_lexicon/2` sets the user lexicon, leaving out
  ACE's function words (APE takes them, but warns that they should not be
  redefined) and returning APE's lexicon warnings; `ace_parse/2` gives a
  text's paraphrase, its DRS (referents numbered, as the query protocol
  sends no variable a query did not have) and APE's messages, or, if it
  is not ACE, the messages alone. A parse has 5 seconds.
* `prolog/ace_server.pl`: the query protocol server with APE loaded, a
  process of its own, started when first needed. Loading APE and Clex
  takes about a second (measured once: 1.1 s with four parses), which the
  reasoning engines do not pay.
* `crates/memex/src/ace.rs`: the client, and `lexicon_of`, the notebook's
  words: each entity a proper name, each one-place fact's name an
  adjective, each two-place fact's name a noun, each label of one word
  (spaces made hyphens) a name for its entity.
* The command `ace SENTENCE` shows APE's paraphrase, or where the
  sentence failed and APE's advice. APE's warning that a definite noun
  phrase has no antecedent is left out: one sentence about a fact has
  none to have.

Measured with APE (`prolog/tests/test_ace.pl` and probes): a name of one
word, with underscores or hyphens, works (`buran_flight`,
`Buran-flight`); a name of two words does not; "Buran's flight" is read
as a flight of Buran whatever the lexicon says; "The flight was in
1989." fails at `was` (ACE has no past tense), and APE's advice is `as`.

Faults found on the way:

* APE's files declare no encoding, and `refres.pl` holds an en dash in a
  comment: in a POSIX locale SWI-Prolog warned of an illegal multibyte
  sequence. `ace.pl` sets the encoding to UTF-8 before loading APE.
* APE keeps lexicon warnings until they are cleared, and reported them as
  each later sentence's own. `ace_lexicon/2` clears them.
* APE keeps its messages in a global variable, and the server runs each
  query in an engine of its own, whose global variables start unset:
  every parse threw `existence_error(variable, message)`. The Prolog
  tests ran in one thread and passed; the memex's tests found it.
  `ace_parse/2` sets the variable first, and a test now parses in an
  engine of its own.

Checked: 11 Prolog tests and 14 Rust tests (11 unit, 3 through the
memex); 14 breaks of the code, each made a test fail (listed in the
commit); `make test` passes in the Nix shell. The APE tests need APE_HOME, so `make test` needs the Nix
shell. Not built yet: repair hints in Suppose, the predictive editor,
`?` in Suppose, the memory of confirmed pairs.

#### Repair hints in Suppose (17 November 2026)

While Suppose is held, each key checks what is typed (`ace_check/2`),
ended with a full stop if it has none: the line below the typing says
`ACE:` and APE's paraphrase, or `not ACE:` with the sentence marked where
parsing stopped and up to three repairs, the first applied by Esc.

APE's own advice for a word was weaker than the design assumed: it gives
the first word one edit away in its lexicon, in a fixed order, whether or
not it fits (`was` → `as`, `dat` → `at`, `flew` → `few`), and has nothing
for the past tense. A repair here is an edit of one word that makes the
text ACE, tried in the sentence: the word where parsing stopped, then
each word APE does not know; replaced by its present if it is a past
(`was`, `were`, `had`, `did`, and `-ed` forms whose present is in the
lexicon), or by each word one edit away. So `dat` becomes `date` and
`was` becomes `is`.

Measured, for the time a key waits:

* APE's paraphrase of a long sentence with relative clauses took 0.5 s at
  every parse: APE tries that paraphrase under its own 0.5 s limit, with
  the comment "due to a bug?", before falling back to full sentences.
  `ace.pl` now allows 50 ms (0.531 s → 0.055 s for the test's sentence;
  commit 3bec6cc).
* The repair search parsed every candidate, and APE catches exceptions
  inside its parser, so a time limit did not stop it: 3.8 s for one long
  sentence. It now checks a deadline (0.25 s) before each candidate, and
  a candidate is parsed without its paraphrase. The four probe sentences,
  up to 33 words: 32–73 ms per check, from 56 ms to 3.8 s before.
* A check is still a parse at each key, and the key waits for it: about
  2 ms for a short sentence that is ACE, 38–57 ms for "The flight was in
  1989", where repairs are sought (measured on this machine).

Checked: 19 Prolog tests and 6 tests through the memex, 14 unit tests in
`ace.rs`; 13 breaks, each made a test fail (listed in the commit);
`make test` passes in the Nix shell. Not checked: a real terminal.

#### The predictive editor (17 November 2026)

While Suppose is held, the line below the typing says what may come next
in ACE, as the user types (`docs/translator.md`: "offers only the next
words the ACE grammar accepts").

APE has no interface for this, so `prolog/ace_next.pl` asks its grammar.
The grammar takes every token through a few predicates of
`grammar_words` (`word/3`, `word_noninitial/4`, ...): a function word by
its form, a content word by a condition naming its category
(`noun_sg(W, Lemma, Gender)`). Each is wrapped (`wrap_predicate/4`):
where the input left is a marker, the wrapper records what was asked for
and fails, so the grammar tries every other way on. A text parsed with the
marker after it thus gives every word that may come next, exactly; APE's
own files are unchanged. A first try, an attributed variable as the next
token, failed: a grammar condition calls `atom_concat/3` on the token.

`ace_check/2` now gives a third answer, `open(Next, Completed)`, for a
text that is not a sentence yet but can go on: the next words, those the
word being typed begins (ignoring case), the notebook's words first, then
kinds of word, then function words; and the text completed when one word
alone fits, which Esc applies. A word is judged once it ends (a space or
an end mark), as in predictive editors. Two faults of the repairs, found
while testing it: a repair had to make the text a whole sentence, so
"The flight was " was not mended to "The flight is " (it goes on); and
the notebook's word came after APE's (dart, data, date).

Measured: the lookahead takes 1–20 ms for a beginning of up to 9 words,
70–200 ms for one of 25 or more, and has a quarter of a second at most;
a check of a short beginning, with it, 3–8 ms.

Checked: 27 Prolog tests (8 new) and 8 tests through the memex (2 new),
16 unit tests in `ace.rs` (2 new); 15 breaks, each made a test fail
(listed in the commit), one only after a test was added for it: the
first of two fitting words taken as the completion.

#### `?` in Suppose (17 November 2026)

`?` lists what may come next with a line of help (design, the outline
model's aid 10, after TOPS-20): the notebook's words (marked `own` by
`ace_next.pl`, so the list can tell them from ACE's words), each kind of
word with what it is and examples, and the words of ACE, ten to a line.
It works with nothing typed (how a sentence may begin) and after a whole
sentence ("and" may follow "John waits"); `ace_next/2` gives the next
words for any text. `?` is also ACE's question mark, so in Suppose it
lists only at the start or after a space; typed after a word it ends a
question.

Found on the way, and fixed first (commit efd7bb7): APE restates no
yes-no question (its own paraphrase is "# ..."), so "Does John wait?"
was said to be `ACE:` with nothing after; and APE's drs_to_drslist/2
left a choice point for a text with a question.

Checked: 30 Prolog tests (2 new), 17 unit tests in `ace.rs` (1 new), 11
tests through the memex (3 new); 11 breaks, each made a test fail.

#### The memory of confirmed pairs (17 November 2026)

The author chose pairs of free English and ACE, as the design's
conversational loop logs them, over pairs of text and fact, which the
journal already held. While Suppose is held, the memex keeps the fullest
text typed that was not ACE (a word half typed or half deleted does not
replace it). When a supposition is written from a sentence of ACE, its
entry records that free English as `english` beside its `name`: a
revision of format version 1, recorded in the design.

`crates/memex/src/pairs.rs` reads the pairs from the journal, latest
first, and adapts one to a new text: the two must have as many words,
differ in at most two words and in fewer than half, each changed word
must be in the ACE, and a word changed in two places must change the
same way. While typing, a text like an earlier free English is offered
the earlier ACE with the new words swapped in, if APE takes it as ACE:
`as before: … · Esc`. No index or BM25: a scan of the pairs, as there are
few (the design's estimate was 0.5–1K lines; this is 116 lines, without
its tests).

Found on the way: a word changed in two places was swapped twice, so
the second swap found nothing and the pair did not adapt. Before it,
commit 9439c4a made the quasimode's key one state (`Hold`) rather than
two flags; two claims in its message were wrong (see the next commit).

Checked: 6 unit tests in `pairs.rs`, 1 in `actions/english.rs`, 3
through the memex (a pair kept; none for a text that was ACE from the
start; a similar text offered the adapted ACE, Esc applying it, and the
pairs read again after reopening); 12 breaks, each made a test fail, two
only after a test was added for them (the latest pair first; at most two
words changed).

### The memex in a window (17 and 18 November 2026)

The author decided that a terminal with kitty's keyboard and graphics
protocols was the wrong dependency, and asked for egui, working under
Wayland too.

Built (`crates/memex/src/gui`, 953 lines with 13 unit tests): winit opens
the window and gives each key's place and the layout's character; ratatui
lays the screen out as before (`crate::ui` is unchanged) into cells held in
memory; egui paints the cells on a monospace grid through wgpu. eframe was
not used: it hides winit's key events, egui-winit drops text while Ctrl is
held, and egui's `Key` has no `(`, `)`, `_` or `<`. Cargo.lock went from
221 packages to 465.

Checked: `tests/memex_in_a_window.py` (in `make test`, in place of the
pseudo-terminal script) runs the real program under Xvfb, typing raw X key
codes with xdotool, and under a headless sway with wtype. 21 checks pass:
typing, Leap held and released, Command with its parentheses, the mouse,
a remapped configuration, the write after a second idle, a violation
observed, and typing and F10 under Wayland. Eight breaks, each made a test
fail (six in the unit tests; in the window test, every key-up read as
key-down failed 11 checks, and a drawn frame asking for another failed the
idle write).

Faults found on the way:

* xdotool's key names press both Alt keys for `Alt_R`, and add Alt to
  `F10`. The test types raw key codes.
* egui has fonts only within a frame; measuring a cell before the first
  frame panicked. The cells are measured and laid out within the frame.
* A drawn frame asked for another, so the loop never waited, and text left
  for a second was never written. It no longer asks.
* Without egui's `default_fonts` every glyph was a box.
* The dependencies' debug information made each memex test binary some
  344 MB, and the build filled the session's disk at 17 GB. Test builds
  now leave it out (Cargo.toml); `target/` was 3.1 GB after building
  memex's tests.
* winit asks for ahash's default features, whose `runtime-rng` brings in
  getrandom, which `tests/memex_holds_no_key.sh` forbids. By the author's
  choice the test stands and ahash is patched: `vendor/ahash` is 0.8.12
  with `runtime-rng` left out of its defaults, and nothing else changed.
  Its hash tables then use fixed seeds. Cargo shows the vendored crate's
  own lint warnings (unexpected cfgs `fuzzing` and pointer width 128,
  unused methods); they are upstream's, not faults.

Limits:

* Under Wayland only typing and F10 are tested. wtype makes up its own
  key codes, so the quasimodes, which go by a key's place, are untested
  there with a real keyboard.
* Not run on a real desktop or GPU: the tests draw with Mesa's software
  drivers.

Then, with the author's agreement, the terminal's code went: `term.rs`
(492 lines), `tests/memex_in_a_pty.py` (267 lines), and ratatui's
crossterm backend (ratatui now builds without its default features).
crossterm stays listed in Cargo.lock, which records optional dependencies
whatever features are on, but nothing builds it.

### Focus lost while a quasimode key is held (18 November 2026)

The fault: a quasimode ends when its key is released, but a key released
while another window has the keyboard is not reported to the memex. On
X11 winit reports such releases as synthetic events, which the window
ignores, as it ignores the presses winit makes up on focus; on Wayland
none is sent. Alt+Tab, with Left Alt as Leap back, left Leap held: what
was typed after it went into the Leap's pattern, and F10 did not quit.
The window test reproduced this under X11 and under Wayland before the
fix.

The fix: when the window loses focus (`Focused(false)`, sent on both), a
quasimode held with nothing else pressed is left without acting, and
Scope puts back the scope it found (`Active::scope_before`); one held
after keys were pressed stays open, latched, with what was typed, and
the message line says so (docs/design.md § Quasimodes, not modes).
Shift needs nothing: winit reports the modifiers cleared on both.

Checked: 3 tests through the memex (alone held, left, nothing written;
Scope's scope put back; typed, kept open, a late release ignored, Enter
leaps) and 2 window checks (Right Alt held while the root window takes
focus under X11; F12, moved to Leap, held while sway moves to an empty
workspace). Five breaks, each made a test fail: focus loss ignored by the
app, by the window (both window checks fail), a lone hold latched, the
scope not put back, a typed one finished. The window test passed 3 runs
in a row.

Found on the way: Backspace on an empty latched Scope leaves it without
putting the scope back, so the next command acts at word scope (read
in the code; the next commit tests and fixes it). wtype loses a key held with `-P` unless keys are 50 ms apart.

### Box-drawing characters without seams (18 November 2026)

The fault, measured: screenshots under headless sway, typing with wtype,
at scales 1, 1.25, 1.5 and 2. Down the line between the panes, at 1.25,
19 rows of pixels were dim, one every 22 rows or so: where one row of
cells meets the next. A cell is then a fractional number of pixels high,
and the font's glyph for a line stops short of the cell's height. At 1,
1.5 and 2 the line was whole.

The fix, as terminals do it: each cell's edges are put on whole pixels,
the same for neighbouring cells (`Grid::rect`), and box-drawing and block
characters are drawn as shapes that fill the cell (`crate::gui::boxes`):
the light and heavy lines, corners, tees and crosses, the rounded corners,
the diagonals, and the full, half and eighth blocks. Lines are a whole
number of pixels wide, about a point. Dashed and double lines and shades
are still the font's; the screen draws none of them.

Checked: at 1.25 the line has no dim row; at the other scales it is still
whole; the Leap box's corners join (seen in a screenshot). 7 unit tests
for the new code, the gui's 20 in all; seven breaks, each made a test
fail. The first test of the seam passed with snapping removed: its cell
(17.6 points at a pixel of 0.8) was a whole 22 pixels high, so it was
changed to 17.5. A rounded corner's straight part first stopped half way
to the cell's edge, found by its test. Rounded corners and diagonals are
tested, not yet seen on screen. `Screen.cell` became `Screen.cell_size`,
since `cell` also names a cell's rectangle.

### A click at a scale of 2 under Wayland (18 November 2026)

The window is told of the pointer in pixels; the cells are measured in
points. At a scale of 1 the two agree, so the clicks tested under Xvfb
could not catch a click put in the wrong cell at another scale.

Under headless sway no pointer had reached the window: sway has no input
devices there, and wlrctl makes a virtual pointer for each action and
removes it as it exits, so the window dropped its pointer before any
motion or click came (seen with WAYLAND_DEBUG: the seat's pointer came
and went at once). `pointer` (crates/tools) keeps one virtual pointer on
the seat while its input is open, and reads `move X Y WIDTH HEIGHT` and
`click`. Its crates were already in Cargo.lock, through winit.

Checked: the window test clicks the first item at a scale of 2 and types
Z into it (now 24 checks); a screenshot with the cursor drawn (`grim -c`)
showed the cursor's tip on the item. Keeping the pointer in pixels makes
that check fail, and only that one.

A click sent with the move was lost, and the first version of the test
paused half a second after each. The cause, traced with WAYLAND_DEBUG on
sway and on the memex: the pointer program made its virtual pointer only
when it sent its first command, since wayland-client sends requests at
the next round trip. Sway took the pointer, the move and the click
together, and only then told the window that the seat had a pointer; the
window then bound its own, and sway sent it the enter. The click had
gone to no one: the window had no pointer to receive it. Neither sway
nor the memex lost it. The pointer program now waits for the compositor
when it makes the pointer, writes "ready" once the pointer is on the
seat and "done" after each command, and the test waits for those, and
for the window to be in sway's tree, instead of pausing. Without the
wait for the compositor, the check fails (twice in two runs).

The other Wayland checks still wait fixed times for the window.

### A keyboard for the Wayland checks, and no fixed waits (18 November 2026)

`keyboard` (crates/tools) keeps one virtual keyboard on the seat while
its input is open, with a US keymap compiled by libxkbcommon, so keys go
by their evdev codes as a real keyboard sends them. It keeps the keys'
state with libxkbcommon and sends the modifiers when a key changes them:
a virtual keyboard must, and without it Shift was lost ("Hello" arrived
as "hello"). It reads `down`, `up`, `key` and `type`, and answers
"ready" and "done" as `pointer` does; what the two share is
`crate::seat`. It replaces wtype, whose first key was lost for the cause
found for the pointer: a device made and used at once. With real key
codes, a Leap held on Right Alt is now checked under Wayland too, and
the focus check holds Right Alt rather than F12 (25 checks).

The fixed waits went: Xvfb says when it is ready (`-displayfd`);
xdotool waits for the window (`search --sync`) and for focus
(`windowfocus --sync`); sway's window events, followed on its IPC socket
from before the memex starts, say when the window has focus; the device
programs say when each command is done; and the crash check kills the
memex once its text is in the journal, within the 2 seconds it had
before. Conditions without an event (sway's socket, the journal, the
window on screen) are checked every tenth of a second to a deadline.

Found on the way:

* `swaymsg -t subscribe -m` writes no reply when it subscribes, so a
  read of one waited for ever: the first version of the test hung. The
  test now speaks sway's IPC itself and reads sway's reply.
* sway reports a new window focused before it shows it. It shows it
  once the window has drawn at the size sway gave it, some 20 ms later
  here. A click before then lands on the empty workspace, which takes
  the keyboard, so keys typed afterwards reach no window (seen with
  WAYLAND_DEBUG: a keyboard leave, then the window's surface entering
  the output). The click now waits until the screen (grim) shows
  something other than the background where it will click. A person
  cannot click so soon; it is the test's fault, not the memex's.

Checked: the window test passed 5 runs. Breaks, each failing checks:
the click not waiting for the screen (the click check), no modifiers
sent (3 Wayland checks), "ready" before the device is on the seat (4
Wayland checks).

### The window test waits on events only (18 November 2026)

The three conditions the test still checked every tenth of a second now
each come as an event:

* sway's readiness: its config's `exec` runs a script once sway takes
  clients, and the script writes `WAYLAND_DISPLAY` and `SWAYSOCK` to a
  FIFO the test reads;
* the crash check's journal: Linux reports each change to the journal's
  directory (inotify, through libc), and the check asks again at each;
* the window on screen: `screen` (crates/tools) copies a region of the
  output with wlr-screencopy and, while it does not show a colour, copies
  it again with damage, which the compositor answers only once the
  screen changes.

Each wait also watches its process's pidfd, so a program that ends ends
the wait: the X11 window search and sway's IPC no longer check the
process once a second. Each wait keeps a deadline, so a fault fails a
check: with sway's readiness never written, the five Wayland checks fail
in 72 s rather than hang. On the way, a device program that had ended
raised a broken pipe instead of failing its check; it now fails it.

Found on the way: the first `screen` reported any change at the click's
point, and the click still missed. The keys and the click came within
0.15 ms of each other, before the window's next frame, and the memex
rightly acts on a click against what it shows, which was the empty
outline (seen with WAYLAND_DEBUG). So the click now waits for what a
person waits for: the first item's row shown in the memex's ink. The
hint of an empty outline is dimmed, never ink; the screen showed no ink
there before the typing and showed it 15 ms after.

Checked: the window test passed 6 runs (3 since the last change). Breaks,
each failing checks: the click not waiting for the screen (the click
check, 2 runs of 2), the crash check not waiting for the journal (its
check), sway's readiness never written (5 Wayland checks), and, in
`screen`'s unit tests, red and blue swapped and a row's padding read as
a pixel. `Tools::capture` held 7 referents, the eight arguments of the
protocol's region capture; the next commit changes the design so that it
does not.

### The screen program copies the whole output (18 November 2026)

`screen` now copies the whole output (`capture_output`, three arguments)
and reads the region out of the copy itself, where it had asked the
compositor for the region (`capture_output_region`, eight). The region
is two spans, of columns and rows, in the output's pixels, which are
what is compared; the test gives the first item's row at twice its
logical place, the scale. The cost: each copy is the whole output, some
6.4 MB here, and a change anywhere wakes the watch, which then looks in
the region again; it still waits on the compositor only. With it,
`Tools::capture` holds 3 referents, and no unit of the code holds more
than 6.

Checked: screen's 3 unit tests; breaks of the region (columns ignored,
not cut to the buffer, a size taken for an end) each fail one, the last
only after the test's region moved off column 0, where a size and an
end are the same. The window test passed 3 runs; the click not waiting
for the screen still fails its check.

### Seal dates in order (18 November 2026)

The checker now refuses a seal dated before the seal before it, naming
its line of `seals.jsonl`; seals in one second may share a date. The
author chose to fail, not warn. The rule joins format version 1 in
docs/design.md § The journal format (version 1 revised): a clock that
went back makes the record's dates untrustworthy. Dates have one width
(`record::is_date` takes only `0000-00-00T00:00:00Z`), so comparing their
text compares their times. The sealer still writes what its clock says;
a journal whose clock went back now fails `check` until that is
explained.

Checked: a journal sealed by a clock giving 10, 10, then 9 seconds is
refused at `seals.jsonl` line 4, its third seal. Breaks, each failing
the test: no date check (the journal passes), equal dates refused (line 3
named, not 4).

### The sealer stops if the clock goes back (18 November 2026)

At the author's request, the sealer now writes no seal dated before the
last seal and stops instead: `sealer run` ends with status 1 and "the
clock went back: it says …, before the last seal's date, …; no seal
written". The batch it was sealing is written whole or not at all, so
no seal of it is written. The rule is one function, `record::may_follow`,
which `check` uses too. The entries wait, unsealed, until the sealer runs
with a clock past the last seal's date.

`check`'s test of an out-of-order seal had the sealer write it; the
sealer now refuses, so the test signs that seal itself, correct but for
its date (a dev-dependency on ed25519-dalek, already in Cargo.lock).

Checked: a new acceptance test seals with a clock giving 10, 11, then 9
(nothing written), then 10, 10, 10 (all three written), then 9 for a
fourth entry (refused, three seals kept). Breaks, each failing it: no
refusal; equal dates refused (which also fails `check`'s test); and the
batch compared only with the file's last seal (which also breaks the
chain, so eight other tests fail with it). A full disk during this work
left four test directories of about 690 MB in /tmp, from tests that
failed before their cleanup; they were removed.

### The memex reads what is written (18 November 2026)

At the author's request, the memex reads each item whose text is ACE as
the facts, claims, rules and constraints it states, and draws read text in
cyan; prose keeps a dim hint at the cursor.

Two faults found while building it:

- A rule read from "then the rest is X - Y" was for `cell(rest, A)`, which
  the memex refuses as recursive (its body reads `cell`). Rules now define
  `rest(A)`, and "the rest" reads `rest(A)` where a rule defines it
  (design.md § Reading controlled English).
- Reading at the keystroke broke the plan's 0.1 s: the first read waits
  for APE to start. Measured on 10,000 items (release build): worst
  keystroke 1.43 s, of which reading 1.42 s; later reads took 3 to 15 ms.
  Reading now waits for a pause (`App::idle`); worst keystroke 7.7 ms.

Checked: `crates/memex/tests/reading.rs`, 11 tests, and 11 Prolog tests
of the reader, pass; `make test` passes (2 min 42.5 s). Breaks, each
failing a test: a claim stated as a fact; no retraction of the old
reading; undo not grouping a reading with its text; no hint; no `read`
entry; no world rules passed to the reader; a rule's head left as a box;
the cyan left out; an emptied item keeping its facts (a fault this test
found, now fixed); constraints of one text given one name; and a
keystroke that reads. Not checked: that read text whose text later
changes without a new reading is drawn uncoloured; this happens only
without APE, which the test environment always has. The window loop's
pause is exercised only by the window test, which checks no reading.

### The tutorial in controlled English (18 November 2026)

At the author's request ("forces the user to type things in both english
and then in prolog"), docs/tutorial.md now has the user write each
statement once, in ACE, and the memex reads it. Counted in the tutorial's
commands (backquoted, by first word):

| | Before | After |
| --- | --- | --- |
| Statements typed in Prolog (`fact`, `claim`, `constraint`, `set`) | 15 | 1 |
| Other commands (`derive`, `layer`) | 9 | 6 |

The one statement left in Prolog is "one value per box", a constraint
over every box at once, for which ACE has no sentence; the tutorial says
so. `derive` and `layer` remain: a template and a view are not
statements. Values change by changing their sentence, not by `set`. The
step that shows prose was kept: the Buran sentence stays prose, with its
hint.

Faults found while rewriting it, each fixed in its own commit: a
constraint read from ACE was kept in silence (1f80921), and editing the
end of an item of two spans panicked a debug build (9099add).

Checked: the tutorial test performs every step and passes; it now also
checks that read text is drawn in cyan, in the outline. Breaks, each
failing it: a sentence in docs/tutorial.md changed (the test finds it
missing), reading turned off in `App::idle`, and the cyan left out.

### Kinds of thing, and why a text is not read (19 November 2026)

The author typed "a fish is an animal." and got "not read: ACE (There is
a fish X1. The fish X1 is an animal.), but not all of it something the
notebook states". The reading was right, the hint was not: ACE reads
the sentence as one fish with no name, and the notebook states facts
only of named things. The hint now says so: "…, which is about
something with no name; write "every" to mean all of them, or give it
a name". A text with a part the notebook cannot state says "…, but the
notebook has no statement for all of it".

The probe of the sentences that do say it found the reader wrong
(fixed in 3b098e6): "Nemo is a fish." read as the box
`cell(fish, named('Nemo'))`, now `fish(nemo)`; "Every fish is an animal."
as `animal(A) :- cell(fish, A)`, now `animal(A) :- fish(A)`; "Every fish
has a fin." as a rule that anything a fish has is a fin, now not read.

Checked: the reader's 14 tests and the memex's 14 reading tests pass.
Breaks, each failing a test: every reason unstated (two reader tests);
the two hints swapped (two memex tests).

### Wrapping (19 November 2026)

The author: "word wrapping is messed up almost everywhere", and the
list shown while Command is held and the "not read" hint were cut off.
Rendered at 80 by 30 (measured): the outline did not wrap at all, so
items, marks and hints were cut at the pane's edge (about 46 columns);
the command list gave each command's whole help (up to 121 characters)
in an unwrapped box; the foot line is cut at the screen's edge.

- The command list (4a40cd0): a brief help per command (at most 32
  characters), the whole help, wrapped, once a command is chosen, and
  "… and N more" when the list is taller than its box.
- The outline: each line is wrapped at spaces to the pane's width, its
  later rows set in under its text (an item's under its text, past its
  number; a mark's under its words, past ⚑). Scrolling, the places a
  click acts on and the text cursor count rows, not lines.
  `crates/memex/src/ui/wrap.rs` does the wrapping and maps a column of
  a line to its row.

- The foot: its two lines (what is typed, and the guide or the
  message) each take as many rows as they wrap to, up to three, so the
  outline keeps the rest of the screen; a message longer than three
  rows is cut at the third.
The referent count does not see a child module, so `ui.rs` reads as
holding 6 modules; a reader holds 7 (its child `wrap` too).

### The top bar, the right pane and the worlds at narrow widths (19 November 2026)

At the author's request, checked at 80 and 60 columns (rendered and
read): the top bar dropped its right side whenever it did not fit
beside the room's name, so at 80 columns "1 disagreement between
sources" was not shown at all; the right pane cut the selected item's
text; the worlds view cut every row and both titles. Fixed:

- The top bar's right side takes a row of its own when it does not fit
  beside the left; the left wraps to at most two rows.
- The right pane's links and the outline pane's title wrap.
- The worlds view wraps each world's rows, later rows set in by two,
  and keeps the worlds row for row: each pair of rows takes as many
  rows as the taller.

A test written for the old top bar (the right side not drawn when it
does not fit) now asserts the new behaviour, keeping its point: the
right side is never drawn over the left.

Not done: the left of the top bar is cut after two rows (a long
supposition's name loses its end, and "journal N" with it, at 60
columns); the worlds view does not scroll, as before.

### Questions in English (19 November 2026)

At the author's request, the reader reads questions, and the memex
answers them under their text: which (what, who, which) gives every
answer, sorted and joined by commas, or none; whether gives yes or no.
A question is kept as a derive entry with "under": true (journal format
version 1 revised; design.md), so its answer follows the facts as
derived text does, and is drawn under the item, which keeps its text.
The memex gives the reader the world's box names, so "What is the
income?" asks for the box's value, since ACE reads "the" as "a".

Checked: the reader's tests, 17 (3 new; one runs the answering goal
against facts); seven memex tests (which, and its answer after a new
fact; whether, yes and no; a box's value; a question made prose loses
its answer; the answer after reopening the journal; each world of a
supposition answers it). Breaks, each failing tests: the question not
asked; the answer in place of the text; no answer line; no box names;
"under" not read back; the old reading kept; a world's row without its
question.

Not done then: "Which things are animals?" was not ACE (no plural after
"are" in ACE); a later change the same day asks such a question in the
singular, for questions only. Answers were written as Prolog writes them (entities in lower
case); since a later change the same day, each is shown by its label
where it has one, sorted by its text.

### Leap or make (19 November 2026)

From the author's choice of the suggestions drawn from personal
knowledge management tools (nvALT's one box for search and creation):
in Leap, Enter with no match makes a new item holding the pattern,
after the cursor's item, and its text waits to be read. Releasing on no
match makes nothing, as before. Checked: four tests in tests/keys.rs;
breaks, each failing a test: Enter ending Leap as before, the new item
not read, the cursor left behind.

### Links typed in text (19 November 2026)

From the author's choice of the suggestions drawn from personal
knowledge management tools (Obsidian's and nvALT's wikilinks): `[[X]]`
in an item's text is read, at the pause, as a link to the item named X,
else to the first whose text is X, else to a new item holding X. Link
entries made so carry `"inline": true` (format version 1 revised), and
the text keeps them: removed brackets take their link back; added ones
add theirs; nothing is rewritten when the links are unchanged. A prose
item whose links change records a reading of nothing after them, so
undo takes the text and its links back together. The reader reads the
text without the brackets.

Checked: six tests in tests/inline_links.rs and two unit tests.
Breaks, each failing a test: no retraction; prose recording no reading
(undo then took back only the link); the brackets handed to the reader;
no new item for a missing target; a text matched before a name.

### Links as facts for views (19 November 2026)

From the same choice (Obsidian's graph view): each link between two
items is offered to goals as item_link(From, To, Type), beside
item_text/2 and item_under/2, so a view can draw the graph of every link,
and a suggestion can tell linked items from unlinked ones (the next
change needs it). Checked: a gallery test in tests/views.rs draws the
graph (Alpha to Beta, not the unlinked Gamma) and reads the type back
through derive; with no link facts it fails.

### Unlinked mentions as suggestions (19 November 2026)

From the same choice (Obsidian's unlinked mentions): an item whose text
names another item's short text, as whole words in any case, and does
not link to it, is offered a link, as a suggestion with its evidence.
Accepting it writes a link that carries "suggestion", which the
measures already count as an adoption. Suggestions are now grouped in
dataflow.pl by the source of their evidence: Wikidata, beliefs, outline.

A world with items but no facts of its own was not read at all; it is
now, as mentions, views of item_text and questions need it. Measured on
10,000 prose items (release build): first evaluation 2.12 s, later
0.137 to 0.139 s; with the mention clause off, 2.13 s, then 0.097 to
0.104 s. So mentions add about 35 to 40 ms per evaluation at that size,
and a notebook of prose alone now pays what one with a single fact did.

Checked: four tests in tests/mentions.rs. Breaks, each failing a test:
linked items offered; no word edges (unseen until a title's last word
was tried: "energia launch" in "energia launches"); sentences taken for
titles; a link that records no suggestion; a world of items alone not
read; outline suggestions not asked.

### Tags (19 November 2026)

From the same choice (Obsidian's tags): each #tag in an item's text is
offered to goals as item_tag(Item, Tag), computed from the text as
item_text is, so it follows the text with no entry to write or retract,
and undo needs nothing. Checked: a unit test of what is a tag; a
constraint over tags fires and clears as the text changes; derived text
counts tagged items. With no tag facts, both memex tests fail.

### Split an item (19 November 2026)

From the same choice (Zettelkasten's atomic notes): `split` breaks an
item at each sentence end; the item keeps its first sentence and links;
each other sentence becomes a new item after it, made of the item's own
spans (its characters keep their addresses), linked back as split_from,
and read at the pause. A closing record, edit op split, names the
split's first entry, and the parts' readings name it too, so one undo
takes the whole split back, before or after the pause (format version 1
revised). A decimal point is no sentence end. An abbreviation such as "e.g. "
was taken for one; a later change the same day ends a sentence only
where the next word starts one (a capital, a digit, a quote or a
bracket) and the word before is no abbreviation (from a list of 26) or
initial. "etc." was in that list, so "… etc. Then …" did not split. The
abbreviations are now of two kinds: 19 that never end a sentence
(e.g., Dr., p., approx., ...), and 7 that may (etc., et al., Jr., Sr.,
ed., eds., St.), which do before a capital letter only, not before a
digit, bracket or quote: "Ed Martin, Jr. Then" splits, "Smith et al.
(2020) saw" does not. Still wrong: "King Jr. Day" splits.

Checked: a unit test of sentences; four tests in tests/split.rs.
Breaks, each failing a test: no closing record; the parts read as their
own (undo after the pause then took back one part); undo ignoring split
records; parts without the item's text; no link back.

### Links to no item wait to be asked for (19 November 2026)

At the author's request ("Fix the gaps"), a [[link]] that names no item
makes nothing. The cursor's line says "not yet items: … · make makes
them"; `make` makes them at the foot of the top level and links them,
closed by an edit op make record so one undo takes it back (format
version 1 revised). An item made by hand later is linked at the pause:
when an item is read, items whose [[links]] name it and do not link it
are linked. This also closes the gap that undoing a text took back an
item its [[link]] had made: only make makes such items now.

Measured on 10,000 items (release): a pause that reads one new item
took 14.3 ms, 9.8 ms without the linking of waiting items.

Checked: five new tests in tests/inline_links.rs (ten in all). Breaks,
each failing a test: no linking at the pause; make without its record;
no hint; the hint shown with nothing to make; undo ignoring make
records; make that links nothing.

### A name links the links that use it (19 November 2026)

At the author's request ("Fix these limits too"): `name` now links again
each item whose [[links]] use the name, so a link waiting for that name
is made at once, and a name moved to another item moves its links. When
any link changed, an edit op relink record names the name's entry, so
one undo takes the name and its links back (format version 1 revised).
Checked: two tests in tests/inline_links.rs; breaks, each failing one:
no relinking, no closing record, undo ignoring it.

### Prolog queries (19 November 2026)

At the author's request, `query GOAL` runs a Prolog goal over the
current world's beliefs, proved as derived text is (through rules), and
says its answers by their variables, or yes or no. The goal is vetted
as a definition is, and run as limit(21, beliefs(Goal)), so a huge or
endless goal shows 20 answers and says there are more. A query writes
nothing. Checked: five tests in tests/query.rs; breaks, each failing a
test: no vetting; facts only (no rules); yes said as no. With no limit,
the many-answers test hangs (killed by a 90 s timeout) rather than
failing; a smaller limit than 1000 is not told apart from 21, as both
show 20. A `?` after a space cannot be typed in Command (it lists), so
`?- goal` cannot be typed; write the goal alone.

### The Prolog form of what was read (19 November 2026)

At the author's request, the right pane shows, under READ AS, each
statement an item's text was read as, in the Prolog the reader wrote
and the memex runs, by kind, and the item's tags. The read record keeps
those texts (`said`, format version 1 revised); a text changed and not
read again shows none. A question's goal now names its new variables
after the question's own (C, D, E), not from 1000 (M38, N38).

Checked: three tests in tests/read_as.rs. Breaks, each failing a test:
statements not recorded; a stale reading shown; tags left out; `said`
not read back; the old variable numbering.

### The guide checked against the code (19 November 2026)

Every claim of docs/guide.md and docs/tutorial.md was read against the
code. Most held. Three were faults of the code, now fixed, each with a
test that failed before the fix:

- `sealer run` stopped ("No such file or directory") when `journal.jsonl`
  did not exist yet. `make run` starts it beside the memex, which makes
  that file only when it opens, so a first session could go unsealed.
- A constraint that put `\+`, `findall` or `forall` around a fact was
  refused with an existence error: the planted check called the
  construct directly, and facts are not predicates.
- The window passed on only F1 to F12, though the configuration takes
  F1 to F35.

The guide's wrong or incomplete claims were corrected (the commit
lists each); among them, that any guide word may be typed (only `to`,
`from` and `after` may), that `unadopt` undoes a whole adoption (one
claim's), that the configuration falls through to the next file when
one is absent (it does not), that any format Pandoc reads can be
imported (six extensions), that purge leaves a hash (it leaves null, and
the blob stays), and the tutorial's silence on READ AS, `query` and
questions.

Not checked: the claims of what has been tested on real desktops.

Then checked, by a probe test (not kept): with the suggestions pane in
focus, a typed character, Enter, Up and Backspace edit the outline at
its cursor, as the design means ("it never takes the keyboard"; the
milestone 5 test types with the outline in focus). The outline's cursor
line stays reversed, but the text cursor is drawn only in the focused
pane (`shows_text_cursor`), so a key's place in the line is not shown.
The links pane, by contrast, refuses unbound keys.

The suggestions pane now refuses them too, as the links pane does
("this pane shows suggestions; F6 to edit"). Checked: the new test
a_pane_that_shows_links_or_suggestions_refuses_keys (a character,
Backspace, Up, Enter and Delete change nothing in either pane, and the
outline takes keys again after `switch`) failed for suggestions before
the change and passes after.

### A box holds one value, in English (19 November 2026)

"One value per box" could be said only in Prolog
(`false :- cell(B, X), cell(B, Y), X \= Y`), and the tutorial asked for
it so. The reader now takes "the value of X" as X's box, `cell(X, R)`,
so `Nothing has more than 1 value.` reads as that constraint, as
`Nothing has more than 1 date.` reads as one date per thing.

Checked: a reader test (`a_box_holds_one_value`) and a memex test
(`a_sentence_says_that_a_box_holds_one_value`: kept after its planted
check, then two rents give 1 violation). With the new clause taken out,
the memex test fails. The reading is new only for text read from now
on: a journal keeps what each text was read as, so old readings stand.

## 4. Where it stands

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
