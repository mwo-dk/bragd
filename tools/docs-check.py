#!/usr/bin/env python3
"""The docs check: README.md and docs/ are held to the code, on every pull request (CLAUDE.md,
Documentation). It blocks merge: it runs in ci.yml's Docs job, part of "CI result".

  python3 tools/docs-check.py [--base <ref>]

1. Every relative link and #anchor in the Markdown files at the root and in docs/ points to a
   file and a heading (GitHub's anchors: lowercase, spaces to '-', punctuation dropped,
   duplicates -1, -2 …; also <a id>/<a name>/id= anchors). Pictures and GIFs too.
2. Once a version is released, the version in Cargo.toml has its row in docs/changelog.md, and the
   README's changelog table is the newest ten rows of docs/changelog.md, word for word (links
   compared by where they lead). Each changelog.d/ file is one row,
   `| **What a user gets.** In plain words. | [Page](area/page.md#anchor) |`, its links written
   as from docs/; a feature's row ends with `<!-- feature -->` and names its tour card,
   `<!-- tour: card-id -->` (packaging/release-pr.py checks the card exists). A release (the
   version raised over the base branch's) leaves changelog.d/ empty.
3. No version older than the current major.minor is shown as Bragd's own (`Bragd 1.2.0`,
   `bragd/1.2.0`, `Bragd_1.2.0_amd64.deb`) outside the changelog: such examples go stale, so they
   say `X.Y.Z`.
4. Every page in docs/README.md's order (its `## [..](..)` headings and `| [..](..) |` table rows,
   then faq.md and changelog.md) starts with a link back to the README and ends with *Previous*
   and *Next* links to its neighbours in that order; a page of more than 150 lines has a table
   of contents.
5. No `<!-- screenshot: name.png: what it must show -->` placeholder is left when the version
   raises its minor or major number over the base branch's (--base, else
   origin/$GITHUB_BASE_REF, else origin/master); otherwise each one is a warning. Pictures in
   NEEDS_HARDWARE are only warned about.
6. Every question under a *Questions* heading is a `####` heading, so other pages can link to its
   answer: no question as a bold paragraph, none at another level.
7. Every name the code has is documented: each `config.toml` key (dotted, `look.theme`), each
   setting descriptor (`theme`, `family_go`, `variant_go-9`), each Theme, Look and Board theme,
   each F1 feature id, and each flag of `bragd-gui` and `bragd-helper` is written in backticks on
   some page under docs/; and each F1 feature's page exists. The names are read from the Rust
   source (check_coverage), so a new one fails the check until a page names it.

Python 3.11+, standard library only.
"""

import argparse
import functools
import itertools
import os
import re
import subprocess
import sys
import tomllib
import urllib.parse
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GHA = os.environ.get("GITHUB_ACTIONS") == "true"
errors: list[str] = []


def error(where: str, msg: str) -> None:
    errors.append(f"{where}: {msg}")
    if GHA:
        file, _, line = where.partition(":")
        print(f"::error file={file},line={line or 1}::{msg}")


def warning(where: str, msg: str) -> None:
    file, _, line = where.partition(":")
    print(f"::warning file={file},line={line or 1}::{msg}" if GHA else f"warning: {where}: {msg}")


@functools.cache
def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def minor(v: str) -> tuple[int, ...]:
    return tuple(map(int, v.split(".")[:2]))


def pages() -> list[Path]:
    """README.md and every page under docs/."""
    return [ROOT / "README.md", *sorted((ROOT / "docs").rglob("*.md"))]


def linked() -> list[Path]:
    """Every Markdown file whose links are checked: the pages, the other files at the root, and
    the packaging and tools notes."""
    extra = [*sorted(ROOT.glob("*.md")), *sorted((ROOT / "packaging").rglob("*.md")), *sorted((ROOT / "tools").rglob("*.md"))]
    return pages() + [p for p in extra if p not in pages()]


def rel(p: Path) -> str:
    return p.relative_to(ROOT).as_posix()


FENCE = re.compile(r"^\s*(```|~~~)")


def prose_lines(text: str):
    """(line number, line) outside fenced code blocks."""
    fence = None
    for n, line in enumerate(text.splitlines(), 1):
        m = FENCE.match(line)
        if m:
            fence = None if fence == m.group(1) else (fence or m.group(1))
            continue
        if fence is None:
            yield n, line


def slug(heading: str) -> str:
    """GitHub's anchor for a heading's text (github-slugger)."""
    t = re.sub(r"!?\[([^\]]*)\]\([^)]*\)", r"\1", heading)  # links and pictures: their text
    # HTML tags go, but not in code, where `<name>` is text
    t = "".join(s if i % 2 else re.sub(r"<[^>]+>", "", s) for i, s in enumerate(re.split(r"`+", t))).lower()
    return "".join("-" if c == " " else c for c in t if c.isalnum() or c in "-_ ")


_anchors: dict[Path, set[str]] = {}


def anchors(page: Path) -> set[str]:
    if page not in _anchors:
        text = read(page)
        seen: dict[str, int] = {}
        found = set(re.findall(r"""(?:\bid|<a\s+name)\s*=\s*["']([^"']+)["']""", text))
        for _, line in prose_lines(text):
            m = re.match(r"^#{1,6}\s+(.*?)\s*#*\s*$", line)
            if m:
                s = slug(m.group(1))
                n = seen.get(s, 0)
                seen[s] = n + 1
                found.add(s if n == 0 else f"{s}-{n}")
        _anchors[page] = found
    return _anchors[page]


LINK = re.compile(r"\]\(\s*<?([^)\s>]+)")
HTML = re.compile(r"""\b(?:href|src|srcset)\s*=\s*["']([^"']+)["']""")
REFDEF = re.compile(r"^\s{0,3}\[[^\]]+\]:\s*<?(\S+?)>?(?:\s|$)")
EXTERNAL = re.compile(r"^[a-zA-Z][a-zA-Z0-9+.-]*:|^//")


def links(page: Path):
    """(line number, target) of every link, picture and HTML reference outside code."""
    for n, line in prose_lines(read(page)):
        line = re.sub(r"(`+).*?\1", "", line)  # inline code
        for m in (*LINK.finditer(line), *HTML.finditer(line)):
            yield n, m.group(1)
        m = REFDEF.match(line)
        if m:
            yield n, m.group(1)


def check_target(where: str, base: Path, target: str, page: Path) -> None:
    path, _, anchor = target.partition("#")
    dest = (base / urllib.parse.unquote(path)).resolve() if path else page
    if not dest.exists():
        error(where, f"{target}: no such file")
    elif anchor and dest.suffix == ".md" and urllib.parse.unquote(anchor) not in anchors(dest):
        error(where, f"{target}: no heading or id #{anchor} in {rel(dest)}")


def check_links() -> int:
    count = 0
    for page in linked():
        for n, target in links(page):
            if EXTERNAL.match(target):
                continue  # https:, mailto: …
            count += 1
            check_target(f"{rel(page)}:{n}", page.parent, target, page)
    return count


ROW = re.compile(r"^\|\s*\*\*(\d+\.\d+\.\d+)\*\*\s*\|")


def changelog_rows(page: Path) -> list[tuple[str, str]]:
    """(version, row with every relative link made relative to the repository) of a table."""
    rows = []
    for line in read(page).splitlines():
        m = ROW.match(line)
        if m:
            def to_root(l: re.Match) -> str:
                t = l.group(1)
                if EXTERNAL.match(t):
                    return l.group(0)
                path, hash_, anchor = t.partition("#")
                return f"]({os.path.relpath(page.parent / path if path else page, ROOT).replace(os.sep, '/')}{hash_}{anchor}"
            rows.append((m.group(1), LINK.sub(to_root, line.strip())))
    return rows


def reading_order() -> list[Path]:
    """docs/README.md, then every page its headings and tables link, in order, then the FAQ and
    the changelog."""
    index = ROOT / "docs/README.md"
    order = [index]
    for _, line in prose_lines(read(index)):
        m = re.match(r"^(?:##\s*|\|\s*)\[[^\]]+\]\(([^)#]+\.md)\)", line)
        if m:
            page = (index.parent / m.group(1)).resolve()
            if page not in order and page.is_relative_to(index.parent):
                order.append(page)
    return order + [p for p in (ROOT / "docs/faq.md", ROOT / "docs/changelog.md") if p not in order and p.exists()]


def check_order() -> None:
    order = reading_order()
    for i, page in enumerate(order):
        if not page.exists():
            continue  # check_links names it
        lines = read(page).splitlines()
        where = rel(page)
        if not lines or "README.md" not in lines[0]:
            error(f"{where}:1", "the first line links back to the README")
        want = {
            "Previous": order[i - 1] if i else ROOT / "README.md",
            "Next": order[i + 1] if i + 1 < len(order) else order[0],
        }
        toc = sum(1 for l in lines if re.match(r"^\s*- \[[^\]]+\]\(#[^)]+\)", l))
        if len(lines) > 150 and toc < 3 and page not in (order[0], ROOT / "docs/changelog.md"):
            error(where, f"{len(lines)} lines and no table of contents: add ## Contents with its headings")
        for word, target in want.items():
            found = [l for l in lines[-5:] if f"{word}:" in l]
            m = re.search(rf"\[[^\]]*{word}:[^\]]*\]\(([^)#]+)", found[-1]) if found else None
            if not m:
                error(where, f"no *{word}* link at the end (it is {rel(target)})")
            elif (page.parent / m.group(1)).resolve() != target:
                error(where, f"*{word}* goes to {m.group(1)}, but docs/README.md's order says {rel(target)}")


def version(cargo: str) -> str:
    return tomllib.loads(cargo)["workspace"]["package"]["version"]


def check_changelog(v: str) -> None:
    full = changelog_rows(ROOT / "docs/changelog.md")
    readme = changelog_rows(ROOT / "README.md")
    # Before the first release no version has a row (packaging/release-pr.py makes the first).
    if full and v not in [r[0] for r in full]:
        error("docs/changelog.md", f"no row for {v}, the version in Cargo.toml")
    want = full[:10]
    if readme != want:
        i, a, b = next((i, a, b) for i, (a, b) in enumerate(itertools.zip_longest(readme, want)) if a != b)
        error(
            "README.md",
            f"changelog row {i + 1} must be docs/changelog.md's "
            f"({b[0] if b else 'none, only ten'}), is {a[0] if a else 'missing'}"
            + (" with other text" if a and b and a[0] == b[0] else ""),
        )


FEATURE = "<!-- feature -->"
TOUR = re.compile(r"<!--\s*tour:\s*[\w.-]+\s*-->")
MARKERS = re.compile(r"(\s*<!--.*?-->)+\s*$")


def check_fragments(v: str, base: str | None) -> int:
    """changelog.d/: one file per pull request, one row each; their count."""
    frags = sorted((ROOT / "changelog.d").glob("*.md"))
    for f in frags:
        body = [l.strip() for l in read(f).splitlines() if l.strip()]
        row = MARKERS.sub("", body[0]) if len(body) == 1 else ""
        cells = re.split(r"(?<!\\)\|", row)[1:-1] if row.endswith("|") else []
        if len(cells) != 2 or not LINK.search(cells[1]):
            error(rel(f), "one line: `| **What a user gets.** In plain words. | [Page](area/page.md#anchor) |`, "
                          "then `<!-- feature --> <!-- tour: card-id -->` for a feature")
            continue
        if FEATURE in body[0] and not TOUR.search(body[0]):
            error(rel(f), "a feature row names its tour card: `<!-- feature --> <!-- tour: card-id -->`")
        for target in (m.group(1) for m in LINK.finditer(row)):
            if not EXTERNAL.match(target):
                check_target(rel(f), ROOT / "docs", target, ROOT / "docs/README.md")
    was = base_version(base)
    if frags and was is not None and tuple(map(int, v.split("."))) > tuple(map(int, was.split("."))):
        error("changelog.d", f"{was} → {v} is a release: move the changelog.d/ rows into {v}'s row (packaging/release-pr.py)")
    return len(frags)


OWN_VERSION = re.compile(r"\b[Bb]ragd[\w-]*[ /_-]v?(\d+)\.(\d+)\.\d+(?![\d.])")
HISTORY = {"docs/changelog.md"}


def check_stale_versions(v: str) -> None:
    now = minor(v)
    for page in pages():
        if rel(page) in HISTORY:
            continue
        for n, line in enumerate(read(page).splitlines(), 1):
            for m in OWN_VERSION.finditer(line):
                history = re.search(r"\b(until|before|since|after|from)\W*$", line[: m.start()], re.I) or re.match(
                    r"\s+(and|or) (later|earlier)", line[m.end() :]
                )  # "until Bragd 1.2.0", "1.2.0 and later": history, not an example
                if (int(m.group(1)), int(m.group(2))) < now and not history:
                    error(f"{rel(page)}:{n}", f"{m.group(0)}: an old version as an example; say X.Y.Z")


def check_questions() -> None:
    for page in pages():
        level = 0  # the level of the Questions heading the lines are under, 0 when none
        for n, line in prose_lines(read(page)):
            where = f"{rel(page)}:{n}"
            if re.match(r"^\*\*[^*].*?\?\*\*", line):
                error(where, "a question as a bold paragraph: make it a #### heading, its answer below")
            m = re.match(r"^(#{1,6})\s+(.*?)\s*$", line)
            if not m:
                continue
            depth, text = len(m.group(1)), m.group(2)
            if text.endswith("?"):
                if level and depth != 4:
                    error(where, f"a question under Questions is a #### heading, not {m.group(1)}")
            elif text.startswith("Questions"):
                level = depth
            elif depth <= level:
                level = 0


@functools.cache
def base_version(base: str | None) -> str | None:
    if not base:
        ref = os.environ.get("GITHUB_BASE_REF")
        base = f"origin/{ref}" if ref else "origin/master"
    try:
        cargo = subprocess.run(
            ["git", "show", f"{base}:Cargo.toml"], cwd=ROOT, capture_output=True, text=True, check=True
        ).stdout
        return version(cargo)
    except (subprocess.CalledProcessError, FileNotFoundError, KeyError, tomllib.TOMLDecodeError):
        warning("Cargo.toml", f"cannot read the version in Cargo.toml at {base}: placeholders are only warned about")
        return None


# Pictures the sandbox cannot take, each with what it needs (a name: why). Any other placeholder
# blocks a minor or major release.
NEEDS_HARDWARE: dict[str, str] = {}


def check_placeholders(v: str, base: str | None) -> int:
    was = base_version(base)
    raised = was is not None and minor(v) > minor(was)
    count = 0
    for page in pages():
        for n, line in enumerate(read(page).splitlines(), 1):
            if "<!-- screenshot:" in line:
                count += 1
                msg = f"screenshot placeholder: {line.strip()}"
                name = line.split("<!-- screenshot:", 1)[1].split(":", 1)[0].strip()
                if name in NEEDS_HARDWARE:
                    warning(f"{rel(page)}:{n}", f"{msg} (needs {NEEDS_HARDWARE[name]})")
                elif raised:
                    error(f"{rel(page)}:{n}", f"{msg} ({was} → {v} is a minor or major release: take it)")
                else:
                    warning(f"{rel(page)}:{n}", msg)
    return count


RUST = ROOT / "crates"


def rust(path: str) -> str:
    return read(RUST / path)


def struct_fields(src: str, name: str) -> list[tuple[str, str]]:
    """(field, type) of `pub struct name { .. }`, without the `#[serde(skip)]` ones."""
    m = re.search(rf"pub struct {name} \{{(.*?)\n\}}", src, re.S)
    if not m:
        return []
    fields, skip = [], False
    for line in m.group(1).splitlines():
        line = line.strip()
        if line.startswith("#[serde(skip"):
            skip = True
        f = re.match(r"pub (\w+): ([\w<>, ]+),", line)
        if f:
            if not skip:
                fields.append((f.group(1), f.group(2)))
            skip = False
    return fields


def code_names() -> dict[str, list[str]]:
    """The names check_coverage holds the docs to, by kind, read from the Rust source."""
    config = rust("bragd-core/src/config.rs")
    keys = []
    for field, ty in struct_fields(config, "Config"):
        sub = struct_fields(config, ty) if ty.endswith("Config") else []
        keys += [f"{field}.{f}" for f, _ in sub] or [field]
    settings = rust("bragd-core/src/settings.rs")
    catalogue = rust("bragd-games/src/catalogue.rs")
    families = re.findall(r'Family::\w+ => "([a-z]+)",', catalogue)
    variants = re.findall(r'row\(Variant::\w+, "([\w-]+)"', catalogue)
    names = re.findall(r'opt\(\s*"(\w+)"', settings)
    names += [f"family_{f}" for f in families] + [f"variant_{v}" for v in variants]
    themes = rust("bragd-core/src/themes.rs")
    looks = re.search(r"pub const LOOKS: &\[&str\] = &\[(.*?)\];", themes, re.S).group(1)
    theme_names = re.findall(r'^\s*\("([\w-]+)", "theme\.group', themes, re.M)
    theme_names += re.findall(r'"(\w+)"', looks)
    theme_names += re.findall(r'"(\w+)"', re.search(r"pub const BOARD_THEMES: \[&str; \d+\] = \[(.*?)\];", themes).group(1))
    features = re.findall(r'^\s*f\("(\w+)", "([^"]+)"\)', rust("bragd-core/src/features.rs"), re.M)
    flags = re.findall(r'"(--?[\w-]+)"', rust("bragd-helper/src/main.rs") + rust("bragd-helper/src/service.rs").split("fn ")[0])
    flags += re.findall(r'Some\("(--?[\w-]+)"(?: \| "(--?[\w-]+)")?\)', read(ROOT / "gui/src-tauri/src/main.rs"))[0]
    # The environment variables the shipped programs read (the readers' test switches are not).
    shipped = [*(ROOT / "gui/src-tauri/src").glob("*.rs"), *(RUST / "bragd-core/src").glob("*.rs"),
               *(RUST / "bragd-engines/src").rglob("*.rs"), *(RUST / "bragd-helper/src").glob("*.rs")]
    env = sorted({v for p in shipped for v in re.findall(r'var(?:_os)?\("(BRAGD_\w+)"', read(p))})
    return {
        "config.toml key": keys,
        "setting": names,
        "Theme, Look or Board theme": theme_names,
        "F1 feature": [f for f, _ in features],
        "flag": [f for f in flags if f] + env,
        "_feature pages": [p for _, p in features],
    }


def check_coverage() -> int:
    """Every name in code_names() is in backticks on a page under docs/, and every F1 feature's
    page exists. Returns how many names were checked."""
    spans: set[str] = set()
    for page in sorted((ROOT / "docs").rglob("*.md")):
        for _, line in prose_lines(read(page)):
            spans.update(re.findall(r"`([^`]+)`", line))
    names = code_names()
    count = 0
    for kind, found in names.items():
        if kind.startswith("_"):
            continue
        if not found:
            error("tools/docs-check.py", f"no {kind} found in the Rust source: the pattern no longer matches the code")
        for name in dict.fromkeys(found):
            count += 1
            if name not in spans:
                error("docs", f"the {kind} `{name}` is on no page: name it in backticks where it is explained")
    for page in names["_feature pages"]:
        if not (ROOT / "docs" / page).exists():
            error("crates/bragd-core/src/features.rs", f"F1's page docs/{page} does not exist")
    return count


def self_check() -> None:
    """GitHub's anchors, as the links rely on them, the fragment format, and the coverage's reader."""
    assert struct_fields("pub struct AConfig {\n    /// x\n    pub a: bool,\n    #[serde(skip)]\n    pub b: u8,\n    pub c: Vec<String>,\n}", "AConfig") == [("a", "bool"), ("c", "Vec<String>")]
    assert slug("How fast is it here, and would another model be faster?") == "how-fast-is-it-here-and-would-another-model-be-faster"
    assert slug("`[themes.<name>]`") == "themesname"
    assert slug("Settings and config.toml") == "settings-and-configtoml"
    assert slug("Engine's [setup](x.md) <b>here</b>") == "engines-setup-here"
    assert MARKERS.sub("", "| a | [b](c.md) | <!-- feature --> <!-- tour: x -->") == "| a | [b](c.md) |"
    assert OWN_VERSION.search("Bragd_1.2.0_amd64.deb") and OWN_VERSION.search("bragd/0.3.1")
    assert not OWN_VERSION.search("Bragd X.Y.Z")


def main() -> int:
    self_check()
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--base", help="the branch the version is compared with (default origin/master)")
    args = ap.parse_args()
    v = version(read(ROOT / "Cargo.toml"))
    n = check_links()
    check_changelog(v)
    u = check_fragments(v, args.base)
    check_stale_versions(v)
    check_order()
    check_questions()
    c = check_coverage()
    p = check_placeholders(v, args.base)
    for e in errors:
        print(e, file=sys.stderr)
    print(f"docs check: {len(pages())} pages, {n} links, {c} names, {p} placeholders, version {v}, {u} unreleased: "
          + (f"{len(errors)} problems" if errors else "all good"))
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
