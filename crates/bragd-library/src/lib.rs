//! Bragd's game databases: Foreign databases read where they lie, through the readers of
//! `bragd_gamefiles` (`chess`), Libraries of Bragd's own in SQLite with Duplicates merged
//! (`library`), Slices over them (`slice`), and a Library's games exported to Parquet (`export`).
//! A player's summary over a Library (`player`). Big downloads are read as a stream, game by game (`stream`), and Strip rules keep only the
//! Games wanted from them, as they come in and once stored (`strip`).

pub mod chess;
pub mod export;
pub mod library;
pub mod player;
pub mod replay;
pub mod slice;
pub mod stream;
pub mod strip;

/// The file formats Bragd reads for the Game family `family`, by the names their users know;
/// none for a Family it has no reader for.
pub fn formats(family: &str) -> &'static [&'static str] {
    match family {
        "chess" => &["PGN", "SCID", "ChessBase"],
        "xiangqi" => &["PGN"],
        "shogi" => &["KIF", "CSA"],
        "go" => &["SGF"],
        "draughts" => &["PDN"],
        _ => &[],
    }
}

/// The formats of `family` a custom URL Source can fetch: those a Library imports from one file.
pub fn url_formats(family: &str) -> Vec<&'static str> {
    formats(family).iter().copied().filter(|f| stream::importable(&format!("games.{}", f.to_ascii_lowercase()))).collect()
}

#[cfg(test)]
mod tests {
    /// Each Family's page names only the formats of its own readers.
    #[test]
    fn formats_of_each_family() {
        assert_eq!(super::formats("go"), ["SGF"]);
        assert!(super::formats("chess").contains(&"ChessBase"));
        assert!(super::formats("tafl").is_empty());
    }

    /// A custom URL takes the formats a Library imports from one file, not SCID or ChessBase.
    #[test]
    fn url_formats_are_the_importable_ones() {
        assert_eq!(super::url_formats("chess"), ["PGN"]);
        assert_eq!(super::url_formats("shogi"), ["KIF", "CSA"]);
        assert_eq!(super::url_formats("go"), ["SGF"]);
        assert_eq!(super::url_formats("draughts"), ["PDN"]);
        assert!(super::url_formats("tafl").is_empty());
    }
}
