//! Capablanca's correctness gate, and what it is honestly worth.
//!
//! **There is no published perft corpus for Capablanca chess** that this
//! project has, so this cannot be what `crates/rules-chess/tests/perft.rs` is —
//! that suite agrees with numbers computed independently by other people, and
//! agreement is the whole value of it.
//!
//! The argument that carried shatranj and chaturanga does not carry here
//! either. There the claim was that every piece moves by a rule chess's gate
//! already exercises at full depth. This engine shares *no code at all* with
//! that one: its own board, its own attack tables, its own generator. Nothing
//! here has been checked by anything.
//!
//! So the counts below are stability evidence and nothing more, and the weight
//! is put where it can do some good:
//!
//! 1. Two counts derivable by hand, with the derivation.
//! 2. Recorded counts, labelled as recorded.
//! 3. **Properties over the whole tree**, which is the part that could catch a
//!    generator wrong the same way every time — and on a ten-file board the
//!    likeliest such error is arithmetic that wraps around an edge, so that is
//!    what is checked.

use rules_capablanca::piece::{Color, Role};
use rules_capablanca::{Move, Position};

fn start() -> Position {
    Position::starting()
}

fn perft(position: &Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = position.legal_moves();
    if depth == 1 {
        return moves.len() as u64;
    }
    (&moves)
        .into_iter()
        .map(|mv| perft(&position.make_move(mv), depth - 1))
        .sum()
}

/// The two counts that can be argued rather than measured.
#[test]
fn the_first_two_plies_are_derivable_by_hand() {
    // Twenty-eight: ten pawns with two steps each is twenty, and eight leaps.
    // The leaps are the interesting half — the knights on b1 and i1 have two
    // squares apiece, and so do the archbishop on c1 and the chancellor on h1,
    // because each of them *is* a knight as well as a slider and the knight
    // half hops the pawn wall on move one. Everything else is walled in.
    assert_eq!(perft(&start(), 1), 28);

    // 784 = 28 x 28. Black's replies mirror White's and nothing White can play
    // on move one touches them: every square White reaches is on ranks 1 to 3,
    // and every square Black's pieces move to is on ranks 6 to 8.
    assert_eq!(perft(&start(), 2), 784);
}

/// Recorded from this engine. **They prove stability and not correctness** —
/// there is no second implementation to agree with, so a mistake would be
/// recorded here as confidently as a right answer. Their job is to make a
/// change in move generation impossible to make by accident.
#[test]
fn deeper_counts_do_not_move_without_someone_meaning_it() {
    assert_eq!(perft(&start(), 3), 25_228);
}

#[test]
#[ignore = "seconds, not milliseconds — run with --ignored"]
fn deeper_still() {
    assert_eq!(perft(&start(), 4), 805_128);
}

/// What must be true of every position in the tree.
#[test]
fn every_position_in_the_tree_is_a_legal_board() {
    fn walk(position: &Position, depth: u32) {
        for color in Color::ALL {
            assert_eq!(
                position.pieces(color, Role::King).count(),
                1,
                "wrong number of kings in {}",
                position.to_fen()
            );
        }

        // No pawn may stand on a promotion rank: reaching one always promotes.
        for color in Color::ALL {
            for square in position.pieces(color, Role::Pawn) {
                assert_ne!(
                    square.rank(),
                    color.promotion_rank(),
                    "an unpromoted pawn on the last rank in {}",
                    position.to_fen()
                );
            }
        }

        // Every FEN this engine writes is one it reads back to the same
        // position. Cheap here, and it exercises two-digit empty runs against
        // thousands of real positions rather than the three in the unit test.
        let fen = position.to_fen();
        assert_eq!(
            Position::from_fen(&fen).map(|p| p.to_fen()),
            Ok(fen.clone()),
            "{fen} does not round trip"
        );

        if depth == 0 {
            return;
        }
        for mv in &position.legal_moves() {
            let next = position.make_move(mv);
            assert!(
                !next.is_check_against(position.side_to_move()),
                "{} leaves the mover in check in {}",
                mv.to_uci(),
                position.to_fen()
            );
            walk(&next, depth - 1);
        }
    }

    walk(&start(), 3);
}

/// **The failure this board invites**, and the reason it gets a test of its own
/// rather than a line in the walk above.
///
/// Every chess engine ever written says 8 for "one rank". This one has to say
/// 10 in every one of those places, and the symptom of missing one is not a
/// crash — it is a piece that steps off the j-file and arrives on the a-file of
/// the next rank, which looks like a legal move and is a teleport.
///
/// So: no move may change a piece's file by more than the geometry of the piece
/// allows, and nothing may join two squares that a bishop, rook, knight or king
/// could not join.
#[test]
fn nothing_wraps_around_an_edge() {
    fn plausible(mv: Move) -> bool {
        let file = mv.to.file() as i16 - mv.from.file() as i16;
        let rank = mv.to.rank() as i16 - mv.from.rank() as i16;
        let (file, rank) = (file.abs(), rank.abs());
        // A knight's leap, a straight line, a diagonal, or a castle — which is
        // the king moving three along its own rank.
        (file == 1 && rank == 2)
            || (file == 2 && rank == 1)
            || (file == 0 && rank > 0)
            || (rank == 0 && file > 0)
            || (file == rank && file > 0)
    }

    fn walk(position: &Position, depth: u32) {
        for mv in &position.legal_moves() {
            assert!(
                plausible(mv),
                "{} joins two squares no piece here can join, in {}",
                mv.to_uci(),
                position.to_fen()
            );
            if depth > 0 {
                walk(&position.make_move(mv), depth - 1);
            }
        }
    }

    walk(&start(), 3);
}
