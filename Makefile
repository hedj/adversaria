# One command runs every test (docs/plan.md § Layout).
.PHONY: test test-spikes test-prolog test-rust test-pty spike-scasp run measures interactivity

test: test-spikes test-prolog test-rust test-pty

# The memex program in a pseudo-terminal, with a script playing the terminal.
test-pty:
	cargo build -q -p memex
	python3 tests/memex_in_a_pty.py target/debug/memex

# One command starts the system: the sealer in the background (making a key
# on first use), then the memex in this terminal. Stops the sealer on exit.
JOURNAL ?= $(HOME)/adversaria-journal
KEY ?= $(HOME)/.adversaria.key
run:
	cargo build -q --release -p memex -p sealer
	test -f "$(KEY)" || target/release/sealer init "$(JOURNAL)" "$(KEY)"
	target/release/sealer run "$(JOURNAL)" "$(KEY)" > /dev/null & \
	  pid=$$!; target/release/memex "$(JOURNAL)"; status=$$?; \
	  sleep 1; kill $$pid; exit $$status

# Unit tests, lints as errors, and formatting.
test-rust:
	cargo test -q
	cargo clippy -q --all-targets -- -D warnings
	cargo fmt --check
	tests/memex_holds_no_key.sh

test-spikes:
	swipl -q -g main -t halt tests/spikes/incremental_tabling.pl

# The Prolog end of the query protocol.
test-prolog:
	swipl -g run_tests -t halt prolog/tests/test_protocol.pl

# Optional: s(CASP) is not needed before stage B. Pass the path to a
# checkout of github.com/SWI-Prolog/sCASP at tag V1.1.4; the Nix dev shell
# (./dev-shell, or nix develop) sets SCASP_HOME to one.
SCASP ?= $(SCASP_HOME)
spike-scasp:
	@test -n "$(SCASP)" || { echo "usage: make spike-scasp SCASP=<checkout>"; exit 2; }
	tests/spikes/scasp_forall.sh "$(SCASP)"

# The measures report (milestone 7), from the journal alone. Run it weekly
# during the month of use, and log the output in docs/log.md.
measures:
	cargo run -q --release -p tools --bin measures -- "$(JOURNAL)"

# How many things each unit of code and documentation refers to; every unit
# over 4 listed (crates/tools/src/interactivity.rs says what is counted).
# INTERACTIVITY_DETAIL=1 lists each unit's referents.
interactivity:
	cargo run -q -p tools --bin interactivity -- .
