#!/usr/bin/env bash
# Builds Bragd's engine images from packaging/images/<engine> with podman, here, as
# localhost/bragd-<engine>:dev: for trying an engine whose image is not published yet. Bragd runs
# these unpinned images only when started with BRAGD_DEV_IMAGES=1 (tools/dev/run.sh --images
# does that); otherwise it never does. Installs nothing on the host: each engine stays inside its
# image, run sealed as Bragd runs every image.
#
#   tools/dev/images.sh                   build every image
#   tools/dev/images.sh stockfish lc0     build only these
#   tools/dev/images.sh --list            list the images there are to build
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
dir="$root/packaging/images"
all=$(find "$dir" -mindepth 2 -maxdepth 2 -name Dockerfile -printf '%h\n' | xargs -rn1 basename | sort)

case "${1:-}" in
  -h|--help) sed -n '2,10p' "$0"; exit 0 ;;
  --list) echo "$all"; exit 0 ;;
esac

if ! command -v podman >/dev/null; then
  echo "podman is not installed: Bragd's engine images are built with podman." >&2
  exit 1
fi

engines=("$@")
if [ ${#engines[@]} -eq 0 ]; then mapfile -t engines <<<"$all"; fi
for e in "${engines[@]}"; do
  if [ ! -f "$dir/$e/Dockerfile" ]; then
    echo "There is no image called $e. There are: $(echo "$all" | tr '\n' ' ')" >&2
    exit 2
  fi
done

built=() failed=()
n=0
for e in "${engines[@]}"; do
  n=$((n + 1))
  echo
  echo "[$n/${#engines[@]}] Building localhost/bragd-$e:dev from packaging/images/$e…"
  # podman's own progress, step by step: a long compile shows as it goes.
  if podman build --layers -t "localhost/bragd-$e:dev" "$dir/$e"; then
    built+=("$e")
  else
    failed+=("$e")
  fi
done

echo
if [ ${#built[@]} -gt 0 ]; then
  echo "Built: ${built[*]}"
  echo
  echo "Start Bragd so it runs them (works in fish, bash and zsh):"
  echo
  echo "  env BRAGD_DEV_IMAGES=1 $root/tools/dev/run.sh --start"
  echo
  echo "Remove one again: podman image rm localhost/bragd-<engine>:dev"
fi
if [ ${#failed[@]} -gt 0 ]; then
  echo "Failed: ${failed[*]} (see podman's output above)" >&2
  exit 1
fi
