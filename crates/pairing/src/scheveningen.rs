//! Scheveningen: two teams of equal size, every player of one meeting every
//! player of the other, in as many rounds as a team has players.
//!
//! No federation publishes a table for it, and the events on chess-results.com
//! each draw their own. Bragd rotates: in round `r`, board `i` of team A meets
//! board `i + r - 1` of team B, counted round the team. White goes to team A
//! when the two board numbers add up to an even number, so every player has
//! White in half their games, give or take one; with an even team size every
//! player alternates colours. A double Scheveningen repeats the rounds with
//! colours reversed.
//!
//! Team A's boards are numbered 1 to `size`, team B's `size + 1` to `2 * size`.

use crate::{Board, Bracket, Error, Pairing, Rule};

/// Pairs round `round` (from 1) of a Scheveningen between teams of `size`
/// players over `cycles` cycles.
pub fn pair(size: u32, cycles: u32, round: u32) -> Result<Pairing, Error> {
    if size == 0 || cycles == 0 {
        return Err(Error::NoLegalPairing);
    }
    if round == 0 || round > size * cycles {
        return Err(Error::TournamentOver);
    }
    let cycle = (round - 1) / size + 1;
    let r = (round - 1) % size + 1;
    let mut boards = Vec::new();
    let mut pairs = Vec::new();
    for i in 1..=size {
        let j = (i + r - 2) % size + 1;
        let (a, b) = (i, size + j);
        let a_white = (i + j).is_multiple_of(2) != cycle.is_multiple_of(2);
        boards.push(if a_white {
            Board { white: a, black: b }
        } else {
            Board { white: b, black: a }
        });
        pairs.push((a, b));
    }
    Ok(Pairing {
        boards,
        bye: None,
        brackets: vec![Bracket {
            rule: Rule::Scheveningen { round: r, cycle },
            score: 0,
            residents: (1..=2 * size).collect(),
            moved_down: Vec::new(),
            pairs,
            floaters: Vec::new(),
            natural: true,
            exchange: None,
            failures: Vec::new(),
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn everyone_meets_the_other_team_once_with_balanced_colours() {
        for size in 1..=12 {
            let mut met = HashSet::new();
            let mut whites: HashMap<u32, u32> = HashMap::new();
            for round in 1..=size {
                for b in pair(size, 1, round).unwrap().boards {
                    let (a, x) = (b.white.min(b.black), b.white.max(b.black));
                    assert!(a <= size && x > size, "same team: {b:?}");
                    assert!(met.insert((a, x)));
                    *whites.entry(b.white).or_default() += 1;
                }
            }
            assert_eq!(met.len() as u32, size * size);
            for p in 1..=2 * size {
                let w = whites.get(&p).copied().unwrap_or(0);
                assert!(
                    w.abs_diff(size - w) <= 1,
                    "size {size}: player {p} has {w} whites"
                );
            }
        }
    }

    #[test]
    fn with_an_even_team_size_colours_alternate() {
        let size = 6;
        let colour = |p: u32, round: u32| {
            pair(size, 1, round)
                .unwrap()
                .boards
                .iter()
                .any(|b| b.white == p)
        };
        for p in 1..=2 * size {
            for round in 1..size {
                assert_ne!(colour(p, round), colour(p, round + 1), "player {p}");
            }
        }
    }

    #[test]
    fn the_second_cycle_reverses_colours() {
        let first = pair(4, 2, 2).unwrap().boards;
        let second = pair(4, 2, 6).unwrap().boards;
        let flipped: Vec<Board> = first
            .iter()
            .map(|b| Board {
                white: b.black,
                black: b.white,
            })
            .collect();
        assert_eq!(second, flipped);
    }
}
