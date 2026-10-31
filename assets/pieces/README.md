# Pieces

All piece art in Bragd comes from this one set. The board draws pieces from the sprite sheets in `gui/src/lib/sprites/`, one sheet per Family (and one each for the two Chess Variants with pieces of their own). Licences and provenance: [`LICENCE.md`](LICENCE.md) and [`docs/CREDITS.md`](../../docs/CREDITS.md).

## What is here

| Folder | What it holds | Made by |
|---|---|---|
| `upstream/` | Cburnett's twelve chess pieces, byte for byte as published. Never edited. | Downloaded |
| `basic/` | The same twelve, colours normalised | `tools/pieces/normalise-upstream-pieces.mjs` |
| `shatranj/` | Ferz, alfil and gaja, drawn on Cburnett parts | By hand |
| `capablanca/` | Archbishop and chancellor, Cburnett knight grafted on bishop and rook | By hand (see below) |
| `seirawan/` | Hawk | By hand |
| `shogi/` | Wedge tiles with Noto Sans CJK kanji outlines | `tools/pieces/build-shogi-pieces.py` |
| `xiangqi/`, `janggi/` | Sources with the characters as `<text>`; never shipped | By hand |
| `contact-sheets/` | Every symbol of every sheet on Racing green squares, for looking at | `tools/pieces/contact-sheets.py` |

## The sheets

| Sheet | Built by | Notice inside |
|---|---|---|
| `chess-pieces.svg` | `node tools/pieces/build-pieces.mjs` | Cburnett, BSD-3 |
| `capablanca-pieces.svg` | `node tools/pieces/build-pieces.mjs` | Cburnett, BSD-3 |
| `seirawan-pieces.svg` | `node tools/pieces/build-pieces.mjs` | Cburnett, BSD-3 |
| `shogi-pieces.svg` | `node tools/pieces/build-pieces.mjs` | Noto Sans CJK, OFL 1.1 |
| `xiangqi-pieces.svg` | `python3 tools/pieces/cjk-outlines.py TC.otf KR.otf` | Noto Sans CJK TC, OFL 1.1 |
| `janggi-pieces.svg` | `python3 tools/pieces/cjk-outlines.py TC.otf KR.otf` | Noto Sans CJK KR, OFL 1.1 |
| `draughts-pieces.svg`, `tafl-pieces.svg`, `go-pieces.svg` | By hand | None needed: ours |
| `backgammon-pieces.svg`, `ur-pieces.svg`, `senet-pieces.svg` | By hand | None needed: ours |

No sheet holds a `<text>` element. Every character is an outline, so no piece depends on an installed font.

`tools/pieces/compose-capablanca-pieces.mjs` draws an earlier design of the archbishop and chancellor. The committed files were revised by hand, so do not run it until it is updated.

## Symbol ids

Every sprite is inlined into one document, where a duplicate id silently resolves to whichever came first. So every id starts with its sheet's name and is unique across all sheets.

A **piece** is `<sheet>-<side><ROLE>`: one hyphen, a lower-case side letter, then the role in upper case.

A **helper** (a shared disc, tile or gradient) is `<sheet>-<words>`, all lower case with at least two hyphens, so it can never look like a piece.

| Sheet | Sides | Roles | Example |
|---|---|---|---|
| `chess` | `w` white, `b` black | `K Q R B N P`; `F` ferz, `E` alfil, `G` gaja | `chess-wK` |
| `capablanca` | `w`, `b` | chess roles; `A` archbishop, `C` chancellor | `capablanca-bA` |
| `seirawan` | `w`, `b` | chess roles; `H` hawk, `S` elephant | `seirawan-wH` |
| `shogi` | `w` (pointing down), `b` (pointing up) | `K R B G S N L P`; promoted `TP` tokin (+P), `TL` (+L), `TN` (+N), `TS` (+S), `H` horse (+B), `D` dragon (+R) | `shogi-bTP` |
| `xiangqi` | `r` red, `b` black | `K A B N R C P` | `xiangqi-rR` |
| `janggi` | `r` Cho (**green**, moves first), `b` Han (red) | `K A B N R C P` | `janggi-rK` |
| `draughts` | `w`, `b` | `M` man, `K` king | `draughts-wM` |
| `tafl` | `a` attacker, `d` defender | `S` soldier, `K` king | `tafl-dK` |
| `go` | `b`, `w` | none: a stone has no role | `go-b` |
| `backgammon` | `w`, `b` | none: a checker | `backgammon-w` |
| `ur` | `l` light, `d` dark | none | `ur-l` |
| `senet` | `e` Evens (cones), `o` Odds (spools) | none | `senet-o` |

Helpers: `xiangqi-disc-red`, `xiangqi-disc-black`, `janggi-tile-{general,major,minor}-{cho,han}`, `go-stone-black`, `go-stone-white`.

Shogi's `T` stands for *turned*: a promoted piece is the same tile turned over. An id cannot hold the `+` of SFEN.

## Checking a change

1. Rebuild the sheet with its tool.
2. Run `python3 tools/pieces/contact-sheets.py`.
3. Look at the PNG in `contact-sheets/`. A piece drawn wrong renders without an error.
