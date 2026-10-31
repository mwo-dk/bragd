#!/usr/bin/env python3
"""Delete old untagged versions of Bragd's engine images on ghcr.io (images.yml, after a master build).

For each folder packaging/images/<engine>, the container package ghcr.io/mwo-dk/bragd-<engine>
keeps:
  - every tagged version (`latest`): never deleted;
  - every digest pinned on this commit, by the `PIN_*` constants of
    crates/bragd-engines/src/known.rs;
  - every digest pinned in the newest 3 release tags (`v*`, by version number); a tag without
    known.rs pins nothing;
  - the newest 5 untagged images;
  - every manifest a kept multi-arch index names. GHCR lists the per-platform manifests of a
    multi-arch image as versions of their own, untagged, so `latest`'s amd64 and arm64
    manifests look untagged; deleting them would break `latest` and every kept pin.
Every other untagged version is deleted. The plan (what is kept and why, what goes) is printed
first. A manifest that cannot be read stops the run before anything is deleted.

    python3 tools/ghcr-prune.py [--dry-run]   # needs GITHUB_TOKEN, and docker logged in to ghcr.io
    python3 tools/ghcr-prune.py --self-test   # against tools/fixtures/ghcr-prune/, no network

GHCR_OWNER (default mwo-dk) and GHCR_OWNER_KIND (`users`, the default, or `orgs`) say whose
packages these are. Python 3 standard library only.
"""

import glob
import json
import os
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

KNOWN = "crates/bragd-engines/src/known.rs"
PIN = re.compile(r'pub const PIN_\w+: Option<&str> = Some\("(sha256:[0-9a-f]{64})"\)')
RELEASES = 3
NEWEST = 5
FIXTURES = Path(__file__).parent / "fixtures" / "ghcr-prune"


def pins(known_at, tags):
    """{digest: why} pinned by known.rs on this commit and in the newest RELEASES `v*` tags.
    known_at(ref) is known.rs at ref (None for this commit), or None when the file is missing."""
    newest = sorted(tags, key=lambda t: [int(n) for n in re.findall(r"\d+", t)], reverse=True)[:RELEASES]
    out = {}
    for ref in [None] + newest:
        for d in PIN.findall(known_at(ref) or ""):
            out.setdefault(d, f"pinned in {ref or 'this commit'}")
    return out


def plan(versions, pinned, children):
    """(keep {digest: why}, delete [version]) for one package. versions: the API's list;
    pinned: {digest: why}; children(digest): the digests an index names ([] for a manifest)."""
    kids = {v["name"]: children(v["name"]) for v in versions}
    every_kid = {k for ks in kids.values() for k in ks}
    keep = {}
    for v in versions:
        if tags := v["metadata"]["container"]["tags"]:
            keep[v["name"]] = "tagged " + ", ".join(tags)
    for d, why in pinned.items():
        keep.setdefault(d, why)
    roots = [v for v in versions if v["name"] not in keep and v["name"] not in every_kid]
    for v in sorted(roots, key=lambda v: v["created_at"], reverse=True)[:NEWEST]:
        keep[v["name"]] = f"one of the newest {NEWEST} untagged"
    for d in list(keep):
        for k in kids.get(d, []):
            keep.setdefault(k, f"named by {d[:19]}…")
    return keep, [v for v in versions if v["name"] not in keep]


def api(method, path):
    req = urllib.request.Request(f"https://api.github.com{path}", method=method, headers={
        "Authorization": "Bearer " + os.environ["GITHUB_TOKEN"],
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
    })
    with urllib.request.urlopen(req, timeout=60) as r:
        body = r.read()
    return json.loads(body) if body else None


def git_known(ref):
    if ref is None:
        return Path(KNOWN).read_text(encoding="utf-8")
    r = subprocess.run(["git", "show", f"{ref}:{KNOWN}"], capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else None


def main():
    if "--self-test" in sys.argv:
        return self_test()
    dry = "--dry-run" in sys.argv
    owner = os.environ.get("GHCR_OWNER", "mwo-dk")
    kind = os.environ.get("GHCR_OWNER_KIND", "users")
    tags = subprocess.run(["git", "tag", "-l", "v*"], capture_output=True, text=True, check=True).stdout.split()
    pinned = pins(git_known, tags)
    engines = sorted(Path(p).parent.name for p in glob.glob("packaging/images/*/Dockerfile"))
    for engine in engines:
        package = f"bragd-{engine}"
        base = f"/{kind}/{owner}/packages/container/{package}/versions"
        versions, page = [], 1
        while batch := api("GET", f"{base}?per_page=100&page={page}"):
            versions += batch
            page += 1

        def children(digest):
            raw = subprocess.run(["docker", "buildx", "imagetools", "inspect", "--raw", f"ghcr.io/{owner}/{package}@{digest}"],
                                 capture_output=True, text=True, check=True).stdout
            return [m["digest"] for m in json.loads(raw).get("manifests", [])]

        keep, delete = plan(versions, pinned, children)
        print(f"## {package}: {len(versions)} versions, {len(keep)} kept, {len(delete)} to delete")
        for d, why in keep.items():
            print(f"keep    {d}  {why}")
        for v in delete:
            print(f"delete  {v['name']}  (id {v['id']}, {v['created_at']})")
        if not dry:
            for v in delete:
                api("DELETE", f"{base}/{v['id']}")
    return 0


def self_test():
    f = json.loads((FIXTURES / "fixture.json").read_text(encoding="utf-8"))
    known = {p.stem: p.read_text(encoding="utf-8") for p in (FIXTURES / "known").glob("*.rs")}
    pinned = pins(lambda ref: known.get(ref or "master"), f["tags"])
    keep, delete = plan(f["versions"], pinned, lambda d: f["manifests"].get(d, []))
    got = {"keep": dict(sorted(keep.items())), "delete": sorted(v["name"] for v in delete)}
    want = {"keep": dict(sorted(f["expect"]["keep"].items())), "delete": sorted(f["expect"]["delete"])}
    if got != want:
        print("ghcr-prune self-test FAILED\nwant:", json.dumps(want, indent=1), "\ngot:", json.dumps(got, indent=1))
        return 1
    print(f"ghcr-prune self-test: {len(keep)} kept, {len(delete)} deleted, as expected")
    return 0


if __name__ == "__main__":
    sys.exit(main())
