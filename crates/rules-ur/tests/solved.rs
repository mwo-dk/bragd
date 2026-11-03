//! The whole tables, solved and checked against the chances the RoyalUr.net team published for
//! the side that throws first (J. Olieslagers, `game_of_ur`, 2025, from the published Finkel,
//! Blitz and Masters maps). Minutes of work and up to 2 GB of memory each, so they run only
//! when asked: `cargo test --release -p rules-ur --test solved -- --ignored --nocapture`.

use rules_ur::solve::Space;
use rules_ur::{BLITZ, FINKEL, MASTERS, Position, Rules, Side};

fn check(rules: &'static Rules, published: f64) {
    let space = Space::new(rules);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let (solved, stats) = space.solve(threads, &|| false, &|_, _| {}).unwrap();
    let start = space.before_throw(&solved, &Position::starting_with(rules, Side::White)).unwrap();
    println!("{}: {:.10} % first; {stats:?}", rules.name, start * 100.0);
    assert!((start * 100.0 - published).abs() < 1e-3, "{}: {start} against {published} %", rules.name);
}

#[test]
#[ignore = "minutes of work: run with --ignored in release"]
fn blitz_matches_the_published_chance() {
    check(&BLITZ, 50.4052555093);
}

#[test]
#[ignore = "minutes of work: run with --ignored in release"]
fn finkel_matches_the_published_chance() {
    check(&FINKEL, 51.5404732509);
}

#[test]
#[ignore = "minutes of work and 2 GB: run with --ignored in release"]
fn masters_matches_the_published_chance() {
    check(&MASTERS, 50.4809002027);
}

/// Aseb has no published value: Bragd's own, from its first solve, so a change shows.
#[test]
#[ignore = "minutes of work: run with --ignored in release"]
fn aseb_keeps_its_chance() {
    check(&rules_ur::ASEB, ASEB_FIRST);
}
const ASEB_FIRST: f64 = 50.6492078304;
