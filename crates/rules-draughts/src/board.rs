//! The board: dark squares only, and the geometry over them.
//!
//! **Only half the squares exist.** Every diagonal variant is played on one
//! colour complex, so a 10×10 board has fifty playable squares and an 8×8 has
//! thirty-two. Both fit in a `u64`, which is why a set of squares is one
//! machine word here as it is in the other engines.
//!
//! The board size is carried at runtime rather than fixed, because unlike tafl
//! this family genuinely needs two: International is 10×10 and every 8×8
//! variant — English, Russian, Brazilian, Italian — shares the smaller board.
//! One geometry with a size parameter beats two modules that drift.
//!
//! Squares are indexed **rank-major from rank 1**, so index 0 is the dark
//! square at the bottom left. That is not the numbering printed on a draughts
//! board, which counts from the other player's side; the printed numbering is a
//! notation concern and is converted at the boundary rather than being built in
//! here.

use core::fmt;

/// A set of squares. Bit *n* is playable square *n*.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);

    pub const fn from_square(square: Square) -> Bitboard {
        Bitboard(1u64 << square.0)
    }

    pub const fn contains(self, square: Square) -> bool {
        self.0 & (1u64 << square.0) != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn with(self, square: Square) -> Bitboard {
        Bitboard(self.0 | (1u64 << square.0))
    }

    pub const fn without(self, square: Square) -> Bitboard {
        Bitboard(self.0 & !(1u64 << square.0))
    }

    pub const fn union(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 | other.0)
    }

    pub const fn intersection(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 & other.0)
    }

    pub const fn difference(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 & !other.0)
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
        Some(Square(index))
    }
}

/// A playable square, indexed from the dark square at the bottom left.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Square(u8);

impl Square {
    pub const fn new(index: u8) -> Square {
        Square(index)
    }

    pub const fn index(self) -> u8 {
        self.0
    }
}

/// The four diagonals a piece can travel along.
///
/// Four, and only four: there is no orthogonal movement in any variant this
/// engine plays. (Turkish draughts is orthogonal, which is exactly why it is
/// not in this crate.)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Direction {
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::NorthEast,
        Direction::NorthWest,
        Direction::SouthEast,
        Direction::SouthWest,
    ];

    pub const fn delta(self) -> (i8, i8) {
        match self {
            Direction::NorthEast => (1, 1),
            Direction::NorthWest => (-1, 1),
            Direction::SouthEast => (1, -1),
            Direction::SouthWest => (-1, -1),
        }
    }

    /// Whether this direction goes towards the far rank for a side moving up.
    pub const fn is_forward(self, upward: bool) -> bool {
        match self {
            Direction::NorthEast | Direction::NorthWest => upward,
            Direction::SouthEast | Direction::SouthWest => !upward,
        }
    }
}

/// The geometry of one board size.
///
/// Held as a value rather than as constants because this family needs two
/// sizes, and a function that takes the board is harder to call with the wrong
/// one than a global is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Geometry {
    /// Squares along one edge: 10 for International, 8 for the rest.
    pub side: u8,
}

impl Geometry {
    pub const INTERNATIONAL: Geometry = Geometry { side: 10 };
    pub const SMALL: Geometry = Geometry { side: 8 };

    /// How many squares are playable. Half of the board, always.
    pub const fn squares(self) -> usize {
        (self.side as usize * self.side as usize) / 2
    }

    /// Every playable square.
    pub fn all(self) -> Bitboard {
        Bitboard((1u128 << self.squares()) as u64 - 1)
    }

    /// The file and rank of a square, both counted from zero at the bottom left.
    ///
    /// Five dark squares to a rank on a 10×10 board, four on an 8×8, and which
    /// files they are alternates — so the file is recovered from the index
    /// within the rank *and* the rank's parity. Getting that parity wrong
    /// produces a board that looks right and whose diagonals are broken.
    pub const fn file_rank(self, square: Square) -> (u8, u8) {
        let per_rank = self.side / 2;
        let rank = square.0 / per_rank;
        let offset = square.0 % per_rank;
        // Rank 0 has dark squares on the even files; rank 1 on the odd ones.
        let file = offset * 2 + (rank % 2);
        (file, rank)
    }

    /// The square at a file and rank, or `None` if it is off the board or light.
    pub const fn at(self, file: i16, rank: i16) -> Option<Square> {
        if file < 0 || rank < 0 || file >= self.side as i16 || rank >= self.side as i16 {
            return None;
        }
        // A light square is not a mistake to be corrected — it is not a square
        // at all. Dark squares are those whose file and rank share a parity,
        // which is what puts a1 in play and b1 out of it.
        if (file as u8 % 2) != (rank as u8 % 2) {
            return None;
        }
        let per_rank = self.side / 2;
        Some(Square(rank as u8 * per_rank + file as u8 / 2))
    }

    /// The square's number, as it is printed on a real board and written in
    /// draughts notation — `1` to `50` on International, `1` to `32` on the
    /// 8×8 variants.
    ///
    /// Numbered from the far side of the board, not this crate's own corner:
    /// the top-left dark square (Black's home row, from White's seat) is 1,
    /// counting left to right along each row and down the board, ending at
    /// the bottom-right (`squares()`, White's home row). That is the reverse
    /// of [`Square::index`], which starts at White's own bottom-left — so
    /// White's first man, on the internal board's square 0, is printed square
    /// `squares() - per_rank + 1`, not `1`.
    ///
    /// The FMJD numbering (International) and the standard English numbering
    /// (checkers) are the same pattern at two sizes, which is why one formula
    /// covers both rather than the ruleset carrying a table.
    pub const fn printed_number(self, square: Square) -> u16 {
        let per_rank = (self.side / 2) as u16;
        let internal_rank = (square.0 as u16) / per_rank;
        let offset = (square.0 as u16) % per_rank;
        let row_from_top = self.side as u16 - 1 - internal_rank;
        row_from_top * per_rank + offset + 1
    }

    /// The square `steps` away along a diagonal.
    pub fn step(self, square: Square, direction: Direction, steps: u8) -> Option<Square> {
        let (file, rank) = self.file_rank(square);
        let (df, dr) = direction.delta();
        self.at(
            file as i16 + df as i16 * steps as i16,
            rank as i16 + dr as i16 * steps as i16,
        )
    }

    /// Whether a square is on the far rank for a side moving upward.
    pub const fn is_promotion_rank(self, square: Square, upward: bool) -> bool {
        let (_, rank) = self.file_rank(square);
        if upward {
            rank == self.side - 1
        } else {
            rank == 0
        }
    }
}

impl fmt::Debug for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The index, because the printed numbering depends on the variant and
        // on which side is at the bottom. Notation is converted at the edge.
        write!(f, "#{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_board_is_half_squares() {
        assert_eq!(Geometry::INTERNATIONAL.squares(), 50);
        assert_eq!(Geometry::SMALL.squares(), 32);
        assert_eq!(Geometry::INTERNATIONAL.all().count(), 50);
        assert_eq!(Geometry::SMALL.all().count(), 32);
    }

    #[test]
    fn every_index_round_trips_through_its_coordinates() {
        // The place a parity error hides: a board that looks right and whose
        // diagonals are broken.
        for geometry in [Geometry::INTERNATIONAL, Geometry::SMALL] {
            for index in 0..geometry.squares() as u8 {
                let square = Square::new(index);
                let (file, rank) = geometry.file_rank(square);
                assert_eq!(
                    geometry.at(file as i16, rank as i16),
                    Some(square),
                    "index {index} on a {}x{} board",
                    geometry.side,
                    geometry.side
                );
                // And it really is a dark square.
                assert_eq!(file % 2, rank % 2, "index {index} is on a light square");
            }
        }
    }

    /// The two corners the FMJD rules name explicitly (Annex 1): the "single
    /// corners" are 5 and 46 on the 10×10 board. Checked against the internal
    /// squares that produce them, rather than reasoned about in the abstract —
    /// this is the one place a sign error in the formula would hide.
    #[test]
    fn the_printed_numbering_matches_the_named_corners() {
        let geometry = Geometry::INTERNATIONAL;
        // White's own bottom-left square (internal 0) is printed square 46:
        // White's home row is the *last* row a top-down printed count reaches.
        assert_eq!(geometry.printed_number(Square::new(0)), 46);
        // The internal top-right (49, White's promotion corner) is printed 5.
        assert_eq!(geometry.printed_number(Square::new(49)), 5);
        // And Black's own home square, internal 45, is printed 1 — the square
        // a printed board's numbering starts counting from.
        assert_eq!(geometry.printed_number(Square::new(45)), 1);
    }

    /// English checkers: Black holds printed 1–12, White 21–32, empty between.
    #[test]
    fn checkers_numbering_splits_the_board_the_way_the_rules_say() {
        let geometry = Geometry::SMALL;
        let printed = |index: u8| geometry.printed_number(Square::new(index));
        // White's first rank (internal 0..4) is printed 29..32.
        assert_eq!((printed(0), printed(3)), (29, 32));
        // Black's back rank (internal 28..31, the crowning rank) is printed 1..4.
        assert_eq!((printed(28), printed(31)), (1, 4));
    }

    /// Every printed number 1..=squares() is hit exactly once — the numbering
    /// is a bijection, not merely a formula that happens to look right on a
    /// couple of corners.
    #[test]
    fn every_square_gets_a_distinct_printed_number_in_range() {
        for geometry in [Geometry::INTERNATIONAL, Geometry::SMALL] {
            let mut seen = vec![false; geometry.squares()];
            for index in 0..geometry.squares() as u8 {
                let printed = geometry.printed_number(Square::new(index));
                assert!(
                    (1..=geometry.squares() as u16).contains(&printed),
                    "square {index} printed as {printed}, out of range"
                );
                let slot = &mut seen[printed as usize - 1];
                assert!(!*slot, "printed number {printed} used twice");
                *slot = true;
            }
        }
    }

    #[test]
    fn light_squares_are_not_squares() {
        let geometry = Geometry::INTERNATIONAL;
        assert_eq!(geometry.at(1, 0), None, "b1 is light");
        assert_eq!(geometry.at(0, 1), None, "a2 is light");
        assert!(geometry.at(0, 0).is_some(), "a1 is dark");
        assert!(geometry.at(1, 1).is_some(), "b2 is dark");
    }

    #[test]
    fn stepping_off_the_board_is_none_rather_than_a_wrap() {
        let geometry = Geometry::INTERNATIONAL;
        let corner = geometry.at(0, 0).unwrap();
        assert_eq!(geometry.step(corner, Direction::SouthWest, 1), None);
        assert_eq!(geometry.step(corner, Direction::SouthEast, 1), None);
        assert_eq!(
            geometry.step(corner, Direction::NorthEast, 1),
            geometry.at(1, 1)
        );
        // The classic wrap: one step east from the right-hand file.
        let edge = geometry.at(9, 1).unwrap();
        assert_eq!(geometry.step(edge, Direction::NorthEast, 1), None);
        assert_eq!(
            geometry.step(edge, Direction::NorthWest, 1),
            geometry.at(8, 2)
        );
    }

    #[test]
    fn the_promotion_rank_is_the_far_one() {
        let geometry = Geometry::INTERNATIONAL;
        // The far rank's dark squares are the odd files, because rank 9 is odd.
        assert!(geometry.is_promotion_rank(geometry.at(1, 9).unwrap(), true));
        assert!(!geometry.is_promotion_rank(geometry.at(1, 9).unwrap(), false));
        assert!(geometry.is_promotion_rank(geometry.at(0, 0).unwrap(), false));
    }
}
