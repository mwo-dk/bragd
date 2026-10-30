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
- [x] S7: GUI shell: branding, splash, sidebar, Themes, Settings with Family toggles, boards for all 25 Variants, play against yourself (wave 2)
- [x] S8: Helper process, Sources (TWIC, custom URL), schedule (wave 2)
- [x] S9: GUI for Databases, Analyse, Engines, Tablebases; install wizard; What's new; F1 Features (wave 3)
- [x] S11: Disk watch and Strip rules: streamed, filtered import of big downloads, pruning of existing downloads and Libraries, a Disk use page with limits and warnings, FICS listed as a Source (wave 3, added by the owner mid-run)
- [x] S10: docs tree, screenshots, morning report in the PR body (wave 3)

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

### S8: Helper

- The Helper is its own program, `bragd-helper`, beside the app. A window starts the one beside its own program when it finds none. Packaging must ship it in every installer, and that becomes an issue.
- Windows reach it over a Unix socket in its folder, `<data>/bragd/helper/`. On Windows it is loopback TCP with the token, as upstream does: a named pipe needs a crate or raw Win32 calls.
- The Helper's lock, address, log, `state.json` and downloads all live in that one folder, in the data folder.
- The Helper does not leave while a job runs, unless the job is paused on the battery.
- A newer window makes the Helper leave. An older window that speaks the same protocol is served, and one that does not is refused. A newer Helper never steps down for an older window.
- "Start with my session" writes a systemd user unit, an XDG autostart entry (the BSDs and the Flatpak), a LaunchAgent, or a Run key on Windows. Task Scheduler is not used. When the registered program is gone after an upgrade, a window registers itself again.
- Battery readers cover Linux, macOS and Windows. The BSDs and illumos count as on mains.
- A metered connection is detected through NetworkManager only. Elsewhere the connection counts as unmetered, so "only on unmetered" never blocks a fetch there.
- Source options and what each fetch remembered (last fetch, last try, last TWIC issue, last error) live in the Helper's `state.json`, not in `config.toml`. They change through the `set_source` request.
- Every Source starts off. The default schedule is weekly for TWIC and custom URLs, and monthly for the listed big Sources.
- On its first fetch TWIC takes only the newest issue. It finds that issue by probing upwards from issue 1500, with steps that double and then halve. After that it takes each new issue in order, at most 26 per fetch.
- A month is 30 days. A failed fetch is not tried again for an hour.
- Lichess database, PGN Mentor, LumbrasGigaBase, OGS, KGS, pychess and Lishogi are listed with rough sizes and URLs, but cannot be fetched. Their notice is `notice.source_out_of_scope`, in English in every locale.
- A custom URL Source has the id `custom:<name>`. It must be an http(s) address of a PGN, SGF, KIF, CSA or PDN file, or a zip of them. It is downloaded again in full on each fetch.
- Downloads are kept, and the oldest go first when the disk cap is passed. Unpacked files are deleted once they are imported. A file inside a zip may unpack to at most 8 GB.
- Sources go into `main.bragd` in the Library folder from Settings, or in `<data>/bragd/libraries/`. Merges use `[knowledge] source_priority`, read when the Helper starts. A change of settings needs a restart, which the `restart` request does.
- A job left running when the Helper stopped starts again from the start, which is safe because imports are idempotent. One whose file has gone fails and says so.
- "Dedupe now" merges again the headers of every Game that has more than one Source, using the current priority (`Library::merge_again`). That makes a change of Source priority reach the Games already merged.
- helper.log has times in UTC. There is no docs page for the Helper yet (S10).
- Tablebase downloads and the Lichess tablebase API use ureq's bundled Mozilla roots, not the OS trust store that the update check uses. To align them, change `bragd-engines/src/fetch.rs`.

### S7: GUI
- `bragd-games` is the one catalogue. `bragd-core/src/families.rs` is deleted; `bragd-core` depends on `bragd-games`, and its Family and Variant switches in Settings are made from the catalogue at start. `Family::from_key` and `Family::has_engine` (false for tafl, Ur and senet) are new in the catalogue.
- `bragd-games` emits the art slice's ids: `capablanca-*` and `seirawan-*` sheets for those Variants, `janggi-r*`/`janggi-b*`, shogi `shogi-b*` for Sente and `shogi-w*` for Gote with `T*`, `H` and `D` for promoted pieces, tafl `aS`/`dS`/`dK`. A test reads `gui/src/lib/sprites/*.svg` and fails on any id the code can emit that no sheet has.
- Backgammon, Ur and senet got small sprite sheets in the draughts style (`backgammon-w/-b`, `ur-l/-d`, `senet-e` cones and `senet-o` spools) rather than shapes drawn in Svelte, so every board draws pieces the same way.
- The render model gained `cells`: every cell with its name, so a click is matched to a legal move's `from` and `to` without the page knowing any notation. A test checks every piece and move end names a cell.
- The page keeps the record (a list of entries) and nothing else; every change replays it in Rust. Undo drops the last entry, a throw included.
- Throws are random in `bragd-gui` (`throw.rs`), from the standard library's random hasher seed, not a cryptographic generator: two dice for backgammon, four two-sided dice for Ur, four sticks for senet (none up counts five). A throw the rules refuse (a double on backgammon's opening roll) is thrown again.
- Several legal moves between the same two cells (promotions, Seirawan gating, draughts capture paths, backgammon turns) are offered as a list by their notation. Ur, senet and backgammon also list every legal move as buttons, since entering from the waiting pieces has no cell.
- Go's scoring: clicking a stone marks it dead one stone at a time (no group selection yet); *Propose* sends `dead:` with them, then Accept or Resume.
- The Rules item shows the `docs/rules` pages for the Family, compiled in with `include_str!` and turned into HTML with `pulldown-cmark`, after the teaching positions. Those pages are development records, not how-to-play pages; links in them are not followed.
- Settings shows Overview, Game families, Look, Engines, Knowledge, Online and Privacy and updates. *Open config.toml*, *Suggest a better translation* and *Run the install wizard again* are not there yet (S9).
- The sidebar's Analyse, Databases, Engines, Tablebases and Opening books open a page saying they come in a later version. The app opens on Play for the first Family that is on.
- The outcome banner shows the winner and the rules crate's result text; the reason (checkmate, …) is not shown yet, and backgammon's "N points" is English from Rust.
- New texts are English in every locale and listed in `UNTRANSLATED.md`.
- `looks.css` is adapted from the source project's, renamed, without its DOS look.
- The sidebar's wordmark is the rune outlines of `assets/brand/wordmark-runes.svg`, recoloured, because runes set in the bundled font draw nothing in WebKitGTK (`BASELINE.md`). The font is still bundled and declared in `app.css`.
- `cap.py` sets the page larger than the window (Emulation.setDeviceMetricsOverride), because Broadway places the window off the page's corner and pictures lost their bottom and right edges.
- Screenshots in `docs/screenshots/`: `sidebar-*`, `settings-families-*`, `play-<family>-*` at 390 and 1100 px, and `play-<variant>-1100.png` for Seirawan, Capablanca, shatranj, janggi, Go 19×19, international draughts, checkers and brandubh.

- Downloads are kept, and the oldest go first when the disk cap is passed. Since S11 nothing is unpacked to disk: zips and packed files are imported as a stream.
### S11: Disk watch and Strip rules
- Big downloads are read as a stream, game by game (`bragd_library::stream`), through the pass over PGN that S4 copied (`chess::games::scan` keeping each game's text). One game's text (at most 16 MB) and one line (at most 1 MB) are held at a time, whatever the file's size. The test proves it by counting: 20,000 games from `.pgn`, `.pgn.gz`, `.pgn.zst`, `.pgn.bz2` and a zip, each unpacked once from start to end, never more than one game held, nothing written beside the file.
- `.pgn.bz2` (FICS) is read with the `bzip2` crate on its pure Rust backend (`libbz2-rs-sys`). The zip crate moved from the Helper to `bragd-library`.
- A PGN or PDN file, packed or not, and every zip are imported as a stream (`Library::import_with`). SCID, ChessBase, SGF, KIF and CSA files still go through the table reader. A zip's SGF, KIF and CSA files are unpacked one at a time beside it (256 MB at most each) and removed once read.
- Sources now hand over their downloads as they came. TWIC's and custom URL zips are imported straight from the zip, and `net::unzip` is gone. A streamed Game's Source reference is `<file>!<member>#<n>` inside a zip, so TWIC Games imported before S11 gain a second reference when imported again.
- A chess Game streamed in without an `Opening` header gets one from its moves, as the table reader gave it.
- New Libraries are made with `auto_vacuum = INCREMENTAL`, so pruning gives pages back with `incremental_vacuum`. Libraries made before stay as they are and are `VACUUM`ed whole after a prune, which needs about as much free space as the Library for a while. Nothing is re-indexed.
- `near_duplicates` gets an index on `long_id`, so deleting a Game does not scan the table. It is made on open, also in existing Libraries.
- A Strip rule (`bragd_library::strip::StripRule`) has: minimum rating of both, either or the average; time control classes; rated only; Variants; a date range; minimum plies; results; titled players only; a player list. Every part must agree for a Game to be kept, and an empty part keeps everything.
- Time control classes follow Lichess: base plus 40 increments, under 3 min bullet (UltraBullet and FICS lightning included), under 8 blitz, under 25 rapid, else classical. `-` and `1/n` mean correspondence. Without a parsable `TimeControl`, the Event's words decide (FICS "standard" is classical). A Game whose class cannot be told, such as most over-the-board Games, is kept by a speed rule.
- A Game counts as rated unless its Event says casual or unrated. A missing rating fails a rating rule, and so does a Game with no known year under a date rule. Lichess's BOT is not a title.
- Pruning a Library deletes a Game only when each of its Sources has a rule and every rule strips it, judged by the merged headers. A Game another Source brought (without a rule, or kept by its rule) stays, with all its Sources recorded. A Source's Strip job looks only at the Games that Source brought (`prune(.., only)`). The Prune job looks at all Games.
- A download is rewritten into a file beside it and renamed over the old one only when complete, so a stop or crash leaves it whole. The rewrite keeps its packing. zstd is written at ruzstd's only level (about zstd -1), so a rewritten Lichess file is larger per Game than Lichess's own. In a zip, only plain PGN or PDN members are rewritten; other members are copied as they are.
- Dry runs count exactly and estimate bytes: a file by the share of game text kept, a Library by the share of Games. The UI shows these numbers before acting.
- Strip rules live in the Source's `Options` in the Helper's `state.json`, as S8 decided for Source options, so `set_source` sets them. Each Source also gets `after_import` (keep, strip, delete) and `auto` (strip, delete oldest, pause), which are all off by default.
- Disk watch lives in the Helper (`bragd_helper::disk`). Its limits are in `[disk]` of config.toml: `total`, `free_below` and `[disk.items]` by item id, all in bytes, 0 meaning none. They are read again at each check, so no restart is needed. They are not in the Settings descriptors yet, because S9 draws them; the `set_disk_limits` request writes them.
- Items: each `*.bragd` in the Library folder (with its `-wal` and `-shm`), each Source's downloads, the Helper's own folder without the downloads, the tablebase folder, each pulled engine image, and `<cache>/bragd`. A tablebase folder outside `<data>/bragd` is marked external: it is shown, counts toward no total and is never touched.
- Free space is checked for `<data>/bragd`, and also for the Library folder when it is elsewhere. It uses `statvfs` through libc on every Unix (libc is now a dependency for all of Unix, not only macOS) and `GetDiskFreeSpaceExW` on Windows. The Windows code is not compiled here: ring and SQLite need MSVC.
- The Helper checks after every job and once an hour. A crossed item limit counts against its Source. The total and the free space count against every Source. The automatic actions run only for Sources that opted in. Strip is queued at most once a day per Source. A paused Source is skipped by the scheduler until a check finds the disk within the limits again.
- Notices are `Crossed` values (an id, a text key and its arguments) sent in `Status.disk` and in the disk report, so each window shows them in its own language. The texts are new keys `notice.disk_*` and `disk.item.*`, in English in every locale and listed in `UNTRANSLATED.md`.
- Image sizes come from `bragd_engines::images::image_size` for `known::images()`. That list is empty until the images are published, so no runtime is asked yet. Tests use a fake list.
- FICS is listed as `fics`, not fetchable yet, like the other big Sources. Comments on the `lichess-db` and `fics` entries describe their future import: kept packed, streamed in with the Strip rule, never unpacked.

### S9c: Wizard and help
- The wizard's logic is `bragd_core::wizard`: the six steps and their Settings areas, the Theme and Board theme swatches, the Knowledge offers, the totals against free space, and what Done saves. `bragd-gui` (`wizard.rs`, `help.rs`) only gathers what it needs: the Helper's Sources, the engines found, the free space.
- Choices in steps 1, 2 and 6 are saved at once, as in Settings. The Knowledge step's ticks stay in the page until Done on the last step. Skip on step 4 unticks everything; Skip on step 6 closes without downloading. Closing the wizard in any way marks it seen. A Settings link in a step closes the wizard and opens that area.
- Recommended and ticked: TWIC (weekly) and Syzygy 3–5 (939 MB, the sum of the mirror's 3-4-5 WDL and DTZ). Sources of Families that are off are left out, and Syzygy too when chess is off. A Source of 10 GB or more carries a disk warning, and the total warns when it would leave less than 5 GB free on the disk of `<data>/bragd`.
- Done turns the chosen Sources on through the Helper (`connect_or_start`, then `set_source` with the options the Helper already holds and `enabled = true`), so the Helper is started then if none runs. The Syzygy download runs on a thread of the window into `[tablebases] path` or `<data>/bragd/tablebases`. TODO(tablebases): its progress and errors only go to stderr until the Tablebases page shows them.
- *Start with my session* in the wizard saves the setting, as Settings does. Registering the service on that switch is not done here (Helper GUI).
- The Engines step groups the known engines by their Game family, since the engines' Variant keys still differ from the catalogue's (S5). Every image shows "not published yet" because every pin is `None`.
- The Online step is a placeholder with links to the four servers.
- Tour cards: six cards for 0.2.0 (`families`, `themes`, `databases`, `engines`, `helper`, `disk`) replace the nine 0.1.0 cards; `play`, `analyse`, `tablebases` and `update` lost their cards and texts. A card may carry the next minor version, since only a release raises the version. `tour::seen` records the newer of the version and the newest card, so a card of the coming release shows once, not at every start. `changelog.d/big-bang.md` names `families`.
- F1 gained `disk` and `wizard` entries. Their docs links point at the area pages that exist (`knowledge/README.md`, `getting-started/README.md`) until S10 writes the feature pages. Search folds case and Latin accents in Rust (a small table, no new crate).
- Notices: `notice_list` gives the config and update notices and, at start, each hint due (`sidebar`, `features`, `wizard`, three starts each). The update check runs once at start on a blocking thread; `BRAGD_OFFLINE` makes `update::check` ask nobody. The upgrade command is copied with the browser's clipboard API.
- *Open config.toml* and the browser links go through `tauri-plugin-opener` from Rust (51 crates more in Cargo.lock, mostly zbus). A missing config.toml is made empty first. *Suggest a better translation* takes the text to mend, optional: `i18n::key_of` finds its key, and the issue names the language, version, key and current text.
- The Family and Variant switches moved to `FamilyToggles.svelte`, which Settings and the wizard share.
- `cap.py` runs the app with `BRAGD_OFFLINE=1`, and gives a demo home without `state.json` one that has seen the wizard, the cards and the hints, so other screenshots stay as they were. `Desktop(state={})` captures a first start.
- The splash was checked: 0.9 s with its fade, 0.6 s and still when motion is reduced; left as it is. The wizard's rise animation runs only without reduced motion.
- `UNTRANSLATED.md` was regenerated from the locale files; it had missed the play and move texts S7 added.
- Screenshots: `wizard-<step>-{390,1100}.png` for the six steps, `whats-new-card-*`, `help-*`, `wizard-welcome-he-1100.png`, `wizard-knowledge-nord-1100.png` and `settings-overview-1100.png`.

### S9a: Knowledge GUI
- The window finds `bragd-helper` beside its own program (`helper::program`) and dials it with `connect_or_start` in the background as it opens; one line serves every request, and a broken line is dialled again once. A Helper that cannot be reached is asked again after 30 s, or at once with *Try again* on the Jobs page.
- Installers carry the Helper as a Tauri sidecar: `packaging/helper-sidecar.sh` builds it for the installer's target into `gui/src-tauri/binaries/bragd-helper-<triple>`, and the release's `tauri build --config` adds `bundle.externalBin`. It is not in `tauri.conf.json`, because tauri-build then wants the file for every `cargo build`. Tauri installs it beside `bragd-gui`, where the window looks. The CI's ARM debug installers do not carry it.
- The file dialog is `tauri-plugin-dialog`, called from Rust only (`db_pick`, `db_save_as`), so the page needs no npm package and no dialog permission.
- Knowledge's commands are in `gui/src-tauri/src/knowledge.rs`, with the Helper's line in `helper_link.rs` and Foreign databases, Libraries, Slices and replay in `databases.rs`. The page's calls are in `gui/src/knowledge.js`, beside `api.js`.
- The sidebar's footer shows the Helper (idle, N jobs, paused on battery, out of reach) with the running job's progress; a click opens the Jobs page. It asks every 5 s, every 1.5 s while jobs run.
- Databases has four tabs per Family: Files (Foreign databases), Libraries, Sources and Disk use. Sources lists the Family's Sources and the custom URL ones. Disk use is also at the end of Settings → Privacy and updates.
- One Foreign database is open at a time, kept in Rust while the window runs, so leaving the page keeps it. Its list asks Rust only for the rows in sight. It filters by player and by opening or ECO, and sorts by any column; a go file filters by player only and does not sort.
- Replay shows a board only where `bragd-games` draws the position the reader gives: chess (and chess960 by the `Variant` header) and shogi. Draughts and xiangqi Games list their moves without a board for now, because their readers give no position per move; go Games show their headers. Each step's board is replayed in Rust from that step's position.
- A Library's chess Game is written out as PGN again and read back, so it replays like a file's. The other Families list their moves.
- The evaluation profile is drawn as a polyline Rust works out, White's advantage up, capped at ±10 pawns, with a mark at the current step.
- "Downloaded" means inside the Helper's folder (Sources' downloads) or in `~/Downloads`. Save as… copies the file; writing over a downloaded file asks first. There is nothing else to save yet, as Games are not edited.
- *Import into Library* is the Helper's `Import` job into the default Library, with the Source `file`.
- The Duplicates review lists Games merged from several Sources (at most 500) with their Sources; the Near-duplicates review lists each pair with links to both. Nothing merges a Near-duplicate: there is no merge for them yet, so the review only opens them. `Library::merged` and `Library::stored` are new in `bragd-library` for this.
- Saving the Source priority writes `[knowledge] source_priority` and restarts the Helper (the `restart` request), which reads it as it starts. A job running then starts again from its beginning.
- Slices are made per Library. Their Family is the page's by default; a Slice with no Family shows under every Family's Databases in the sidebar. The Variant is typed as the Library stores it (`standard`, `chess960`), as the Library's ids are not aligned with `bragd-games` yet.
- "Use current board position" takes the position of the Game last stepped through in replay on the Databases page, not the Play board. Its key goes to the page as text, since JavaScript numbers lose a u64. Only chess positions match, because only chess keys positions the same way in the Library.
- *Keep as a separate copy* writes `<name>.bragd` beside the Library, which then lists as a Library of its own.
- Sizes are typed as shown (`5 GB`, `500 MB`, in powers of 1000 as Disk watch writes them) and read in Rust (`knowledge::parse_size`). An empty field is no limit.
- `disk_total` and `disk_free_below` are setting descriptors now, in Privacy and updates. A number typed into a text field is saved as a number for an option that holds one (`settings::as_number`). Settings shows them on the Disk use page, not as plain rows. Per-item limits stay a table, `[disk.items]`, with no descriptor each.
- Strip rule fields are edited as the rule's own JSON; a rule that keeps everything is saved as none. Preview and Apply save the Source's options first, then start the job, and its line shows the report.
- New texts are English in every language of its own and listed in `UNTRANSLATED.md`. The variant locales (en-AU, de-AT, …) take them from their base language and were not touched.
- `docs/screenshots/demo-home.sh` builds `main.bragd` with `bragd-library`'s new `demo_library` example: the demo games as TWIC, then a Duplicate and a Near-duplicate as Lichess, and the Slice "White wins". The Opera game carries an evaluation profile and a comment.
- Screenshots: `databases-*`, `databases-replay-*`, `databases-libraries-*`, `databases-sources-*`, `databases-disk-*` and `jobs-*` at 390 and 1100 px.

- `bragd-engines` depends on no other Bragd crate, as S1 and S3 were built at the same time. Variants are string keys (`chess`, `kingofthehill`, `go19`, `international`), to be tied to `bragd-games`' catalogue later. (Since S9b it depends on `bragd-games` and uses its keys.)
- Moves go to and come from engines in the engine's own notation (`e2e4`, `7g7f`, `D4`, `32-28`). Turning them into a Variant's notation is the rules crates' work. (Since S9b: `bragd_engines::seam` does it against the seam's legal moves.)
### S9b: Engines GUI
- `bragd-engines` now depends on `bragd-games` and uses its Variant keys (`go-9`, `checkers`, `draughts`, `draughts-brazilian`): its own key list and `Family` enum are gone, so there is one catalogue. A test in `bragd_engines::seam` lists, for all 25 Variants, the engines that play it and what Fairy-Stockfish (`UCI_Variant`) and Scan (`variant`) call it.
- Turning moves between engines and the seam is `bragd_engines::seam`, not the rules crates: an engine's move is matched against the seam's legal moves, so what comes back is always an entry the rules accept, or nothing. Chess and its Variants, shogi and xiangqi with Fairy-Stockfish use the entries as they are (UCI, USI; Chess960 castles king-takes-rook in both). Pikafish and UCCI engines count xiangqi ranks from 0 (ICCS), so their moves are shifted by one rank; any xiangqi engine whose name does not start with "Fairy" is taken to write ranks from 0. Go is GTP's upper case; Hub writes a capture's taken squares after `x` (`28x19x23`), and a capture named by its ends alone is taken when only one legal capture has them.
- GNU Backgammon is given a Position ID made from the seam's board, the player not on roll first (as GNU Backgammon's own code orders it; its manual says the other way round, and the start position cannot tell them apart), and the dice; no Match ID. Its turns are matched by what they do to the mover's checkers, so `8/5(2)`, chains and `*` all match. Backgammon has no review yet, and an Engine Side always takes a double and never offers one.
- A set-up go position cannot be given to a GTP engine (GTP has no position command here): Analyse refuses it. Go's scoring entries are not sent.
- An engine added by its path is checked by starting it in its Family's protocols. One that names itself as an engine Bragd knows (Stockfish) becomes that engine's program (`[engines.paths]`); any other is added under its own name (`[engines.added]` name → Family). It is offered for every Variant of its Family, and refused at start where it lacks `UCI_Variant` (chess Variants, janggi) or USI (shogi).
- New in `config.toml`: `[engines] default` (Variant → engine; none: the first that is installed or pulled), `added`, and `books` (Polyglot books). Books sit under `[engines]` rather than a new area. `Config::remove_value` takes a key out, keeping the file's comments.
- Engine options are kept in `<config>/bragd/engine-options.toml` (the `OptionStore`), listed from a running engine and set through `options::set_option`.
- "Download image" shows only when an engine has a published pin; every pin is `None`, so each engine not installed says "Bragd's image of it is not yet published" with an install hint (package name and download page). No container runtime is asked about an unpublished image, and none ran in the captures.
- Strength is UCI's `Skill Level` (0–20) where the engine offers it; elsewhere only the time per move applies. An Engine in Play keeps running between its moves; Undo against it takes back its reply too.
- Analysis runs on a thread of the window with `Limit::Depth(99)` (an engine stops by itself at the 60 s cap) and the page asks for its lines every 0.4 s; it follows the position while on. Three lines, ten moves each, shown in the Variant's notation, scored from the first Side's view; clicking a move plays the line up to it. Polling was chosen over events: it is what the review already did.
- The review uses `review.rs` at 300 ms a position, cached in `<cache>/bragd/reviews`. Its marks show after each move in the move list.
- Tablebases: the folder is `tablebases.path` (default `<data>/bragd/tablebases`, as the Helper's Disk watch has it). A set's size is asked of the mirror (one small file) only when *Show size* is pressed; the download then runs on a thread of the window (the Helper has no tablebase job yet), with progress and Stop. 7 pieces is a folder only.
- Bragd reads no Syzygy file itself (no probing crate: the Rust ones need GPL code): in Analyse a position the local tables cover says so, and the engine reads them; one they do not cover is asked of the Lichess tablebase when the switch is on. Only standard chess and Chess960 are probed.
- `BRAGD_OFFLINE=1` stops every tablebase lookup and download; `cap.py` sets it, and the tests that touch it set it.
- Book moves and the opening's name (ECO, from `openings.tsv`) show under the board in Play and Analyse, for standard chess only, the name only for Games from the start.
- The fair-play guard is `analyse::locked`: no analysis, review, book moves or tablebase while a rated online Game against a person is live. Online play does not exist yet, so the list of live Games is empty; the function is tested.
- Analyse is the Play page in an analyse mode (same board and record) with an analysis panel; "From the current game" takes the Game last played in Play.
- No new F1 entry or tour card: `analyse`, `engines` and `tablebases` already have both. New texts are English in every language and listed in `UNTRANSLATED.md`.
- Screenshots: `engines-chess-*`, `analyse-chess-*`, `tablebases-*` and `review-chess-*` at 390 and 1100 px, with a fake UCI engine script set as Stockfish's path in the demo home.

### S10: Docs
- 42 feature pages and 7 area indexes under `docs/`, in the order of `docs/README.md`: Getting started (install, the wizard, Game families, the sidebar, F1, What's new), Play (against yourself, against an Engine, one page per Family), Knowledge (Foreign databases, Libraries, Duplicates, Slices, Parquet export, Sources, Strip rules, Disk watch, the Helper), Engines (the page, images, Analyse, review, tablebases, books), Look (Themes, Looks, Board themes, languages), Online (one page, marked not yet available) and Reference (`config.toml`, every setting, the command line, files on disk, privacy). The rules pages stay as they were, behind `rules/README.md`.
- Game families got a page of their own in Getting started and F1 Help one too, though the slice list did not name them: both are F1 entries or visible on every start.
- `docs/README.md` lists every page in a table under its area heading, which is what the docs check reads as the reading order. Each page's title in that table is the one Previous and Next use.
- F1's *Show me* links point at the new pages, and F1 lists the features in the docs' order (the wizard first). `help-*.png` were taken again for it.
- The docs check's coverage is real: it reads the names from the Rust source with patterns (`config.rs`, `settings.rs`, the catalogue, `themes.rs`, `features.rs`, the two `main.rs` and the shipped crates' `BRAGD_*` variables), not from a `cargo run` example, so it stays standard-library Python and needs no build. A name counts as documented when it is a whole backtick span on some page under `docs/`; `config.toml` keys are written dotted (`look.theme`). It also fails when an F1 page does not exist, and when a pattern stops matching the code.
- Every Family and Variant switch (`family_go`, `variant_go-9`) is a setting descriptor, so `reference/settings.md` names all 34.
- The README has two pictures: the GIF and Settings → Look. Its install table follows the systems order (BSDs, Linux, Any, macOS, Windows) and says the channels are empty until the first release.
- `docs/screenshots/pages.py` takes the pictures that need more than one screen: `board-<family>-<theme>.png` for nine Families in four Board themes (the whole window at 1100 px, so the Theme shows; Match with Nord), `play-chess-self.gif` (7.7 s, 800 px, 12 fps, 125 kB) with its first frame, `play-engine-chess-1100.png`, `play-chess-nord-1100.png`, `look-win95-1100.png`, `settings-look-1100.png`, `settings-privacy-1100.png`, `whats-new-1100.png`, `books-1100.png`, `duplicates-1100.png`, `slice-editor-1100.png` and `strip-rule-1100.png`. It writes the demo home's `config.toml` for each picture and removes it after.
- `docs/screenshots/fake-uci`, copied into the demo home by `demo-home.sh`, stands in for Stockfish: it answers from one line of the Italian game. No real engine and no container ran.
- The 390 px pictures that no page shows are kept as the look check `CLAUDE.md` asks for.
- Bug fixed while capturing: against an Engine, the Engine was asked about the position before your last move, so as Black it answered `e2e4` to 1. e4 ("does not name a legal move here"). `Play.svelte`'s `load` set the new state before the new record, and the Engine's turn read both in between. `load` now takes the record's change and makes it first.
- Found and documented as it is, not changed (each for an issue):
  - Castling on 8×8 boards is the king onto its own rook, in standard chess too; dragging the king two squares does nothing.
  - New game in Chess960 always sets up the standard array; `chess960_start` is not used.
  - The move list numbers plies (1. e4 2. e5), not moves.
  - Parquet export has a job in the Helper but no button or command in the app.
  - *Start with my session* only keeps a running Helper; registering it needs `bragd-helper --service on` by hand.
  - `install/install.sh` and `install.ps1` copy only `bragd-gui`, not `bragd-helper`.
  - The banner on a file of a Family that is off is not built, though F1's `families` answer and the spec promise it. F1's `play` line names online play, which does not exist yet.
  - `settings.update_host` is never shown, so Settings does not name where the update check asks.
  - Engine image updates and the 90-day prune are written but never run, and `images_used` is never written.
  - `BRAGD_OFFLINE` is read by the window, not by the Helper.
  - The Duplicates count stops at 500, a Library's Games list is not limited to the page's Family, and a custom URL Source cannot be removed.
  - In draughts, *Which move?* names a capture by its two ends, so two routes can read the same.
  - Seirawan gating works only through *Which move?*; clicking a hawk or elephant in the tray does nothing. Backgammon has no resign button.
