//! Chaturanga's correctness gate, and what it is honestly worth.
//!
//! The same shape as `tests/shatranj.rs`, and for the same reason: there is no
//! published perft corpus, so this cannot be what `tests/perft.rs` is. It is
//! weaker here than there in one specific way, and it is worth naming rather
//! than leaving to be discovered.
//!
//! For shatranj the argument was that no piece moves by a rule the chess perft
//! gate has not already exercised. **That is not quite true here.** The gaja is
//! a genuinely new table, and it is the first piece in this crate besides the
//! pawn whose moves depend on which side owns it. Colour-dependence is the
//! thing most likely to be got wrong in one direction only, and a
//! self-recorded count cannot see a mistake it recorded.
//!
//! So the weight is deliberately shifted off the counts and onto the
//! properties: the tree walk below checks the gaja's asymmetry from both sides
//! in every position it reaches, which is the check a corpus would otherwise
//! have done.

use rules_chess::piece::{Color, Role};
use rules_chess::variant::{Chaturanga, Variant};
use rules_chess::{Position, chaturanga};

fn start() -> Position {
    Chaturanga.starting_position()
}

fn perft(position: &Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = chaturanga::legal_moves(position);
    if depth == 1 {
        return moves.len() as u64;
    }
    moves
        .into_iter()
        .map(|mv| perft(&position.make_move(mv), depth - 1))
        .sum()
}

/// The two counts that can be argued rather than measured.
#[test]
fn the_first_two_plies_are_derivable_by_hand() {
    // Twelve: eight pawns with one step each, and four knight moves. Nothing
    // else can move at all. The gaja steps one square forward or one diagonal,
    // and from c1 that is b2, c2 and d2 — all three occupied by its own pawns.
    // The mantri and the king are hemmed in for the same reason.
    //
    // This is the number that shows the two elephants apart. Shatranj's opening
    // has sixteen moves, because an alfil on c1 *leaps* to a3 and e3 over the
    // pawn wall. A gaja cannot leap, so it starts the game entombed.
    assert_eq!(perft(&start(), 1), 12);

    // 144 = 12 x 12. Black's twelve replies mirror White's and nothing White
    // plays on move one can touch them: every square White reaches is on ranks
    // 1 to 3, and every square Black's pieces move to is on ranks 6 to 8.
    assert_eq!(perft(&start(), 2), 144);
}

/// Deeper counts, recorded from this engine.
///
/// **These prove stability and nothing else** — there is no second
/// implementation and no published table to agree with. Their job is to make a
/// change in move generation impossible to make by accident.
#[test]
fn deeper_counts_do_not_move_without_someone_meaning_it() {
    assert_eq!(perft(&start(), 3), 1_956);
    assert_eq!(perft(&start(), 4), 26_570);
}

#[test]
#[ignore = "seconds, not milliseconds — run with --ignored"]
fn deeper_still() {
    assert_eq!(perft(&start(), 5), 402_297);
}

/// Walks the tree checking what must hold of *every* position rather than of a
/// count. This is the part that could catch a generator wrong the same way
/// every time, which is precisely what the recorded counts above cannot.
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

        // Chaturanga can produce none of these. The opening array has no queen,
        // bishop or alfil, and the only promotion is to a mantri — so one
        // appearing means a chess or shatranj code path was reached by a
        // chaturanga position.
        assert!(
            position.by_role(Role::Queen).is_empty()
                && position.by_role(Role::Bishop).is_empty()
                && position.by_role(Role::Alfil).is_empty(),
            "a piece from another variant appeared in {}",
            position.to_fen()
        );

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

        assert!(
            position.en_passant_square().is_none(),
            "an en passant square in a game with no double step: {}",
            position.to_fen()
        );

        if depth == 0 {
            return;
        }
        for mv in chaturanga::legal_moves(position) {
            let next = position.make_move(mv);
            assert!(
                !next.is_check_against(position.side_to_move()),
                "{mv} leaves the mover in check in {}",
                position.to_fen()
            );
            walk(&next, depth - 1);
        }
    }

    walk(&start(), 3);
}

/// The gaja's asymmetry, checked in every position the tree reaches rather than
/// in one hand-built diagram.
///
/// This is the test carrying the weight the missing corpus would have carried.
/// A gaja never moves backwards, and "backwards" means opposite directions for
/// the two colours — so a table wired up for White and copied for Black passes
/// every count above and fails here on the first black elephant that moves.
#[test]
fn no_gaja_ever_moves_backwards() {
    fn walk(position: &Position, depth: u32) {
        let us = position.side_to_move();
        let gajas = position.pieces(us, Role::Gaja);
        for mv in chaturanga::legal_moves(position) {
            if gajas.contains(mv.from()) {
                let advance = mv.to().rank() as i8 - mv.from().rank() as i8;
                assert_eq!(
                    advance,
                    if us == Color::White { 1 } else { -1 },
                    "a gaja moved the wrong way with {mv} in {}",
                    position.to_fen()
                );
            }
            if depth > 0 {
                walk(&position.make_move(mv), depth - 1);
            }
        }
    }

    walk(&start(), 3);
}

/// A played-out game ends, and — as in shatranj — it is [`rules_chess::Game`] that
/// guarantees it rather than the position.
///
/// Chaturanga has no draw by insufficient material and no stalemate draw, so
/// two players with pieces left can shuffle for ever and every position in that
/// sequence is legitimately undecided. The history-aware backstop on `Game` is
/// what stops it, and this variant inherits that without asking for it.
#[test]
fn a_played_out_game_reaches_an_ending() {
    use rules_chess::{Game, Rules};

    let mut game = Game::start(Rules::Chaturanga);
    let mut plies = 0;

    while game.outcome().is_none() {
        assert!(plies < 1_000, "no ending after {plies} plies");
        let moves = game.legal_moves();
        assert!(!moves.is_empty(), "no moves but no outcome either");
        let chosen = moves
            .into_iter()
            .find(|mv| mv.is_capture())
            .unwrap_or_else(|| game.legal_moves().into_iter().next().expect("a move"));
        game.play(chosen).expect("a generated move is legal");
        plies += 1;
    }

    assert!(game.outcome().is_some(), "{plies} plies and no ending");
}
