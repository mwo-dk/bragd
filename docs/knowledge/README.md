[← README](../../README.md) · [Docs index](../README.md)

# Knowledge

Bragd keeps Games in two ways. A **Foreign database** is a game file in someone else's format, such as PGN or SGF. Bragd reads it where it is and changes nothing in it. A **Library** is Bragd's own SQLite file, where imports land and each Game appears once. **Sources** such as TWIC bring new Games on a schedule, each through its **Routes** into one Library or several. The **Helper**, a background process shared by every window, does the fetching, importing and Duplicate finding, so the windows stay quick. **Strip rules** and **Disk watch** hold what is stored to the Games you want and the space you allow.

Everything here is reached from the sidebar: a Family, then **Databases**. The page has three tabs: *Files*, *Libraries* and *Sources*. Disk watch concerns every Family, so it is in *Settings → Knowledge*.

| Page | What it covers |
|---|---|
| [Foreign databases](foreign-databases.md) | Opening PGN, SCID, ChessBase, SGF, KIF, CSA and PDN files in place, the list, replay, Save as |
| [Libraries](libraries.md) | Bragd's own store, importing, where it is kept |
| [Duplicates](duplicates.md) | How a Duplicate is found, merging, Source priority, Near-duplicates |
| [Slices](slices.md) | Named queries, live or kept as a copy, position search |
| [Player summary](players.md) | One player's results, rating, openings, best wins and worst losses in a Library |
| [Export to Parquet](parquet-export.md) | A Library as a Parquet file for DuckDB |
| [Sources](sources.md) | TWIC, custom URLs, the listed big Sources, schedules |
| [Strip rules](strip-rules.md) | Which Games a Source keeps, preview and apply, after import |
| [Disk watch](disk-watch.md) | What Bragd keeps on disk, limits, warnings and what happens when one is crossed |
| [Position store](position-store.md) | Evaluations and Annotations kept by position, Export and Import |
| [Language model](language-model.md) | Translate a Game's comments with a model on this machine or one you choose |
| [The Helper](helper.md) | The background process, its jobs, *Start with my session*, battery |

---
[← Previous: Senet](../play/senet.md) · [Next: Foreign databases →](foreign-databases.md)
