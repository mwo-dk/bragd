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
    /// The move list's number before each entry of `history`. Chess, xiangqi and draughts
    /// number full moves: `1.` before the first Side's move, nothing before the reply, and `1…`
    /// when a Game opens with the second Side. Every other Family numbers each entry: `1`, `2` …
    pub numbers: Vec<String>,
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
    /// Every cell a piece can stand on or move to, with its name in the Family's
    /// notation, so a front end can match a click to a legal move's `from` and `to`.
    pub cells: Vec<Cell>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub x: u8,
    pub y: u8,
    pub square: String,
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
    if state.numbers.is_empty() {
        state.numbers = (1..=state.history.len()).map(|n| n.to_string()).collect();
    }
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

/// Full-move numbers for `n` entries, the first of them ply `first` of the Game (0 is the
/// first Side's first move, 1 the second Side's reply, 2 the first Side's second move …).
pub(crate) fn full_moves(first: u32, n: usize) -> Vec<String> {
    (0..n as u32)
        .map(|i| {
            let ply = first + i;
            let number = ply / 2 + 1;
            match (ply % 2, i) {
                (0, _) => format!("{number}."),
                (_, 0) => format!("{number}…"),
                _ => String::new(),
            }
        })
        .collect()
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

#[cfg(test)]
mod numbering {
    #[test]
    fn full_moves_pair_the_two_sides() {
        assert_eq!(super::full_moves(0, 3), ["1.", "", "2."]);
        assert_eq!(super::full_moves(23, 3), ["12…", "13.", ""]);
        assert!(super::full_moves(0, 0).is_empty());
    }
}

#[cfg(test)]
mod sprite_sheets {
    use super::*;
    use std::collections::BTreeSet;

    /// Every `<symbol id>` in the sprite sheets the app draws from.
    fn sheets() -> BTreeSet<String> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../gui/src/lib/sprites");
        let mut ids = BTreeSet::new();
        for entry in std::fs::read_dir(dir).expect("the sprite sheets") {
            let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
            for part in text.split("<symbol id=\"").skip(1) {
                ids.insert(part.split('"').next().unwrap().to_string());
            }
        }
        ids
    }

    /// Every id each Family's code can emit, role by role.
    fn emitted() -> BTreeSet<String> {
        use rules_chess::Role as R;
        let mut ids = BTreeSet::new();
        let plain = [R::King, R::Queen, R::Rook, R::Bishop, R::Knight, R::Pawn];
        for color in rules_chess::Color::ALL {
            for role in plain.iter().chain(&[R::Ferz, R::Alfil, R::Gaja]) {
                ids.insert(chess::sprite(Variant::Chess, color, *role));
            }
            for role in plain.iter().chain(&[R::Hawk, R::Elephant]) {
                ids.insert(chess::sprite(Variant::Seirawan, color, *role));
            }
        }
        for color in rules_capablanca::Color::ALL {
            for role in rules_capablanca::Role::ALL {
                ids.insert(capablanca::sprite(color, role));
            }
        }
        for side in rules_shogi::Side::ALL {
            for role in rules_shogi::Role::ALL {
                ids.insert(shogi::sprite(rules_shogi::Piece::new(side, role)));
            }
        }
        for variant in [Variant::Xiangqi, Variant::Janggi] {
            for side in rules_xiangqi::Side::ALL {
                for role in rules_xiangqi::Role::ALL {
                    ids.insert(xiangqi::sprite(variant, side, role.letter()));
                }
            }
        }
        for side in rules_backgammon::Side::ALL {
            ids.insert(backgammon::sprite(side).into());
        }
        for side in [rules_ur::Side::White, rules_ur::Side::Black] {
            ids.insert(ur::sprite(side).into());
        }
        for side in [rules_senet::Side::White, rules_senet::Side::Black] {
            ids.insert(senet::sprite(side).into());
        }
        // Draughts and tafl name theirs inline.
        for id in ["draughts-wM", "draughts-bM", "draughts-wK", "draughts-bK"] {
            ids.insert(id.into());
        }
        ids
    }

    #[test]
    fn every_sprite_id_exists_in_the_sheets() {
        let sheets = sheets();
        let mut ids = emitted();
        // And whatever real boards show: every start and every teaching position.
        let lessons = positions::POSITIONS.iter().map(|p| {
            let (initial, line) = p.start();
            (p.variant, initial, line)
        });
        let starts = VARIANTS.iter().map(|info| (info.key, None, &[][..]));
        for (key, initial, line) in starts.chain(lessons) {
            let state = replay(key, initial, line).unwrap_or_else(|e| panic!("{key}: {e}"));
            let board = &state.board;
            ids.extend(board.pieces.iter().map(|p| p.sprite.clone()));
            for hand in board.hands.iter().chain(&board.off) {
                ids.extend(hand.pieces.iter().map(|p| p.sprite.clone()));
            }
            ids.extend(state.moves.iter().filter_map(|m| m.drop.clone()));
        }
        let missing: Vec<_> = ids.iter().filter(|id| !sheets.contains(*id)).collect();
        assert!(missing.is_empty(), "no sprite for {missing:?}");
    }
}
