//! Chess games and positions: PGN text (`pgn`), SCID (`scid`) and ChessBase (`cbh`, `cbv`)
//! databases, each game read as the PGN it holds; Polyglot books (`polyglot`); EPD and FEN text
//! as its positions (`positions`). One pass lists a file's games (`scan`), the other families'
//! PGN-like files too (draughts, xiangqi, shogi). Files keep English SAN.

pub mod cbh;
pub mod cbv;
pub mod lex;
pub mod openings;
pub mod pgn;
pub mod polyglot;
pub mod scan;
pub mod scid;

pub use pgn::{Game, Node, Token};

use serde::Serialize;

/// What kind of chess file a name is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    /// Games: `.pgn`, `.pgn.gz`, `.pgn.zst`, `.pgn.bz2`, a SCID database's `.si4` or `.si5` (`scid`), and a ChessBase
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
    /// Xiangqi games: a PGN file whose tags say so (`crate::xiangqi::is_xiangqi`).
    Xiangqi,
    /// Shogi games (`crate::shogi`): KIF (`.kif`, `.kifu`) and CSA (`.csa`).
    Kif,
    Csa,
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
            let ok = pgn::board(&fen).map(|b| b.to_fen());
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
    fn positions_of_epd_and_fen() {
        let p = positions("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - bm Qxf7#; id \"mate\";\n\nnonsense\n8/8/8/8/8/8/8/K6k b - - 3 60\n");
        assert_eq!(p.len(), 3);
        assert!(p[0].fen.is_some() && p[0].about.starts_with("bm Qxf7#"));
        assert!(p[1].fen.is_none());
        assert_eq!(p[2].fen.as_deref(), Some("8/8/8/8/8/8/8/K6k b - - 3 60"));
        let sq = squares(pgn::START);
        assert_eq!((sq[0], sq[4], sq[60], sq[27]), (Some('R'), Some('K'), Some('k'), None));
    }
}
