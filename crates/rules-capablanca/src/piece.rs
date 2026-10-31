//! Colours and piece kinds.
//!
//! Its own `Role` rather than `engine`'s, and the two must not be shared. A role
//! there is an index into a Zobrist table and a set of bitboards that are all
//! sixty-four bits wide; a role here indexes eighty-square ones. Coupling them
//! would mean this crate could not add a piece without moving numbers in the
//! crate that carries chess's perft gate — which is the whole reason this is a
//! separate engine.

use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub const ALL: [Color; 2] = [Color::White, Color::Black];

    pub const fn opponent(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    pub const fn index(self) -> usize {
        self as usize
    }

    /// Which way this side's pawns move, in board-index steps.
    ///
    /// **Ten, not eight.** Every place a chess engine says 8 for "one rank" this
    /// says 10, and getting one of them wrong produces a pawn that moves
    /// diagonally for no reason. It is a constant here so there is one of them.
    pub const fn forward(self) -> i16 {
        match self {
            Color::White => crate::board::FILES as i16,
            Color::Black => -(crate::board::FILES as i16),
        }
    }

    /// The rank this side's pawns start on, zero-based.
    pub const fn pawn_rank(self) -> u8 {
        match self {
            Color::White => 1,
            Color::Black => 6,
        }
    }

    /// The rank this side promotes on, zero-based.
    pub const fn promotion_rank(self) -> u8 {
        match self {
            Color::White => 7,
            Color::Black => 0,
        }
    }

    /// The rank this side's back pieces start on, zero-based.
    pub const fn back_rank(self) -> u8 {
        match self {
            Color::White => 0,
            Color::Black => 7,
        }
    }
}

/// What a piece is, independent of whose it is.
///
/// The two additions are **compound** pieces: an archbishop is a bishop and a
/// knight in one, a chancellor is a rook and a knight. That is the reason
/// Capablanca chess is a cheap game to build once the board is wide enough —
/// neither piece needs a movement rule that does not already exist, only the
/// union of two that do.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Role {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
    /// Bishop plus knight. Capablanca called it the archbishop; other sources
    /// call it the cardinal or the princess.
    Archbishop,
    /// Rook plus knight. Also the marshal, or the empress.
    Chancellor,
}

impl Role {
    pub const ALL: [Role; 8] = [
        Role::Pawn,
        Role::Knight,
        Role::Bishop,
        Role::Rook,
        Role::Queen,
        Role::King,
        Role::Archbishop,
        Role::Chancellor,
    ];

    /// What a pawn may become. Six choices where chess has four, and the two
    /// extra ones are not decoration: a chancellor is worth more than a rook,
    /// so an underpromotion here is a real decision rather than a stalemate
    /// dodge.
    pub const PROMOTIONS: [Role; 6] = [
        Role::Queen,
        Role::Chancellor,
        Role::Archbishop,
        Role::Rook,
        Role::Bishop,
        Role::Knight,
    ];

    pub const COUNT: usize = 8;

    pub const fn index(self) -> usize {
        self as usize
    }

    /// The letter in notation and in FEN. `A` for the archbishop and `C` for
    /// the chancellor, which is what the published Capablanca literature uses.
    pub const fn letter(self) -> char {
        match self {
            Role::Pawn => 'P',
            Role::Knight => 'N',
            Role::Bishop => 'B',
            Role::Rook => 'R',
            Role::Queen => 'Q',
            Role::King => 'K',
            Role::Archbishop => 'A',
            Role::Chancellor => 'C',
        }
    }

    pub const fn from_letter(letter: char) -> Option<Role> {
        match letter.to_ascii_uppercase() {
            'P' => Some(Role::Pawn),
            'N' => Some(Role::Knight),
            'B' => Some(Role::Bishop),
            'R' => Some(Role::Rook),
            'Q' => Some(Role::Queen),
            'K' => Some(Role::King),
            'A' => Some(Role::Archbishop),
            'C' => Some(Role::Chancellor),
            _ => None,
        }
    }
}

/// A piece: a role, owned by a colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Piece {
    pub color: Color,
    pub role: Role,
}

impl Piece {
    pub const fn new(color: Color, role: Role) -> Self {
        Piece { color, role }
    }

    /// FEN's letter: uppercase White, lowercase Black.
    pub const fn fen_char(self) -> char {
        let letter = self.role.letter();
        match self.color {
            Color::White => letter,
            Color::Black => letter.to_ascii_lowercase(),
        }
    }

    pub const fn from_fen_char(letter: char) -> Option<Piece> {
        let color = if letter.is_ascii_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        match Role::from_letter(letter) {
            Some(role) => Some(Piece::new(color, role)),
            None => None,
        }
    }
}

impl fmt::Display for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.fen_char())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fen_chars_round_trip() {
        for color in Color::ALL {
            for role in Role::ALL {
                let piece = Piece::new(color, role);
                assert_eq!(Piece::from_fen_char(piece.fen_char()), Some(piece));
            }
        }
    }

    /// A rank is ten squares here, and a pawn step is one rank. Stated as a
    /// test because "8" is written into every chess engine ever published and
    /// this one has to say 10 everywhere instead.
    #[test]
    fn a_pawn_step_is_the_width_of_the_board() {
        assert_eq!(Color::White.forward(), 10);
        assert_eq!(Color::Black.forward(), -10);
    }

    #[test]
    fn a_pawn_may_become_six_things_and_never_a_king_or_a_pawn() {
        assert_eq!(Role::PROMOTIONS.len(), 6);
        assert!(!Role::PROMOTIONS.contains(&Role::King));
        assert!(!Role::PROMOTIONS.contains(&Role::Pawn));
    }
}
