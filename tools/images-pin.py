#!/usr/bin/env python3
"""Bring the container images Bragd pins up to date.

Every image Bragd uses is pinned by its multi-arch index digest. Bragd's own engine images are
pinned in `crates/bragd-engines/src/known.rs` (each engine's `tag` and its `PIN_` constant; its
`image_size` is set to what a pull fetches on amd64); every other image is written as
`name:tag@sha256:…`, in the `FROM` lines of `packaging/images/*/Dockerfile` and the job containers
of the workflows. An engine image not published yet is reported, not pinned.
This asks each image's registry (anonymously, as for any public image) which digest its tag has
now, writes the new ones into those files, and prints a Markdown table of what changed, old to
new. Nothing changed: no output. Run it before a release, check each image's release notes and
the sizes Bragd shows before a download, and run the tests on the new pins.

    python3 tools/images-pin.py            # update the files, print the table
    python3 tools/images-pin.py --check    # print only, change nothing
"""

import glob
import json
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

FILES = sorted(glob.glob("crates/*/src/**/*.rs", recursive=True) + glob.glob("packaging/images/*/Dockerfile") + glob.glob(".github/workflows/*.yml"))
# Bragd's own engine images: known.rs names each one's tag and its PIN_ constant.
KNOWN = "crates/bragd-engines/src/known.rs"
REGISTRY = "ghcr.io/mwo-dk"
OWN = re.compile(r'key: "([a-z0-9-]+)",[^}]*?tag: Some\("([^"]+)"\),\s*pin: (PIN_\w+)')
PINNED = re.compile(r"\b((?:[a-z0-9.-]+\.[a-z]{2,}/)?[a-z0-9._/-]+):([\w.-]+)@(sha256:[0-9a-f]{64})")
ACCEPT = ", ".join(
    [
        "application/vnd.oci.image.index.v1+json",
        "application/vnd.docker.distribution.manifest.list.v2+json",
        "application/vnd.oci.image.manifest.v1+json",
        "application/vnd.docker.distribution.manifest.v2+json",
    ]
)


def split(name):
    """(registry host, repository) of an image name, as `docker pull` reads it."""
    first, _, rest = name.partition("/")
    if rest and ("." in first or ":" in first):
        host, repo = first, rest
    else:
        host, repo = "docker.io", name
    if host == "docker.io":
        host = "registry-1.docker.io"
        if "/" not in repo:
            repo = "library/" + repo
    return host, repo


def manifest(name, ref, method="GET"):
    """(headers, body) of the manifest `ref` (a tag or a digest) of `name`, anonymously."""
    host, repo = split(name)
    url = f"https://{host}/v2/{repo}/manifests/{ref}"
    headers = {"Accept": ACCEPT, "User-Agent": "bragd-images-pin"}
    for _ in range(2):
        req = urllib.request.Request(url, method=method, headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return r.headers, r.read()
        except urllib.error.HTTPError as e:
            challenge = e.headers.get("WWW-Authenticate", "")
            if e.code != 401 or not challenge.startswith("Bearer ") or "Authorization" in headers:
                raise
            # An anonymous pull token, as the registry asks for one.
            params = dict(re.findall(r'(\w+)="([^"]*)"', challenge))
            realm = params.pop("realm")
            with urllib.request.urlopen(f"{realm}?{urllib.parse.urlencode(params)}", timeout=60) as t:
                body = json.load(t)
            headers["Authorization"] = "Bearer " + (body.get("token") or body["access_token"])
    raise RuntimeError(f"{name}:{ref}: no answer")


def digest(name, tag):
    """The digest `name:tag` has now: the index's, for a multi-arch image."""
    return manifest(name, tag, "HEAD")[0]["Docker-Content-Digest"]


def download_size(name, ref):
    """The bytes a pull of `name@ref` fetches on amd64: its config and its compressed layers."""
    index = json.loads(manifest(name, ref)[1])
    for m in index.get("manifests", []):
        if m.get("platform", {}).get("architecture") == "amd64":
            index = json.loads(manifest(name, m["digest"])[1])
            break
    return index["config"]["size"] + sum(layer["size"] for layer in index["layers"])


def own_pins(known):
    """known.rs with each engine image's PIN_ and image_size set to what its version tag on
    REGISTRY is now; the changes as (image, old, new), and the images not read."""
    changed, failed = [], []
    for key, tag, const in OWN.findall(known):
        name = f"{REGISTRY}/bragd-{key}"
        try:
            new = digest(name, tag)
            size = download_size(name, new)
        except Exception as e:  # not published yet, or the registry is down
            failed.append(f"{name}:{tag}: {e}")
            continue
        line = re.compile(rf'pub const {const}: Option<&str> = (None|Some\("(sha256:[0-9a-f]{{64}})"\));')
        old = line.search(known).group(2)
        if old != new:
            changed.append((f"{name}:{tag}", old or "none", new))
            known = line.sub(f'pub const {const}: Option<&str> = Some("{new}");', known)
        known = re.sub(rf'(key: "{re.escape(key)}",[^}}]*?image_size: )[0-9_]+', lambda m: m.group(1) + f"{size:_}", known)
    return known, changed, failed


def main():
    check = "--check" in sys.argv
    texts = {f: open(f, encoding="utf-8").read() for f in FILES}
    texts = {f: t for f, t in texts.items() if PINNED.search(t)}
    found = sorted({m.groups() for t in texts.values() for m in PINNED.finditer(t)})
    changed, failed = [], []
    for name, tag, old in found:
        if old == "sha256:" + "0" * 64:  # a test's placeholder, not a pin
            continue
        try:
            new = digest(name, tag)
        except Exception as e:  # one registry down does not hold up the others
            failed.append(f"{name}:{tag}: {e}")
            continue
        if new and new != old:
            changed.append((name, tag, old, new))
            for f in texts:
                texts[f] = texts[f].replace(f"{name}:{tag}@{old}", f"{name}:{tag}@{new}")
    known, own, own_failed = own_pins(open(KNOWN, encoding="utf-8").read())
    texts[KNOWN] = known
    changed += [(image.rsplit(":", 1)[0], image.rsplit(":", 1)[1], old, new) for image, old, new in own]
    failed += own_failed
    if not check:
        for f, t in texts.items():
            if t != open(f, encoding="utf-8").read():
                open(f, "w", encoding="utf-8").write(t)
    if changed:
        print("| Image | Old digest | New digest |\n|---|---|---|")
        for name, tag, old, new in changed:
            print(f"| `{name}:{tag}` | {f'`{old[:19]}…`' if old.startswith('sha256:') else old} | `{new[:19]}…` |")
    for f in failed:
        print(f"::warning title=Digest not read::{f}", file=sys.stderr)
    # Every image failing is the network, not the images: say so with a failure.
    return 1 if found and len(failed) == len(found) else 0


if __name__ == "__main__":
    sys.exit(main())
