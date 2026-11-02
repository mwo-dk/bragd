//! The dice, and the throw they make.
//!
//! **Nothing here generates anything.** The dice are tetrahedra with two of their
//! four corners marked, so each one shows a mark or does not, and a throw is how
//! many came up marked. The caller throws them and writes the result into the
//! record; this module only says what a throw *is* and how it is spelled. How
//! many dice there are, and what a throw of nought moves, belong to the rule set
//! ([`crate::Rules`]).
//!
//! The distribution matters to a player and so it is written down where it can be
//! read: the dice are coins, so with four of them 0 through 4 come with weights
//! 1, 4, 6, 4, 1 out of sixteen. A throw of 2 is six times as likely as a throw
//! of 0, and the two extremes are a sixteenth each.

/// The most dice any rule set throws.
pub const MOST_DICE: u8 = 4;

/// A throw: how many of the dice showed a mark.
///
/// A newtype rather than a bare `u8` because the bare number is also a step count,
/// a piece count and a square index, and the one thing every one of those has in
/// common is that mixing them up compiles. It is the count of marks, not the
/// steps: with three dice a throw of nought moves four.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Throw(u8);

impl Throw {
    /// A throw, if it is one that up to four dice can make.
    pub fn new(marks: u8) -> Option<Throw> {
        (marks <= MOST_DICE).then_some(Throw(marks))
    }

    /// How many marked tips came up.
    pub fn marks(self) -> u8 {
        self.0
    }

    /// Reads a throw back off the wire.
    ///
    /// The spelling is the digit, because a throw has exactly one number in it
    /// and any decoration would be a second spelling of the same thing.
    pub fn parse(text: &str) -> Option<Throw> {
        match text.as_bytes() {
            [digit @ b'0'..=b'4'] => Some(Throw(digit - b'0')),
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
        for marks in 0..=MOST_DICE {
            assert!(Throw::new(marks).is_some(), "{marks} should be a throw");
        }
        assert_eq!(Throw::new(5), None);
        assert_eq!(Throw::new(255), None);
    }

    #[test]
    fn a_throw_survives_the_wire() {
        for marks in 0..=MOST_DICE {
            let throw = Throw::new(marks).unwrap();
            assert_eq!(Throw::parse(&throw.to_wire()), Some(throw));
            assert_eq!(throw.marks(), marks);
        }
    }

    #[test]
    fn rubbish_does_not_parse_as_a_throw() {
        for text in ["", " ", "5", "-1", "00", "1 ", "four", "1-2"] {
            assert_eq!(Throw::parse(text), None, "{text:?} should not parse");
        }
    }
}
