# Coxswain sync

[← README](../README.md)

Bragd started from a copy of coxswain's board-game, engine, container, settings, translation and update code ([ADR 0001](adr/0001-copy-game-code-from-coxswain.md)). This page records the last coxswain commit we synced from. The weekly *Coxswain sync* workflow opens one issue listing what changed upstream since then, on the paths we copied, and the coxswain issues touching them.

Each item is ported in a PR or closed as not wanted, with a reason. Once every item is handled, the commit below moves forward.

Last synced: `9e328c8` (coxswain 2.34.2 plus #713, 2026-10-30).

## Paths we copied

| coxswain | Bragd |
|---|---|
| `crates/coxswain-core/src/chess/` | `crates/bragd-library/src/chess/` |
| `crates/coxswain-core/src/{go,shogi,xiangqi,draughts}.rs` | `crates/bragd-library/src/` |
| `crates/coxswain-core/src/store.rs` (positions) | `crates/bragd-library/src/library.rs` |
| `crates/coxswain-core/src/chess/play.rs`, `review.rs` | `crates/bragd-engines/src/` |
| `crates/coxswain-core/src/{images,tools,preview_tools}.rs` | `crates/bragd-engines/src/` |
| `crates/coxswain-core/src/{config,state,settings,i18n,update,notices,features,guide,helper}.rs` | `crates/bragd-core/src/`, `crates/bragd-helper/src/` |
| `crates/coxswain-core/locales/` | `crates/bragd-core/locales/` |
| `gui/src/{looks.css,Settings.svelte,Guide.svelte,i18n.svelte.js}` | `gui/src/` |
| `tools/`, `packaging/`, `.github/workflows/`, `docs/screenshots/` | same |

## Upstream issues carried as Bragd issues

Coxswain 2.36.0: #702, #701, #700, #685, #684, #683, #677, #676, #668, #660, #630, #624, #615, #590.
