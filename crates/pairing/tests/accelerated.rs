//! A Swiss paired with Baku acceleration (C.04.7 1, applied from 1 February
//! 2026), replayed round by round. The organiser's conditions say the Masters
//! used "Baku Acceleration of FIDE Swiss Pairings (Dutch)"; source URLs are at
//! the top of the fixture. Rounds that differ are judged as in `published.rs`.

mod common;

use pairing::dutch::Verdict;

const SUSSEX: &str = include_str!("fixtures/sussex-masters-2026.trf");

/// Rounds that differ from the published pairing, and the verdict on them.
fn known(round: usize) -> Option<Verdict> {
    match round {
        // Two players who had byes in round 1 and no game yet: C.04.3 5.2.5
        // gives the higher ranked White by its TPN's parity. The file keeps only
        // final start numbers, which may not be the TPNs this round used.
        2 => Some(Verdict::Colours(vec![pairing::Board {
            white: 54,
            black: 38,
        }])),
        9 => Some(Verdict::Worse {
            bracket: 8,
            criterion: pairing::Criterion::ColourPreference,
        }),
        _ => None,
    }
}

#[test]
fn sussex_masters() {
    let t = common::load(SUSSEX);
    let mut wrong = Vec::new();
    for round in 1..=t.rounds.unwrap() as usize {
        let (same, why) = common::compare_with(pairing::dutch::pair, &t, round, true);
        let verdict = (!same).then(|| common::judge_with(&t, round, true));
        if verdict != known(round) {
            wrong.push(format!("round {round}: got {verdict:?}\n{why}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn without_acceleration_the_early_rounds_differ() {
    let t = common::load(SUSSEX);
    // Round 1 unaccelerated would pair 1 against about 29; the event paired 1-15.
    for round in [1, 2, 3] {
        assert!(
            !common::compare_with(pairing::dutch::pair, &t, round, false).0,
            "round {round}"
        );
    }
}
