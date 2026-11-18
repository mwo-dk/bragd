# Coxswain sync

[← README](../README.md)

Bragd started from a copy of coxswain's board-game, engine, container, settings, translation and update code ([ADR 0001](adr/0001-copy-game-code-from-coxswain.md)). The game code now moves into shared crates in Bragd, step by step, which coxswain then uses ([ADR 0008](adr/0008-one-game-file-crate-shared-from-bragd.md)). This page records the last coxswain commit we synced from. The weekly *Coxswain sync* workflow opens one issue listing what changed upstream since then, on the paths we copied, and the coxswain issues touching them.

Each item is ported in a PR or closed as not wanted, with a reason. Once every item is handled, the commit below moves forward.

Last synced: `9e328c8` (coxswain 2.34.2 plus #713, 2026-10-30).

## Paths we copied

| coxswain | Bragd |
|---|---|
| `crates/coxswain-core/src/chess/{pgn,lex,openings,polyglot,scid,cbh,cbv}.rs`, `openings.tsv` | `crates/bragd-gamefiles/src/chess/` |
| `crates/coxswain-core/src/chess/games.rs` (the pass over a file) | `crates/bragd-gamefiles/src/chess/scan.rs` |
| `crates/coxswain-core/src/chess/games.rs` (the table), `mod.rs` (file names) | `crates/bragd-library/src/chess/` |
| `crates/coxswain-core/src/{go,shogi,xiangqi,draughts}.rs` | `crates/bragd-gamefiles/src/` |
| `crates/coxswain-core/src/store.rs` (positions) | `crates/bragd-library/src/library.rs` |
| `crates/coxswain-core/src/chess/play.rs`, `review.rs` | `crates/bragd-engines/src/` |
| `crates/coxswain-core/src/{images,tools,preview_tools}.rs` | `crates/bragd-engines/src/` |
| `crates/coxswain-core/src/{config,state,settings,i18n,update,notices,features,guide,helper}.rs` | `crates/bragd-core/src/`, `crates/bragd-helper/src/` |
| `crates/coxswain-core/locales/` | `crates/bragd-core/locales/` |
| `gui/src/{looks.css,Settings.svelte,Guide.svelte,i18n.svelte.js}` | `gui/src/` |
| `tools/`, `packaging/`, `.github/workflows/`, `docs/screenshots/` | same |

## Game file readers

The readers in `crates/bragd-gamefiles` were compared with coxswain's `master` at `c6cadae7` (2026-11-18) when they became a crate ([#387](https://github.com/mwo-dk/bragd/issues/387)). From then on they change only in Bragd.

| coxswain change | In Bragd |
|---|---|
| #880: opening books and Go files inside an archive read in memory | Ported as readers that take any source: `polyglot::Book::new` and `go::Collection::open` read a file or bytes in memory alike, and `Book::sane` tells a book by its bytes. Bragd reads no files inside an archive yet, so nothing calls them with bytes. |
| #795: `[%evp …]` left out of a comment | Bragd already reads it as the game's evaluation profile (`pgn::take_evp`), which is more than leaving it out. |
| #877: SCID 5 as SCID 5.2 writes it | Ported before, in [#375](https://github.com/mwo-dk/bragd/issues/375). |
| #824, #847, #870, #888, #911, #914: the games table on the disk | Not readers: they wait for the games table's move ([#388](https://github.com/mwo-dk/bragd/issues/388)). |
| #893, #929, #939, #953, #1018, #921: the position index and the opening tree | [#388](https://github.com/mwo-dk/bragd/issues/388). |
| #825, #1016: positions analysed, kept in the cache (`memory.rs`) | Play and review: [#389](https://github.com/mwo-dk/bragd/issues/389). |

## Upstream issues carried as Bragd issues

Coxswain 2.36.0: #702, #701, #700, #685, #684, #683, #677, #676, #668, #660, #630, #624, #615, #590.
