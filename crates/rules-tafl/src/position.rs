//! A position: who is where, and whose turn it is.
//!
//! Two sides that are not two colours. **The asymmetry is the game** and it
//! runs all the way down: the attackers have twice the material, start on the
//! edges, and win by capturing the king; the defenders start in the centre
//! around him and win by getting him out. Neither side has the other's pieces,
//! the other's shape, or the other's objective.
//!
//! That is why nothing here is called white or black, and why there is no
//! `opponent()` returning a mirror image. `Side::Attackers` and
//! `Side::Defenders` are different kinds of thing that happen to alternate.

use crate::board::{Bitboard, Direction, STRIDE, Square};
use core::fmt;

/// Which army a piece belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Side {
    /// The besiegers. Twice the material, no king, and they move first.
    Attackers,
    /// The besieged. The king and his guard.
    Defenders,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::Attackers, Side::Defenders];

    pub const fn other(self) -> Side {
        match self {
            Side::Attackers => Side::Defenders,
            Side::Defenders => Side::Attackers,
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Side::Attackers => 0,
            Side::Defenders => 1,
        }
    }
}

/// What a piece is.
///
/// Two kinds, and only one of each side has the second: there is exactly one
/// king, he is a defender, and he is the whole point of the game.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Role {
    Soldier,
    King,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Piece {
    pub side: Side,
    pub role: Role,
}

/// The centre square, where the king begins. Called the throne, or the castle.
///
/// Every tafl board is odd-sided precisely so that this square exists: the game
/// is about a king leaving the exact middle, and an even board has no middle.
pub const fn throne(size: u8) -> Square {
    let middle = size / 2;
    Square::at(middle * STRIDE + middle)
}

/// The four corners, which is where the king is trying to get to.
pub const fn corners(size: u8) -> [Square; 4] {
    let last = size - 1;
    [
        Square::at(0),
        Square::at(last),
        Square::at(last * STRIDE),
        Square::at(last * STRIDE + last),
    ]
}

/// A move: a piece, and where it goes.
///
/// That is the whole of it. There is no move kind, no promotion and no
/// castling — and captures are **not** part of a move, because they are a
/// consequence of it. A tafl capture takes pieces the mover never touched, and
/// possibly several at once, so recording "what was captured" in the move would
/// be recording a derived fact and inviting it to disagree with the board.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move {
    pub from: Square,
    pub to: Square,
}

impl Move {
    pub const fn new(from: Square, to: Square) -> Move {
        Move { from, to }
    }

    /// The wire form: the two squares, run together. `f6f9`.
    ///
    /// Unambiguous by construction — every move is exactly one piece going from
    /// one square to another, and no two legal moves in a position can share a
    /// pair. There is no established tafl notation to be compatible with, which
    /// is a freedom rather than a problem: this one is readable, sortable, and
    /// cannot be confused with chess's.
    pub fn to_wire(self) -> String {
        format!("{}{}", self.from, self.to)
    }

    pub fn parse(text: &str) -> Option<Move> {
        // Guarded before slicing: this string arrives from outside, and
        // splitting a multi-byte character by byte index is a panic rather than
        // a rejection.
        if !text.is_ascii() {
            return None;
        }
        // Squares are two or three characters (`a1` to `k11`), so the split
        // point is found rather than assumed.
        for split in 2..=3.min(text.len().saturating_sub(2)) {
            if let (Some(from), Some(to)) =
                (Square::parse(&text[..split]), Square::parse(&text[split..]))
            {
                return Some(Move::new(from, to));
            }
        }
        None
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_wire())
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_wire())
    }
}

/// Everything about a position.
///
/// Copy-make, like the chess engine: a move produces a new position rather than
/// mutating one and remembering how to undo it. An undo path is a second
/// implementation of every rule, and the two drift.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    /// `by_side[side]` — every piece of that side, king included.
    by_side: [Bitboard; 2],
    /// The king. One bit, always, while he is on the board.
    king: Bitboard,
    to_move: Side,
    /// How wide the board is: 7, 9 or 11.
    ///
    /// On the position rather than a constant, which is what makes three games
    /// out of one engine. The index space does not change with it — see the
    /// note in `board.rs` — so this is only ever consulted about where the
    /// *edge* is.
    size: u8,
}

impl Position {
    /// The standard 11×11 opening array.
    ///
    /// Twenty-four attackers in four groups of six against the middle of each
    /// edge, and thirteen defenders in a cross around the throne. This layout
    /// is shared by every 11×11 ruleset in modern use — Fetlar, Copenhagen and
    /// the rest disagree about the rules, not about where the pieces start.
    /// How wide this board is: 7, 9 or 11.
    pub const fn size(&self) -> u8 {
        self.size
    }

    /// The centre square of *this* board.
    pub const fn throne(&self) -> Square {
        throne(self.size)
    }

    /// The four corners of *this* board.
    pub const fn corners(&self) -> [Square; 4] {
        corners(self.size)
    }

    /// Every square on this board.
    pub const fn board(&self) -> Bitboard {
        Bitboard::board(self.size)
    }

    /// **Brandubh**, the Irish 7×7 game.
    ///
    /// Four attackers against four defenders and a king — the smallest tafl
    /// there is, and the one with the most surviving Irish poetry attached and
    /// the least surviving rule. The layout is the one the archaeology
    /// supports: a cross of attackers against the middle of each edge, and a
    /// cross of defenders around the throne.
    pub fn brandubh() -> Position {
        Position::from_picture(
            &[
                "...X...", "...X...", "...O...", "XXOKOXX", "...O...", "...X...", "...X...",
            ],
            Side::Attackers,
        )
    }

    /// **Tablut**, the Sámi 9×9 game.
    ///
    /// Sixteen attackers against eight defenders and a king, in the layout
    /// Linnaeus drew in his Lapland journal in 1732 — which is the only
    /// contemporary description of any tafl game that survives, and is a
    /// traveller's note rather than a rulebook. The rules document says so.
    pub fn tablut() -> Position {
        Position::from_picture(
            &[
                "...XXX...",
                "....X....",
                "....O....",
                "X...O...X",
                // Four defenders across the middle rank, not two: the guard is
                // eight strong and forms a cross two deep in each direction.
                // A first attempt at this drew six and the count assertion
                // caught it, which is exactly what that assertion is for.
                "XXOOKOOXX",
                "X...O...X",
                "....O....",
                "....X....",
                "...XXX...",
            ],
            Side::Attackers,
        )
    }

    pub fn standard() -> Position {
        // Written as a picture, because a bitboard literal would be
        // unreviewable and this is the one place the board's shape is asserted
        // rather than derived. Rank 11 first, the way it is printed.
        const LAYOUT: [&str; 11] = [
            "...XXXXX...",
            ".....X.....",
            "...........",
            "X....O....X",
            "X...OOO...X",
            "XX.OOKOO.XX",
            "X...OOO...X",
            "X....O....X",
            "...........",
            ".....X.....",
            "...XXXXX...",
        ];

        // The attackers move first, in every ruleset. The besieged do not get
        // to run before the siege begins.
        Position::from_picture(&LAYOUT, Side::Attackers)
    }

    /// A position drawn as a picture: `X` attacker, `O` defender, `K` king.
    ///
    /// Rank 11 first, the way a board is printed. This is how positions are
    /// written in tests, because a tafl position is about *shape* — who
    /// surrounds whom — and a diff of two pictures is readable where a diff of
    /// two bitboards is not.
    ///
    /// The board's size is taken from the picture: a square picture of an odd
    /// side is a board, and anything else is a broken test.
    ///
    /// # Panics
    /// If the picture is not square, is not one of the three sizes, or holds a
    /// character that is not one of `XOK.`. A malformed picture in a test is a
    /// broken test, and failing at the first one beats a silently different
    /// board.
    pub fn from_picture(layout: &[&str], to_move: Side) -> Position {
        let size = layout.len() as u8;
        assert!(
            crate::board::SIZES.contains(&size),
            "{size} is not a tafl board"
        );
        let mut by_side = [Bitboard::EMPTY; 2];
        let mut king = Bitboard::EMPTY;
        for (row, line) in layout.iter().enumerate() {
            assert_eq!(
                line.chars().count(),
                size as usize,
                "row {row} is the wrong width"
            );
            let rank = size - 1 - row as u8;
            for (file, character) in line.chars().enumerate() {
                let square = Square::from_file_rank(file as u8, rank).expect("on the board");
                match character {
                    'X' => by_side[Side::Attackers.index()] |= Bitboard::from_square(square),
                    'O' => by_side[Side::Defenders.index()] |= Bitboard::from_square(square),
                    'K' => {
                        by_side[Side::Defenders.index()] |= Bitboard::from_square(square);
                        king = Bitboard::from_square(square);
                    }
                    '.' => {}
                    other => panic!("{other:?} is not a piece"),
                }
            }
        }
        Position {
            by_side,
            king,
            to_move,
            size,
        }
    }

    /// A position from lists of squares, for tests that build a shape rather
    /// than draw one.
    ///
    /// # Panics
    /// On a square name that does not parse, which in a test is a broken test.
    pub fn from_squares(
        attackers: &[&str],
        defenders: &[&str],
        king: &str,
        to_move: Side,
    ) -> Position {
        let parse =
            |name: &str| Square::parse(name).unwrap_or_else(|| panic!("{name} is not a square"));
        let mut by_side = [Bitboard::EMPTY; 2];
        for name in attackers {
            by_side[Side::Attackers.index()] |= Bitboard::from_square(parse(name));
        }
        for name in defenders {
            by_side[Side::Defenders.index()] |= Bitboard::from_square(parse(name));
        }
        let king_square = parse(king);
        by_side[Side::Defenders.index()] |= Bitboard::from_square(king_square);
        Position {
            by_side,
            king: Bitboard::from_square(king_square),
            to_move,
            // The 11×11 board, because that is what every caller of this builds
            // and because `a1`-style names carry no size. A smaller board is
            // written as a picture, where the size is visible.
            size: 11,
        }
    }

    pub fn to_move(self) -> Side {
        self.to_move
    }

    pub fn side(self, side: Side) -> Bitboard {
        self.by_side[side.index()]
    }

    pub fn occupied(self) -> Bitboard {
        self.by_side[0] | self.by_side[1]
    }

    /// Where the king stands, or `None` if he has been taken.
    pub fn king_square(self) -> Option<Square> {
        self.king.first()
    }

    pub fn piece_at(self, square: Square) -> Option<Piece> {
        let role = if self.king.contains(square) {
            Role::King
        } else {
            Role::Soldier
        };
        Side::ALL
            .into_iter()
            .find(|&side| self.side(side).contains(square))
            .map(|side| Piece { side, role })
    }

    /// Every square a piece on `from` could reach, ignoring whose turn it is.
    ///
    /// Rook movement: any distance along a rank or file, stopping before the
    /// first occupied square. `blocked` is the set of squares this piece may
    /// not *land* on even when they are empty — the throne and the corners are
    /// restricted to the king in most rulesets, and that is a rule rather than
    /// geometry, so it arrives as a parameter.
    ///
    /// Note what is **not** here: nothing about capture. A tafl piece never
    /// moves onto an enemy, so the destination set does not depend on which
    /// side is which.
    pub fn destinations(self, from: Square, blocked: Bitboard) -> Bitboard {
        let occupied = self.occupied();
        let mut reachable = Bitboard::EMPTY;
        for direction in Direction::ALL {
            let mut steps = 1;
            while let Some(square) = from.step(direction, steps, self.size) {
                if occupied.contains(square) {
                    // Nothing jumps, ever. Not even the king.
                    break;
                }
                if !blocked.contains(square) {
                    reachable |= Bitboard::from_square(square);
                }
                // A blocked square that is empty may still be *passed through*
                // or not, which is a ruleset question — the caller decides by
                // what it puts in `blocked` versus in `occupied`.
                steps += 1;
            }
        }
        reachable
    }

    /// Moves the piece on `from` to `to`, with no rule applied but the board's.
    ///
    /// Captures are not resolved here: they belong to the ruleset, and this is
    /// the mechanical half. `rules::play` is what a caller wants.
    pub fn slide(self, mv: Move) -> Position {
        let mut next = self;
        let side = self
            .piece_at(mv.from)
            .expect("a move from an empty square")
            .side;
        next.by_side[side.index()] = next.by_side[side.index()].without(mv.from).with(mv.to);
        if self.king.contains(mv.from) {
            next.king = Bitboard::from_square(mv.to);
        }
        next.to_move = self.to_move.other();
        next
    }

    /// The same position with a different side to move.
    ///
    /// Only for asking hypothetical questions — "could the attackers break this
    /// wall" — where the answer must not depend on whose turn it happens to be.
    /// Passing is not a legal move in tafl.
    pub fn pass_to(self, side: Side) -> Position {
        Position {
            to_move: side,
            ..self
        }
    }

    /// Takes a piece off the board.
    pub fn capture(self, square: Square) -> Position {
        let mut next = self;
        for side in Side::ALL {
            next.by_side[side.index()] = next.by_side[side.index()].without(square);
        }
        next.king = next.king.without(square);
        next
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..self.size).rev() {
            for file in 0..self.size {
                let square = Square::from_file_rank(file, rank).expect("on the board");
                f.write_str(match self.piece_at(square) {
                    Some(Piece {
                        role: Role::King, ..
                    }) => "K",
                    Some(Piece {
                        side: Side::Attackers,
                        ..
                    }) => "X",
                    Some(Piece {
                        side: Side::Defenders,
                        ..
                    }) => "O",
                    None if square == self.throne() => "+",
                    None if self.corners().contains(&square) => "*",
                    None => ".",
                })?;
            }
            writeln!(f)?;
        }
        write!(f, "{:?} to move", self.to_move)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_array_is_the_one_everybody_plays() {
        let start = Position::standard();
        // Twenty-four against twelve and the king: the two-to-one material
        // imbalance that the asymmetric win conditions exist to offset.
        assert_eq!(start.side(Side::Attackers).count(), 24);
        assert_eq!(start.side(Side::Defenders).count(), 13);
        assert_eq!(start.king_square(), Some(throne(11)));
        assert_eq!(start.to_move(), Side::Attackers);
    }

    #[test]
    fn the_array_is_symmetric_under_every_quarter_turn() {
        // The strongest cheap check on a hand-transcribed layout: the opening
        // array has four-fold rotational symmetry, so a single misplaced piece
        // breaks it. A typo that survives this is a typo in four places.
        let start = Position::standard();
        for side in Side::ALL {
            let pieces = start.side(side);
            for square in pieces {
                let (file, rank) = (square.file(), square.rank());
                let rotated =
                    Square::from_file_rank(rank, STRIDE - 1 - file).expect("on the board");
                assert!(
                    pieces.contains(rotated),
                    "{square} has no counterpart at {rotated}"
                );
            }
        }
    }

    #[test]
    fn the_throne_is_the_centre_and_the_corners_are_the_corners() {
        assert_eq!(throne(11).to_string(), "f6");
        let named: Vec<String> = corners(11)
            .iter()
            .map(|square| square.to_string())
            .collect();
        assert_eq!(named, ["a1", "k1", "a11", "k11"]);
        for corner in corners(11) {
            assert!(corner.is_edge(11));
        }
    }

    #[test]
    fn pieces_move_like_rooks_and_nothing_jumps() {
        let start = Position::standard();
        // The attacker on f10 is directly above his own group and below
        // nothing, so his moves are the file and the rank he can see.
        let f10 = Square::parse("f10").unwrap();
        let reachable = start.destinations(f10, Bitboard::EMPTY);

        assert!(
            reachable.contains(Square::parse("f9").unwrap()),
            "one step down"
        );
        assert!(
            reachable.contains(Square::parse("a10").unwrap()),
            "along the rank"
        );
        assert!(
            !reachable.contains(Square::parse("f11").unwrap()),
            "occupied by his own side"
        );
        assert!(
            !reachable.contains(Square::parse("f8").unwrap()),
            "f8 holds a defender, and nothing in tafl jumps"
        );
        assert!(
            !reachable.contains(Square::parse("f7").unwrap()),
            "and nothing beyond him either"
        );
    }

    #[test]
    fn a_blocked_square_can_be_passed_but_not_landed_on() {
        // The throne is restricted to the king in most rulesets, and that is a
        // rule rather than geometry — so it arrives as a parameter and the
        // generator treats it as "may not land" rather than "may not pass".
        let start = Position::standard();
        let empty = Position {
            size: 11,
            by_side: [Bitboard::EMPTY, Bitboard::from_square(throne(11))],
            king: Bitboard::from_square(throne(11)),
            to_move: Side::Defenders,
        };
        let _ = start;

        let king_moves = empty.destinations(throne(11), Bitboard::EMPTY);
        assert_eq!(king_moves.count(), (STRIDE as u32 - 1) * 2, "a clear board");

        let restricted = Bitboard::EMPTY.with(Square::parse("f1").unwrap());
        let fewer = empty.destinations(throne(11), restricted);
        assert!(!fewer.contains(Square::parse("f1").unwrap()));
        assert!(
            fewer.contains(Square::parse("f2").unwrap()),
            "and everything short of it is still reachable"
        );
    }

    #[test]
    fn a_move_round_trips_through_its_wire_form() {
        for from in 0..121u8 {
            for to in [0u8, 5, 60, 119, 120] {
                let mv = Move::new(Square::at(from), Square::at(to));
                assert_eq!(Move::parse(&mv.to_wire()), Some(mv), "{mv}");
            }
        }
        assert_eq!(
            Move::parse("f6f9"),
            Some(Move::new(throne(11), Square::parse("f9").unwrap()))
        );
        for nonsense in ["", "f6", "f6f", "zzzz", "f6f12", "🙂🙂"] {
            assert_eq!(Move::parse(nonsense), None, "parsed {nonsense:?}");
        }
    }

    #[test]
    fn sliding_moves_the_piece_and_passes_the_turn() {
        let start = Position::standard();
        let mv = Move::new(Square::parse("f10").unwrap(), Square::parse("f9").unwrap());
        let after = start.slide(mv);
        assert_eq!(after.piece_at(Square::parse("f10").unwrap()), None);
        assert_eq!(
            after.piece_at(Square::parse("f9").unwrap()).map(|p| p.side),
            Some(Side::Attackers)
        );
        assert_eq!(after.to_move(), Side::Defenders);
    }

    #[test]
    fn the_king_is_tracked_wherever_he_goes() {
        let start = Position::standard();
        let out = start.slide(Move::new(throne(11), Square::parse("f8").unwrap()));
        assert_eq!(out.king_square(), Some(Square::parse("f8").unwrap()));
        assert_eq!(
            out.piece_at(Square::parse("f8").unwrap()).map(|p| p.role),
            Some(Role::King)
        );
        // And taking him off leaves no king at all, which is how the attackers
        // win rather than a state that needs a flag.
        let taken = out.capture(Square::parse("f8").unwrap());
        assert_eq!(taken.king_square(), None);
    }
}
