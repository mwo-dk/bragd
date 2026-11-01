//! Judging a pairing changed by hand, on the worked example of Mastering the Dutch 2026.

mod common;

use pairing::Board;
use pairing::dutch::{Criterion, Verdict, assess};

const HELD: &str = include_str!("fixtures/held-2026.trf");

fn judge(round: usize, boards: &[Board]) -> Verdict {
    let t = common::load(HELD);
    assess(
        &common::settings(&t),
        &common::players(&t, round),
        &common::paired_in(&t, round),
        boards,
    )
    .unwrap()
}

fn published(round: usize) -> Vec<Board> {
    common::published(&common::load(HELD), round).0
}

#[test]
fn the_published_round_is_the_same() {
    assert_eq!(judge(5, &published(5)), Verdict::Same);
}

#[test]
fn swapped_colours_are_named() {
    let mut boards = published(2);
    let b = boards[0];
    boards[0] = Board {
        white: b.black,
        black: b.white,
    };
    assert_eq!(judge(2, &boards), Verdict::Colours(vec![boards[0]]));
}

#[test]
fn a_rematch_is_illegal() {
    // Round 3: 1 and 8 met in round 1. Swap partners so that they meet again.
    let mut boards = published(3);
    let i = boards
        .iter()
        .position(|b| b.white == 1 || b.black == 1)
        .unwrap();
    let j = boards
        .iter()
        .position(|b| b.white == 8 || b.black == 8)
        .unwrap();
    let (a, b) = (boards[i], boards[j]);
    let other_of = |x: Board, p: u32| if x.white == p { x.black } else { x.white };
    boards[i] = Board { white: 1, black: 8 };
    boards[j] = Board {
        white: other_of(a, 1),
        black: other_of(b, 8),
    };
    assert!(
        matches!(judge(3, &boards), Verdict::Illegal { .. }),
        "{:?}",
        judge(3, &boards)
    );
}

#[test]
fn round_one_without_the_natural_split_is_generated_later() {
    // Round 1: 1-9 and 2-8 instead of 1-8 and 9-2 keeps every criterion; the rules
    // still take the candidate generated first.
    let mut boards = published(1);
    for b in boards.iter_mut() {
        match (b.white, b.black) {
            (1, 8) => *b = Board { white: 1, black: 9 },
            (9, 2) => *b = Board { white: 8, black: 2 },
            _ => {}
        }
    }
    assert_eq!(judge(1, &boards), Verdict::GeneratedLater { bracket: 0 });
}

#[test]
fn an_avoidable_downfloater_is_worse() {
    // Round 2: swapping the opponents of 2-5 and 4-13 forces an extra downfloater
    // out of the one-point bracket.
    let boards: Vec<Board> = published(2)
        .into_iter()
        .map(|b| match (b.white, b.black) {
            (2, 5) => Board {
                white: 2,
                black: 13,
            },
            (4, 13) => Board { white: 4, black: 5 },
            _ => b,
        })
        .collect();
    assert_eq!(
        judge(2, &boards),
        Verdict::Worse {
            bracket: 2,
            criterion: Criterion::Downfloaters
        }
    );
}
