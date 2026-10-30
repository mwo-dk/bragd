#!/usr/bin/env python3
"""The weekly Coxswain sync issue (.github/workflows/coxswain-sync.yml, docs/coxswain-sync.md).

Reads the last synced upstream commit and the copied paths from docs/coxswain-sync.md, asks the
GitHub API (through `gh`) for the upstream commits since then that touch those paths and the
upstream issues updated since then about games, engines, images, translations or updates, and
opens one issue here titled "Coxswain sync: N commits, M issues" with a checklist. When such an
issue is open already, it is updated instead, and the boxes ticked there stay ticked.

    .github/coxswain-sync.py                       open or update the issue
    .github/coxswain-sync.py --dry-run             print the issue, change nothing
    .github/coxswain-sync.py --fixtures F.json     answer the API from F.json (with --dry-run)
    .github/coxswain-sync.py --self-test           --dry-run against .github/coxswain-sync-fixtures.json

Needs gh and GH_TOKEN (GITHUB_TOKEN reads the public upstream). Python's standard library only.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import urllib.parse
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
UPSTREAM = "mwo-dk/coxswain"
MARKER = "<!-- coxswain-sync -->"
TITLE = re.compile(r"^Coxswain sync: \d+ commits?, \d+ issues?$")
TOPICS = re.compile(r"\b(games?|chess|go|shogi|xiangqi|draughts|engines?|images?|containers?|translations?|locales?|updates?)\b", re.I)
FIXTURES = ROOT / ".github/coxswain-sync-fixtures.json"


def expand(path: str) -> list[str]:
    """`a/{b,c}.rs` as `a/b.rs`, `a/c.rs`."""
    m = re.search(r"\{([^{}]*)\}", path)
    if not m:
        return [path]
    return [p for alt in m.group(1).split(",") for p in expand(path[: m.start()] + alt + path[m.end():])]


def read_page(text: str) -> tuple[str, list[str], set[int]]:
    """The last synced commit, the upstream paths of the table, and the upstream issues carried
    already, from docs/coxswain-sync.md."""
    m = re.search(r"^Last synced: `([0-9a-f]{7,40})`", text, re.M)
    if not m:
        sys.exit("coxswain-sync.py: no 'Last synced: `<commit>`' line in docs/coxswain-sync.md")
    paths = []
    table = re.search(r"^## Paths we copied\n(.*?)(?=^## |\Z)", text, re.M | re.S)
    for line in (table.group(1) if table else "").splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 2 or set(cells[0]) <= set("-: ") or cells[0] == "coxswain":
            continue
        for p in re.findall(r"`([^`]+)`", cells[0]):
            # A bare name after a path is its sibling: `a/b/play.rs`, `review.rs`.
            if "/" not in p and paths:
                p = paths[-1].rsplit("/", 1)[0] + "/" + p
            paths += expand(p)
    if not paths:
        sys.exit("coxswain-sync.py: no paths in the table of docs/coxswain-sync.md")
    carried = re.search(r"^## Upstream issues carried as Bragd issues\n(.*?)(?=^## |\Z)", text, re.M | re.S)
    numbers = {int(n) for n in re.findall(r"#(\d+)", carried.group(1))} if carried else set()
    return m.group(1), paths, numbers


class Api:
    """`gh api`, or a dict of path → answer (the fixtures)."""

    def __init__(self, fixtures: dict | None = None):
        self.fixtures = fixtures
        self.writes: list[tuple[str, str, dict]] = []

    def get(self, path: str):
        if self.fixtures is not None:
            if path not in self.fixtures:
                sys.exit(f"coxswain-sync.py: no fixture for {path}")
            return self.fixtures[path]
        r = subprocess.run(["gh", "api", "--paginate", path], capture_output=True, text=True)
        if r.returncode != 0:
            sys.exit(f"coxswain-sync.py: gh api {path} failed: {r.stderr.strip()}")
        out = r.stdout.strip()
        if not out.startswith("["):
            return json.loads(out)
        # --paginate prints one JSON array per page.
        return [x for page in re.split(r"(?<=\])\s*(?=\[)", out) if page for x in json.loads(page)]

    def write(self, method: str, path: str, fields: dict, dry: bool):
        self.writes.append((method, path, fields))
        if dry:
            return
        args = ["gh", "api", "-X", method, path]
        for k, v in fields.items():
            args += ["-f", f"{k}={v}"]
        r = subprocess.run(args, capture_output=True, text=True)
        if r.returncode != 0:
            sys.exit(f"coxswain-sync.py: gh api -X {method} {path} failed: {r.stderr.strip()}")


def upstream_commits(api: Api, since_sha: str, paths: list[str]) -> list[dict]:
    """Upstream commits after `since_sha` touching any of `paths`, oldest first."""
    since = api.get(f"repos/{UPSTREAM}/commits/{since_sha}")["commit"]["committer"]["date"]
    found: dict[str, dict] = {}
    for p in paths:
        q = urllib.parse.urlencode({"since": since, "path": p.rstrip("/"), "per_page": 100})
        for c in api.get(f"repos/{UPSTREAM}/commits?{q}"):
            if not c["sha"].startswith(since_sha):
                found.setdefault(c["sha"], c)
    return sorted(found.values(), key=lambda c: (c["commit"]["committer"]["date"], c["sha"]))


def upstream_issues(api: Api, since_sha: str, carried: set[int]) -> list[dict]:
    """Upstream issues (not pull requests) updated since the synced commit, on our topics, less
    the ones carried as Bragd issues already."""
    since = api.get(f"repos/{UPSTREAM}/commits/{since_sha}")["commit"]["committer"]["date"]
    q = urllib.parse.urlencode({"state": "all", "since": since, "per_page": 100})
    out = []
    for i in api.get(f"repos/{UPSTREAM}/issues?{q}"):
        if "pull_request" in i or i["number"] in carried:
            continue
        text = " ".join([i["title"], *(l["name"] for l in i.get("labels", []))])
        if TOPICS.search(text):
            out.append(i)
    return sorted(out, key=lambda i: i["number"])


def body_of(repo: str, since_sha: str, commits: list[dict], issues: list[dict], ticked: set[str]) -> str:
    def box(key: str) -> str:
        return "- [x]" if key in ticked else "- [ ]"

    lines = [
        MARKER,
        f"What changed upstream in [{UPSTREAM}](https://github.com/{UPSTREAM}) since `{since_sha}`, the commit "
        f"[docs/coxswain-sync.md](https://github.com/{repo}/blob/master/docs/coxswain-sync.md) names, on the paths we copied.",
        "",
        "Each item is ported in a pull request or ticked as not wanted, with a reason in a comment. "
        "Once every box is ticked, move *Last synced* in docs/coxswain-sync.md to the newest commit below and close this issue.",
        "",
        f"## Commits ({len(commits)})",
        "",
    ]
    for c in commits:
        key = c["sha"][:7]
        subject = c["commit"]["message"].splitlines()[0]
        lines.append(f"{box(key)} [`{key}`]({c['html_url']}) {subject} ({c['commit']['committer']['date'][:10]})")
    if not commits:
        lines.append("None.")
    lines += ["", f"## Issues ({len(issues)})", ""]
    for i in issues:
        key = f"#{i['number']}"
        lines.append(f"{box(key)} [{UPSTREAM}{key}]({i['html_url']}) {i['title']} ({i['state']})")
    if not issues:
        lines.append("None.")
    return "\n".join(lines) + "\n"


def ticked_in(body: str) -> set[str]:
    """The items ticked in an earlier body: a commit's short hash or an issue's #number."""
    out = set()
    for line in body.splitlines():
        m = re.match(r"^- \[[xX]\] \[(?:`([0-9a-f]{7})`|[\w.-]+/[\w.-]+(#\d+))\]", line)
        if m:
            out.add(m.group(1) or m.group(2))
    return out


def run(api: Api, repo: str, page: str, dry: bool) -> tuple[str, str]:
    since_sha, paths, carried = read_page(page)
    commits = upstream_commits(api, since_sha, paths)
    issues = upstream_issues(api, since_sha, carried)
    title = f"Coxswain sync: {len(commits)} commit{'s' * (len(commits) != 1)}, {len(issues)} issue{'s' * (len(issues) != 1)}"
    mine = [i for i in api.get(f"repos/{repo}/issues?state=open&per_page=100")
            if "pull_request" not in i and TITLE.match(i["title"]) and MARKER in (i.get("body") or "")]
    body = body_of(repo, since_sha, commits, issues, ticked_in(mine[0]["body"]) if mine else set())
    if mine:
        n = mine[0]["number"]
        if mine[0]["title"] == title and mine[0]["body"] == body:
            print(f"#{n} is up to date: {title}")
        else:
            api.write("PATCH", f"repos/{repo}/issues/{n}", {"title": title, "body": body}, dry)
            print(f"{'would update' if dry else 'updated'} #{n}: {title}")
    elif commits or issues:
        api.write("POST", f"repos/{repo}/issues", {"title": title, "body": body}, dry)
        print(f"{'would open' if dry else 'opened'}: {title}")
    else:
        print(f"Nothing changed upstream since {since_sha}: no issue.")
    return title, body


def self_test() -> int:
    """The fixtures: two commits on copied paths (one twice, through two paths), the synced commit
    itself left out, a pull request, an off-topic issue and a carried issue left out, and an open
    sync issue whose ticked box stays ticked."""
    assert expand("a/{b,c}.rs") == ["a/b.rs", "a/c.rs"]
    assert expand("x/{a,b}/{c,d}") == ["x/a/c", "x/a/d", "x/b/c", "x/b/d"]
    sibling = "Last synced: `abcdef0`\n\n## Paths we copied\n\n| a | b |\n|---|---|\n| `s/c/play.rs`, `review.rs` | x |\n"
    assert read_page(sibling)[1] == ["s/c/play.rs", "s/c/review.rs"], read_page(sibling)
    fx = json.loads(FIXTURES.read_text(encoding="utf-8"))
    api = Api(fx["api"])
    title, body = run(api, fx["repo"], fx["page"], dry=True)
    assert title == "Coxswain sync: 2 commits, 2 issues", title
    assert api.writes and api.writes[0][:2] == ("PATCH", f"repos/{fx['repo']}/issues/7"), api.writes
    assert "- [x] [`bbbbbbb`]" in body and "- [ ] [`ccccccc`]" in body, body
    assert body.index("bbbbbbb") < body.index("ccccccc"), "oldest first"
    assert "[`aaaaaaa`]" not in body, "the synced commit itself is not listed"
    assert "#801" in body and "#802" in body and "#803" not in body and "#804" not in body and "#805" not in body, body
    _, paths, carried = read_page(fx["page"])
    assert "crates/coxswain-core/src/go.rs" in paths and "tools/" in paths and carried == {700}, (paths, carried)
    # The real page still reads: its commit, a dozen paths or more, its carried issues.
    sha, paths, carried = read_page((ROOT / "docs/coxswain-sync.md").read_text(encoding="utf-8"))
    assert len(paths) >= 12 and carried, (sha, paths, carried)
    print(body)
    print("coxswain-sync.py: self-test passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--dry-run", action="store_true", help="print the issue, change nothing")
    ap.add_argument("--fixtures", type=Path, help="answer the API from this JSON file (its `api` map)")
    ap.add_argument("--self-test", action="store_true", help="--dry-run against the fixtures, with checks")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    if a.fixtures:
        fx = json.loads(a.fixtures.read_text(encoding="utf-8"))
        api, repo, page = Api(fx["api"]), fx["repo"], fx.get("page") or (ROOT / "docs/coxswain-sync.md").read_text(encoding="utf-8")
        a.dry_run = True
    else:
        repo = os.environ.get("GITHUB_REPOSITORY", "mwo-dk/bragd")
        api, page = Api(), (ROOT / "docs/coxswain-sync.md").read_text(encoding="utf-8")
    title, body = run(api, repo, page, a.dry_run)
    if a.dry_run:
        print(f"\n{title}\n\n{body}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
