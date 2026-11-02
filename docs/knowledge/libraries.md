[← README](../../README.md) · [Docs index](../README.md) · [Knowledge](README.md)

# Libraries

A Library is Bragd's own store of Games: one SQLite file ending in `.bragd`. Imports land in it, and each Game appears once, however many Sources brought it. Every Game remembers its Sources. The default Library is `main.bragd`.

![The Libraries tab: main.bragd listed with its Games, size, Duplicates and Near-duplicates, its tabs and Export to Parquet, its Games below, and the Source priority list at the bottom](../screenshots/databases-libraries-1100.png)
*Databases → Libraries.*

## How to reach it

1. In the sidebar, open a Family and choose **Databases**, then the **Libraries** tab.
2. The table lists every `.bragd` file in the Library folder, with its Games, Duplicates and Near-duplicates of the Family whose page you are on, and its size. Click a name to open it.
3. An open Library has its own tabs: **Games**, **Duplicates**, **New Slice**, and one button per saved Slice (a kept copy is marked ⧉). **Export to Parquet** sits under them ([Export to Parquet](parquet-export.md)).
4. **Games** lists the Library's Games of the Family whose page you are on: chess Games on the chess page, go Games on the go page.
5. Click a Game to step through it on its board: **←**, **→**, **Home** and **End**. Its moves are written as its Family writes them, and its comments and move glyphs (`!`, `?!`) show where the file had them.
6. **Analyse** above the board opens the position in [Analyse](../engines/analyse.md), with the moves that led to it, so the Game can be analysed or reviewed from there.

![A go Game of the Library on the go board, stepped to its last move, its moves listed beside it as 1. e5 2. g7 and so on, with Analyse, Flip board and Close above](../screenshots/library-replay-go-1100.png)
*Go → Databases → Libraries: a go Game stepped through on its board.*

Games get into a Library in three ways:

- **Import into Library** on the *Files* tab, for the file that is open ([Foreign databases](foreign-databases.md)).
- A [Source](sources.md) that is on, when the Helper fetches it.
- *Keep as a separate copy* on a [Slice](slices.md), which makes a new Library beside the first.

Imports are jobs of the [Helper](helper.md). They show in the sidebar's Helper line and on the Jobs page.

## What it needs

Nothing to install. The Helper does the imports, and a window starts it when none runs. A Game of any Family from a Library replays on its board: a chess Game is written out as PGN and read back, and the others are played again from their start, each move found by the position it leads to.

## Settings and config.toml

| Setting | `config.toml` key | Default | What it does |
|---|---|---|---|
| Library folder | `knowledge.library` | empty: `<data>/bragd/libraries` | Where Libraries are kept. Sources and imports go into `main.bragd` there |
| Source priority | `knowledge.source_priority` | `["twic", "lichess-db"]` | When Duplicates disagree on a header, the Source listed first wins ([Duplicates](duplicates.md)) |
| Largest unpacking ratio | `knowledge.max_unpack_ratio` | `50` | How far a packed game file may unpack before its import, or reading it in place, stops ([Sources](sources.md#why-did-an-import-stop-saying-the-file-unpacks-too-far)) |

`<data>` is your system's data folder: `~/.local/share` on Linux and the BSDs, `~/Library/Application Support` on macOS and `%APPDATA%` on Windows. See [Files on disk](../reference/files.md).

## Questions

#### Why do the counts change from one Family's page to another?

The Games, Duplicates and Near-duplicates are counted for the page's Family only, as its Games list is. One Library holds the Games of every Family; its size is the whole file's.

#### Is a Library a database server?

No. It is one SQLite file you can copy, back up or move like any other file. Nothing listens on the network.

#### Why is my Library empty?

Nothing has been imported yet. Open a file and press **Import into Library**, or turn on a Source. The first import makes `main.bragd`.

#### Can I have more than one Library?

Yes. Every `.bragd` file in the Library folder is listed. Sources and **Import into Library** always use `main.bragd`; a Slice kept as a separate copy becomes a Library of its own.

#### Does importing the same file twice double its Games?

No. A Game already in the Library is recognised by its moves and kept once. Its Source is recorded again only if it is a new one. Imports are safe to repeat, which is why a job cut short starts again from the start.

#### Why does a Game from my Library have no comments?

A Library keeps the comments and move glyphs of a Game's main line from 0.5.0. A Library made before that kept only the moves and headers. Import the same file again: each Game already there gets its comments, and nothing is added twice. Comments in variations are not kept, since a Library keeps only the main line.

#### What does a Library do with very large files?

A PGN or PDN file, packed or not, and every zip is read as a stream, one Game at a time. Nothing is unpacked to disk, and memory stays small whatever the file's size.

#### Does the Games list show only the Family I came from?

Yes. On the chess page the Games tab lists the Library's chess Games, on the go page its go Games, and so on, filtered further by the player you type. The Libraries table counts every Game, whatever its Family. A Slice can be limited to one Variant of the Family as well.

#### Which Variant ids does a Library store?

The ids of Bragd's own Variants, the same as in *Settings → Game families* and `config.toml`: `chess`, `chess960`, `kingofthehill`, `draughts` (international), `checkers`, `draughts-brazilian`, `xiangqi`, `shogi`, `go-9`, `go-13` and `go-19`. A PGN `Variant` tag of `Standard` or `From Position` is `chess`. A Variant Bragd does not play keeps its own name in lower case, without spaces: `three-check`, `go-7`, `gametype-22`.

---
[← Previous: Foreign databases](foreign-databases.md) · [Next: Duplicates →](duplicates.md)
