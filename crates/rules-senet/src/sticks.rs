//! The four sticks, and the throw they make.
//!
//! **Nothing here generates anything**: the caller throws and writes the
//! result into the record, and a rules crate that could roll its own would make a Library unreplayable in silence.
//!
//! Four sticks, each with a plain face and a marked one. The throw is how many
//! marked faces land up — **except that none of them showing scores five**,
//! which is the rule that makes senet's dice unlike anything else in Bragd.
//! There is no zero and there is no missed turn: every throw moves something if
//! anything can move.
//!
//! That one substitution is also what makes the distribution interesting. Five is
//! the rarest throw and the most valuable — it is the only way off the board from
//! the House of Beauty, and the only way past the water row entirely — and it
//! comes up once in sixteen.

/// The number of sticks.
pub const STICKS: u8 = 4;

/// How many of the sixteen equally likely stick arrangements make each throw,
/// indexed by throw − 1. Sums to sixteen.
///
/// Note the shape: 4, 6, 4, 1, 1. It is a binomial with its nought folded onto
/// the end, which is why it is not symmetric and why a two is six times as likely
/// as a five.
pub const WAYS: [u8; 5] = [4, 6, 4, 1, 1];

/// A throw: one to five.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Throw(u8);

impl Throw {
    /// A throw, if it is one of the five senet can make.
    pub fn new(value: u8) -> Option<Throw> {
        (1..=5).contains(&value).then_some(Throw(value))
    }

    /// The throw made by this many marked faces landing up.
    ///
    /// **Nought marked faces is a five**, which is the whole of senet's dice and
    /// the one line worth reading twice in this file.
    pub fn from_marks(marks: u8) -> Option<Throw> {
        match marks {
            0 => Some(Throw(5)),
            marks if marks <= STICKS => Some(Throw(marks)),
            _ => None,
        }
    }

    /// How many houses this throw moves a piece.
    pub fn steps(self) -> u8 {
        self.0
    }

    /// Whether this throw earns another after the move is played.
    ///
    /// One, four and five do; two and three end the turn. **"After otherwise
    /// moving" is Kendall's phrasing and it matters**: a throw that produced no
    /// move earns nothing, and a water event ends the turn whatever was thrown.
    /// Neither of those is decidable here, so this answers only the question the
    /// throw itself can answer.
    pub fn throws_again(self) -> bool {
        matches!(self.0, 1 | 4 | 5)
    }

    /// How many of the sixteen arrangements produce this throw.
    pub fn ways(self) -> u8 {
        WAYS[self.0 as usize - 1]
    }

    pub fn parse(text: &str) -> Option<Throw> {
        match text {
            "1" => Some(Throw(1)),
            "2" => Some(Throw(2)),
            "3" => Some(Throw(3)),
            "4" => Some(Throw(4)),
            "5" => Some(Throw(5)),
            _ => None,
        }
    }

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
    fn a_throw_is_one_to_five_and_never_nought() {
        for value in 1..=5 {
            assert!(Throw::new(value).is_some(), "{value} should be a throw");
        }
        assert_eq!(Throw::new(0), None, "senet has no zero");
        assert_eq!(Throw::new(6), None);
        assert_eq!(Throw::new(255), None);
    }

    #[test]
    fn no_marked_faces_is_a_five() {
        assert_eq!(Throw::from_marks(0), Throw::new(5));
        for marks in 1..=STICKS {
            assert_eq!(Throw::from_marks(marks), Throw::new(marks), "{marks} marks");
        }
        assert_eq!(Throw::from_marks(STICKS + 1), None);
    }

    #[test]
    fn the_ways_sum_to_sixteen_and_five_is_as_rare_as_four() {
        let total: u32 = WAYS.iter().map(|&w| w as u32).sum();
        assert_eq!(total, 16, "four two-faced sticks have sixteen arrangements");
        assert_eq!(Throw::new(1).unwrap().ways(), 4);
        assert_eq!(Throw::new(2).unwrap().ways(), 6);
        assert_eq!(Throw::new(3).unwrap().ways(), 4);
        assert_eq!(Throw::new(4).unwrap().ways(), 1);
        assert_eq!(
            Throw::new(5).unwrap().ways(),
            1,
            "the five is the folded nought"
        );
    }

    #[test]
    fn one_four_and_five_earn_another_throw() {
        for value in [1, 4, 5] {
            assert!(
                Throw::new(value).unwrap().throws_again(),
                "{value} should repeat"
            );
        }
        for value in [2, 3] {
            assert!(
                !Throw::new(value).unwrap().throws_again(),
                "{value} should end the turn"
            );
        }
    }

    #[test]
    fn a_throw_survives_the_wire() {
        for value in 1..=5 {
            let throw = Throw::new(value).unwrap();
            assert_eq!(Throw::parse(&throw.to_wire()), Some(throw));
            assert_eq!(throw.steps(), value);
        }
    }

    #[test]
    fn rubbish_does_not_parse_as_a_throw() {
        for text in ["", " ", "0", "6", "-1", "01", "1 ", "five", "1-2"] {
            assert_eq!(Throw::parse(text), None, "{text:?} should not parse");
        }
    }
}
