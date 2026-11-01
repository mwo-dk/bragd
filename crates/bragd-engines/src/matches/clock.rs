//! A match's clocks: each engine's own time for the Game, as a tournament gives it.

use serde::{Deserialize, Serialize};

use crate::Limit;

/// A time control: `base` for the Game (or for each period of `moves` moves), and `inc` added
/// after every move. Milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeControl {
    pub base_ms: u32,
    #[serde(default)]
    pub inc_ms: u32,
    /// Moves per period: after each `moves` moves of its own, a Side gets `base_ms` again.
    /// None: the base is for the whole Game.
    #[serde(default)]
    pub moves: Option<u32>,
}

/// Both Sides' clocks in one Game.
#[derive(Clone, Debug, PartialEq)]
pub struct Clocks {
    tc: TimeControl,
    left: [u64; 2],
    made: [u32; 2],
}

impl Clocks {
    pub fn new(tc: TimeControl) -> Clocks {
        Clocks { tc, left: [tc.base_ms as u64; 2], made: [0; 2] }
    }

    /// What Side `side` (0 the first) is told as it is to move.
    pub fn limit(&self, side: usize) -> Limit {
        let ms = |s: usize| self.left[s].min(u32::MAX as u64) as u32;
        let to_go = self.tc.moves.filter(|m| *m > 0).map(|m| m - self.made[side] % m);
        Limit::Clock { own: ms(side), other: ms(1 - side), inc: self.tc.inc_ms, to_go, first: side == 0 }
    }

    /// Side `side` took `ms` over its move. False: its time ran out, and it has lost.
    pub fn spend(&mut self, side: usize, ms: u64) -> bool {
        if ms > self.left[side] {
            self.left[side] = 0;
            return false;
        }
        self.left[side] -= ms;
        self.left[side] += self.tc.inc_ms as u64;
        self.made[side] += 1;
        if self.tc.moves.is_some_and(|m| m > 0 && self.made[side].is_multiple_of(m)) {
            self.left[side] += self.tc.base_ms as u64;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_spends_its_time_and_earns_the_increment() {
        let mut c = Clocks::new(TimeControl { base_ms: 60_000, inc_ms: 1_000, moves: None });
        assert!(c.spend(0, 5_000));
        assert_eq!(c.limit(1), Limit::Clock { own: 60_000, other: 56_000, inc: 1_000, to_go: None, first: false });
        assert_eq!(c.limit(0), Limit::Clock { own: 56_000, other: 60_000, inc: 1_000, to_go: None, first: true });
    }

    #[test]
    fn a_period_gives_the_base_again_after_its_moves() {
        let mut c = Clocks::new(TimeControl { base_ms: 10_000, inc_ms: 0, moves: Some(2) });
        assert!(matches!(c.limit(0), Limit::Clock { to_go: Some(2), .. }));
        assert!(c.spend(0, 4_000));
        assert!(matches!(c.limit(0), Limit::Clock { own: 6_000, to_go: Some(1), .. }));
        assert!(c.spend(0, 4_000));
        assert!(matches!(c.limit(0), Limit::Clock { own: 12_000, to_go: Some(2), .. }));
    }

    #[test]
    fn a_move_longer_than_the_time_left_loses() {
        let mut c = Clocks::new(TimeControl { base_ms: 1_000, inc_ms: 5_000, moves: None });
        assert!(c.spend(1, 1_000), "the whole time, to the millisecond, is still in time");
        assert!(!c.spend(0, 1_001));
        assert!(matches!(c.limit(0), Limit::Clock { own: 0, .. }));
    }
}
