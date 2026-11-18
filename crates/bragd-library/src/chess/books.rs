//! Polyglot opening books on the disk: which `.bin` file is a book, the one a game file uses,
//! and its moves (`bragd_gamefiles::chess::polyglot`).

use std::path::{Path, PathBuf};

use bragd_gamefiles::chess::pgn;
use bragd_gamefiles::chess::polyglot::{Book, BookMove};
use rules_chess::Position;
use serde::Serialize;

/// The book at `path`, only when it is a regular file.
pub fn open(path: &Path) -> std::io::Result<Book> {
    Book::new(super::games::open_regular(path)?)
}

/// Whether the file is a Polyglot book: a `.bin` file the reader takes for one (`Book::sane`).
/// Reads a few KB of it.
pub fn is_book(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("bin")) && open(path).is_ok_and(|mut b| b.sane())
}

/// The book for a game file: the file itself when it is a book, else the `configured` one
/// (relative to the game file's folder) when it is one, else the first `.bin` book in the game file's folder, by
/// name.
pub fn find(game: &Path, configured: &str) -> Option<PathBuf> {
    if is_book(game) {
        return Some(game.to_path_buf());
    }
    let configured = configured.trim();
    if !configured.is_empty() {
        let p = game.parent().unwrap_or(Path::new("")).join(configured);
        if is_book(&p) {
            return Some(p);
        }
    }
    let dir = game.parent()?;
    let mut bins: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("bin")))
        .take(1000)
        .collect();
    bins.sort();
    bins.into_iter().find(|p| is_book(p))
}

/// A book in brief: its entries, and its moves from the start position.
#[derive(Clone, Debug, Serialize)]
pub struct Summary {
    pub entries: u64,
    pub moves: Vec<BookMove>,
}

pub fn summary(path: &Path) -> std::io::Result<Summary> {
    let mut b = open(path)?;
    Ok(Summary { entries: b.entries(), moves: b.moves(&Position::starting()) })
}

/// The book's moves for the position `fen`, from the book at `path`.
pub fn moves_at(path: &Path, fen: &str) -> Vec<BookMove> {
    match (open(path), pgn::board(fen)) {
        (Ok(mut book), Some(b)) => book.moves(&b),
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reader's test book: found beside a game file, named in Settings, or not a book.
    #[test]
    fn a_book_on_the_disk() {
        let tiny = Path::new(env!("CARGO_MANIFEST_DIR")).join("../bragd-gamefiles/src/chess/polyglot-test/tiny.bin");
        let d = std::env::temp_dir().join(format!("bragd-books-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("tiny.bin");
        std::fs::copy(&tiny, &p).unwrap();
        assert!(is_book(&p));
        let s = summary(&p).unwrap();
        assert_eq!((s.entries, s.moves.len()), (5, 3));
        assert_eq!(moves_at(&p, &s.moves[0].fen)[0].san, "c5");
        assert!(moves_at(&p, "not a position").is_empty());
        assert_eq!(find(&d.join("games.pgn"), ""), Some(p.clone()));
        assert_eq!(find(&d.join("games.pgn"), &p.to_string_lossy()), Some(p.clone()));
        // Not books: a file of the wrong length, nothing, another ending, a folder.
        let other = d.join("other.bin");
        std::fs::write(&other, [1u8; 17]).unwrap();
        assert!(!is_book(&other));
        assert!(!is_book(&d.join("missing.bin")));
        let renamed = d.join("tiny.dat");
        std::fs::copy(&p, &renamed).unwrap();
        assert!(!is_book(&renamed));
        std::fs::create_dir_all(d.join("dir.bin")).unwrap();
        assert!(!is_book(&d.join("dir.bin")));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
