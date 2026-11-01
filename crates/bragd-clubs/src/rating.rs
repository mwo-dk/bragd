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
    const SCALE: f64 = 173.7178;
    const EPSILON: f64 = 0.000001;

    let mu = (player.rating - 1500.0) / SCALE;
    let phi = player.deviation / SCALE;
    if games.is_empty() {
        return Rating {
            rating: player.rating,
            deviation: SCALE * (phi * phi + player.volatility * player.volatility).sqrt(),
            volatility: player.volatility,
        };
    }

    let g = |opponent_phi: f64| {
        1.0 / (1.0 + 3.0 * opponent_phi * opponent_phi / std::f64::consts::PI.powi(2)).sqrt()
    };
    let mut sum_g_score = 0.0;
    let mut sum_g_squared_variance = 0.0;
    for game in games {
        let opponent_mu = (game.opponent.rating - 1500.0) / SCALE;
        let opponent_phi = game.opponent.deviation / SCALE;
        let gj = g(opponent_phi);
        let expected = 1.0 / (1.0 + (-gj * (mu - opponent_mu)).exp());
        sum_g_score += gj * (game.score - expected);
        sum_g_squared_variance += gj * gj * expected * (1.0 - expected);
    }
    let variance = 1.0 / sum_g_squared_variance;
    let delta = variance * sum_g_score;
    let a = (player.volatility * player.volatility).ln();
    let f = |x: f64| {
        let exp_x = x.exp();
        exp_x * (delta * delta - phi * phi - variance - exp_x)
            / (2.0 * (phi * phi + variance + exp_x).powi(2))
            - (x - a) / (TAU * TAU)
    };

    let mut lower = a;
    let mut upper = if delta * delta > phi * phi + variance {
        (delta * delta - phi * phi - variance).ln()
    } else {
        let mut k = 1;
        while f(a - (k as f64) * TAU) < 0.0 {
            k += 1;
        }
        a - (k as f64) * TAU
    };
    let mut f_lower = f(lower);
    let mut f_upper = f(upper);
    while (upper - lower).abs() > EPSILON {
        let candidate = lower + (lower - upper) * f_lower / (f_upper - f_lower);
        let f_candidate = f(candidate);
        if f_candidate * f_upper <= 0.0 {
            lower = upper;
            f_lower = f_upper;
        } else {
            f_lower /= 2.0;
        }
        upper = candidate;
        f_upper = f_candidate;
    }
    let volatility = (lower / 2.0).exp();
    let phi_star = (phi * phi + volatility * volatility).sqrt();
    let new_phi = 1.0 / (1.0 / (phi_star * phi_star) + 1.0 / variance).sqrt();
    let new_mu = mu + new_phi * new_phi * sum_g_score;

    Rating {
        rating: SCALE * new_mu + 1500.0,
        deviation: SCALE * new_phi,
        volatility,
    }
}

/// The rating after one period of Elo games, all against the opponents' ratings before it.
pub fn elo(player: Rating, games: &[Game]) -> Rating {
    let change: f64 = games
        .iter()
        .map(|game| {
            let expected = 1.0 / (1.0 + 10.0_f64.powf((game.opponent.rating - player.rating) / 400.0));
            game.score - expected
        })
        .sum();
    Rating { rating: player.rating + K * change, deviation: player.deviation, volatility: player.volatility }
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
