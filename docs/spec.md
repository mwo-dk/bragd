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

- **Theme:** 30 Themes: 28 from the Own, Windows, Mac, Other desktops and Home computers groups, plus Racing green light and dark, plus Looks and user themes. **Racing green** is the default, built on the brand ramp `#00301b` `#004225` `#0b5c39` `#4f9d76` `#e9f0ea`.
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
- **Chat on Lichess (1.0.0):** a Lichess Game's chat, through the Lichess API, with Lichess's own rules. Lichess Games get no Call: the opponent may not be using Bragd.

## Playing together (1.1.0)

Two Bragd players on the Bragd server can Chat and Call during a Game.

- **Bragd server:** a docker compose of its own with Caddy on the Hetzner host, not the pine-z instance. It holds accounts and Friends, runs Games between Bragd players, and brings Calls together with a signalling service and STUN/TURN (coturn).
- **Media:** a Call's voice and camera go directly between the two machines, encrypted end to end (WebRTC, DTLS-SRTP). When no direct path exists, TURN relays the encrypted packets and cannot read them.
- **Consent:** Chat, voice and camera each open only when both Players say yes.
  - All are off by default. Settings holds a default for each: *Ask me* or *Off*.
  - In a Game, a request shows as "Anna would like voice: Allow / Not now".
  - Either Player can close a channel at any time, which closes it for both.
  - A clear indicator shows while your camera or microphone is live.
  - Nothing is recorded. A Chat is kept with the Game only if both agree.
- **Friends:** both players accept, by an invite or a request. A setting counts the members of your Clubs as Friends.
  - Blocking hides a player and refuses their requests. Reporting goes to the Bragd server's operator.
- **Strangers:** a Call is only ever between Friends. A Chat with a stranger opens if both agree, with a word filter you can turn off.
- **In Bragd:** the webview's own WebRTC handles the camera, microphone and media. Rust decides who may call and holds the consent, the server connection and the signalling. Svelte draws the tiles and controls.
  - On Linux this needs GStreamer's WebRTC plugins: the Flatpak runtime has them, and the .deb, .rpm and AppImage list them.

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

No installer is signed with a paid certificate. The macOS app is ad-hoc signed and not notarised; the Homebrew cask clears the quarantine flag, and the install page says how to open the `.dmg` the first time. On Windows, SmartScreen warns once; the install page says to choose *More info*, then *Run anyway*.

The app identifier is `dk.mwo.bragd`. The code is under the MIT licence, and the art notices are listed in [`CREDITS.md`](CREDITS.md).

## Clubs and tournaments (0.3.0)

Everything is local in 0.3.0. Online sync comes later, on Bragd's own server.

- **Club:** members with roles (director, arbiter, member), IDs from FIDE, the national federation and Lichess, and a club rating.
  - The rating is Elo or Glicko-2, chosen per Club; Glicko-2 is the default.
  - A director can edit a rating. Every edit is kept in an audit log: who, when, old value, new value and why.
  - A Club is one file. Copying it shares it, and a lock stops two directors working on it at once.
- **Rating lists:**
  - FIDE, the Danish Chess Union (DSU), and every federation in Bragd's locale countries that publishes a usable list (the research page `reference/rating-lists.md` comes with 0.3.0).
  - The Helper fetches them monthly in the background, once you agree in the wizard.
  - At check-in, players are found in them by name or ID.
- **Pairing systems:** Swiss (FIDE Dutch), Monrad, Nordic Swiss, round-robin (Berger tables), knockout, Scheveningen, team Leagues, and the Drop-in tournament ("spil når du vil").
  - Bragd pairs automatically. The director can change any pairing. Bragd warns when a change breaks a rule and logs the change with its reason.
- **Drop-in tournament:** whoever turns up on an evening is paired among themselves.
  - Standings are by total points, percentage (the default), or best N results.
  - An absence counts as half a point for pairing only.
- **Tiebreaks:** the FIDE set (C.07), ordered per Tournament. Danish defaults come from the DSU rules.
- **On the night:** register, check in, pair, print or show on a second screen, enter results, correct, late entries and withdrawals, publish.
- **Exports:**
  - FIDE TRF;
  - the DSU submission format, if one is published;
  - PGN;
  - a `.bragd-tournament` bundle;
  - standings and crosstable as static HTML.
- **Engine matches:**
  - round-robin, gauntlet, or a two-engine match with SPRT;
  - books, adjudication (including tablebases), and parallel games in the Helper;
  - results go into a Library.
- **Every Family** can run tournaments. Go has handicap pairing as an option, and backgammon has match play to N points.

## Position store (0.4.0)

The same position turns up in thousands of Games. Bragd judges it once and keeps the judgement (`docs/adr/0004-one-position-store-per-machine.md`).

- **Position store:** one SQLite file per machine. It is shared by every Library, Foreign database and Game played, and deleting a Library does not touch it.
  - Rows are keyed by Family, Variant and position key: the Polyglot Zobrist key for chess, a FEN or SFEN hash for draughts, xiangqi and shogi, and a new Zobrist key for go (stones, side to move, ko point).
  - Export and import go through Parquet, for backups, moving a store by hand, and reading it in DuckDB.
- **Evaluation:** one Edition's score, depth and best line for one position.
  - A score has a kind: centipawns, mate in N, win rate (go) or equity (backgammon).
  - Every Edition is kept. Bragd shows the newest Edition's result, and the deepest within it.
  - A new Edition never re-queues anything by itself. It analyses positions you open, or a Slice you ask for.
- **Annotation:** a move's mark (?? ? ?! ! !!), the better move and its line, and named motifs. Bragd words it from its own texts, so it shows in every language.
  - Marks and better moves come in every Family with an Engine. Named motifs (fork, pin, hangs a piece, missed mate) start with chess.
- **Background analysis:** a queue in the Helper.
  - It is filled by positions and Games you open (low priority), and by Slices you mark *Analyse in background*, each with its Engines, Editions and a depth or time per position.
  - Within a job, the positions that occur most often go first.
  - Whole Libraries are never queued.
  - Settings limit it per machine: hours, threads, and never on battery.
  - Each job shows its progress and an estimate of the time left.
- **Housekeeping:** Disk watch shows the store's size per Engine and Edition. One action each prunes:
  - Editions older than the newest;
  - Engines that were removed;
  - shallow Evaluations where a deeper one of the same Edition exists.
- **Families:** every Family with an Engine: chess and its Variants, shogi, xiangqi, janggi, draughts, go and backgammon. Tafl, ur and senet join when their Bots do.

## Peers and Commentary (0.5.0)

- **Peers:** your own machines, paired in Settings → Peers.
  - One machine shows a 6-digit code, and you type it on the other. Each keeps the other's public key.
  - They talk only TLS with those pinned keys. They find each other on the home network (mDNS), or by an address you type, such as a Tailscale name.
  - Either side can unpair.
  - Peers share only the Position store and the queue. Libraries, Slices and Settings stay on each machine. A job carries its positions, so a Peer can work on a Slice it does not have.
  - Merging never conflicts: rows are facts tagged by Edition, and the deeper one wins.
- **Shared queue:** a Peer takes a job when it is idle, inside its own limits, and only if it has a qualifying Edition.
  - A job names the Engine and a minimum Edition, such as "Stockfish 17.1 or newer". Each result records the exact Edition.
  - Container Engines run the same Edition on every Peer, pinned by image digest, so they are preferred for shared jobs.
- **Language model:** Settings → *Language model*, an OpenAI-compatible address, off by default.
  - Bragd offers a local Ollama, Lemonade or LM Studio it finds running.
  - A cloud model needs a key, kept in the OS keychain, and Bragd says plainly that positions and comments then leave the machine.
  - Model jobs go through the shared queue, so one Peer's model can serve the others.
- **Comments:** text a person wrote into a Game is translated only on demand, with a *Translate* button. The result is kept per language and labelled as machine-translated, and the original stays one click away.
- **Commentary:** prose about a position or a move, written by the language model in the reader's language from the Evaluation and the Annotation. It is kept per language, records the model that wrote it, and is labelled as machine-written. It is stored apart from Annotations, as its own column.

## 1.0

1.0.0 is the first release. Nothing is published before it. It follows 0.5.0, once all of these hold:

- the Known broken list in `BASELINE.md` is empty;
- every Family has been played against a real Engine;
- online play on Lichess works;
- Settings and file formats are stable: nothing breaks without a migration.

Bragd's own online server comes after 1.0.
