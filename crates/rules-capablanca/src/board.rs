//! The board: ten files by eight ranks, and why that needs its own engine.
//!
//! **Eighty squares do not fit in a `u64`.** That single fact is the reason
//! Capablanca chess is a crate rather than a variant in `crates/rules-chess`: every
//! bitboard there is a `u64` holding exactly sixty-four squares, and widening
//! it would rewrite the type that carries the published-perft gate for chess
//! and Chess960. Adding two pieces to that engine was cheap; adding
//! two *files* is not the same kind of change at all.
//!
//! Eighty squares fit in a `u128` with room to spare, which is the answer
//! `crates/rules-xiangqi` already reached for its ninety points. This module follows
//! it deliberately rather than inventing a second approach.
//!
//! Files run a–j left to right from White's side and ranks 1–8 bottom to top,
//! with rank 1 White's back line — the same convention as chess, extended.

use core::fmt;

pub const FILES: u8 = 10;
pub const RANKS: u8 = 8;
pub const SQUARES: usize = (FILES as usize) * (RANKS as usize);

/// A square, indexed from a1.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Square(u8);

impl Square {
    /// # Panics
    /// If the index is off the board.
    pub const fn at(index: u8) -> Square {
        assert!((index as usize) < SQUARES, "not a square on this board");
        Square(index)
    }

    pub const fn new(index: u8) -> Option<Square> {
        if (index as usize) < SQUARES {
            Some(Square(index))
        } else {
            None
        }
    }

    /// Signed parameters so that arithmetic which walks off an edge answers
    /// `None` rather than wrapping into the opposite side of the board — the
    /// bug every leaper table has to avoid.
    pub const fn from_file_rank(file: i16, rank: i16) -> Option<Square> {
        if file < 0 || rank < 0 || file >= FILES as i16 || rank >= RANKS as i16 {
            return None;
        }
        Some(Square((rank * FILES as i16 + file) as u8))
    }

    pub const fn index(self) -> u8 {
        self.0
    }

    pub const fn file(self) -> u8 {
        self.0 % FILES
    }

    pub const fn rank(self) -> u8 {
        self.0 / FILES
    }
}

impl fmt::Display for Square {
    /// `a1` through `j8`. Ten files means the letters run past `h`, which is
    /// the first place a reader notices this is not a chessboard.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            (b'a' + self.file()) as char,
            (b'1' + self.rank()) as char
        )
    }
}

impl fmt::Debug for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

/// A set of squares, one bit each.
///
/// `u128` rather than `u64`, with forty-eight bits spare. The spare bits are
/// never set, and [`Bitboard::BOARD`] is the mask that says so.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Bitboard(pub u128);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    /// Every square on the board, and nothing above it.
    pub const BOARD: Bitboard = Bitboard((1u128 << SQUARES) - 1);

    pub const fn from_square(square: Square) -> Bitboard {
        Bitboard(1u128 << square.0)
    }

    pub const fn contains(self, square: Square) -> bool {
        self.0 & (1u128 << square.0) != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn with(self, square: Square) -> Bitboard {
        Bitboard(self.0 | (1u128 << square.0))
    }

    pub const fn without(self, square: Square) -> Bitboard {
        Bitboard(self.0 & !(1u128 << square.0))
    }

    /// The only square in this set, or `None` if there is not exactly one.
    pub fn single(self) -> Option<Square> {
        if self.count() == 1 {
            Square::new(self.0.trailing_zeros() as u8)
        } else {
            None
        }
    }
}

impl Iterator for Bitboard {
    type Item = Square;

    fn next(&mut self) -> Option<Square> {
        if self.0 == 0 {
            return None;
        }
        let index = self.0.trailing_zeros() as u8;
        self.0 &= self.0 - 1;
        Square::new(index)
    }
}

impl core::ops::BitAnd for Bitboard {
    type Output = Bitboard;
    fn bitand(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 & other.0)
    }
}

impl core::ops::BitOr for Bitboard {
    type Output = Bitboard;
    fn bitor(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 | other.0)
    }
}

impl core::ops::BitOrAssign for Bitboard {
    fn bitor_assign(&mut self, other: Bitboard) {
        self.0 |= other.0;
    }
}

impl core::ops::BitAndAssign for Bitboard {
    fn bitand_assign(&mut self, other: Bitboard) {
        self.0 &= other.0;
    }
}

impl core::ops::Not for Bitboard {
    type Output = Bitboard;
    /// Complement **within the board**, so the forty-eight unused bits stay
    /// clear. A plain `!` would set them and every later intersection would
    /// have to remember to mask.
    fn not(self) -> Bitboard {
        Bitboard(!self.0 & Bitboard::BOARD.0)
    }
}

impl fmt::Debug for Bitboard {
    /// Rank 8 first, the way a board is drawn.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..RANKS).rev() {
            for file in 0..FILES {
                let square =
                    Square::from_file_rank(file as i16, rank as i16).expect("on the board");
                write!(f, "{}", if self.contains(square) { '#' } else { '.' })?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eighty_squares_named_the_way_a_player_would() {
        assert_eq!(SQUARES, 80);
        assert_eq!(Square::at(0).to_string(), "a1");
        assert_eq!(Square::at(9).to_string(), "j1");
        assert_eq!(Square::at(70).to_string(), "a8");
        assert_eq!(Square::at(79).to_string(), "j8");
    }

    /// The bug this forbids is the one every wider board invites: stepping off
    /// the j-file and arriving on the a-file of the next rank.
    #[test]
    fn walking_off_an_edge_is_not_a_square() {
        assert_eq!(Square::from_file_rank(10, 0), None, "past the j-file");
        assert_eq!(Square::from_file_rank(-1, 0), None, "before the a-file");
        assert_eq!(Square::from_file_rank(0, 8), None, "above rank 8");
        assert_eq!(Square::from_file_rank(0, -1), None, "below rank 1");
    }

    #[test]
    fn the_board_mask_is_exactly_the_board() {
        assert_eq!(Bitboard::BOARD.count(), 80);
        // The forty-eight spare bits must never be set, or every complement
        // would carry squares that do not exist.
        assert_eq!(!Bitboard::EMPTY, Bitboard::BOARD);
        assert_eq!(!Bitboard::BOARD, Bitboard::EMPTY);
    }

    #[test]
    fn a_bitboard_iterates_the_squares_it_holds() {
        let set = Bitboard::EMPTY
            .with(Square::at(0))
            .with(Square::at(9))
            .with(Square::at(79));
        let squares: Vec<String> = set.map(|s| s.to_string()).collect();
        assert_eq!(squares, ["a1", "j1", "j8"]);
    }

    #[test]
    fn every_square_round_trips_through_file_and_rank() {
        for index in 0..SQUARES as u8 {
            let square = Square::at(index);
            assert_eq!(
                Square::from_file_rank(square.file() as i16, square.rank() as i16),
                Some(square)
            );
        }
    }
}
