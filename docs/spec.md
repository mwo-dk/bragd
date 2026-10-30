# Bragd: specification

[← README](../README.md)

Bragd (Old Norse *bragð*, "a clever move") is a desktop app for playing, studying and keeping turn-based board games. It is built with Tauri: Rust does all the work, and Svelte only draws. Terms are defined in [`GLOSSARY.md`](../GLOSSARY.md), and decisions are recorded in [`docs/adr/`](adr/).

## Game families and Variants

All 25 Variants of the ten Game families are playable from the start:

| Game family | Variants |
|---|---|
| Chess | chess, chess960, shatranj, chaturanga, king of the hill, racing kings, seirawan, capablanca |
| Xiangqi | xiangqi, janggi |
| Shogi | shogi |
| Go | 9×9, 13×13, 19×19 |
| Draughts | English checkers, international, Brazilian, Russian |
| Tafl | hnefatafl (Copenhagen), Fetlar, tablut, brandubh |
| Backgammon | backgammon |
| Ur | ur (Finkel) |
| Senet | senet (Kendall) |

- Every Family and Variant can be turned off in Settings and in the install wizard, and all are on by default.
- A Family or Variant that is off disappears from the sidebar, New game, the wizard and engine setup.
- Its files still open, with a banner offering to turn it back on, and none of its data is deleted.

## Navigation

- A collapsible sidebar sits on the left for left-to-right languages and on the right for Hebrew and Farsi. Settings, Help and What's new are always there.
- With two or more Families on, the Families are the outer items. With one Family on, that Family's items come straight to the top.
- Each Family has these items: **Play** (against an Engine, against yourself, or online), **Analyse**, **Databases**, **Engines** and **Rules**.
- Chess also has **Tablebases** and **Opening books**.
- A Family with no Engine shows only Play, Databases and Rules.

## Look

- **Theme:** the 31 Themes (Own, Windows, Mac, Other desktops, Home computers), plus Looks and user themes. **Racing green** is the default, built on the brand ramp `#00301b` `#004225` `#0b5c39` `#4f9d76` `#e9f0ea`.
- **Board theme:**
  - Exactly four colours: light square, dark square, highlight and marker.
  - Racing green (`#efe7d6` / `#17553a` / `#d8b74a`) is the default. Slate and Walnut are also offered.
  - *Match theme* takes the colours from the Theme.
  - A Board theme never inverts with a dark Theme.
- **Artwork:** one lineage. Chess uses Cburnett (BSD-3) plus our fairy pieces (ferz, alfil, archbishop, chancellor, hawk). Shogi, xiangqi and janggi glyphs are outlines from Noto Sans CJK (OFL). Go, draughts and tafl have their own pieces.
- **Brand:**
  - The name is written in Younger Futhark: **ᛒᚱᛅᚴᚦ**.
  - The window title is `Bragd ᛒᚱᛅᚴᚦ`.
  - The icon is a Racing green roundel with a gold **ᛒ**.
  - The splash shows the rune wordmark, *Bragd* and *bragð — a clever move*, for at most about one second, and respects reduced motion.
  - Noto Sans Runic (OFL) is bundled.

## Languages

The app ships in a fixed set of locales, kept in `crates/bragd-core/locales/`. Every user-facing string exists in each one.

## Knowledge

- A **Foreign database** is read in place. Formats:
  - PGN, including `.pgn.gz` and `.pgn.zst`
  - EPD/FEN
  - SCID 4/5
  - ChessBase `.cbh` and `.cbv`
  - SGF, KIF/CSA, xiangqi PGN, PDN
  - Polyglot opening books

  Bragd only rewrites a Foreign database when you choose Save. For a downloaded file it warns first.
- A **Library** is Bragd's own SQLite store, where imports land. A Library can be exported to Parquet for DuckDB.
- **Duplicates** are found by comparing the Zobrist key chain from the same start position, ignoring headers.
  - When Duplicates merge, one Game is kept and every Source is recorded on it.
  - Header conflicts resolve by a Source priority you set, with TWIC above Lichess by default.
  - A Game whose moves are a prefix of another's is a **Near-duplicate**, flagged for review.
- A **Slice** is a named query over Libraries, such as "Grünfeld". It is live by default, and can also be kept as a separate copy.
- **Sources:**
  - TWIC
  - the Lichess database (monthly, with an import filter)
  - the Lichess puzzle and opening explorer APIs
  - PGN Mentor
  - LumbrasGigaBase
  - a custom URL
  - OGS/KGS archives (go), and pychess and Lishogi exports

  Every Source is opt-in. Its size is shown before it downloads, and it can refetch on a schedule.
- **Tablebases:** Syzygy.
  - The wizard recommends 3–5 pieces (about 1 GB).
  - 6 pieces is offered as a download or a folder.
  - 7 pieces is a folder only.
  - When a position isn't covered locally, the Lichess tablebase API answers. This fallback can be turned off.
  - The tablebase folder is passed to every engine.

## The Helper

The Helper is a separate background process shared by every Bragd window ([ADR 0003](adr/0003-helper-is-a-separate-shared-process.md)).
- It fetches Sources, indexes, and finds Duplicates.
- It stays ten minutes after the last window closes, or keeps running with *Start with my session*.
- It survives upgrades and pauses on battery.

## Engines

| Game family | Engines | Protocol |
|---|---|---|
| Chess and its Variants | Stockfish, Fairy-Stockfish | UCI |
| Xiangqi, janggi | Fairy-Stockfish, Pikafish | UCI / UCCI |
| Shogi | Fairy-Stockfish, YaneuraOu | USI |
| Go | KataGo, GNU Go | GTP |
| Draughts | Scan | Hub |
| Backgammon | GNU Backgammon | CLI |
| Tafl, ur, senet | built-in Bots | — |

How engines run:
- An installed Engine is used first. Otherwise Bragd pulls a sealed container image from `ghcr.io/mwo-dk/bragd-<engine>`, with podman or docker.
- Images are pinned by digest. They run with no network, a read-only filesystem, no capabilities and an unprivileged user.
- Pulls show progress and retry by themselves on passing faults.
- An update replaces only our own superseded copy. Images unused for 90 days are pruned.
- Settings shows disk use.

## Online

- **Phase 1:** play on Lichess, Lishogi, pychess.org and OGS.
  - Sign-in is per server over OAuth. Tokens are kept in the OS keychain.
  - Analysis is locked while a rated game against a person is live.
- **Phase 2:** our own Rust game server, as a container on our host, for the Families nobody else hosts. It brings ratings and tournaments, and Bragd's rules crates are authoritative on it.

## First run and upgrades

**Install wizard** steps:
1. Welcome: language, Theme and Board theme.
2. Game families and Variants.
3. Engines: those found installed, or images to pull.
4. Knowledge: recommended Sources and Syzygy 3–5, with size and disk warnings.
5. Online sign-in.
6. Helper: *Start with my session*.

Every step can be skipped, and each links to its Settings area. The wizard can be run again from Settings.

**What's new:** after an upgrade, the tour cards for each feature newer than the version you last saw appear in the same frame as the install wizard.

## Updates

- At start, and once a day after that, Bragd checks the channel it was installed from and shows that channel's upgrade command. It never installs updates itself.
- Channels: GitHub, Homebrew cask, WinGet, Flatpak and Nix.
- The check can be turned off in Settings → Privacy and updates.

## Publishing

| Channel | Files |
|---|---|
| GitHub release | `.deb`, `.rpm`, AppImage (x64, arm64); `.dmg` (Apple Silicon, x64); NSIS `.exe` and `.msi` (x64, arm64); BSD desktop tarballs |
| Homebrew | tap cask |
| WinGet | `mwo-dk.Bragd` |
| Flatpak | `io.github.mwo_dk.Bragd` |
| Nix | flake, nixpkgs `bragd` |
| Scripts | `install/*.sh`, `install/*.ps1` |

The app identifier is `dk.mwo.bragd`. The code is under the MIT licence, and the art notices are listed in [`CREDITS.md`](CREDITS.md).
