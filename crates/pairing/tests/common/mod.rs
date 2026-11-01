//! Replaying published tournaments round by round.
#![allow(dead_code)] // Each test file uses part of it.

use pairing::dutch::{self, Pairing};
use pairing::trf::{Trf, TrfGame, parse};
use pairing::{Board, Colour, Player, Round, Settings};

/// Points for a win; a draw is half of it.
pub const WIN: u32 = 2;

pub fn load(text: &str) -> Trf {
    parse(text).expect("fixture parses")
}

fn round_of(g: &TrfGame) -> Round {
    let pts = |c: char| match c {
        '1' | 'W' | '+' | 'F' | 'U' => WIN,
        '=' | 'D' | 'H' => WIN / 2,
        _ => 0,
    };
    match (g.opponent, g.result) {
        (_, 'U') => Round::PairingBye { points: WIN },
        (Some(opponent), '+' | '-') => Round::Forfeit {
            opponent,
            points: pts(g.result),
        },
        (Some(opponent), r) => Round::Played {
            opponent,
            colour: g.colour.expect("a played game has a colour"),
            points: pts(r),
        },
        (None, r) => Round::Absent { points: pts(r) },
    }
}

fn cell(t: &Trf, tpn: u32, round: usize) -> TrfGame {
    let p = t.players.iter().find(|p| p.start_rank == tpn).unwrap();
    p.rounds.get(round - 1).copied().unwrap_or(TrfGame {
        opponent: None,
        colour: None,
        result: ' ',
    })
}

/// Had the player entered the tournament when `round` was paired? A file only
/// holds the final TPNs (C.04.2 2.5). A player counts as entered once paired,
/// or after a round in which they scored without playing; a player whose
/// first rounds are byes is taken to be a late entry given those byes.
fn entered(t: &Trf, tpn: u32, round: usize) -> bool {
    let paired = |r: usize| {
        let g = cell(t, tpn, r);
        g.opponent.is_some() || g.result == 'U'
    };
    (1..=round).any(paired) || (1..round).any(|r| matches!(cell(t, tpn, r).result, 'H' | 'F'))
}

/// The TPNs in force when `round` was paired: the final ones, closed up over
/// players who had not entered yet (C.04.2 2.4-2.5).
pub fn tpn_at(t: &Trf, round: usize) -> std::collections::HashMap<u32, u32> {
    let mut ranks: Vec<u32> = t
        .players
        .iter()
        .map(|p| p.start_rank)
        .filter(|&n| entered(t, n, round))
        .collect();
    ranks.sort_unstable();
    ranks
        .into_iter()
        .enumerate()
        .map(|(i, n)| (n, i as u32 + 1))
        .collect()
}

/// Every entered player with their rounds before `round`, under that round's TPNs.
pub fn players(t: &Trf, round: usize) -> Vec<Player> {
    let map = tpn_at(t, round);
    let tpn = |n: u32| map[&n];
    t.players
        .iter()
        .filter(|p| map.contains_key(&p.start_rank))
        .map(|p| Player {
            tpn: tpn(p.start_rank),
            history: (1..round)
                .map(|r| match round_of(&cell(t, p.start_rank, r)) {
                    Round::Played {
                        opponent,
                        colour,
                        points,
                    } => Round::Played {
                        opponent: tpn(opponent),
                        colour,
                        points,
                    },
                    Round::Forfeit { opponent, points } => Round::Forfeit {
                        opponent: tpn(opponent),
                        points,
                    },
                    other => other,
                })
                .collect(),
        })
        .collect()
}

/// Players paired in `round`, under its TPNs: they have an opponent or the pairing-allocated bye.
pub fn paired_in(t: &Trf, round: usize) -> Vec<u32> {
    let map = tpn_at(t, round);
    t.players
        .iter()
        .filter(|p| {
            let g = cell(t, p.start_rank, round);
            g.opponent.is_some() || g.result == 'U'
        })
        .map(|p| map[&p.start_rank])
        .collect()
}

/// The published pairing of `round` under its TPNs: boards sorted, and the bye.
pub fn published(t: &Trf, round: usize) -> (Vec<Board>, Option<u32>) {
    let map = tpn_at(t, round);
    let mut boards = Vec::new();
    let mut bye = None;
    for p in &t.players {
        let g = cell(t, p.start_rank, round);
        if g.result == 'U' {
            bye = Some(map[&p.start_rank]);
        }
        if let (Some(o), Some(Colour::White)) = (g.opponent, g.colour) {
            boards.push(Board {
                white: map[&p.start_rank],
                black: map[&o],
            });
        }
    }
    boards.sort_by_key(|b| (b.white, b.black));
    (boards, bye)
}

pub fn settings(t: &Trf) -> Settings {
    Settings {
        rounds: t.rounds.expect("XXR"),
        win: WIN,
        loss: 0,
        initial_colour: t.initial_colour.expect("XXC"),
    }
}

/// A pairing system's entry point.
pub type System = fn(&Settings, &[Player], &[u32]) -> Result<Pairing, pairing::Error>;

/// Pairs `round` from the history before it.
pub fn pair(t: &Trf, round: usize) -> Pairing {
    pair_with(dutch::pair, t, round)
}

pub fn pair_with(system: System, t: &Trf, round: usize) -> Pairing {
    system(&settings(t), &players(t, round), &paired_in(t, round)).expect("round pairs")
}

/// Ours against the published one: (equal, a description of the difference).
pub fn compare(t: &Trf, round: usize) -> (bool, String) {
    compare_with(dutch::pair, t, round)
}

pub fn compare_with(system: System, t: &Trf, round: usize) -> (bool, String) {
    let ours = pair_with(system, t, round);
    let mut mine = ours.boards.clone();
    mine.sort_by_key(|b| (b.white, b.black));
    let (theirs, bye) = published(t, round);
    if mine == theirs && ours.bye == bye {
        return (true, String::new());
    }
    let fmt = |v: &[Board]| {
        v.iter()
            .map(|b| format!("{}-{}", b.white, b.black))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let only_mine: Vec<Board> = mine
        .iter()
        .filter(|b| !theirs.contains(b))
        .copied()
        .collect();
    let only_theirs: Vec<Board> = theirs
        .iter()
        .filter(|b| !mine.contains(b))
        .copied()
        .collect();
    (
        false,
        format!(
            "round {round}: ours {} (bye {:?}); published {} (bye {:?})\n  brackets: {:?}",
            fmt(&only_mine),
            ours.bye,
            fmt(&only_theirs),
            bye,
            ours.brackets
                .iter()
                .map(|b| (
                    b.score,
                    &b.moved_down,
                    &b.residents,
                    &b.pairs,
                    &b.floaters,
                    &b.failures
                ))
                .collect::<Vec<_>>()
        ),
    )
}

/// The published round judged against the rules.
pub fn judge(t: &Trf, round: usize) -> dutch::Verdict {
    let (boards, _) = published(t, round);
    dutch::assess(
        &settings(t),
        &players(t, round),
        &paired_in(t, round),
        &boards,
    )
    .expect("assess")
}
