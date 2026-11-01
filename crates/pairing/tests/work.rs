//! How much work a large round takes, counted in matching steps rather than
//! timed. The 353-player Granada Open once took up to 665 million steps (four
//! seconds) a round; every round now stays under 40 million.

mod common;

const GRANADA: &str = include_str!("fixtures/granada-open-2026.trf");

#[test]
fn a_large_open_pairs_every_round_within_budget() {
    let t = common::load(GRANADA);
    for round in 1..=t.rounds.unwrap() as usize {
        let before = pairing::work();
        common::pair(&t, round);
        let steps = pairing::work() - before;
        assert!(steps < 40_000_000, "round {round}: {steps} steps");
    }
}
