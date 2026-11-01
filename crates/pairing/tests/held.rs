//! Mastering the Dutch 2026 (Mario Held, IA, FIDE TEC): a 15-player, nine-round
//! tournament paired by hand under C.04.3 2026, every round explained.
//! Source: https://tec.fide.com/wp-content/uploads/2026/07/Mastering_the_Dutch_2026.pdf

mod common;

const HELD: &str = include_str!("fixtures/held-2026.trf");

#[test]
fn every_round_matches_the_worked_example() {
    let t = common::load(HELD);
    let mut bad = Vec::new();
    for round in 1..=9 {
        let (ok, why) = common::compare(&t, round);
        if !ok {
            bad.push(why);
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
