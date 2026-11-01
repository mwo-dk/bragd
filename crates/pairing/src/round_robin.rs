//! Round-robin by FIDE's Berger tables (Handbook C.05 Annex 1).
//!
//! Players are numbered 1 to N by lot. With an odd number the highest number,
//! N + 1, counts as a bye. Every round of the table for an even `size` follows
//! one rule: in round `r` the players `a` and `b` below `size` meet when
//! `a + b ≡ r + 1 (mod size - 1)`; the one left over meets `size`, who is Black
//! in odd rounds and White in even ones. Of the others, White goes to the
//! player from whom the opponent is an odd number of steps on (mod `size - 1`).
//!
//! In a double round-robin the second cycle repeats the first with colours
//! reversed, as at FIDE's Candidates 2026. C.05 Annex 1 also recommends
//! swapping the last two rounds of the first cycle, so that no one has the
//! same colour three times running; `swap_last_two` asks for that.

use crate::{Board, Bracket, Error, Pairing, Rule};

/// Pairs round `round` (from 1) of a round-robin of `players` players over
/// `cycles` cycles (1 for single, 2 for double).
pub fn pair(players: u32, cycles: u32, swap_last_two: bool, round: u32) -> Result<Pairing, Error> {
    if players < 2 || cycles == 0 {
        return Err(Error::NoLegalPairing);
    }
    let size = players + players % 2;
    let per_cycle = size - 1;
    if round == 0 || round > per_cycle * cycles {
        return Err(Error::TournamentOver);
    }
    let cycle = (round - 1) / per_cycle + 1;
    let mut row = (round - 1) % per_cycle + 1;
    if swap_last_two && cycles > 1 && cycle == 1 && per_cycle >= 2 && row + 1 >= per_cycle {
        // C.05 Annex 1: the last two rounds of the first cycle swap places.
        row = if row == per_cycle {
            per_cycle - 1
        } else {
            per_cycle
        };
    }
    let mut boards = Vec::new();
    let mut bye = None;
    let mut pairs = Vec::new();
    for (white, black) in table(size, row) {
        let (white, black) = if cycle.is_multiple_of(2) {
            (black, white)
        } else {
            (white, black)
        };
        if white > players || black > players {
            bye = Some(white.min(black));
            continue;
        }
        boards.push(Board { white, black });
        pairs.push((white.min(black), white.max(black)));
    }
    Ok(Pairing {
        boards,
        bye,
        brackets: vec![Bracket {
            rule: Rule::Berger {
                size,
                round: row,
                cycle,
            },
            score: 0,
            residents: (1..=players).collect(),
            moved_down: Vec::new(),
            pairs,
            floaters: bye.into_iter().collect(),
            natural: true,
            exchange: None,
            failures: Vec::new(),
        }],
    })
}

/// Round `r` of the Berger table for an even `size`, boards in printed order,
/// each as (White, Black).
pub fn table(size: u32, r: u32) -> Vec<(u32, u32)> {
    let m = size - 1;
    // Numbers 1..=m, read modulo m.
    let wrap = |x: i64| ((x - 1).rem_euclid(m as i64) + 1) as u32;
    // The player who meets `size`: 2a ≡ r + 1 (mod m).
    let first = (1..=m)
        .find(|&a| (2 * a) % m == (r + 1) % m)
        .expect("m is odd");
    let mut out = vec![if r % 2 == 1 {
        (first, size)
    } else {
        (size, first)
    }];
    for k in 1..=(m / 2) as i64 {
        let (a, b) = (wrap(first as i64 + k), wrap(first as i64 - k));
        // White is the player from whom the opponent is an odd number of steps on.
        out.push(if (b + m - a) % m % 2 == 1 {
            (a, b)
        } else {
            (b, a)
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pair_meets_once_with_a_bye_for_odd_fields() {
        for players in 2..=21 {
            let size = players + players % 2;
            let mut met = std::collections::HashSet::new();
            let mut byes = Vec::new();
            for round in 1..size {
                let p = pair(players, 1, false, round).unwrap();
                for b in &p.boards {
                    assert!(met.insert((b.white.min(b.black), b.white.max(b.black))));
                }
                byes.extend(p.bye);
            }
            let n = players as usize;
            assert_eq!(met.len(), n * (n - 1) / 2, "{players}");
            if players % 2 == 1 {
                byes.sort_unstable();
                assert_eq!(byes, (1..=players).collect::<Vec<_>>());
            }
        }
    }

    #[test]
    fn rounds_outside_the_tournament_are_refused() {
        assert_eq!(pair(6, 1, false, 6), Err(Error::TournamentOver));
        assert_eq!(pair(6, 2, false, 11), Err(Error::TournamentOver));
        assert_eq!(pair(1, 1, false, 1), Err(Error::NoLegalPairing));
    }
}
