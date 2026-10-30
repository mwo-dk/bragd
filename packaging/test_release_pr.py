#!/usr/bin/env python3
"""release-pr.py in a throwaway repository: exit 2 with no rows, exit 5 for a feature row with no
tour card or one tour.rs lacks, and a minor release that folds the rows into docs/changelog.md
and the README and deletes them. --unreleased-ok and --open-issues-ok keep it off the network
(exits 3 and 4 need GitHub). Needs cargo. Run from anywhere: python3 packaging/test_release_pr.py"""
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "release-pr.py"
HEAD = "| Version | Date | What's new |\n|---|---|---|\n"
FLAGS = ["--unreleased-ok", "--open-issues-ok"]


def run(cwd: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(SCRIPT), *args, *FLAGS], cwd=cwd, capture_output=True, text=True)


def main() -> int:
    with tempfile.TemporaryDirectory() as d:
        root = Path(d)
        (root / "src").mkdir()
        (root / "src/lib.rs").write_text("")
        (root / "Cargo.toml").write_text('[package]\nname = "t"\nversion.workspace = true\nedition = "2024"\n\n'
                                         '[workspace]\n\n[workspace.package]\nversion = "0.1.0"\n')
        (root / "docs").mkdir()
        (root / "docs/changelog.md").write_text(f"# Changelog\n\n## Versions\n\n{HEAD}")
        (root / "README.md").write_text(f"# T\n\n## Changelog\n\n{HEAD}\nLicence.\n")
        (root / "changelog.d").mkdir()
        (root / "crates/bragd-core/src").mkdir(parents=True)

        assert run(root, "--next").returncode == 2

        frag = root / "changelog.d/a.md"
        frag.write_text("| **Play chess.** Moves are checked. | [Play](play/README.md#chess) | <!-- feature -->\n")
        r = run(root, "--next")
        assert r.returncode == 5 and "no <!-- tour" in r.stderr, r
        frag.write_text("| **Play chess.** Moves are checked. | [Play](play/README.md#chess) | <!-- feature --> <!-- tour: play-chess -->\n")
        r = run(root, "--next")
        assert r.returncode == 5 and '"play-chess" is not in' in r.stderr, r
        (root / "crates/bragd-core/src/tour.rs").write_text('Card { id: "play-chess", version: "0.2.0" }\n')
        (root / "changelog.d/b.md").write_text("| **A fix.** Words. | [Rules](rules/README.md) |\n")
        r = run(root, "--next")
        assert r.returncode == 0 and r.stdout.strip() == "0.2.0", r

        r = run(root)
        assert r.returncode == 0 and r.stdout.strip() == "0.2.0", r
        assert 'version = "0.2.0"' in (root / "Cargo.toml").read_text()
        log = (root / "docs/changelog.md").read_text()
        assert "| **0.2.0** |" in log and "**Play chess.** Moves are checked. [Play](play/README.md#chess) **A fix.**" in log, log
        assert "<!--" not in log, log
        readme = (root / "README.md").read_text()
        assert "[Play](docs/play/README.md#chess)" in readme, readme
        assert not list((root / "changelog.d").glob("*.md"))
    print("release-pr.py: tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
