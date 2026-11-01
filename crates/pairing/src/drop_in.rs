//! The Drop-in tournament ("spil når du vil"): whoever turns up on an evening
//! is paired among themselves, and the standings run over the whole season.
//!
//! - Pairing score: the points so far, plus half a win for every round a
//!   player was away. The half point counts for the pairing only.
//! - Players are ranked by pairing score, then by pairing number. With an odd
//!   number, the bye goes to the lowest ranked of those with the fewest byes.
//! - From the top, each player meets the highest ranked player below whom they
//!   have met least often. Players turn up again and again over a season, so
//!   meeting twice is allowed, only put off.
//! - Colours as in Monrad: fewer Whites gets White; with equal counts, the
//!   lower ranked player.

use crate::{Board, Bracket, Colour, Error, Pairing, Player, Round, Rule, Settings};
use std::collections::HashMap;

/// Pairs one round of a Drop-in tournament among the players `present`.
pub fn pair(settings: &Settings, players: &[Player], present: &[u32]) -> Result<Pairing, Error> {
    let by_tpn = index(players)?;
    for &t in present {
        if !by_tpn.contains_key(&t) {
            return Err(Error::UnknownPlayer(t));
        }
    }
    let score = |t: u32| pairing_score(settings, by_tpn[&t]);
    let mut order: Vec<u32> = present.to_vec();
    order.sort_by_key(|&t| (std::cmp::Reverse(score(t)), t));
    if let Some(w) = order.windows(2).find(|w| w[0] == w[1]) {
        return Err(Error::DuplicatePlayer(w[0]));
    }
    let byes = |t: u32| {
        by_tpn[&t]
            .history
            .iter()
            .filter(|r| matches!(r, Round::PairingBye { .. }))
            .count()
    };
    let bye = (order.len() % 2 == 1).then(|| {
        let fewest = order.iter().map(|&t| byes(t)).min().unwrap_or(0);
        *order.iter().rev().find(|&&t| byes(t) == fewest).unwrap()
    });
    let mut left: Vec<u32> = order.iter().copied().filter(|&t| Some(t) != bye).collect();
    let met = |a: u32, b: u32| {
        by_tpn[&a]
            .history
            .iter()
            .filter(|r| matches!(r, Round::Played { opponent, .. } if *opponent == b))
            .count()
    };
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
    let mut pairs = Vec::new();
    let mut boards = Vec::new();
    while !left.is_empty() {
        let a = left.remove(0);
        // The first of the least met: `min_by_key` keeps the first on ties.
        let (k, &b) = left
            .iter()
            .enumerate()
            .min_by_key(|&(_, &b)| met(a, b))
            .ok_or(Error::NoLegalPairing)?;
        left.remove(k);
        pairs.push((a, b));
        boards.push(if whites(a) < whites(b) {
            Board { white: a, black: b }
        } else {
            Board { white: b, black: a }
        });
    }
    let natural = pairs.iter().all(|&(a, b)| met(a, b) == 0)
        && pairs.iter().enumerate().all(|(k, &(a, b))| {
            let rest: Vec<u32> = order.iter().copied().filter(|&t| Some(t) != bye).collect();
            (rest[2 * k], rest[2 * k + 1]) == (a, b)
        });
    Ok(Pairing {
        boards,
        bye,
        brackets: vec![Bracket {
            rule: Rule::DropIn,
            score: 0,
            residents: order,
            moved_down: Vec::new(),
            pairs,
            floaters: bye.into_iter().collect(),
            natural,
            exchange: None,
            failures: Vec::new(),
        }],
    })
}

/// Points so far plus half a win for each round away.
pub fn pairing_score(settings: &Settings, p: &Player) -> u32 {
    p.history
        .iter()
        .map(|r| match r {
            Round::Absent { .. } => settings.win / 2,
            r => r.points(),
        })
        .sum()
}

fn index(players: &[Player]) -> Result<HashMap<u32, &Player>, Error> {
    let mut m = HashMap::new();
    for p in players {
        if m.insert(p.tpn, p).is_some() {
            return Err(Error::DuplicatePlayer(p.tpn));
        }
    }
    Ok(m)
}

/// How the standings are ranked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// All points scored.
    Total,
    /// Points as a share of the most that could have been scored in the
    /// rounds played. The default.
    Percentage,
    /// The best `n` results only.
    Best(u32),
}

/// One line of the standings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub tpn: u32,
    /// Points that count: all of them, or the best `n` results.
    pub points: u32,
    /// Rounds that count: the rounds played (a bye or forfeit included), or at most `n`.
    pub rounds: u32,
}

/// The standings, best first; ties by pairing number. An absence is not a
/// result: it counts for the pairing only. Under [`Standing::Percentage`] a
/// player with no rounds comes last.
pub fn standings(players: &[Player], by: Standing) -> Vec<Row> {
    let mut rows: Vec<Row> = players
        .iter()
        .map(|p| {
            let mut results: Vec<u32> = p
                .history
                .iter()
                .filter(|r| !matches!(r, Round::Absent { .. }))
                .map(|r| r.points())
                .collect();
            if let Standing::Best(n) = by {
                results.sort_unstable_by(|a, b| b.cmp(a));
                results.truncate(n as usize);
            }
            Row {
                tpn: p.tpn,
                points: results.iter().sum(),
                rounds: results.len() as u32,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        let better = match by {
            // a/ra against b/rb, by cross-multiplying; no rounds goes last.
            Standing::Percentage => match (a.rounds, b.rounds) {
                (0, 0) => std::cmp::Ordering::Equal,
                (0, _) => std::cmp::Ordering::Greater,
                (_, 0) => std::cmp::Ordering::Less,
                _ => (u64::from(b.points) * u64::from(a.rounds))
                    .cmp(&(u64::from(a.points) * u64::from(b.rounds))),
            },
            _ => b.points.cmp(&a.points),
        };
        better.then(a.tpn.cmp(&b.tpn))
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Settings = Settings {
        rounds: 99,
        win: 2,
        loss: 0,
        initial_colour: Colour::White,
        baku: None,
    };

    fn played(opponent: u32, colour: Colour, points: u32) -> Round {
        Round::Played {
            opponent,
            colour,
            points,
        }
    }

    fn player(tpn: u32, history: Vec<Round>) -> Player {
        Player { tpn, history }
    }

    #[test]
    fn an_absence_counts_half_a_point_for_pairing_only() {
        let away = player(1, vec![Round::Absent { points: 0 }]);
        let lost = player(2, vec![played(3, Colour::White, 0)]);
        let won = player(3, vec![played(2, Colour::Black, 2)]);
        assert_eq!(pairing_score(&S, &away), 1);
        let p = pair(&S, &[away.clone(), lost.clone(), won.clone()], &[1, 2, 3]).unwrap();
        // 3 (2 points) is ranked above 1 (1 for pairing) above 2 (0): 2 has the bye.
        assert_eq!(p.brackets[0].residents, vec![3, 1, 2]);
        assert_eq!(p.bye, Some(2));
        let st = standings(&[away, lost, won], Standing::Total);
        assert_eq!(
            st.iter().map(|r| (r.tpn, r.points)).collect::<Vec<_>>(),
            [(3, 2), (1, 0), (2, 0)]
        );
    }

    #[test]
    fn only_those_present_are_paired_least_met_first() {
        let h = |opp: u32, c| vec![played(opp, c, 1)];
        let players = vec![
            player(1, h(2, Colour::White)),
            player(2, h(1, Colour::Black)),
            player(3, h(4, Colour::White)),
            player(4, h(3, Colour::Black)),
            player(5, vec![Round::Absent { points: 0 }]),
        ];
        let p = pair(&S, &players, &[1, 2, 3, 4]).unwrap();
        // All on 1 point; 1 has met 2, so 1 meets 3, and 2 meets 4.
        assert_eq!(p.brackets[0].pairs, vec![(1, 3), (2, 4)]);
        assert_eq!(p.bye, None);
        // Both 1 and 3 had White once: the lower ranked, 3, has White.
        assert_eq!(p.boards[0], Board { white: 3, black: 1 });
    }

    #[test]
    fn a_repeat_is_allowed_when_nothing_else_is_left() {
        let players = vec![
            player(1, vec![played(2, Colour::White, 1)]),
            player(2, vec![played(1, Colour::Black, 1)]),
        ];
        let p = pair(&S, &players, &[1, 2]).unwrap();
        assert_eq!(p.boards, vec![Board { white: 2, black: 1 }]);
        assert!(!p.brackets[0].natural);
    }

    #[test]
    fn standings_by_percentage_and_best_results() {
        // 1: two wins in two evenings. 2: three wins and a loss in four.
        let players = vec![
            player(
                1,
                vec![
                    played(9, Colour::White, 2),
                    played(9, Colour::White, 2),
                    Round::Absent { points: 0 },
                    Round::Absent { points: 0 },
                ],
            ),
            player(
                2,
                vec![
                    played(9, Colour::White, 2),
                    played(9, Colour::White, 2),
                    played(9, Colour::White, 2),
                    played(9, Colour::White, 0),
                ],
            ),
            player(3, vec![Round::Absent { points: 0 }; 4]),
        ];
        let order = |by| -> Vec<u32> { standings(&players, by).iter().map(|r| r.tpn).collect() };
        assert_eq!(order(Standing::Percentage), [1, 2, 3]);
        assert_eq!(order(Standing::Total), [2, 1, 3]);
        // Best two: both have 4 points; ties go by pairing number.
        assert_eq!(order(Standing::Best(2)), [1, 2, 3]);
        assert_eq!(order(Standing::Best(3)), [2, 1, 3]);
    }
}
