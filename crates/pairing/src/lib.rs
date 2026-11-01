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
pub mod knockout;
pub mod monrad;
pub mod nordic;
pub mod round_robin;
pub mod scheveningen;
pub mod trf;

mod colour;
mod matching;
mod wide;

/// Matching steps (vertex and edge visits) done on this thread so far. Tests
/// use it to bound how much work a pairing takes without timing it.
#[doc(hidden)]
pub use matching::work;

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
    Played {
        opponent: u32,
        colour: Colour,
        points: u32,
    },
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

/// A paired round, the same shape from every pairing system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pairing {
    /// Boards in publishing order.
    pub boards: Vec<Board>,
    /// The player given the pairing-allocated bye, if the number of players is odd.
    pub bye: Option<u32>,
    /// How each group of players was paired, in the order the system paired
    /// them: for a Swiss system its brackets from the top scoregroup down.
    pub brackets: Vec<Bracket>,
}

/// How one group of players was paired: the facts behind a director's "why".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bracket {
    /// The rule the group was paired by.
    pub rule: Rule,
    /// The score of the resident players, in the caller's points (0 where
    /// the system does not pair by score).
    pub score: u32,
    /// Resident players, by pairing number, in bracket order.
    pub residents: Vec<u32>,
    /// Moved-down players from the bracket above, in bracket order.
    pub moved_down: Vec<u32>,
    /// Pairs made in this bracket, higher ranked player first.
    pub pairs: Vec<(u32, u32)>,
    /// Players who float down to the next bracket (or take the bye).
    pub floaters: Vec<u32>,
    /// True when the pairs are the first candidate: no transposition, no exchange.
    pub natural: bool,
    /// The resident exchange used, as (moved from S1 to S2, moved from S2 to S1), by TPN.
    pub exchange: Option<(Vec<u32>, Vec<u32>)>,
    /// Every quality criterion the chosen candidate does not fully meet, with
    /// its failure count. Empty for a perfect candidate.
    pub failures: Vec<(Criterion, u32)>,
}

/// What a group of players was paired by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rule {
    /// A scoregroup of the FIDE Dutch system (C.04.3).
    Dutch,
    /// A round of FIDE's Berger table for `size` players (C.05 Annex 1).
    /// In `cycle` 2 of a double round-robin the colours are reversed.
    Berger { size: u32, round: u32, cycle: u32 },
    /// A round of a knockout bracket (FIDE World Cup regulations); the byes
    /// are the bracket's floaters.
    Knockout { round: u32 },
    /// A round of a Scheveningen between two teams; colours are reversed in
    /// cycle 2 of a double one.
    Scheveningen { round: u32, cycle: u32 },
    /// DSU's Monrad system: players in placement order, each meeting the
    /// nearest one below not yet met. `natural` when that is 1-2, 3-4, ...
    Monrad,
    /// A score pool of the Nordic Swiss (DSU's Dansk Schweizer): the strong
    /// half meets the weak half, `a1` against `b1`, the middle player floating.
    Nordic,
}

/// The criteria of C.04.3 2.3-2.4 a bracket can fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Criterion {
    /// C5: the score of the player who takes the bye.
    ByeScore,
    /// C6: downfloaters from this bracket.
    Downfloaters,
    /// C7: the scores of the downfloaters, highest first.
    DownfloaterScores,
    /// C8: downfloaters this choice forces out of the next bracket.
    NextBracketDownfloaters,
    /// C8: the scores of those downfloaters, highest first.
    NextBracketDownfloaterScores,
    /// C9: unplayed rounds of the player who takes the bye.
    ByeUnplayedRounds,
    /// C10: topscorers or their opponents beyond a colour difference of 2.
    TopscorerColourDifference,
    /// C11: topscorers or their opponents with the same colour three times running.
    TopscorerColourRepeat,
    /// C12: players who do not get their colour preference.
    ColourPreference,
    /// C13: players who do not get their strong colour preference.
    StrongColourPreference,
    /// C14: resident downfloaters who floated down last round.
    RepeatDownfloat,
    /// C15: opponents of moved-down players who floated up last round.
    RepeatUpfloat,
    /// C16: resident downfloaters who floated down two rounds ago.
    RepeatDownfloatTwoRounds,
    /// C17: opponents of moved-down players who floated up two rounds ago.
    RepeatUpfloatTwoRounds,
    /// C18: moved-down players who floated down last round, again.
    RepeatMovedDown,
    /// C19: their opponents who floated up last round.
    RepeatMovedDownOpponent,
    /// C20: moved-down players who floated down two rounds ago, again.
    RepeatMovedDownTwoRounds,
    /// C21: their opponents who floated up two rounds ago.
    RepeatMovedDownOpponentTwoRounds,
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
    /// A bracket mixes more score levels than the pairing weights can hold.
    TooComplex,
    /// A knockout match of an earlier round has no single winner.
    NoWinner { round: u32, board: u32 },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnknownPlayer(t) => write!(f, "player {t} is present but not registered"),
            Error::DuplicatePlayer(t) => write!(f, "player number {t} is used twice"),
            Error::UnevenHistory(t) => write!(f, "player {t} has a different number of rounds"),
            Error::TournamentOver => write!(f, "every round has been played"),
            Error::NoLegalPairing => write!(f, "no pairing meets the absolute criteria"),
            Error::TooComplex => write!(f, "a bracket mixes too many scores to pair"),
            Error::NoWinner { round, board } => {
                write!(f, "match {board} of round {round} has no single winner")
            }
        }
    }
}

impl std::error::Error for Error {}
