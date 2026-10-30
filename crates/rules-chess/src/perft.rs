//! Perft — counting the leaves of the legal move tree.
//!
//! This is the correctness gate for move generation. It is in the
//! library rather than the test file so that `divide` is available when a count
//! disagrees: the reference counts tell you *that* the generator is wrong,
//! `divide` tells you *where*.
//!
//! The debugging loop, which is worth writing down because it is not obvious
//! the first time:
//!
//! 1. `divide` the failing position at the failing depth.
//! 2. Compare against another engine's divide for the same position.
//! 3. Find the one move whose subtree count differs, play it, and repeat at
//!    depth minus one.
//! 4. At depth 1 the disagreement is a move that should or should not exist.

use crate::chess_move::{CastlingNotation, Move};
use crate::position::Position;

/// Counts the leaf nodes reachable in exactly `depth` plies.
///
/// Note this counts *leaves*, not positions: a position reachable two ways is
/// counted twice, which is what the published reference values do.
pub fn perft(position: &Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = position.legal_moves();
    // At depth 1 the moves themselves are the leaves; making them would be work
    // thrown away, and this is the base case the whole tree bottoms out in.
    if depth == 1 {
        return moves.len() as u64;
    }
    moves
        .into_iter()
        .map(|mv| perft(&position.make_move(mv), depth - 1))
        .sum()
}

/// Perft, broken down by first move — the tool for finding *which* move's
/// subtree is wrong. Sorted by move for stable output between runs.
pub fn divide(position: &Position, depth: u32) -> Vec<(Move, u64)> {
    assert!(depth >= 1, "divide needs at least one ply to divide");
    let mut counts: Vec<(Move, u64)> = position
        .legal_moves()
        .into_iter()
        .map(|mv| (mv, perft(&position.make_move(mv), depth - 1)))
        .collect();
    counts.sort_by_key(|(mv, _)| mv.to_uci(CastlingNotation::KingTakesRook));
    counts
}

/// `divide` rendered the way every engine prints it, so output can be diffed
/// against another implementation directly.
pub fn divide_report(position: &Position, depth: u32) -> String {
    let counts = divide(position, depth);
    let total: u64 = counts.iter().map(|(_, count)| count).sum();
    let mut report = String::new();
    for (mv, count) in &counts {
        report.push_str(&format!(
            "{}: {count}\n",
            mv.to_uci(CastlingNotation::KingTakesRook)
        ));
    }
    report.push_str(&format!("\nNodes searched: {total}\n"));
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_zero_is_the_position_itself() {
        assert_eq!(perft(&Position::starting(), 0), 1);
    }

    #[test]
    fn the_first_two_plies_are_the_famous_numbers() {
        assert_eq!(perft(&Position::starting(), 1), 20);
        assert_eq!(perft(&Position::starting(), 2), 400);
    }

    #[test]
    fn divide_sums_to_perft() {
        let position = Position::starting();
        for depth in 1..=3 {
            let total: u64 = divide(&position, depth).iter().map(|(_, n)| n).sum();
            assert_eq!(total, perft(&position, depth), "at depth {depth}");
        }
    }

    #[test]
    fn checkmate_has_no_leaves_below_it() {
        // Fool's mate. There is nothing to count once the game is over, which
        // is what makes perft blind to termination — see `Outcome`.
        let position =
            Position::from_fen("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3")
                .unwrap();
        assert_eq!(perft(&position, 1), 0);
        assert_eq!(perft(&position, 2), 0);
    }

    #[test]
    fn divide_reports_in_the_conventional_format() {
        let report = divide_report(&Position::starting(), 1);
        assert!(report.contains("a2a3: 1\n"));
        assert!(report.ends_with("Nodes searched: 20\n"));
    }
}
