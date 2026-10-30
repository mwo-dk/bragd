//! Perft — the correctness gate for move generation.
//!
//! Perft counts leaf nodes of the legal move tree to a fixed depth. The counts
//! below are the published reference values; a generator that matches them is
//! essentially correct, because these six positions were chosen precisely to
//! exercise what naive generators get wrong: en passant that would expose one's
//! own king, castling rights lost when a rook is captured, castling through an
//! attacked square, promotion with check, and pins along a diagonal.
//!
//! Shallow depths run on every `cargo test`. The deep ones are `#[ignore]`d and
//! run in CI under `cargo test --release -- --ignored`, because they are worth
//! minutes but not seconds.
//!
//! A count that is *wrong* is far more useful than a count that is missing:
//! when one of these fails, use `rules_chess::perft::divide` to find the move whose
//! subtree disagrees, then recurse into it. The bug is almost always one ply
//! below where it first shows up.

use rules_chess::{Position, perft::perft};

/// Reference positions. Names are the ones used in the literature, so the
/// numbers can be checked against any published table.
struct Case {
    name: &'static str,
    fen: &'static str,
    /// Expected node counts, index 0 being depth 1.
    counts: &'static [u64],
    /// Depths above this index run only under `--ignored`.
    quick_depth: usize,
}

const CASES: &[Case] = &[
    Case {
        name: "startpos",
        fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        counts: &[20, 400, 8_902, 197_281, 4_865_609, 119_060_324],
        quick_depth: 4,
    },
    Case {
        // Kiwipete. Dense with castling, pins and en passant at once.
        name: "kiwipete",
        fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        counts: &[48, 2_039, 97_862, 4_085_603, 193_690_690],
        quick_depth: 3,
    },
    Case {
        // Sparse endgame. The classic en-passant-discovers-check position.
        name: "position 3",
        fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        counts: &[14, 191, 2_812, 43_238, 674_624, 11_030_083],
        quick_depth: 4,
    },
    Case {
        // Promotions, including promotion with check and under-promotion.
        name: "position 4",
        fen: "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        counts: &[6, 264, 9_467, 422_333, 15_833_292],
        quick_depth: 3,
    },
    Case {
        // Castling rights lost by rook capture.
        name: "position 5",
        fen: "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        counts: &[44, 1_486, 62_379, 2_103_487, 89_941_194],
        quick_depth: 3,
    },
    Case {
        name: "position 6",
        fen: "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        counts: &[46, 2_079, 89_890, 3_894_594, 164_075_551],
        quick_depth: 3,
    },
];

fn run(case: &Case, from_depth: usize, to_depth: usize) {
    let position = Position::from_fen(case.fen)
        .unwrap_or_else(|e| panic!("{}: FEN did not parse: {e}", case.name));

    for depth in from_depth..=to_depth.min(case.counts.len()) {
        let expected = case.counts[depth - 1];
        let actual = perft(&position, depth as u32);
        assert_eq!(
            actual, expected,
            "{} at depth {depth}: expected {expected} nodes, got {actual}. \
             Run `rules_chess::perft::divide` on this position to find the offending subtree.",
            case.name
        );
    }
}

#[test]
fn perft_shallow() {
    for case in CASES {
        run(case, 1, case.quick_depth);
    }
}

#[test]
#[ignore = "minutes, not seconds — CI runs this under --release"]
fn perft_deep() {
    for case in CASES {
        run(case, case.quick_depth + 1, case.counts.len());
    }
}

/// Perft counts leaves, so a generator that emits an illegal move *and* omits a
/// legal one can still total correctly. Cross-check the shape of the tree, not
/// just its size: every generated move must survive a round trip through the
/// position it produces.
#[test]
fn every_generated_move_leaves_the_mover_out_of_check() {
    for case in CASES {
        let position = Position::from_fen(case.fen).unwrap();
        for mv in position.legal_moves() {
            let after = position.make_move(mv);
            assert!(
                !after.is_check_against(position.side_to_move()),
                "{}: {mv} left {:?} in check",
                case.name,
                position.side_to_move()
            );
        }
    }
}

/// FEN is how positions cross every boundary in this system — the seam,
/// a Library, a file. A position that does not survive the round trip is a position we
/// cannot store.
#[test]
fn fen_round_trips() {
    for case in CASES {
        let position = Position::from_fen(case.fen).unwrap();
        assert_eq!(position.to_fen(), case.fen, "{}", case.name);
    }
}
