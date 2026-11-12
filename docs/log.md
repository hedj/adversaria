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

### Sizes (counted, after the move to Rust)

| Language | Lines | Files |
| --- | --- | --- |
| Rust | 876 (381 code, 495 tests) | 3 |
| Prolog | 56 | 2 (the spikes) |
| Make | 21 | 1 |
| Shell | 21 | 1 |
| Nix | 53 | 1 |

Tests: 23 Rust unit tests; 2 spikes (tabling in `make test`, s(CASP) optional).
