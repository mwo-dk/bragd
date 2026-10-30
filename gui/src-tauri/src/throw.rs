//! Throwing the dice and sticks of the race Families. The randomness lives here, in the app:
//! the rules crates and `bragd-games` only ever see the throw as an entry.

use bragd_games::Family;
use std::hash::{BuildHasher, RandomState};

/// Random bits: each `RandomState` is seeded afresh by the standard library.
// shortcut: not a cryptographic generator, which a die does not need; upgrade for online play.
fn bits() -> u64 {
    RandomState::new().hash_one(std::time::SystemTime::now())
}

/// One throw of the Family's implements, written as its entry.
fn cast(family: Family, bits: u64) -> Option<String> {
    let sides = (bits & 0xF).count_ones();
    match family {
        // Two dice, the higher first: `6-5`.
        Family::Backgammon => {
            let (a, b) = (bits % 6 + 1, (bits / 6) % 6 + 1);
            Some(format!("{}-{}", a.max(b), a.min(b)))
        }
        // Four tetrahedral dice, each with two marked tips of four: the marked ones count.
        Family::Ur => Some(sides.to_string()),
        // Four sticks: the light faces up count, and none up counts five.
        Family::Senet => Some(if sides == 0 { 5 } else { sides }.to_string()),
        _ => None,
    }
}

/// A throw the rules accept now: one of `allowed`, thrown again until it is (the opening roll
/// of backgammon takes no doubles).
pub fn throw(family: Family, allowed: &[String]) -> Option<String> {
    if allowed.is_empty() {
        return None;
    }
    (0..1000)
        .filter_map(|_| cast(family, bits()))
        .find(|t| allowed.contains(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throws_cover_every_face_and_only_those() {
        let all = |family, n: u64| {
            let mut seen: Vec<String> = (0..n).filter_map(|b| cast(family, b * 7919)).collect();
            seen.sort();
            seen.dedup();
            seen
        };
        assert_eq!(all(Family::Backgammon, 5000).len(), 21);
        assert_eq!(all(Family::Ur, 64), ["0", "1", "2", "3", "4"]);
        assert_eq!(all(Family::Senet, 64), ["1", "2", "3", "4", "5"]);
        assert_eq!(cast(Family::Chess, 1), None);
    }

    #[test]
    fn throw_keeps_to_what_the_rules_allow() {
        let state = bragd_games::replay::<&str>("backgammon", None, &[]).unwrap();
        for _ in 0..50 {
            let t = throw(Family::Backgammon, &state.throws).unwrap();
            assert!(state.throws.contains(&t), "{t}");
        }
        assert_eq!(throw(Family::Ur, &[]), None);
        assert_eq!(throw(Family::Ur, &["3".to_string()]).as_deref(), Some("3"));
    }
}
