#!/usr/bin/env bash
# Asks the GNU Backgammon image named $1 for a hint, held as Bragd holds it: it must list a move
# with its equity, so its networks and bear-off databases were found.
set -euo pipefail
out=$(printf 'set player 0 human\nset player 1 human\nnew game\nset board 4HPwATDgc/ABMA\nset dice 3 1\nhint 1\nquit\ny\n' \
  | timeout 120 "${RUNTIME:-docker}" run --rm -i --network=none --read-only --tmpfs /tmp --cap-drop=ALL --security-opt no-new-privileges --user 65534:65534 -e HOME=/tmp -e LC_ALL=C "$1" 2>&1)
echo "$out" | grep 'Eq\.:' || { echo "$out"; echo "GNU Backgammon gave no hint" >&2; exit 1; }
if echo "$out" | grep -q 'could not be found'; then echo "$out"; echo "a database is missing" >&2; exit 1; fi
