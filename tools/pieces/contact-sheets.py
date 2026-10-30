#!/usr/bin/env python3
"""Draws every symbol of every sprite on Racing green board squares, for looking at.

    python3 tools/pieces/contact-sheets.py

Writes assets/pieces/contact-sheets/<sheet>.png with rsvg-convert. Look at them
after changing any piece: a piece drawn wrong renders without an error.
"""

import pathlib
import re
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
SPRITES = ROOT / "gui" / "src" / "lib" / "sprites"
OUT = ROOT / "assets" / "pieces" / "contact-sheets"
LIGHT, DARK = "#efe7d6", "#17553a"  # the Racing green Board theme
SQUARE, COLUMNS, LABEL = 96, 6, 18
PIECE_ID = re.compile(r'<symbol id="([a-z]+-[a-z][A-Z]*)"')  # helpers have a second hyphen


def sheet(path: pathlib.Path) -> str:
    text = path.read_text()
    inner = re.search(r"<svg\b[^>]*>(.*)</svg>", text, re.S)[1]
    ids = PIECE_ID.findall(text)
    assert ids, f"{path.name}: no piece symbols"
    rows = -(-len(ids) // COLUMNS)
    width, height = COLUMNS * SQUARE, rows * (SQUARE + LABEL)
    cells = []
    for i, piece in enumerate(ids):
        x, y = i % COLUMNS * SQUARE, i // COLUMNS * (SQUARE + LABEL)
        colour = LIGHT if (i % COLUMNS + i // COLUMNS) % 2 == 0 else DARK
        cells.append(
            f'<rect x="{x}" y="{y}" width="{SQUARE}" height="{SQUARE}" fill="{colour}"/>'
            f'<use href="#{piece}" x="{x}" y="{y}" width="{SQUARE}" height="{SQUARE}"/>'
            f'<text x="{x + SQUARE / 2}" y="{y + SQUARE + 13}" font-family="sans-serif" '
            f'font-size="12" text-anchor="middle" fill="#00301b">{piece}</text>'
        )
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}"><rect width="100%" height="100%" fill="#e9f0ea"/>'
        f"<defs>{inner}</defs>{''.join(cells)}</svg>\n"
    )


def main() -> None:
    render = shutil.which("rsvg-convert")
    if not render:
        raise SystemExit("needs rsvg-convert (librsvg)")
    OUT.mkdir(parents=True, exist_ok=True)
    for path in sorted(SPRITES.glob("*-pieces.svg")):
        name = path.name.removesuffix("-pieces.svg")
        png = OUT / f"{name}.png"
        subprocess.run([render, "-o", str(png)], input=sheet(path).encode(), check=True)
        print(png.relative_to(ROOT))


if __name__ == "__main__":
    main()
