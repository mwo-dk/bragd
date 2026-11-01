//! The Monrad system as the Danish Chess Union (DSU) defines it:
//! *Monrads turneringssystem* (love.skak.dk, DSU's rules site) and the DSU
//! arbiter course *TD3 Monradsystemet* (November 2025).
//!
//! - Players are numbered by lot; the pairing number is that draw number.
//! - Before each round they are placed by points, then by their placement at
//!   the previous pairing (before round 2: the draw number).
//! - From the top, each player meets the nearest player below not yet met.
//!   When the rest cannot be paired, the last pair made is broken up and its
//!   higher placed player takes the next one down, "always from the top".
//! - Meeting twice is the only absolute rule. White goes to the player who has
//!   had White fewer times; with equal counts, to the lower placed player.
//! - With an odd number of players a dummy is placed last; whoever meets it
//!   has the bye.
//!
//! A game not yet played counts as a draw for the pairing: the caller records
//! it with half the points of a win until the result is known. DSU keeps a
//! player who sends apologies for one round in the pairing; leaving them out
//! of `present` is the director's choice.

use crate::matching::max_weight_matching;
use crate::{Board, Bracket, Colour, Error, Pairing, Player, Round, Rule, Settings};
use std::collections::HashMap;

/// The dummy that makes an odd field even.
const DUMMY: u32 = u32::MAX;

/// Pairs one round under DSU's Monrad rules. `settings.initial_colour` is not
/// used: the colour rule settles round 1 too (no. 2 has White against no. 1).
pub fn pair(settings: &Settings, players: &[Player], present: &[u32]) -> Result<Pairing, Error> {
    let by_tpn = index(players)?;
    let done = players.first().map_or(0, |p| p.history.len());
    if done as u32 >= settings.rounds {
        return Err(Error::TournamentOver);
    }
    for &t in present {
        if !by_tpn.contains_key(&t) {
            return Err(Error::UnknownPlayer(t));
        }
    }
    let mut order = placement(&by_tpn, present, done);
    let mut met: HashMap<u32, Vec<u32>> =
        order.iter().map(|&t| (t, opponents(by_tpn[&t]))).collect();
    if order.len() % 2 == 1 {
        let had_bye: Vec<u32> = order
            .iter()
            .copied()
            .filter(|&t| {
                by_tpn[&t]
                    .history
                    .iter()
                    .any(|r| matches!(r, Round::PairingBye { .. }))
            })
            .collect();
        for &t in &had_bye {
            met.get_mut(&t).unwrap().push(DUMMY);
        }
        met.insert(DUMMY, had_bye);
        order.push(DUMMY);
    }
    let pairs = top_down(&order, &met).ok_or(Error::NoLegalPairing)?;
    let natural = pairs
        .iter()
        .enumerate()
        .all(|(k, &(a, b))| (a, b) == (2 * k, 2 * k + 1));
    let whites = |t: u32| {
        by_tpn[&t]
            .history
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    Round::Played {
                        colour: Colour::White,
                        ..
                    }
                )
            })
            .count()
    };
    let mut boards = Vec::new();
    let mut bye = None;
    for &(hi, lo) in &pairs {
        let (a, b) = (order[hi], order[lo]);
        if b == DUMMY {
            bye = Some(a);
            continue;
        }
        // Fewer whites gets White; with equal counts, the lower placed player.
        boards.push(if whites(a) < whites(b) {
            Board { white: a, black: b }
        } else {
            Board { white: b, black: a }
        });
    }
    order.retain(|&t| t != DUMMY);
    Ok(Pairing {
        boards,
        bye,
        brackets: vec![Bracket {
            rule: Rule::Monrad,
            score: 0,
            residents: order.clone(),
            moved_down: Vec::new(),
            pairs: pairs
                .iter()
                .filter(|&&(_, lo)| lo < order.len())
                .map(|&(hi, lo)| (order[hi], order[lo]))
                .collect(),
            floaters: bye.into_iter().collect(),
            natural,
            exchange: None,
            failures: Vec::new(),
        }],
    })
}

fn index(players: &[Player]) -> Result<HashMap<u32, &Player>, Error> {
    let mut m = HashMap::new();
    let len = players.first().map_or(0, |p| p.history.len());
    for p in players {
        if p.history.len() != len {
            return Err(Error::UnevenHistory(p.tpn));
        }
        if m.insert(p.tpn, p).is_some() {
            return Err(Error::DuplicatePlayer(p.tpn));
        }
    }
    Ok(m)
}

fn opponents(p: &Player) -> Vec<u32> {
    p.history
        .iter()
        .filter_map(|r| match *r {
            Round::Played { opponent, .. } => Some(opponent),
            _ => None,
        })
        .collect()
}

/// The present players in placement order before round `done + 1`: by points,
/// then by the placement before the previous round, from the draw numbers up.
fn placement(by_tpn: &HashMap<u32, &Player>, present: &[u32], done: usize) -> Vec<u32> {
    let mut order: Vec<u32> = present.to_vec();
    order.sort_unstable();
    for r in 1..=done {
        let pts = |t: u32| -> u32 { by_tpn[&t].history[..r].iter().map(|x| x.points()).sum() };
        // A stable sort keeps the previous placement within each score.
        order.sort_by_key(|&t| std::cmp::Reverse(pts(t)));
    }
    order
}

/// Pairs by placement index, each pair (higher placed, lower placed): the
/// first complete pairing in the rules' order of trial.
fn top_down(order: &[u32], met: &HashMap<u32, Vec<u32>>) -> Option<Vec<(usize, usize)>> {
    let n = order.len();
    let can = |a: usize, b: usize| !met[&order[a]].contains(&order[b]);
    // The rest can be paired at all: a perfect matching exists.
    let completes = |free: &[bool]| -> bool {
        let left: Vec<usize> = (0..n).filter(|&i| free[i]).collect();
        let edges: Vec<(usize, usize, i64)> = (0..left.len())
            .flat_map(|x| (x + 1..left.len()).map(move |y| (x, y)))
            .filter(|&(x, y)| can(left[x], left[y]))
            .map(|(x, y)| (x, y, 1))
            .collect();
        max_weight_matching(left.len(), &edges)
            .iter()
            .all(|m| m.is_some())
    };
    let mut free = vec![true; n];
    if !completes(&free) {
        return None;
    }
    let mut pairs = Vec::with_capacity(n / 2);
    while let Some(a) = (0..n).find(|&i| free[i]) {
        free[a] = false;
        // Trying a partner and backing out when the rest cannot be paired is
        // the rules' breaking up of the last pair, done before it is needed.
        let b = (a + 1..n).find(|&b| {
            if !free[b] || !can(a, b) {
                return false;
            }
            free[b] = false;
            let ok = completes(&free);
            free[b] = true;
            ok
        })?;
        free[b] = false;
        pairs.push((a, b));
    }
    Some(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn met(list: &[(u32, u32)], n: u32) -> HashMap<u32, Vec<u32>> {
        let mut m: HashMap<u32, Vec<u32>> = (1..=n).map(|t| (t, Vec::new())).collect();
        for &(a, b) in list {
            m.get_mut(&a).unwrap().push(b);
            m.get_mut(&b).unwrap().push(a);
        }
        m
    }

    #[test]
    fn the_last_pair_is_broken_up_when_the_bottom_cannot_meet() {
        // TD3 slides 23-24: places 1-2 and 3-4 stand; 5 cannot meet 6; 7 and 8
        // have met. Here 5 has met 6, and 7 has met 8: 5 takes 7, 6 takes 8.
        let order: Vec<u32> = (1..=8).collect();
        let m = met(&[(5, 6), (7, 8)], 8);
        let p = top_down(&order, &m).unwrap();
        assert_eq!(p, vec![(0, 1), (2, 3), (4, 6), (5, 7)]);
    }

    #[test]
    fn a_round_that_cannot_be_paired_is_refused() {
        // TD3 slides 26-29: six players where odd and even numbers have all met.
        let order: Vec<u32> = (1..=6).collect();
        let m = met(
            &[
                (1, 2),
                (1, 4),
                (1, 6),
                (3, 2),
                (3, 4),
                (3, 6),
                (5, 2),
                (5, 4),
                (5, 6),
            ],
            6,
        );
        assert_eq!(top_down(&order, &m), None);
    }
}
