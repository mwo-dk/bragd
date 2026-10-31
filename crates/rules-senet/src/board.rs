//! The thirty houses, the five that mean something, and the snake.
//!
//! **The path is the game and the grid is only for drawing.** A senet board is
//! three rows of ten walked boustrophedon — left to right, then right to left,
//! then left to right — so house 11 sits directly under house 10 and house 21
//! directly under house 20. Everything in this crate counts in houses; the row
//! and column are worked out once, here, for a component that has to put a
//! rectangle somewhere.
//!
//! Getting that the wrong way round is the characteristic bug of this family:
//! a board stored in grid order plays a game where the bends are wrong, and it
//! looks completely normal.

/// A side. Named for the seats, not for a colour — Kendall paints them white
/// and black, and Bragd's own word is Side.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Side {
    White,
    Black,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::White => Side::Black,
            Side::Black => Side::White,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Side::White => "white",
            Side::Black => "black",
        }
    }

    /// Reads a seat back. `None` rather than a default, because guessing puts
    /// the wrong player on move in a real Game.
    pub fn parse(text: &str) -> Option<Side> {
        match text {
            "white" => Some(Side::White),
            "black" => Some(Side::Black),
            _ => None,
        }
    }
}

/// How many pieces each side brings.
///
/// **Seven, and this is the number most often got wrong.** Popular summaries
/// regularly attribute five pieces on houses 1–10 to Kendall; the transcriptions
/// say the opposite, and five-on-1–10 belongs to Jéquier and the modern sets.
/// `docs/rules/ur-and-senet-rules.md` has the citation and the warning.
pub const PIECES: u8 = 7;

/// The houses, numbered 1..=30 along the path.
pub const HOUSES: usize = 30;

pub const ROWS: usize = 3;
pub const COLUMNS: usize = 10;

/// Where the drowned come back to, when it is free.
pub const REBIRTH: u8 = 15;

/// The House of Beauty. Nothing may pass it; a piece must land on it exactly.
pub const BEAUTY: u8 = 26;

/// The House of Water. The trap.
pub const WATER: u8 = 27;

/// The three houses a piece leaves the board from, and the exact throw each
/// needs. House 30 is the exception and takes anything.
pub const THREE_TRUTHS: u8 = 28;
pub const RE_ATOUM: u8 = 29;
pub const LAST: u8 = 30;

/// A house's row and column, for drawing only.
///
/// Rows count from the top and columns from the left. The middle row runs
/// backwards, which is the whole of the boustrophedon and the reason this
/// function exists rather than a division.
pub fn grid(house: u8) -> Option<(u8, u8)> {
    if !(1..=HOUSES as u8).contains(&house) {
        return None;
    }
    let index = house - 1;
    let row = index / COLUMNS as u8;
    let along = index % COLUMNS as u8;
    // Rows 0 and 2 run left to right; row 1 runs right to left.
    let column = if row == 1 {
        COLUMNS as u8 - 1 - along
    } else {
        along
    };
    Some((row, column))
}

/// Whether two houses are next to each other **along the path**.
///
/// Consecutive numbers, bends included: 10 and 11 are adjacent, and so are 20
/// and 21, because a piece walks from one to the other in a single step. This is
/// the reading protection uses, and it is a decision rather than a lookup — the
/// booklet says "stood next to" and does not say whether it means the path or
/// the grid. `docs/rules/ur-and-senet-rules.md` names it as ours.
///
/// On the physical board the two happen to coincide at the bends, which is why
/// the choice is easy to make without noticing it is a choice.
pub fn adjacent(a: u8, b: u8) -> bool {
    a.abs_diff(b) == 1
}

/// What a house is called, for a front end and for a screen reader.
pub fn house_name(house: u8) -> Option<&'static str> {
    match house {
        REBIRTH => Some("the House of Rebirth"),
        BEAUTY => Some("the House of Beauty"),
        WATER => Some("the House of Water"),
        THREE_TRUTHS => Some("the House of the Three Truths"),
        RE_ATOUM => Some("the House of Re-Atoum"),
        LAST => Some("the last house"),
        _ => None,
    }
}

/// The exact throw that bears a piece off from this house, if it needs one.
///
/// The arithmetic is `31 - house` throughout — 26 needs a 5, 27 needs a 4, 28 a
/// 3, 29 a 2 — which is one rule rather than four coincidences, and it is why
/// the water's exit is a 4 rather than something the water invented. House 30 is
/// the exception: it leaves on anything.
pub fn exact_exit(house: u8) -> Option<u8> {
    match house {
        LAST => None,
        house if (BEAUTY..LAST).contains(&house) => Some(31 - house),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_snakes_and_the_bends_line_up() {
        // House 1 is the top left; 10 is the top right; 11 is directly under 10;
        // 20 is under 1; 21 is under 20; 30 is the bottom right.
        assert_eq!(grid(1), Some((0, 0)));
        assert_eq!(grid(10), Some((0, 9)));
        assert_eq!(grid(11), Some((1, 9)), "11 sits under 10");
        assert_eq!(grid(20), Some((1, 0)), "20 sits under 1");
        assert_eq!(grid(21), Some((2, 0)), "21 sits under 20");
        assert_eq!(grid(30), Some((2, 9)));
    }

    #[test]
    fn every_house_has_a_place_and_no_two_share_one() {
        let mut seen = Vec::new();
        for house in 1..=HOUSES as u8 {
            let at = grid(house).unwrap_or_else(|| panic!("house {house} has no place"));
            assert!((at.0 as usize) < ROWS);
            assert!((at.1 as usize) < COLUMNS);
            seen.push(at);
        }
        assert_eq!(seen.len(), HOUSES, "the loop did not run over every house");
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), HOUSES, "two houses share a place");
    }

    #[test]
    fn nothing_outside_one_to_thirty_is_a_house() {
        assert_eq!(grid(0), None);
        assert_eq!(grid(31), None);
        assert_eq!(grid(255), None);
    }

    #[test]
    fn consecutive_houses_are_always_physically_touching() {
        // The claim the boustrophedon makes, checked rather than assumed: a
        // piece's every step is onto a square it could reach with a finger.
        let mut checked = 0;
        for house in 1..HOUSES as u8 {
            let (row, column) = grid(house).unwrap();
            let (next_row, next_column) = grid(house + 1).unwrap();
            let apart = row.abs_diff(next_row) + column.abs_diff(next_column);
            assert_eq!(apart, 1, "{house} and {} are not touching", house + 1);
            checked += 1;
        }
        assert_eq!(checked, HOUSES - 1);
    }

    #[test]
    fn adjacency_is_along_the_path_including_the_bends() {
        assert!(adjacent(10, 11), "the first bend must protect");
        assert!(adjacent(20, 21), "the second bend must protect");
        assert!(adjacent(4, 5));
        assert!(adjacent(5, 4), "adjacency has no direction");
        assert!(!adjacent(4, 6));
        assert!(
            !adjacent(1, 20),
            "the grid puts these in one column; the path does not"
        );
        assert!(!adjacent(7, 7), "a house is not next to itself");
    }

    #[test]
    fn the_named_houses_are_where_the_rules_say() {
        assert_eq!(REBIRTH, 15);
        assert_eq!(BEAUTY, 26);
        assert_eq!(WATER, 27);
        assert_eq!(THREE_TRUTHS, 28);
        assert_eq!(RE_ATOUM, 29);
        assert_eq!(LAST, 30);
        for house in [REBIRTH, BEAUTY, WATER, THREE_TRUTHS, RE_ATOUM, LAST] {
            assert!(house_name(house).is_some(), "{house} should be named");
        }
        assert_eq!(house_name(1), None);
        assert_eq!(house_name(25), None);
    }

    #[test]
    fn the_exits_are_one_rule_and_not_four_coincidences() {
        assert_eq!(exact_exit(BEAUTY), Some(5));
        assert_eq!(exact_exit(WATER), Some(4));
        assert_eq!(exact_exit(THREE_TRUTHS), Some(3));
        assert_eq!(exact_exit(RE_ATOUM), Some(2));
        assert_eq!(exact_exit(LAST), None, "the last house takes any throw");
        for house in 1..BEAUTY {
            assert_eq!(exact_exit(house), None, "{house} is not an exit");
        }
        // And the rule itself, stated once more as arithmetic.
        for house in BEAUTY..LAST {
            assert_eq!(exact_exit(house), Some(31 - house));
        }
    }
}
