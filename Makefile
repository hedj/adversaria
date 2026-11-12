# One command runs every test (docs/plan.md § Layout).
.PHONY: test test-spikes spike-scasp

test: test-spikes

test-spikes:
	swipl -q -g main -t halt tests/spikes/incremental_tabling.pl

# Optional: s(CASP) is not needed before stage B. Pass the path to a
# checkout of github.com/SWI-Prolog/sCASP at tag V1.1.4.
spike-scasp:
	@test -n "$(SCASP)" || { echo "usage: make spike-scasp SCASP=<checkout>"; exit 2; }
	tests/spikes/scasp_forall.sh "$(SCASP)"
