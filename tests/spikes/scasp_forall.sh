#!/bin/sh
# Spike, milestone 0: reproduce s(CASP) issue #13 and its workaround.
# Usage: tests/spikes/scasp_forall.sh <s(CASP) source checkout, tag V1.1.4>
# Passes while the bug is present under the default algorithm and absent
# under --forall=sasp. If the default algorithm is fixed, it fails, which
# says the design's workaround (§ Falsifiability, engine notes) can go.
set -u
SCASP=${1:?path to an s(CASP) checkout}
DIR=$(dirname "$0")
run() {
    timeout 60 swipl -p library="$SCASP/prolog" "$SCASP/prolog/scasp/main.pl" \
        -s1 --forall="$1" "$DIR/scasp_forall.pl" </dev/null 2>&1
}
status=0
for alg in all sasp; do
    if run "$alg" | grep -q 'No models'; then got=no_model; else got=model; fi
    case $alg in all) want=model ;; sasp) want=no_model ;; esac
    echo "--forall=$alg: not bad -> $got (expected $want)"
    [ "$got" = "$want" ] || status=1
done
exit $status
