# One command runs every test (docs/plan.md § Layout).
.PHONY: test test-spikes test-prolog test-rust spike-scasp

test: test-spikes test-prolog test-rust

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
# (nix develop) sets SCASP_HOME to one.
SCASP ?= $(SCASP_HOME)
spike-scasp:
	@test -n "$(SCASP)" || { echo "usage: make spike-scasp SCASP=<checkout>"; exit 2; }
	tests/spikes/scasp_forall.sh "$(SCASP)"
