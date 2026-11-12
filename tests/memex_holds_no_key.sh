#!/bin/sh
# Plan, milestone 1: the memex never writes seals.jsonl and never holds a key.
# Checks that the memex's dependencies hold no signing or randomness code,
# and that no source in crates/memex or prolog/ names the seals file or a key.
# Limit: it cannot see a path built at run time from other strings.
set -eu
cd "$(dirname "$0")/.."
deps=$(cargo tree -q -p memex -e normal --prefix none --format '{p}' | awk '{print $1}' | sort -u)
status=0
for bad in sealer check ed25519-dalek ed25519 curve25519-dalek getrandom signature; do
    if echo "$deps" | grep -qx "$bad"; then echo "memex depends on $bad"; status=1; fi
done
dirs=crates/memex/src
[ -d prolog ] && dirs="$dirs prolog"
# Comment lines are skipped: a comment may say what the memex must not do.
if grep -rnE 'seals\.jsonl|\bSEALS\b|SigningKey|\.key\b|key_path' $dirs | grep -vE '^[^:]+:[0-9]+:[[:space:]]*(//|%)'; then
    echo "memex or prolog source names the seals file or a key"; status=1
fi
[ $status -eq 0 ] && echo "ok: memex has no signing code and names neither seals.jsonl nor a key"
exit $status
