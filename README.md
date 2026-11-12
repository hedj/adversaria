# Adversaria

Adversaria is a notebook that runs in a terminal on your own computer.
Notes are kept as an outline. Every change is written to an append-only
journal, and a separate program signs and dates the journal's entries.
Facts in the notes are Prolog terms. Material from outside (documents,
Wikidata, analysis results) is recorded as claims with their sources,
separate from what you have adopted as your own beliefs. Constraints
written in Prolog mark facts and claims that conflict. A supposition opens
a second set of facts beside the current one, so the two can be compared.
An item can draw facts as a chart, a table, a timeline or a diagram, from
marks declared over Prolog queries; these update as the facts change.

This repository holds a prototype, written in Rust with SWI-Prolog. It
was built to test three questions over a month of use: whether
constraints find errors in real notes, whether suggestions are adopted
and kept, and whether clerical work decreases. It has not yet been run in
a real terminal, only in tests that simulate one (see docs/log.md).

## Requirements

- A terminal that supports the kitty keyboard protocol and the kitty
  graphics protocol, such as kitty, WezTerm or Ghostty. The memex checks
  for both at start-up and exits with a message if either is missing.
- Nix, which provides everything else. Without Nix you
  need Rust (cargo), SWI-Prolog (`swipl`), Pandoc and curl, and Python 3
  to run the tests.

## Getting started

Clone the repository and enter the development shell:

```bash
git clone https://github.com/hedj/adversaria.git
```

```bash
cd adversaria
```

```bash
./dev-shell
```

`./dev-shell` runs `nix-shell` and needs no experimental Nix features.
If you have flakes enabled, `nix develop` gives the same shell. Both use
the versions pinned in `flake.lock`.

Start the sealer and the memex:

```bash
make run
```

On first use this creates a journal directory at `~/adversaria-journal`
and a signing key at `~/.adversaria.key`. To use other locations:

```bash
make run JOURNAL=$HOME/notes KEY=$HOME/.notes.key
```

In the memex:

- Type to write an item. Enter starts a new item; Tab indents it.
- Hold Left Ctrl, type a command such as `help`, and release Left Ctrl to
  run it.
- Press F10 to quit.

docs/tutorial.md takes you through a first session step by step.

## Documentation

- docs/tutorial.md: a first session.
- docs/guide.md: the user guide, covering every command, the files kept,
  the tools, and how to change keys and enable the mouse.
- docs/config.example.toml: the default configuration.
- docs/design.md: the design and the decisions behind it.
- docs/plan.md: the implementation plan.
- docs/log.md: what has been built and tested, faults found, and known
  limits.

## Tests

```bash
make test
```

## Licence

GPL-3.0. See LICENSE.
