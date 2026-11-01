[← README](../../README.md) · [Docs index](../README.md) · [Reference](README.md)

# config.toml

Every choice you make in Settings is written to one file, `config.toml`, in Bragd's config folder ([Files on disk](files.md)). You can also edit it by hand. Every key has a default, so a missing file, area or key means the default.

## Contents

- [How to reach it](#how-to-reach-it)
- [Every key](#every-key)
- [An example](#an-example)
- [When the file has a mistake](#when-the-file-has-a-mistake)
- [Questions](#questions)

## How to reach it

| Where | What to do |
|---|---|
| Settings | *Settings → Overview* → **Open config.toml** opens it in your system's editor. When there is no file yet, Bragd makes an empty one first |
| By hand | Edit the file in any editor. Bragd reads it when it starts |

A change made in Settings is written at once. Bragd changes only that key and keeps your comments and layout.

## Every key

Keys are written dotted here: `look.theme` is `theme` under `[look]`.

| Key | Default | What it does |
|---|---|---|
| `language` | `"auto"` | The language. `auto` follows the system, or the nearest language Bragd has. Otherwise a code such as `"da"`, `"en-AU"` or `"es-AR"` ([Languages](../look/languages.md)) |
| `look.theme` | `"racing-green"` | The Theme: a built-in name, or one of your `themes` ([Themes](../look/themes.md)) |
| `look.look` | `""` | The Look laid over the Theme. Empty: the Theme's own ([Looks](../look/looks.md)) |
| `look.board_theme` | `"racing"` | `racing`, `slate`, `walnut`, or `match` for colours taken from the Theme ([Board themes](../look/board-themes.md)) |
| `families.family` | every Family on | A table: Family key → `true` or `false`. Only what is off needs writing ([Game families and Variants](../getting-started/game-families.md)) |
| `families.variant` | every Variant on | A table: Variant key → `true` or `false` |
| `engines.containers` | `true` | Run an Engine from its sealed image when it is not installed ([Engine container images](../engines/container-images.md)) |
| `engines.runtime` | `"auto"` | `auto` (podman, else docker), `podman` or `docker` |
| `engines.paths` | empty | A table: Engine key → the program to run instead of the one found ([The Engines page](../engines/engines.md)) |
| `engines.default` | empty | A table: Variant key → the Engine it is played and analysed with. Missing: automatic |
| `engines.added` | empty | A table: Engines added by their path, name → the Game family they play |
| `engines.books` | `[]` | Polyglot opening books for chess, by path ([Opening books](../engines/opening-books.md)) |
| `tablebases.path` | `""` | The Syzygy folder. Empty: `tablebases` in Bragd's data folder ([Tablebases](../engines/tablebases.md)) |
| `tablebases.online` | `true` | Ask the Lichess tablebase API for a position your tables do not cover |
| `knowledge.library` | `""` | Where Libraries are kept. Empty: `libraries` in Bragd's data folder ([Libraries](../knowledge/libraries.md)) |
| `knowledge.source_priority` | `["twic", "lichess-db"]` | Sources in order: the first wins a header conflict between Duplicates ([Duplicates](../knowledge/duplicates.md)) |
| `knowledge.max_unpack_ratio` | `50` | How many times its own size a packed game file (`.pgn.zst`, `.pgn.bz2`, `.pgn.gz`) may unpack to. Past it the import stops, and so does reading it as a Foreign database ([Sources](../knowledge/sources.md#why-did-an-import-stop-saying-the-file-unpacks-too-far)) |
| `helper.start_with_session` | `false` | Start the Helper with your session, not only with a window ([The Helper](../knowledge/helper.md)) |
| `updates.check` | `true` | Ask this copy's channel for a newer version at start and once a day ([Privacy](privacy.md)) |
| `tips.frequency` | `"sometimes"` | How often a Tip may show: `often` (once a day at most), `sometimes` (once a week at most) or `off` ([Tips](../getting-started/tips.md)) |
| `disk.total` | `0` | The most, in bytes, that everything Bragd keeps may take together. `0` is no limit ([Disk watch](../knowledge/disk-watch.md)) |
| `disk.free_below` | `0` | Warn when free space on a disk Bragd uses falls below this many bytes. `0` is no warning |
| `disk.items` | empty | A table: item id → the most bytes it may take. Ids: `library:main.bragd`, `downloads:twic`, `helper`, `tablebases`, `image:<name>`, `positions`, `cache` |
| `analysis.open_games` | `true` | Queue each chess Game you open for background analysis ([Background analysis](../engines/background-analysis.md)) |
| `analysis.depth` | `18` | Half-moves deep for an opened Game's positions |
| `analysis.threads` | `1` | Engines at once in background analysis, each on one thread |
| `analysis.from_hour` | `0` | Background analysis runs from this hour, local time |
| `analysis.until_hour` | `24` | … until this hour; the same as `analysis.from_hour` is any hour |
| `analysis.on_battery` | `false` | Background analysis runs on the battery too |
| `peers.on` | `false` | The Helper listens for your other machines and announces itself on the home network ([Pairing your machines](../peers/peers.md)) |
| `peers.interface` | `""` | The interface the Helper listens on for Peers. Empty: the home network's private addresses |
| `themes` | none | Your own Themes, one table `[themes.<name>]` each. A colour not given is Racing green's. A name like a built-in one replaces it ([Themes](../look/themes.md)) |

## An example

```toml
# Danish, the Nord Theme on Walnut boards
language = "da"

[look]
theme = "nord"
board_theme = "walnut"

[families.family]
backgammon = false

[families.variant]
chaturanga = false

[engines]
runtime = "podman"
books = ["/home/me/books/gm2001.bin"]

[engines.paths]
stockfish = "/opt/stockfish/stockfish"

[engines.default]
xiangqi = "pikafish"

[tablebases]
path = "/data/syzygy"
online = false

[knowledge]
source_priority = ["twic", "lichess-db", "custom:club"]

[helper]
start_with_session = true

[updates]
check = false

[disk]
total = 50000000000
free_below = 5000000000

[disk.items]
"downloads:twic" = 2000000000
```

## When the file has a mistake

A mistake costs only its area. If `[look]` has a value of the wrong type, `[look]` keeps its defaults and the rest of the file is used as you wrote it. A notice names the area and what is wrong, until you mend it or close the notice. A file that is not TOML at all gives the defaults, with a notice.

Settings still save while a mistake of your own is in the file, as long as the save does not add a new one.

A key or area Bragd does not know, such as one from a newer version, is ignored and left in the file.

## Questions

#### Does Bragd rewrite my whole file when I change a setting?

No. It changes the one key, in place, and keeps your comments and the order of the file. A table written inline, such as `family = { go = false }`, stays inline.

#### Do I need to restart after editing the file by hand?

Yes, for most keys: the app reads the file when it starts. Disk watch's limits are read again by the Helper at each check. `knowledge.source_priority` reaches the Helper when it starts; saving it from the Libraries page restarts the Helper for you.

#### Why is `families.family` empty though I turned chess off?

It should hold `chess = false`. Only what is off is written: a Family or Variant missing from the table is on.

#### Can I keep my Engine options in this file?

No. Engine options live in `engine-options.toml` beside it, per Engine ([The Engines page](../engines/engines.md)).

#### Where are Sources and Strip rules?

In the Helper's own `state.json`, not in `config.toml` ([Files on disk](files.md)). Change them on the Sources page.

#### Why are disk sizes in bytes?

TOML has no units. Disk watch lets you type `5 GB` or `500 MB` and writes the bytes for you, in powers of 1000.

---
[← Previous: Reference](README.md) · [Next: Every setting →](settings.md)
