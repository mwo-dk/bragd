//! Club ratings: Glicko-2 (the default) and Elo. Pure functions over one rating period.

use serde::{Deserialize, Serialize};

/// How a Club rates its members.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum System {
    #[default]
    Glicko2,
    Elo,
}

/// A member's rating: Elo uses only `rating`; Glicko-2 also its deviation and volatility.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rating {
    pub rating: f64,
    pub deviation: f64,
    pub volatility: f64,
}

impl Default for Rating {
    /// A new member: 1500, deviation 350, volatility 0.06 (Glickman's suggested start).
    fn default() -> Rating {
        Rating { rating: 1500.0, deviation: 350.0, volatility: 0.06 }
    }
}

/// One game of a rating period: the opponent's rating before the period, and the score
/// (1 a win, 0.5 a draw, 0 a loss).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Game {
    pub opponent: Rating,
    pub score: f64,
}

/// Glicko-2's system constant τ, which limits how fast volatility changes.
pub const TAU: f64 = 0.5;
/// The Elo K-factor.
pub const K: f64 = 20.0;

/// The rating after one Glicko-2 rating period (Glickman, "Example of the Glicko-2 system",
/// steps 1–8). With no games only the deviation grows.
pub fn glicko2(player: Rating, games: &[Game]) -> Rating {
    todo!()
}

/// The rating after one period of Elo games, all against the opponents' ratings before it.
pub fn elo(player: Rating, games: &[Game]) -> Rating {
    todo!()
}

/// The rating after one period under `system`.
pub fn rate(system: System, player: Rating, games: &[Game]) -> Rating {
    match system {
        System::Glicko2 => glicko2(player, games),
        System::Elo => elo(player, games),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(rating: f64, deviation: f64) -> Rating {
        Rating { rating, deviation, volatility: 0.06 }
    }

    /// Glickman's worked example: 1500/200/0.06 beats 1400/30, loses to 1550/100 and 1700/300,
    /// and comes out at 1464.06, deviation 151.52, volatility 0.05999.
    #[test]
    fn glicko2_matches_glickmans_worked_example() {
        let games = [Game { opponent: r(1400.0, 30.0), score: 1.0 }, Game { opponent: r(1550.0, 100.0), score: 0.0 }, Game { opponent: r(1700.0, 300.0), score: 0.0 }];
        let after = glicko2(r(1500.0, 200.0), &games);
        assert!((after.rating - 1464.06).abs() < 0.01, "{after:?}");
        assert!((after.deviation - 151.52).abs() < 0.01, "{after:?}");
        assert!((after.volatility - 0.05999).abs() < 0.00001, "{after:?}");
    }

    #[test]
    fn glicko2_without_games_only_grows_the_deviation() {
        let after = glicko2(r(1500.0, 200.0), &[]);
        assert_eq!(after.rating, 1500.0);
        assert_eq!(after.volatility, 0.06);
        // φ* = sqrt(φ² + σ²), on the Glicko-2 scale.
        let phi = 200.0 / 173.7178;
        assert!((after.deviation - (phi * phi + 0.06f64 * 0.06).sqrt() * 173.7178).abs() < 1e-9, "{after:?}");
    }

    #[test]
    fn elo_moves_by_k_times_the_surprise() {
        // Equal ratings: a win is worth K/2.
        let after = elo(r(1500.0, 350.0), &[Game { opponent: r(1500.0, 350.0), score: 1.0 }]);
        assert!((after.rating - 1510.0).abs() < 1e-9, "{after:?}");
        // 400 points stronger: expected 10/11, so a draw costs K·(0.5 − 10/11).
        let after = elo(r(1900.0, 350.0), &[Game { opponent: r(1500.0, 350.0), score: 0.5 }]);
        assert!((after.rating - (1900.0 + K * (0.5 - 10.0 / 11.0))).abs() < 1e-9, "{after:?}");
        // The deviation and volatility are left as they were.
        assert_eq!((after.deviation, after.volatility), (350.0, 0.06));
        assert_eq!(elo(r(1700.0, 350.0), &[]).rating, 1700.0);
        assert_eq!(rate(System::Elo, r(1500.0, 350.0), &[Game { opponent: r(1500.0, 350.0), score: 0.0 }]).rating, 1490.0);
    }
}
