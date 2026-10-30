//! The Family and Variant catalogue, and one seam over every rules crate.
//!
//! A Game is stored as a Variant key, an optional initial position and a list
//! of entries in the Variant's own notation. [`replay`] turns that into a
//! [`State`]: whose turn it is, what is legal, how it ended and what to draw.
//! [`apply`] is the same with one more entry, refused if it is not legal.
//!
//! The rules crates decide; this crate only translates. Dice and throws are
//! entries in the record like moves, written by whoever threw them: nothing
//! here is random.

mod backgammon;
mod capablanca;
mod catalogue;
mod chess;
mod draughts;
mod go;
pub mod positions;
mod senet;
mod shogi;
mod tafl;
mod ur;
mod xiangqi;

pub use catalogue::{Family, Geometry, VARIANTS, Variant, VariantInfo};

use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;

/// Everything a front end needs after a list of entries has been played.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub variant: Variant,
    /// The current position in the Family's own notation (FEN, SFEN, …).
    pub position: String,
    /// Who acts next, or `None` once the Game is over.
    pub to_move: Option<Seat>,
    /// Whether the next entry is a move or a throw of the dice.
    pub awaiting: Awaiting,
    /// Every legal move, when a move is awaited.
    pub moves: Vec<LegalMove>,
    /// Legal moves grouped by the square they start from.
    pub targets: BTreeMap<String, Vec<String>>,
    /// Every throw entry the rules accept, when a throw is awaited.
    pub throws: Vec<String>,
    /// The entries played so far, written the way a Player reads them.
    pub history: Vec<String>,
    /// The side to move is in check (chess-like Families only).
    pub check: bool,
    pub outcome: Option<Outcome>,
    pub board: Board,
}

/// One of the two Sides, by its place in the catalogue and its Variant's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Seat {
    /// 0 for the first Side the catalogue names, 1 for the second.
    pub index: u8,
    pub name: &'static str,
}

impl Seat {
    pub(crate) fn of(variant: Variant, index: u8) -> Seat {
        Seat {
            index,
            name: variant.info().sides[index as usize],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Awaiting {
    Move,
    Throw,
    Nothing,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalMove {
    /// The entry to send back to [`apply`].
    pub wire: String,
    /// How a Player writes it: SAN for chess, the Family's notation elsewhere.
    pub notation: String,
    pub from: Option<String>,
    pub to: Option<String>,
    /// The sprite of a piece dropped from hand, for shogi drops and Seirawan gating.
    pub drop: Option<String>,
}

impl LegalMove {
    pub(crate) fn plain(wire: String) -> LegalMove {
        LegalMove {
            notation: wire.clone(),
            wire,
            from: None,
            to: None,
            drop: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// `None` for a draw.
    pub winner: Option<Seat>,
    /// The rules crate's reason, as a camelCase token such as `checkmate`.
    pub reason: String,
    /// `1-0`, `½-½`, a Go score or a backgammon point count.
    pub result: String,
}

/// A family-neutral description of the board to draw.
///
/// Cells are addressed by `x` (left to right) and `y` (bottom to top) as the
/// first Side sees the board. What a cell is depends on [`Geometry`]: a square,
/// an intersection, a backgammon point or a square of a race track.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub geometry: Geometry,
    pub files: u8,
    pub ranks: u8,
    pub chequered: bool,
    pub pieces: Vec<Placed>,
    pub highlights: Vec<Highlight>,
    /// Pieces in hand (shogi, Seirawan) or waiting to enter the board (Ur, senet).
    pub hands: Vec<Hand>,
    /// Pieces that have finished: borne off in backgammon, home in Ur and senet.
    pub off: Vec<Hand>,
    /// The doubling cube, for backgammon.
    pub cube: Option<CubeView>,
    /// The dice or sticks of the throw being played, for backgammon, Ur and senet.
    pub dice: Vec<u8>,
    /// Printed cell labels: draughts square numbers, senet house numbers.
    pub labels: Vec<Label>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Placed {
    pub x: u8,
    pub y: u8,
    /// The cell's name in the Family's notation.
    pub square: String,
    /// The sprite symbol id, carrying the Family name, such as `chess-wK`.
    pub sprite: String,
    /// How many stand here: more than one only for stacked checkers and race pieces.
    pub count: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Highlight {
    pub x: u8,
    pub y: u8,
    pub kind: HighlightKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HighlightKind {
    /// Where the last move started and ended.
    LastMove,
    /// A king in check.
    Check,
    /// A cell the rules give a meaning: a tafl throne or corner, an Ur rosette,
    /// a senet house, a Go star point.
    Marked,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hand {
    pub seat: u8,
    pub pieces: Vec<HandPiece>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CubeView {
    pub value: u16,
    /// The seat that may double next, or `None` while the cube is in the middle.
    pub owner: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandPiece {
    pub sprite: String,
    pub count: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub x: u8,
    pub y: u8,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum GameError {
    UnknownVariant { key: String },
    BadPosition { reason: String },
    /// Entry number `index` (from 0) was not legal where it was played.
    Refused { index: usize, entry: String, reason: String },
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameError::UnknownVariant { key } => write!(f, "no Variant called {key:?}"),
            GameError::BadPosition { reason } => write!(f, "that is not a position: {reason}"),
            GameError::Refused {
                index,
                entry,
                reason,
            } => write!(f, "entry {index} ({entry:?}) refused: {reason}"),
        }
    }
}

impl std::error::Error for GameError {}

impl GameError {
    pub(crate) fn refused(index: usize, entry: &str, reason: impl fmt::Display) -> GameError {
        GameError::Refused {
            index,
            entry: entry.to_string(),
            reason: reason.to_string(),
        }
    }

    pub(crate) fn bad_position(reason: impl fmt::Display) -> GameError {
        GameError::BadPosition {
            reason: reason.to_string(),
        }
    }
}

/// Looks a Variant up by its stored key.
pub fn variant(key: &str) -> Result<Variant, GameError> {
    Variant::from_key(key).ok_or_else(|| GameError::UnknownVariant {
        key: key.to_string(),
    })
}

/// The Variant's own starting position, in its Family's notation.
pub fn start_position(key: &str) -> Result<String, GameError> {
    let variant = variant(key)?;
    Ok(match variant.family() {
        Family::Chess if variant == Variant::Capablanca => capablanca::start(),
        Family::Chess => chess::start(variant),
        Family::Xiangqi => xiangqi::start(variant),
        Family::Shogi => shogi::start(),
        Family::Go => go::start(variant),
        Family::Draughts => draughts::start(variant),
        Family::Tafl => tafl::start(variant),
        Family::Backgammon => backgammon::start(),
        Family::Ur => ur::start(),
        Family::Senet => senet::start(),
    })
}

/// The starting FEN of Chess960 arrangement `index` (taken modulo 960; 518 is standard chess).
pub fn chess960_start(index: u16) -> String {
    chess::chess960(index)
}

/// Plays `entries` from `initial` (or the Variant's start) and describes the result.
pub fn replay<S: AsRef<str>>(
    key: &str,
    initial: Option<&str>,
    entries: &[S],
) -> Result<State, GameError> {
    let variant = variant(key)?;
    let entries: Vec<&str> = entries.iter().map(AsRef::as_ref).collect();
    let initial = initial.filter(|text| !text.trim().is_empty());
    let mut state = match variant.family() {
        Family::Chess if variant == Variant::Capablanca => capablanca::replay(initial, &entries),
        Family::Chess => chess::replay(variant, initial, &entries),
        Family::Xiangqi => xiangqi::replay(variant, initial, &entries),
        Family::Shogi => shogi::replay(initial, &entries),
        Family::Go => go::replay(variant, initial, &entries),
        Family::Draughts => draughts::replay(variant, initial, &entries),
        Family::Tafl => tafl::replay(variant, initial, &entries),
        Family::Backgammon => backgammon::replay(initial, &entries),
        Family::Ur => ur::replay(initial, &entries),
        Family::Senet => senet::replay(initial, &entries),
    }?;
    state.targets = targets(&state.moves);
    Ok(state)
}

/// [`replay`] with one more entry: the Game after `entry`, or why it is refused.
pub fn apply<S: AsRef<str>>(
    key: &str,
    initial: Option<&str>,
    entries: &[S],
    entry: &str,
) -> Result<State, GameError> {
    let mut all: Vec<&str> = entries.iter().map(AsRef::as_ref).collect();
    all.push(entry);
    replay(key, initial, &all)
}

fn targets(moves: &[LegalMove]) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for mv in moves {
        if let (Some(from), Some(to)) = (&mv.from, &mv.to) {
            let list = map.entry(from.clone()).or_default();
            if !list.contains(to) {
                list.push(to.clone());
            }
        }
    }
    map
}
