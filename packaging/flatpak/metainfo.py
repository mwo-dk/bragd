#!/usr/bin/env python3
"""The metainfo file with its screenshots (from docs/screenshots, at this version's tag) and its
releases (the newest rows of README.md's changelog), written to stdout. Run from the repository
root: python3 packaging/flatpak/metainfo.py > io.github.mwo_dk.Bragd.metainfo.xml"""
import html
import os
import re
import sys
import time

HERE = "packaging/flatpak/io.github.mwo_dk.Bragd.metainfo.xml"
# The pictures Flathub shows, first the default. Empty until the GUI's screenshots are taken
# (docs/screenshots/cap.py); the linter's missing-screenshots error is let through until then.
SHOTS: list[tuple[str, str]] = []
KEEP = 10


def plain(md):
    """A changelog cell as plain text: the docs links at its end go, other links become their
    words, emphasis and code marks go."""
    md = re.sub(r"(\s*\[[^\]]*\]\([^)]*\)(\s*·)?)+\s*$", "", md)
    md = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", md)
    md = md.replace("\\|", "|").replace("**", "").replace("`", "")
    md = re.sub(r"(?<![\w*])\*([^*]+)\*(?![\w*])", r"\1", md)
    # AppStream allows no web addresses in a description: they keep their words without the scheme.
    return re.sub(r"https?://", "", md).strip()


def main():
    version = re.search(r'^version = "([^"]+)"', open("Cargo.toml").read(), re.M).group(1)
    rows = re.findall(r"^\| \*\*(\d+\.\d+\.\d+)\*\* \| (\d{4}-\d\d-\d\d) \| (.*) \|$", open("README.md").read(), re.M)
    # Before the first release there are no rows; after it, the newest row is this version's.
    if rows and rows[0][0] != version:
        sys.exit(f"metainfo.py: README.md's changelog does not start with {version}")
    releases = "\n".join(
        f'    <release version="{v}" date="{d}">\n      <description>\n        <p>{html.escape(plain(t), quote=False)}</p>\n      </description>\n'
        f'      <url>https://github.com/mwo-dk/bragd/releases/tag/v{v}</url>\n    </release>'
        for v, d, t in rows[:KEEP]
    )
    # Before the first release, this build as a development snapshot: AppStream wants one release.
    if not rows:
        day = time.strftime("%Y-%m-%d", time.gmtime(int(os.environ.get("SOURCE_DATE_EPOCH", time.time()))))
        releases = f'    <release version="{version}" date="{day}" type="development"/>'
    missing = [f for f, _ in SHOTS if not os.path.isfile(f"docs/screenshots/{f}")]
    if missing:
        sys.exit(f"metainfo.py: no {', '.join(missing)} in docs/screenshots")
    base = f"https://raw.githubusercontent.com/mwo-dk/bragd/v{version}/docs/screenshots"
    shots = "\n".join(
        f'    <screenshot{" type=\"default\"" if i == 0 else ""}>\n      <caption>{c}</caption>\n      <image>{base}/{f}</image>\n    </screenshot>'
        for i, (f, c) in enumerate(SHOTS)
    )
    text = open(HERE).read()
    text = text.replace("  <screenshots/>\n", f"  <screenshots>\n{shots}\n  </screenshots>\n" if shots else "")
    text = text.replace("  <releases/>\n", f"  <releases>\n{releases}\n  </releases>\n" if releases else "")
    sys.stdout.write(text)


if __name__ == "__main__":
    main()
