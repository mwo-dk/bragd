//! Colours and piece kinds.

use core::fmt;

/// Which side a piece belongs to, and which Player is to move.
///
/// Note the collision with the glossary: in `CONTEXT.md`, *light* and *dark*
/// are Theme values and also the two square colours. The two sides of a chess
/// game are `White` and `Black` and always will be — the board theme is free to
/// paint them however it likes.
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
    pub const fn forward(self) -> i8 {
        match self {
            Color::White => 8,
            Color::Black => -8,
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
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Role {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
    /// Shatranj's counsellor: one square diagonally. Never appears in a chess
    /// or Chess960 position — see the note on [`Role::Alfil`].
    Ferz,
    /// Shatranj's elephant: exactly two squares diagonally, leaping whatever
    /// stands between.
    ///
    /// This and [`Role::Ferz`] are dead code for chess *by construction*
    /// rather than by discipline: no chess starting position contains one and
    /// no chess move can produce one, so every chess code path that matches on
    /// a role is unreachable for these two. That is why they are their own
    /// roles instead of a variant flag threaded through move generation, which
    /// would have put a branch in the hottest path in the engine and made the
    /// perft gate answer a question it is not asking.
    ///
    /// They are appended rather than inserted so that every existing role
    /// keeps its index.
    Alfil,
    /// Chaturanga's elephant: one square straight forward, or one square in any
    /// diagonal direction.
    ///
    /// The same *piece* as the [`Role::Alfil`] — both are the gaja, the war
    /// elephant — under a different reconstruction of how it moved. It is a
    /// separate role rather than a flag on the alfil because it is the only
    /// piece in this crate whose table depends on which side owns it: a gaja
    /// moves towards the enemy, like a pawn, so White's and Black's are mirror
    /// images. See [`crate::chaturanga`] for which reconstruction and why.
    Gaja,
    /// Seirawan chess's **hawk**: a bishop and a knight at once.
    ///
    /// Appended after the gaja so every index above stays put — a role's index
    /// is a table offset in `attacks` and a Zobrist key offset, and renumbering
    /// them would silently change every stored position's hash.
    Hawk,
    /// Seirawan chess's **elephant**: a rook and a knight at once.
    ///
    /// Not to be confused with the alfil or the gaja, which are also elephants
    /// and are also in Bragd. Chess history has a great many elephants; this
    /// one is the strongest of them by a distance.
    Elephant,
}

impl Role {
    pub const ALL: [Role; 11] = [
        Role::Pawn,
        Role::Knight,
        Role::Bishop,
        Role::Rook,
        Role::Queen,
        Role::King,
        Role::Ferz,
        Role::Alfil,
        Role::Gaja,
        Role::Hawk,
        Role::Elephant,
    ];

    /// The roles standard chess can produce. Chess move generation walks this
    /// rather than [`Role::ALL`], so adding a variant's piece cannot slow the
    /// gated path down or change what it generates.
    pub const CHESS: [Role; 6] = [
        Role::Pawn,
        Role::Knight,
        Role::Bishop,
        Role::Rook,
        Role::Queen,
        Role::King,
    ];

    pub const COUNT: usize = 11;

    pub const fn index(self) -> usize {
        self as usize
    }

    /// The letter used in algebraic notation. Pawns have none, so this returns
    /// `None` rather than an empty string — the caller has to decide.
    pub const fn letter(self) -> Option<char> {
        match self {
            Role::Pawn => None,
            Role::Knight => Some('N'),
            Role::Bishop => Some('B'),
            Role::Rook => Some('R'),
            Role::Queen => Some('Q'),
            Role::King => Some('K'),
            Role::Ferz => Some('F'),
            Role::Alfil => Some('E'),
            Role::Gaja => Some('G'),
            // `H` and `E` are what Seirawan chess itself uses. `E` collides
            // with the alfil's letter, and deliberately so: the two never
            // appear in the same variant, and following each game's own
            // notation matters more than a globally unique alphabet.
            Role::Hawk => Some('H'),
            Role::Elephant => Some('E'),
        }
    }

    /// `F` for the ferz, `E` for the alfil and `G` for the gaja, rather than
    /// reusing `Q` and `B`.
    ///
    /// Shatranj FENs found in the wild conventionally reuse the chess letters,
    /// because the opening array is chess's. Distinct letters are chosen here
    /// anyway: there is no shatranj corpus to ingest — unlike chess, whose
    /// published perft positions arrive as FENs we do not control — so the only
    /// reader of these is us, and a letter that names exactly one role keeps
    /// this function total and its inverse unambiguous. A shared letter would
    /// mean a FEN could not be parsed without knowing the variant, which is the
    /// same parameter-threading this representation was chosen to avoid.
    pub const fn from_letter(letter: char) -> Option<Role> {
        match letter.to_ascii_uppercase() {
            'P' => Some(Role::Pawn),
            'N' => Some(Role::Knight),
            'B' => Some(Role::Bishop),
            'R' => Some(Role::Rook),
            'Q' => Some(Role::Queen),
            'K' => Some(Role::King),
            'F' => Some(Role::Ferz),
            'E' => Some(Role::Alfil),
            'G' => Some(Role::Gaja),
            'H' => Some(Role::Hawk),
            // **`S`, not `E`.** Seirawan chess writes its elephant `E`, and so
            // does shatranj's alfil — which shipped first and is in stored
            // games that must keep reading back correctly for ever.
            // A shared parser cannot return both, and guessing from context is
            // exactly the kind of thing that silently rewrites one game's
            // history into another's.
            //
            // So storage uses `S` and the *player* still sees `E`: `Role::letter`
            // is what SAN is written with, and an S-chess player writes `Ed4`.
            // Two alphabets is a smaller cost than an ambiguous one, and nothing
            // here imports a published S-chess FEN.
            'S' => Some(Role::Elephant),
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

    /// FEN's letter for this piece: uppercase White, lowercase Black.
    pub const fn fen_char(self) -> char {
        let letter = match self.role {
            Role::Pawn => 'P',
            Role::Knight => 'N',
            Role::Bishop => 'B',
            Role::Rook => 'R',
            Role::Queen => 'Q',
            Role::King => 'K',
            Role::Ferz => 'F',
            Role::Alfil => 'E',
            Role::Gaja => 'G',
            Role::Hawk => 'H',
            // `S` rather than `E`. See `Role::from_letter` for why storage and
            // notation disagree about this one piece.
            Role::Elephant => 'S',
        };
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

    #[test]
    fn pawns_move_towards_their_promotion_rank() {
        for color in Color::ALL {
            assert_eq!(color.opponent().opponent(), color);
            assert_eq!(color.back_rank(), color.opponent().promotion_rank());
            let direction = color.promotion_rank() as i8 - color.pawn_rank() as i8;
            assert_eq!(direction.signum(), color.forward().signum());
        }
    }

    #[test]
    fn rejects_pieces_that_are_not_pieces() {
        for letter in ['x', '1', ' ', '/'] {
            assert_eq!(Piece::from_fen_char(letter), None);
        }
    }
}
