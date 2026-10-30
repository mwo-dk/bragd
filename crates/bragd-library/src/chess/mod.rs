//! Chess games and positions: PGN files (also `.pgn.gz` and `.pgn.zst`), SCID and ChessBase
//! databases as a table of games, each game with its moves and variations; EPD and FEN files as
//! their positions. The table also holds the other families' game files that are read the same
//! way (draughts, xiangqi, shogi). Files keep English SAN.

pub mod cbh;
pub mod cbv;
pub mod games;
pub(crate) mod lex;
pub mod openings;
pub mod pgn;
pub mod scid;
pub mod polyglot;

pub use games::{Table, Progress, Query, Row, Sort};
pub use pgn::{Game, Node, Token};

use serde::Serialize;
use std::path::Path;

/// What kind of chess file a name is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    /// Games: `.pgn`, `.pgn.gz`, `.pgn.zst`, a SCID database's `.si4` or `.si5` (`scid`), and a ChessBase
    /// database's `.cbh` (`cbh`), each read as the PGN it holds.
    Pgn,
    /// Positions, one a line: `.epd`, `.fen`.
    Epd,
    /// Go games: `.sgf` (`crate::go`).
    Sgf,
    /// A Polyglot opening book (`polyglot`): a `.bin` file that is one, by its bytes.
    Book,
    /// Draughts games: `.pdn` (`crate::draughts`), in the same table as PGN games.
    Pdn,
    /// Xiangqi games: a PGN file whose tags say so (`games::kind`, `crate::xiangqi`).
    Xiangqi,
    /// Shogi games (`crate::shogi`): KIF (`.kif`, `.kifu`) and CSA (`.csa`).
    Kif,
    Csa,
}

/// The chess file `path` is, by its name.
pub fn format(path: &Path) -> Option<Format> {
    let n = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    // A SCID database is shown by its index, as a PGN file is (`scid`).
    if [".pgn", ".pgn.gz", ".pgn.zst", ".si4", ".si5", ".cbh", ".cbv"].iter().any(|e| n.ends_with(e)) {
        Some(Format::Pgn)
    } else if n.ends_with(".epd") || n.ends_with(".fen") {
        Some(Format::Epd)
    } else if n.ends_with(".sgf") {
        Some(Format::Sgf)
    } else if n.ends_with(".pdn") {
        Some(Format::Pdn)
    } else { crate::shogi::format(&n) }
}

/// The games file `path` is: `format`, for a `.bin` file whether its bytes are a Polyglot
/// book's, and for any other whether they are a DhtmlXQ xiangqi game's (read as PGN). It reads the file, so it is for one file the user is on, not a listing.
pub fn sniff(path: &Path) -> Option<Format> {
    format(path).or_else(|| polyglot::is_book(path).then_some(Format::Book)).or_else(|| games::is_dhtml(path).then_some(Format::Pgn))
}

/// A position of an EPD or FEN file.
#[derive(Clone, Debug, Serialize)]
pub struct Position {
    /// Its line in the file, from 1.
    pub line: usize,
    /// The full FEN (`pgn::full_fen`); `None` when the line is no position.
    pub fen: Option<String>,
    /// The rest of an EPD line: its operations (`bm Nf3; id "WAC.001";`).
    pub about: String,
}

/// The most positions read from one file.
pub const MAX_POSITIONS: usize = 100_000;

/// The positions of the EPD or FEN file at `path`, from its first 16 MB.
pub fn read_positions(path: &Path) -> std::io::Result<Vec<Position>> {
    use std::io::Read;
    let mut bytes = vec![];
    std::fs::File::open(path)?.take(games::MAX_GAME as u64).read_to_end(&mut bytes)?;
    Ok(positions(&String::from_utf8_lossy(&bytes)))
}

/// The positions of an EPD or FEN file: each line that is not empty.
pub fn positions(text: &str) -> Vec<Position> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .take(MAX_POSITIONS)
        .map(|(i, l)| {
            let fields: Vec<&str> = l.split_whitespace().collect();
            // FEN has six fields (counters last), EPD four and then operations.
            let counters = fields.len() >= 6 && fields[4].parse::<u32>().is_ok() && fields[5].parse::<u32>().is_ok();
            let n = if counters { 6 } else { fields.len().min(4) };
            let fen = fields[..n].join(" ");
            let ok = pgn::board(&fen).map(|b| b.to_string());
            Position { line: i + 1, fen: ok, about: fields[n..].join(" ") }
        })
        .collect()
}

/// A move as shown: its piece letters (and a promotion's) in `pieces` (King, Queen, Rook,
/// Bishop, Knight: figurines or a language's letters), the rest
/// as in the file. `Nf3` is `♘f3`, `e8=Q+` is `e8=♕+`.
pub fn show_san(san: &str, pieces: &[String; 5]) -> String {
    let mut out = String::with_capacity(san.len() + 4);
    for (i, c) in san.chars().enumerate() {
        let k = match c {
            'K' => 0,
            'Q' => 1,
            'R' => 2,
            'B' => 3,
            'N' => 4,
            // A pawn's own letter, which some files write, is not shown.
            'P' if i == 0 => continue,
            c => {
                out.push(c);
                continue;
            }
        };
        out.push_str(&pieces[k]);
    }
    out
}

/// A NAG as its symbol: `$1` is `!`, `$14` is `⩲`. One without a symbol is shown as `$n`.
pub fn nag(n: u8) -> String {
    let s = match n {
        1 => "!",
        2 => "?",
        3 => "‼",
        4 => "⁇",
        5 => "⁉",
        6 => "⁈",
        7 => "□",
        10..=12 => "=",
        13 => "∞",
        14 => "⩲",
        15 => "⩱",
        16 => "±",
        17 => "∓",
        18 | 20 => "+−",
        19 | 21 => "−+",
        22 | 23 => "⨀",
        32 | 33 => "⟳",
        36 | 37 => "↑",
        40 | 41 => "→",
        44 | 45 => "=∞",
        132 | 133 => "⇆",
        138 | 139 => "⊕",
        140 => "∆",
        142 => "⌓",
        145 => "RR",
        146 => "N",
        _ => return format!("${n}"),
    };
    s.to_string()
}

/// The NAGs of a move as shown after it: move marks (`!?`) glued on, the rest after a space.
pub fn nags(list: &[u8]) -> (String, String) {
    let mut glued = String::new();
    let mut apart = vec![];
    for &n in list {
        if (1..=6).contains(&n) {
            // ‼ and friends as two plain characters, which every font has.
            glued.push_str(["!", "?", "!!", "??", "!?", "?!"][n as usize - 1]);
        } else {
            apart.push(nag(n));
        }
    }
    (glued, apart.join(" "))
}

/// The pieces on a board, from FEN: 64 squares from a1 to h8, each a piece letter (`K` White's
/// king, `k` Black's) or `None`.
pub fn squares(fen: &str) -> [Option<char>; 64] {
    let mut out = [None; 64];
    let placement = fen.split_whitespace().next().unwrap_or("");
    for (r, row) in placement.split('/').take(8).enumerate() {
        let rank = 7 - r;
        let mut file = 0usize;
        for c in row.chars() {
            if let Some(d) = c.to_digit(10) {
                file += d as usize;
            } else if file < 8 {
                out[rank * 8 + file] = Some(c);
                file += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_as_shown() {
        let fig = ["♔", "♕", "♖", "♗", "♘"].map(String::from);
        assert_eq!(show_san("Nf3", &fig), "♘f3");
        assert_eq!(show_san("Bxb5+", &fig), "♗xb5+");
        assert_eq!(show_san("e8=Q#", &fig), "e8=♕#");
        assert_eq!(show_san("O-O", &fig), "O-O");
        assert_eq!(show_san("Pe4", &fig), "e4");
        let de = ["K", "D", "T", "L", "S"].map(String::from);
        assert_eq!(show_san("Qxd7+", &de), "Dxd7+");
        assert_eq!(nags(&[1, 14]), ("!".into(), "⩲".into()));
        assert_eq!(nag(200), "$200");
    }

    #[test]
    fn names_and_positions() {
        assert_eq!(format(Path::new("/a/Games.PGN.gz")), Some(Format::Pgn));
        assert_eq!(format(Path::new("/a/Club.si4")), Some(Format::Pgn));
        assert_eq!(format(Path::new("/a/Club.si5")), Some(Format::Pgn));
        assert_eq!(format(Path::new("/a/Mega.CBV")), Some(Format::Pgn));
        assert_eq!(format(Path::new("x.fen")), Some(Format::Epd));
        assert_eq!(format(Path::new("x.gz")), None);
        assert_eq!(format(Path::new("Mega.CBH")), Some(Format::Pgn));
        let p = positions("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - bm Qxf7#; id \"mate\";\n\nnonsense\n8/8/8/8/8/8/8/K6k b - - 3 60\n");
        assert_eq!(p.len(), 3);
        assert!(p[0].fen.is_some() && p[0].about.starts_with("bm Qxf7#"));
        assert!(p[1].fen.is_none());
        assert_eq!(p[2].fen.as_deref(), Some("8/8/8/8/8/8/8/K6k b - - 3 60"));
        let sq = squares(pgn::START);
        assert_eq!((sq[0], sq[4], sq[60], sq[27]), (Some('R'), Some('K'), Some('k'), None));
    }
}
