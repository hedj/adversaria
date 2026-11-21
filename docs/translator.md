# Complex Koopman Reservoir: a local English–Prolog translator

10 November 2026. Exported from the living Claude Doc of the same name.

## 1. Aims

### Summary

This document designs a byte-level English ↔ Prolog translator of about 9.6M parameters. The model is a Complex Koopman Reservoir: a fixed complex linear recurrence with no weights, a raw window, a hashed copy window over a page of text, and a quadratic Kolmogorov–Arnold readout.

Inference uses only additions and multiplications. The hardware that would run the model at speed is out of scope here. A deterministic translator handles controlled English, and the model rewrites free English into it. Training costs about $15–50 in GPU time and data, plus an estimated 1–4 weeks of work on the controlled grammar and verb lexicon. Every figure for the model is an estimate from a paper design; nothing has been built or trained. The measured figures are APE's acceptance rates and timings and the s(CASP) trials.

### Goals and constraints

The translator runs locally, with no trust in any outside party, and every output is checked before use.

| Item | Choice | Reason |
| --- | --- | --- |
| Task | English ↔ Prolog, one clause or query at a time, in both directions | Prolog parses and runs each output on tests, and a back-translation checks its meaning, so a weak model is acceptable and the whole loop runs locally. |
| Rate | about 80 tokens per second, the agreed target | 1 s per token is the floor below which a model is not useful |
| Exact context | one dense page, about 4.3K bytes | A statement and its translation fit with room to spare |
| Weights | 8 bits | Close to lossless after training, so no quantisation-aware training |
| Arithmetic | additions and multiplications only | Exact integer specification is possible; no exp, softmax or square root in inference |
| Checker | Prolog: APE and the memex's SWI-Prolog server | No outside party is needed to check an output |

Outsourcing to untrusted servers was considered and rejected for this task. With one untrusted party, confidentiality needs homomorphic encryption, whose cost in traffic and computation is not estimated here. A local model of this size avoids the question.

### Risks, tests and open questions

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

- [ ] On real tasks, what share of interactions are questions in English, against navigation and editing?
- [ ] How long does each task take with the English loop, and with ACE or Prolog typed directly?

### Sources

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

Other figures are measured in this document's trials, or are estimates from this design and from memory, marked as such where they appear.

## 2. The model

### Model structure

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

### Candidate refinements: loops and wavelets

These are proposals, not measured; the sources are recalled, not checked.

**Loop only around nonlinear parts.** Applying the reservoir's linear step n times equals one step with each eigenvalue raised to the nth power, so looping the linear part adds nothing. Looping helps only around a nonlinearity.

| Idea | Source | Where it fits | Cost |
| --- | --- | --- | --- |
| One readout block applied k times instead of k distinct blocks | Universal Transformers (Dehghani et al. 2018); looped transformers (Giannou et al. 2023; Yang et al. 2023) | the KAN mixers: parameters fall about k-fold, and so do the bytes of weights read per step | k times the arithmetic per byte; backpropagation through the loops, which costs more and can be unstable |
| Adaptive halting: easy bytes take one loop, hard ones several | adaptive computation time (Graves 2016) | the "do you mean?" round has time to spare | harder training |
| Repeated wavelet and modulus | the scattering transform (Mallat); Glinsky's Heisenberg scattering transform | the reservoir is already complex, so the modulus is natural | a nonlinearity outside the model's linear-dynamics premise |

**A wavelet pyramid over the reservoir states.** A mid/side split of adjacent pairs, mid = (a + b)/√2 and side = (a − b)/√2, is one Haar step; repeating it on the mids gives the Haar pyramid, log₂ n levels over n samples. The split applies to the reservoir states or to embedded vectors, never to raw byte codes, whose averages mean nothing. Level j updates once every 2ʲ bytes, so a streaming pyramid costs O(1) additions per byte, amortised, and keeps O(log n) state: coarse summaries of long context, beside the copy window's exact recall. It stays linear, so it keeps the model's cheap, parallel training.

**Lifting, and Haar as its simplest case.** The lifting scheme (Daubechies and Sweldens 1998) splits samples into even and odd, predicts each odd sample from evens, and updates the evens. Haar is lifting with the simplest predictor: d = odd − even, then s = even + d/2.

| | Haar | CDF 5/3 (JPEG 2000 lossless) | CDF 9/7 (JPEG 2000 lossy) | Learned lifting |
| --- | --- | --- | --- | --- |
| Predict and update from | one neighbour | neighbours on both sides | four lifting steps and a scale | small learned steps |
| Operations per pair of samples | 1 addition, 1 subtraction, 1 shift | about 4 additions, 2 shifts | about 4–6 multiplications and additions | depends on the steps |
| Vanishing moments (trends the detail channel cancels) | 1: constants only | 2: linear trends too | 4 | learned |
| Separation of scales | poor: blocky, with leakage | better | good | as trained |
| Causal, as left-to-right prediction needs | yes, one sample's delay per level | symmetric steps need one or two later samples per level; a causal predictor gives up the symmetry | more delay | causal by construction |
| Exact inverse | yes | yes, integer to integer with rounding | yes | yes, whatever the predictor |

The model only analyses its input and never reconstructs it, so lifting's exact inverse matters little here; causality and vanishing moments decide. The proposal is to build the pyramid from lifting steps with Haar's predictor, which is cheap and causal, and later try a causal two-sample predictor or a learned one without changing the rest. Reservoir states drift smoothly, and one vanishing moment leaves linear drift in the detail channel, so the test is whether a second vanishing moment improves prediction by enough to pay for its cost.

**Looping changes the context only if each loop reads something new.** A looped readout that sees the same inputs every time gains depth, not context. If each loop reads one more level of the pyramid, the context becomes a pyramid: exact near the present, half the resolution at each step back, spanning about 2ᵏ times the base window after k loops. If each loop queries the copy window again, with a query updated by the previous loop, the context becomes a chain of up to k linked lookups through the text ("X is defined as Y; Y is defined at Z").

### Trial: does looping widen the context?

This trial tests the two shapes above. It sits outside stage A of `docs/plan.md`, which uses no model, and needs a GPU; its cost is not yet estimated.

**Hypotheses.**

* H0, the null: no form of looping lowers prediction loss on long-range text beyond the spread between training seeds, once compute is matched.
* H1, the pyramid: when each loop reads one more pyramid level, loss falls on bytes whose evidence lies beyond the raw window, and falls more as k grows.
* H2, multi-hop recall: when each loop re-queries the copy window, accuracy rises on references that need several hops, up to about k hops.

**What varies, and what is held constant.**

| Varies | Values |
| --- | --- |
| Form | base (no loop); looped readout on the same inputs; pyramid; multi-hop; pyramid and multi-hop |
| Loops k | 1, 2, 4, 8 |

Held constant: the training data and its order, the number of bytes seen in training, the optimiser and its schedule, the reservoir's eigenvalues, the raw and copy windows, and the number of learned parameters, since the looped block is weight-tied. Each cell runs with 3 seeds.

**The compute control.** A looped model does k times the arithmetic. To separate "more context" from "more compute", each looped form is compared with an unlooped model whose readout is k times deeper, with untied layers and the same arithmetic per byte. A gain that the deeper model matches is a gain from compute, not from context.

**Test sets.**

1. Held-out text from the training corpora: the general case.
2. Long-range text: Prolog programs in which a predicate is defined far from its use, and ACE texts in which a pronoun or a definite noun refers far back. Each byte is labelled with its distance to the evidence it needs.
3. Synthetic chains: "a1 = a2. a2 = a3. … What is a1?", with the number of hops and the distance between links set by the generator, so the answer is known.

**Measures.**

* Bits per byte, overall and by distance bucket (within 8 bytes, within the copy window, beyond it).
* On the synthetic chains, accuracy by number of hops and by distance.
* Cost: arithmetic per byte, state carried per byte, and training time, each measured.

**Checks that the trial itself is sound.**

* Causality: changing any later byte must leave every earlier prediction unchanged. A test plants a change and checks.
* The pyramid is used: replacing its levels with zeros at test time must raise the loss of the pyramid form; if it does not, the form learned to ignore it.
* A planted fault: a version that leaks one future byte into the pyramid must score much better than it should, and the causality test must catch it.

**Order.** Run a small model first (readout width 128 instead of 512), so a full grid trains quickly, and run the full 9.6M-parameter model only for the forms that beat their compute controls.

**Decision.** Adopt a form only if it lowers bits per byte on the long-range set by more than twice the standard deviation across seeds, beats its compute control, and its cost per byte is acceptable for the "do you mean?" round. Report every cell, including those that show nothing.

### Training and data

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
| The author's technical prose, written in STE | controlled English, for pretraining the English side | about 108K words | the author's |

To cut the cost of the rest:

1. Extract pairs from documentation, the PlDoc comments beside SWI-Prolog's predicates, for free.
2. Manufacture pairs from the deterministic path: Prolog → controlled English by the lexicon, then an open model of about 8B parameters rewrites each sentence into 2–3 free-English variants. Each variant paired with its Prolog is one training example.
3. Generate synthetic pairs from a grammar: a fixed set of English templates over facts, rules and queries, each with its Prolog. This costs nothing but covers only the templates.
4. Check each generated pair by running its Prolog on test queries, and by translating it back and comparing.
5. Add the model's own outputs that pass the checks.

Training on untrusted servers is safe. Neither data nor weights are secret, and every output of the finished model is checked.

### Restricted input languages: what to borrow

The cheapest gain is to accept only a controlled English in which each sentence has one meaning. The model then shrinks to a normaliser from free English into that subset, and a deterministic grammar does much of the rest.

| System | Restriction on the input | Translates into | Idea to borrow |
| --- | --- | --- | --- |
| [Attempto Controlled English](http://attempto.ifi.uzh.ch/site/main.php?text=description/description.html) (Zurich, since 1995) | ["Each ACE sentence has only one meaning"](http://attempto.ifi.uzh.ch/site/docs/ace_interpretationrules.html): prepositional phrases attach to the verb, relative clauses to the preceding noun, a pronoun to the most recent accessible noun phrase, "a" is existential. [Construction rules](http://attempto.ifi.uzh.ch/site/docs/ace_constructionrules.html): a passive needs "by", phrasal verbs are hyphenated ("waits-on"). | Discourse representation structures, then first-order logic, TPTP, OWL | Fixed interpretation rules remove ambiguity without a large grammar. The Horn-clause part of first-order logic is Prolog, so a deterministic path can cover what ACE parses (my inference). |
| [Lojban](https://en.wikipedia.org/wiki/Lojban) (Logical Language Group, 1987) | An unambiguous grammar, parsable by a parsing expression grammar; each predicate has a fixed place structure | Parts of it into predicate logic | Fix each verb's argument order ("X sends Y to Z"), so a verb maps to a Prolog predicate of known arity |
| [Simplified Technical English](https://en.wikipedia.org/wiki/Simplified_Technical_English) (ASD-STE100) | About 900 approved words; one word, one part of speech, one meaning; at most 20 words in an instruction and 25 in a description | (a writing standard) | Part of it is easy to check by machine: sentences of at most 25 words, no semicolons, one word per action |
| [Inform 7](https://ganelson.github.io/inform-website/) (2006; open source since 2022) | Natural-language syntax for programs | Interactive fiction | A controlled English can carry a whole programming language |

Restrictions worth adopting, in order of value:

1. One meaning per sentence, by ACE's interpretation rules. This makes parsing deterministic.
2. A fixed argument order per verb, as in Lojban. Each verb then names a Prolog predicate of known arity, and each noun phrase fills one of its places.
3. STE's limits: at most 25 words, one word per action.
4. A deterministic translator for the sentences the controlled grammar parses, with the model only for the rest and for Prolog → English.

Logical English (Kowalski and Dávila) and PENG (Schwitter) compile controlled English to Prolog and are the closest prior work, but their pages could not be reached, so they are not assessed here.

## 3. Conversation

### Conversational loop

The model proposes a sentence in Attempto Controlled English (ACE), and the user confirms it before anything is translated. The user sees APE's own restatement of what it parsed, not the model's sentence, so a wrong parse shows. The model's proposals set most of a round's time: APE's parse and restatement take about 3.6 ms (measured; see the memex design's Prolog timings).

*Diagram, kept in the living doc only: Conversational loop: one round of 'do you mean?'.*

1. The user types free English.
2. The model proposes up to k ACE sentences. APE parses each, and the first that parses is kept.
3. APE restates its parse in ACE, and the user sees "Do you mean: …?". When APE cannot restate a parse, the ACE sentence itself is shown.
4. The user accepts, edits the ACE directly, or rejects it, which brings the next candidate.
5. An accepted parse is translated to Prolog deterministically, and its tests run in Prolog.
6. Each accepted pair of free English and ACE is logged, for retraining offline. These pairs are real input confirmed by the user, better than the synthetic ones.

Measured on 711 sentences of the first 200 GSM8K-Prolog problems, with APE and the full Clex lexicon:

| Input | Sentences APE accepts | Problems APE accepts |
| --- | --- | --- |
| As written | 20 (2.8%) | 0 of 200 |
| After a rewriting step (present tense, "12 dollars" for "$12", "for each" for "per", no filler adverbs) | 40 (5.6%) | 0 of 200 |

Of 15 sentences newly accepted after rewriting, APE restated 12 faithfully, 1 with a wrong reading of scope, and could not restate 2. So free English needs the model for nearly every sentence, and the restatement is what catches its mistakes.

The model stays untrusted: it only proposes, and the user, who knows what was meant, confirms.

#### Proposing a formulation: alternatives to the model

A neural model is not the only way to propose the controlled form for "do you mean?", and it is probably not the best first one. A predictive editor and a memory of confirmed pairs teach the system better and need no trained model. The model remains the best guesser for new free English, so it fits as a fallback. Only the rewriting rules are measured; the rest are estimates.

| Mechanism | How it proposes | Free English it covers | What it teaches | Cost | Latency |
| --- | --- | --- | --- | --- | --- |
| Predictive editor | offers only the next words the ACE grammar accepts, as the user types, as in ACE's and PENG's editors | none: the user writes controlled English, guided | the most: every keystroke shows what is accepted | the grammar's lookahead in APE, a few hundred lines | per keystroke |
| Memory of confirmed pairs | finds the most similar past input (word overlap or BM25) and adapts its ACE by swapping in the new names and numbers | grows with use, and is personal to the user | good: proposals resemble the user's own accepted sentences | an index and slot filling, about 0.5–1K lines | milliseconds |
| APE's repair hints | APE marks where parsing failed and suggests a repair; the nearest edits are offered | small: errors near the subset | good: shows which word broke the sentence | small: it already exists | about one parse |
| Rewriting rules | present tense, "12 dollars" for "$12", fillers dropped | small: 2.8% to 5.6% accepted on GSM8K (measured) | weak: the fixes are invisible | about 200 lines (written) | microseconds |
| Intent classification and slot filling (small statistical classifiers) | picks one of a fixed set of commands or question forms and fills its slots | good for the shell's 30–50 verbs and fixed question forms; poor otherwise | moderate | tiny: kilobytes of weights | milliseconds |
| The neural model (9.6M) | rewrites arbitrary free English into ACE | the broadest | moderate: shows a correct form after the fact | about $15–50 of training, and hardware to run it at speed | set by that hardware |

Recommended order, if measurement agrees:

1. The predictive editor and APE's repair hints, to teach the subset as the user types.
2. The memory of confirmed pairs, for inputs that resemble earlier ones.
3. The neural model as a fallback, for free English that matches nothing in memory.

The memex design's Decisions follow this. To decide, log real inputs for a while and measure what share the editor, the memory and the rewriting rules handle without the model. The remainder is the model's only job.

#### Where APE spends its time

APE (about 19K lines of Prolog, LGPL-3.0, with the Clex lexicon) spends most of its time in a grammar that backtracks heavily, not in the lexicon, so choice points and the trail must be fast. This was measured on the 711 GSM8K sentences, with the full Clex lexicon, by APE's own phase timers and SWI-Prolog's profiler.

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

What this means for speed:

- Tokenising outside Prolog would remove about 38% of the work.
- A hash table for the lexicon, one lookup per word instead of probing each of about 13 word classes, would remove about 15–25% more.
- What remains is general Prolog execution with deep backtracking, in both the parse and the restatement.

The profiler samples, its top 30 predicates cover only about 55% of the time, and most of the sample is failed parses, whose cost was not separated from accepted ones.

Clex is GPL-3.0, which matters if the lexicon is distributed with the memex.

### A knowledge-base chatbot

The same model and Prolog can answer questions about the world if the knowledge lives in Prolog facts, not in the weights. A model of 9.6M parameters holds only about 2.4 MB of knowledge, at roughly 2 bits per parameter (a figure recalled from published scaling studies, not checked). That is a few hundred thousand simple facts at most, against more than a billion statements in Wikidata (from memory).

| Approach | Knowledge held | Quality | Fit |
| --- | --- | --- | --- |
| Knowledge in the weights, 9.6M | about 2.4 MB | fluent on narrow topics, wrong or invented elsewhere | poor: answers cannot be checked |
| A larger model, knowledge in the weights (3B at 4 bits) | broad | useful | answers cannot be checked |
| The small model plus retrieved text in the copy window | as much text as storage holds; a compressed English Wikipedia is several GB (from memory) | good for looking things up, weak at reasoning | needs storage of several GB and an index |
| **Facts in Prolog, reached through the conversational loop (chosen)** | a Wikidata subset of about 25M facts, about 0.8 GB (below) | exact where the base covers the question; an explicit "no answer" where it does not | the same model and Prolog, and every answer can be checked |

A question takes one turn of the same loop:

1. The user asks in English. The model proposes an ACE question, and the user confirms APE's restatement.
2. The question becomes a Prolog query against the knowledge base.
3. The answer comes back as facts, which APE phrases in ACE. The user sees exactly what the base says, and which facts it rests on.
4. The model phrases the answer fluently from the facts in its copy window. APE parses that phrasing back into ACE, and Prolog checks that every claim is among the returned facts before the user sees it.

Limits:

- The chatbot answers only what the base holds. It can give the population of Montana, but it cannot discuss the plot of a film.
- Questions outside the controlled grammar need a round of "do you mean?" before they can be asked.
- On a PC the knowledge base is a local file, loaded into SWI-Prolog. Its lookup time is not measured.

#### Roles in a conversation

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

#### The Wikidata subset

A useful subset is about 25M facts and about 0.8 GB: the roughly 1M best-known entities, about 300 common relations, and no external identifiers. Only the starting totals come from a source: about 1.78 billion statements on 121 million items, of which scholarly articles are 55.5% and astronomical objects 8.5%, leaving about 641 million ([Phabricator T437880](https://phabricator.wikimedia.org/T437880), undated). Every cut after that is an estimate.

| Subset | Facts | Facts at about 30 bytes each, in three orderings | English labels | Total |
| --- | --- | --- | --- | --- |
| A: all but scholarly articles and astronomy, without external identifiers (assumed about 40% of the 641M) | about 256M | about 7.7 GB | about 1.8 GB | about 9.5 GB |
| B: items with an English Wikipedia article (about 7M, from memory), about 25 facts each | about 175M | about 5.3 GB | about 0.2 GB | about 5.5 GB |
| C: the about 1M best-known items, about 25 facts each, about 300 relations (chosen) | about 25M | about 0.75 GB | about 25 MB | about 0.8 GB |
| D: a pocket base, about 100K items × 20 facts | about 2M | about 60 MB | about 2.5 MB | about 63 MB |

- A fact is a relation (2 bytes), a subject (4 bytes) and an object (4 bytes, or a reference to a value): about 10 bytes, stored in three orderings (subject–relation–object, relation–object–subject, object–subject–relation) so that any pattern of bound arguments is a prefix of one. Quantities and dates need about 8 bytes more.
- Questions concentrate on well-known entities, so the top 1M items should cover most everyday questions. This is an assumption, not a measurement.
- Each of the about 300 relations needs a verb frame in the lexicon, so the relation list also bounds the lexicon work.
- Wikidata is CC0 (from memory), so a subset can ship with the memex.

Open points:

- [ ] What share of a sample of expected questions does subset C answer, and which relations do the answers use?
- [ ] How are relations named, so that each verb frame in the lexicon maps to a predicate of the base?

### A natural-language shell

The same loop can drive a shell: the user types a command in English, and its effects are shown and confirmed before anything changes. This is close kin to the fourth-generation languages and English database front ends, such as Intellect, of the early 1980s. Their history, recalled here from memory and not checked, gives the lessons.

| Lesson from the 4GL era | How this design fares |
| --- | --- |
| Habitability: users could not predict which English was accepted, and hit invisible walls | Better: the model rewrites free English into the subset, and "do you mean?" shows what was understood |
| Brittle at the edge of the domain | The same: commands work only within the facts and rules written down, but "no answer" is honest |
| The survivors were declarative cores: SQL and spreadsheets | ACE translated to Prolog is such a core, and an expert can use it directly |
| Performance surprises: a short query could expand into a costly plan | The same risk: step limits must cap a search |
| Lock-in to one vendor's language | Avoided where possible: ACE, Prolog and s(CASP) are open, and the facts are CC0 |

A shell adds side effects, which the 4GLs guarded only with "are you sure?". Worlds give a dry run:

1. The command runs first in a sprouted world, so its effects are only recorded there.
2. The world's changes are restated in ACE: "This will delete 3 files: …".
3. The user confirms, and the world is committed: `commit` becomes "execute". Or the world is discarded.

A dry run covers only effects that a world can capture: files, facts, and messages queued but not yet sent. An effect that leaves the machine, such as a byte already on the serial line, cannot be undone, so it must be held back until the commit.

## 4. Reasoning with it

### Suggesting facts, rules and constraints

The model has a further purpose: it suggests formal statements for the user to accept, so the user writes less Prolog. This meets the memex's main risk, that a knowledge base stalls when entering knowledge costs more than it returns. Wikidata already supplies facts about the public world; the suggestions cover the rest.

| Suggestion | Drawn from | Checked by |
| --- | --- | --- |
| A fact: a mention linked to its Wikidata item (“Buran” to its item), or a claim restated as a fact | the user's sentence and the label index | the “do you mean?” round: APE restates it, and one key accepts it |
| A constraint | Wikidata's property constraints (single value, value type, conflicts with; property P2302, recalled and not checked), offered when an item uses the property; or a constraint pattern that the model fills from the user's sentence (“an event has one date”) | the constraint runs over the store at once, so the suggestion shows the violations it would raise before it is accepted |
| A rule | rules mined from the facts, as AMIE mines them from knowledge bases (Galárraga et al. 2013, recalled and not checked), chosen and phrased by the model | each rule shows its support and its exceptions in the store |
| A reference: a source for a claim | works already in the knowledge base, found by the citation graph (works that the claim's other sources cite, or that cite them; co-citation and bibliographic coupling, after Small 1973 and Kessler 1963, recalled), by main subject (P921, recalled), and by the claim's words against titles; chosen and ranked by the model | the suggestion shows why it was offered (the shared citations or subject) and opens the source to read; adopting it adds a `cites` fact. The model never writes a reference: it only chooses among works that exist in the store, so it cannot invent one |

Three rules keep the suggestions honest. A suggestion enters the journal only when the user accepts it. Each is shown restated in controlled English, with its evidence: support, exceptions and violations. And each comes from a fixed pattern that the model chooses and fills, not free Prolog, which a model of 9.6M parameters is unlikely to write reliably (an estimate, untested). The journal records every acceptance and refusal, which measures the acceptance rate at no cost and gives training pairs for the gaps.

The model runs all the time, over whatever the user is working on, and keeps a pane of suggestions up to date. One click adopts a suggestion and another unadopts it: adopting writes an entry to the journal and unadopting retracts it, so both are ordinary edits under universal undo, and the dataflow graph recomputes what depends on them. Pointing at a suggestion previews it in place: the violations it would raise or clear, and the items it would change. The pane follows the rules for a system that offers help unasked, after Horvitz's principles of mixed-initiative interfaces (1999, recalled and not checked): it never takes the keyboard or moves what the user is looking at; it never reorders a suggestion under the pointer; it ranks by evidence and keeps few suggestions in view; and each dismissal is recorded, so a refused suggestion does not come back unless its evidence changes.

### An agent: a general language model with tools

This subplan adds a second, separate model: a general language model, run locally, that reasons in several steps and calls Adversaria's tools. It is a proposal for stage B, built only if stage A's numbers show that suggestions are adopted and kept. The reservoir cannot fill this role: its exact context is a few thousand bytes, and multi-step planning appears in models of billions of parameters (recalled, not checked). Resizing the reservoir does not help either: training an agent-class model from scratch needs about 7 × 10²³ operations, centuries on one consumer GPU (estimate), and the recall over long contexts that an agent needs requires gates that depend on the input, which a fixed reservoir lacks.

**Who does what.**

| Part | Does | Never does |
| --- | --- | --- |
| The reservoir | restatements, suggestions from fixed patterns, narrow calls inside a loop Adversaria runs | run a loop of its own |
| The agent | opens worlds, makes claims in them, runs queries, reads violations and differences, and recommends | write to the actual world, adopt, retract, purge, or hold a key |
| Prolog | derives results from a world's premises and checks constraints | guess |
| The user | asks, reviews, adopts or retracts | — |

**Boundaries, kept by structure, not by prompts.** A prompt is a request; these are enforced by what the agent's connection can do.

1. **No path to the actual world.** The agent's connection holds no capability to write the user's working branch, and the tool server offers no adopt, retract or purge. Adoption exists only in the user's interface.
2. **Its own author.** Every entry the agent causes is authored by the agent, named with its model, version and the hash of its weights, and sealed with a key of its own that the sealer holds. The user's key never signs the agent's work.
3. **Worlds are kept, not discarded.** A world is a branch of the journal; closing it leaves it in the tree, so the agent's reasoning can be audited later. Only the user can purge.
4. **Three kinds of statement kept apart.** What the agent supposes (a world's premises), what it claims (`claims(agent, P)`), and what Prolog derives from the premises (checked). Each shows as its kind.
5. **Sources are checked.** Every source the agent cites goes through the quote and source checks of the design's § Testing claims; a citation that fails is marked, so an invented reference cannot pass as evidence.
6. **The agent only makes claims; Adversaria judges them.** The agent cannot be trusted not to invent evidence when it describes what it has done, shown or learned, so it has no channel for free prose about its work: no commentary, no narrated summary, no reasons in its own words. Everything it says is a claim, and every claim is shown beside Adversaria's judgement of it: the quote and source rulings, whether Prolog derives it from the world's premises, the constraints it satisfies or violates, the disagreements it joins, and what interrogation finds. The account of what a session did comes only from the journal, which the memex writes: worlds opened, entries written, queries run with their answers, violations found, recommendations made.
7. **Budgets.** Each session has a limit on entries written, worlds opened, queries run and time; the tool server refuses calls past a limit and says which.
8. **Local by default.** The agent runs on the user's machine. Connecting a cloud model is the user's explicit choice, shown on screen while it lasts, because a cloud model sees the notebook.

**The API.** A small set of tools with strict JSON schemas, offered as native function calls to a local model and, through the same code, as an MCP server over standard input and output for other clients. Each tool's description is written as carefully as a prompt, and each error says what to do next.

| Tool | Arguments | Returns | Effect on the journal |
| --- | --- | --- | --- |
| `open_world` | a name, the world to branch from (default: the user's current world), a reason | the world's identifier | a `suppose` entry, by the agent |
| `suppose` | world, a premise as a term or controlled English | the premise as stored | an entry in that world |
| `claim` | world, a statement, its source or sources (each a quote and an identifier) | the claim, with its quote and source rulings | an entry in that world: `claims(agent, P)` with its evidence |
| `query` | world, a goal, a limit on answers | answers, each with its justification | an entry recording the query and its answer, unless query journalling is switched off |
| `get_state` | world, an optional item or pattern | the world's premises, claims and derived facts that match | none |
| `get_violations` | world | constraint violations and disagreements, each with the items it names | none |
| `get_differences` | two worlds | the A/B difference table | none |
| `interrogate` | world, a claim | who else claims it and who claims the contrary, its provenance, its source's record, and what follows if it is adopted | none |
| `recommend` | world, the entries the agent recommends for adoption, and for each the entries in the journal that support it (identifiers only, no prose) | an identifier for the recommendation | a claim by the agent, shown in the suggestion pane with Adversaria's judgement of each recommended entry and of each cited support |
| `close_world` | world | confirmation | none: the session stops using the world, which stays in the tree, as a closed supposition does |

No tool adopts, retracts, purges, signs, or writes outside the agent's own worlds. The tools use only entry kinds the journal format already has, so the format does not change.

**The loop.** The user asks a question. The memex starts a session with its budgets. The agent opens worlds, supposes, claims, queries and inspects, then recommends. The memex writes the account of the session from the journal and shows the recommendations in the suggestion pane, each beside Adversaria's judgement of it, where the user adopts or rejects each with the usual one click, and can open any of the agent's worlds beside the actual one.

**Model and hardware (estimates).** An open-weights model of about 14B parameters at 4-bit quantisation fits a 16 GB GPU with room for context; one of 32B does not. A model of about 7–8B can be fine-tuned on Adversaria's tool calls with low-rank adapters on 4-bit weights within 16 GB. Speed and the rate of correct tool calls must be measured before the trial below.

**The trial.** One hypothesis: the agent's recommendations are adopted, and kept, at least as often as suggestions made without a model. The journal measures, per source of suggestion, the share offered that are adopted, the share of those later retracted, and the share of the agent's cited sources that fail the checks. A high rate of retraction after adoption means users trust the agent too readily, and counts against it.

### What-if questions: counterfactual simulation

The chatbot answers "what if" by supposing a condition in a separate world, setting aside the facts that contradict it, and then answering or simulating forward. It can simulate only worlds whose rules of change are written down, by the user in ACE or by an engineer as a library. "What if the reservoir loses 5% a day?" is in scope; "what if Napoleon had won?" is not. This is a design sketch; nothing in it is built or measured.

#### Simulation in the style of ROSS

| Idea from RAND's ROSS | Role here |
| --- | --- |
| Objects with attributes | simulation objects, with their initial attributes taken from the knowledge base |
| Behaviours fired by message patterns | rules of change as Prolog clauses, such as "Every day, the tank loses 5 litres." |
| Scheduled messages ("plan …", from memory) | a queue of events in logical time; each event runs its behaviours, which may schedule more |
| Reruns with changed conditions | the baseline and the supposition run in sibling worlds, and the answer is their difference |

Time is logical only, so the same question always gets the same answer. An inverse question, such as "what price makes the profit at least 100 dollars?", is a constraint problem, not a simulation, and uses the constraint-Prolog extension.

#### Worlds

Each supposition runs in a world, after VPRI's Worlds (Warth, Ohshima, Kaehler and Kay, 2011). A child world reads through to its parent and keeps its own writes, so storage grows only with the changes, not with the 0.8 GB base.

| Worlds idea | Role here |
| --- | --- |
| `sprout` | a supposition, from the knowledge base or from the current scenario |
| Sibling worlds | baseline and supposition side by side; Prolog's backtracking cannot hold both outcomes at once |
| Nested worlds | "and suppose also …", stacked on the first supposition |
| `commit` | only when the user says a supposed fact of their own is really true; never into the shared base |

A world is a small table of facts added and removed. A lookup checks the current world, then its parent, up to the base, and each lookup gains a world tag. Every answer inside a scenario is labelled, so a hypothetical is never taken for a fact.

In the memex a world is a branch of the journal's undo tree (see the memex design): Suppose sprouts a branch, undoing and then editing sprouts another, each named by its first change, and commit makes a branch the actual world. Reversing one earlier decision, such as an adopted suggestion, is a retraction on the current branch, not a new world.

#### Counterfactual revision

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

#### Prior work, and a trial of s(CASP)

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

