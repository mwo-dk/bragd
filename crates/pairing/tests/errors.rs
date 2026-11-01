//! What the pairing refuses, and the small cases it must get right.

use pairing::dutch::pair;
use pairing::{Board, Colour, Error, Player, Round, Settings};

const S: Settings = Settings {
    rounds: 3,
    win: 2,
    loss: 0,
    initial_colour: Colour::White,
};

fn fresh(n: u32) -> Vec<Player> {
    (1..=n)
        .map(|tpn| Player {
            tpn,
            history: Vec::new(),
        })
        .collect()
}

#[test]
fn round_one_pairs_top_half_against_bottom_half() {
    let p = pair(&S, &fresh(5), &[1, 2, 3, 4, 5]).unwrap();
    assert_eq!(
        p.boards,
        vec![Board { white: 1, black: 3 }, Board { white: 4, black: 2 }]
    );
    assert_eq!(p.bye, Some(5));
}

#[test]
fn absent_players_are_not_paired() {
    let p = pair(&S, &fresh(4), &[1, 3, 4]).unwrap();
    assert_eq!(p.boards, vec![Board { white: 1, black: 3 }]);
    assert_eq!(p.bye, Some(4));
}

#[test]
fn players_who_met_cannot_meet_again() {
    let played = |opponent, colour, points| Round::Played {
        opponent,
        colour,
        points,
    };
    let players = vec![
        Player {
            tpn: 1,
            history: vec![played(2, Colour::White, 2)],
        },
        Player {
            tpn: 2,
            history: vec![played(1, Colour::Black, 0)],
        },
    ];
    assert_eq!(pair(&S, &players, &[1, 2]), Err(Error::NoLegalPairing));
}

#[test]
fn a_second_bye_is_refused() {
    // Three players; 3 had the bye, 1 beat 2. Only 3 is free to meet anyone new,
    // and the bye must go to 1 or 2.
    let players = vec![
        Player {
            tpn: 1,
            history: vec![Round::Played {
                opponent: 2,
                colour: Colour::White,
                points: 2,
            }],
        },
        Player {
            tpn: 2,
            history: vec![Round::Played {
                opponent: 1,
                colour: Colour::Black,
                points: 0,
            }],
        },
        Player {
            tpn: 3,
            history: vec![Round::PairingBye { points: 2 }],
        },
    ];
    let p = pair(&S, &players, &[1, 2, 3]).unwrap();
    // C5: the lowest score that can take it is 2's.
    assert_eq!(p.bye, Some(2));
    assert_eq!(p.boards, vec![Board { white: 3, black: 1 }]);
}

#[test]
fn bad_input_is_named() {
    assert_eq!(pair(&S, &fresh(2), &[1, 7]), Err(Error::UnknownPlayer(7)));
    let mut uneven = fresh(2);
    uneven[1].history.push(Round::Absent { points: 0 });
    assert_eq!(pair(&S, &uneven, &[1, 2]), Err(Error::UnevenHistory(2)));
    let over: Vec<Player> = fresh(2)
        .into_iter()
        .map(|mut p| {
            p.history = vec![Round::Absent { points: 0 }; 3];
            p
        })
        .collect();
    assert_eq!(pair(&S, &over, &[1, 2]), Err(Error::TournamentOver));
}
