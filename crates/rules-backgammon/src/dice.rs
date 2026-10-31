//! The dice, which this crate never throws.
//!
//! **A roll is an input, not an event**, and that is the reason the whole
//! crate is shaped the way it is. The caller throws the dice and writes
//! them into the record as an entry of their own, before either player has seen
//! them ([`crate::Game::roll`]); the turn played with them is the entry after.
//! Nothing here reads a clock, seeds a generator, or depends on anything but its
//! arguments, so replaying a recorded game years later produces the same game.
//!
//! The wrong answer is a generator in the engine. It fails *silently*: nothing
//! breaks, no test goes red, and the damage appears the first time somebody
//! opens an old game and finds a different one.
//!
//! The opening roll — one die each, higher goes first, equal is thrown again —
//! is not modelled here either. It decides who moves first, which is a fact
//! about the Game rather than about the position, so the platform decides it and
//! hands the engine the resulting roll and the side to play it.

use core::fmt;

/// Two dice, kept high first.
///
/// **Order is dropped deliberately.** Two dice are thrown together; which one
/// came to rest first is not information anybody can act on, and normalising
/// means `5-3` and `3-5` compare equal instead of producing two spellings of one
/// roll in a Library. The rule about being made to play the larger die needs
/// "the larger" to be a cheap question, and this is what makes it one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Roll {
    high: u8,
    low: u8,
}

impl Roll {
    /// A roll, or `None` if either die is not a die.
    pub fn new(one: u8, two: u8) -> Option<Roll> {
        if !(1..=6).contains(&one) || !(1..=6).contains(&two) {
            return None;
        }
        Some(Roll {
            high: one.max(two),
            low: one.min(two),
        })
    }

    pub const fn high(self) -> u8 {
        self.high
    }

    pub const fn low(self) -> u8 {
        self.low
    }

    pub const fn is_double(self) -> bool {
        self.high == self.low
    }

    /// The dice available this turn: two, or **four** on a double.
    ///
    /// A double is not a roll of two that happens to match — it is four moves,
    /// and treating it as two is the single most visible way to get backgammon
    /// wrong.
    pub fn dice(self) -> Vec<u8> {
        if self.is_double() {
            vec![self.high; 4]
        } else {
            vec![self.high, self.low]
        }
    }

    /// How many moves this roll is worth.
    pub const fn count(self) -> usize {
        if self.is_double() { 4 } else { 2 }
    }

    /// Reads `5-3`. Written with the separator so a database row is legible to
    /// somebody working out what went wrong, which is the same reason a tafl
    /// position is a picture.
    pub fn parse(text: &str) -> Option<Roll> {
        let (one, two) = text.split_once('-')?;
        Roll::new(one.parse().ok()?, two.parse().ok()?)
    }
}

impl fmt::Display for Roll {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.high, self.low)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_double_is_four_moves_and_not_two() {
        let double = Roll::new(4, 4).expect("a roll");
        assert!(double.is_double());
        assert_eq!(double.count(), 4);
        assert_eq!(double.dice(), vec![4, 4, 4, 4]);

        let plain = Roll::new(4, 3).expect("a roll");
        assert!(!plain.is_double());
        assert_eq!(plain.count(), 2);
        assert_eq!(plain.dice(), vec![4, 3]);
    }

    #[test]
    fn a_roll_forgets_which_die_landed_first() {
        assert_eq!(Roll::new(3, 5), Roll::new(5, 3));
        assert_eq!(Roll::new(3, 5).unwrap().high(), 5);
        assert_eq!(Roll::new(3, 5).unwrap().low(), 3);
    }

    #[test]
    fn nothing_outside_one_to_six_is_a_roll() {
        assert_eq!(Roll::new(0, 3), None);
        assert_eq!(Roll::new(3, 7), None);
        assert_eq!(Roll::new(7, 7), None);
    }

    #[test]
    fn a_roll_reads_back_the_way_it_was_written() {
        for high in 1..=6 {
            for low in 1..=high {
                let roll = Roll::new(high, low).expect("a roll");
                assert_eq!(Roll::parse(&roll.to_string()), Some(roll));
            }
        }
        assert_eq!(Roll::parse("5-3").unwrap().to_string(), "5-3");
        assert_eq!(Roll::parse("nonsense"), None);
        assert_eq!(Roll::parse("9-1"), None);
    }
}
