# bragd-gamefiles

Readers for board-game files, as [Bragd](https://github.com/mwo-dk/bragd) reads them.

| Family | Formats | Module |
|---|---|---|
| Chess | PGN, SCID 4 and 5, ChessBase (`.cbh` and its `.cbv` archives), Polyglot books, EPD and FEN | `chess` |
| Go | SGF | `go` |
| Shogi | KIF, CSA | `shogi` |
| Xiangqi | PGN, DhtmlXQ | `xiangqi` |
| Draughts | PDN | `draughts` |

Each reader reads from a source it is handed: anything that is `Read + Seek + Send` (a file, or a
`Cursor` over bytes in memory). It never opens a path. Opening files, unpacking `.gz`, `.zip` or
`.7z` archives and knowing where files live are the caller's.

A database kept in several files under one name (SCID's `.si4`, `.sg4` and `.sn4`; ChessBase's
`.cbh`, `.cbg`, `.cba` and the rest) is handed over as `Files`: a function that opens each file by
its ending.

```rust,ignore
use bragd_gamefiles::chess::{pgn, scid};
use bragd_gamefiles::Source;
use std::{fs::File, path::PathBuf};

let index = PathBuf::from("Club.si4");
let base = scid::Base::open(
    Box::new(move |ending| Ok(Box::new(File::open(index.with_extension(ending))?) as Box<dyn Source>)),
    false,
)?;
let game = pgn::parse(&base.pgn(0)?);
```

Board logic (legal moves, SAN, FEN, keys) comes from the `rules-*` crates.

Every byte is untrusted. Sizes and counts are checked before use, a game is read up to a limit,
and no input makes a reader panic.

## Versions

The crate has its own version, apart from Bragd's. Programs that use it pin an exact version.
Changes are listed in Bragd's [docs/changelog.md](https://github.com/mwo-dk/bragd/blob/master/docs/changelog.md)
and its decision record [docs/adr/0008](https://github.com/mwo-dk/bragd/blob/master/docs/adr/0008-one-game-file-crate-shared-from-bragd.md).

## Licence

MIT.
