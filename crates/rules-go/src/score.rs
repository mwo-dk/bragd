//! Counting the board, both ways.
//!
//! **Which stones are dead is an argument between two people, and this crate
//! will not have an opinion about it.** At the end of a game the stones still
//! standing are not all alive: some are inside the opponent's territory with no
//! way of living, and everyone agrees they come off without anybody having to
//! play the moves. Which ones those are is agreed between the players, and if
//! they cannot agree the rules say to play it out. No server can decide it —
//! any evaluation strong enough to be right would still be a guess in exactly
//! the positions the players are arguing about, and being confidently wrong
//! about a life-and-death fight is worse than asking.
//!
//! So the dead set is an **argument to these functions**, not something they
//! work out. Collecting it — offering, countering, resuming play — is a
//! protocol between two people and lives one layer up, in
//! [`crate::game::Entry`]; timing it out is Bragd's, because the engine has
//! no clock. The job here is smaller than any of that: given this board and
//! this set of dead stones, here is the score.
//!
//! The two methods differ in what they count, not in how the empty points are
//! shared out, so the walk over the board is written once:
//!
//! - **Area**, Chinese: your stones, plus the empty points only you surround.
//! - **Territory**, Japanese: the empty points only you surround, plus every
//!   stone of the opponent's you have taken — including the dead ones, which
//!   become prisoners as they come off.
//!
//! An empty region touching both colours is **dame**: neutral, nobody's, and
//! counted for neither. Handing it to the player who happens to have more
//! stones round it is the mistake that makes a half-finished board score as a
//! blowout.

use crate::board::{Board, Colour, MAX_POINTS, Point};
use core::fmt;

/// A counted board. Both figures are **twice** the points, so that a
/// half-point komi is an exact integer — see [`crate::ruleset::Ruleset`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Score {
    pub black_halves: i32,
    pub white_halves: i32,
}

impl Score {
    /// Who won, or `None` for *jigo* — a tie. A half-point komi makes a tie
    /// impossible, which is what it is for; with an integer komi, or none, it
    /// is a real result and not an error.
    pub fn winner(&self) -> Option<Colour> {
        match self.black_halves.cmp(&self.white_halves) {
            core::cmp::Ordering::Greater => Some(Colour::Black),
            core::cmp::Ordering::Less => Some(Colour::White),
            core::cmp::Ordering::Equal => None,
        }
    }

    /// How much the winner won by, doubled. Zero for a tie.
    pub fn margin_halves(&self) -> i32 {
        (self.black_halves - self.white_halves).abs()
    }
}

impl fmt::Display for Score {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "B {} W {}",
            points(self.black_halves),
            points(self.white_halves)
        )
    }
}

/// Halves written back as points: `13` becomes `6.5`.
fn points(halves: i32) -> String {
    let whole = halves.div_euclid(2);
    match halves.rem_euclid(2) {
        0 => format!("{whole}"),
        _ => format!("{whole}.5"),
    }
}

/// Chinese counting: stones on the board plus the empty points you surround,
/// and komi to White.
pub fn area(board: &Board, dead: &[Point], komi_halves: i16) -> Score {
    // How many stones came off is not asked for: a dead stone is worth the
    // point it stood on and nothing more. That it is worth *twice* that under
    // territory counting is the whole reason the two methods can disagree.
    let (board, _) = without_the_dead(board, dead);
    let counted = count(&board);
    Score {
        black_halves: 2 * (counted.stones[0] + counted.territory[0]) as i32,
        white_halves: 2 * (counted.stones[1] + counted.territory[1]) as i32 + komi_halves as i32,
    }
}

/// Japanese counting: the empty points you surround plus the stones you have
/// taken, and komi to White.
///
/// `prisoners` is indexed by the colour of the stone that was taken, as
/// [`crate::position::Position::prisoners`] returns it — so Black's score picks
/// up the *white* entry. A dead stone counts twice over: it comes off, which
/// turns the point it stood on into its captor's territory, and it is handed
/// over as a prisoner.
pub fn territory(board: &Board, dead: &[Point], prisoners: [u16; 2], komi_halves: i16) -> Score {
    let (board, lifted) = without_the_dead(board, dead);
    let counted = count(&board);
    let black = counted.territory[0] + (prisoners[1] + lifted[1]) as u32;
    let white = counted.territory[1] + (prisoners[0] + lifted[0]) as u32;
    Score {
        black_halves: 2 * black as i32,
        white_halves: 2 * white as i32 + komi_halves as i32,
    }
}

/// The board with the dead stones taken off, and how many of each colour that
/// was. A dead point with nothing on it is ignored rather than rejected: the
/// set comes from two people agreeing, and a stone one of them names may
/// already have been captured.
fn without_the_dead(board: &Board, dead: &[Point]) -> (Board, [u16; 2]) {
    let mut lifted = board.clone();
    let mut removed = [0u16; 2];
    for &point in dead {
        if let Some(colour) = lifted.at(point) {
            removed[colour.index()] += 1;
            lifted.set(point, None);
        }
    }
    (lifted, removed)
}

struct Counted {
    stones: [u32; 2],
    territory: [u32; 2],
}

/// One walk of the whole board: stones counted where they stand, and every
/// empty region flooded to see who is round it.
///
/// The region is collected into a `Vec`, which is the one place in this crate
/// that allocates. It is deliberate: counting happens once, at the end of a
/// game, off the hot path entirely, and a region can be most of the board.
fn count(board: &Board) -> Counted {
    let mut counted = Counted {
        stones: [0, 0],
        territory: [0, 0],
    };
    let mut seen = [false; MAX_POINTS];
    let mut region: Vec<Point> = Vec::new();

    for point in board.points() {
        if let Some(colour) = board.at(point) {
            counted.stones[colour.index()] += 1;
            continue;
        }
        if seen[board.index(point)] {
            continue;
        }

        let mut borders = [false; 2];
        region.clear();
        region.push(point);
        seen[board.index(point)] = true;
        let mut next = 0;
        while next < region.len() {
            let empty = region[next];
            next += 1;
            for neighbour in board.neighbours(empty) {
                match board.at(neighbour) {
                    Some(colour) => borders[colour.index()] = true,
                    None => {
                        let index = board.index(neighbour);
                        if !seen[index] {
                            seen[index] = true;
                            region.push(neighbour);
                        }
                    }
                }
            }
        }

        // Both colours round it, or neither — on a board with no stones at
        // all — and it is nobody's.
        match (borders[0], borders[1]) {
            (true, false) => counted.territory[0] += region.len() as u32,
            (false, true) => counted.territory[1] += region.len() as u32,
            _ => {}
        }
    }
    counted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;

    fn point(text: &str) -> Point {
        Point::parse(text).expect("a point")
    }

    /// A 9×9 board split down the middle: a black wall on the d file, a white
    /// wall on the e file, and nothing else. Every empty point is behind one
    /// wall or the other, so there is no dame anywhere.
    fn split() -> Position {
        Position::from_picture(
            &[
                "...XO....",
                "...XO....",
                "...XO....",
                "...XO....",
                "...XO....",
                "...XO....",
                "...XO....",
                "...XO....",
                "...XO....",
            ],
            Colour::Black,
        )
    }

    #[test]
    fn area_scoring_shares_out_every_point_when_nothing_is_dead_and_nothing_is_neutral() {
        let position = split();
        let score = area(position.board(), &[], 0);
        assert_eq!(
            score.black_halves,
            2 * 36,
            "the d file and everything left of it"
        );
        assert_eq!(
            score.white_halves,
            2 * 45,
            "the e file and everything right of it"
        );
        assert_eq!(
            score.black_halves + score.white_halves,
            2 * position.board().intersections() as i32
        );
    }

    #[test]
    fn territory_scoring_counts_the_empty_points_and_not_the_stones() {
        // The same board. Black's nine wall stones are worth nothing to
        // Japanese counting and everything to Chinese counting, which is the
        // whole difference between the two.
        let position = split();
        let score = territory(position.board(), &[], [0, 0], 0);
        assert_eq!(score.black_halves, 2 * 27);
        assert_eq!(score.white_halves, 2 * 36);
    }

    #[test]
    fn a_region_touching_both_colours_is_neutral() {
        // One gap in each wall, so the two halves join into a single region
        // that both colours touch. Under area scoring the players keep their
        // stones and nothing else.
        let position = Position::from_picture(
            &[
                "...XO....",
                "...XO....",
                "...XO....",
                "...XO....",
                ".........",
                "...XO....",
                "...XO....",
                "...XO....",
                "...XO....",
            ],
            Colour::Black,
        );
        let score = area(position.board(), &[], 0);
        assert_eq!(score.black_halves, 2 * 8, "eight stones and no territory");
        assert_eq!(score.white_halves, 2 * 8);
    }

    #[test]
    fn a_dead_stone_becomes_territory_and_a_prisoner_at_once() {
        // One white stone sitting inside Black's area, agreed dead. Under
        // territory scoring Black gains the point it stood on *and* the stone
        // itself; under area scoring only the point.
        let position = Position::from_picture(
            &[
                "...XO....",
                "...XO....",
                "...XO....",
                "...XO....",
                ".O.XO....",
                "...XO....",
                "...XO....",
                "...XO....",
                "...XO....",
            ],
            Colour::Black,
        );
        let dead = [point("b5")];

        let counted = territory(position.board(), &dead, [0, 0], 0);
        assert_eq!(
            counted.black_halves,
            2 * (27 + 1),
            "the point and the prisoner"
        );
        assert_eq!(counted.white_halves, 2 * 36);

        let by_area = area(position.board(), &dead, 0);
        assert_eq!(by_area.black_halves, 2 * 36, "the point, and no prisoner");
    }

    #[test]
    fn komi_goes_to_white_and_cannot_be_tied() {
        let position = split();
        let score = area(position.board(), &[], 13);
        assert_eq!(score.white_halves, 2 * 45 + 13);
        assert_eq!(score.winner(), Some(Colour::White));
        assert_eq!(score.margin_halves(), 2 * 9 + 13);
        assert_eq!(score.to_string(), "B 36 W 51.5");
    }

    #[test]
    fn an_empty_board_belongs_to_nobody() {
        let position = Position::empty(9).expect("a board");
        let score = area(position.board(), &[], 0);
        assert_eq!(score.black_halves, 0);
        assert_eq!(score.white_halves, 0);
        assert_eq!(score.winner(), None, "jigo, with no komi to break it");
    }
}
