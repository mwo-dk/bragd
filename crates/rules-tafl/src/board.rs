//! The board: its size, its squares, and sets of them.
//!
//! Tafl boards are odd-sided squares — 7×7, 9×9, 11×11 — because the king
//! starts in the exact centre and the game is about getting him out. This crate
//! plays all three: brandubh, tablut and hnefatafl.
//!
//! **The index space is fixed at eleven wide whatever the board is**, and that
//! is the whole trick that made three sizes affordable. A square is still
//! `rank * 11 + file`, so `file()` and `rank()` are still one instruction and a
//! set of squares is still one `u128` — 121 bits with seven to spare. A 7×7
//! board simply leaves files 7–10 and ranks 7–10 empty and unreachable.
//!
//! The alternative was a stride that varied with the board, which would have
//! made every square's arithmetic depend on a value carried at run time and
//! every stored index ambiguous about which board it belonged to. This way the
//! only things that need to know the size are the ones that genuinely depend on
//! where the *edge* is: whether a square is on it, whether a step falls off it,
//! and which squares are the corners.
//!
//! Nothing here knows a rule. What lives in this module is geometry.

use core::fmt;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign};

/// How far apart two squares on the same file are: the width of the index
/// space, **not** the width of the board.
///
/// Fixed at eleven for every size. See the module note for why.
pub const STRIDE: u8 = 11;

/// The largest board this crate plays, and the one the index space is sized
/// for.
pub const MAX_SIZE: u8 = 11;

/// How many indices there are. Not how many squares are in play on a small
/// board — that is `size * size`.
pub const SQUARES: usize = (STRIDE as usize) * (STRIDE as usize);

/// The three boards.
///
/// Brandubh is the Irish 7×7 game, tablut the Sámi 9×9 one that Linnaeus wrote
/// down in 1732, and hnefatafl the 11×11 game every modern ruleset is built on.
/// A size is only ever one of these three: the layouts, the throne and the
/// corners are all defined per size, so an arbitrary number would have nothing
/// to start from.
pub const SIZES: [u8; 3] = [7, 9, 11];

/// A square, indexed from the bottom left: 0 is a1, 120 is k11.
///
/// Files are lettered a–k **including i**. Chess skips it to avoid confusion
/// with the numeral one; tafl notation does not, and matching what tafl players
/// already write is worth more here than matching chess.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Square(u8);

impl Square {
    /// # Panics
    /// If `index` is not on the board. Callers taking an index from outside
    /// must use [`Square::new`].
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

    /// A square from its coordinates, in the **index space** — so this admits
    /// any file and rank below [`STRIDE`], whatever board is being played.
    /// Whether the square is on a particular board is
    /// [`Square::is_on`].
    pub const fn from_file_rank(file: u8, rank: u8) -> Option<Square> {
        if file < STRIDE && rank < STRIDE {
            Some(Square(rank * STRIDE + file))
        } else {
            None
        }
    }

    pub const fn index(self) -> u8 {
        self.0
    }

    pub const fn file(self) -> u8 {
        self.0 % STRIDE
    }

    pub const fn rank(self) -> u8 {
        self.0 / STRIDE
    }

    /// Whether this square exists on a board of `size`.
    ///
    /// The index space is eleven wide whatever the board is, so on a 7×7 board
    /// most indices name nothing. Anything taking a square from outside — a
    /// move off the wire, a position out of storage — has to ask.
    pub const fn is_on(self, size: u8) -> bool {
        self.file() < size && self.rank() < size
    }

    /// Whether this square is on an edge of the board.
    ///
    /// Worth a method because it is a *rule* in some rulesets — an edge escape
    /// — and because the encirclement test asks it constantly.
    pub const fn is_edge(self, size: u8) -> bool {
        let (file, rank) = (self.file(), self.rank());
        file == 0 || rank == 0 || file == size - 1 || rank == size - 1
    }

    /// The square `steps` away in a direction, or `None` if that leaves the
    /// board.
    ///
    /// Checked against the file as well as the index, because a rank wraps: one
    /// step east from k5 is a6 if you only add one.
    pub fn step(self, direction: Direction, steps: u8, size: u8) -> Option<Square> {
        let (df, dr) = direction.delta();
        let file = self.file() as i16 + df as i16 * steps as i16;
        let rank = self.rank() as i16 + dr as i16 * steps as i16;
        if !(0..size as i16).contains(&file) || !(0..size as i16).contains(&rank) {
            return None;
        }
        Square::from_file_rank(file as u8, rank as u8)
    }

    /// Reads `a1`-style notation. Files a–k, ranks 1–11.
    pub fn parse(text: &str) -> Option<Square> {
        let mut chars = text.chars();
        let file = chars.next()?.to_ascii_lowercase();
        if !file.is_ascii_lowercase() {
            return None;
        }
        let file = file as u8 - b'a';
        let rank: u8 = chars.as_str().parse().ok()?;
        if rank == 0 {
            return None;
        }
        Square::from_file_rank(file, rank - 1)
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", (b'a' + self.file()) as char, self.rank() + 1)
    }
}

impl fmt::Debug for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

/// The four directions a piece moves in.
///
/// Four rather than eight: **every piece in every tafl variant moves like a
/// rook.** There are no diagonals anywhere in the game — not in movement, not
/// in capture, not in the escape conditions. That is worth stating because it
/// is the single biggest structural difference from chess, and it makes a lot
/// of what follows simpler than its chess equivalent.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::North,
        Direction::South,
        Direction::East,
        Direction::West,
    ];

    /// The two axes, as pairs of opposed directions.
    ///
    /// Capture is by custody — a piece caught between two enemies — so the
    /// question is always asked of an axis rather than of a direction.
    pub const AXES: [(Direction, Direction); 2] = [
        (Direction::North, Direction::South),
        (Direction::East, Direction::West),
    ];

    pub const fn delta(self) -> (i8, i8) {
        match self {
            Direction::North => (0, 1),
            Direction::South => (0, -1),
            Direction::East => (1, 0),
            Direction::West => (-1, 0),
        }
    }

    pub const fn opposite(self) -> Direction {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East => Direction::West,
            Direction::West => Direction::East,
        }
    }
}

/// A set of squares, held as one machine word.
///
/// 121 squares in a `u128`, with the top seven bits unused. Every operation
/// masks against [`Bitboard::BOARD`] so a stray high bit cannot become a square
/// that does not exist.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Bitboard(pub u128);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);

    /// Every square on a board of `size`, and nothing else.
    ///
    /// **Not a contiguous run of bits**, unlike the fixed-size version this
    /// replaced: on a 7×7 board the live squares are seven runs of seven,
    /// eleven bits apart. Computed rather than tabled because it is asked for
    /// rarely — complementing a set is the only thing that needs it.
    pub const fn board(size: u8) -> Bitboard {
        let mut mask: u128 = 0;
        let mut rank = 0u8;
        while rank < size {
            let mut file = 0u8;
            while file < size {
                mask |= 1u128 << (rank * STRIDE + file);
                file += 1;
            }
            rank += 1;
        }
        Bitboard(mask)
    }

    /// This set's complement **within a board of `size`**, so no bit outside
    /// the board can become a square that does not exist.
    ///
    /// A method rather than [`Not`], because the answer depends on the board
    /// and an operator cannot take a second argument. That is a feature: `!set`
    /// on a 7×7 board would silently set seventy-two bits that are not squares,
    /// and anything iterating afterwards would walk them.
    pub const fn complement(self, size: u8) -> Bitboard {
        Bitboard(!self.0 & Bitboard::board(size).0)
    }

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
}

impl BitOr for Bitboard {
    type Output = Bitboard;
    fn bitor(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 | other.0)
    }
}

impl BitAnd for Bitboard {
    type Output = Bitboard;
    fn bitand(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 & other.0)
    }
}

impl BitXor for Bitboard {
    type Output = Bitboard;
    fn bitxor(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 ^ other.0)
    }
}

impl BitOrAssign for Bitboard {
    fn bitor_assign(&mut self, other: Bitboard) {
        self.0 |= other.0;
    }
}

impl BitAndAssign for Bitboard {
    fn bitand_assign(&mut self, other: Bitboard) {
        self.0 &= other.0;
    }
}

impl BitXorAssign for Bitboard {
    fn bitxor_assign(&mut self, other: Bitboard) {
        self.0 ^= other.0;
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
        // Printed the way the board looks, because a bitboard bug is almost
        // always a geometry bug and reading it as a number will not show you.
        // The whole index space, not one board: a bitboard does not know which
        // size it belongs to, and a debug print that hid bits outside the
        // current board would hide exactly the bug it exists to show.
        for rank in (0..STRIDE).rev() {
            for file in 0..STRIDE {
                let square = Square::from_file_rank(file, rank).expect("on the board");
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
    fn squares_index_from_the_bottom_left() {
        assert_eq!(Square::at(0).to_string(), "a1");
        assert_eq!(Square::at(120).to_string(), "k11");
        // The centre, where the king starts in every tafl variant.
        assert_eq!(Square::from_file_rank(5, 5).unwrap().to_string(), "f6");
    }

    #[test]
    fn the_file_letters_include_i() {
        // Chess skips it; tafl does not, and matching what players write is
        // worth more than matching chess.
        assert_eq!(Square::from_file_rank(8, 0).unwrap().to_string(), "i1");
        assert_eq!(Square::parse("i1"), Square::from_file_rank(8, 0));
    }

    #[test]
    fn notation_round_trips_for_every_square() {
        for index in 0..SQUARES as u8 {
            let square = Square::at(index);
            assert_eq!(Square::parse(&square.to_string()), Some(square));
        }
    }

    #[test]
    fn nonsense_notation_is_none_rather_than_a_square() {
        for text in ["", "a", "a0", "a12", "l1", "z9", "1a", "aa", "a-1", "🙂"] {
            assert_eq!(Square::parse(text), None, "parsed {text:?}");
        }
    }

    #[test]
    fn stepping_off_the_board_is_none_rather_than_a_wrap() {
        // The bug this forbids: adding one to k5's index gives a6, which is a
        // real square and the wrong answer.
        let east_edge = Square::parse("k5").unwrap();
        assert_eq!(east_edge.step(Direction::East, 1, 11), None);
        assert_eq!(
            east_edge.step(Direction::West, 1, 11),
            Square::parse("j5"),
            "and one step back is on the same rank"
        );

        let corner = Square::parse("a1").unwrap();
        assert_eq!(corner.step(Direction::South, 1, 11), None);
        assert_eq!(corner.step(Direction::West, 1, 11), None);
    }

    #[test]
    fn edges_are_the_outer_ring() {
        assert_eq!(
            (0..SQUARES as u8)
                .filter(|&i| Square::at(i).is_edge(11))
                .count(),
            // Four sides of eleven, less the four corners counted twice.
            4 * STRIDE as usize - 4
        );
        assert!(Square::parse("a1").unwrap().is_edge(11));
        assert!(Square::parse("f1").unwrap().is_edge(11));
        assert!(!Square::parse("b2").unwrap().is_edge(11));
        assert!(!Square::parse("f6").unwrap().is_edge(11));
    }

    #[test]
    fn a_bitboard_holds_exactly_the_board_it_is_asked_about() {
        for size in SIZES {
            assert_eq!(Bitboard::board(size).count(), (size as u32) * (size as u32));
            // The bits outside the board must never appear. Complementing is
            // where they would, which is why `complement` takes a size and
            // there is no `Not`.
            assert_eq!(
                Bitboard::EMPTY.complement(size).count(),
                (size as u32) * (size as u32)
            );
            assert_eq!(Bitboard::board(size).complement(size), Bitboard::EMPTY);
        }
    }

    /// The property that makes one index space serve three boards: a small
    /// board's squares are a **subset** of a large one's, at the same indices.
    /// Without it a stored square would mean different things on different
    /// boards, and a stored brandubh game would replay as nonsense.
    #[test]
    fn a_smaller_board_is_a_subset_of_a_larger_one() {
        for pair in SIZES.windows(2) {
            let (small, large) = (pair[0], pair[1]);
            let inside = Bitboard::board(small) & Bitboard::board(large);
            assert_eq!(inside, Bitboard::board(small), "{small} inside {large}");
        }
        // And a1 is index 0 on every one of them.
        for size in SIZES {
            assert!(Bitboard::board(size).contains(Square::at(0)));
            assert!(Square::at(0).is_on(size));
        }
    }

    #[test]
    fn the_edge_moves_with_the_board() {
        // g7 is the far corner of a brandubh board and an interior square on
        // every larger one. This is the single fact the whole refactor turns
        // on, and getting it backwards would make a 7x7 king unable to escape.
        let corner_of_seven = Square::parse("g7").unwrap();
        assert!(corner_of_seven.is_edge(7));
        assert!(!corner_of_seven.is_edge(9));
        assert!(!corner_of_seven.is_edge(11));
        assert_eq!(corner_of_seven.step(Direction::East, 1, 7), None);
        assert!(corner_of_seven.step(Direction::East, 1, 9).is_some());
    }

    #[test]
    fn the_throne_is_the_middle_of_whatever_board_it_is_on() {
        use crate::position::{corners, throne};
        assert_eq!(throne(7).to_string(), "d4");
        assert_eq!(throne(9).to_string(), "e5");
        assert_eq!(throne(11).to_string(), "f6");
        assert_eq!(
            corners(7).map(|square| square.to_string()),
            ["a1", "g1", "a7", "g7"]
        );
        assert_eq!(
            corners(11).map(|square| square.to_string()),
            ["a1", "k1", "a11", "k11"]
        );
    }

    #[test]
    fn a_bitboard_iterates_its_squares_in_order() {
        let set = Bitboard::EMPTY
            .with(Square::at(120))
            .with(Square::at(0))
            .with(Square::at(60));
        assert_eq!(
            set.collect::<Vec<_>>(),
            [Square::at(0), Square::at(60), Square::at(120)]
        );
    }

    #[test]
    fn axes_are_pairs_of_opposites() {
        // Capture is by custody, so every capture question is asked of an axis.
        for (one, other) in Direction::AXES {
            assert_eq!(one.opposite(), other);
            assert_eq!(other.opposite(), one);
        }
    }
}
