# Adversaria

A notebook that records everything, checks claims, and compares
alternatives. It runs on your own computer, in a terminal. It keeps a
permanent, dated, signed record of notes, sources and decisions. Everything
imported is a claim to test, not a fact. Alternatives sit side by side as
worlds.

This repository holds a prototype, built for a one-month experiment: do
constraints find real errors, do people adopt suggestions and keep them,
and does clerical work fall?

## Start

You need Nix, and a terminal with the kitty keyboard and graphics
protocols (kitty, WezTerm or Ghostty).

```bash
nix develop
```

```bash
make run
```

## Read

- docs/tutorial.md: a first session, about twenty minutes.
- docs/guide.md: the user guide, including how to change the keys.
- docs/config.example.toml: the default key configuration.
- docs/design.md: what the system is for and why it is built this way.
- docs/plan.md: the implementation plan.
- docs/log.md: what has been built, tested and found, milestone by
  milestone, and what has not been tested.

## Test

```bash
make test
```

## Licence

GPL-3.0; see LICENSE.
