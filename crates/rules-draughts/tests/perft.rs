//! Perft — the correctness gate for draughts.
//!
//! Unlike tafl, this family **has** a published corpus, and choosing it second
//! was largely for that reason: build the game you can prove while the harness
//! is young.
//!
//! **International 10×10, duplicates removed.** Two independent
//! implementations — Ed Gilbert's and Aart Bik's — agree exactly on these
//! numbers from the opening position, and Rein Halbersma's `dctl` confirms the
//! shallow ones. That agreement is what makes a table trustworthy; a single
//! source is a mirror.
//!
//! The table is also a live trap for one specific bug. A published run once
//! gave 1,666,207,133 at depth eleven against the correct 1,665,861,398 — a gap
//! of 345,735 nodes — and its author diagnosed it as a generator that "does not
//! check for duplicate moves". That is exactly the bug this engine's move
//! representation is shaped to make impossible: a move is an origin, a
//! destination and a captured set, so two capture paths that agree on those
//! three *are* one move rather than being deduplicated afterwards.
//!
//! **English 8×8 keeps duplicates**, and its published table is counted that
//! way. So the two cannot be checked by the same harness setting, and the
//! harness must count what it is given rather than deciding — a `dedupe` flag
//! on a shared perft harness would be the harness learning a rule.

use rules_draughts::{ENGLISH, INTERNATIONAL, Position, Ruleset};

fn perft(rules: &Ruleset, position: &Position, depth: u32) -> u64 {
    // Counted in the variant's own published convention. International's tables
    // remove duplicate capture paths and English's keep them, so an engine that
    // could only produce one could only be checked against one.
    let moves = position.moves_as_counted(rules);
    if depth <= 1 {
        return moves.len() as u64;
    }
    moves
        .into_iter()
        .map(|mv| perft(rules, &position.play(rules, mv), depth - 1))
        .sum()
}

/// Gilbert and Bik, from the opening position, duplicates removed.
const INTERNATIONAL_COUNTS: &[u64] = &[
    9, 81, 658, 4_265, 27_117, 167_140, 1_049_442, 6_483_961, 41_022_423,
];

#[test]
fn international_shallow() {
    let start = Position::start(INTERNATIONAL.geometry);
    for (index, &expected) in INTERNATIONAL_COUNTS.iter().take(6).enumerate() {
        let depth = index as u32 + 1;
        assert_eq!(
            perft(&INTERNATIONAL, &start, depth),
            expected,
            "international perft({depth})"
        );
    }
}

#[test]
#[ignore = "minutes: run with --release -- --ignored"]
fn international_deep() {
    let start = Position::start(INTERNATIONAL.geometry);
    for (index, &expected) in INTERNATIONAL_COUNTS.iter().enumerate() {
        let depth = index as u32 + 1;
        assert_eq!(
            perft(&INTERNATIONAL, &start, depth),
            expected,
            "international perft({depth})"
        );
    }
}

/// Bik, from the opening position, **duplicates kept**.
///
/// A different convention from International's table, and the reason the
/// harness must count what it is given: English generators traditionally emit
/// both orders of a capture that can be played two ways, and the published
/// numbers include them. Applying International's deduplication here would
/// disagree with every table in the literature.
const ENGLISH_COUNTS: &[u64] = &[7, 49, 302, 1_469, 7_361, 36_768, 179_740, 845_931];

#[test]
fn english_shallow() {
    let start = Position::start(ENGLISH.geometry);
    for (index, &expected) in ENGLISH_COUNTS.iter().take(6).enumerate() {
        let depth = index as u32 + 1;
        assert_eq!(
            perft(&ENGLISH, &start, depth),
            expected,
            "english perft({depth})"
        );
    }
}

#[test]
#[ignore = "run with --release -- --ignored"]
fn english_deep() {
    let start = Position::start(ENGLISH.geometry);
    for (index, &expected) in ENGLISH_COUNTS.iter().enumerate() {
        let depth = index as u32 + 1;
        assert_eq!(
            perft(&ENGLISH, &start, depth),
            expected,
            "english perft({depth})"
        );
    }
}

#[test]
fn the_two_counting_conventions_really_do_differ() {
    // Otherwise `counts_duplicate_captures` is a field nobody can tell is
    // wrong. A duplicate needs a piece that can come back to where it started
    // by two routes, which needs a king — so this walks far enough into an
    // English game to find one and asserts the two counts diverge somewhere.
    fn walk(rules: &Ruleset, position: &Position, depth: u32, diverged: &mut bool) {
        if position.captures(rules).len() != position.captures_as_generated(rules).len() {
            *diverged = true;
            return;
        }
        if depth == 0 || *diverged {
            return;
        }
        for mv in position.legal_moves(rules) {
            walk(rules, &position.play(rules, mv), depth - 1, diverged);
        }
    }

    // A king with four men round it in a square: it can take all four
    // clockwise or anticlockwise, ending where it began with the same set.
    let mut diverged = false;
    walk(
        &INTERNATIONAL,
        &Position::start(INTERNATIONAL.geometry),
        9,
        &mut diverged,
    );
    assert!(
        diverged,
        "no position in nine plies has two paths to the same capture — \
         `counts_duplicate_captures` would be untestable"
    );
}

#[test]
fn the_opening_array_is_what_the_rules_say() {
    let start = Position::start(INTERNATIONAL.geometry);
    assert_eq!(start.side(rules_draughts::Side::White).count(), 20);
    assert_eq!(start.side(rules_draughts::Side::Black).count(), 20);
    assert!(start.kings().is_empty());
    assert_eq!(start.to_move(), rules_draughts::Side::White);
}
