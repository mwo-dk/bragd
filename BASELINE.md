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

Nothing recorded yet.
