//! Which draughts this is.
//!
//! Unlike tafl, the disagreements here are not gaps in the record — they are
//! **different games with different names**, each with its own federation and
//! its own literature. "Checkers" is not a game you can offer; one of these is.
//!
//! Four rules do the separating, and every field below is one of them, plus who
//! moves first: White in three, Black in English draughts.

use crate::board::Geometry;
use crate::position::{Move, Side};

/// When a man that reaches the far rank becomes a king.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Promotion {
    /// Only when the move *ends* there. A man that jumps through the far rank
    /// and keeps going stays a man — International, Brazilian.
    OnStopping,
    /// The moment it arrives, continuing the sequence as a king. Russian.
    MidSequence,
    /// Not at all. No mainstream variant; here so the field cannot be a bool
    /// pretending the question has two answers.
    Never,
}

/// How much you are obliged to take.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CaptureChoice {
    /// Any available capture. English, Russian, Pool.
    Any,
    /// The one taking the most pieces, kings counting the same as men.
    /// International, Brazilian.
    Maximum,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ruleset {
    /// Stable. Stored on every Game, and a rating pool is keyed by it.
    pub key: &'static str,
    pub name: &'static str,
    pub geometry: Geometry,

    /// Whether a man may jump backwards. Its *walking* direction is forward
    /// either way — these are two rules and conflating them is a real bug.
    pub men_capture_backwards: bool,

    /// Whether a king slides any distance, and may land anywhere beyond a
    /// victim. The choice of landing square is a branching decision, not a
    /// formality.
    pub flying_kings: bool,

    pub promotion: Promotion,
    pub capture_choice: CaptureChoice,

    /// Whether this variant's **published perft tables** count a capture twice
    /// when it can be played by two paths to the same end.
    ///
    /// Not a rule of the game: two paths with the same origin, destination and
    /// captured set reach the same position, so for *play* they are one move
    /// and offering both would be a bug in the move list. It is a convention of
    /// the literature, and the two families of table use different ones —
    /// International's are published with duplicates removed, English's with
    /// them kept.
    ///
    /// It lives here rather than on a shared perft harness because a `dedupe`
    /// flag on the harness would be the harness learning a rule. The harness counts what it is given; the variant owns what a
    /// move is.
    pub counts_duplicate_captures: bool,

    /// The side that makes the first move. Black (the dark men, on printed
    /// squares 1–12) in English draughts; White in the others.
    pub first: Side,
}

/// **International draughts**, 10×10. The FMJD game, and the engine's core.
///
/// Chosen as the first because it has the strongest evidence: two independent
/// implementations agree exactly on its perft to depth eleven, and the table is
/// a live trap for the duplicate-capture bug — a published run once differed at
/// depth eleven by 345,735 nodes for exactly that reason.
pub const INTERNATIONAL: Ruleset = Ruleset {
    key: "draughts",
    name: "International draughts",
    geometry: Geometry::INTERNATIONAL,
    men_capture_backwards: true,
    flying_kings: true,
    promotion: Promotion::OnStopping,
    capture_choice: CaptureChoice::Maximum,
    counts_duplicate_captures: false,
    first: Side::White,
};

/// **English draughts**, 8×8 — what most English speakers mean by "checkers".
///
/// Three of the four rules differ from International, which is why they cannot
/// share a catalogue row: men do not jump backwards, kings do not fly, and you
/// may take any capture rather than the largest.
pub const ENGLISH: Ruleset = Ruleset {
    key: "checkers",
    name: "Checkers",
    geometry: Geometry::SMALL,
    men_capture_backwards: false,
    flying_kings: false,
    promotion: Promotion::OnStopping,
    capture_choice: CaptureChoice::Any,
    counts_duplicate_captures: true,
    first: Side::Black,
};

/// **Brazilian draughts** — International's rules on the small board.
pub const BRAZILIAN: Ruleset = Ruleset {
    key: "draughts-brazilian",
    name: "Brazilian draughts",
    geometry: Geometry::SMALL,
    men_capture_backwards: true,
    flying_kings: true,
    promotion: Promotion::OnStopping,
    capture_choice: CaptureChoice::Maximum,
    counts_duplicate_captures: false,
    first: Side::White,
};

/// **Russian draughts** — Brazilian's board and pieces, two rules apart.
///
/// Kept because those two rules are the classic divergence: a man promotes
/// *during* a sequence and carries on as a king, and there is no obligation to
/// take the most.
pub const RUSSIAN: Ruleset = Ruleset {
    key: "draughts-russian",
    name: "Russian draughts",
    geometry: Geometry::SMALL,
    men_capture_backwards: true,
    flying_kings: true,
    promotion: Promotion::MidSequence,
    capture_choice: CaptureChoice::Any,
    counts_duplicate_captures: true,
    first: Side::White,
};

impl Ruleset {
    /// Every variant this crate plays, in the order Bragd lists them.
    ///
    /// These are four *different games*, each with its own federation — a man
    /// jumps backwards in three of them and not in the fourth, kings fly in
    /// three and not in the fourth, and you are obliged to take the most in two
    /// of them and not in the other two. So a key that opened the wrong one
    /// would not misplay a game, it would play a different game, and the moves
    /// would replay without erring.
    pub const ALL: [Ruleset; 4] = [INTERNATIONAL, ENGLISH, BRAZILIAN, RUSSIAN];

    /// The variant a stored key names, or nothing.
    ///
    /// **Here rather than at the boundary**, which held a `match` over the same
    /// four keys with a `_ => None` of its own — a second table with nothing
    /// holding it to the first, so a fifth variant added here would be a
    /// catalogue row nobody could open.
    pub fn from_key(key: &str) -> Option<Ruleset> {
        Ruleset::ALL.into_iter().find(|rules| rules.key == key)
    }

    /// Narrows a capture list to what the rules oblige.
    pub fn filter_captures(&self, mut captures: Vec<Move>) -> Vec<Move> {
        if self.capture_choice == CaptureChoice::Any {
            return captures;
        }
        // Quantity only. A king counts exactly the same as a man — there is no
        // quality tiebreak in International, and adding one would be Italian's
        // rule in International's clothing.
        let most = captures
            .iter()
            .map(|mv| mv.captured.count())
            .max()
            .unwrap_or(0);
        captures.retain(|mv| mv.captured.count() == most);
        captures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_finds_its_variant_and_an_unknown_one_finds_nothing() {
        for rules in Ruleset::ALL {
            assert_eq!(Ruleset::from_key(rules.key), Some(rules));
        }
        assert_eq!(Ruleset::from_key("hnefatafl"), None);
        assert_eq!(Ruleset::from_key(""), None);
    }

    /// Four games, four keys. A duplicate would mean one of them was
    /// unreachable through `from_key` and would look like a typo in a table
    /// nobody reads.
    #[test]
    fn every_variant_has_a_key_of_its_own() {
        let mut keys: Vec<&str> = Ruleset::ALL.iter().map(|rules| rules.key).collect();
        keys.sort_unstable();
        let distinct = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), distinct, "two variants share a key");
    }
}
