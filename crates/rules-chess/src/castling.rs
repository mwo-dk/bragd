//! Castling rights.
//!
//! **A right is the square its rook started on.** Not a bit, and not a
//! direction — the square. That is the whole design, and everything awkward
//! about Chess960 castling follows from getting it right or wrong here.
//!
//! In standard chess the rook's home is a corner, so a right can be four bits
//! and the corner looked up from a table. Chess960 shuffles the back rank, so
//! the rook's home is a fact about *this game* that the board stops being able
//! to answer the moment anything moves. Carrying it inside the right makes two
//! things unrepresentable rather than merely wrong: a right with no rook, and a
//! rook home with no right. They cannot drift apart because they are one value.
//!
//! What survives from standard chess is the only part Chess960 does not move:
//! **the destinations**. A castling king always ends on the g- or c-file and
//! its rook always ends on the f- or d-file, in every arrangement. Everything
//! else — where the king starts, where the rook starts, which way either
//! travels, whether either moves at all — varies.
//!
//! So `Side` no longer means a direction of travel. Castling "kingside" can
//! move the king towards the a-file, or leave it where it stands. `Side` names
//! *which rook*, and nothing more.

use crate::piece::Color;
use crate::square::{Square, squares::*};
use core::fmt;

/// Which of the two rooks a castling right belongs to.
///
/// Deliberately **not** a direction. In Chess960 the king may move zero
/// squares castling kingside, and may move towards the h-file castling
/// queenside. The names survive because the destinations do — kingside ends on
/// g and f, queenside on c and d — not because anything heads anywhere.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Side {
    /// The right whose king lands on the g-file and rook on the f-file.
    Kingside,
    /// The right whose king lands on the c-file and rook on the d-file.
    Queenside,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::Kingside, Side::Queenside];

    pub const fn index(self) -> usize {
        match self {
            Side::Kingside => 0,
            Side::Queenside => 1,
        }
    }

    /// Where the king lands. Invariant across every arrangement.
    pub const fn king_destination(self, color: Color) -> Square {
        match (color, self) {
            (Color::White, Side::Kingside) => G1,
            (Color::White, Side::Queenside) => C1,
            (Color::Black, Side::Kingside) => G8,
            (Color::Black, Side::Queenside) => C8,
        }
    }

    /// Where the rook lands. Likewise invariant.
    pub const fn rook_destination(self, color: Color) -> Square {
        match (color, self) {
            (Color::White, Side::Kingside) => F1,
            (Color::White, Side::Queenside) => D1,
            (Color::Black, Side::Kingside) => F8,
            (Color::Black, Side::Queenside) => D8,
        }
    }

    /// The file a standard-chess rook of this side starts on.
    ///
    /// Only for reading and writing FEN, where X-FEN's `K` and `Q` mean "the
    /// outermost rook", and for [`CastlingRights::STANDARD`]. Move generation
    /// must never consult it — that was the bug.
    pub const fn standard_rook_file(self) -> u8 {
        match self {
            Side::Kingside => 7,
            Side::Queenside => 0,
        }
    }
}

/// The castling rights alive in a position, each carrying the file its rook
/// started on.
///
/// `None` is an absent right. There is no way to hold a file without a right or
/// a right without a file, which is what makes the two incapable of disagreeing.
/// The rank is not stored because it is the colour's back rank and can be
/// nothing else.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct CastlingRights([Option<u8>; 4]);

const fn slot(color: Color, side: Side) -> usize {
    color.index() * 2 + side.index()
}

impl CastlingRights {
    pub const NONE: CastlingRights = CastlingRights([None; 4]);

    /// Standard chess: rooks on the a- and h-files, all four rights alive.
    ///
    /// Replaces the old `ALL`, which could not survive: "all rights" is not a
    /// value any more without saying which rooks they belong to.
    pub const STANDARD: CastlingRights = CastlingRights([Some(7), Some(0), Some(7), Some(0)]);

    pub const fn has(self, color: Color, side: Side) -> bool {
        self.0[slot(color, side)].is_some()
    }

    /// The file this right's rook started on.
    pub const fn rook_file(self, color: Color, side: Side) -> Option<u8> {
        self.0[slot(color, side)]
    }

    /// The square this right's rook started on, and therefore still stands on:
    /// a right dies the moment its rook leaves or is taken.
    pub fn rook_origin(self, color: Color, side: Side) -> Option<Square> {
        self.rook_file(color, side)
            .map(|file| Square::from_file_rank(file, color.back_rank()))
    }

    pub const fn with(self, color: Color, side: Side, rook_file: u8) -> CastlingRights {
        let mut next = self;
        next.0[slot(color, side)] = Some(rook_file);
        next
    }

    pub const fn without(self, color: Color, side: Side) -> CastlingRights {
        let mut next = self;
        next.0[slot(color, side)] = None;
        next
    }

    /// Removes both of a colour's rights, as when its king moves.
    pub const fn without_color(self, color: Color) -> CastlingRights {
        self.without(color, Side::Kingside)
            .without(color, Side::Queenside)
    }

    /// The rights that survive a move.
    ///
    /// Two separate questions, and the old six-constant square table answered
    /// neither correctly once the rooks moved off the corners:
    ///
    /// - **Did a king move?** A role question. Any king move — including
    ///   castling itself — ends both of that colour's rights, wherever the king
    ///   was standing.
    /// - **Did a castling rook leave its square, or get taken on it?** A
    ///   question about the *stored* square, which is why the square is stored.
    ///   Checking `from` catches the rook moving away; checking `to` catches it
    ///   being captured where it stands, which is the case perft position 5
    ///   exists to catch. A promoted rook arriving on a1 grants nothing,
    ///   because the right names a square and that square's original rook is
    ///   long gone.
    pub fn after_move(
        self,
        mover: Color,
        is_king: bool,
        from: Square,
        to: Square,
    ) -> CastlingRights {
        let mut next = if is_king {
            self.without_color(mover)
        } else {
            self
        };
        for color in Color::ALL {
            for side in Side::ALL {
                if let Some(origin) = next.rook_origin(color, side)
                    && (origin == from || origin == to) {
                        next = next.without(color, side);
                    }
            }
        }
        next
    }

    pub const fn is_empty(self) -> bool {
        self.0[0].is_none() && self.0[1].is_none() && self.0[2].is_none() && self.0[3].is_none()
    }

    /// Every live right, as (colour, side, rook file).
    pub fn alive(self) -> impl Iterator<Item = (Color, Side, u8)> {
        Color::ALL.into_iter().flat_map(move |color| {
            Side::ALL
                .into_iter()
                .filter_map(move |side| self.rook_file(color, side).map(|file| (color, side, file)))
        })
    }
}

/// Why a castling field could not be read.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CastlingFieldError {
    /// Not a letter any dialect uses.
    NotACastlingLetter(char),
    /// The same right named twice.
    Repeated(Color, Side),
    /// A colour has rights but no king on its back rank to hold them.
    NoKing(Color),
    /// A file letter naming a square with no rook of that colour on it.
    NoRookOnFile(Color, u8),
    /// `K` or `Q` with no rook outward of the king on that side.
    NoOutermostRook(Color, Side),
    /// A rook named on the king's own square, or on the wrong side of it.
    WrongSideOfTheKing(Color, u8),
}

impl fmt::Display for CastlingFieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CastlingFieldError::NotACastlingLetter(letter) => {
                write!(f, "{letter:?} is not a castling letter")
            }
            CastlingFieldError::Repeated(color, side) => {
                write!(f, "{color:?} {side:?} is named twice")
            }
            CastlingFieldError::NoKing(color) => write!(f, "{color:?} has rights but no king"),
            CastlingFieldError::NoRookOnFile(color, file) => {
                write!(f, "no {color:?} rook on file {}", (b'a' + file) as char)
            }
            CastlingFieldError::NoOutermostRook(color, side) => {
                write!(f, "no outermost {color:?} rook for {side:?}")
            }
            CastlingFieldError::WrongSideOfTheKing(color, file) => {
                write!(
                    f,
                    "the {color:?} rook on file {} is not on the side it claims",
                    (b'a' + file) as char
                )
            }
        }
    }
}

impl fmt::Debug for CastlingRights {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "-");
        }
        // Shredder-style, which is the unambiguous one: a right *is* a rook
        // file, and this is for reading a failing test rather than for a wire.
        for (color, _side, file) in self.alive() {
            let letter = (b'A' + file) as char;
            f.write_str(&if color == Color::White {
                letter.to_string()
            } else {
                letter.to_ascii_lowercase().to_string()
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const D4: Square = Square::new(27);
    const H7: Square = Square::new(55);

    #[test]
    fn rights_are_independent() {
        let mut rights = CastlingRights::STANDARD;
        rights = rights.without(Color::White, Side::Kingside);
        assert!(!rights.has(Color::White, Side::Kingside));
        assert!(rights.has(Color::White, Side::Queenside));
        assert!(rights.has(Color::Black, Side::Kingside));
        assert!(rights.has(Color::Black, Side::Queenside));
    }

    #[test]
    fn a_right_knows_which_rook_it_names() {
        let rights = CastlingRights::NONE.with(Color::White, Side::Queenside, 1);
        assert_eq!(rights.rook_origin(Color::White, Side::Queenside), Some(B1));
        // And the rank is the colour's, never stored.
        let black = CastlingRights::NONE.with(Color::Black, Side::Kingside, 5);
        assert_eq!(black.rook_origin(Color::Black, Side::Kingside), Some(F8));
    }

    #[test]
    fn capturing_a_rook_where_it_stands_ends_that_right() {
        // The case perft position 5 exists to catch, now asked of the stored
        // square rather than of a corner.
        let rights = CastlingRights::STANDARD.after_move(Color::White, false, A1, H8);
        assert!(!rights.has(Color::Black, Side::Kingside));
        assert!(
            !rights.has(Color::White, Side::Queenside),
            "the rook left a1"
        );
        assert!(rights.has(Color::White, Side::Kingside));
    }

    #[test]
    fn a_rook_away_from_the_corner_still_loses_its_right() {
        // The whole point. A Chess960 rook on b1 holds the queenside right, and
        // the old square table would have kept that right alive for ever.
        let rights = CastlingRights::NONE.with(Color::White, Side::Queenside, 1);
        assert!(rights.after_move(Color::White, false, B1, D4).is_empty());
        assert!(
            rights.after_move(Color::Black, false, D4, B1).is_empty(),
            "captured"
        );
    }

    #[test]
    fn a_promoted_rook_arriving_on_a_corner_grants_nothing() {
        // A right names a square whose original rook is gone. Nothing that
        // later stands there can revive it.
        let rights = CastlingRights::NONE;
        assert!(rights.after_move(Color::White, false, H7, H8).is_empty());
    }

    #[test]
    fn a_king_move_ends_both_of_its_rights_wherever_it_stood() {
        // Asked of the role, not of e1 — a Chess960 king starts anywhere.
        let rights = CastlingRights::STANDARD.after_move(Color::White, true, B1, C1);
        assert!(!rights.has(Color::White, Side::Kingside));
        assert!(!rights.has(Color::White, Side::Queenside));
        assert!(rights.has(Color::Black, Side::Kingside));
    }

    #[test]
    fn castling_destinations_are_the_part_that_never_moves() {
        // Everything else about castling varies with the arrangement; these
        // four squares per colour do not, in any of the 960.
        for color in Color::ALL {
            for side in Side::ALL {
                let king_to = side.king_destination(color);
                let rook_to = side.rook_destination(color);
                assert_eq!(king_to.rank(), color.back_rank());
                assert_eq!(rook_to.rank(), color.back_rank());
                assert_eq!(
                    king_to.file(),
                    match side {
                        Side::Kingside => 6,
                        Side::Queenside => 2,
                    }
                );
                assert_eq!(
                    rook_to.file(),
                    match side {
                        Side::Kingside => 5,
                        Side::Queenside => 3,
                    }
                );
            }
        }
    }
}
