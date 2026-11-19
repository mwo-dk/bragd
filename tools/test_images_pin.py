#!/usr/bin/env python3
"""tools/images-pin.py reads its arguments: `-h`/`--help` prints help and an unknown flag is refused,
both before any registry is asked or any file is written. Run in an empty folder, so a script that
ignores its arguments fails here instead of rewriting pins.

    python3 tools/test_images_pin.py
"""
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "images-pin.py"


def run(*args):
    with tempfile.TemporaryDirectory() as d:
        r = subprocess.run([sys.executable, str(SCRIPT), *args], cwd=d, capture_output=True, text=True, timeout=60)
        assert not any(Path(d).iterdir()), f"{args}: wrote files"
        return r


for flag in ("-h", "--help"):
    r = run(flag)
    assert r.returncode == 0, (flag, r.returncode, r.stderr)
    assert "--check" in r.stdout and "usage" in r.stdout.lower(), (flag, r.stdout)
for args in (["--chek"], ["--check", "extra"]):
    r = run(*args)
    assert r.returncode == 2, (args, r.returncode, r.stderr)
    assert "unrecognized arguments" in r.stderr, (args, r.stderr)
print("test_images_pin: ok")
