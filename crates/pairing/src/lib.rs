//! Tournament pairing: who plays whom in the next Round.
//!
//! Pure logic with no dependencies and no I/O. The caller hands over every
//! player's history so far and the players who are present; a pairing system
//! hands back the boards, the pairing-allocated bye and, for each bracket, the
//! reasons behind its choice. Other pairing systems sit beside [`dutch`].
//!
//! Scores are whole numbers in the caller's own unit. With the usual 1, ½ and 0
//! points the caller passes `win: 2` and records a draw as 1.

pub mod dutch;
pub mod trf;

mod colour;
mod matching;
mod wide;

/// A colour of the pieces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Colour {
    White,
    Black,
}

impl Colour {
    pub fn opposite(self) -> Colour {
        match self {
            Colour::White => Colour::Black,
            Colour::Black => Colour::White,
        }
    }
}

/// What happened to one player in one finished round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Round {
    /// A game was played.
    Played { opponent: u32, colour: Colour, points: u32 },
    /// The pair was made but the game was not played (a forfeit).
    Forfeit { opponent: u32, points: u32 },
    /// The pairing-allocated bye.
    PairingBye { points: u32 },
    /// Not paired: a requested bye, a late entry or an absence.
    Absent { points: u32 },
}

impl Round {
    pub fn points(self) -> u32 {
        match self {
            Round::Played { points, .. }
            | Round::Forfeit { points, .. }
            | Round::PairingBye { points }
            | Round::Absent { points } => points,
        }
    }
}

/// A player and their history, one entry per finished round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    /// The Tournament Pairing Number (C.04.2 2.3). 1 is the highest ranked.
    pub tpn: u32,
    pub history: Vec<Round>,
}

/// The tournament settings a pairing needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Rounds declared for the tournament (C.04.1 1).
    pub rounds: u32,
    /// Points for a win.
    pub win: u32,
    /// Points for a loss, usually 0.
    pub loss: u32,
    /// The colour drawn by lot before round 1 (C.04.3 5.1).
    pub initial_colour: Colour,
}

/// One board of a pairing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    pub white: u32,
    pub black: u32,
}

/// Why a round cannot be paired, or why the input is unusable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A present player is not in the player list.
    UnknownPlayer(u32),
    /// Two players carry the same TPN.
    DuplicatePlayer(u32),
    /// Histories differ in length; every player needs one entry per finished round.
    UnevenHistory(u32),
    /// The tournament has no rounds left.
    TournamentOver,
    /// No pairing satisfies the absolute criteria (C.04.3 1.9.3): the Chief
    /// Arbiter decides.
    NoLegalPairing,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnknownPlayer(t) => write!(f, "player {t} is present but not registered"),
            Error::DuplicatePlayer(t) => write!(f, "player number {t} is used twice"),
            Error::UnevenHistory(t) => write!(f, "player {t} has a different number of rounds"),
            Error::TournamentOver => write!(f, "every round has been played"),
            Error::NoLegalPairing => write!(f, "no pairing meets the absolute criteria"),
        }
    }
}

impl std::error::Error for Error {}
