#!/usr/bin/env bash
# This run's draft of TAG: its id, after making it when there is none. release.yml's `draft` job
# makes it; `upload` calls this again, so after a failed attempt whose `discard` deleted the draft,
# `gh run rerun <id> --failed` makes it again and finishes the release. A draft or release that
# is not this run's is left alone: exit 1.
#
#   packaging/release-draft.sh TAG SHA REHEARSAL      (REHEARSAL: true or false)
#
# Needs GH_TOKEN and GITHUB_RUN_ID. The draft names its run in a comment of its notes.
set -euo pipefail
tag=$1 sha=$2 rehearsal=$3
here="$(cd "$(dirname "$0")" && pwd)"
mark="<!-- release run $GITHUB_RUN_ID -->"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
if info="$(gh release view "$tag" --json isDraft,body,databaseId 2>"$tmp/err")"; then
  if [ "$(jq -r .isDraft <<<"$info")" = true ] && grep -qxF "$mark" <(jq -r .body <<<"$info"); then
    jq -r .databaseId <<<"$info"
    exit 0
  fi
  echo "::error::$tag is another run's draft, or released: run $GITHUB_RUN_ID leaves it alone." >&2
  exit 1
elif ! grep -q "release not found" "$tmp/err"; then
  cat "$tmp/err" >&2
  exit 1
fi
{ cat "$here/../.github/release-notes.md"; printf '\n%s\n' "$mark"
  [ "$rehearsal" != true ] || echo '<!-- rehearsal -->'; } > "$tmp/notes.md"
"$here/retry.sh" gh release create "$tag" --draft --target "$sha" --title "Bragd $tag" --notes-file "$tmp/notes.md" >&2
gh release view "$tag" --json databaseId -q .databaseId
