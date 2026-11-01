//! The protocols engines speak, as pure dialects: each turns Bragd's requests (start, set a
//! position, think within a limit, stop, quit) into the lines its engines read, and the lines
//! they write back into events. No dialect runs anything, so each is tested with transcripts.
//!
//! | Protocol | Engines | Game family |
//! |---|---|---|
//! | UCI | Stockfish, Fairy-Stockfish, Pikafish | Chess and its Variants, xiangqi, janggi |
//! | UCCI | ElephantEye and other xiangqi engines | Xiangqi |
//! | USI | YaneuraOu, Fairy-Stockfish in USI mode | Shogi |
//! | GTP | KataGo, GNU Go | Go |
//! | Hub | Scan | Draughts |
//! | GNU Backgammon's command line | GNU Backgammon | Backgammon |
//!
//! Moves go to and come from the engine in its own notation (UCI's `e2e4`, USI's `7g7f`, GTP's
//! `D4`, Hub's `32-28`, GNU Backgammon's `8/5 6/5`). `crate::seam` turns them into the
//! entries of `bragd-games` and back.

use serde::{Deserialize, Serialize};

pub mod gnubg;
pub mod gtp;
pub mod hub;
pub mod uci;

use crate::options::EngineOption;

/// The protocol an engine speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Uci,
    Ucci,
    Usi,
    Gtp,
    Hub,
    /// GNU Backgammon's own command line (`gnubg -t`), over its standard input and output.
    Gnubg,
}

impl Protocol {
    /// A fresh dialect of this protocol.
    pub fn dialect(self) -> Box<dyn Dialect> {
        match self {
            Protocol::Uci | Protocol::Ucci | Protocol::Usi => Box::new(uci::Uci::new(self)),
            Protocol::Gtp => Box::new(gtp::Gtp::default()),
            Protocol::Hub => Box::new(hub::Hub),
            Protocol::Gnubg => Box::new(gnubg::Gnubg::default()),
        }
    }
}

/// Where a Game starts: its Variant's own start, or a set-up position in the notation the
/// protocol takes (FEN for UCI and UCCI, SFEN for USI, a Hub position for Scan, GNU
/// Backgammon's `PositionID:MatchID`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Start {
    #[default]
    Initial,
    Setup(String),
}

/// A position to think about: a Variant (its key, `chess`, `xiangqi`, `go19`), where it
/// started, and the moves played since, in the engine's notation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub variant: String,
    pub start: Start,
    pub moves: Vec<String>,
    /// Backgammon only: the dice to play.
    pub dice: Option<[u8; 2]>,
}

impl Position {
    pub fn new(variant: &str, moves: &[&str]) -> Position {
        Position { variant: variant.into(), moves: moves.iter().map(|m| m.to_string()).collect(), ..Default::default() }
    }

    /// Whether the first Side (White, Red, Black in shogi and go) is to move: the set-up
    /// position's side to move, then one Side after the other.
    pub fn first_to_move(&self) -> bool {
        let first = match &self.start {
            Start::Initial => true,
            Start::Setup(s) => {
                let w: Vec<&str> = s.split_whitespace().collect();
                match w.as_slice() {
                    // SFEN: board, side (`b` sente, who moves first), hand, number.
                    [_, side, _, _] if self.variant == "shogi" => *side != "w",
                    // FEN: board, side, ….
                    [_, side, ..] => *side != "b",
                    // A Hub position: its side first (`W` White, who moves first in draughts).
                    [one] => !one.starts_with('B'),
                    _ => true,
                }
            }
        };
        first ^ (self.moves.len() % 2 == 1)
    }
}

/// What ends an engine's thinking about a position.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Limit {
    /// So many milliseconds.
    Time(u32),
    /// So many half-moves deep.
    Depth(u32),
    /// So many positions searched (visits for KataGo).
    Nodes(u64),
    /// A game clock (`matches::clock`): the mover's time left and the other Side's, in
    /// milliseconds, the increment per move, the moves to the next period, and whether the
    /// mover is the first Side.
    Clock { own: u32, other: u32, inc: u32, to_go: Option<u32>, first: bool },
}

/// How long a depth or node count may take before the engine is told to stop.
pub const CAP: std::time::Duration = std::time::Duration::from_secs(60);

impl Limit {
    /// When the engine is told to stop if it has not answered by itself.
    pub fn cap(self) -> std::time::Duration {
        match self {
            Limit::Time(ms) | Limit::Clock { own: ms, .. } => std::time::Duration::from_millis(ms as u64 + 5_000),
            _ => CAP,
        }
    }
}

/// An engine's evaluation, from the side to move's view when a dialect reads it, and from the
/// first Side's view in an `Analysis`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Score {
    /// Centipawns (chess, xiangqi, shogi; draughts in hundredths of a man).
    Cp(i32),
    /// Mate in so many moves; minus: the other side mates. `Mate(0)`: mated now.
    Mate(i32),
    /// Go: the chance to win, 0 to 1, and the lead in points.
    Go { winrate: f32, lead: f32 },
    /// Backgammon: the equity, in points per game.
    Equity(f32),
}

/// A mate counts as this many centipawns and more (`Score::cp`).
pub const MATE_CP: i32 = 1000;

impl Score {
    /// The same evaluation from the other side.
    pub fn flipped(self) -> Score {
        match self {
            Score::Cp(c) => Score::Cp(-c),
            Score::Mate(0) => Score::Mate(0),
            Score::Mate(m) => Score::Mate(-m),
            Score::Go { winrate, lead } => Score::Go { winrate: 1.0 - winrate, lead: -lead },
            Score::Equity(e) => Score::Equity(-e),
        }
    }

    /// In centipawns, for the marks of a review: a mate is `MATE_CP` and more, nearer mates
    /// further out; mated now is the least. None for go and backgammon, which have no
    /// centipawns.
    pub fn cp(self) -> Option<i32> {
        match self {
            Score::Cp(c) => Some(c),
            Score::Mate(m) if m > 0 => Some(MATE_CP + 100 - m.min(99)),
            Score::Mate(m) => Some(-(MATE_CP + 100 + m.max(-99))),
            _ => None,
        }
    }

    /// As shown: `+0.35`, `−1.20`, `#3`, `#−2`, `+2.5 (64%)`, `+0.123`.
    pub fn shown(self) -> String {
        let signed = |v: f64, places: usize| if v < 0.0 { format!("−{:.*}", places, -v) } else { format!("+{v:.places$}") };
        match self {
            Score::Mate(m) if m < 0 => format!("#−{}", -m),
            Score::Mate(m) => format!("#{m}"),
            Score::Cp(c) => signed(c as f64 / 100.0, 2),
            Score::Go { winrate, lead } => format!("{} ({:.0}%)", signed(lead as f64, 1), winrate * 100.0),
            Score::Equity(e) => signed(e as f64, 3),
        }
    }
}

/// One of the engine's reports while it thinks.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Info {
    pub depth: Option<u32>,
    /// Which of its lines, from 1.
    pub line: usize,
    pub score: Option<Score>,
    pub nodes: Option<u64>,
    /// Its line of play, in the engine's notation.
    pub pv: Vec<String>,
}

/// The move the engine settled on.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Best {
    /// None: it has no move (mated, stalemated, no legal move).
    pub mv: Option<String>,
    pub ponder: Option<String>,
}

/// What an engine said.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Event {
    Info(Info),
    Best(Best),
    /// It refused a command: its words.
    Refused(String),
}

/// What an engine says about itself when it starts.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct About {
    /// Its name, as it gives it (`Stockfish 17.1`).
    pub name: String,
    pub options: Vec<EngineOption>,
}

impl About {
    /// Its option called `name`, letter case aside.
    pub fn option(&self, name: &str) -> Option<&EngineOption> {
        self.options.iter().find(|o| o.name.eq_ignore_ascii_case(name))
    }
}

/// A step of the handshake.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Keep reading.
    More,
    /// Send this and keep reading.
    Reply(String),
    /// Ready: send this (maybe nothing) and start.
    Ready(String),
}

/// A protocol, as a state machine over lines. Every text it returns is whole lines, each
/// ending in `\n`; an empty text sends nothing.
pub trait Dialect: Send {
    fn protocol(&self) -> Protocol;
    /// What starts the handshake.
    fn hello(&self) -> String;
    /// One line of the handshake read into `about`.
    fn greeted(&mut self, line: &str, about: &mut About) -> Step;
    /// Sets the option `name` to `value`, both as the engine names them.
    fn set_option(&self, name: &str, value: &str) -> String;
    /// Sets up `p`, with the options the Variant needs (`UCI_Variant`, `UCI_Chess960`).
    fn position(&mut self, about: &About, p: &Position) -> String;
    /// Starts thinking about the position set, for `lines` lines where the engine can.
    fn go(&mut self, about: &About, limit: Limit, lines: u32) -> String;
    /// What one line the engine wrote says.
    fn read(&mut self, line: &str) -> Vec<Event>;
    /// What tells it to stop and name its move; empty where the protocol has none.
    fn stop(&self) -> String;
    fn quit(&self) -> String;
}

/// The word after `key` in `w`.
pub(crate) fn after<'a>(w: &[&'a str], key: &str) -> Option<&'a str> {
    w.iter().position(|x| *x == key).and_then(|i| w.get(i + 1)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_as_shown_and_turned_round() {
        assert_eq!(Score::Cp(35).shown(), "+0.35");
        assert_eq!(Score::Cp(-120).shown(), "−1.20");
        assert_eq!(Score::Mate(3).shown(), "#3");
        assert_eq!(Score::Mate(-2).shown(), "#−2");
        assert_eq!(Score::Go { winrate: 0.64, lead: 2.5 }.shown(), "+2.5 (64%)");
        assert_eq!(Score::Equity(-0.0505).shown(), "−0.051");
        assert_eq!(Score::Cp(30).flipped(), Score::Cp(-30));
        assert_eq!(Score::Mate(0).flipped(), Score::Mate(0));
        assert_eq!(Score::Go { winrate: 0.25, lead: 1.0 }.flipped(), Score::Go { winrate: 0.75, lead: -1.0 });
        assert_eq!((Score::Mate(2).cp(), Score::Mate(5).cp(), Score::Mate(-2).cp(), Score::Mate(0).cp()), (Some(1098), Some(1095), Some(-1098), Some(-1100)));
        assert_eq!(Score::Equity(0.1).cp(), None);
    }

    #[test]
    fn the_side_to_move() {
        assert!(Position::new("chess", &[]).first_to_move());
        assert!(!Position::new("chess", &["e2e4"]).first_to_move());
        let mut p = Position::new("chess", &["c7c5"]);
        p.start = Start::Setup("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".into());
        assert!(p.first_to_move());
        let mut s = Position::new("shogi", &[]);
        s.start = Start::Setup("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1".into());
        assert!(!s.first_to_move());
        let mut d = Position::new("draughts", &[]);
        d.start = Start::Setup("Bbbbbbbbbbbbbbbbbbbbbeeeeeeeeeewwwwwwwwwwwwwwwwwwww".into());
        assert!(!d.first_to_move());
    }
}
