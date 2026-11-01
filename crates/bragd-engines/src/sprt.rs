//! The statistics of an engine match: the Elo difference a score means, with its 95 % margin,
//! and the sequential probability ratio test (SPRT) that stops a two-engine match once the
//! games say which of two Elo differences is the truth.
//!
//! Results come as frequencies: three (losses, draws, wins) for single games, or five for game
//! pairs played from the same opening with the colours swapped (both lost, a loss and a draw,
//! a win and a loss or two draws, a win and a draw, both won). The log-likelihood ratio is the
//! generalised one (GSPRT) with the expected score as its statistic, in logistic Elo, as
//! fishtest computes it: each hypothesis' most likely distribution with the expected score it
//! names (the secular equation), and the log of their ratio over the observed games.

/// The expected score of a player `elo` stronger (logistic Elo).
pub fn score_of(elo: f64) -> f64 {
    todo!()
}

/// The Elo difference a score means, the score kept within 0.001 and 0.999.
pub fn elo_of(score: f64) -> f64 {
    todo!()
}

/// The log-likelihood ratio of `elo1` against `elo0` for `results` (three or five
/// frequencies). A frequency of 0 counts as 0.001, so a first lost game has a finite ratio.
pub fn llr(results: &[u64], elo0: f64, elo1: f64) -> f64 {
    todo!()
}

/// The bounds the ratio is held against: below the first, `elo0` is accepted; above the
/// second, `elo1`. `alpha` and `beta`: the chances of the two errors.
pub fn bounds(alpha: f64, beta: f64) -> (f64, f64) {
    todo!()
}

/// The Elo difference `results` (three or five frequencies) show, and half the width of its
/// 95 % confidence interval. None before the first game.
pub fn elo(results: &[u64]) -> Option<(f64, f64)> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64, eps: f64) {
        assert!((a - b).abs() < eps, "{a} is not {b}");
    }

    /// The bounds fishtest shows for α = β = 0.05: ±2.94, that is ±ln 19.
    #[test]
    fn bounds_are_wald_s() {
        let (lo, hi) = bounds(0.05, 0.05);
        near(lo, -2.944_438_979, 1e-9);
        near(hi, 2.944_438_979, 1e-9);
        let (lo, hi) = bounds(0.05, 0.1);
        near(lo, (0.1f64 / 0.95).ln(), 1e-12);
        near(hi, (0.9f64 / 0.05).ln(), 1e-12);
    }

    #[test]
    fn elo_and_score() {
        near(elo_of(0.75), 190.848_501_888, 1e-6);
        near(elo_of(0.5), 0.0, 1e-12);
        near(score_of(190.848_501_888), 0.75, 1e-9);
        near(elo_of(1.0), elo_of(0.999), 1e-12);
    }

    /// Against an independent implementation of fishtest's GSPRT (SciPy's root finder on the
    /// secular equation), for single games and for game pairs.
    #[test]
    fn llr_against_known_values() {
        near(llr(&[4000, 9000, 4500], 0.0, 5.0), 11.087_313_091, 1e-6);
        near(llr(&[100, 200, 130], 0.0, 10.0), 1.283_055_095, 1e-6);
        near(llr(&[10, 20, 10], -5.0, 5.0), 0.0, 1e-9);
        near(llr(&[1200, 2100, 4800, 2200, 1300], 0.0, 2.0), 2.178_358_704, 1e-6);
        near(llr(&[30, 40, 60, 45, 25], -10.0, 10.0), -0.378_132_503, 1e-6);
        // Nothing played yet: no evidence either way.
        near(llr(&[0, 0, 0], 0.0, 5.0), 0.0, 1e-6);
    }

    /// Elo ± its 95 % margin, as fishtest's `get_elo` gives them.
    #[test]
    fn elo_with_its_margin() {
        let (e, m) = elo(&[4000, 9000, 4500]).unwrap();
        near(e, 9.929_433_490, 1e-6);
        near(m, 3.587_555_981, 1e-6);
        let (e, m) = elo(&[100, 200, 130]).unwrap();
        near(e, 24.279_136_141, 1e-6);
        near(m, 24.063_325_048, 1e-6);
        let (e, m) = elo(&[30, 40, 60, 45, 25]).unwrap();
        near(e, -4.343_171_035, 1e-6);
        near(m, 29.802_659_963, 1e-6);
        assert_eq!(elo(&[0, 0, 0]), None);
    }
}
