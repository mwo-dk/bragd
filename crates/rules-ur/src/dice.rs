//! The four dice, and the throw they make.
//!
//! **Nothing here generates anything.** The dice are tetrahedra with two of their
//! four corners marked, so each one shows a mark or does not, and a throw is how
//! many of the four came up marked. The caller throws them and writes the
//! result into the record; this module only says what a throw *is* and how it is
//! spelled.
//!
//! The distribution matters to a player and so it is written down where it can be
//! read: four coins, so 0 through 4 with weights 1, 4, 6, 4, 1 out of sixteen. A
//! throw of 2 is six times as likely as a throw of 0, and the two extremes are a
//! sixteenth each. That is why a piece four steps from a rosette is in less
//! danger than it looks and one two steps away is in more.

/// The number of dice.
pub const DICE: u8 = 4;

/// How many ways each total can be thrown, indexed by the total. Sums to sixteen.
pub const WAYS: [u8; 5] = [1, 4, 6, 4, 1];

/// A throw: how many of the four dice showed a mark.
///
/// A newtype rather than a bare `u8` because the bare number is also a step count,
/// a piece count and a square index, and the one thing every one of those has in
/// common is that mixing them up compiles.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Throw(u8);

impl Throw {
    /// A throw, if it is one of the five that four dice can make.
    pub fn new(marks: u8) -> Option<Throw> {
        (marks <= DICE).then_some(Throw(marks))
    }

    /// How many steps this throw moves a piece.
    pub fn steps(self) -> u8 {
        self.0
    }

    /// **A throw of nought moves nothing and passes the turn.** It is not a
    /// missed turn to be re-thrown and it is not an error; it is a legal,
    /// recorded, one-in-sixteen event, and it is why a game of Ur is not a race
    /// either side can be sure of leading.
    pub fn moves_nothing(self) -> bool {
        self.0 == 0
    }

    /// How many of the sixteen equally likely die faces produce this throw.
    pub fn ways(self) -> u8 {
        WAYS[self.0 as usize]
    }

    /// Reads a throw back off the wire.
    ///
    /// The spelling is the digit, because a throw has exactly one number in it
    /// and any decoration would be a second spelling of the same thing.
    pub fn parse(text: &str) -> Option<Throw> {
        match text {
            "0" => Some(Throw(0)),
            "1" => Some(Throw(1)),
            "2" => Some(Throw(2)),
            "3" => Some(Throw(3)),
            "4" => Some(Throw(4)),
            _ => None,
        }
    }

    /// The wire spelling.
    pub fn to_wire(self) -> String {
        self.0.to_string()
    }
}

impl std::fmt::Display for Throw {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_throw_is_nought_to_four_and_nothing_else() {
        for marks in 0..=DICE {
            assert!(Throw::new(marks).is_some(), "{marks} should be a throw");
        }
        assert_eq!(Throw::new(5), None);
        assert_eq!(Throw::new(255), None);
    }

    #[test]
    fn the_ways_sum_to_sixteen_and_are_symmetric() {
        let total: u32 = WAYS.iter().map(|&w| w as u32).sum();
        assert_eq!(total, 16, "four two-sided dice have sixteen outcomes");
        assert_eq!(WAYS[0], WAYS[4]);
        assert_eq!(WAYS[1], WAYS[3]);
        assert_eq!(WAYS[2], 6);
        for marks in 0..=DICE {
            assert_eq!(Throw::new(marks).unwrap().ways(), WAYS[marks as usize]);
        }
    }

    #[test]
    fn only_a_nought_moves_nothing() {
        assert!(Throw::new(0).unwrap().moves_nothing());
        for marks in 1..=DICE {
            assert!(
                !Throw::new(marks).unwrap().moves_nothing(),
                "{marks} moves something"
            );
        }
    }

    #[test]
    fn a_throw_survives_the_wire() {
        for marks in 0..=DICE {
            let throw = Throw::new(marks).unwrap();
            assert_eq!(Throw::parse(&throw.to_wire()), Some(throw));
            assert_eq!(throw.steps(), marks);
        }
    }

    #[test]
    fn rubbish_does_not_parse_as_a_throw() {
        for text in ["", " ", "5", "-1", "00", "1 ", "four", "1-2"] {
            assert_eq!(Throw::parse(text), None, "{text:?} should not parse");
        }
    }
}
