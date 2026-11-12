# One command runs every test (docs/plan.md § Layout).
.PHONY: test test-spikes

test: test-spikes

test-spikes:
	swipl -q -g main -t halt tests/spikes/incremental_tabling.pl
