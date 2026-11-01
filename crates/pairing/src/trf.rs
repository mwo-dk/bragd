//! Reading FIDE Tournament Report Files (TRF-16, FIDE Handbook C.04 Annex,
//! with the `XXR` rounds and `XXC` initial-colour extensions).
//!
//! Text in, structure out. Only what pairing needs is kept.

use crate::Colour;

/// A parsed tournament report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trf {
    /// Line `012`, trimmed.
    pub name: String,
    /// Line `XXR`, the number of rounds declared.
    pub rounds: Option<u32>,
    /// Line `XXC`: `white1` gives White, `black1` gives Black.
    pub initial_colour: Option<Colour>,
    /// Lines `001`, in file order.
    pub players: Vec<TrfPlayer>,
}

/// One `001` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrfPlayer {
    /// Columns 5-8.
    pub start_rank: u32,
    /// Columns 15-47, trimmed.
    pub name: String,
    /// Columns 49-52; 0 when blank.
    pub rating: u32,
    /// One entry per round present on the line, starting at column 92, ten
    /// columns apart.
    pub rounds: Vec<TrfGame>,
}

/// One round cell of a `001` line: columns 92-95 opponent, 97 colour, 99 result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrfGame {
    /// The opponent's start rank; `None` for `0000` or blank.
    pub opponent: Option<u32>,
    /// `w` or `b`; `None` for `-` or blank.
    pub colour: Option<Colour>,
    /// The result character as written: `1 0 = + - W D L H F U Z`, or ' ' when blank.
    pub result: char,
}

/// Why a file could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrfError {
    /// 1-based line number.
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for TrfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for TrfError {}

/// Parses a TRF-16 file.
pub fn parse(text: &str) -> Result<Trf, TrfError> {
    let _ = text;
    todo!("TRF-16 reader")
}
