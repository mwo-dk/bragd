[← README](../README.md)

# Bragd documentation

Every feature has its own page: what it does, how to reach it, what it needs, its settings and `config.toml` keys, and the questions people ask. The pages are grouped in areas below, each with an index of its own. Every page ends with *Previous* and *Next* links in this order, so the docs can be read from start to end.

New here? Start with [Installing Bragd](getting-started/install.md), [The install wizard](getting-started/first-start.md) and [Playing against yourself](play/against-yourself.md). Every version is in [the changelog](changelog.md).

## [Getting started](getting-started/README.md)

| Page | What it covers |
|---|---|
| [Getting started](getting-started/README.md) | The area's pages |
| [Installing Bragd](getting-started/install.md) | Every channel per system, the Helper beside the app, checking the download |
| [The install wizard](getting-started/first-start.md) | The six steps of the first start, skipping, running it again |
| [Game families and Variants](getting-started/game-families.md) | Turning Families and Variants on and off, what disappears and what stays |
| [The sidebar](getting-started/sidebar.md) | Families and their items, collapsing, the Helper line, Settings, Help and What's new |
| [Help (F1) and notices](getting-started/help.md) | F1 → Features, the filter, *Show me*, notices and hints |
| [What's new](getting-started/whats-new.md) | Tour cards after an upgrade, the list of every version |
| [Tips](getting-started/tips.md) | One-line suggestions of features you have not used, and how often they come |

## [Play](play/README.md)

| Page | What it covers |
|---|---|
| [Play](play/README.md) | The area's pages, and the controls every Family shares |
| [Playing against yourself](play/against-yourself.md) | New game, moving, Undo, flipping, the result |
| [Playing against an Engine](play/against-an-engine.md) | Choosing the Engine's Side, strength, time per move, book moves |
| [Chess and its Variants](play/chess.md) | Promotion, castling, Chess960, Seirawan gating, Capablanca |
| [Xiangqi and janggi](play/xiangqi.md) | The palace, the river, janggi's passes |
| [Shogi](play/shogi.md) | Drops, promotion, the hands |
| [Go](play/go.md) | Placing stones, passing, marking dead stones, scoring |
| [Draughts](play/draughts.md) | Captures with several paths, crowning, the four rulesets |
| [Tafl](play/tafl.md) | Attackers and Defenders, the king, the four sizes |
| [Backgammon](play/backgammon.md) | Throwing the dice, whole turns, the bar, bearing off, the doubling cube |
| [Ur](play/ur.md) | Throwing four dice, entering, rosettes |
| [Senet](play/senet.md) | Throwing the sticks, the special squares, bearing off |

## [Knowledge](knowledge/README.md)

| Page | What it covers |
|---|---|
| [Knowledge](knowledge/README.md) | The area's pages, and how Foreign databases, Libraries and Sources fit together |
| [Foreign databases](knowledge/foreign-databases.md) | Opening PGN, SCID, ChessBase, SGF, KIF, CSA and PDN files in place, the list, replay, Save as |
| [Libraries](knowledge/libraries.md) | Bragd's own store, importing, where it is kept |
| [Duplicates](knowledge/duplicates.md) | How a Duplicate is found, merging, Source priority, Near-duplicates |
| [Slices](knowledge/slices.md) | Named queries, live or kept as a copy, position search |
| [Player summary](knowledge/players.md) | One player's results, rating, openings, best wins and worst losses in a Library |
| [Export to Parquet](knowledge/parquet-export.md) | A Library as a Parquet file for DuckDB |
| [Sources](knowledge/sources.md) | TWIC, custom URLs, the listed big Sources, schedules |
| [Strip rules](knowledge/strip-rules.md) | Which Games a Source keeps, preview and apply, after import |
| [Disk watch](knowledge/disk-watch.md) | What Bragd keeps on disk, limits, warnings and what happens when one is crossed |
| [Position store](knowledge/position-store.md) | Evaluations and Annotations kept by position, Export and Import |
| [Language model](knowledge/language-model.md) | Translate a Game's comments with a model on this machine or one you choose |
| [The Helper](knowledge/helper.md) | The background process, its jobs, *Start with my session*, battery |

## [Peers](peers/README.md)

| Page | What it covers |
|---|---|
| [Peers](peers/README.md) | The area's pages |
| [Pairing your machines](peers/peers.md) | Turning Peers on, showing and typing a code, finding a machine, unpairing |
| [Sharing between Peers](peers/sharing.md) | The Position store's sync, the shared queue, and a Peer's language model |
| [How Peers talk](peers/protocol.md) | The pairing code, SPAKE2, TLS with pinned keys, mDNS and the messages |

## [Engines](engines/README.md)

| Page | What it covers |
|---|---|
| [Engines](engines/README.md) | The area's pages, and which Engine plays which Family |
| [The Engines page](engines/engines.md) | Engines found, adding one by its path, the default per Variant, options |
| [Engine container images](engines/container-images.md) | Sealed images, podman or docker, pins, pruning |
| [Analyse](engines/analyse.md) | Engine lines on any position, clicking into a line |
| [Game review](engines/review.md) | Marks on every move, the evaluation profile |
| [Tablebases](engines/tablebases.md) | Syzygy sets, their folder, the Lichess fallback |
| [Opening books](engines/opening-books.md) | Polyglot books and opening names |
| [Engine matches](engines/matches.md) | Round robins, gauntlets and SPRT matches between Engines, played in the Helper |
| [Background analysis](engines/background-analysis.md) | The Helper's queue of positions for the Position store, and its limits |

## [Clubs](clubs/README.md)

| Page | What it covers |
|---|---|
| [Clubs](clubs/README.md) | The area's pages |
| [Clubs](clubs/clubs.md) | Members, roles and IDs in one Club file, club ratings and their audit log, the lock, finding players in rating lists |
| [Tournaments](clubs/tournaments.md) | Swiss and round-robin Tournaments on the night: check-in, pairing, changing a pairing, results, FIDE tiebreaks, publishing and exports |
| [Team Leagues](clubs/leagues.md) | Team Leagues: Teams in board order, Berger Rounds, line-ups, board results, standings by match points then board points |

## [Look](look/README.md)

| Page | What it covers |
|---|---|
| [Look](look/README.md) | The area's pages |
| [Themes](look/themes.md) | The 30 built-in Themes in their groups, and your own |
| [Looks](look/looks.md) | The chrome laid over a Theme |
| [Board themes](look/board-themes.md) | Racing green, Slate, Walnut and Match theme, on every Family's board |
| [Languages](look/languages.md) | Choosing a language, *Automatic*, right to left, mending a translation |
| [File picker](look/file-picker.md) | Coxswain's picker or the system's dialog, for every file Bragd opens or saves |

## [Online](online/README.md)

| Page | What it covers |
|---|---|
| [Online](online/README.md) | The area's pages |
| [Playing online](online/playing-online.md) | Chess on Lichess: signing in, seeks, challenges, clocks, offers, the fair-play lock across windows and chat |
| [Playing on Lishogi](online/lishogi.md) | Shogi on Lishogi: byoyomi, sente and gote, what Lishogi does not offer |
| [Playing on OGS](online/ogs.md) | Go on OGS: rules, byoyomi, passing and counting the score |
| [pychess.org](online/pychess.md) | Why pychess.org cannot be played from Bragd |

## [Reference](reference/README.md)

| Page | What it covers |
|---|---|
| [Reference](reference/README.md) | The area's pages |
| [config.toml](reference/config-toml.md) | Every key, its default and what it does |
| [Every setting](reference/settings.md) | Every item in Settings, by area, with the key it writes |
| [Command line](reference/command-line.md) | `bragd-gui` and `bragd-helper`, their flags and environment variables |
| [Files on disk](reference/files.md) | Where config, data, Libraries, the Helper, tablebases and the cache are kept |
| [File formats](reference/file-formats.md) | Each file's format and version, and what happens to a file from a newer Bragd |
| [Privacy](reference/privacy.md) | What leaves the machine, when, and how to stop it |
| [Rating lists](reference/rating-lists.md) | FIDE's and each federation's list: where, format, terms, rhythm, and which Bragd fetches |
| [Swiss pairing](reference/swiss-pairing.md) | How Bragd pairs a Swiss round under the FIDE Dutch rules, and how that was tested |
| [Round-robin, knockout and Scheveningen](reference/round-robin-and-knockout.md) | Berger tables, knockout brackets with seeds and byes, and team-against-team matches |
| [Monrad and Nordic Swiss](reference/monrad.md) | The Danish Chess Union's Monrad system and its Dansk Schweizer, and how they were tested |
| [Drop-in tournament](reference/drop-in.md) | Club evenings where whoever turns up is paired, and the season's standings |

## [Rules](rules/README.md)

| Page | What it covers |
|---|---|
| [Rules](rules/README.md) | The rules of every Game family and Variant Bragd knows |

---
[← Previous: README](../README.md) · [Next: Getting started →](getting-started/README.md)
