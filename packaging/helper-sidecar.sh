#!/usr/bin/env bash
# The Helper for an installer: `bragd-helper` built for the installer's target and put where
# Tauri takes a sidecar from (`bundle.externalBin`, given to `tauri build` with --config), named
# with the target triple. Tauri installs it beside `bragd-gui` without the triple, where the app
# looks for it (`bragd_helper::helper::program`).
#
#   packaging/helper-sidecar.sh [--target <triple>]
set -euo pipefail
cd "$(dirname "$0")/.."
target=""
if [ "${1:-}" = "--target" ]; then target="${2:?--target needs a triple}"; fi
triple="${target:-$(rustc -vV | sed -n 's/^host: //p')}"
exe=""
case "$triple" in *windows*) exe=".exe" ;; esac
cargo build --release --locked -p bragd-helper ${target:+--target "$target"}
mkdir -p gui/src-tauri/binaries
cp "target/${target:+$target/}release/bragd-helper$exe" "gui/src-tauri/binaries/bragd-helper-$triple$exe"
echo "gui/src-tauri/binaries/bragd-helper-$triple$exe"
