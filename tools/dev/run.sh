#!/usr/bin/env bash
# Builds Bragd from this checkout and prints how to start it with a throwaway home, so a test
# run never touches your own config, Libraries or engines. Installs nothing.
#
#   tools/dev/run.sh            build, then print the start command
#   tools/dev/run.sh --start    build, then start it at once
#   tools/dev/run.sh --keep DIR use DIR as the home (kept between runs) instead of a new one
#   tools/dev/run.sh --images   also build the engine images here (tools/dev/images.sh), then
#                               start it with BRAGD_DEV_IMAGES=1, so it runs them
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"

start=0
images=0
home=""
while [ $# -gt 0 ]; do
  case "$1" in
    --start) start=1 ;;
    --images) images=1; start=1 ;;
    --keep) home=${2:?--keep needs a folder}; shift ;;
    -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

# Every step shows its own progress: a first build takes minutes, and nothing should look hung.
echo "[1/2] Building the interface (gui/dist)…"
(cd gui && { [ -d node_modules ] || npm ci --no-audit --no-fund; } && npm run build)
echo
echo "[2/2] Building bragd-gui and bragd-helper (release; a first build takes a few minutes)…"
cargo build --release -p bragd-gui -p bragd-helper

if [ "$images" = 1 ]; then
  echo
  echo "Building the engine images (tools/dev/images.sh)…"
  "$root/tools/dev/images.sh"
fi

if [ -z "$home" ]; then home=$(mktemp -d "${TMPDIR:-/tmp}/bragd-home.XXXXXX"); fi
mkdir -p "$home"
home=$(cd "$home" && pwd)

# Images built here are run only when asked for (BRAGD_DEV_IMAGES=1), never by default.
dev=""
if [ "$images" = 1 ] || [ "${BRAGD_DEV_IMAGES:-}" = 1 ]; then dev="BRAGD_DEV_IMAGES=1 "; fi
cmd="env ${dev}HOME='$home' XDG_CONFIG_HOME='$home/.config' XDG_DATA_HOME='$home/.local/share' XDG_CACHE_HOME='$home/.cache' '$root/target/release/bragd-gui'"

echo
echo "Built. Start Bragd with a throwaway home (works in fish, bash and zsh):"
echo
echo "  $cmd"
echo
echo "First start shows the install wizard. Play against yourself: sidebar → a Family → Play → New game."
echo "Remove the home afterwards: rm -rf '$home'"

if [ "$start" = 1 ]; then eval "exec $cmd"; fi
