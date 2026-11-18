//! Chess game files on the disk, read through `bragd_gamefiles`: PGN files (also `.pgn.gz`,
//! `.pgn.zst` and `.pgn.bz2`), SCID and ChessBase databases (`bases`) as a table of games
//! (`games`), Polyglot books (`books`), and EPD and FEN files as their positions. The table also
//! holds the other families' game files that are read the same way (draughts, xiangqi, shogi).

pub mod bases;
pub mod books;
pub mod games;

pub use games::{Query, Row, Sort, Table};

use bragd_gamefiles::chess::{Format, Position, positions};
use std::path::Path;

/// The chess file `path` is, by its name.
pub fn format(path: &Path) -> Option<Format> {
    let n = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    // A SCID database is shown by its index, as a PGN file is (`bases`).
    if [".pgn", ".pgn.gz", ".pgn.zst", ".pgn.bz2", ".si4", ".si5", ".cbh", ".cbv"].iter().any(|e| n.ends_with(e)) {
        Some(Format::Pgn)
    } else if n.ends_with(".epd") || n.ends_with(".fen") {
        Some(Format::Epd)
    } else if n.ends_with(".sgf") {
        Some(Format::Sgf)
    } else if crate::stream::unpacked_name(&n).ends_with(".pdn") {
        Some(Format::Pdn)
    } else {
        bragd_gamefiles::shogi::format(&n)
    }
}

/// The games file `path` is: `format`, for a `.bin` file whether its bytes are a Polyglot
/// book's, and for any other whether they are a DhtmlXQ xiangqi game's (read as PGN). It reads the file, so it is for one file the user is on, not a listing.
pub fn sniff(path: &Path) -> Option<Format> {
    format(path).or_else(|| books::is_book(path).then_some(Format::Book)).or_else(|| games::is_dhtml(path).then_some(Format::Pgn))
}

/// The Go games of the SGF file at `path`, listed as they are read; the file is opened again
/// for each game read.
pub fn go_games(path: &Path) -> std::io::Result<bragd_gamefiles::go::Collection> {
    let p = path.to_path_buf();
    bragd_gamefiles::go::Collection::open(std::sync::Arc::new(move || Ok(Box::new(games::open_regular(&p)?) as Box<dyn bragd_gamefiles::Source>)))
}

/// The positions of the EPD or FEN file at `path`, from its first 16 MB.
pub fn read_positions(path: &Path) -> std::io::Result<Vec<Position>> {
    use std::io::Read;
    let mut bytes = vec![];
    std::fs::File::open(path)?.take(bragd_gamefiles::chess::scan::MAX_GAME as u64).read_to_end(&mut bytes)?;
    Ok(positions(&String::from_utf8_lossy(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(format(Path::new("/a/Games.PGN.gz")), Some(Format::Pgn));
        assert_eq!(format(Path::new("a.pdn.gz")), Some(Format::Pdn));
        assert_eq!(format(Path::new("/a/Club.si4")), Some(Format::Pgn));
        assert_eq!(format(Path::new("/a/Club.si5")), Some(Format::Pgn));
        assert_eq!(format(Path::new("/a/Mega.CBV")), Some(Format::Pgn));
        assert_eq!(format(Path::new("x.fen")), Some(Format::Epd));
        assert_eq!(format(Path::new("x.gz")), None);
        assert_eq!(format(Path::new("Mega.CBH")), Some(Format::Pgn));
        assert_eq!(format(Path::new("g.kifu")), Some(Format::Kif));
    }
}
