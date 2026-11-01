//! The named motifs of a chess move (`rules_chess::motifs`).

use rules_chess::Position;
use rules_chess::motifs::{Motif, motifs};

fn of(fen: &str, uci: &str) -> Vec<Motif> {
    motifs(&Position::from_fen(fen).unwrap(), uci)
}

#[test]
fn a_knight_forks_king_and_rook() {
    assert_eq!(of("r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1", "b5c7"), [Motif::Fork]);
    assert_eq!(of("r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1", "b5d4"), []);
}

#[test]
fn a_fork_of_two_defended_pieces_worth_less_is_none() {
    // The queen on d5 attacks the pawns c6 and f7, both defended, by a pawn and the king; c6 takes it.
    assert_eq!(of("4k3/1p3p2/2p5/8/8/8/8/3QK3 w - - 0 1", "d1d5"), [Motif::Hangs]);
}

#[test]
fn a_bishop_pins_a_knight_to_its_king() {
    assert_eq!(of("4k3/8/2n5/8/8/8/8/4KB2 w - - 0 1", "f1b5"), [Motif::Pin]);
    // A knight pins nothing.
    assert_eq!(of("4k3/8/2n5/8/8/8/8/4KN2 w - - 0 1", "f1d2"), []);
}

#[test]
fn a_rook_pins_a_knight_to_a_queen() {
    assert_eq!(of("3qk3/8/3n4/8/8/8/8/R3K3 w - - 0 1", "a1d1"), [Motif::Pin]);
    // A pawn in front of a knight, "pinned" by a bishop or a queen, is not named.
    assert_eq!(of("4k1n1/5p2/8/8/8/1B6/8/4K3 w - - 0 1", "b3a2"), []);
    assert_eq!(of("4k3/8/7q/8/8/8/3P4/2B1K3 b - - 0 1", "h6g5"), []);
    // A queen in front of a knight is not pinned to it.
    assert_eq!(of("3nk3/8/3q4/8/8/8/8/R3K3 w - - 0 1", "a1d1"), []);
}

#[test]
fn a_queen_moved_where_a_pawn_takes_it_hangs() {
    assert_eq!(of("4k3/8/3p4/8/8/Q7/8/4K3 w - - 0 1", "a3c5"), [Motif::Hangs]);
    assert_eq!(of("4k3/8/3p4/8/8/Q7/8/4K3 w - - 0 1", "a3a4"), []);
    // Defended, but attacked by a pawn: still hangs.
    assert_eq!(of("4k3/8/4p3/8/8/8/3R4/3QK3 w - - 0 1", "d2d5"), [Motif::Hangs]);
}

#[test]
fn a_piece_that_hung_before_the_move_is_not_news() {
    assert_eq!(of("4k3/8/3p4/2N5/8/8/P7/4K3 w - - 0 1", "a2a3"), []);
    // Moving away a defender leaves the knight hanging.
    assert_eq!(of("4k3/8/8/2b5/8/4N3/8/K3R3 w - - 0 1", "e1f1"), [Motif::Hangs]);
}

#[test]
fn black_moves_too_and_an_illegal_move_has_none() {
    assert_eq!(of("4k3/1n6/8/8/8/8/8/R3K2R b - - 0 1", "b7c5"), []);
    assert_eq!(of("4k3/8/8/8/8/2n5/8/R3K2R b - - 0 1", "c3e4"), []);
    assert_eq!(of("r3k2r/8/8/8/8/8/8/4K3 w - - 0 1", "e1e3"), []);
    assert_eq!(of("4k3/8/8/8/1n6/8/8/R3K3 b - - 0 1", "b4c2"), [Motif::Fork]);
}
