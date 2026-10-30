#!/usr/bin/env python3
"""Builds the shogi piece sprite: a wedge tile with the kanji as an outline.

Why paths rather than text
--------------------------

Shogi pieces are kanji, and Bragd has one hard rule about piece art: glyphs are
drawn as paths, never as `<text>`, so no piece depends on installed fonts.

That rules out `<text>` with a font stack, which is the tempting shortcut: a
device with no CJK font renders every piece as a tofu box, and a board of tofu
boxes is worse than no board. Outlines always draw.

So the fourteen glyphs are pulled out of Noto Sans CJK as bezier paths, once, at
build time, and land in the sprite beside the hand-drawn pieces. That is exactly
what a design tool does when you "convert text to outlines", and the OFL permits
it — the licence is about redistributing *the font*, and this redistributes a
document containing outlines. The provenance is recorded in
assets/pieces/LICENCE.md all the same, because the rule here is that a borrowed
thing says where it came from.

Why the tile
------------

Both players' pieces are the **same colour** in shogi; whose they are is shown
by which way the wedge points. That is genuinely different from every other
Family's set, where colour is ownership — so White's tile is the same artwork
rotated through 180°, and the sprite emits both.

Run: python3 tools/pieces/build-shogi-pieces.py
"""

import pathlib
import subprocess
import sys

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.ttLib import TTFont

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "assets" / "pieces" / "shogi"

# The font. Noto Sans CJK is OFL, which is what makes this reproducible on
# another machine and legally clean.
FONT = "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"
FALLBACKS = [
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Medium.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Bold.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-DemiLight.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Light.ttc",
]

# role → (kanji for Black, kanji for White, is it a promoted piece)
#
# The king is the one piece written differently for the two players — 王 for the
# stronger player and 玉 for the other — and that is not decoration, it is how
# every physical set is made.
#
# The promoted pieces use the standard abbreviations rather than the two-
# character forms: と, 杏, 圭, 全, 馬, 龍. They are what is printed on a real
# set, in red.
PIECES = [
    ("pawn", "歩", "歩", False),
    ("lance", "香", "香", False),
    ("knight", "桂", "桂", False),
    ("silver", "銀", "銀", False),
    ("gold", "金", "金", False),
    ("bishop", "角", "角", False),
    ("rook", "飛", "飛", False),
    ("king", "王", "玉", False),
    ("tokin", "と", "と", True),
    ("promoted-lance", "杏", "杏", True),
    ("promoted-knight", "圭", "圭", True),
    ("promoted-silver", "全", "全", True),
    ("horse", "馬", "馬", True),
    ("dragon", "龍", "龍", True),
]

# The wedge, pointing up. Narrow at the top, wide at the base, with the corners
# eased — a shogi piece is a flat pentagon and the shape alone identifies the
# game across a room.
TILE = "M 22.5,3.5 L 33.6,8.6 L 36.4,41 L 8.6,41 L 11.4,8.6 Z"

INK = "#241c14"
WOOD = "#f0d9a6"
PROMOTED_INK = "#a8321f"


def find_font() -> str:
    for candidate in FALLBACKS:
        if pathlib.Path(candidate).exists():
            return candidate
    sys.exit(
        "no Noto Sans CJK found. Install noto-fonts-cjk, or point FONT at an "
        "OFL-licensed CJK font."
    )


def glyph_path(font: TTFont, character: str, size: float, cx: float, cy: float) -> str:
    """One character as an SVG path, scaled and centred on (cx, cy)."""
    cmap = font.getBestCmap()
    name = cmap.get(ord(character))
    if name is None:
        sys.exit(f"{character!r} is not in the font")

    glyphs = font.getGlyphSet()
    pen = SVGPathPen(glyphs)
    glyphs[name].draw(pen)
    commands = pen.getCommands()
    if not commands:
        sys.exit(f"{character!r} produced no outline")

    units = font["head"].unitsPerEm
    scale = size / units
    # The glyph's own box, so the character is centred on what it actually
    # covers rather than on its advance width — kanji vary and a centred
    # advance leaves some of them visibly off.
    bounds = glyphs[name]._glyph.__dict__ if hasattr(glyphs[name], "_glyph") else {}
    x_min = bounds.get("xMin", 0)
    x_max = bounds.get("xMax", units)
    y_min = bounds.get("yMin", 0)
    y_max = bounds.get("yMax", units)
    width = (x_max - x_min) * scale
    height = (y_max - y_min) * scale
    # SVG's y runs down and a font's runs up, hence the negative scale.
    dx = cx - width / 2 - x_min * scale
    dy = cy + height / 2 + y_min * scale
    return f'<path transform="translate({dx:.2f} {dy:.2f}) scale({scale:.5f} {-scale:.5f})" d="{commands}"/>'


def piece_svg(font: TTFont, character: str, promoted: bool, flipped: bool) -> str:
    ink = PROMOTED_INK if promoted else INK
    body = [
        f'  <path d="{TILE}" fill="{WOOD}" stroke="{INK}" stroke-width="1.6" stroke-linejoin="round"/>',
        f'  <g fill="{ink}">',
        "    " + glyph_path(font, character, 23.0, 22.5, 25.5),
        "  </g>",
    ]
    inner = "\n".join(body)
    if flipped:
        # **Ownership is orientation, not colour.** The opponent's piece is the
        # same tile turned to face them, which is how a real set works and the
        # only game here where that is true.
        inner = f'  <g transform="rotate(180 22.5 22.5)">\n{inner}\n  </g>'
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        f"<!--\n"
        f"\t{character} — a shogi tile.\n\n"
        f"\tGenerated by tools/pieces/build-shogi-pieces.py. The kanji is an outline\n"
        f"\ttaken from Noto Sans CJK (OFL); see assets/pieces/LICENCE.md.\n"
        f"\tDo not edit by hand.\n"
        f"-->\n"
        '<svg xmlns="http://www.w3.org/2000/svg" width="45" height="45" viewBox="0 0 45 45">\n'
        f"{inner}\n"
        "</svg>\n"
    )


def main() -> None:
    path = find_font()
    # A .ttc holds several fonts; the Japanese one is first in Noto's ordering.
    font = TTFont(path, fontNumber=0)
    OUT.mkdir(parents=True, exist_ok=True)

    written = 0
    for role, black, white, promoted in PIECES:
        (OUT / f"{role}-b.svg").write_text(piece_svg(font, black, promoted, False))
        (OUT / f"{role}-w.svg").write_text(piece_svg(font, white, promoted, True))
        written += 2

    print(f"wrote {written} shogi tiles from {pathlib.Path(path).name}")


if __name__ == "__main__":
    main()
