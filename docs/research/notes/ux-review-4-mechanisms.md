## Verdict

A learner has to hold about 230 separate mechanisms (my own count, method below). About 60 of them belong to input alone: 5 quasimodes, each used two ways, 50 commands, 8 function keys and 8 kinds of presentation for the mouse. The burden comes less from the number than from collisions. Esc completes in one place and cancels in another. "rule", "mark", "cell", "back" and "#" each carry two to four meanings. Holding Left Ctrl turns every familiar Ctrl chord into a prefix-matched command. And five verbs take things back. Three cuts would remove about nine mechanisms and lose no capability: drop the Scope quasimode, make Suppose a pre-filled Command line, and fold `unadopt` into `retract`. The core choices hold up and should stay: quasimodes, presentations, journal-backed undo, and the two syntaxes (ACE and Prolog).

## The count

**Method.** One mechanism is one thing that has its own name or its own behaviour, which a learner must recognise or recall. I counted from the guide's tables and lists and from `COMMANDS` in `crates/memex/src/actions/dispatch.rs`. That file has 50 entries in 4 `GROUPS`, a test asserts they match (checked), and I counted the entries by hand. Categories that overlap are counted once.

| Category | Count | What it covers (source) |
| --- | --- | --- |
| Quasimodes and ways to use them | 7 | Leap forward, Leap back, Command, Scope, Suppose; hold, tap-latch (§2.3, checked) |
| Gestures inside a quasimode, and confirmations | 11 | Esc completes, `?` lists, Enter, second tap, Backspace on empty, the "release now" window, repeat Leap, Leap-or-make, the near-miss second Enter, the Suppose readings (Enter/Tab/Esc), the retract confirmation (checked) |
| Editing keys | 7 | §2.3 table (checked) |
| Function keys | 8 | F1–F4, F6–F8, F10 (checked) |
| Commands and their sub-forms | 57 | 50 names, plus `check` ×2, `import` ×3, `pane` ×4, and `palette` (checked) |
| Syntax conventions | 8 | unique prefix; `to`/`from`/`after` optional; `as` required; any other guide word must be left out; `where`, `:-`, `from` in `claim`, quoting (checked) |
| Argument and address forms | 8 | path number, `#entry`, name, text prefix; entry `12`/`#12`; suggestion N; link N for `follow`; `export … NUMBER` (checked) |
| Screen regions | 6 | top bar, two panes, foot, menu, dim hint line |
| Contents of the right pane | 10 | outline, links, suggestions, matches, interrogation, constraint, layers, revision changes, READ AS, world comparison (checked) |
| Indicators on items | 17 | ▼▶•, ‼, ?, coloured rule bullet, cyan, `[derived]`, `[source]`, `input changed`, `→`, `prose:`, `not read:`, `not yet items`, `[[` completion, struck-through or set aside, bold for a difference, cursor tint (checked) |
| Kinds of item text | 14 | prose, belief, claim, adopted claim, rule, constraint, question, derived text, timeline, view, tag, label, box, result (checked) |
| Links | 10 | types link, cites, supports, opposes, split_from, custom; `[[ ]]`, transclusion, unlinked mention, name as marker (checked) |
| Worlds | 6 | actual, supposed, revision, named empty world, set aside, "left for you" (checked) |
| Views | 28 | layer, 3 frames, 10 marks, 10 channels, 4 outline predicates (checked) |
| Kinds of suggestion | 6 | §3.4 (checked) |
| Mouse | 14 | 6 gestures, 8 presentation types (checked) |
| Plumbing | 10 | the concept and 9 built-in rules (checked) |
| Configuration sections | 4 | checked |
| **Total** | **231** | an estimate in the end, because the boundaries are my own choice |

A newcomer meets fewer. The tutorial names 17 of the 50 commands (I counted the backticked mentions with grep) and 6 function keys.

## The hardest spots

1. **Esc completes in some places and cancels in others** (checked). It completes or mends in Command, Leap, Suppose and `[[`. It cancels the Suppose readings ("Esc supposes nothing"), the retract confirmation ("Esc keeps it") and the menu. In every other tool Esc cancels (my knowledge of other tools). A user who presses Esc to escape a half-typed Suppose instead gets a word put in place.
2. **Names that collide** (checked):
   - "rule" means four things:
     - a definition (`rule`);
     - a constraint ("the item that states the rule"; the tutorial says "reads it as a rule, a constraint");
     - the view mark `rule(y(..))`;
     - a plumbing rule.
   - "mark" means a line under an item (`‼`/`?`) and also a bar or dot in a view. The guide has to say "a mark of a view".
   - `cell` is how a box is stored, a grid mark, and the screen's character cells. One recipe line uses both Prolog senses: `layer cell(...) where at(B,C,R), value(B,V)` with `value` defined over `cell(B,V)`.
   - "back" is Leap back (search upward), `back` (the trail), and "back returns to where the Leap began".
   - `#` starts a tag (`#reusable`), an item address (`#28`) and a Markdown heading. A bare `12` is an entry in `retract` but an item address in `go`.
3. **Held Ctrl is Command** (inferred from §2.3 and §2.4; not run). Familiar chords become prefix matches. Ctrl+O runs `outdent`, Ctrl+B `back`, Ctrl+H `help`, Ctrl+G `go`, and Ctrl+E and Ctrl+T try `export` and `transclude`. Ctrl+Z, Ctrl+S and Ctrl+C report "no command" or an ambiguous prefix. Every habit from Word, VS Code or a browser misfires here.
4. **Scope costs a whole quasimode for one command** (checked). "Scope matters only to `delete`". At statement and at branch, delete does the same thing, so the three steps act as two. The tutorial asks some users to move Scope to the Menu key to free Right Ctrl for Suppose. The design table promises a "group" step that does not exist.
5. **Five verbs take things back** (checked): `undo`, `retract`, `unadopt` (which "retracts the target's last adoption"), `dismiss`, and `close` (which rejects a whole revision). A revision change is rejected with `retract`; the right-click menu calls this "reject this change".
6. **The right pane is a mode** (checked). Its 10 contents are chosen partly by `pane` and partly by the cursor. With focus on it, "keys … do nothing there". §3.4 says the suggestions pane "never takes the keyboard", yet §2.3 describes it holding focus. `pane` sets the *other* pane, so what it does depends on where focus is (`show_other` in `editing.rs`, checked). This conflicts with the design's own "No modes" decision.
7. **Things only the guide reveals** (checked):
   - six structural moves with no key;
   - the guide-word rules (some optional, `as` required, the rest must be left out);
   - `export` takes NUMBER, not ADDRESS;
   - `make`;
   - plumbing;
   - pointing at an argument.
8. **Three ways to get help** (checked): `help` prints on the message line, `?` lists inside Command, and F1 opens the palette.

The tap-or-hold choice can be defended: it is for accessibility and for laptops. But a latched quasimode is a mode in Raskin's sense, and the foot is its only sign.

## Suggestions

**1. Remove the Scope quasimode.**
- **Replaces:** Scope, its key, its three target steps, and the rule "returns to statement after each command".
- **Borrows:** selection as the target (Word, VS Code); Ctrl+Backspace to delete a word (Windows, GTK).
- **Capabilities:** word delete is kept, by `delete word`, or by dragging to select and then `delete`. Drag-select already exists; today it only plumbs. Deleting an item and its branch stays the default of `delete`. Nothing is lost, because statement and branch already behave the same.
- **Net:** −4.
- **Cost:** low.
- **Risk and principle:** it drops NLS's editing by entity (N7), which the design chose on purpose. That choice is only on paper until `move` and `copy` honour Scope, so bring Scope back then. It also frees Right Ctrl.

**2. Make Suppose (and F1) a Command line opened with text already in it.**
- **Replaces:** Suppose as its own quasimode, its own set of Esc and `?` behaviours, and the special binding value `"palette"`.
- **How:** a quasimode key may name a command prefix, such as `suppose = "suppose "`. Holding or tapping it opens Command with that text. F1 becomes Command with `?` already pressed.
- **Borrows:** VS Code's palette prefixes (`>`, `@`) and Spotlight.
- **Capabilities:** the live ACE line (next words, mending, "as before") becomes the argument help of `suppose`. `ace` gets the same line, which is a gain, and `?`-after-a-word still ends a question. Hold-type-release still works, so no keystrokes are added.
- **Net:** −2. There are then two kinds of quasimode, Leap and Command.
- **Cost:** medium.
- **Risk:** low. It is still a quasimode, so Raskin's principle is kept.

**3. One verb for reversing an earlier entry.**
- **Replaces:** `unadopt`, and the extra word "reject" for revision changes.
- **How:** `retract` with no argument offers the target's latest entry that can be retracted: an adoption, a change, a layer or a constraint. Its existing confirmation lists what it will retract, and F4 runs it.
- **Borrows:** git `revert` against `reset`; Word's accept and reject.
- **Capabilities:** an item with two claims still takes two F4 presses, as today. `undo`/`redo` stay for the current branch. `dismiss` stays, because it hides a suggestion until its evidence changes, which is a different act.
- **Net:** −2.
- **Cost:** low to medium.
- **Risk:** retracting the wrong entry. The confirmation that already exists guards against it.

**4. Rename to end the collisions.** No capability changes.
- Keep "rule" for definitions only:
  - the view mark `rule` becomes `refline`;
  - plumbing rules become "plumbing patterns";
  - the tutorial says "constraint".
- Lines under items become "findings". The guide already says "a finding about it".
- "Leap back" and "Leap forward" become "Leap up" and "Leap down".
- Bare numbers always mean items. Entries are always `#N`, which removes the form `retract 12`.
- Store boxes as `box(Name,V)`, the word the prose uses. This changes stored terms, so it needs a journal format version (CLAUDE.md treats the format as a contract).
- **Net:** −1 address form; 5 collisions removed.
- **Cost:** low, except the `box` change, which is medium.

**5. Protect the familiar chords inside Command.**
- **Replaces:** the prefix match on one letter while Command is held. Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z run `undo`/`redo`, and Ctrl+F opens Leap. Any other single letter says "Command needs two letters".
- **Borrows:** every desktop application.
- **Capabilities:** the one-letter prefixes `o`, `b`, `h`, `g`, `e`, `t` still work in latched Command, or with two letters.
- **Net:** +1 (a small table), and one serious trap removed.
- **Cost:** low.
- **Risk:** low.

**6. Make the right pane an inspector without focus.**
- **Replaces:** the dead-key state, and `switch`/F6 for anything but two outlines.
- **How:** visible tabs (Links · Suggestions · Matches · Read as) that you click or choose with `pane`. Contents that follow the cursor (interrogation, constraint, layers) take sections in the inspector.
- **Borrows:** Obsidian's right sidebar, VS Code's panel.
- **Capabilities:** a click on a row jumps, as now. `pane outline` with `switch` stays for two outline panes.
- **Net:** −1 mode, and 4 contents moved from recall to recognition.
- **Cost:** medium.
- **Risk:** low.

**7. Make `help` open the F1 list.**
- **Net:** −1.
- **Cost:** low.

## Redundancies, and fixes by renaming alone

- `fold` and `unfold` could be one toggle, as in Workflowy and Org-mode. The right-click menu already says "hide or show". Net −1.
- `go TEXT`, Leap and plumbing's "find" do one job. This is defensible, since `go` is the typed form, but say so in one place.
- `next`, `previous`, `first`, `last`, `parent` and `child` have no keys. Give them a palette group of their own, "Moving", or bind them as Org-mode does.
- `fact` makes a belief, so the command and the concept disagree.
- `accept` (a suggestion) and `adopt` (a claim) are near-synonyms. Keep both, but put a cross-reference in their help.
- design.md describes Suppose as "commands act in a sprouted world" and Scope as including "group". Neither is how it is built (checked), so a reader of the design forms the wrong model.

These can be defended and should stay:
- ACE and Prolog side by side, since Prolog reaches past ACE;
- `query` beside ACE questions;
- two Leap keys, after the Canon Cat;
- presentations;
- tap-latch.

## Ranked summary

| # | Suggestion | Mechanisms removed | Capabilities kept | Effect on learnability | Cost |
| --- | --- | --- | --- | --- | --- |
| 1 | Remove the Scope quasimode | 4 | yes | high: frees a key, ends a mode with one use | low |
| 2 | Suppose and F1 as pre-filled Command | 2 | yes (and `ace` gains the live line) | high: two kinds of quasimode, one help model | med |
| 4 | Renames for rule, mark, cell, back, # | 1 (and 5 collisions) | yes | high | low (med for `box`) |
| 5 | Familiar chords inside Command | −1 (adds a table) | yes | high for people coming from other tools | low |
| 3 | One verb for reversing an entry | 2 | yes | medium | low–med |
| 6 | Right pane as an inspector without focus | 1 mode | yes | medium | med |
| 7 | `help` opens the F1 list | 1 | yes | low | low |

How each claim was checked:
- Commands and groups: counted in `dispatch.rs`.
- Behaviours: from `docs/guide.md` §2–4, `docs/tutorial.md` and `docs/design.md` §4.
- The Ctrl-chord outcomes: inferred from the prefix rule, not run.
- Tool conventions: from my knowledge of those tools.

I changed no files.