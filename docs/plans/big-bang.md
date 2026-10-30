# Big-bang plan

This is the first big PR. Everything comes on branch `big-bang`, in one commit per slice, and every commit builds and passes its tests locally. A slice that cannot go green gets a line under *Known broken* in `BASELINE.md`, and the work moves on. The PR stays a draft: it is never merged or released from this run.

Out of scope: Online play, built-in Bots, the big Sources (the Lichess dump, LumbrasGigaBase), real translations of new texts, and publishing to channels.

## Crates

| Crate | Holds |
|---|---|
| `crates/rules-<family>` | Pure rules, no dependencies (from ../games): chess, capablanca, xiangqi, shogi, go, draughts, tafl, backgammon, ur, senet |
| `crates/bragd-games` | The Family and Variant catalogue, and one native seam over every rules crate: start, replay, legal moves, the board to draw, the result |
| `crates/bragd-core` | Config, state, setting descriptors, i18n and locales, Themes and Board themes, update check, features, notices, tour cards |
| `crates/bragd-library` | Readers for Foreign databases, SQLite Libraries, Duplicates, Slices, Parquet export |
| `crates/bragd-engines` | UCI/UCCI/USI/GTP/Hub sessions, discovery, sealed container images and housekeeping, Syzygy |
| `crates/bragd-helper` | The Helper process: its connection, lifetime, Sources and schedule |
| `gui/` | Svelte UI and `gui/src-tauri` (`bragd-gui`), the Tauri commands |
| `assets/` | Pieces, sprites, brand art |

## Slices

- [x] S0: the constitution (`CLAUDE.md`), spec, glossary, ADRs, baseline, credits, sync page, workspace
- [ ] S1: rules crates and `bragd-games` (wave 1, agent "rules")
- [ ] S2: artwork, sprites, CJK glyphs as paths, brand: icon drafts, splash, rune wordmark (wave 1, agent "art")
- [ ] S3: `bragd-core`: config, state, settings, i18n with every locale, Themes, update check, features, notices (wave 1, agent "core")
- [ ] S4: `bragd-library`: formats, Library, dedupe, Slices, Parquet (wave 1, agent "library")
- [ ] S5: `bragd-engines`: protocols, discovery, images, Syzygy (wave 1, agent "engines")
- [ ] S6: CI, release tooling, docs check, sync workflow, screenshot tooling (wave 1, agent "ci")
- [ ] S7: GUI shell: branding, splash, sidebar, Themes, Settings with Family toggles, boards for all 25 Variants, play against yourself (wave 2)
- [ ] S8: Helper process, Sources (TWIC, custom URL), schedule (wave 2)
- [ ] S9: GUI for Databases, Analyse, Engines, Tablebases; install wizard; What's new; F1 Features (wave 3)
- [ ] S10: docs tree, screenshots, morning report in the PR body (wave 3)

## Decisions made alone during the run

Each one is listed here, so the owner can overturn it in the morning.
