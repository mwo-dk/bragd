//! The boards, the paths across them, the rosettes, and the rule sets that use them.
//!
//! Ur is the first game in Bragd where **the two players do not share a
//! board in the sense the other families mean it.** A chess square is the same
//! square to both sides; here, square 0 is on White's own row and Black can
//! never stand on it. Some squares are shared and some are private, and every
//! rule that matters — capture, safety, blocking — is a rule about which of
//! those two kinds a square is.
//!
//! So the geometry is written down once per rule set, here, as two paths, and
//! everything else asks [`Rules`] a question rather than knowing the answer.
//! A rule set is data, not code: the four rule sets Bragd plays differ only in
//! the board, the paths, the number of pieces and dice, and three switches.

use crate::dice::Throw;

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

    fn index(self) -> usize {
        match self {
            Side::White => 0,
            Side::Black => 1,
        }
    }
}

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
/// positions that differ only in which slot holds which piece are the same
/// position, and sorting is what makes `==` say so.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Spot {
    /// Waiting to enter. Step zero.
    Waiting,
    /// On the board, at this many steps along its owner's path (1..=path length).
    Step(u8),
    /// Home, and out of the game.
    Off,
}

/// A square of the board, as a row and a column, so that a drawing can place it
/// without knowing anything about paths.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Square {
    pub row: u8,
    pub column: u8,
}

const fn sq(row: u8, column: u8) -> Square {
    Square { row, column }
}

/// One rule set: a board, the two paths across it, and the rules that differ
/// between the reconstructions. Sources for each are in `docs/rules/ur.md`.
#[derive(PartialEq, Eq, Debug)]
pub struct Rules {
    /// Its name in the code and the docs: `finkel`, `masters`, `blitz`, `aseb`.
    pub name: &'static str,
    /// The squares, in a fixed order that the wire and the picture use as an index.
    pub board: &'static [Square],
    /// The columns and rows the board spans, for a drawing.
    pub columns: u8,
    pub rows: u8,
    /// White's path and Black's, as indices into `board`, in the order a piece walks them.
    pub(crate) paths: [&'static [usize]; 2],
    /// The rosettes, as indices into `board`. Landing on one earns another throw.
    pub rosettes: &'static [usize],
    /// How many pieces each side brings.
    pub pieces: u8,
    /// How many tetrahedral dice are thrown. A throw counts their marked tips up.
    pub dice: u8,
    /// How many steps a throw of nought moves: nothing, or (with three dice) four.
    pub nought: u8,
    /// Whether a piece on a shared rosette cannot be taken.
    pub safe_rosettes: bool,
    /// Whether taking a piece earns another throw.
    pub capture_again: bool,
}

/// Finkel's reconstruction: the board found at Ur, R. C. Bell's path of fourteen
/// squares, seven pieces, four dice, and the middle rosette safe.
pub const FINKEL: Rules = Rules {
    name: "finkel",
    board: &STANDARD,
    columns: 8,
    rows: 3,
    paths: [&BELL_WHITE, &BELL_BLACK],
    rosettes: &STANDARD_ROSETTES,
    pieces: 7,
    dice: 4,
    nought: 0,
    safe_rosettes: true,
    capture_again: false,
};

/// James Masters' path of sixteen squares, a rosette every fourth; three dice, a
/// throw of nought moves four, and no square is safe.
pub const MASTERS: Rules = Rules {
    name: "masters",
    board: &STANDARD,
    columns: 8,
    rows: 3,
    paths: [&MASTERS_WHITE, &MASTERS_BLACK],
    rosettes: &STANDARD_ROSETTES,
    pieces: 7,
    dice: 3,
    nought: 4,
    safe_rosettes: false,
    capture_again: false,
};

/// RoyalUr.net's Blitz: Masters' path, four dice, five pieces, no safe square,
/// and a capture earns another throw.
pub const BLITZ: Rules = Rules {
    name: "blitz",
    board: &STANDARD,
    columns: 8,
    rows: 3,
    paths: [&MASTERS_WHITE, &MASTERS_BLACK],
    rosettes: &STANDARD_ROSETTES,
    pieces: 5,
    dice: 4,
    nought: 0,
    safe_rosettes: false,
    capture_again: true,
};

/// Aseb, the game of twenty squares on its own board: four squares to each side,
/// twelve shared, five rosettes, five pieces, four dice.
pub const ASEB: Rules = Rules {
    name: "aseb",
    board: &ASEB_BOARD,
    columns: 12,
    rows: 3,
    paths: [&ASEB_WHITE, &ASEB_BLACK],
    rosettes: &ASEB_ROSETTES,
    pieces: 5,
    dice: 4,
    nought: 0,
    safe_rosettes: true,
    capture_again: false,
};

/// Every rule set, Finkel's first.
pub const ALL: [&Rules; 4] = [&FINKEL, &MASTERS, &BLITZ, &ASEB];

impl Rules {
    /// The squares a side walks, in order.
    pub fn path(&self, side: Side) -> &'static [usize] {
        self.paths[side.index()]
    }

    /// How many steps a piece takes from entering to its last square.
    pub fn path_len(&self) -> u8 {
        self.paths[0].len() as u8
    }

    /// How far along the path a spot is, counting `Waiting` as nought and `Off` as
    /// one past the end. This is the number the arithmetic and the wire use.
    pub fn distance(&self, spot: Spot) -> u8 {
        match spot {
            Spot::Waiting => 0,
            Spot::Step(step) => step,
            Spot::Off => self.path_len() + 1,
        }
    }

    /// The spot at this distance, if there is one.
    pub fn spot(&self, distance: u8) -> Option<Spot> {
        match distance {
            0 => Some(Spot::Waiting),
            step if step <= self.path_len() => Some(Spot::Step(step)),
            step if step == self.path_len() + 1 => Some(Spot::Off),
            _ => None,
        }
    }

    /// The spot this many steps further on, or `None` if that would overshoot
    /// bearing off. **An exact throw is required to come home** in every rule set
    /// Bragd plays.
    pub fn advance(&self, spot: Spot, steps: u8) -> Option<Spot> {
        self.spot(self.distance(spot).checked_add(steps)?)
    }

    /// Which board square a side's piece stands on, or `None` if it is not on the
    /// board at all.
    pub fn square_at(&self, side: Side, spot: Spot) -> Option<usize> {
        match spot {
            Spot::Step(step) => Some(self.path(side)[step as usize - 1]),
            _ => None,
        }
    }

    /// Whether this board square carries a rosette.
    pub fn is_rosette(&self, square: usize) -> bool {
        self.rosettes.contains(&square)
    }

    /// Whether both sides walk this square. This is the question capture asks, and
    /// it is asked of the *square*: on Masters' path the sides reach the same
    /// square at different steps.
    pub fn is_shared(&self, square: usize) -> bool {
        self.paths[0].contains(&square) && self.paths[1].contains(&square)
    }

    /// Whether a piece on this square cannot be taken.
    pub fn is_safe(&self, square: usize) -> bool {
        self.safe_rosettes && self.is_rosette(square) && self.is_shared(square)
    }

    /// A throw these dice can make: nought to the number of dice.
    pub fn throw(&self, marks: u8) -> Option<Throw> {
        (marks <= self.dice).then(|| Throw::new(marks)).flatten()
    }

    /// Every throw these dice can make, nought first.
    pub fn throws(&self) -> impl Iterator<Item = Throw> + '_ {
        (0..=self.dice).filter_map(|marks| self.throw(marks))
    }

    /// How many steps a throw moves a piece.
    pub fn steps(&self, throw: Throw) -> u8 {
        match throw.marks() {
            0 => self.nought,
            marks => marks,
        }
    }

    /// How many of the `2^dice` equally likely ways the dice can fall make this
    /// throw: the binomial coefficient, so 1, 4, 6, 4, 1 for four dice.
    pub fn ways(&self, throw: Throw) -> u32 {
        let (n, k) = (self.dice as u32, throw.marks() as u32);
        (0..k).fold(1, |acc, i| acc * (n - i) / (i + 1))
    }
}

/// The board found at Ur: three rows of eight, with the waist missing from the
/// top and bottom rows. Row 0 is White's own row, row 1 the middle, row 2
/// Black's; within a row left to right.
const STANDARD: [Square; 20] = [
    sq(0, 0), sq(0, 1), sq(0, 2), sq(0, 3), sq(0, 6), sq(0, 7),
    sq(1, 0), sq(1, 1), sq(1, 2), sq(1, 3), sq(1, 4), sq(1, 5), sq(1, 6), sq(1, 7),
    sq(2, 0), sq(2, 1), sq(2, 2), sq(2, 3), sq(2, 6), sq(2, 7),
];
const STANDARD_ROSETTES: [usize; 5] = [0, 9, 4, 14, 18];

/// Bell's path, which Finkel plays: backwards along its own row from column 3 to
/// column 0, the whole middle row, then the last two squares of its own row.
const BELL_WHITE: [usize; 14] = [3, 2, 1, 0, 6, 7, 8, 9, 10, 11, 12, 13, 5, 4];
const BELL_BLACK: [usize; 14] = [17, 16, 15, 14, 6, 7, 8, 9, 10, 11, 12, 13, 19, 18];

/// Masters' path: its own four, the middle row to column 6, across the far end
/// through the other side's last two squares, and back into its own.
const MASTERS_WHITE: [usize; 16] = [3, 2, 1, 0, 6, 7, 8, 9, 10, 11, 12, 18, 19, 13, 5, 4];
const MASTERS_BLACK: [usize; 16] = [17, 16, 15, 14, 6, 7, 8, 9, 10, 11, 12, 4, 5, 13, 19, 18];

/// The board of twenty squares: four to each side at one end, and a middle row
/// of twelve.
const ASEB_BOARD: [Square; 20] = [
    sq(0, 0), sq(0, 1), sq(0, 2), sq(0, 3),
    sq(1, 0), sq(1, 1), sq(1, 2), sq(1, 3), sq(1, 4), sq(1, 5),
    sq(1, 6), sq(1, 7), sq(1, 8), sq(1, 9), sq(1, 10), sq(1, 11),
    sq(2, 0), sq(2, 1), sq(2, 2), sq(2, 3),
];
const ASEB_ROSETTES: [usize; 5] = [0, 16, 7, 11, 15];
const ASEB_WHITE: [usize; 16] = [3, 2, 1, 0, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const ASEB_BLACK: [usize; 16] = [19, 18, 17, 16, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

#[cfg(test)]
mod tests {
    use super::*;

    fn rosette_steps(rules: &Rules, side: Side) -> Vec<u8> {
        rules
            .path(side)
            .iter()
            .enumerate()
            .filter(|&(_, &square)| rules.is_rosette(square))
            .map(|(step, _)| step as u8 + 1)
            .collect()
    }

    fn shared(rules: &Rules) -> Vec<usize> {
        let mut squares: Vec<usize> = (0..rules.board.len()).filter(|&s| rules.is_shared(s)).collect();
        squares.sort_unstable();
        squares
    }

    #[test]
    fn every_path_square_exists_and_no_side_repeats_one() {
        for rules in ALL {
            assert_eq!(rules.path(Side::White).len(), rules.path(Side::Black).len());
            for side in [Side::White, Side::Black] {
                let mut seen = rules.path(side).to_vec();
                assert!(seen.iter().all(|&s| s < rules.board.len()), "{} {side:?}", rules.name);
                seen.sort_unstable();
                seen.dedup();
                assert_eq!(seen.len(), rules.path_len() as usize, "{} {side:?} walks a square twice", rules.name);
            }
            for rosette in rules.rosettes {
                assert!(*rosette < rules.board.len());
            }
        }
    }

    #[test]
    fn every_board_has_twenty_distinct_squares_inside_its_frame() {
        for rules in ALL {
            assert_eq!(rules.board.len(), 20, "{}", rules.name);
            let mut seen: Vec<(u8, u8)> = rules.board.iter().map(|s| (s.row, s.column)).collect();
            assert!(rules.board.iter().all(|s| s.row < rules.rows && s.column < rules.columns));
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), 20, "{} lists a square twice", rules.name);
        }
        // The waist of the board found at Ur.
        assert!(!STANDARD.iter().any(|s| s.row != 1 && (s.column == 4 || s.column == 5)));
    }

    #[test]
    fn finkel_shares_the_middle_row_and_keeps_rosettes_at_4_8_14() {
        assert_eq!(FINKEL.path_len(), 14);
        assert_eq!(shared(&FINKEL), (6..=13).collect::<Vec<_>>());
        for side in [Side::White, Side::Black] {
            assert_eq!(rosette_steps(&FINKEL, side), [4, 8, 14]);
        }
        let safe: Vec<usize> = (0..20).filter(|&s| FINKEL.is_safe(s)).collect();
        assert_eq!(safe, [9], "only the middle rosette is safe");
        assert_eq!(FINKEL.board[9], sq(1, 3));
    }

    #[test]
    fn masters_path_is_sixteen_with_a_rosette_every_fourth_and_crosses_the_far_end() {
        for rules in [&MASTERS, &BLITZ] {
            assert_eq!(rules.path_len(), 16);
            for side in [Side::White, Side::Black] {
                assert_eq!(rosette_steps(rules, side), [4, 8, 12, 16], "{}", rules.name);
            }
            // The middle row to column 6, then the far two columns of all three rows.
            assert_eq!(shared(rules), [4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 18, 19]);
            assert!((0..20).all(|s| !rules.is_safe(s)), "no square is safe");
        }
        // White reaches Black's rosette at step 12, where Black stands at step 16.
        assert_eq!(MASTERS.square_at(Side::White, Spot::Step(12)), Some(18));
        assert_eq!(MASTERS.square_at(Side::Black, Spot::Step(16)), Some(18));
    }

    #[test]
    fn aseb_shares_twelve_and_keeps_three_safe_rosettes() {
        assert_eq!(ASEB.path_len(), 16);
        assert_eq!(shared(&ASEB), (4..=15).collect::<Vec<_>>());
        for side in [Side::White, Side::Black] {
            assert_eq!(rosette_steps(&ASEB, side), [4, 8, 12, 16]);
        }
        let safe: Vec<usize> = (0..20).filter(|&s| ASEB.is_safe(s)).collect();
        assert_eq!(safe, [7, 11, 15]);
    }

    #[test]
    fn the_dice_and_what_a_nought_moves() {
        assert_eq!(FINKEL.throws().map(|t| FINKEL.steps(t)).collect::<Vec<_>>(), [0, 1, 2, 3, 4]);
        assert_eq!(MASTERS.throws().map(|t| MASTERS.steps(t)).collect::<Vec<_>>(), [4, 1, 2, 3]);
        assert_eq!(FINKEL.throws().map(|t| FINKEL.ways(t)).collect::<Vec<_>>(), [1, 4, 6, 4, 1]);
        assert_eq!(MASTERS.throws().map(|t| MASTERS.ways(t)).collect::<Vec<_>>(), [1, 3, 3, 1]);
        assert_eq!(MASTERS.throw(4), None, "three dice cannot show four marks");
    }

    #[test]
    fn a_spot_advances_to_off_only_on_an_exact_throw() {
        for rules in ALL {
            let last = rules.path_len();
            assert_eq!(rules.advance(Spot::Waiting, 1), Some(Spot::Step(1)));
            assert_eq!(rules.advance(Spot::Step(last), 1), Some(Spot::Off));
            assert_eq!(rules.advance(Spot::Step(last), 2), None, "overshooting should not bear off");
            assert_eq!(rules.advance(Spot::Step(last - 1), 2), Some(Spot::Off));
        }
    }

    #[test]
    fn square_at_answers_only_for_pieces_on_the_board() {
        assert_eq!(FINKEL.square_at(Side::White, Spot::Waiting), None);
        assert_eq!(FINKEL.square_at(Side::White, Spot::Off), None);
        assert_eq!(FINKEL.square_at(Side::White, Spot::Step(1)), Some(3));
        assert_eq!(FINKEL.square_at(Side::Black, Spot::Step(8)), Some(9));
    }
}
