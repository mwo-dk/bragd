//! The board, the two paths across it, and the rosettes.
//!
//! Ur is the first game in Bragd where **the two players do not share a
//! board in the sense the other families mean it.** A chess square is the same
//! square to both sides; here, square 0 is on White's own row and Black can
//! never stand on it. Eight of the twenty squares are shared and twelve are
//! private, six to each side, and every rule that matters — capture, safety,
//! blocking — is a rule about which of those two kinds a square is.
//!
//! So the geometry is written down once, here, as two paths, and everything
//! else asks it a question rather than knowing the answer.

/// A side. Named for the seats, not for a colour — Bragd's own word is Side
/// and storage calls the two seats `white` and `black` because chess arrived
/// first.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Side {
    White,
    Black,
}

impl Side {
    /// The other one. Ur has no third state and no neutral, so this is total.
    pub fn other(self) -> Side {
        match self {
            Side::White => Side::Black,
            Side::Black => Side::White,
        }
    }

    /// The wire spelling, which is the seat name storage uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Side::White => "white",
            Side::Black => "black",
        }
    }

    /// Reads a seat back. Returns `None` rather than defaulting to White — a
    /// side that failed to parse is a bug upstream, and guessing it would put
    /// the wrong player on move in a real Game.
    pub fn parse(text: &str) -> Option<Side> {
        match text {
            "white" => Some(Side::White),
            "black" => Some(Side::Black),
            _ => None,
        }
    }
}

/// How many pieces each side brings. Seven, which is what the excavated boards
/// were found with — fourteen shell counters in two sets of seven, in the grave
/// at Ur that gives the game its name.
pub const PIECES: u8 = 7;

/// The number of squares on the board.
pub const SQUARES: usize = 20;

/// How many steps a piece takes from entering to bearing off.
///
/// Fourteen, not twenty: a piece walks four squares of its own row, then the
/// eight shared ones, then two more of its own, and the twentieth step is off
/// the board rather than onto a square.
pub const PATH: usize = 14;

/// The board is three rows of eight, with four squares missing from the middle
/// of the top and bottom rows — the waist that makes the shape everybody
/// recognises.
pub const ROWS: usize = 3;
pub const COLUMNS: usize = 8;

/// Where a piece is, from its owner's point of view.
///
/// **Not a square.** A piece that has not entered is nowhere on the board, and a
/// piece that has borne off is nowhere on the board either, and those two are
/// not the same nowhere. Squares alone cannot say which, and a representation
/// that used a sentinel square for both is the one that lets a borne-off piece
/// be captured.
///
/// The ordering is declaration order — `Waiting` before every `Step` before
/// `Off` — which is the order a piece passes through them, and it is what lets a
/// side's pieces be kept in a canonical order. They are interchangeable, so two
/// positions that differ only in which array slot holds which piece are the same
/// position, and sorting is what makes `==` say so.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Spot {
    /// Waiting to enter. Step zero.
    Waiting,
    /// On the board, at this many steps along its owner's path (1..=PATH).
    Step(u8),
    /// Home, and out of the game.
    Off,
}

impl Spot {
    /// How far along the path this is, counting `Waiting` as nought and `Off` as
    /// one past the end. This is the number the arithmetic is done in.
    pub fn distance(self) -> u8 {
        match self {
            Spot::Waiting => 0,
            Spot::Step(step) => step,
            Spot::Off => PATH as u8 + 1,
        }
    }

    /// The spot this many steps further on, or `None` if that would overshoot
    /// bearing off. **An exact throw is required to come home** — see
    /// [`crate::position::Position::moves_for`] for why that is the reading this
    /// engine takes.
    pub fn advance(self, steps: u8) -> Option<Spot> {
        let to = self.distance() + steps;
        match to {
            0 => Some(Spot::Waiting),
            step if step as usize <= PATH => Some(Spot::Step(step)),
            step if step as usize == PATH + 1 => Some(Spot::Off),
            _ => None,
        }
    }
}

/// A square of the board, as a row and a column, so that a drawing can place it
/// without knowing anything about paths.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Square {
    pub row: u8,
    pub column: u8,
}

impl Square {
    const fn new(row: u8, column: u8) -> Square {
        Square { row, column }
    }
}

/// The twenty squares, in a fixed order that the wire uses as an index.
///
/// Row 0 is White's own row, row 1 is the shared middle, row 2 is Black's. The
/// order within a row is left to right, and the four squares that do not exist
/// — the waist — are simply absent, which is why this is a table and not
/// arithmetic over `ROWS * COLUMNS`.
pub const BOARD: [Square; SQUARES] = [
    // White's row: columns 0-3 and 6-7. Columns 4 and 5 are the waist.
    Square::new(0, 0),
    Square::new(0, 1),
    Square::new(0, 2),
    Square::new(0, 3),
    Square::new(0, 6),
    Square::new(0, 7),
    // The shared middle row, all eight columns.
    Square::new(1, 0),
    Square::new(1, 1),
    Square::new(1, 2),
    Square::new(1, 3),
    Square::new(1, 4),
    Square::new(1, 5),
    Square::new(1, 6),
    Square::new(1, 7),
    // Black's row, mirroring White's.
    Square::new(2, 0),
    Square::new(2, 1),
    Square::new(2, 2),
    Square::new(2, 3),
    Square::new(2, 6),
    Square::new(2, 7),
];

/// White's path, as indices into [`BOARD`], in the order a piece walks them.
///
/// It runs **backwards along its own row** — a piece enters at column 3 and
/// works left to column 0 — then turns into the shared row at column 0 and runs
/// right to column 7, then turns back into its own row for the last two
/// squares. That is the route the board's own decoration implies and the one
/// Finkel reads; it is also the only route on which the five rosettes fall
/// where a rosette is useful.
const WHITE_PATH: [usize; PATH] = [3, 2, 1, 0, 6, 7, 8, 9, 10, 11, 12, 13, 5, 4];

/// Black's path, the mirror of White's through the middle row.
const BLACK_PATH: [usize; PATH] = [17, 16, 15, 14, 6, 7, 8, 9, 10, 11, 12, 13, 19, 18];

/// The squares a side walks, in order.
pub fn path(side: Side) -> &'static [usize; PATH] {
    match side {
        Side::White => &WHITE_PATH,
        Side::Black => &BLACK_PATH,
    }
}

/// Which board square a side's piece stands on at this many steps, or `None` if
/// it is not on the board at all.
pub fn square_at(side: Side, spot: Spot) -> Option<usize> {
    match spot {
        Spot::Step(step) => Some(path(side)[step as usize - 1]),
        _ => None,
    }
}

/// The five rosettes, as indices into [`BOARD`].
///
/// Landing on one is worth an extra throw, and the middle one is worth more than
/// that: it is the only square in the shared row where a piece cannot be taken.
pub const ROSETTES: [usize; 5] = [0, 9, 4, 14, 18];

/// Whether this board square carries a rosette.
pub fn is_rosette(square: usize) -> bool {
    ROSETTES.contains(&square)
}

/// The one rosette in the shared row, which is the safe square.
///
/// **Named rather than derived.** It is the fourth square of the middle row and
/// also the eighth step of either path, and both of those are true by
/// coincidence of layout rather than by rule. A reader that worked it out from
/// the geometry would keep working if the geometry changed and stop meaning what
/// it means.
pub const SAFE: usize = 9;

/// Whether a square is one of the eight both sides walk.
///
/// This is the question capture asks, and it is asked of the *square* rather
/// than of the step, because the two paths meet on the same squares at the same
/// step numbers and that coincidence is not something to rely on.
pub fn is_shared(square: usize) -> bool {
    (6..=13).contains(&square)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_square_exists_and_no_side_repeats_one() {
        for side in [Side::White, Side::Black] {
            let walk = path(side);
            for &square in walk.iter() {
                assert!(
                    square < SQUARES,
                    "{side:?} walks square {square}, which is not on the board"
                );
            }
            let mut seen = walk.to_vec();
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), PATH, "{side:?} walks a square twice");
        }
    }

    #[test]
    fn the_two_paths_share_exactly_the_middle_row() {
        let white: Vec<usize> = WHITE_PATH.to_vec();
        let shared: Vec<usize> = BLACK_PATH
            .iter()
            .copied()
            .filter(|s| white.contains(s))
            .collect();
        assert_eq!(
            shared,
            vec![6, 7, 8, 9, 10, 11, 12, 13],
            "the shared stretch is not the middle row"
        );
        assert_eq!(shared.len(), 8);
        for &square in &shared {
            assert!(
                is_shared(square),
                "{square} is walked by both and is_shared says no"
            );
        }
        for square in 0..SQUARES {
            if !shared.contains(&square) {
                assert!(
                    !is_shared(square),
                    "{square} is private and is_shared says yes"
                );
            }
        }
    }

    #[test]
    fn both_sides_meet_the_shared_squares_at_the_same_step() {
        // Relied on by nothing — asserted so that if it ever stops being true
        // the tests say so rather than a capture quietly going wrong.
        for step in 0..PATH {
            let white = WHITE_PATH[step];
            if is_shared(white) {
                assert_eq!(
                    BLACK_PATH[step], white,
                    "the paths diverge over the shared row at step {step}"
                );
            }
        }
    }

    #[test]
    fn the_rosettes_are_five_and_two_are_private_to_each_side() {
        assert_eq!(ROSETTES.len(), 5);
        let mut sorted = ROSETTES;
        sorted.sort_unstable();
        let mut deduped = sorted.to_vec();
        deduped.dedup();
        assert_eq!(deduped.len(), 5, "a rosette is listed twice");

        let shared: Vec<usize> = ROSETTES.iter().copied().filter(|&s| is_shared(s)).collect();
        assert_eq!(
            shared,
            vec![SAFE],
            "exactly one rosette is in the shared row"
        );

        for side in [Side::White, Side::Black] {
            let own = path(side)
                .iter()
                .filter(|&&s| is_rosette(s) && !is_shared(s))
                .count();
            assert_eq!(own, 2, "{side:?} should walk two private rosettes");
        }
    }

    #[test]
    fn a_side_walks_three_rosettes_and_they_are_where_the_rules_say() {
        for side in [Side::White, Side::Black] {
            let steps: Vec<usize> = path(side)
                .iter()
                .enumerate()
                .filter(|&(_, &square)| is_rosette(square))
                .map(|(step, _)| step + 1)
                .collect();
            assert_eq!(
                steps,
                vec![4, 8, 14],
                "{side:?}'s rosettes are not at steps 4, 8 and 14"
            );
        }
    }

    #[test]
    fn the_safe_square_is_the_middle_rosette_and_is_shared() {
        assert!(is_rosette(SAFE));
        assert!(is_shared(SAFE));
        assert_eq!(BOARD[SAFE], Square::new(1, 3));
    }

    #[test]
    fn the_board_is_twenty_squares_with_the_waist_missing() {
        assert_eq!(BOARD.len(), SQUARES);
        for square in BOARD.iter() {
            assert!((square.row as usize) < ROWS);
            assert!((square.column as usize) < COLUMNS);
            if square.row != 1 {
                assert!(
                    square.column != 4 && square.column != 5,
                    "the waist at row {} column {} should not exist",
                    square.row,
                    square.column
                );
            }
        }
        let mut seen: Vec<(u8, u8)> = BOARD.iter().map(|s| (s.row, s.column)).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), SQUARES, "a square is listed twice");
    }

    #[test]
    fn a_spot_advances_to_off_only_on_an_exact_throw() {
        assert_eq!(Spot::Waiting.advance(1), Some(Spot::Step(1)));
        assert_eq!(Spot::Step(PATH as u8).advance(1), Some(Spot::Off));
        assert_eq!(
            Spot::Step(PATH as u8).advance(2),
            None,
            "overshooting should not bear off"
        );
        assert_eq!(Spot::Step(PATH as u8 - 1).advance(3), None);
        assert_eq!(Spot::Step(PATH as u8 - 1).advance(2), Some(Spot::Off));
    }

    #[test]
    fn square_at_answers_only_for_pieces_on_the_board() {
        assert_eq!(square_at(Side::White, Spot::Waiting), None);
        assert_eq!(square_at(Side::White, Spot::Off), None);
        assert_eq!(square_at(Side::White, Spot::Step(1)), Some(3));
        assert_eq!(square_at(Side::White, Spot::Step(4)), Some(0));
        assert_eq!(square_at(Side::Black, Spot::Step(1)), Some(17));
        assert_eq!(square_at(Side::Black, Spot::Step(8)), Some(SAFE));
    }
}
