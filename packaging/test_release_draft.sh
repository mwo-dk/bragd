#!/usr/bin/env bash
# release-draft.sh against a fake `gh` that keeps one release in a file: no draft (made), this
# run's draft (kept, not made again), another run's draft or a published release (exit 1, left
# alone). Run from anywhere: packaging/test_release_draft.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
t="$(mktemp -d)"
trap 'rm -rf "$t"' EXIT
mkdir "$t/bin"
cat > "$t/bin/gh" <<'GH'
#!/usr/bin/env bash
set -euo pipefail
s="$STATE/release.json"
case "$1 $2" in
  "release view")
    [ -f "$s" ] || { echo "release not found" >&2; exit 1; }
    q=.; for ((i = 1; i <= $#; i++)); do [ "${!i}" = -q ] && { j=$((i + 1)); q="${!j}"; }; done
    jq -r "$q" "$s" ;;
  "release create")
    while [ $# -gt 0 ]; do [ "$1" = --notes-file ] && notes="$2"; shift; done
    jq -n --rawfile body "$notes" '{isDraft: true, body: $body, databaseId: 42}' > "$s"
    echo made >> "$STATE/log" ;;
  *) echo "fake gh: $*" >&2; exit 2 ;;
esac
GH
chmod +x "$t/bin/gh"
export PATH="$t/bin:$PATH" STATE="$t" GITHUB_RUN_ID=7
fail() { echo "FAIL: $*" >&2; exit 1; }

# No draft: made, with this run's mark and the rehearsal's.
[ "$("$here/release-draft.sh" v1.0.0 abc true)" = 42 ] || fail "no draft: id"
grep -qx '<!-- release run 7 -->' <(jq -r .body "$t/release.json") || fail "no run mark"
grep -qx '<!-- rehearsal -->' <(jq -r .body "$t/release.json") || fail "no rehearsal mark"
# This run's draft (a rerun of failed jobs): the same one, not made again.
[ "$("$here/release-draft.sh" v1.0.0 abc true)" = 42 ] || fail "own draft: id"
[ "$(wc -l < "$t/log")" = 1 ] || fail "own draft made again"
# Deleted by `discard` after a failed attempt: made again.
rm "$t/release.json"
[ "$("$here/release-draft.sh" v1.0.0 abc false)" = 42 ] || fail "after discard: id"
[ "$(wc -l < "$t/log")" = 2 ] || fail "after discard: not made again"
# Another run's draft, then a published release: left alone.
GITHUB_RUN_ID=8 "$here/release-draft.sh" v1.0.0 abc false 2>/dev/null && fail "took another run's draft"
jq '.isDraft = false' "$t/release.json" > "$t/x" && mv "$t/x" "$t/release.json"
"$here/release-draft.sh" v1.0.0 abc false 2>/dev/null && fail "took a published release"
[ "$(wc -l < "$t/log")" = 2 ] || fail "made a draft beside another"
echo "release-draft.sh: all cases pass"
