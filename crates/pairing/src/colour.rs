//! Colour preferences (C.04.3 1.6-1.7) and colour allocation (C.04.3 5).
//!
//! Only played games count; unplayed rounds are skipped (C.04.2 3.4).

use crate::Colour;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Strength {
    Mild,
    Strong,
    Absolute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preference {
    pub colour: Colour,
    pub strength: Strength,
}

/// White games minus Black games (1.6).
pub fn difference(played: &[Colour]) -> i32 {
    played
        .iter()
        .map(|c| if *c == Colour::White { 1 } else { -1 })
        .sum()
}

/// The colour preference after these played games (1.7); none before the first game.
pub fn preference(played: &[Colour]) -> Option<Preference> {
    let last = *played.last()?;
    let cd = difference(played);
    let pref = |colour, strength| Some(Preference { colour, strength });
    if cd > 1 {
        return pref(Colour::Black, Strength::Absolute);
    }
    if cd < -1 {
        return pref(Colour::White, Strength::Absolute);
    }
    if played.len() >= 2 && played[played.len() - 2] == last {
        return pref(last.opposite(), Strength::Absolute);
    }
    match cd {
        1 => pref(Colour::Black, Strength::Strong),
        -1 => pref(Colour::White, Strength::Strong),
        _ => pref(last.opposite(), Strength::Mild),
    }
}

/// One player of a pair, as colour allocation sees them.
pub struct Side<'a> {
    pub played: &'a [Colour],
    pub tpn: u32,
}

/// The colour for `higher`, the higher ranked player of the pair (1.2), by 5.2.
pub fn allocate(higher: &Side, lower: &Side, initial: Colour) -> Colour {
    let (hp, lp) = (preference(higher.played), preference(lower.played));
    match (hp, lp) {
        (Some(h), Some(l)) if h.colour != l.colour => h.colour,
        (Some(h), None) => h.colour,
        (None, Some(l)) => l.colour.opposite(),
        (Some(h), Some(l)) => {
            // 5.2.2: the stronger preference; between two absolute ones, the wider difference.
            if h.strength != l.strength {
                return if h.strength > l.strength {
                    h.colour
                } else {
                    h.colour.opposite()
                };
            }
            if h.strength == Strength::Absolute {
                let (dh, dl) = (
                    difference(higher.played).abs(),
                    difference(lower.played).abs(),
                );
                if dh != dl {
                    return if dh > dl {
                        h.colour
                    } else {
                        h.colour.opposite()
                    };
                }
            }
            // 5.2.3: alternate to the latest round in which they had different colours.
            let (a, b) = (higher.played, lower.played);
            for k in 1..=a.len().min(b.len()) {
                let (ca, cb) = (a[a.len() - k], b[b.len() - k]);
                if ca != cb {
                    return ca.opposite();
                }
            }
            // 5.2.4: the higher ranked player's preference.
            h.colour
        }
        // 5.2.5: neither has played.
        (None, None) => {
            if higher.tpn % 2 == 1 {
                initial
            } else {
                initial.opposite()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Colour::{Black as B, White as W};

    fn h(s: &str) -> Vec<Colour> {
        s.chars()
            .filter(|c| *c != 'u')
            .map(|c| if c == 'W' { W } else { B })
            .collect()
    }

    fn side(s: &[Colour], tpn: u32) -> Side<'_> {
        Side { played: s, tpn }
    }

    #[test]
    fn preferences_follow_1_7() {
        let p = |s: &str| preference(&h(s)).map(|p| (p.colour, p.strength));
        assert_eq!(p(""), None);
        assert_eq!(p("W"), Some((B, Strength::Strong)));
        assert_eq!(p("WB"), Some((W, Strength::Mild)));
        assert_eq!(p("BWW"), Some((B, Strength::Absolute)));
        assert_eq!(p("WWB"), Some((B, Strength::Strong)));
        assert_eq!(p("WBWW"), Some((B, Strength::Absolute)));
        // The annotated rules' example: WBBWuW counts as WBBWW, which ends in two Whites.
        assert_eq!(p("WBBWuW"), Some((B, Strength::Absolute)));
        assert_eq!(p("BWBB"), Some((W, Strength::Absolute)));
    }

    #[test]
    fn wider_difference_wins_between_absolute_topscorers() {
        // Annotated Dutch 2026, 5.2.2: BBWBWB (CD -2) against WWBWBB (CD 0); #1 gets White.
        let (a, b) = (h("BBWBWB"), h("WWBWBB"));
        assert_eq!(allocate(&side(&a, 1), &side(&b, 2), W), W);
        assert_eq!(allocate(&side(&b, 1), &side(&a, 2), W), B);
    }

    #[test]
    fn absolute_beats_strong_whatever_the_difference() {
        // Annotated Dutch 2026, 5.2.2: WBBWBWB (strong, CD -1) against uWWBWBB (absolute, CD 0).
        let (a, b) = (h("WBBWBWB"), h("uWWBWBB"));
        assert_eq!(allocate(&side(&a, 1), &side(&b, 2), W), B);
    }

    #[test]
    fn equal_mild_preferences_fall_back_to_the_higher_ranked() {
        // Mastering the Dutch 2026, round 9: #11 (WBWBBWBW) is higher ranked than #5
        // (uuWBBWBW); the common part is identical, so #11 gets their Black.
        let (p11, p5) = (h("WBWBBWBW"), h("uuWBBWBW"));
        assert_eq!(allocate(&side(&p11, 11), &side(&p5, 5), W), B);
    }

    #[test]
    fn alternation_looks_back_to_the_last_difference() {
        // Both want White mildly (last game Black); two rounds earlier they differed.
        let (a, b) = (h("WBWB"), h("BWWB"));
        assert_eq!(preference(&a).unwrap().strength, Strength::Mild);
        assert_eq!(allocate(&side(&a, 1), &side(&b, 2), W), W);
        assert_eq!(allocate(&side(&b, 1), &side(&a, 2), W), B);
    }

    #[test]
    fn first_round_uses_the_initial_colour_and_tpn_parity() {
        assert_eq!(allocate(&side(&[], 1), &side(&[], 8), W), W);
        assert_eq!(allocate(&side(&[], 2), &side(&[], 9), W), B);
        assert_eq!(allocate(&side(&[], 3), &side(&[], 10), B), B);
    }

    #[test]
    fn a_player_without_games_takes_the_other_colour() {
        let a = h("W");
        assert_eq!(allocate(&side(&[], 1), &side(&a, 9), W), W);
    }
}
