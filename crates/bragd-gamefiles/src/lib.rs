//! Readers for board-game files: chess (`chess`: PGN, SCID, ChessBase, Polyglot books, EPD),
//! Go (`go`: SGF), shogi (`shogi`: KIF, CSA), xiangqi (`xiangqi`: PGN, DhtmlXQ) and draughts
//! (`draughts`: PDN). Board logic comes from the `rules-*` crates.
//!
//! A reader reads from a source it is handed (`Source`), never from a path: opening files,
//! unpacking archives and knowing where files live are the caller's. A database kept in
//! several files (SCID, ChessBase) is handed over as `Files`, which opens each by its ending.
//!
//! Every byte is untrusted: sizes and counts are checked before use, a game is read up to a
//! limit, and no input makes a reader panic.

// The readers spell out their tuple types.
#![allow(clippy::type_complexity)]

pub mod chess;
pub mod draughts;
pub mod go;
pub mod shogi;
pub mod xiangqi;

use std::io::{self, Read, Seek, SeekFrom};

use serde::Serialize;

/// What a reader reads from: a file, or bytes in memory (`std::io::Cursor`).
pub trait Source: Read + Seek + Send {}
impl<T: Read + Seek + Send> Source for T {}

/// A database kept in several files under one name: each opened by its ending in lower case,
/// without the dot (`"sg4"`, `"cbg"`). A file that is not there is an error.
pub type Files = dyn Fn(&str) -> io::Result<Box<dyn Source>> + Send + Sync;

/// The length of `s`, which is left at its start.
pub(crate) fn len(s: &mut dyn Source) -> io::Result<u64> {
    let n = s.seek(SeekFrom::End(0))?;
    s.seek(SeekFrom::Start(0))?;
    Ok(n)
}

/// How far a listing of a file's games has come.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Progress {
    pub games: usize,
    pub done: bool,
    /// Bytes of the file read, of its size.
    pub read: u64,
    pub size: u64,
    /// Stopped at a limit: only the first `games` games are listed.
    pub capped: bool,
    pub error: Option<String>,
}

/// A time limit for a test, only against a hang: ten times longer in a debug build.
#[cfg(test)]
pub(crate) fn test_limit(normal: std::time::Duration) -> std::time::Duration {
    if cfg!(debug_assertions) { (normal * 10).max(std::time::Duration::from_secs(120)) } else { normal * 2 }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The files beside `path` under its name, for a test.
    pub(crate) fn beside(path: &std::path::Path) -> Box<Files> {
        let p = path.to_path_buf();
        Box::new(move |ext| Ok(Box::new(std::fs::File::open(p.with_extension(ext))?) as Box<dyn Source>))
    }
}
