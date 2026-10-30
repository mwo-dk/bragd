#!/usr/bin/env python3
"""The release pull request's changes, made in the checkout it runs in (CLAUDE.md, Shipping).

    packaging/release-pr.py --next    print the next version and change nothing
    --unreleased-ok                   (with either) go on although the current version has no
                                      published GitHub release (the owner's decision only)
    --open-issues-ok                  (with either) go on although the new version's milestone
                                      has open issues (the owner's decision only)
    packaging/release-pr.py           make the release: print the version, then
        bump [workspace.package] in Cargo.toml, Cargo.lock with it (cargo update --workspace),
        every changelog.d/ row moved into one row for the version in docs/changelog.md and
        README.md (which keeps ten), and the changelog.d/ files deleted.

A changelog.d/ file holds one row, then its markers:

    | **What a user gets.** In plain words. | [Page](area/page.md#anchor) | <!-- feature --> <!-- tour: card-id -->

A minor release when any row is marked `<!-- feature -->`, a patch release otherwise; a major one
is made by hand. Exit codes, each changing nothing:

  2  changelog.d/ has no rows.
  3  the current version has no published GitHub release (`gh release view`): bumping past it
     would leave it unpublished for good, so its release run is finished first. Before the first
     release (no published release at all) this does not hold.
  4  the new version's GitHub milestone (named X.Y.Z) has open issues: a release is done when its
     milestone is empty. Issues labelled `post-release` or titled "After the release…" are left
     open on purpose and do not count; a version with no milestone is not held.
  5  a feature row has no tour card: every `<!-- feature -->` row names its card with
     `<!-- tour: card-id -->`, and crates/bragd-core/src/tour.rs has a card with that id (the
     string "card-id" in quotes). The release gate of CLAUDE.md, Every feature.

Run from the repository's root.
"""
import datetime
import json
import re
import subprocess
import sys
from pathlib import Path

FEATURE = "<!-- feature -->"
TOUR = re.compile(r"<!--\s*tour:\s*([\w.-]+)\s*-->")
MARKERS = re.compile(r"(\s*<!--.*?-->)+\s*$")
TOUR_RS = Path("crates/bragd-core/src/tour.rs")
LINK = re.compile(r"\]\(([^)]+)\)")


def gh(args: list[str], why: str) -> subprocess.CompletedProcess:
    try:
        return subprocess.run(["gh", *args], capture_output=True, text=True)
    except FileNotFoundError:
        sys.exit(f"release-pr.py: gh is needed {why}")


def published(tag: str) -> bool:
    """Whether `tag` has a published (not draft) GitHub release, or none is published at all
    (the first release); exits on any other gh fault."""
    why = "to see whether the current version is released (or pass --unreleased-ok)"
    r = gh(["release", "view", tag, "--json", "isDraft", "-q", ".isDraft"], why)
    if r.returncode == 0:
        return r.stdout.strip() == "false"
    if "release not found" not in r.stderr:
        sys.exit(f"release-pr.py: gh release view {tag} failed: {r.stderr.strip()}")
    r = gh(["release", "list", "--exclude-drafts", "--limit", "1", "--json", "tagName", "-q", ".[].tagName"], why)
    if r.returncode != 0:
        sys.exit(f"release-pr.py: gh release list failed: {r.stderr.strip()}")
    return r.stdout.strip() == ""


def gh_json(path: str) -> list:
    """`gh api --paginate` of a list endpoint; exits on any gh fault."""
    r = gh(["api", "--paginate", path], "to read the milestone (or pass --open-issues-ok)")
    if r.returncode != 0:
        sys.exit(f"release-pr.py: gh api {path} failed: {r.stderr.strip()}")
    # --paginate prints one JSON array per page.
    return [x for page in re.split(r"(?<=\])\s*(?=\[)", r.stdout.strip()) if page for x in json.loads(page)]


def post_release(issue: dict) -> bool:
    """Left open on purpose until the release is out: checking every channel."""
    return (any(l["name"] == "post-release" for l in issue.get("labels", []))
            or issue["title"].lower().startswith("after the release"))


def open_issues(version: str) -> list[dict]:
    """The open issues (not pull requests) of milestone `version` that hold its release."""
    stones = [m for m in gh_json("repos/{owner}/{repo}/milestones?state=all&per_page=100")
              if m["title"] == version]
    if not stones:
        return []
    issues = gh_json(f"repos/{{owner}}/{{repo}}/issues?milestone={stones[0]['number']}&state=open&per_page=100")
    return [i for i in issues if "pull_request" not in i and not post_release(i)]


def missing_tours(frags: dict[Path, str]) -> list[str]:
    """Each feature row without a tour card: no `<!-- tour: id -->`, or an id tour.rs lacks."""
    cards = TOUR_RS.read_text(encoding="utf-8") if TOUR_RS.exists() else ""
    out = []
    for f, row in frags.items():
        if FEATURE not in row:
            continue
        m = TOUR.search(row)
        if not m:
            out.append(f"{f}: a feature row with no <!-- tour: card-id --> marker")
        elif f'"{m.group(1)}"' not in cards:
            out.append(f"{f}: tour card \"{m.group(1)}\" is not in {TOUR_RS}")
    return out


def main() -> int:
    args = sys.argv[1:]
    unreleased_ok = "--unreleased-ok" in args
    open_ok = "--open-issues-ok" in args
    args = [a for a in args if a not in ("--unreleased-ok", "--open-issues-ok")]
    frags = {f: f.read_text(encoding="utf-8").strip() for f in sorted(Path("changelog.d").glob("*.md"))}
    if not frags:
        print("changelog.d/ has no rows: nothing to release.", file=sys.stderr)
        return 2
    rows = list(frags.values())
    features = [r for r in rows if FEATURE in r]
    cargo = Path("Cargo.toml").read_text(encoding="utf-8")
    old = re.search(r'^\[workspace\.package\]\n(?:.*\n)*?version = "(\d+)\.(\d+)\.(\d+)"', cargo, re.M)
    major, minor, patch = map(int, old.groups())
    was = ".".join(old.groups())
    if not unreleased_ok and not published(f"v{was}"):
        print(f"release-pr.py: v{was}, the version in Cargo.toml, has no published GitHub release. "
              f"A new release would skip it for good. Finish its release first: fix what failed, "
              f"then start Release by hand (gh workflow run release.yml). To skip v{was} on "
              f"purpose, pass --unreleased-ok.", file=sys.stderr)
        return 3
    new = f"{major}.{minor + 1}.0" if features else f"{major}.{minor}.{patch + 1}"
    if not open_ok and (held := open_issues(new)):
        print(f"release-pr.py: milestone {new} has {len(held)} open issue(s); a release is done when "
              f"its milestone is empty. Close them, or move them to a later milestone:", file=sys.stderr)
        for i in held:
            print(f"  #{i['number']} {i['title']}", file=sys.stderr)
        print("To release anyway, pass --open-issues-ok (the owner's decision only).", file=sys.stderr)
        return 4
    if lacking := missing_tours(frags):
        print("release-pr.py: every feature ships a tour card (CLAUDE.md, Every feature):", file=sys.stderr)
        for line in lacking:
            print(f"  {line}", file=sys.stderr)
        return 5
    print(new)
    if args == ["--next"]:
        return 0

    cargo, n = re.subn(rf'^version = "{re.escape(was)}"$', f'version = "{new}"', cargo, count=1, flags=re.M)
    if n != 1:
        sys.exit(f"release-pr.py: Cargo.toml's version {was} line not found")
    Path("Cargo.toml").write_text(cargo, encoding="utf-8")

    # Features first, then fixes; each row's text and then its links, as from docs/.
    text = []
    for r in features + [r for r in rows if r not in features]:
        cells = [c.strip() for c in re.split(r"(?<!\\)\|", MARKERS.sub("", r))[1:-1]]
        text.append(" ".join(cells))
    date = datetime.datetime.now(datetime.timezone.utc).date().isoformat()
    row = f"| **{new}** | {date} | {' '.join(text)} |"

    def readme_link(m: re.Match) -> str:
        t = m.group(1)
        return m.group(0) if re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:|^[#/]", t) else f"](docs/{t})"

    head = "| Version | Date | What's new |\n|---|---|---|\n"
    for page, line, keep in (("docs/changelog.md", row, None), ("README.md", LINK.sub(readme_link, row), 10)):
        body = Path(page).read_text(encoding="utf-8")
        at = body.index(head, body.index("## Versions" if page.startswith("docs") else "## Changelog")) + len(head)
        body = body[:at] + line + "\n" + body[at:]
        if keep:
            lines = body.split("\n")
            versions = [i for i, l in enumerate(lines) if re.match(r"^\| \*\*\d+\.\d+\.\d+\*\* \|", l)]
            for i in reversed(versions[keep:]):
                del lines[i]
            body = "\n".join(lines)
        Path(page).write_text(body, encoding="utf-8")

    for f in frags:
        f.unlink()
    subprocess.run(["cargo", "update", "--workspace", "--quiet"], check=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
