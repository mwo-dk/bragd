//! What a ruleset is, and the two Bragd plays.
//!
//! Go's rules are not in doubt the way tafl's are — capture, liberties and ko
//! are the same game everywhere it is played. What the rule sets disagree about
//! is **how the game is counted and how repetition is stopped**, and those two
//! disagreements are enough to change who won a finished board. So they are
//! parameters, and there are only four of them.
//!
//! What is deliberately *not* a parameter is suicide, which Japanese and
//! Chinese rules agree is illegal (see [`crate::position::Position::after`]),
//! and the board size at which each ruleset is played, which is free: unlike
//! tafl, where a ruleset and a board are one choice, Chinese rules on a 9×9 are
//! Chinese rules.

use crate::board::plays;
use crate::position::Position;

/// How the board is counted at the end.
///
/// The difference is not cosmetic and it is not usually decisive either: on a
/// board played out to the end the two methods normally give the same winner,
/// and the reason they exist separately is what they ask the players to do.
/// Area counting rewards playing another stone inside your own territory as
/// worth exactly as much as the point it sits on, so nothing is lost by filling
/// dame; territory counting punishes it, which is why Japanese games end with
/// an argument about which stones were dead and Chinese ones mostly do not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scoring {
    /// Chinese: your stones on the board, plus the empty points only you
    /// surround.
    Area,
    /// Japanese: the empty points only you surround, plus the stones you have
    /// captured.
    Territory,
}

/// How a position is stopped from coming round again.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Repetition {
    /// The immediate retake is forbidden, and nothing else is. Japanese. It is
    /// the rule everybody is taught, and it does not close every loop — a
    /// double ko cycles under it for ever, which real Japanese rules answer by
    /// declaring the game void rather than by banning a move.
    SimpleKo,
    /// No whole-board position may occur twice, whoever is to play. Chinese and
    /// AGA. It closes the loops simple ko leaves open, at the cost of a rule
    /// that cannot be checked by looking at the board.
    ///
    /// *Positional*, not *situational*: the side to move is not part of what is
    /// compared. Situational superko — the board **and** whose turn it is — is
    /// the AGA's own wording and forbids strictly less. The difference shows up
    /// only in positions almost nobody reaches, and Bragd is better served
    /// by the stricter rule than by two of them.
    PositionalSuperko,
}

/// A ruleset: how big, how counted, how repetition is stopped, and by how much
/// White is compensated for playing second.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ruleset {
    /// Stable. A Game is stored with it and a rating pool is keyed by it —
    /// together with [`Ruleset::size`], because 9×9 and 19×19 are not the same
    /// game and a rating that mixed them would mean nothing.
    pub key: &'static str,
    pub name: &'static str,

    /// 9, 13 or 19.
    pub size: u8,

    pub scoring: Scoring,
    pub repetition: Repetition,

    /// **Twice** the komi, in whole points.
    ///
    /// Komi compensates White for moving second and is a half-integer — 6.5,
    /// 7.5 — precisely so that the game cannot be tied. Storing it as a float
    /// would put the outcome of a game behind a floating-point comparison, and
    /// a rule whose entire purpose is "these two numbers are never equal"
    /// deserves arithmetic that is exact rather than arithmetic that is very
    /// nearly exact. Doubling it makes every score an integer, every comparison
    /// exact, and the impossibility of a tie a fact about the numbers rather
    /// than a hope about the rounding.
    pub komi_halves: i16,
}

impl Ruleset {
    /// Every ruleset there is, so that nothing has to write the list again.
    ///
    /// The boundary looks a stored key up in here rather than in a `match` of
    /// its own with a `default:` arm. Two tables of the same thing is the shape
    /// that has produced three blank boards in Bragd side, and it is the same
    /// shape in Rust: the copy that is not the definition falls behind, and what
    /// falls through its default is a game refused or — worse — a game opened
    /// under whatever the default was.
    pub const ALL: [Ruleset; 2] = [Ruleset::CHINESE, Ruleset::JAPANESE];

    /// The ruleset a stored key names, or nothing.
    ///
    /// Nothing rather than a fallback: a game replayed under rules that are not
    /// its own is a different game, and it would reach a Library looking
    /// correct.
    pub fn from_key(key: &str) -> Option<Ruleset> {
        // Sizes are deliberately not part of this. A key names the rules; the
        // board is the second argument, and `plays` is what vets it.
        Ruleset::ALL.into_iter().find(|rules| rules.key == key)
    }

    /// Chinese rules on 19×19: area scoring, positional superko, **7.5** komi.
    ///
    /// The two rule sets here carry different komi, and that is not an
    /// oversight. Area scoring counts a point Black would have had to play
    /// anyway, so it runs about a point in Black's favour against territory
    /// scoring — and the komi that goes with it is a point higher to match.
    /// Chinese tournaments and the AGA both play 7.5 with area scoring;
    /// Japanese rules play 6.5 with territory.
    ///
    /// Komi is a parameter regardless: it has been raised three times in living
    /// memory as the professional view of Black's advantage moved, and it will
    /// move again.
    pub const CHINESE: Ruleset = Ruleset {
        key: "chinese",
        name: "Chinese",
        size: 19,
        scoring: Scoring::Area,
        repetition: Repetition::PositionalSuperko,
        komi_halves: 15,
    };

    /// Japanese rules on 19×19: territory scoring, simple ko, 6.5 komi.
    pub const JAPANESE: Ruleset = Ruleset {
        key: "japanese",
        name: "Japanese",
        size: 19,
        scoring: Scoring::Territory,
        repetition: Repetition::SimpleKo,
        komi_halves: 13,
    };

    /// The same rules on a smaller board.
    ///
    /// # Panics
    /// If the size is not one Bragd plays.
    pub const fn on(mut self, size: u8) -> Ruleset {
        assert!(plays(size), "Bragd plays 9, 13 and 19");
        self.size = size;
        self
    }

    /// The same rules at a different komi.
    ///
    /// **This exists for handicap games and takes no view about them.** How many
    /// stones a game is given, and therefore what its komi should be, is a
    /// decision made before the game about how strong two people are; the engine is handed the
    /// starting position and the number that goes with it. What it must not do
    /// is *count* a handicap game at the even game's 7.5, which is the failure
    /// this makes avoidable: area scoring gives Black a point for each free
    /// stone he was allowed to place, so a nine-stone board counted at even komi
    /// hands him eight points nobody agreed to — and the result is a plausible
    /// number rather than an error.
    ///
    /// Halves, like the field, so nothing here ever divides.
    pub const fn with_komi(mut self, komi_halves: i16) -> Ruleset {
        self.komi_halves = komi_halves;
        self
    }

    /// An empty board with Black to play. There are no handicap stones here:
    /// placing them is a decision about how strong the two players are,
    /// and the engine will take any starting position it is handed.
    pub fn starting_position(&self) -> Position {
        Position::empty(self.size).expect("a ruleset's size is a board that exists")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::SIZES;

    #[test]
    fn komi_is_stored_doubled_so_that_a_tie_is_arithmetic_rather_than_luck() {
        assert_eq!(Ruleset::CHINESE.komi_halves, 15, "seven and a half points");
        assert_eq!(Ruleset::JAPANESE.komi_halves, 13, "six and a half points");
        // **The two differ on purpose.** Area scoring runs about a point in
        // Black's favour against territory scoring, and the komi that goes with
        // it is a point higher to match. Asserting they were equal — which this
        // test used to — would pin an error in place.
        assert_eq!(
            Ruleset::CHINESE.komi_halves,
            Ruleset::JAPANESE.komi_halves + 2
        );
        for ruleset in [Ruleset::CHINESE, Ruleset::JAPANESE] {
            assert_eq!(
                ruleset.komi_halves % 2,
                1,
                "an odd number of halves is the half point that stops a tie"
            );
        }
    }

    #[test]
    fn a_handicap_komi_replaces_the_even_one_and_changes_nothing_else() {
        // 1.5 for two stones, 8.5 for nine: the compensation area scoring needs
        // for the points Black's free stones stand on. The number is the caller's
        // — what this holds is that setting it leaves the rest of the ruleset
        // alone, because a game counted under rules that are not its own is a
        // different game.
        //
        // **Built on nine, and that is the point of the line rather than a
        // detail.** `Ruleset::CHINESE.size` is already 19, so `.on(19)` was a
        // no-op and the size assertion could not tell "with_komi left the size
        // alone" from "with_komi wrote 19 into it" — the one field the test
        // claims to be about that it was not checking. Writing a size in here
        // would count every 9x9 and 13x13 handicap game on a 19x19 ruleset,
        // which is exactly the failure the paragraph above invokes.
        let two = Ruleset::CHINESE.on(9).with_komi(3);
        assert_eq!(two.komi_halves, 3);
        assert_eq!(two.key, Ruleset::CHINESE.key);
        assert_eq!(two.size, 9);
        assert_eq!(two.scoring, Scoring::Area);
        assert_eq!(two.repetition, Repetition::PositionalSuperko);
        assert_eq!(Ruleset::CHINESE.with_komi(17).komi_halves, 17);
        // Untouched: `with_komi` copies, so the constant is still the constant.
        assert_eq!(Ruleset::CHINESE.komi_halves, 15);
    }

    #[test]
    fn a_smaller_board_keeps_the_rules_and_the_key() {
        let small = Ruleset::JAPANESE.on(9);
        assert_eq!(small.size, 9);
        assert_eq!(small.key, Ruleset::JAPANESE.key);
        assert_eq!(small.scoring, Scoring::Territory);
        assert_eq!(small.starting_position().board().intersections(), 81);
    }

    #[test]
    #[should_panic(expected = "Bragd plays 9, 13 and 19")]
    fn a_board_size_nobody_plays_is_refused() {
        let _ = Ruleset::CHINESE.on(11);
    }

    /// The two readings of the size list agree about *which* sizes, which is
    /// what one list buys. They still disagree about how to refuse one — this
    /// panics and the boundary returns an error — and that is deliberate.
    #[test]
    fn the_sizes_this_accepts_are_the_sizes_the_list_holds() {
        for size in SIZES {
            assert!(plays(size), "{size} is in SIZES and `plays` says no");
            assert_eq!(Ruleset::CHINESE.on(size).size, size);
        }
        for nobody in [0, 1, 8, 10, 11, 12, 14, 18, 20, 255] {
            assert!(!plays(nobody), "{nobody} is not a board Bragd offers");
        }
    }

    #[test]
    fn a_key_finds_its_ruleset_and_an_unknown_one_finds_nothing() {
        for rules in Ruleset::ALL {
            assert_eq!(Ruleset::from_key(rules.key), Some(rules));
        }
        // Bragd's own catalogue key, which is deliberately not one of these:
        // `go-19` is a ruleset *and* a size and is split apart above this crate.
        assert_eq!(Ruleset::from_key("go-19"), None);
        assert_eq!(Ruleset::from_key(""), None);
    }
}
