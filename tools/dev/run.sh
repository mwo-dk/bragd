#!/usr/bin/env bash
# Builds Bragd from this checkout and prints how to start it with a throwaway home, so a test
# run never touches your own config, Libraries or engines. Installs nothing.
#
#   tools/dev/run.sh            build, then print the start command
#   tools/dev/run.sh --start    build, then start it at once
#   tools/dev/run.sh --keep DIR use DIR as the home (kept between runs) instead of a new one
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"

start=0
home=""
while [ $# -gt 0 ]; do
  case "$1" in
    --start) start=1 ;;
    --keep) home=${2:?--keep needs a folder}; shift ;;
    -h|--help) sed -n '2,7p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

echo "Building the interface (gui/dist)…"
(cd gui && { [ -d node_modules ] || npm ci --no-audit --no-fund; } && npm run build --silent)
echo "Building bragd-gui and bragd-helper (release)…"
cargo build --release --quiet -p bragd-gui -p bragd-helper

if [ -z "$home" ]; then home=$(mktemp -d "${TMPDIR:-/tmp}/bragd-home.XXXXXX"); fi
mkdir -p "$home"
home=$(cd "$home" && pwd)

cmd="env HOME='$home' XDG_CONFIG_HOME='$home/.config' XDG_DATA_HOME='$home/.local/share' XDG_CACHE_HOME='$home/.cache' '$root/target/release/bragd-gui'"

echo
echo "Built. Start Bragd with a throwaway home (works in fish, bash and zsh):"
echo
echo "  $cmd"
echo
echo "First start shows the install wizard. Play against yourself: sidebar → a Family → Play → New game."
echo "Remove the home afterwards: rm -rf '$home'"

if [ "$start" = 1 ]; then eval "exec $cmd"; fi
