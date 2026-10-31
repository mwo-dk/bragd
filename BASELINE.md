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
- The engine images are published, but Bragd does not pin them yet (every pin is `None`), so none is pulled.
- The Duplicates count stops at 500. A Library's Games list is not limited to the page's Family.
- A custom URL Source can't be removed.
- Bragd's Variant ids are not aligned with `bragd-games` (`standard`, `international`). Slices take the Library's id.
