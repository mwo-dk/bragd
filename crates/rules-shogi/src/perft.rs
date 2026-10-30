//! Node counting, for holding move generation to published figures.
//!
//! The same gate chess stands on: either a count agrees with the
//! published table or the generator is wrong somewhere it would take a lost
//! game to notice. Shogi's table counts drops alongside moves, so this gates
//! exactly the half of the generator that chess's perft never could.

use crate::Position;

/// Nodes reachable in exactly `depth` plies. Depth zero is one node: here.
pub fn perft(position: &Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = position.legal_moves();
    if depth == 1 {
        return moves.len() as u64;
    }
    moves
        .iter()
        .map(|&mv| perft(&position.after(mv), depth - 1))
        .sum()
}
