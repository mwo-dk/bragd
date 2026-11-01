//! Annotations: a move's mark (?? ? ?! ! !!), the better move and its line, and named motifs.
//! Bragd words them in the reader's language from its own texts; the store keeps them as
//! structured rows, keyed like Evaluations and by the move (`Store::put_annotation`).
//!
//! A mark comes from two Evaluations, of the position before the move and after it, read from
//! the mover's side, in the units of their score kind:
//!
//! | Kind | Units | `?!` | `?` | `??` | Already won |
//! |---|---|---|---|---|---|
//! | cp, mate | centipawns, capped at 1000 either way; a mate counts as 1000 | 50 | 100 | 300 | 300 |
//! | winrate (go) | percentage points of the chance to win | 5 | 10 | 20 | 90 |
//! | equity (backgammon) | points per game | 0.04 | 0.08 | 0.16 | 1.0 |
//!
//! A move that lost at least the `?!`, `?` or `??` amount gets that mark. The Engine's own best
//! move, played, where its second choice is worse by at least the `?` amount gets `!`, and by
//! at least the `??` amount `!!`, unless the mover was already winning by the last column.

use serde::{Deserialize, Serialize};

use crate::{Edition, Score};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mark {
    Blunder,
    Mistake,
    Inaccuracy,
    Good,
    Brilliant,
}

impl Mark {
    pub const ALL: [Mark; 5] = [Mark::Blunder, Mark::Mistake, Mark::Inaccuracy, Mark::Good, Mark::Brilliant];

    /// As a move list writes it: `??`, `?`, `?!`, `!`, `!!`. The store keeps it so.
    pub fn sign(self) -> &'static str {
        match self {
            Mark::Blunder => "??",
            Mark::Mistake => "?",
            Mark::Inaccuracy => "?!",
            Mark::Good => "!",
            Mark::Brilliant => "!!",
        }
    }

    /// Its texts' key: `annotation.mark.<id>`.
    pub fn id(self) -> &'static str {
        match self {
            Mark::Blunder => "blunder",
            Mark::Mistake => "mistake",
            Mark::Inaccuracy => "inaccuracy",
            Mark::Good => "good",
            Mark::Brilliant => "brilliant",
        }
    }

    pub fn from_sign(s: &str) -> Option<Mark> {
        Mark::ALL.into_iter().find(|m| m.sign() == s)
    }
}

/// A named motif. Chess only, for now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Motif {
    Fork,
    Pin,
    /// Leaves a piece hanging.
    Hangs,
    /// The mover had a forced mate and played a move that has none.
    MissedMate,
}

impl Motif {
    pub const ALL: [Motif; 4] = [Motif::Fork, Motif::Pin, Motif::Hangs, Motif::MissedMate];

    /// As the store keeps it, and its texts' key: `annotation.motif.<id>`.
    pub fn id(self) -> &'static str {
        match self {
            Motif::Fork => "fork",
            Motif::Pin => "pin",
            Motif::Hangs => "hangs",
            Motif::MissedMate => "missed_mate",
        }
    }

    pub fn from_id(s: &str) -> Option<Motif> {
        Motif::ALL.into_iter().find(|m| m.id() == s)
    }
}

/// One move's Annotation, by one Edition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub family: String,
    pub variant: String,
    /// The key of the position the move was played in.
    pub key: u64,
    /// The move, in the Engine's notation.
    pub mv: String,
    pub edition: Edition,
    pub mark: Option<Mark>,
    /// The Engine's line from the position before, when its first move is not the one played;
    /// empty otherwise. Moves in the Engine's notation, separated by spaces.
    pub better: String,
    pub motifs: Vec<Motif>,
    /// The lesser depth of the two Evaluations it comes from.
    pub depth: u32,
    /// The later time of the two, in seconds since 1970.
    pub time: i64,
}

/// The amounts for `?!`, `?`, `??` and "already won", in a score kind's units.
pub fn thresholds(s: Score) -> [f64; 4] {
    match s {
        Score::Cp(_) | Score::Mate(_) => [50.0, 100.0, 300.0, 300.0],
        Score::WinRate { .. } => [5.0, 10.0, 20.0, 90.0],
        Score::Equity(_) => [0.04, 0.08, 0.16, 1.0],
    }
}

/// A centipawn score is capped at this either way, and a mate counts as it.
pub const CAP: f64 = 1000.0;

/// `s` (from the first Side's view) from the mover's side, in its kind's units, and which scale
/// it is on (cp and mate share one). `mated_now`: a `Mate(0)` is the side to move mated, and
/// the side to move is not the mover.
fn value(s: Score, mover_first: bool, mated_now: bool) -> (u8, f64) {
    let sign = if mover_first { 1.0 } else { -1.0 };
    match s {
        Score::Cp(c) => (0, (sign * c as f64).clamp(-CAP, CAP)),
        Score::Mate(0) => (0, if mated_now { CAP } else { -CAP }),
        Score::Mate(m) => (0, sign * m.signum() as f64 * CAP),
        Score::WinRate { win, .. } => (1, if mover_first { win * 100.0 } else { 100.0 - win * 100.0 }),
        Score::Equity(e) => (2, sign * e),
    }
}

/// How much worse the Engine's second choice is than its first, from the mover's side, in the
/// first one's units. Both scores from the first Side's view.
pub fn gap(best: Score, second: Score, mover_first: bool) -> Option<f64> {
    let (a, b) = (value(best, mover_first, false), value(second, mover_first, false));
    (a.0 == b.0).then_some(a.1 - b.1)
}

/// The mark of a move by the first Side (`mover_first`) or the second, that took the
/// Evaluation from `before` to `after` (both from the first Side's view). `gap`: the move was
/// the Engine's best, and by how much its second choice is worse (`gap`); None when it was not,
/// or that is not known.
pub fn mark(before: Score, after: Score, mover_first: bool, gap: Option<f64>) -> Option<Mark> {
    let (k0, v0) = value(before, mover_first, false);
    let (k1, v1) = value(after, mover_first, true);
    if k0 != k1 {
        return None;
    }
    let [inaccuracy, mistake, blunder, won] = thresholds(before);
    let lost = v0 - v1;
    if lost >= blunder {
        Some(Mark::Blunder)
    } else if lost >= mistake {
        Some(Mark::Mistake)
    } else if lost >= inaccuracy {
        Some(Mark::Inaccuracy)
    } else if v0 >= won {
        None
    } else {
        match gap {
            Some(g) if g >= blunder => Some(Mark::Brilliant),
            Some(g) if g >= mistake => Some(Mark::Good),
            _ => None,
        }
    }
}

/// Whether the mover had a forced mate before the move and none after it.
pub fn missed_mate(before: Score, after: Score, mover_first: bool) -> bool {
    let mates = |s: Score, after: bool| match s {
        Score::Mate(0) => after,
        Score::Mate(m) => (m > 0) == mover_first,
        _ => false,
    };
    mates(before, false) && !mates(after, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centipawn_marks_by_what_the_mover_lost() {
        // White from +0.30: to −0.10 lost 40, nothing; −0.30 ?!; −1.00 ?; −3.00 ??.
        assert_eq!(mark(Score::Cp(30), Score::Cp(-10), true, None), None);
        assert_eq!(mark(Score::Cp(30), Score::Cp(-30), true, None), Some(Mark::Inaccuracy));
        assert_eq!(mark(Score::Cp(30), Score::Cp(-100), true, None), Some(Mark::Mistake));
        assert_eq!(mark(Score::Cp(30), Score::Cp(-300), true, None), Some(Mark::Blunder));
        // Black's side: White's score going up is Black losing.
        assert_eq!(mark(Score::Cp(-30), Score::Cp(300), false, None), Some(Mark::Blunder));
        assert_eq!(mark(Score::Cp(-30), Score::Cp(-300), false, None), None);
        // Winning a little less in a won position is no blunder.
        assert_eq!(mark(Score::Cp(2000), Score::Cp(1000), true, None), None);
    }

    #[test]
    fn mates_count_past_the_cap() {
        // A mate in 3 thrown away for +5.00: 500 lost.
        assert_eq!(mark(Score::Mate(3), Score::Cp(500), true, None), Some(Mark::Blunder));
        assert_eq!(mark(Score::Mate(3), Score::Mate(5), true, None), None);
        // Black walks into a mate.
        assert_eq!(mark(Score::Cp(0), Score::Mate(4), false, None), Some(Mark::Blunder));
        // The mating move: the side to move is mated now.
        assert_eq!(mark(Score::Mate(1), Score::Mate(0), true, None), None);
        assert_eq!(mark(Score::Mate(-1), Score::Mate(0), false, None), None);
        assert!(missed_mate(Score::Mate(2), Score::Cp(900), true));
        assert!(missed_mate(Score::Mate(-2), Score::Cp(0), false));
        assert!(!missed_mate(Score::Mate(2), Score::Mate(1), true));
        assert!(!missed_mate(Score::Mate(1), Score::Mate(0), true));
        assert!(!missed_mate(Score::Mate(-2), Score::Cp(0), true), "the other side's mate");
    }

    #[test]
    fn the_only_good_move_is_marked_good() {
        assert_eq!(mark(Score::Cp(0), Score::Cp(0), true, Some(150.0)), Some(Mark::Good));
        assert_eq!(mark(Score::Cp(0), Score::Cp(0), true, Some(400.0)), Some(Mark::Brilliant));
        assert_eq!(mark(Score::Cp(0), Score::Cp(0), true, Some(60.0)), None);
        // Not when already winning.
        assert_eq!(mark(Score::Cp(900), Score::Cp(900), true, Some(400.0)), None);
        assert_eq!(gap(Score::Cp(-20), Score::Cp(200), false), Some(220.0));
        assert_eq!(gap(Score::Cp(20), Score::WinRate { win: 0.5, lead: 0.0 }, true), None);
    }

    #[test]
    fn go_marks_by_the_chance_to_win() {
        let w = |win: f64| Score::WinRate { win, lead: 0.0 };
        assert_eq!(mark(w(0.60), w(0.57), true, None), None);
        assert_eq!(mark(w(0.60), w(0.54), true, None), Some(Mark::Inaccuracy));
        assert_eq!(mark(w(0.60), w(0.45), true, None), Some(Mark::Mistake));
        assert_eq!(mark(w(0.60), w(0.30), true, None), Some(Mark::Blunder));
        // White (the second Side in go): Black's chance going up.
        assert_eq!(mark(w(0.40), w(0.65), false, None), Some(Mark::Blunder));
        assert_eq!(mark(w(0.95), w(0.95), true, Some(30.0)), None);
        assert_eq!(mark(w(0.50), w(0.50), true, Some(12.0)), Some(Mark::Good));
    }

    #[test]
    fn backgammon_marks_by_equity() {
        assert_eq!(mark(Score::Equity(0.10), Score::Equity(0.07), true, None), None);
        assert_eq!(mark(Score::Equity(0.10), Score::Equity(0.05), true, None), Some(Mark::Inaccuracy));
        assert_eq!(mark(Score::Equity(0.10), Score::Equity(0.0), true, None), Some(Mark::Mistake));
        assert_eq!(mark(Score::Equity(0.10), Score::Equity(-0.10), true, None), Some(Mark::Blunder));
        assert_eq!(mark(Score::Equity(-0.10), Score::Equity(0.10), false, None), Some(Mark::Blunder));
    }

    #[test]
    fn marks_and_motifs_read_back() {
        for m in Mark::ALL {
            assert_eq!(Mark::from_sign(m.sign()), Some(m));
        }
        for m in Motif::ALL {
            assert_eq!(Motif::from_id(m.id()), Some(m));
        }
        assert_eq!(Mark::from_sign("!?"), None);
    }
}
