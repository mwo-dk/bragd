[← README](../../README.md) · [Docs index](../README.md) · [Knowledge](README.md)

# Export to Parquet

A Library can be written out as a Parquet file, for DuckDB or any other tool that reads Parquet to query. The export is a copy: the Library stays the store, and nothing reads the Parquet file back into Bragd.

**Not yet in the app.** The Helper can do the export (its `Export` job), but no button in this version starts it. Until one does, a Library can be queried directly with any SQLite tool, since it is an SQLite file.

![The Libraries tab, where the export will be started from](../screenshots/databases-libraries-1100.png)
*Databases → Libraries.*

## How to reach it

Not from the app yet. When it comes, it will be a job of the [Helper](helper.md), shown as *Export main.bragd* in the sidebar's Helper line and on the Jobs page, like an import.

## What the file holds

One row per Game, in the order of the Library, compressed with Snappy, 65,536 rows to a row group:

| Column | What it holds |
|---|---|
| `id` | The Game's number in the Library |
| `family`, `variant` | Its Game family and Variant |
| `white`, `black`, `white_elo`, `black_elo` | The players and their ratings (the ratings may be empty) |
| `date`, `event`, `site`, `eco`, `opening`, `result` | The headers, as merged from its Sources |
| `headers` | Every header |
| `moves` | The moves |
| `start` | The starting position, when it is not the usual one |
| `ply_count` | The number of half-moves |
| `zobrist_final`, `zobrist_chain_hash` | The key of the final position and of the whole move chain ([Duplicates](duplicates.md)) |
| `sources` | Every Source that brought it, apart by commas |

## What it needs

A Library with Games, and room on the disk for the file.

## Settings and config.toml

None. The Library comes from the Library folder, `knowledge.library`.

## Questions

#### Why Parquet and not a DuckDB file?

Parquet is read by DuckDB and many other tools, and DuckDB never owns the data that way. The Library stays the one store.

#### Does the export change the Library?

No. It only reads it.

#### Can I query a Library without exporting it?

Yes. A Library is an SQLite file. Open it read-only with `sqlite3` or DuckDB's SQLite reader. Do not write to it while Bragd runs.

---
[← Previous: Slices](slices.md) · [Next: Sources →](sources.md)
