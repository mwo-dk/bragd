#!/usr/bin/env bash
# Bragd's bills of materials and licence notices, for a release (tools/bom/README.md):
#
#   bragd-<version>.sbom.cdx.json   the app's components, Rust and npm
#   bragd-<version>.cbom.cdx.json   the cryptography it uses
#   THIRD-PARTY-NOTICES.md          every bundled component's licence text
#
#   tools/bom/generate.sh [OUT_DIR]             (default target/bom)
#
# Needs cargo-cyclonedx, cargo-about (versions below), Node.js with `npm ci` done in gui/, and
# python3. Checks as it goes: npm licences against the allowed list, and crypto.toml against the
# build and the source.
set -euo pipefail
cd "$(dirname "$0")/../.."

CYCLONEDX_RUST=0.5.7
CYCLONEDX_NPM=6.0.1
ABOUT=0.8.4

out="${1:-target/bom}"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
# The commit's time, not the clock's: the same commit gives the same files.
timestamp="$(git log -1 --format=%cI 2>/dev/null || date -u +%Y-%m-%dT%H:%M:%SZ)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"; find . -name "bom-rust.json" -not -path "./target/*" -delete' EXIT
mkdir -p "$out"

have() {
  local got
  got="$("$@" --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)" || true
  [ -n "$got" ] || { echo "generate.sh: '$*' is missing; see tools/bom/README.md" >&2; exit 1; }
  echo "$got"
}
[ "$(have cargo cyclonedx)" = "$CYCLONEDX_RUST" ] || echo "note: cargo-cyclonedx is not $CYCLONEDX_RUST" >&2
[ "$(have cargo about)" = "$ABOUT" ] || echo "note: cargo-about is not $ABOUT" >&2
have python3 >/dev/null
[ -d gui/node_modules ] || { echo "generate.sh: run 'npm ci' in gui/ first" >&2; exit 1; }

echo "== Rust SBOM"
# Every target a release builds for; build tools are not shipped.
cargo cyclonedx -q -f json --spec-version 1.5 --target all --no-build-deps --override-filename bom-rust
cp gui/src-tauri/bom-rust.json "$tmp/rust.json"

echo "== npm SBOM (what ships in the app)"
(cd gui && npx --yes "@cyclonedx/cyclonedx-npm@$CYCLONEDX_NPM" --omit dev --spec-version 1.6 \
  --output-format JSON --output-reproducible --output-file "$tmp/npm.json")

echo "== npm licences"
python3 tools/bom/bom.py licenses --sbom "$tmp/npm.json"

echo "== SBOM"
python3 tools/bom/bom.py sbom --version "$version" --timestamp "$timestamp" \
  --rust "$tmp/rust.json" --npm "$tmp/npm.json" --out "$out/bragd-$version.sbom.cdx.json"

echo "== CBOM"
cargo metadata --format-version 1 --locked > "$tmp/metadata.json"
python3 tools/bom/bom.py cbom --version "$version" --timestamp "$timestamp" \
  --metadata "$tmp/metadata.json" --out "$out/bragd-$version.cbom.cdx.json"

echo "== Third-party notices"
cargo about generate --workspace --locked -c tools/bom/about.toml tools/bom/about.hbs -o "$tmp/rust-notices.md"
python3 tools/bom/bom.py notices --version "$version" --rust "$tmp/rust-notices.md" \
  --npm-sbom "$tmp/npm.json" --metadata "$tmp/metadata.json" --out "$out/THIRD-PARTY-NOTICES.md"

echo "Done: $out"
