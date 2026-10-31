//! The board: two sides, twenty-four points, and the numbering everything else
//! is written in.
//!
//! **Each player counts the points from their own end.** A player writing
//! `13/11` means their own 13-point to their own 11-point, and the opponent
//! calls those same two places their 12-point and their 14-point. Nobody uses a
//! neutral numbering, and inventing one would mean every line of notation had
//! to be translated before a human could read it — which is exactly the mistake
//! that makes a record unreadable to the people who play the game.
//!
//! So the array is absolute and fixed, index 0 to 23, and the numbering is a
//! *view* onto it: [`index_of`] turns a player's own point number into an array
//! index, [`number_at`] turns one back. Index 0 is White's 1-point and Black's
//! 24-point. White walks the indices downwards and bears off past 0; Black walks
//! them upwards and bears off past 23.
//!
//! That mirror is the one place in this crate where a sign error can hide, which
//! is why it is two named functions with a test rather than arithmetic sprinkled
//! through the movement code.

/// How many points the board has.
pub const POINTS: usize = 24;

/// How many checkers each side owns. It never changes: a checker is on a point,
/// on the bar, or borne off, and there is no fourth place for one to be.
pub const CHECKERS: u8 = 15;

/// The size of a home board, in points. A side's home is its points 1 to 6.
pub const HOME: u8 = 6;

/// The point number the bar counts as, for entering and for pip counts.
///
/// A checker entering with a 5 lands on its owner's 20-point, which is
/// `BAR_POINT - 5`. Writing it this way rather than as `25 - die` in three
/// places is the difference between a rule and a magic number.
pub const BAR_POINT: u8 = 25;

/// Which player. Backgammon's sides really are colours, unlike tafl's, so they
/// are named as such — and they line up with the platform's two seats without a
/// mapping.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Side {
    White,
    Black,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::White, Side::Black];

    pub const fn other(self) -> Side {
        match self {
            Side::White => Side::Black,
            Side::Black => Side::White,
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Side::White => 0,
            Side::Black => 1,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Side::White => "white",
            Side::Black => "black",
        }
    }
}

impl core::fmt::Display for Side {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// The array index of `side`'s point `number`, which must be 1 to 24.
///
/// White's numbering runs with the indices and Black's runs against them, so the
/// two conversions are the same expression per side and are their own inverses.
pub const fn index_of(side: Side, number: u8) -> usize {
    match side {
        Side::White => number as usize - 1,
        Side::Black => POINTS - number as usize,
    }
}

/// What `index` is called in `side`'s own numbering.
pub const fn number_at(side: Side, index: usize) -> u8 {
    match side {
        Side::White => index as u8 + 1,
        Side::Black => (POINTS - index) as u8,
    }
}

/// What stands on a point: an owner and a count.
///
/// The compact spelling is one signed integer per point — positive for White,
/// negative for Black — and it is what most engines use. It is rejected here
/// because it gives the empty point two spellings and turns every read into a
/// sign test that the compiler cannot check you got the right way round. This is
/// two bytes instead of one, a whole position still copies in a few
/// instructions, and the invariant that a point with no checkers has no owner is
/// enforced by the constructor rather than remembered by the reader.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Point {
    owner: Option<Side>,
    count: u8,
}

impl Point {
    pub const EMPTY: Point = Point {
        owner: None,
        count: 0,
    };

    /// `count` checkers belonging to `side`. Zero of them is the empty point,
    /// which is the invariant this constructor exists to keep.
    pub const fn of(side: Side, count: u8) -> Point {
        if count == 0 {
            Point::EMPTY
        } else {
            Point {
                owner: Some(side),
                count,
            }
        }
    }

    pub const fn owner(self) -> Option<Side> {
        self.owner
    }

    pub const fn count(self) -> u8 {
        self.count
    }

    pub const fn is_empty(self) -> bool {
        self.count == 0
    }

    /// A single checker, alone and exposed. Hitting one is the whole reason
    /// backgammon is not a race.
    pub const fn is_blot(self) -> bool {
        self.count == 1
    }

    pub fn held_by(self, side: Side) -> bool {
        self.owner == Some(side)
    }

    /// Whether `side` may land here: empty, theirs, or a lone enemy blot.
    ///
    /// **Two enemy checkers close a point**, and there is no capture-by-numbers
    /// anywhere in the game — a stack of five is no easier to land on than a
    /// stack of two.
    pub fn is_open_to(self, side: Side) -> bool {
        match self.owner {
            None => true,
            Some(owner) => owner == side || self.count == 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_numberings_are_mirror_images() {
        for index in 0..POINTS {
            let white = number_at(Side::White, index);
            let black = number_at(Side::Black, index);
            assert_eq!(
                white + black,
                25,
                "index {index} is White's {white} and Black's {black}"
            );
            assert_eq!(index_of(Side::White, white), index);
            assert_eq!(index_of(Side::Black, black), index);
        }
    }

    #[test]
    fn each_sides_home_is_the_others_outfield() {
        // The two home boards do not overlap, which is what makes an opening
        // checker on your 24-point a checker sitting in your opponent's home.
        for number in 1..=HOME {
            assert_ne!(
                index_of(Side::White, number),
                index_of(Side::Black, number),
                "the {number}-points collided"
            );
        }
        assert_eq!(index_of(Side::White, 24), index_of(Side::Black, 1));
    }

    #[test]
    fn a_point_with_no_checkers_has_no_owner() {
        assert_eq!(Point::of(Side::White, 0), Point::EMPTY);
        assert_eq!(Point::of(Side::Black, 0).owner(), None);
        assert!(Point::of(Side::White, 0).is_empty());
    }

    #[test]
    fn a_lone_enemy_checker_leaves_the_point_open() {
        let blot = Point::of(Side::Black, 1);
        assert!(blot.is_blot());
        assert!(blot.is_open_to(Side::White), "a blot can be hit");

        let made = Point::of(Side::Black, 2);
        assert!(!made.is_blot());
        assert!(!made.is_open_to(Side::White), "two checkers close a point");
        assert!(made.is_open_to(Side::Black));

        // And a stack of five is exactly as closed as a stack of two.
        assert!(!Point::of(Side::Black, 5).is_open_to(Side::White));
    }
}
