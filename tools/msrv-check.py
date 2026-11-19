#!/usr/bin/env python3
"""Fail when a BSD cannot build the locked dependencies with the Rust it packages.

Pull requests do not run the BSD virtual machines (.github/workflows/bsd.yml), and those take
Rust from their own packages (OpenBSD has no rustup), which can be older than what a fresh
dependency asks for. Without this the update would show up only in the nightly run or the
release. Only what each system builds is checked, so nothing is held back on the others.

    tools/msrv-check.py
"""
import json
import subprocess
import sys

# The Rust each system's packages give bsd.yml (pkg install / pkgin / pkg_add rust). Raise a
# line when that system's packages move on.
PACKAGED_RUST = {
    "x86_64-unknown-freebsd": "1.98",  # FreeBSD 14, quarterly packages
    "x86_64-unknown-netbsd": "1.96",   # NetBSD 10.1, pkgin
    "x86_64-unknown-openbsd": "1.94",  # OpenBSD 7.9
}


def version(v):
    parts = [int(p) for p in v.split(".")[:3]]
    return tuple(parts + [0] * (3 - len(parts)))


def too_new(packages, built, rust):
    """The packages of `built` whose `rust_version` is newer than `rust`."""
    return sorted(
        f'{p["name"]} {p["version"]} needs Rust {p["rust_version"]}'
        for p in packages
        if p["id"] in built and p.get("rust_version") and version(p["rust_version"]) > version(rust))


def check(target, rust):
    meta = json.loads(subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target],
        check=True, capture_output=True, text=True).stdout)
    return too_new(meta["packages"], {n["id"] for n in meta["resolve"]["nodes"]}, rust)


def main():
    if sys.argv[1:] == ["--self-test"]:
        pkgs = [{"id": "a", "name": "a", "version": "1.0.0", "rust_version": "1.95"},
                {"id": "b", "name": "b", "version": "2.0.0", "rust_version": "1.94.1"},
                {"id": "c", "name": "c", "version": "3.0.0", "rust_version": "1.99"}]
        assert too_new(pkgs, {"a", "b"}, "1.94") == ["a 1.0.0 needs Rust 1.95", "b 2.0.0 needs Rust 1.94.1"]
        assert too_new(pkgs, {"a", "b"}, "1.95") == [], "1.95 builds 1.95 and 1.94.1"
        assert too_new(pkgs, {"a"}, "1.90") == ["a 1.0.0 needs Rust 1.95"], "c is not built"
        print("self-test passed")
        return 0
    failed = False
    for target, rust in PACKAGED_RUST.items():
        bad = check(target, rust)
        if bad:
            failed = True
            print(f"{target} packages Rust {rust}, which cannot build:")
            print("\n".join(f"  {b}" for b in bad))
        else:
            print(f"{target}: every dependency builds with Rust {rust}.")
    if failed:
        print("Keep the older version (cargo update <name> --precise <version>) until that system"
              " packages a newer Rust, then raise its line in tools/msrv-check.py.")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
