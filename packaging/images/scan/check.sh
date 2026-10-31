#!/usr/bin/env bash
# Talks Hub to the Scan image named $1, held as Bragd holds it: it must name a move.
set -euo pipefail
out=$(printf 'hub\ninit\nnew-game\npos pos=Wbbbbbbbbbbbbbbbbbbbbeeeeeeeeeewwwwwwwwwwwwwwwwwwww\nlevel depth=6\ngo think\n' | { cat; sleep 5; echo quit; } \
  | timeout 120 "${RUNTIME:-docker}" run --rm -i --network=none --read-only --tmpfs /tmp --cap-drop=ALL --security-opt no-new-privileges --user 65534:65534 -e HOME=/tmp "$1")
echo "$out" | grep '^done move=' || { echo "$out"; echo "Scan named no move" >&2; exit 1; }
