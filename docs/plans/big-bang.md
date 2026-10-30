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
- [x] S1: rules crates and `bragd-games` (wave 1, agent "rules")
- [x] S2: artwork, sprites, CJK glyphs as paths, brand: icon drafts, splash, rune wordmark (wave 1, agent "art")
- [x] S3: `bragd-core`: config, state, settings, i18n with every locale, Themes, update check, features, notices (wave 1, agent "core")
- [x] S4: `bragd-library`: formats, Library, dedupe, Slices, Parquet (wave 1, agent "library")
- [x] S5: `bragd-engines`: protocols, discovery, images, Syzygy (wave 1, agent "engines")
- [x] S6: CI, release tooling, docs check, sync workflow, screenshot tooling (wave 1, agent "ci")
- [ ] S7: GUI shell: branding, splash, sidebar, Themes, Settings with Family toggles, boards for all 25 Variants, play against yourself (wave 2)
- [ ] S8: Helper process, Sources (TWIC, custom URL), schedule (wave 2)
- [ ] S9: GUI for Databases, Analyse, Engines, Tablebases; install wizard; What's new; F1 Features (wave 3)
- [ ] S10: docs tree, screenshots, morning report in the PR body (wave 3)

## Decisions made alone during the run

Each one is listed here, so the owner can overturn it in the morning.
- Commits and the PR body carry no Claude attribution, following the commit rule in `CLAUDE.md`.
- Sprite ids are `<sheet>-<side><ROLE>`. Capablanca and seirawan have their own sheets. Janggi `r` is Cho, painted green (`assets/pieces/README.md`).
- Icon draft 1 (plain roundel) is the provisional app icon until the owner picks one of the three in `assets/brand/`.
- `tools/pieces/compose-capablanca-pieces.mjs` draws an older design than the committed pieces. Running it overwrites them. It needs an update or deletion.
- There are 30 Themes, not 31. The source has 33, and removing Cyber and the four DOS themes leaves 28. Racing green light and Racing green dark bring it to 30.
- "Match theme" board colours come from the Theme. The light square is the lighter of page and text; the dark square is the first clearly darker of accent, selection, border and dim.
- `[families.family]` and `[families.variant]` are separate tables, because "chess" names both a Family and a Variant.
- `bragd-core/src/families.rs` duplicates the catalogue in `bragd-games`. Once S1 lands, one is deleted.
- English fills in the untranslated keys in every locale. The list is in `crates/bragd-core/locales/UNTRANSLATED.md`.
- The upstream per-file game table is called `chess::Table`, because Library is the glossary's word for the SQLite store.
- The Duplicate key hashes Family, Variant, start position and every ply's key, using deterministic SplitMix64 and FNV. Chess positions use the Polyglot Zobrist key. Draughts, xiangqi and shogi use a hash of the FEN/SFEN. Go only has a move-sequence key for now, so position search does not work for go.
- The Source priority is held in memory, not stored in the Library, and changing it affects only later merges.
- The `[%evp]` layout (start ply, then centipawns from White's side) was inferred from a single sample (upstream #615). It needs checking against a real ChessBase export.
- The reader texts that used translations are plain English constants for now, until the library crate takes texts from `bragd-core`.
- The Variant ids in `bragd-library` must be aligned with `bragd-games` when S1 lands.

### S5: engines

- `bragd-engines` depends on no other Bragd crate, as S1 and S3 were built at the same time. Variants are string keys (`chess`, `kingofthehill`, `go19`, `international`), to be tied to `bragd-games`' catalogue later.
- Its texts are plain English until the commands that show them go through `t!`.
- Moves go to and come from engines in the engine's own notation (`e2e4`, `7g7f`, `D4`, `32-28`). Turning them into a Variant's notation is the rules crates' work.
- Pikafish is started as UCI, which it speaks. UCCI is there for other xiangqi engines; an engine chosen by its path is tried as UCI, then as UCCI.
- Fairy-Stockfish speaks USI for shogi, so shogi moves are always in USI notation, and UCI for everything else.
- GNU Backgammon runs in its own command-line mode over stdin, not in its socket mode (`external`), because a sealed image has no network.
- Scan plays international draughts only. English, Brazilian and Russian draughts have no engine for now.
- An image runs only when pinned by digest. Every pin is `None` (unpublished) until the images are pushed, so for now an engine runs only when installed.
- An image's entrypoint already speaks its protocol (KataGo with its network and config, GNU Go with `--mode gtp`). Arguments such as `gtp` go only to installed programs.
- Engine containers also get `--pull=never`, `--user 65534:65534`, a 256 MB `/tmp` and `LC_ALL=C`. The tablebase folder is mounted read-only at `/syzygy`.
- Engine options are kept per engine in their own TOML file (`OptionStore`), not in `config.toml`. The caller chooses where the file goes.
- A pull tries again after 5 s, 30 s and 2 min on a passing network or registry fault. After that it says "no network" or "the registry is busy". `next_try` gives the app 5 min, then 15 min, then hourly. An unknown fault counts as lasting.
- The Syzygy plan reads the mirror's `bytes.tsv` (one request) for exact sizes. Downloads check each file's size but not its BLAKE3 sum yet.
- Reviews mark moves only where engines score in centipawns (chess, xiangqi, shogi, draughts). Go and backgammon reviews show evaluations without marks.
- The KataGo image uses the g170e b15c192 network (48 MB), which is light enough for the CPU.

### S1: rules

- There are nine Families, ten rules crates and 25 Variants. Capablanca and seirawan are chess Variants.
- Rules crates are at `crates/rules-<family>`, not `crates/rules/*`.
- The seam keeps no state: every call replays the whole record. Dice, throws and the doubling cube are entries in the record.
- Shogi's Sides are Sente and Gote. White moves first in all four draughts rulesets, following the rules crate; English checkers traditionally starts with Black.
- Go uses Chinese area scoring with komi 7.5. After two passes, the scoring entries are `dead:`, `accept` and `resume`.
- Backgammon's legal moves are whole turns.
- The sprite ids from `bragd-games` and from the art slice must be reconciled in S7. Examples: janggi is `xiangqi-janggi-*` in one and a `janggi-*` sheet in the other, and shogi promoted pieces are `spP` in one and `TP` in the other.

### S6: CI

- The first release is 0.2.0, or 0.1.1 if there is no feature row. Exit 3 does not apply before the first release.
- Tour-card marker: a fragment ends with `<!-- feature --> <!-- tour: card-id -->`. `release-pr.py` exits 5 when the card id is not in `tour.rs`.
- The Flatpak bundle is a release file. It is not on Flathub.
- WiX en-GB is forced in the release config.
- Nix is Linux only.
- The BSD desktop tarballs are not adapted yet, and neither is the nixpkgs package. Both become issues.
- `gui/src-tauri` must join the workspace in S7.
