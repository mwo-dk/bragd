//! The board: ninety points, a river and two palaces.
//!
//! **Pieces stand on intersections, not in cells.** That is a real difference
//! from every other engine here and not only a way of drawing it: the board is
//! nine lines by ten, so there are ninety points where chess has sixty-four
//! squares, and the river is the gap *between* two ranks rather than a rank of
//! its own.
//!
//! Ninety points fit in a `u128` with room to spare. Files run a–i left to
//! right from Red's side and ranks 1–10 bottom to top, with rank 1 Red's back
//! line — the convention the published perft positions use.
//!
//! **The board is shared with janggi**, which is why the two games live in one
//! crate.
//! Everything here that is a rule rather than a shape is therefore optional to
//! the caller: janggi has no river, so [`Point::is_own_half`] is simply never
//! asked; janggi draws the palace diagonals as real lines, so
//! [`Point::palace_diagonal`] exists and xiangqi never asks that one. Neither
//! game pays for the other's rule, because a rule nobody asks about costs
//! nothing.

use core::fmt;

pub const FILES: u8 = 9;
pub const RANKS: u8 = 10;
pub const POINTS: usize = (FILES as usize) * (RANKS as usize);

/// A point on the board, indexed from a1.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point(u8);

impl Point {
    /// # Panics
    /// If the index is off the board.
    pub const fn at(index: u8) -> Point {
        assert!((index as usize) < POINTS, "not a point on this board");
        Point(index)
    }

    pub const fn new(index: u8) -> Option<Point> {
        if (index as usize) < POINTS {
            Some(Point(index))
        } else {
            None
        }
    }

    pub const fn from_file_rank(file: i16, rank: i16) -> Option<Point> {
        if file < 0 || rank < 0 || file >= FILES as i16 || rank >= RANKS as i16 {
            return None;
        }
        Some(Point(rank as u8 * FILES + file as u8))
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

    /// The point `(df, dr)` away, or `None` if that leaves the board.
    ///
    /// Checked against the file rather than the index, because a rank wraps:
    /// one step east from i5 is a6 if you only add one.
    pub const fn offset(self, df: i16, dr: i16) -> Option<Point> {
        Point::from_file_rank(self.file() as i16 + df, self.rank() as i16 + dr)
    }

    /// Whether this point is on `side`'s half of the river.
    ///
    /// Ranks 1–5 are Red's, 6–10 are Black's. The river is the boundary between
    /// them and occupies no points at all.
    pub const fn is_own_half(self, red: bool) -> bool {
        if red {
            self.rank() < 5
        } else {
            self.rank() >= 5
        }
    }

    /// Whether this point is inside `side`'s palace: files d–f, and the three
    /// ranks nearest that side.
    pub const fn is_palace(self, red: bool) -> bool {
        let file = self.file();
        let rank = self.rank();
        if file < 3 || file > 5 {
            return false;
        }
        if red { rank < 3 } else { rank >= RANKS - 3 }
    }

    /// Whether this is the middle point of a palace: e2 or e9.
    ///
    /// It has a name because every diagonal line drawn on a palace runs from a
    /// corner *through* it — there is no diagonal between two corners that
    /// misses it — which is the whole of the geometry [`Point::palace_diagonal`]
    /// needs.
    pub const fn is_palace_centre(self) -> bool {
        self.file() == 4 && (self.rank() == 1 || self.rank() == RANKS - 2)
    }

    /// One diagonal step **along a line drawn on the palace**, or `None` if
    /// there is no such line.
    ///
    /// Only janggi asks. In xiangqi the palace's diagonals are decoration: the
    /// advisor's move happens to trace them and nothing else may use them, so
    /// the code that generates advisor moves needs no notion of a line at all.
    /// In janggi they are real lines, and *any* piece standing on one may
    /// travel along it — the general, the guards, a chariot, a cannon, even a
    /// soldier. That is why this is geometry rather than part of one piece's
    /// move.
    ///
    /// The test is not "both ends are in a palace". `d2`–`e3` has both ends
    /// inside Red's palace and is not a line anybody drew; taking that reading
    /// would give a chariot on d2 a diagonal move it does not have. What makes
    /// a step a line is that one of its two ends is the palace's centre.
    pub fn palace_diagonal(self, df: i16, dr: i16) -> Option<Point> {
        if df == 0 || dr == 0 {
            return None;
        }
        let to = self.offset(df, dr)?;
        let same_palace = (self.is_palace(true) && to.is_palace(true))
            || (self.is_palace(false) && to.is_palace(false));
        if same_palace && (self.is_palace_centre() || to.is_palace_centre()) {
            Some(to)
        } else {
            None
        }
    }

    pub fn parse(text: &str) -> Option<Point> {
        let mut chars = text.chars();
        let file = chars.next()?.to_ascii_lowercase();
        if !('a'..='i').contains(&file) {
            return None;
        }
        let rank: u8 = chars.as_str().parse().ok()?;
        if rank == 0 {
            return None;
        }
        Point::from_file_rank((file as u8 - b'a') as i16, (rank - 1) as i16)
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", (b'a' + self.file()) as char, self.rank() + 1)
    }
}

impl fmt::Debug for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

/// A set of points, one bit each.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Bitboard(pub u128);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const BOARD: Bitboard = Bitboard((1u128 << POINTS) - 1);

    pub const fn contains(self, point: Point) -> bool {
        self.0 & (1u128 << point.0) != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn with(self, point: Point) -> Bitboard {
        Bitboard(self.0 | (1u128 << point.0))
    }

    pub const fn without(self, point: Point) -> Bitboard {
        Bitboard(self.0 & !(1u128 << point.0))
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

    pub const fn first(self) -> Option<Point> {
        if self.is_empty() {
            None
        } else {
            Point::new(self.0.trailing_zeros() as u8)
        }
    }
}

impl Iterator for Bitboard {
    type Item = Point;

    fn next(&mut self) -> Option<Point> {
        let point = self.first()?;
        self.0 &= self.0 - 1;
        Some(point)
    }
}

impl fmt::Debug for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..RANKS).rev() {
            for file in 0..FILES {
                let point = Point::from_file_rank(file as i16, rank as i16).expect("on the board");
                f.write_str(if self.contains(point) { "x" } else { "." })?;
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
    fn the_board_is_nine_by_ten() {
        assert_eq!(POINTS, 90);
        assert_eq!(Bitboard::BOARD.count(), 90);
        assert_eq!(Point::at(0).to_string(), "a1");
        assert_eq!(Point::at(89).to_string(), "i10");
    }

    #[test]
    fn notation_round_trips_for_every_point() {
        for index in 0..POINTS as u8 {
            let point = Point::at(index);
            assert_eq!(Point::parse(&point.to_string()), Some(point));
        }
        for nonsense in ["", "a", "a0", "a11", "j1", "z9", "1a", "🙂"] {
            assert_eq!(Point::parse(nonsense), None, "parsed {nonsense:?}");
        }
    }

    #[test]
    fn a_rank_does_not_wrap() {
        // One step east from the right-hand file is off the board, not the
        // start of the next rank.
        let edge = Point::parse("i5").unwrap();
        assert_eq!(edge.offset(1, 0), None);
        assert_eq!(edge.offset(-1, 0), Point::parse("h5"));
    }

    #[test]
    fn the_river_is_a_boundary_and_not_a_rank() {
        // It occupies no points: ranks 5 and 6 are both playable and adjacent.
        assert!(Point::parse("a5").unwrap().is_own_half(true));
        assert!(!Point::parse("a6").unwrap().is_own_half(true));
        assert!(Point::parse("a6").unwrap().is_own_half(false));
        assert_eq!(
            Point::parse("a5").unwrap().offset(0, 1),
            Point::parse("a6"),
            "one step crosses it"
        );
    }

    #[test]
    fn a_palace_is_nine_points_at_each_end() {
        let red: Vec<Point> = (0..POINTS as u8)
            .map(Point::at)
            .filter(|point| point.is_palace(true))
            .collect();
        assert_eq!(red.len(), 9);
        assert!(red.contains(&Point::parse("d1").unwrap()));
        assert!(red.contains(&Point::parse("e2").unwrap()));
        assert!(red.contains(&Point::parse("f3").unwrap()));
        assert!(!red.contains(&Point::parse("e4").unwrap()));
        assert!(!red.contains(&Point::parse("c1").unwrap()));

        let black: Vec<Point> = (0..POINTS as u8)
            .map(Point::at)
            .filter(|point| point.is_palace(false))
            .collect();
        assert_eq!(black.len(), 9);
        assert!(black.contains(&Point::parse("e10").unwrap()));
        assert!(black.contains(&Point::parse("e9").unwrap()));
    }

    #[test]
    fn a_palace_diagonal_is_a_line_and_not_merely_a_pair_of_palace_points() {
        let at = |text: &str| Point::parse(text).unwrap();

        // The four lines of Red's palace, each running corner to centre.
        assert_eq!(at("d1").palace_diagonal(1, 1), Some(at("e2")));
        assert_eq!(at("f1").palace_diagonal(-1, 1), Some(at("e2")));
        assert_eq!(at("e2").palace_diagonal(1, 1), Some(at("f3")));
        assert_eq!(at("e2").palace_diagonal(-1, -1), Some(at("d1")));

        // d2 and e3 are both inside the palace and there is no line between
        // them. A reading that only asked "are both ends in a palace" would
        // give a chariot on d2 a move it does not have.
        assert_eq!(at("d2").palace_diagonal(1, 1), None);
        assert_eq!(at("e3").palace_diagonal(-1, -1), None);

        // A line stops at the palace edge rather than continuing across the
        // board.
        assert_eq!(at("f3").palace_diagonal(1, 1), None);
        assert_eq!(at("c1").palace_diagonal(1, 1), None);

        // Black's palace has the same four, at the other end.
        assert_eq!(at("d10").palace_diagonal(1, -1), Some(at("e9")));
        assert_eq!(at("e9").palace_diagonal(1, -1), Some(at("f8")));
        assert_eq!(at("e8").palace_diagonal(1, 1), None, "e8 is not the centre");
    }

    #[test]
    fn each_palace_has_exactly_one_centre() {
        let centres: Vec<Point> = (0..POINTS as u8)
            .map(Point::at)
            .filter(|point| point.is_palace_centre())
            .collect();
        assert_eq!(
            centres,
            vec![Point::parse("e2").unwrap(), Point::parse("e9").unwrap()]
        );
    }
}
