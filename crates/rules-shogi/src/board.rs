//! The board: eighty-one squares, and which way each side is facing.
//!
//! Nine by nine, so eighty-one squares fit in a `u128` with plenty to spare —
//! the same reason the tafl and xiangqi engines use one.
//!
//! **Indexed from Black's bottom-left**, file 1 to 9 and rank 1 to 9, with
//! index 0 the square Western notation calls 1a. Shogi's own notation writes a
//! square as file-then-rank with the file counted from the *right* — 7g is the
//! square in front of Black's left-hand silver — and this crate speaks that
//! notation at its edges and nothing but indices inside. The conversion lives
//! in one place, [`Square::name`], for the same reason every other engine here
//! keeps notation at the boundary: a coordinate system that leaks is a
//! coordinate system that gets used inconsistently.
//!
//! Black moves first and moves *up* the board. That is the opposite of what
//! "black" means everywhere else in Bragd, and it is shogi's own naming —
//! see [`crate::piece::Side`].

use core::fmt;

pub const FILES: u8 = 9;
pub const RANKS: u8 = 9;
pub const SQUARES: usize = (FILES as usize) * (RANKS as usize);

/// A square, indexed from 0.
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

    /// `file` and `rank` are zero-based **from the left and the bottom**, which
    /// is not how shogi writes them. See [`Square::name`].
    pub const fn from_file_rank(file: i16, rank: i16) -> Option<Square> {
        if file < 0 || rank < 0 || file >= FILES as i16 || rank >= RANKS as i16 {
            return None;
        }
        Some(Square(rank as u8 * FILES + file as u8))
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

    /// The square `(df, dr)` away, or `None` if that leaves the board.
    ///
    /// Checked against the file rather than the index, because a rank wraps:
    /// one step right from the rightmost file is the leftmost file of the next
    /// rank if you only add one.
    pub const fn offset(self, df: i16, dr: i16) -> Option<Square> {
        Square::from_file_rank(self.file() as i16 + df, self.rank() as i16 + dr)
    }

    /// The square as shogi writes it: `7g`.
    ///
    /// **Files are counted from the right**, so internal file 0 is written `9`
    /// and internal file 8 is written `1`. Ranks are lettered from the top, so
    /// internal rank 8 is `a` and internal rank 0 is `i`. Both inversions are
    /// here and nowhere else.
    pub fn name(self) -> String {
        let file = FILES - self.file();
        let rank = (b'a' + (RANKS - 1 - self.rank())) as char;
        format!("{file}{rank}")
    }

    /// Reads `7g`. The inverse of [`Square::name`], and the only other place
    /// that knows the notation is inverted.
    pub fn parse(text: &str) -> Option<Square> {
        let mut characters = text.chars();
        let file = characters.next()?.to_digit(10)? as u8;
        let rank = characters.next()?;
        if characters.next().is_some() || file == 0 || file > FILES {
            return None;
        }
        if !rank.is_ascii_lowercase() {
            return None;
        }
        let rank = rank as u8 - b'a';
        if rank >= RANKS {
            return None;
        }
        Square::from_file_rank((FILES - file) as i16, (RANKS - 1 - rank) as i16)
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

impl fmt::Debug for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

/// A set of squares, one machine word.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Bitboard(pub u128);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    /// Every square, and nothing above it.
    pub const BOARD: Bitboard = Bitboard((1u128 << SQUARES) - 1);

    pub const fn from_square(square: Square) -> Bitboard {
        Bitboard(1u128 << square.index())
    }

    pub const fn contains(self, square: Square) -> bool {
        self.0 & (1u128 << square.index()) != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn with(self, square: Square) -> Bitboard {
        Bitboard(self.0 | (1u128 << square.index()))
    }

    pub const fn without(self, square: Square) -> Bitboard {
        Bitboard(self.0 & !(1u128 << square.index()))
    }

    pub const fn first(self) -> Option<Square> {
        if self.is_empty() {
            None
        } else {
            Square::new(self.0.trailing_zeros() as u8)
        }
    }

    /// Complemented **within the board**, so the bits above square 80 stay
    /// clear. An operator would be tidier and would hide the mask, which is
    /// the bug: forty-seven stray bits iterate as squares that do not exist.
    pub const fn complement(self) -> Bitboard {
        Bitboard(!self.0 & Bitboard::BOARD.0)
    }

    /// Every square on `file`, zero-based from the left. Used by the rule that
    /// forbids two unpromoted pawns on one file, which is the only rule in
    /// shogi that asks about a whole file at once.
    pub const fn file(file: u8) -> Bitboard {
        let mut mask = 0u128;
        let mut rank = 0;
        while rank < RANKS {
            mask |= 1u128 << (rank * FILES + file);
            rank += 1;
        }
        Bitboard(mask)
    }
}

impl core::ops::BitOr for Bitboard {
    type Output = Bitboard;
    fn bitor(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 | other.0)
    }
}

impl core::ops::BitAnd for Bitboard {
    type Output = Bitboard;
    fn bitand(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 & other.0)
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

impl Iterator for Bitboard {
    type Item = Square;
    fn next(&mut self) -> Option<Square> {
        let square = self.first()?;
        self.0 &= self.0 - 1;
        Some(square)
    }
}

impl fmt::Debug for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Printed the way the board looks. A bitboard bug is almost always a
        // geometry bug, and reading it as a number will not show you.
        for rank in (0..RANKS).rev() {
            for file in 0..FILES {
                let square =
                    Square::from_file_rank(file as i16, rank as i16).expect("on the board");
                f.write_str(if self.contains(square) { "x" } else { "." })?;
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
    fn the_notation_is_inverted_in_both_directions() {
        // The two corners and the centre, which is the cheapest way to catch a
        // flipped axis: getting one wrong moves everything.
        assert_eq!(Square::at(0).name(), "9i");
        assert_eq!(Square::at(8).name(), "1i");
        assert_eq!(Square::at(80).name(), "1a");
        assert_eq!(Square::at(72).name(), "9a");
        assert_eq!(Square::from_file_rank(4, 4).unwrap().name(), "5e");
    }

    #[test]
    fn every_square_round_trips_through_its_name() {
        for index in 0..SQUARES as u8 {
            let square = Square::at(index);
            assert_eq!(Square::parse(&square.name()), Some(square), "{square}");
        }
    }

    #[test]
    fn refuses_what_is_not_a_square() {
        for text in ["", "0a", "10a", "1j", "1A", "1aa", "a1", "5"] {
            assert_eq!(Square::parse(text), None, "parsed {text:?}");
        }
    }

    #[test]
    fn a_rank_does_not_wrap() {
        let right_edge = Square::parse("1e").unwrap();
        assert_eq!(right_edge.offset(1, 0), None);
        let left_edge = Square::parse("9e").unwrap();
        assert_eq!(left_edge.offset(-1, 0), None);
    }

    #[test]
    fn a_bitboard_holds_the_board_and_nothing_above_it() {
        assert_eq!(Bitboard::BOARD.count(), SQUARES as u32);
        assert_eq!(Bitboard::EMPTY.complement().count(), SQUARES as u32);
        assert_eq!(Bitboard::BOARD.complement(), Bitboard::EMPTY);
    }

    #[test]
    fn a_file_is_nine_squares_of_one_file() {
        for file in 0..FILES {
            let mask = Bitboard::file(file);
            assert_eq!(mask.count(), RANKS as u32);
            for square in mask {
                assert_eq!(square.file(), file);
            }
        }
    }
}
