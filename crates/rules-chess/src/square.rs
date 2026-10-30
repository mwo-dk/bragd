//! Squares, files and ranks.

use core::fmt;

/// A square, indexed a1 = 0 through h8 = 63.
///
/// The ordering matters and is relied on throughout: index = rank * 8 + file,
/// so a bitboard's least significant bit is a1 and shifting left by 8 moves a
/// piece one rank towards Black.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Square(u8);

impl Square {
    pub const COUNT: usize = 64;

    /// # Panics
    /// If `index` is not a square.
    pub const fn new(index: u8) -> Self {
        assert!(index < 64, "square index out of range");
        Square(index)
    }

    pub const fn from_file_rank(file: u8, rank: u8) -> Self {
        assert!(file < 8 && rank < 8, "file or rank out of range");
        Square(rank * 8 + file)
    }

    pub const fn index(self) -> u8 {
        self.0
    }

    pub const fn file(self) -> u8 {
        self.0 & 7
    }

    pub const fn rank(self) -> u8 {
        self.0 >> 3
    }

    /// The same square seen from the other side of the board. Used so that
    /// White-relative logic can be reused for Black.
    pub const fn flip_rank(self) -> Self {
        Square(self.0 ^ 56)
    }

    /// Shifts by a whole number of squares in the index ordering, returning
    /// `None` off the edge. Callers doing file-relative arithmetic must mask
    /// the file themselves — this only guards the ends of the board.
    pub const fn offset(self, delta: i8) -> Option<Self> {
        let target = self.0 as i16 + delta as i16;
        if target < 0 || target > 63 {
            None
        } else {
            Some(Square(target as u8))
        }
    }

    /// Parses algebraic coordinates such as `e4`. Rejects anything else.
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 2 {
            return None;
        }
        let file = bytes[0].checked_sub(b'a')?;
        let rank = bytes[1].checked_sub(b'1')?;
        if file < 8 && rank < 8 {
            Some(Square::from_file_rank(file, rank))
        } else {
            None
        }
    }
}

impl fmt::Display for Square {
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
        fmt::Display::fmt(self, f)
    }
}

/// Named squares. Only the ones the rules themselves talk about — castling
/// origins and destinations, and the rook corners whose capture ends a right.
pub mod squares {
    use super::Square;

    pub const A1: Square = Square::new(0);
    pub const B1: Square = Square::new(1);
    pub const C1: Square = Square::new(2);
    pub const D1: Square = Square::new(3);
    pub const E1: Square = Square::new(4);
    pub const F1: Square = Square::new(5);
    pub const G1: Square = Square::new(6);
    pub const H1: Square = Square::new(7);

    pub const A8: Square = Square::new(56);
    pub const B8: Square = Square::new(57);
    pub const C8: Square = Square::new(58);
    pub const D8: Square = Square::new(59);
    pub const E8: Square = Square::new(60);
    pub const F8: Square = Square::new(61);
    pub const G8: Square = Square::new(62);
    pub const H8: Square = Square::new(63);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn algebraic_round_trips() {
        for index in 0..64u8 {
            let square = Square::new(index);
            assert_eq!(Square::parse(&square.to_string()), Some(square));
        }
    }

    #[test]
    fn rejects_nonsense() {
        for text in ["", "e", "e9", "i4", "e44", "4e", "E4"] {
            assert_eq!(Square::parse(text), None, "accepted {text:?}");
        }
    }

    #[test]
    fn corners_are_where_they_should_be() {
        assert_eq!(squares::A1.to_string(), "a1");
        assert_eq!(squares::H8.to_string(), "h8");
        assert_eq!(squares::E1.flip_rank(), squares::E8);
    }
}
