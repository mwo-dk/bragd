//! A move, and a list of them.
//!
//! **Not bit-packed.** `crates/rules-chess` squeezes a move into sixteen bits
//! because it chases published perft numbers at depth and the packing pays for
//! itself there. Here it would not: eighty squares need seven bits each, six
//! promotion pieces plus castling and en passant need five more, and the
//! result is a `u32` that is harder to read than the struct below and no faster
//! at the depths this engine is ever asked for. A plain struct is three bytes
//! and says what it means.

use crate::board::Square;
use crate::piece::Role;

/// What kind of move this is — which is to say, what `make_move` has to do
/// beyond picking a piece up and putting it down.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveKind {
    Quiet,
    Capture,
    /// A pawn's two-square opening step. Named because it is the only move that
    /// *creates* an en passant square.
    DoublePush,
    EnPassant,
    /// The king toward the j-file rook.
    CastleKingSide,
    /// The king toward the a-file rook.
    CastleQueenSide,
    Promotion {
        to: Role,
        capture: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub kind: MoveKind,
}

impl Move {
    pub const fn new(from: Square, to: Square, kind: MoveKind) -> Move {
        Move { from, to, kind }
    }

    pub const fn is_capture(self) -> bool {
        matches!(
            self.kind,
            MoveKind::Capture | MoveKind::EnPassant | MoveKind::Promotion { capture: true, .. }
        )
    }

    pub const fn promotion_role(self) -> Option<Role> {
        match self.kind {
            MoveKind::Promotion { to, .. } => Some(to),
            _ => None,
        }
    }

    pub const fn is_castle(self) -> bool {
        matches!(
            self.kind,
            MoveKind::CastleKingSide | MoveKind::CastleQueenSide
        )
    }

    /// The wire form: from-square, to-square, and a promotion letter.
    ///
    /// A castle is spelled by the king's **destination** — `f1i1`, `f1c1` —
    /// which is what the king actually does. Chess960 needed a different
    /// spelling because its king's destination is ambiguous; this
    /// game's is not, since the king starts on f and the arrangement is fixed.
    pub fn to_uci(self) -> String {
        let mut text = format!("{}{}", self.from, self.to);
        if let Some(role) = self.promotion_role() {
            text.push(role.letter().to_ascii_lowercase());
        }
        text
    }
}

/// A fixed-capacity list, so move generation allocates nothing.
///
/// 256 against a branching factor that peaks near a hundred: a queen, a
/// chancellor and an archbishop loose on an open ten-by-eight board is the
/// widest this game gets, and it is not close to the cap.
#[derive(Clone)]
pub struct MoveList {
    moves: [Move; MoveList::CAPACITY],
    length: usize,
}

impl MoveList {
    pub const CAPACITY: usize = 256;

    pub fn new() -> MoveList {
        MoveList {
            // A filler that is never read: `length` is the authority and
            // `as_slice` never looks past it.
            moves: [Move::new(Square::at(0), Square::at(0), MoveKind::Quiet); MoveList::CAPACITY],
            length: 0,
        }
    }

    /// # Panics
    /// If the list is full, which would mean a position generated more moves
    /// than this game can produce — a bug, and one worth stopping for rather
    /// than silently truncating a move list the rules depend on.
    pub fn push(&mut self, mv: Move) {
        assert!(self.length < MoveList::CAPACITY, "move list overflowed");
        self.moves[self.length] = mv;
        self.length += 1;
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.length]
    }

    pub fn contains(&self, mv: Move) -> bool {
        self.as_slice().contains(&mv)
    }

    pub fn retain(&mut self, mut keep: impl FnMut(Move) -> bool) {
        let mut kept = 0;
        for index in 0..self.length {
            let mv = self.moves[index];
            if keep(mv) {
                self.moves[kept] = mv;
                kept += 1;
            }
        }
        self.length = kept;
    }
}

impl Default for MoveList {
    fn default() -> MoveList {
        MoveList::new()
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = Move;
    type IntoIter = core::iter::Copied<core::slice::Iter<'a, Move>>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_castle_is_spelled_by_where_the_king_lands() {
        let mv = Move::new(Square::at(5), Square::at(8), MoveKind::CastleKingSide);
        assert_eq!(mv.to_uci(), "f1i1");
    }

    #[test]
    fn a_promotion_carries_its_piece_in_the_wire_form() {
        let mv = Move::new(
            Square::at(70),
            Square::at(70),
            MoveKind::Promotion {
                to: Role::Chancellor,
                capture: false,
            },
        );
        assert_eq!(mv.to_uci(), "a8a8c");
        assert_eq!(mv.promotion_role(), Some(Role::Chancellor));
    }

    #[test]
    fn retain_keeps_order_and_length() {
        let mut list = MoveList::new();
        for index in 0..10u8 {
            list.push(Move::new(
                Square::at(index),
                Square::at(index + 1),
                MoveKind::Quiet,
            ));
        }
        list.retain(|mv| mv.from.index() % 2 == 0);
        assert_eq!(list.len(), 5);
        assert_eq!(list.as_slice()[0].from.index(), 0);
        assert_eq!(list.as_slice()[4].from.index(), 8);
    }
}
