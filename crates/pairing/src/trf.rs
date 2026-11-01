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
    let mut t = Trf {
        name: String::new(),
        rounds: None,
        initial_colour: None,
        players: Vec::new(),
    };
    for (k, raw) in text.lines().enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let err = |message: String| TrfError {
            line: k + 1,
            message,
        };
        if let Some(rest) = line.strip_prefix("012 ") {
            t.name = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("XXR ") {
            t.rounds = Some(
                rest.trim()
                    .parse()
                    .map_err(|_| err(format!("bad round count {:?}", rest.trim())))?,
            );
        } else if let Some(rest) = line.strip_prefix("XXC ") {
            t.initial_colour = match rest.trim() {
                "white1" => Some(Colour::White),
                "black1" => Some(Colour::Black),
                other => return Err(err(format!("bad initial colour {other:?}"))),
            };
        } else if line.starts_with("001") {
            t.players.push(player(line).map_err(err)?);
        }
    }
    Ok(t)
}

/// Columns `from..=to` (1-based), clipped to the line; empty when the line is shorter.
fn cols(line: &str, from: usize, to: usize) -> &str {
    line.get(from - 1..to.min(line.len())).unwrap_or("")
}

fn player(line: &str) -> Result<TrfPlayer, String> {
    let rank = cols(line, 5, 8).trim();
    let start_rank = rank
        .parse()
        .map_err(|_| format!("bad start rank {rank:?}"))?;
    let rating = cols(line, 49, 52).trim();
    let rating = if rating.is_empty() {
        0
    } else {
        rating
            .parse()
            .map_err(|_| format!("bad rating {rating:?}"))?
    };
    let mut rounds = Vec::new();
    let mut at = 92;
    while line.len() >= at + 3 {
        let opp = cols(line, at, at + 3).trim();
        let opponent = match opp {
            "" | "0000" => None,
            o => Some(o.parse().map_err(|_| format!("bad opponent {o:?}"))?)
                .filter(|&n: &u32| n != 0),
        };
        let colour = match cols(line, at + 5, at + 5) {
            "w" => Some(Colour::White),
            "b" => Some(Colour::Black),
            "" | " " | "-" => None,
            c => return Err(format!("bad colour {c:?}")),
        };
        let result = cols(line, at + 7, at + 7).chars().next().unwrap_or(' ');
        rounds.push(TrfGame {
            opponent,
            colour,
            result,
        });
        at += 10;
    }
    Ok(TrfPlayer {
        start_rank,
        name: cols(line, 15, 47).trim().to_string(),
        rating,
        rounds,
    })
}
