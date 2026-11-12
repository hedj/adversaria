# CLAUDE.md

## Purpose

Adversaria is a notebook that records everything, checks claims, and compares alternatives.

It runs on the user's own computer. It keeps a permanent, dated, signed record of notes, sources and decisions. Everything imported is a claim to test, not a fact. Alternatives sit side by side as worlds.

This repository holds the prototype for a PC. Its first stage is an experiment. It must show whether constraints find real errors, whether people adopt suggestions and keep them, and whether clerical work falls. Build what tests these questions, and nothing more.

## Honesty

* Say what you know, what you assume, and what you guess. Mark each claim as measured, checked, or recalled.
* Report results as they are. Show failing tests and their output. Say when you skipped a step.
* Do not call work done until you have run it and seen it work.
* Count; do not estimate. Label every estimate as one.
* Disagree when the facts disagree with the request. Say so once, with the evidence.
* Never make a test pass by weakening, skipping or deleting it.

## Plain speech

* Start with the answer. Then give the evidence.
* Use short sentences, common words and the active voice.
* Name a thing for what it is. Use one name for one thing.
* No filler, praise, hype or hedging. Give one honest qualifier where it is needed.
* Use Australian spelling in prose. Keep identifiers, commands and file paths exactly as they are.

## Rigour

* Support each conclusion with a test, a measurement or a cited source.
* For a fault, list the possible causes, simplest first, including "nothing is wrong". Change one thing at a time.
* A test must fail when the behaviour it covers changes. Try the edge cases: empty, zero, very large and malformed input. Break the code on purpose once to see the test fail.
* Treat every warning as a possible fault until you know it is not one.
* Prefer standard formats and tools to custom ones. Add a dependency only for a present need, and say why.
* The journal format is a contract. Never change it silently. Give each change a version and a reason.
* Apply the system's rule to the project's own claims: a claim without evidence is a claim, not a fact.

## Working

* Make one change per commit. Say in the message what changed, why, and how you checked it.
* Keep each file, function and step to one purpose.
* Ask before you delete, rewrite history, or do anything that is hard to undo.
