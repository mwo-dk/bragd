//! Round-robin rounds by Berger tables (FIDE C.05, Annex 1): who plays whom, and with which
//! Side, in each round. Kept apart from the matches so it can move into a pairing crate whole.

/// The rounds of a round robin of `n` players, numbered from 0: each round's pairs as (first
/// Side, second Side), as the Berger table for `n` (or `n + 1` when `n` is odd, the extra one
/// being the bye, whose pairs are left out) has them.
pub fn rounds(n: usize) -> Vec<Vec<(usize, usize)>> {
    if n == 0 {
        return Vec::new();
    }

    let m = n + n % 2;
    let fixed = m - 1;
    let half = m / 2;

    (0..fixed)
        .map(|r| {
            let p = (r * half) % fixed;
            let mut pairs = Vec::with_capacity(half);
            let fixed_pair = if r % 2 == 0 { (p, fixed) } else { (fixed, p) };
            if fixed_pair.0 < n && fixed_pair.1 < n {
                pairs.push(fixed_pair);
            }

            for k in 1..half {
                let pair = ((p + k) % fixed, (p + fixed - k) % fixed);
                if pair.0 < n && pair.1 < n {
                    pairs.push(pair);
                }
            }
            pairs
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FIDE's tables, numbered from 1.
    fn table(n: usize) -> Vec<Vec<(usize, usize)>> {
        rounds(n).into_iter().map(|r| r.into_iter().map(|(a, b)| (a + 1, b + 1)).collect()).collect()
    }

    #[test]
    fn fide_tables() {
        assert_eq!(table(4), [vec![(1, 4), (2, 3)], vec![(4, 3), (1, 2)], vec![(2, 4), (3, 1)]]);
        assert_eq!(
            table(6),
            [
                vec![(1, 6), (2, 5), (3, 4)],
                vec![(6, 4), (5, 3), (1, 2)],
                vec![(2, 6), (3, 1), (4, 5)],
                vec![(6, 5), (1, 4), (2, 3)],
                vec![(3, 6), (4, 2), (5, 1)],
            ]
        );
    }

    #[test]
    fn odd_numbers_sit_out_in_turn_and_everyone_meets_once() {
        // Three players: the table for four, without the pairs of player 4.
        assert_eq!(table(3), [vec![(2, 3)], vec![(1, 2)], vec![(3, 1)]]);
        for n in 2..=11 {
            let mut met = std::collections::BTreeSet::new();
            for r in rounds(n) {
                for (a, b) in r {
                    assert!(a < n && b < n && a != b);
                    assert!(met.insert((a.min(b), a.max(b))), "{n}: {a}-{b} twice");
                }
            }
            assert_eq!(met.len(), n * (n - 1) / 2, "{n}");
        }
        assert!(rounds(1).iter().all(Vec::is_empty) && rounds(0).is_empty());
    }
}
