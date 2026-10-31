# Baseline

What is settled, and what is known to be broken. Read this before starting work.

## Do not re-litigate

- Bragd and coxswain are separate apps. The code is copied, not shared ([ADR 0001](docs/adr/0001-copy-game-code-from-coxswain.md)).
- Desktop only, with no web client.
- Libraries are SQLite. Foreign databases are read in place. Parquet is used only for export ([ADR 0002](docs/adr/0002-sqlite-libraries-foreign-databases-in-place.md)).
- The Helper is a separate process shared by every window ([ADR 0003](docs/adr/0003-helper-is-a-separate-shared-process.md)).
- One artwork lineage, with Cburnett chess pieces. Glyphs are paths.
- Racing green is the default Theme and the default Board theme.
- The set of locales is fixed.
- No self-updater: the app only checks for updates and names the channel's upgrade command.

## Known broken

- Text set in the bundled Noto Sans Runic draws nothing in WebKitGTK: the font loads, but the runes have no width. The sidebar shows the rune wordmark from `assets/brand/wordmark-runes.svg` (outlines) instead. The cause is not found yet.
- Castling on 8×8 boards means moving the king onto its own rook. Dragging the king two squares does nothing.
- New game in Chess960 always sets up the standard array.
- The move list numbers half-moves (1. e4 2. e5), not full moves.
- Parquet export has no button or command in the app.
- The "turn it back on" banner for a Family that is off is not built.
- Engine image updates and the 90-day prune never run, and no image is published (every pin is `None`).
- The Duplicates count stops at 500. A Library's Games list is not limited to the page's Family.
- A custom URL Source can't be removed.
- Draughts *Which move?* names a capture only by its two ends. Seirawan gating works only through *Which move?*. Backgammon has no resign button.
- Bragd's Variant ids are not aligned with `bragd-games` (`standard`, `international`). Slices take the Library's id.
- No real engine has been run: the Hub (Scan), gnubg and KataGo parsers, the janggi pass and the gnubg Position ID player order rest on documents only.
- Tablebase downloads check size only, not BLAKE3.
