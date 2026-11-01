//! Real tournaments, replayed round by round against the pairings published for them.
//!
//! Each fixture is a finished 2026 Swiss (after the 1 February 2026 rules took
//! effect) from chess-results.com, turned into TRF-16 with names removed; its
//! source URLs are at the top of the file. Every round is paired from the
//! history before it and compared with what was played. A round that differs is
//! judged with [`pairing::dutch::assess`]; the list below records each such
//! round and why, so that any change in behaviour shows up here.
//!
//! What the files cannot tell us, and the usual reasons for a difference:
//! - TPNs are the final ones. Rating corrections may renumber players until
//!   round 4 (C.04.2 2.3), so early rounds can be paired from other numbers.
//! - Arbiters may change a pairing by hand (C.04.2 4.4).
//! - The pairing system is not published; a Swiss-Manager event may use Dubov,
//!   Burstein or Lim rather than Dutch.

mod common;

use pairing::dutch::Criterion::*;
use pairing::dutch::Verdict::{self, *};

/// Rounds that differ from the published pairing, and the verdict on them.
fn known(file: &str, round: usize) -> Option<Verdict> {
    Some(match (file, round) {
        ("arnhem-group-a-2026.trf", 1) => GeneratedLater { bracket: 0 },
        ("arnhem-group-a-2026.trf", 3) => Worse {
            bracket: 1,
            criterion: ColourPreference,
        },
        ("arnhem-group-a-2026.trf", 4) => GeneratedLater { bracket: 4 },
        ("arnhem-group-b-2026.trf", 1) => GeneratedLater { bracket: 0 },
        ("arnhem-group-b-2026.trf", 2) => Worse {
            bracket: 1,
            criterion: Downfloaters,
        },
        ("arnhem-group-b-2026.trf", 3) => Colours(vec![pairing::Board {
            white: 66,
            black: 41,
        }]),
        ("arnhem-group-b-2026.trf", 4) => Worse {
            bracket: 3,
            criterion: ColourPreference,
        },
        ("bjorkstadsschacket-2026.trf", 1) => Colours(vec![pairing::Board {
            white: 19,
            black: 38,
        }]),
        ("granada-open-2026.trf", 1) => GeneratedLater { bracket: 0 },
        ("haustmot-tr-open-2026.trf", 2) => Colours(vec![pairing::Board {
            white: 19,
            black: 8,
        }]),
        ("kangasala-weekend-2026.trf", 1) => GeneratedLater { bracket: 0 },
        ("kangasala-weekend-2026.trf", 2) => GeneratedLater { bracket: 2 },
        ("rotterdam-congress-open-2026.trf", 1) => GeneratedLater { bracket: 0 },
        ("rotterdam-congress-open-2026.trf", 4) => Worse {
            bracket: 2,
            criterion: RepeatDownfloat,
        },
        ("rotterdam-congress-open-2026.trf", 5) => Worse {
            bracket: 1,
            criterion: StrongColourPreference,
        },
        ("vastgota-open-2026.trf", 4) => GeneratedLater { bracket: 2 },
        _ => return None,
    })
}

fn replay(file: &str) {
    let text = std::fs::read_to_string(format!(
        "{}/tests/fixtures/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let t = common::load(&text);
    let mut wrong = Vec::new();
    for round in 1..=t.rounds.unwrap() as usize {
        let (same, why) = common::compare(&t, round);
        let verdict = if same {
            None
        } else {
            Some(common::judge(&t, round))
        };
        if verdict != known(file, round) {
            wrong.push(format!(
                "round {round}: expected {:?}, got {verdict:?}\n{why}",
                known(file, round)
            ));
        }
    }
    assert!(wrong.is_empty(), "{file}:\n{}", wrong.join("\n"));
}

#[test]
fn arnhem_group_a() {
    replay("arnhem-group-a-2026.trf");
}

#[test]
fn arnhem_group_b() {
    replay("arnhem-group-b-2026.trf");
}

#[test]
fn bjorkstadsschacket() {
    replay("bjorkstadsschacket-2026.trf");
}

#[test]
fn granada_open() {
    replay("granada-open-2026.trf");
}

#[test]
fn haustmot_tr_open() {
    replay("haustmot-tr-open-2026.trf");
}

#[test]
fn hoogeveen_open() {
    replay("hoogeveen-open-2026.trf");
}

#[test]
fn kangasala_weekend() {
    replay("kangasala-weekend-2026.trf");
}

#[test]
fn moravia_silesia_open() {
    replay("moravia-silesia-open-2026.trf");
}

#[test]
fn rotterdam_congress_open() {
    replay("rotterdam-congress-open-2026.trf");
}

#[test]
fn scheldemond() {
    replay("scheldemond-2026.trf");
}

#[test]
fn tammer_weekend_a() {
    replay("tammer-weekend-a-2026.trf");
}

#[test]
fn u2000_tr() {
    replay("u2000-tr-2026.trf");
}

#[test]
fn vastgota_open() {
    replay("vastgota-open-2026.trf");
}
