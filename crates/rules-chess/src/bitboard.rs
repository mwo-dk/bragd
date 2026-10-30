//! A set of squares, held as one machine word.

use crate::square::Square;
use core::fmt;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

/// A set of squares. Bit *n* set means square *n* is in the set.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const FULL: Bitboard = Bitboard(!0);

    pub const fn from_square(square: Square) -> Self {
        Bitboard(1u64 << square.index())
    }

    pub const fn contains(self, square: Square) -> bool {
        self.0 & (1u64 << square.index()) != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn with(self, square: Square) -> Self {
        Bitboard(self.0 | (1u64 << square.index()))
    }

    pub const fn without(self, square: Square) -> Self {
        Bitboard(self.0 & !(1u64 << square.index()))
    }

    /// The lowest square in the set, or `None` if empty.
    pub const fn first(self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            Some(Square::new(self.0.trailing_zeros() as u8))
        }
    }

    /// The set's only square.
    ///
    /// # Panics
    /// If the set does not hold exactly one square. Used where the invariant is
    /// structural — there is exactly one king per side — so a panic here means
    /// the position itself is corrupt, which we would rather learn loudly.
    pub fn single(self) -> Square {
        debug_assert_eq!(self.count(), 1, "expected exactly one square");
        Square::new(self.0.trailing_zeros() as u8)
    }

    /// Shifts every square by `delta` in index order, discarding what falls off
    /// the ends. File wrap-around is *not* handled — mask the file first.
    pub const fn shift(self, delta: i8) -> Self {
        if delta >= 0 {
            Bitboard(self.0 << delta)
        } else {
            Bitboard(self.0 >> -delta)
        }
    }
}

/// File masks, a to h.
pub const FILES: [Bitboard; 8] = {
    let mut files = [Bitboard::EMPTY; 8];
    let mut file = 0;
    while file < 8 {
        files[file] = Bitboard(0x0101_0101_0101_0101u64 << file);
        file += 1;
    }
    files
};

/// Rank masks, 1 to 8.
pub const RANKS: [Bitboard; 8] = {
    let mut ranks = [Bitboard::EMPTY; 8];
    let mut rank = 0;
    while rank < 8 {
        ranks[rank] = Bitboard(0xffu64 << (rank * 8));
        rank += 1;
    }
    ranks
};

impl IntoIterator for Bitboard {
    type Item = Square;
    type IntoIter = Squares;

    fn into_iter(self) -> Squares {
        Squares(self.0)
    }
}

/// Iterates the squares of a set, lowest index first.
pub struct Squares(u64);

impl Iterator for Squares {
    type Item = Square;

    fn next(&mut self) -> Option<Square> {
        if self.0 == 0 {
            return None;
        }
        let square = Square::new(self.0.trailing_zeros() as u8);
        self.0 &= self.0 - 1;
        Some(square)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.0.count_ones() as usize;
        (count, Some(count))
    }
}

impl ExactSizeIterator for Squares {}

macro_rules! binary_op {
    ($trait:ident, $method:ident, $assign_trait:ident, $assign_method:ident, $op:tt) => {
        impl $trait for Bitboard {
            type Output = Bitboard;
            fn $method(self, rhs: Bitboard) -> Bitboard {
                Bitboard(self.0 $op rhs.0)
            }
        }
        impl $assign_trait for Bitboard {
            fn $assign_method(&mut self, rhs: Bitboard) {
                self.0 = self.0 $op rhs.0;
            }
        }
    };
}

binary_op!(BitAnd, bitand, BitAndAssign, bitand_assign, &);
binary_op!(BitOr, bitor, BitOrAssign, bitor_assign, |);
binary_op!(BitXor, bitxor, BitXorAssign, bitxor_assign, ^);

impl Not for Bitboard {
    type Output = Bitboard;
    fn not(self) -> Bitboard {
        Bitboard(!self.0)
    }
}

impl fmt::Debug for Bitboard {
    /// Prints as a board, rank 8 at the top, so a wrong mask is obvious at a
    /// glance instead of being a hex literal to decode by hand.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        for rank in (0..8).rev() {
            for file in 0..8 {
                let occupied = self.contains(Square::from_file_rank(file, rank));
                write!(f, "{} ", if occupied { '1' } else { '.' })?;
            }
            writeln!(f, " {}", rank + 1)?;
        }
        writeln!(f, "a b c d e f g h")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::square::squares::*;

    #[test]
    fn iterates_in_index_order() {
        let set = Bitboard::EMPTY.with(H8).with(A1).with(E1);
        assert_eq!(set.into_iter().collect::<Vec<_>>(), vec![A1, E1, H8]);
    }

    #[test]
    fn files_and_ranks_do_not_overlap_more_than_once() {
        for file in &FILES {
            for rank in &RANKS {
                assert_eq!((*file & *rank).count(), 1);
            }
        }
        assert_eq!(FILES[0].count(), 8);
        assert_eq!(RANKS[7], Bitboard(0xff00_0000_0000_0000));
    }

    #[test]
    fn empty_set_has_no_first_square() {
        assert_eq!(Bitboard::EMPTY.first(), None);
        assert_eq!(Bitboard::FULL.first(), Some(A1));
    }
}
