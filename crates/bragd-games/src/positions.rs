//! Teaching positions: a board with a common mistake in it, and a paragraph
//! on the move nearly everybody plays there and why it loses.
//!
//! Every Family has some. A position is reached one of two ways:
//!
//! - `line` is empty: the Game starts from `diagram`, which is in the
//!   Family's own notation.
//! - `line` is not empty: the Game starts from the Variant's own start (in
//!   `arrangement`, when the Variant has several) and plays `line`. This is
//!   for the Families whose position notation loses something: Seirawan's FEN
//!   drops the pieces in hand, and the draughts, backgammon, Ur and senet
//!   positions end on the throw or capture the lesson is about. `diagram` is
//!   then only the picture, and the test holds the two together.
//!
//! Its title and text are in the locales, as `position.<key>.title` and
//! `position.<key>.text`.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// A Variant key from the catalogue.
    pub variant: &'static str,
    /// Stable and unique: `<variant>.<slug>`. Its texts are `position.<key>.title` and `.text`.
    pub key: &'static str,
    /// The position in the Family's own notation, as the lesson draws it.
    pub diagram: &'static str,
    /// Entries from the Variant's start that reach the position, or empty.
    pub line: &'static [&'static str],
    /// Which starting arrangement `line` is played from, for backgammon and Ur.
    pub arrangement: Option<u16>,
    /// The seat the text addresses: 0 for the first Side, 1 for the second.
    pub to_move: u8,
    /// The move the text names, in the Family's wire form.
    pub answer: Option<&'static str>,
}

impl Position {
    /// The initial position and entries to hand to [`crate::replay`].
    pub fn start(&self) -> (Option<&'static str>, &'static [&'static str]) {
        if self.line.is_empty() {
            (Some(self.diagram), self.line)
        } else {
            (None, self.line)
        }
    }
}

pub const POSITIONS: &[Position] = &[
    Position {
        variant: "chess",
        key: "chess.back-rank-mate",
        diagram: "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d1d8"),
    },
    Position {
        variant: "chess",
        key: "chess.stalemate-thrown-away",
        diagram: "7k/8/5K2/6Q1/8/8/8/8 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("g5g7"),
    },
    Position {
        variant: "chess",
        key: "chess.knight-fork-on-the-queen",
        diagram: "4k3/5ppp/8/1q6/2N5/8/5PPP/6K1 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("c4d6"),
    },
    Position {
        variant: "chess960",
        key: "chess960.castling-goes-somewhere-else",
        diagram: "r3r1k1/pp3ppp/8/8/8/8/PP3PPP/RK5R w KQ - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("b1a1"),
    },
    Position {
        variant: "shatranj",
        key: "shatranj.baring-the-king",
        diagram: "8/8/k7/1p1F4/4K3/3E4/8/8 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d3b5"),
    },
    Position {
        variant: "chaturanga",
        key: "chaturanga.the-elephant-steps-forward",
        diagram: "2r1k3/5ppp/2PG4/8/8/8/5PPP/6K1 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d6d7"),
    },
    Position {
        variant: "seirawan",
        key: "seirawan.spending-a-gate-square",
        diagram: "rhbqkbsr/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
        line: &["e2e4", "e7e5", "g1f3", "b8c6h", "f1c4", "g8f6e"],
        arrangement: None,
        to_move: 0,
        answer: Some("e1g1H"),
    },
    Position {
        variant: "seirawan",
        key: "seirawan.the-hawk-does-not-come-back",
        diagram: "r1bqkbnr/pppp1ppp/h1n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
        line: &["e2e4", "e7e5", "g1f3", "b8c6h", "f1b5", "b8a6"],
        arrangement: None,
        to_move: 0,
        answer: Some("b5a6"),
    },
    Position {
        variant: "capablanca",
        key: "capablanca.the-archbishop-mates-alone",
        diagram: "k9/10/10/4A5/10/10/10/9K w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("e5c6"),
    },
    Position {
        variant: "capablanca",
        key: "capablanca.a-fork-with-two-different-legs",
        diagram: "5k4/C4ppp2/10/3a6/10/10/5PPP2/6K3 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("a7d7"),
    },
    Position {
        variant: "kingofthehill",
        key: "kingofthehill.the-hill-is-four-squares",
        diagram: "r5k1/5ppp/2n5/8/8/3K4/5PPP/8 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d3e4"),
    },
    Position {
        variant: "racingkings",
        key: "racingkings.the-check-you-cannot-play",
        diagram: "8/1K2k3/8/8/8/8/8/7R w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("h1h8"),
    },
    Position {
        variant: "checkers",
        key: "checkers.past-the-line",
        diagram: "w..w..ww.wb..w.wb.....bbbb.bbb.b",
        line: &["10-14", "22-17", "11-16", "17x10:14", "7x14:10", "26-22", "9-13", "30-26", "3-7", "21-17", "14x30:25:17", "24-20", "7-11", "23-19", "30x23:26", "27x18:23", "16x23:19", "31-27"],
        arrangement: None,
        to_move: 1,
        answer: Some("23-26"),
    },
    Position {
        variant: "draughts",
        key: "draughts.must-take-the-most",
        diagram: "wwwwwww.wwwwwww.w..w.....bb.b.b..b.bbbb.bbb.bbbbbb",
        line: &["31-27", "18-23", "32-28", "23x21:27:28", "33-29", "20-24", "29x20:24", "14x25:20", "38-32", "25-30", "34x25:30", "9-14", "25-20", "15x24:20", "43-38", "17-22"],
        arrangement: None,
        to_move: 0,
        answer: Some("32-27"),
    },
    Position {
        variant: "draughts-brazilian",
        key: "draughts-brazilian.where-the-king-stops",
        diagram: "wwwwww.wb.ww..........bbbbbbW.bb",
        line: &["22-18", "9-14", "18x9:14", "5x14:9", "21-17", "14x21:17", "23-19", "1-5", "27-23", "10-15", "19x1:15:6", "2-6"],
        arrangement: None,
        to_move: 0,
        answer: Some("1x19:6"),
    },
    Position {
        variant: "hnefatafl",
        key: "hnefatafl.shieldwall",
        diagram: ".........../.......X.../.........../........X../.X........./..........X/......XO.../.......X.../........X../...XXX...../..XOKO.....",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("g5g1"),
    },
    Position {
        variant: "hnefatafl",
        key: "hnefatafl.caught-beside-the-throne",
        diagram: ".........../.........../.........../.........../....X..O.../...XK....../.........../.XO.......X/.........../.......X.../....X......",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("e1e5"),
    },
    Position {
        variant: "hnefatafl-fetlar",
        key: "hnefatafl-fetlar.no-shieldwall-in-fetlar",
        diagram: ".........../.......X.../.........../........X../.X........./..........X/......XO.../.......X.../........X../...XXX...../..XOKO.....",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("g5g1"),
    },
    Position {
        variant: "hnefatafl-fetlar",
        key: "hnefatafl-fetlar.fort-that-wins-nothing",
        diagram: ".........../.........../.........../.........../.........../...X......./.X.X.X..X../..X.X....../..OOO....../..O.O....../..OKO......",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: None,
    },
    Position {
        variant: "brandubh",
        key: "brandubh.captured-against-the-corner",
        diagram: "...X.../..X..../...OO../X..K..X/......./...X.../.O...X.",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("f1c1"),
    },
    Position {
        variant: "tablut",
        key: "tablut.lines-before-material",
        diagram: "........./.......X./..X....../.....XO../X.......X/........./..K....../.......X./.........",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("h2c2"),
    },
    Position {
        variant: "xiangqi",
        key: "xiangqi.cannon-needs-a-screen",
        diagram: "4k4/9/b8/9/5N3/9/9/1p4p2/9/3KC4 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("f6e8"),
    },
    Position {
        variant: "xiangqi",
        key: "xiangqi.generals-may-not-face",
        diagram: "4kr3/9/4P4/9/9/4R4/9/9/9/3K5 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("e8e9"),
    },
    Position {
        variant: "janggi",
        key: "janggi.cannon-with-nothing-to-leap",
        diagram: "3ak4/9/9/p8/3P5/4C4/9/9/3K5/9 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d6e6"),
    },
    Position {
        variant: "janggi",
        key: "janggi.bikjang",
        diagram: "2ba5/4k4/9/9/4Pn3/9/9/9/4K4/R2A1R3 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("f1f6"),
    },
    Position {
        variant: "shogi",
        key: "shogi.pawn-drop-that-may-not-mate",
        diagram: "l6nk/9/2p1p4/7NN/9/9/2P1P4/9/4K4 b GP 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("G*1b"),
    },
    Position {
        variant: "shogi",
        key: "shogi.knight-out-of-nowhere",
        diagram: "4k1r1l/3g5/pppp2ppp/9/9/9/PPPP1PPPP/9/4K4 b N 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("N*4c"),
    },
    Position {
        variant: "go-19",
        key: "go-19.the-ladder",
        diagram: ".................../.................../.................../.................../.................../.................../.................../.................../.................../.................../.................../.................../.................../.................../.................../..XO.............../...XX............../.................../................... b",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d5"),
    },
    Position {
        variant: "go-19",
        key: "go-19.the-ladder-that-does-not-work",
        diagram: ".................../.................../.................../.................../.................../.................../.................../.................../.................../.........O........./.................../.................../.................../.................../.................../..XO.............../...XX............../.................../................... b",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: None,
    },
    Position {
        variant: "go-13",
        key: "go-13.the-cut-that-hits-both",
        diagram: "............./............./............./............./............./............./...X........./..XO........./............./..XO........./...X........./............./............. b",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d5"),
    },
    Position {
        variant: "go-9",
        key: "go-9.the-vital-point",
        diagram: "........./........./........./........./........./........./OOOOOOO../OXXXXXO../OX...XO.. b",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d1"),
    },
    Position {
        variant: "backgammon",
        key: "backgammon.the-two-dice-are-one-move",
        diagram: "B1,-,-,-,W2,W4,-,W2,-,-,-,B6,W5,-,-,-,B3,-,B5,-,-,-,-,W2 bar:0/0 off:0/0 cube:1- w",
        line: &["3-1", "8/5 6/5", "6-5", "24/18 18/13", "6-1"],
        arrangement: Some(0),
        to_move: 0,
        answer: Some("13/7 8/7"),
    },
    Position {
        variant: "backgammon",
        key: "backgammon.the-anchor-is-the-take",
        diagram: "-,-,-,-,B2,W5,W5,W3,-,W2,-,B3,-,-,-,-,-,-,B6,-,B4,-,-,- bar:0/0 off:0/0 cube:1- w",
        line: &["6-5", "24/18 18/13", "4-4", "24/20 24/20 13/9 13/9", "6-5", "24/18 18/13", "5-4", "9/4 8/4", "6-6", "13/7 13/7 13/7 13/7", "3-2", "9/6 8/6", "3-3", "13/10 13/10 13/10 10/7", "4-2", "8/4 6/4"],
        arrangement: Some(0),
        to_move: 0,
        answer: None,
    },
    Position {
        variant: "ur",
        key: "ur.the-safe-square-is-worth-sitting-on",
        diagram: "..W.....BW.......... 56 00 w",
        line: &["4", "0-4", "4", "4-8", "2", "0-2", "4", "0-4", "3", "4-7", "1"],
        arrangement: Some(0),
        to_move: 0,
        answer: Some("2-3"),
    },
    Position {
        variant: "senet",
        key: "senet.the-water-is-not-progress",
        diagram: "BWBWBWBWBWBW..B..........W.... 00 w",
        line: &["1", "14-15", "4", "15-19", "4", "19-23", "3", "23-26", "2", "13-15", "1"],
        arrangement: None,
        to_move: 0,
        answer: Some("10-11"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_position_names_a_variant_and_has_a_unique_key() {
        let mut keys: Vec<&str> = POSITIONS.iter().map(|p| p.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), POSITIONS.len());
        for position in POSITIONS {
            assert!(crate::variant(position.variant).is_ok(), "{}", position.key);
            assert!(position.key.starts_with(position.variant), "{}", position.key);
        }
    }

    #[test]
    fn every_position_replays_and_its_answer_is_legal() {
        for position in POSITIONS {
            let (initial, line) = position.start();
            let state = crate::replay(position.variant, initial, line)
                .unwrap_or_else(|e| panic!("{}: {e}", position.key));
            assert_eq!(
                state.to_move.map(|seat| seat.index),
                Some(position.to_move),
                "{}",
                position.key
            );
            if let Some(answer) = position.answer {
                crate::apply(position.variant, initial, line, answer)
                    .unwrap_or_else(|e| panic!("{}: {e}", position.key));
            }
        }
    }
}
