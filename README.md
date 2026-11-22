# Adversaria

Adversaria is a notebook that keeps what you hold true apart from what
your sources say, notices when they conflict, and lets you try "what if
the other source is right?" side by side, without losing anything.

You write notes as an outline. Short sentences in plain, controlled
English, such as `The date of Buran-flight is 1988.` or
`Talk-notes says that the date of Buran-flight is 1989.`, are read as
facts and claims; a rule such as `Nothing has more than 1 date.` is
checked whenever they change, and marks the items that break it.
Everything else stays prose. Documents, Wikidata and analysis results
come in as claims with their sources, until you adopt them. Every change
is kept in a permanent journal, which a separate program signs and
dates. Facts can be drawn as charts, tables, timelines and diagrams that
follow the facts.

The notebook's window is called the memex, after Vannevar Bush's 1945
design for a personal knowledge machine. It reasons in Prolog, a logic
language; you can write Prolog where English is not enough, but you
need not know it to begin. docs/introduction.md says, in a page, what
it is for.

This repository holds a prototype, written in Rust with SWI-Prolog. It
was built to test three questions over a month of use: whether
constraints find errors in real notes, whether suggestions are adopted
and kept, and whether clerical work decreases. It has been run only in
tests, which type into its window under X11 and Wayland servers with no
screen (see docs/log.md).

## Requirements

- A desktop under X11 or Wayland. The memex opens a window of its own and
  draws through Vulkan or OpenGL; the Nix shell supplies Mesa's drivers.
  NVIDIA's own driver outside NixOS needs nixGL (recalled, not tested).
- Nix, which provides everything else. Without Nix you
  need Rust (cargo), SWI-Prolog (`swipl`), APE (the controlled English
  parser, found through `APE_HOME`), Pandoc and curl, and Python 3 to run
  the tests.

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
- Hold Left Ctrl, type a command such as `find draft`, and release Left
  Ctrl to run it. F1 lists every command, with what it does.
- Press F10 to quit.

To run the tests:

```bash
make test
```

## Documentation

- Why: docs/introduction.md, what it is for, in a page.
- Start: docs/tutorial.md, a first session.
- Use: docs/guide.md, the user guide: every command, the files kept, the
  tools, and the configuration.
- Understand: docs/design.md, the design and the decisions behind it.
- Record: docs/log.md, what has been built and tested, faults found, and
  known limits; it leads to the plan and to the structure of the code and
  documents.

## Licence

GPL-3.0. See LICENSE.
