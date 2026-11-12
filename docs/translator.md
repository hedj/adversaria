# Complex Koopman Reservoir: a local English–Prolog translator

10 November 2026. Exported from the living Claude Doc of the same name.

## Summary

A dedicated LFE5U-85F with one 32-bit DDR3-800 channel can run a byte-level English ↔ Prolog translator of about 9.6M parameters at about 310 bytes per second, about 90 tokens per second. The model is a Complex Koopman Reservoir: a fixed complex linear recurrence with no weights, a raw window, a hashed copy window over a page of text, and a quadratic Kolmogorov–Arnold readout.

Inference uses only additions and multiplications. A deterministic translator handles controlled English, and the model rewrites free English into it. Training costs about $15–50 in GPU time and data, plus an estimated 1–4 weeks of work on the controlled grammar and verb lexicon. Every figure here is an estimate from a paper design; nothing has been built, synthesised or trained. The only measured figures are the FPGA resource counts reported by nextpnr 0.11.1.

## Goals and constraints

The translator runs locally, with no trust in any outside party, and every output is checked before use.

| Item | Choice | Reason |
| --- | --- | --- |
| Task | English ↔ Prolog, one clause or query at a time, in both directions | Prolog on the WAM engine, a second ECP5-85F beside bcw-3, parses and runs each output on tests, and a back-translation checks its meaning, so a weak model is acceptable and the whole loop runs locally. Lean runs on neither machine. |
| Rate | about 90 tokens per second (about 310 bytes per second) | The agreed target is about 80 tokens per second; 1 s per token is the floor below which a model is not useful |
| Exact context | one dense page, about 4.3K bytes | A statement and its translation fit with room to spare |
| Weights | 8 bits | Close to lossless after training, so no quantisation-aware training |
| Memory | one 32-bit DDR3-800 channel | A standard bus; about 65 pins, fits the CABGA381 package |
| Device | a dedicated LFE5U-85F, beside bcw-3 | 208 DP16KD, 156 MULT18X18D, 83,640 LUT4 (nextpnr 0.11.1) |
| Arithmetic | additions and multiplications only | Exact integer specification is possible; no exp, softmax or square root in the datapath |
| Checker | the WAM engine: a second ECP5-85F with 1 GB of DDR3 and two 512 GB A2-class SD cards, index and data, each with its own controller that uses the card's command queue, joined to bcw-3 by a dedicated 32-bit link at 100 MHz; the only place Prolog runs | Room for the interpreter and its test data; no outside party is needed to check an output |

Outsourcing to untrusted servers was considered and rejected for this task. With one untrusted party, confidentiality needs homomorphic encryption, whose cost in traffic and computation is not estimated here. A local model of this size avoids the question.

## Model structure

The model reads one byte at a time. A recurrence with fixed weights keeps the long-range statistics, two windows keep the exact recent text, and one learned polynomial readout combines them.

*Diagram, kept in the living doc only: Complex Koopman Reservoir: data flow for one byte.*

The reservoir's state is updated as s = λ·s + (1 − |λ|)·\[x = v\] for each byte value v and each of the 12 complex eigenvalues λ. The eigenvalues lie on a log scale of time constants, and no gradient flows through them.

| Part | Parameters | Why this choice |
| --- | --- | --- |
| Fixed complex reservoir (256 × 12) | 0 | Koopman eigenvalues keep both magnitude and phase; fixed weights make training fully parallel, with no backpropagation through time |
| Raw window (8 bytes, one-hot) | 1.05M | Exact local order. Because x² = x for one-hot inputs, the squares are dropped. |
| Quadratic KAN input layer (\[Re s, Im s, squares\] → 512) | 6.29M | A quadratic KAN layer is a linear layer on \[x, x²\]; its squares give \|s\|² and, after a mix, gated products |
| 4 quadratic KAN mixers (512 → 512) | 2.10M | Each layer doubles the degree of the polynomial |
| Output layer (512 → 256) | 0.13M | Byte vocabulary: no large embedding table |
| Copy window projections (query and keys) | about 0.07M | Exact recall of identifiers over a page |
| **Total** | **about 9.6M** |  |

The exact context is the 8 raw bytes plus the copy window. Beyond it the reservoir keeps only blurred statistics, out to about 2K bytes at the longest time constant.

## Accelerator design

The accelerator is organised around one fact: streaming the weights from DDR3 sets the speed, so the weights are stored in the order they are used and the stream never stalls; the multiply-accumulate array is sized to keep pace with it.

| Unit | Work per byte | Resources (estimate) |
| --- | --- | --- |
| Reservoir: 3,072 complex states at 18 bits, one complex multiplier (Gauss's 3-multiply form) | 3,072 complex multiply-adds, about 32 µs | 3 MULT18X18D, 6 DP16KD, about 300 LUT4 |
| Feature quantiser: Re and Im to 8 bits, with their squares | 6,144 values and 6,144 squares | 2 MULT18X18D, about 200 LUT4 |
| Weight streamer: one DDR3 controller and a clock-crossing FIFO | about 8.6 MB | 3–6K LUT4, 4–8 DP16KD |
| Multiply-accumulate array: 32 lanes of 8 × 8 | about 8.6M multiply-adds | about 2.5K LUT4; accumulators in 2–4 DP16KD |
| Per-layer unit: residuals and 5 RMSNorms (reciprocal square root by a table and 2 Newton steps) | about 3K operations | 2–4 MULT18X18D, about 400 LUT4 |
| Copy window: 2 heads × 48 values at 8 bits, a 16-bit hash of the last 4 bytes, the byte | about 413K multiply-adds and one argmax, about 34 µs | about 186 DP16KD, 128 MULT18X18D, about 2K LUT4 for the hash filter |
| Output head: 512 → 256 logits and argmax | 131K multiply-adds | 4–6 DP16KD |

Optimisations that the structure gives:

1. The raw window is one-hot, and x² = x for 0/1 values, so its square terms drop out: about 1.05M fewer parameters.
2. One-hot inputs to a linear layer are row lookups: 8 rows of 512 are gathered and summed, about 4K additions instead of about 1M multiply-adds.
3. The quadratic KAN layer a + Bx + C(x⊙x) is a linear layer on \[x, x²\], so it runs on the same lanes as every other layer.
4. The copy pointer takes a hard argmax over the scores that pass the hash filter, so no softmax or exp is needed.
5. Every byte takes a fixed number of cycles, whatever the data, apart from DDR refresh.

The interface to bcw-3 is a link that carries one byte in and one predicted byte out, plus a reset that wipes the reservoir, the windows and the keys in about 1K cycles. A dedicated 85F lies outside the hardware the book proves, unless the book specifies it and proves a refinement for it.

## Budget and throughput

The design uses all of the block RAM and most of the multipliers, but under a fifth of the LUTs. DDR3 bandwidth and arithmetic are balanced: at 32 lanes the arithmetic takes about 2.8 ms of each 3.2 ms step.

| Resource | Used (estimate) | Available on the LFE5U-85F |
| --- | --- | --- |
| DP16KD | about 208: copy window about 186, other units about 22 | 208 |
| MULT18X18D | about 135–140 | 156 |
| LUT4 | about 9–14K | 83,640 |
| DDR3 pins | about 65 | about 205 user I/O on CABGA381 (from memory) |

| Measure | Value |
| --- | --- |
| Bytes read per step | about 8.6 MB: 8.55M weights at 8 bits, plus scales. The raw window's 1.05M weights are gathered, 8 rows of 512 (about 4 KB) a byte |
| Effective DDR3 bandwidth | about 2.7 GB/s (85% of 3.2 GB/s) |
| Rate | about 310 bytes per second, about 80–100 tokens per second |
| Multiply-adds | about 2.7 × 10⁹ per second needed (8.6M × 310); 32 lanes at 95 MHz give 3.0 × 10⁹ |
| Step time | about 3.2 ms; the arithmetic takes about 2.8 ms of it at 32 lanes |

Sentences that the controlled grammar parses skip the model, so 310 bytes per second is the rate only for free English. What share of real input the grammar parses is not yet measured.

Alternatives considered:

| Option | Rate (bytes per second) | Cost or risk |
| --- | --- | --- |
| 3-bit weights | about 800 | needs quantisation-aware training, and about 75 lanes to keep pace |
| Input layer factored through rank 128 (about 4.9M parameters) | about 700 | the low-rank bottleneck may cost quality |
| Two 32-bit DDR3 channels | about 620 | about 115–130 pins, the CABGA756 package and a custom board, and about 56 lanes |
| One 16-bit DDR3 channel (common on 85F boards) | about 155 | about 45 tokens per second, half the target |
| **One 32-bit DDR3 channel, 8-bit weights (chosen)** | about 310 | a board with 32-bit DDR3 must be found |

## Training and data

The model stays at 9.6M parameters but does a narrower job: it rewrites free English into the controlled English that a deterministic translator handles. Training costs about $15–50 in GPU time and data. The largest cost is about 1–4 weeks of engineering on the controlled grammar and the verb lexicon (a guess).

| Direction | Handled deterministically | Left to the model |
| --- | --- | --- |
| Prolog → English | All of it, given a lexicon of verb frames: each predicate and its places fill a template ("X sends Y to Z") | Predicates with no lexicon entry, at a guess a few per cent |
| English → Prolog | Sentences that the controlled grammar parses | Free English outside the grammar, rewritten into controlled English, which the deterministic path then translates |

The extra capacity of 9.6M parameters, against about 3M for this task alone, goes into the hard part: reading free English and choosing the right controlled phrasing. The grammar keeps the output exact, and the Prolog checker catches what slips through.

The reservoir is fixed, and no learned layer feeds back into it, so no gradient flows through the recurrence. Each position trains independently, as in a feedforward network, with no scan kernel. Compute follows the rule 6 × parameters × bytes, with N = 9.6M.

| Item | Assumption | Cost |
| --- | --- | --- |
| Pretraining compute | 6 × N × D, N = 9.6M, D up to about 19B bytes (100 times compute-optimal), on H100-class GPUs at about 100–300 TFLOP/s effective | about 1–4 hours: about $3–12 |
| Fine-tuning | about 1B bytes of generated pairs, a few epochs | about $1 |
| Generated pairs | about 250M tokens (about 1B bytes) from an open 8B model at 5–20K tokens per second | about 3–14 GPU-hours: about $10–40 |
| Checking the pairs | parse each output, translate it, and run its Prolog on tests against the original's, on a local CPU | negligible |
| Controlled grammar and verb lexicon | an ACE-like grammar (a DCG of perhaps hundreds to a couple of thousand lines) and verb frames for each domain | about 1–4 weeks of engineering (a guess) |
| **Total, excluding engineering time** |  | **about $15–50** |

Corpora (checked on their pages unless marked):

| Corpus | Direction | Size | Licence |
| --- | --- | --- | --- |
| [GSM8K-Prolog](https://huggingface.co/datasets/Thomas-X-Yang/gsm8k-prolog) | arithmetic word problems → Prolog | 7.47K train, 1.32K test | MIT |
| GeoQuery | English questions → Prolog queries | 880 (not checked) | not checked |
| SWI-Prolog library and its PlDoc comments | Prolog ↔ English | not counted | not checked |
| The book's prose | controlled English (STE), for pretraining the English side | about 108K words | the author's |

To cut the cost of the rest:

1. Extract pairs from documentation, the PlDoc comments beside SWI-Prolog's predicates, for free.
2. Manufacture pairs from the deterministic path: Prolog → controlled English by the lexicon, then an open model of about 8B parameters rewrites each sentence into 2–3 free-English variants. Each variant paired with its Prolog is one training example.
3. Generate synthetic pairs from a grammar: a fixed set of English templates over facts, rules and queries, each with its Prolog. This costs nothing but covers only the templates.
4. Check each generated pair by running its Prolog on test queries, and by translating it back and comparing.
5. Add the model's own outputs that pass the checks.

Training on untrusted servers is safe. Neither data nor weights are secret, and every output of the finished model is checked.

## Restricted input languages: what to borrow

The cheapest gain is to accept only a controlled English in which each sentence has one meaning. The model then shrinks to a normaliser from free English into that subset, and a deterministic grammar does much of the rest.

| System | Restriction on the input | Translates into | Idea to borrow |
| --- | --- | --- | --- |
| [Attempto Controlled English](http://attempto.ifi.uzh.ch/site/main.php?text=description/description.html) (Zurich, since 1995) | ["Each ACE sentence has only one meaning"](http://attempto.ifi.uzh.ch/site/docs/ace_interpretationrules.html): prepositional phrases attach to the verb, relative clauses to the preceding noun, a pronoun to the most recent accessible noun phrase, "a" is existential. [Construction rules](http://attempto.ifi.uzh.ch/site/docs/ace_constructionrules.html): a passive needs "by", phrasal verbs are hyphenated ("waits-on"). | Discourse representation structures, then first-order logic, TPTP, OWL | Fixed interpretation rules remove ambiguity without a large grammar. The Horn-clause part of first-order logic is Prolog, so a deterministic path can cover what ACE parses (my inference). |
| [Lojban](https://en.wikipedia.org/wiki/Lojban) (Logical Language Group, 1987) | An unambiguous grammar, parsable by a parsing expression grammar; each predicate has a fixed place structure | Parts of it into predicate logic | Fix each verb's argument order ("X sends Y to Z"), so a verb maps to a Prolog predicate of known arity |
| [Simplified Technical English](https://en.wikipedia.org/wiki/Simplified_Technical_English) (ASD-STE100) | About 900 approved words; one word, one part of speech, one meaning; at most 20 words in an instruction and 25 in a description | (a writing standard) | The book already enforces part of it: `tools/ste.py` limits sentences to 25 words, bans semicolons and allows one word per action |
| [Inform 7](https://ganelson.github.io/inform-website/) (2006; open source since 2022) | Natural-language syntax for programs | Interactive fiction | A controlled English can carry a whole programming language |

Restrictions worth adopting, in order of value:

1. One meaning per sentence, by ACE's interpretation rules. This makes parsing deterministic.
2. A fixed argument order per verb, as in Lojban. Each verb then names a Prolog predicate of known arity, and each noun phrase fills one of its places.
3. The book's STE rules: at most 25 words, one word per action.
4. A deterministic translator for the sentences the controlled grammar parses, with the model only for the rest and for Prolog → English.

Logical English (Kowalski and Dávila) and PENG (Schwitter) compile controlled English to Prolog and are the closest prior work, but their pages could not be reached, so they are not assessed here.

## Conversational loop

The model proposes a sentence in Attempto Controlled English (ACE), and the user confirms it before anything is translated. The user sees APE's own restatement of what it parsed, not the model's sentence, so a wrong parse shows. A round should take about 0.4–1.2 s on the WAM engine, set mostly by the model's proposals (see the memex design's Timing estimates).

*Diagram, kept in the living doc only: Conversational loop: one round of 'do you mean?'.*

1. The user types free English.
2. The model proposes up to k ACE sentences. APE parses each, and the first that parses is kept.
3. APE restates its parse in ACE, and the user sees "Do you mean: …?". When APE cannot restate a parse, the ACE sentence itself is shown.
4. The user accepts, edits the ACE directly, or rejects it, which brings the next candidate.
5. An accepted parse is translated to Prolog deterministically, and its tests run on the WAM engine.
6. Each accepted pair of free English and ACE is logged, for retraining offline. These pairs are real input confirmed by the user, better than the synthetic ones.

Measured on 711 sentences of the first 200 GSM8K-Prolog problems, with APE and the full Clex lexicon:

| Input | Sentences APE accepts | Problems APE accepts |
| --- | --- | --- |
| As written | 20 (2.8%) | 0 of 200 |
| After a rewriting step (present tense, "12 dollars" for "$12", "for each" for "per", no filler adverbs) | 40 (5.6%) | 0 of 200 |

Of 15 sentences newly accepted after rewriting, APE restated 12 faithfully, 1 with a wrong reading of scope, and could not restate 2. So free English needs the model for nearly every sentence, and the restatement is what catches its mistakes.

| Step of one round | On the WAM engine (estimate) |
| --- | --- |
| Model proposes about 100 bytes of ACE | about 0.4 s per candidate |
| APE parse | about 3–8 ms |
| APE restatement | about 2–6 ms |
| One round, one to three candidates | about 0.4–1.2 s |

The model stays untrusted: it only proposes, and the user, who knows what was meant, confirms.

### Proposing a formulation: alternatives to the model

A neural model is not the only way to propose the controlled form for "do you mean?", and it is probably not the best first one. A predictive editor and a memory of confirmed pairs teach the system better and need no accelerator. The model remains the best guesser for new free English, so it fits as a fallback. Only the rewriting rules are measured; the rest are estimates.

| Mechanism | How it proposes | Free English it covers | What it teaches | Cost | Latency |
| --- | --- | --- | --- | --- | --- |
| Predictive editor | offers only the next words the ACE grammar accepts, as the user types, as in ACE's and PENG's editors | none: the user writes controlled English, guided | the most: every keystroke shows what is accepted | the grammar's lookahead in APE, a few hundred lines; no new hardware | per keystroke; fast with the WAM engine |
| Memory of confirmed pairs | finds the most similar past input (word overlap or BM25) and adapts its ACE by swapping in the new names and numbers | grows with use, and is personal to the user | good: proposals resemble the user's own accepted sentences | an index and slot filling, about 0.5–1K lines, on bcw-3 | milliseconds |
| APE's repair hints | APE marks where parsing failed and suggests a repair; the nearest edits are offered | small: errors near the subset | good: shows which word broke the sentence | small: it already exists | about one parse |
| Rewriting rules | present tense, "12 dollars" for "$12", fillers dropped | small: 2.8% to 5.6% accepted on GSM8K (measured) | weak: the fixes are invisible | about 200 lines (written) | microseconds |
| Intent classification and slot filling (small statistical classifiers) | picks one of a fixed set of commands or question forms and fills its slots | good for the shell's 30–50 verbs and fixed question forms; poor otherwise | moderate | tiny: kilobytes of weights, on bcw-3 | milliseconds |
| The neural model (9.6M) | rewrites arbitrary free English into ACE | the broadest | moderate: shows a correct form after the fact | the model accelerator and about $15–50 of training | about 0.4 s per proposal |

Recommended order, if measurement agrees:

1. The predictive editor and APE's repair hints, to teach the subset as the user types.
2. The memory of confirmed pairs, for inputs that resemble earlier ones.
3. The neural model as a fallback, for free English that matches nothing in memory.

The order of work in the memex design follows this. To decide, log real inputs for a while and measure what share the editor, the memory and the rewriting rules handle without the model. The remainder is the model's only job.

## The WAM engine's work for APE

Prolog runs only on the WAM engine, a second ECP5-85F: bcw-1's peripherals, which bcw-3 adopts, require each process to fit its region whole, and no worker addresses external memory. Running APE compiled ahead of time, the engine should bring one parse plus its restatement to about 5–13 ms (see the memex design's Timing estimates). The model then sets the time of a round. The engine joins bcw-3 by a dedicated 32-bit link at 100 MHz and carries 1 GB of DDR3 and two 512 GB A2-class SD cards, one for the index and one for the data, each with its own controller that uses the card's command queue. The index card holds every fact in three orderings (subject–relation–object, relation–object–subject, object–subject–relation), so any pattern of bound arguments is a prefix of one and each lookup costs a bounded number of reads; the search itself is not bounded. Fact changes wait in the DDR3 and reach the index card lazily; after power-off the engine replays them from the memex journal (see the memex design's Hardware section). These are estimates; no engine has been built.

The smallest design:

1. Compile APE (about 19K lines of Prolog, LGPL-3.0) and a lexicon for the domain, taken from Clex, offline into Warren Abstract Machine (WAM) code. Clauses and queries made at run time, such as translated sentences, still need a small compiler or interpreter on the engine (an open point below).
2. Tokenise in native code on bcw-3, not in Prolog: the profile below puts about 38% of APE's time there.
3. A WAM engine built for backtracking: tagged 32-bit words, with units to dereference, unify, and push the trail and choice points, at about 2–4 cycles per WAM instruction.
4. The heap, stacks and trail in DDR3, with their tops held in block RAM.
5. The lexicon as a hash table in DDR3: one lookup per word, instead of probing each of about 13 word classes in turn.

| Resource | Use (estimate) | On the engine's ECP5-85F |
| --- | --- | --- |
| LUT4 | about 8–15K (not synthesised), plus two SD controllers and the link | fits: 83,640 available |
| DP16KD | about 22 for stack tops and caches | fits: 208 available |
| MULT18X18D | 0–2 | fits: 156 available |
| DDR3 | about 5–20 MB: WAM code, lexicon, heap | fits in the engine's 1 GB, beside the knowledge base's working set |

Timings for APE on the engine are in the memex design's Timing estimates. The engine estimate assumes about 12.5–30M WAM instructions per second: a clock of about 50–60 MHz at 2–4 cycles per instruction; the tv80 and PDP-11 cores measured 45–66 MHz on the ECP5. Loading Clex (97K entries), about 0.42 s on the Xeon, costs nothing on the engine: it is compiled ahead of time into DDR3, perhaps 10–20 MB.

### Where APE spends its time

APE spends most of its time in a grammar that backtracks heavily, not in the lexicon, so the engine must make choice points and the trail fast. This was measured on the 711 GSM8K sentences, with the full Clex lexicon, by APE's own phase timers and SWI-Prolog's profiler.

| Phase | As written (s) | After rewriting (s) | Per sentence, after rewriting | Share |
| --- | --- | --- | --- | --- |
| Tokenise | 1.056 | 0.682 | about 0.96 ms | about 38% |
| Parse | 0.809 | 1.081 | about 1.52 ms | about 61% |
| Anaphora resolution | 0.018 | 0.019 | about 0.03 ms | under 1% |
| Restate, accepted sentences only | 0.021 for 20 | 0.043 for 40 | about 1.07 ms | (extra, per accepted sentence) |

| Group (rewritten run; the 30 costliest predicates cover about 55% of the time) | Share of profiled time | Main predicates |
| --- | --- | --- |
| Grammar | about 34% | `word_capitalize` 6.9%, `determiner` 5.4% (343K redos), `word` 3.6%, `get_position` 3.3%, `np` 2.4% (1.45M redos), `specifier` 1.9% |
| Lexicon | about 15%; about 25% as written, where unknown words are probed in every word class | `noun_mass` 2.5%, each of about 13 word classes about 1% (60–100K calls each); `is_contentword` 3.5M redos |
| Text handling | about 6%; about 13% as written | `atom_codes` 3.5% (1M calls as written), `convert_ascii_char` 3.7% as written |

What this means for the engine:

- Native tokenising removes about 38% of the work before any hardware.
- The hash lexicon removes about 15–25% more.
- What remains is general WAM execution with deep backtracking, in both the parse and the restatement. A unit for the lexicon alone would not be enough.

The profiler samples, its top 30 predicates cover only about 55% of the time, and most of the sample is failed parses, whose cost was not separated from accepted ones.

This is a new mechanism. Like the model, it lies outside the hardware the book proves unless the book specifies it, and it needs the author's approval. Clex is GPL-3.0, which matters if the lexicon is distributed with the device.

Open points:

- [ ] Which SWI-Prolog built-ins does APE use, and what does porting them to the engine cost?
- [ ] What does a small compiler or interpreter for clauses and queries made at run time cost on the engine?

## A knowledge-base chatbot

The same model and engine can answer questions about the world if the knowledge lives in Prolog facts, not in the weights. A model of 9.6M parameters holds only about 2.4 MB of knowledge, at roughly 2 bits per parameter (a figure recalled from published scaling studies, not checked). That is a few hundred thousand simple facts at most, against more than a billion statements in Wikidata (from memory).

| Approach | Knowledge held | Rate | Quality | Fit with bcw-3 |
| --- | --- | --- | --- | --- |
| Knowledge in the weights, 9.6M | about 2.4 MB | about 310 bytes per second | fluent on narrow topics, wrong or invented elsewhere | poor: answers cannot be checked |
| A larger model, knowledge in the weights (3B at 4 bits) | broad | about 1–2 s per token | useful | too slow, and needs a wider memory bus |
| The small model plus retrieved text in the copy window | as much text as storage holds; a compressed English Wikipedia is several GB (from memory) | about 310 bytes per second, plus search | good for looking things up, weak at reasoning | needs storage of several GB and an index |
| **Facts in Prolog, reached through the conversational loop (chosen)** | a Wikidata subset of about 25M facts, about 0.8 GB (below) | one round of "do you mean?", then a query | exact where the base covers the question; an explicit "no answer" where it does not | the same model and engine, and every answer can be checked |

A question takes one turn of the same loop:

1. The user asks in English. The model proposes an ACE question, and the user confirms APE's restatement.
2. The question becomes a Prolog query against the knowledge base, run on the WAM engine through the ordering that has its bound arguments as a prefix.
3. The answer comes back as facts, which APE phrases in ACE. The user sees exactly what the base says, and which facts it rests on.
4. The model phrases the answer fluently from the facts in its copy window. APE parses that phrasing back into ACE, and Prolog checks that every claim is among the returned facts before the user sees it.

Limits:

- The chatbot answers only what the base holds. It can give the population of Montana, but it cannot discuss the plot of a film.
- Questions outside the controlled grammar need a round of "do you mean?" before they can be asked.
- The knowledge base lives on the WAM engine: subset C fits its 1 GB of DDR3 as a working set, and A and B are read from its index and data cards. Their lookup time is not measured.

### Roles in a conversation

The model specialises in conversation and in working out what the user means; the facts stay in Prolog. With the knowledge held outside, its 9.6M parameters go to fluency and intent, which a small model handles far better than world knowledge.

| Job | Who does it | Why |
| --- | --- | --- |
| Working out intent, and asking a clarifying question ("Which Paris: France or Texas?") | the model | classifying and rephrasing suit a small model |
| Fixing the meaning of the question | APE's restatement, confirmed by the user | stays exact and checkable |
| Facts | the Prolog base | exact, with an explicit "no answer" |
| Conversation state: the entities mentioned, the current topic, the user's earlier answers | Prolog facts, updated each turn | The copy window holds only about 4.3K bytes, a few turns. A compact state lets "and its population?" resolve after many turns. |
| Phrasing the answer | the model, from the facts in its copy window | fluent, but able to invent, so it is checked |
| Checking the phrased answer | APE parses it back into ACE, and Prolog confirms each claim among the returned facts | catches an invented or altered fact before the user sees it |

Limits:

- At 9.6M parameters, small talk will be simple and sometimes stilted, near the fluency of models trained on children's stories. Intent, clarification and phrasing facts are narrower, and should be good.
- The check needs the phrased answer to parse as ACE. An answer that does not parse falls back to APE's plain restatement of the facts.
- Training data shifts to dialogue. Synthetic conversations around knowledge-base queries (question, clarification, ACE, facts, answer) cost little to generate. Open dialogue corpora exist, but which ones, and under what licence, is not checked.

### The Wikidata subset

A useful subset is about 25M facts and about 0.8 GB: the roughly 1M best-known entities, about 300 common relations, and no external identifiers. Only the starting totals come from a source: about 1.78 billion statements on 121 million items, of which scholarly articles are 55.5% and astronomical objects 8.5%, leaving about 641 million ([Phabricator T437880](https://phabricator.wikimedia.org/T437880), undated). Every cut after that is an estimate.

| Subset | Facts | Facts at about 30 bytes each, in three orderings | English labels | Total | Where it lives on the engine |
| --- | --- | --- | --- | --- | --- |
| A: all but scholarly articles and astronomy, without external identifiers (assumed about 40% of the 641M) | about 256M | about 7.7 GB | about 1.8 GB | about 9.5 GB | index and data cards |
| B: items with an English Wikipedia article (about 7M, from memory), about 25 facts each | about 175M | about 5.3 GB | about 0.2 GB | about 5.5 GB | index and data cards |
| C: the about 1M best-known items, about 25 facts each, about 300 relations (chosen) | about 25M | about 0.75 GB | about 25 MB | about 0.8 GB | the cards, with the working set in the 1 GB of DDR3 |
| D: a pocket base, about 100K items × 20 facts | about 2M | about 60 MB | about 2.5 MB | about 63 MB | DDR3 |

- A fact is a relation (2 bytes), a subject (4 bytes) and an object (4 bytes, or a reference to a value): about 10 bytes, stored in three orderings (subject–relation–object, relation–object–subject, object–subject–relation) so that any pattern of bound arguments is a prefix of one. Quantities and dates need about 8 bytes more. Every subset is small beside the two 512 GB cards.
- Questions concentrate on well-known entities, so the top 1M items should cover most everyday questions. This is an assumption, not a measurement.
- Each of the about 300 relations needs a verb frame in the lexicon, so the relation list also bounds the lexicon work.
- Wikidata is CC0 (from memory), so a subset can ship with the device.

Open points:

- [ ] What share of a sample of expected questions does subset C answer, and which relations do the answers use?
- [ ] How are relations named, so that each verb frame in the lexicon maps to a predicate of the base?
- [ ] What random-read rate do the engine's cards give with the command queue, measured, and so how long does a lookup take?

## Suggesting facts, rules and constraints

The model has a further purpose: it suggests formal statements for the user to accept, so the user writes less Prolog. This meets the memex's main risk, that a knowledge base stalls when entering knowledge costs more than it returns. Wikidata already supplies facts about the public world; the suggestions cover the rest.

| Suggestion | Drawn from | Checked by |
| --- | --- | --- |
| A fact: a mention linked to its Wikidata item (“Buran” to its item), or a claim restated as a fact | the user's sentence and the label index | the “do you mean?” round: APE restates it, and one key accepts it |
| A constraint | Wikidata's property constraints (single value, value type, conflicts with; property P2302, recalled and not checked), offered when an item uses the property; or a constraint pattern that the model fills from the user's sentence (“an event has one date”) | the constraint runs over the store at once, so the suggestion shows the violations it would raise before it is accepted |
| A rule | rules mined from the facts, as AMIE mines them from knowledge bases (Galárraga et al. 2013, recalled and not checked), chosen and phrased by the model | each rule shows its support and its exceptions in the store |
| A reference: a source for a claim | works already in the knowledge base, found by the citation graph (works that the claim's other sources cite, or that cite them; co-citation and bibliographic coupling, after Small 1973 and Kessler 1963, recalled), by main subject (P921, recalled), and by the claim's words against titles; chosen and ranked by the model | the suggestion shows why it was offered (the shared citations or subject) and opens the source to read; adopting it adds a `cites` fact. The model never writes a reference: it only chooses among works that exist in the store, so it cannot invent one |

Three rules keep the suggestions honest. A suggestion enters the journal only when the user accepts it. Each is shown restated in controlled English, with its evidence: support, exceptions and violations. And each comes from a fixed pattern that the model chooses and fills, not free Prolog, which a model of 9.6M parameters is unlikely to write reliably (an estimate, untested). The journal records every acceptance and refusal, which measures the acceptance rate at no cost and gives training pairs for the gaps.

The model runs all the time, over whatever the user is working on, and keeps a pane of suggestions up to date. One click adopts a suggestion and another unadopts it: adopting writes an entry to the journal and unadopting retracts it, so both are ordinary edits under universal undo, and the dataflow graph recomputes what depends on them. Pointing at a suggestion previews it in place: the violations it would raise or clear, and the items it would change. The pane follows the rules for a system that offers help unasked, after Horvitz's principles of mixed-initiative interfaces (1999, recalled and not checked): it never takes the keyboard or moves what the user is looking at; it never reorders a suggestion under the pointer; it ranks by evidence and keeps few suggestions in view; and each dismissal is recorded, so a refused suggestion does not come back unless its evidence changes.

## What-if questions: counterfactual simulation

The chatbot answers "what if" by supposing a condition in a separate world, setting aside the facts that contradict it, and then answering or simulating forward. It can simulate only worlds whose rules of change are written down, by the user in ACE or by an engineer as a library. "What if the reservoir loses 5% a day?" is in scope; "what if Napoleon had won?" is not. This is a design sketch; nothing in it is built or measured.

### Simulation in the style of ROSS

| Idea from RAND's ROSS | Role here |
| --- | --- |
| Objects with attributes | simulation objects, with their initial attributes taken from the knowledge base |
| Behaviours fired by message patterns | rules of change as Prolog clauses, such as "Every day, the tank loses 5 litres." |
| Scheduled messages ("plan …", from memory) | a queue of events in logical time; each event runs its behaviours, which may schedule more |
| Reruns with changed conditions | the baseline and the supposition run in sibling worlds, and the answer is their difference |

Time is logical only, so `system.no-clock` holds and the same question always gets the same answer. An inverse question, such as "what price makes the profit at least 100 dollars?", is a constraint problem, not a simulation, and uses the constraint-Prolog extension.

### Worlds

Each supposition runs in a world, after VPRI's Worlds (Warth, Ohshima, Kaehler and Kay, 2011). A child world reads through to its parent and keeps its own writes, so storage grows only with the changes, not with the 0.8 GB base.

| Worlds idea | Role here |
| --- | --- |
| `sprout` | a supposition, from the knowledge base or from the current scenario |
| Sibling worlds | baseline and supposition side by side; Prolog's backtracking cannot hold both outcomes at once |
| Nested worlds | "and suppose also …", stacked on the first supposition |
| `commit` | only when the user says a supposed fact of their own is really true; never into the shared base |

A world is a small table of facts added and removed. A lookup checks the current world, then its parent, up to the base, and the planned hash lookup gains a world tag. Every answer inside a scenario is labelled, so a hypothetical is never taken for a fact.

In the memex a world is a branch of the journal's undo tree (see the memex design): Suppose sprouts a branch, undoing and then editing sprouts another, each named by its first change, and commit makes a branch the actual world. Reversing one earlier decision, such as an adopted suggestion, is a retraction on the current branch, not a new world.

### Counterfactual revision

Before it answers, a supposition sets aside the facts that contradict it, as belief revision does: remove what contradicts the condition, then add it (the Levi identity of AGM revision, from memory).

*Diagram, kept in the living doc only: Counterfactual revision: one supposition.*

1. Sprout a world W, and impose the supposition in it.
2. Run the integrity constraints that touch what changed. A violation names a conflict set.
3. From each conflict set, set aside the least entrenched fact in W. When the ordering does not decide, ask the user through the "do you mean?" loop: "Should Paris still be the capital of France?"
4. Repeat until no constraint is violated, then answer or simulate in W, and report what was set aside.

| Design choice | Recommendation | Why |
| --- | --- | --- |
| What counts as a conflict | explicit integrity constraints: single values, disjoint classes, the order of dates | without constraints, nothing conflicts. Wikidata publishes property constraints that can be harvested (from memory) |
| Derived facts | store only base facts, and derive the rest by rules at query time | a derived fact then follows its world, with no truth-maintenance system: true by structure |
| Which fact goes | an ordering of entrenchment: class and type facts over specific facts, the user's statements over the base; ask when it is a tie | a counterfactual rarely has one right revision |
| Arithmetic conflicts | constraint Prolog finds them, and a minimal-conflict search (QuickXplain, from memory) picks a set to drop | ties in with the constraint extension |
| Cost | check only the constraints that touch what changed, through the indexes | most conflicts are local: a single-valued fact replaces the old value |

| Work (estimate) | Size |
| --- | --- |
| Grammar for suppositions, time and rates ("Suppose that …", "every day", "by 2%") | about 1–2K lines, with restatement |
| Simulation kernel and worlds in Prolog | a few hundred lines |
| Revision loop | a few hundred lines |
| Integrity constraints for the about 300 relations | harvested where Wikidata has them, written otherwise |
| Domain behaviour libraries (populations, money, stocks and flows, queues) | open-ended: the main cost, and what decides what can be simulated |
| New hardware | none: the checks and the simulation are queries on the WAM engine |

### Prior work, and a trial of s(CASP)

No existing system found does all of this, but s(CASP) with Logical English covers enough to replace three parts of it: the constraint extension, the search in the revision loop, and the explanations.

| System | Overlap | What it lacks here |
| --- | --- | --- |
| [s(CASP)](https://arxiv.org/abs/2009.10238) with [Logical English](https://ceur-ws.org/Vol-3193/paper5GDE.pdf) | goal-directed answer-set programming with constraints; justifications rendered in English; counterfactuals over "multiple possible worlds" ([Dasgupta et al. 2023](https://arxiv.org/abs/2310.14497v1)) | dialogue, a general knowledge base, simulation in time |
| Cyc ([OpenCyc introduction](https://www.qrg.northwestern.edu/nextkb/IntroOpenCycOnt.pdf)) | microtheories hold alternate hypotheses and fictional worlds; more specific arguments are preferred | huge and proprietary; no confirmation loop with the user |
| [RACE](https://attempto.ifi.uzh.ch/site/courses/files/ACE.Course.UniZH.1206.Reasoning.pdf), Attempto's reasoner | consistency checking and question answering over ACE | lists hypothetical reasoning ("What happens if …?") as future work |

Three questions were run with s(CASP) 1.1.4 on SWI-Prolog 10.0.2. Base facts hold unless set aside, and setting one aside is an abducible.

| Question | Result | Time per answer |
| --- | --- | --- |
| "Suppose Paris is in Germany. Is it still the capital of France?" | No: it sets aside "Paris is in France" and "Paris is the capital of France"; Lyon stays in France | about 6–10 ms |
| "Suppose the tank loses 8 litres a day, not 5. How much is left after 10 days?" | 420 litres, against 450 without the supposition, with `set_aside(loss5)` reported | about 2–59 ms |
| "What daily loss empties the tank in exactly 20 days?" | 25 litres, by constraints over rationals | about 5 ms |

Findings:

1. Global constraints with variables were not enforced: `:- capital_of(C, X), not in_country(C, X).` let Paris stay capital. The same constraint written for Paris and France was enforced. The cause is a known, open bug in the default algorithm for universal quantification, \[issue #13\](https://github.com/SWI-Prolog/sCASP/issues/13): negating a body with two linked variables over two or more facts checks only some combinations of values. Running with \`--forall=sasp\` or \`--forall=prev\` fixes it (below).
2. s(CASP) does not choose a minimal revision: in two queries it also set aside "Lyon is in France". Offering only the facts in conflict sets as revisable, and preferring the answer that sets aside fewest, remain this design's work.
3. Justifications in English come from one-line directives, such as "after 2 days the tank holds 484 litres, because after 1 days the tank holds 492 litres … we set aside the fact loss5". They are readable but stilted, and not ACE.
4. s(CASP) runs on Prolog with CLP(Q), so the WAM engine would need attributed variables. That cost is not estimated.

The bug was isolated by changing one thing at a time. A constraint with variables over one fact was enforced in all 6 forms tried. It failed once a second fact was added, and only when the constraint joined two variables. Without any constraint, `bad :- p(X, Y), q(X, Y).` with the facts `p(a,x)`, `p(b,y)` and `q(a,x)` let `not bad` succeed, checking `not p(a, y)` instead of every Y. Constraints written for one entity work because they have no variables.

| `--forall=` | Failing cases answered correctly | The earlier trial (Paris, tank, inverse) |
| --- | --- | --- |
| `all` (the default) | 0 of 5 | Paris wrongly stays capital |
| `all_c` | 0 of 5, with up to 8,512 spurious models | not run |
| `prev` | 5 of 5 | correct; two answers to "no longer the capital" |
| `sasp` | 5 of 5 | correct; same results and times (2–66 ms) |

Use `--forall=sasp` until issue #13 is fixed. The lack of a minimal revision is a separate problem and remains.

Limits:

- A conflict that no constraint expresses is never found, so the counterfactual is only as good as its constraints.
- Revisions can cascade, and each tie is a question to the user, so a far-fetched supposition costs many questions.

Open points:

- [ ] The full Worlds paper's rules for commit and conflicts (only abstracts and summaries were read).
- [ ] Whether a fact removed in a child world masks the parent's entry in all three orderings cleanly.
- [ ] How deep suppositions nest in real conversations, which sets the cost of each lookup.
- [ ] What do attributed variables and CLP(Q) cost in the WAM engine?

## A natural-language shell

The same loop can drive a shell: the user types a command in English, and its effects are shown and confirmed before anything changes. This is close kin to the fourth-generation languages and English database front ends, such as Intellect, of the early 1980s. Their history, recalled here from memory and not checked, gives the lessons.

| Lesson from the 4GL era | How this design fares |
| --- | --- |
| Habitability: users could not predict which English was accepted, and hit invisible walls | Better: the model rewrites free English into the subset, and "do you mean?" shows what was understood |
| Brittle at the edge of the domain | The same: commands work only within the facts and rules written down, but "no answer" is honest |
| The survivors were declarative cores: SQL and spreadsheets | ACE translated to Prolog is such a core, and an expert can use it directly |
| Performance surprises: a short query could expand into a costly plan | The same risk: step limits and bcw-3's bounded-time rules must cap a search |
| Lock-in to one vendor's language | Avoided where possible: ACE, Prolog and s(CASP) are open, and the facts are CC0 |

A shell adds side effects, which the 4GLs guarded only with "are you sure?". Worlds give a dry run:

1. The command runs first in a sprouted world, so its effects are only recorded there.
2. The world's changes are restated in ACE: "This will delete 3 files: …".
3. The user confirms, and the world is committed: `commit` becomes "execute". Or the world is discarded.
4. The shell's requests still go through the room's capabilities, so even a confirmed command reaches only what the room holds (`design.isolation`).

A dry run covers only effects that a world can capture: files, facts, and messages queued but not yet sent. An effect that leaves the machine, such as a byte already on the serial line, cannot be undone, so it must be held back until the commit.

## Fit with a cognitive augment

For a cognitive augment in the sense of Engelbart or the memex, the new hardware (the WAM engine and the model accelerator) makes sense only in part. The core of augmentation is the user's own material: capturing it, linking it, finding it fast and viewing it many ways. That is software on bcw-3 with its external memory, and the journal on SPI flash. The new hardware pays for reasoning with the store, for live interaction, and for questions in English. Timings are those of the memex design's Timing estimates table ([bcw-3 memex: working design](https://claude.ai/artifact/Lm8Rj4ym7PeSsEvLmzTzRt)). The history is recalled from memory; the assessment is this design's own.

| What augmentation needs (Bush, Engelbart) | What serves it here | Needs new hardware? |
| --- | --- | --- |
| A personal store of documents and notes | external memory and SPI flash, plus an index | no |
| Associative trails, made and followed by the user | links held as Prolog facts, or a plain link store | no |
| Fast retrieval, and many views of the same structure | full-text and structural indexes on bcw-3 | no |
| Responsiveness that keeps the user's flow (`design.latency`) | navigation and editing in native code within 0.1 s; dragging controls and views that run Prolog filters need the WAM engine | the WAM engine, for live updates within 0.1 s; not the model accelerator |
| Reasoning over what the user knows | Prolog queries, counterfactual revision, simulation in worlds | the WAM engine: a parse and its restatement in about 5–13 ms |
| Reaching the store in English, for occasional or untrained use | the 9.6M model and APE's restatement | yes: this is what the model accelerator is for |
| Trust in the tool | checked answers, "no answer" instead of invention, local and private | the checking is the point, but the measured 39K-line symbolic stack exceeds the 40-day audit budget, and which parts enter the book is open |
| An efficient interface for a trained user (Engelbart chose the chord keyset over the easiest device) | ACE or Prolog typed directly | no: the expert path skips the model |

The order of work this implies is kept in one place, the Order of work of [bcw-3 memex: working design](https://claude.ai/artifact/Lm8Rj4ym7PeSsEvLmzTzRt): the memex as software first, then the WAM engine, then "do you mean?" without a model, and the model accelerator last.

Open points:

- [ ] On real tasks, what share of interactions are questions in English, against navigation and editing?
- [ ] How long does each task take with the English loop, and with the expert path?
- [ ] Which parts enter the book, its proofs and its audit: the WAM engine, the model accelerator, APE, s(CASP), the memex software?
- [ ] How does one WAM engine serve every room while keeping `system.answers` and `system.no-timing-channel`? The question is kept in the memex design's open questions.

## Risks, tests and open questions

The largest risk is that the fixed reservoir, with no learned recurrence, is too weak for language; the copy task decides this before anything is built.

| Risk | Test (one variable changed; same data, size and training budget) | Null hypothesis |
| --- | --- | --- |
| A fixed reservoir is weaker than a learned recurrence | Fixed reservoir against a learned Koopman front end; translation exact-match and byte perplexity | The fixed reservoir is no worse |
| Recall at a distance fails | Copy an identifier after gaps of 8 to 4K bytes, with and without the copy window | The copy window adds nothing |
| Two 48-value heads lack precision | Copy task with 1 head of 96 values against 2 heads of 48, with and without the hash | Heads and hash add nothing |
| A quadratic readout is no better than a linear one | Linear against quadratic readout, recurrence fixed | No difference |
| Phase carries no information | Modulus \|s\|² against the complex state (Re, Im) | No difference |
| The hard argmax at inference mismatches the soft attention in training | Train with straight-through gradients; compare inference with soft and hard pointers | No difference |
| Quality is too low at this size | Exact-match on held-out Prolog clauses and queries, after the check-and-resample loop | (a measurement, not a hypothesis) |

Open questions:

- [ ] Which LFE5U-85F board carries a 32-bit DDR3 bus? The 85F boards I recall carry 16 bits, which would halve the rate.
- [ ] What efficiency does the DDR3 controller reach on that board, against the assumed 85%?
- [ ] How many LUT4 does an 8 × 8 lane take in synthesis, against the assumed 60–80?
- [ ] Does the design meet 95 MHz on the 85F? tv80 lost about 10% of its Fmax moving from the 25F (one seed).
- [ ] Is the accelerator specified and proved in the book, or kept outside it as an untrusted device whose outputs are checked?

## Sources

Pages opened for this document:

- [Attempto Controlled English: description](http://attempto.ifi.uzh.ch/site/main.php?text=description/description.html)
- [ACE construction rules](http://attempto.ifi.uzh.ch/site/docs/ace_constructionrules.html) and [interpretation rules](http://attempto.ifi.uzh.ch/site/docs/ace_interpretationrules.html)
- [Lojban (Wikipedia)](https://en.wikipedia.org/wiki/Lojban)
- [Simplified Technical English (Wikipedia)](https://en.wikipedia.org/wiki/Simplified_Technical_English)
- [Inform 7](https://ganelson.github.io/inform-website/)
- [GSM8K-Prolog](https://huggingface.co/datasets/Thomas-X-Yang/gsm8k-prolog)
- [Phabricator T437880: Wikidata statement counts](https://phabricator.wikimedia.org/T437880)
- [Warth, Ohshima, Kaehler and Kay: Worlds (ECOOP 2011)](https://link.springer.com/doi/10.1007/978-3-642-22655-7_9)
- [RAND R-3160: ROSS, an object-oriented language for constructing simulations](https://www.rand.org/pubs/reports/R3160.html)
- [RAND N-1854-1: The ROSS Language Manual](https://www.rand.org/pubs/notes/N1854-1.html)
- [s(CASP): justifications for goal-directed constraint answer set programming (arXiv 2009.10238)](https://arxiv.org/abs/2009.10238)
- [Logical English and s(CASP) (CEUR Vol-3193)](https://ceur-ws.org/Vol-3193/paper5GDE.pdf)
- [Counterfactual explanation generation with s(CASP) (arXiv 2310.14497)](https://arxiv.org/abs/2310.14497v1)
- [OpenCyc: introduction to the ontology](https://www.qrg.northwestern.edu/nextkb/IntroOpenCycOnt.pdf)
- [Attempto: reasoning with ACE (course slides)](https://attempto.ifi.uzh.ch/site/courses/files/ACE.Course.UniZH.1206.Reasoning.pdf)
- [s(CASP) issue #13: negated existential returns a false answer with a multi-variable body](https://github.com/SWI-Prolog/sCASP/issues/13)

From the bcw-3 repository: `tools/ste.py` (the book's STE checker), and nextpnr 0.11.1's resource counts for the LFE5U-25F, -45F and -85F. All other figures are estimates from this design and from memory, marked as such where they appear.
