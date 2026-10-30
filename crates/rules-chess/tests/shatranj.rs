//! Shatranj's correctness gate, and an honest account of what it is worth.
//!
//! **There is no published perft corpus for shatranj**, so this cannot be what
//! `tests/perft.rs` is. A missing corpus is the normal reason to refuse to
//! build a variant, and it is acceptable here specifically: every shatranj piece either moves exactly as it does in chess
//! or is a fixed offset table, and nothing generates a move by a rule the chess
//! gate has not already exercised at full depth.
//!
//! What is left to check is what that argument does *not* cover, which is
//! roughly three things:
//!
//! 1. **Counts that can be derived by hand.** Two of them can, and they are
//!    below with the derivation rather than as bare numbers.
//! 2. **Regression counts.** The deeper numbers are this engine's own output,
//!    recorded so a change that alters them has to be deliberate. They are
//!    evidence of *stability*, not of correctness, and saying otherwise would
//!    be pretending a corpus exists.
//! 3. **Properties that must hold in every position reached.** These are worth
//!    more than the regression counts, because they would catch a generator
//!    that is consistently wrong — which is exactly what a self-recorded count
//!    cannot.

use rules_chess::piece::{Color, Role};
use rules_chess::variant::{Shatranj, Variant};
use rules_chess::{Position, shatranj};

fn start() -> Position {
    Shatranj.starting_position()
}

fn perft(position: &Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = shatranj::legal_moves(position);
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
    // Sixteen: eight pawns with one step each (never two), four knight moves
    // — b1 to a3 and c3, g1 to f3 and h3, exactly as in chess — and four alfil
    // leaps, c1 to a3 and e3 and f1 to d3 and h3, over the unmoved pawn wall.
    // The ferz is hemmed in by its own pawns and the king has nowhere to go,
    // which is the same reason neither moves in chess's opening position.
    assert_eq!(perft(&start(), 1), 16);

    // 256 = 16 x 16. Black's sixteen replies are the mirror of White's and
    // cannot be reduced by anything White plays on move one: no first move can
    // capture, none can give check, and none can block a black pawn or a black
    // alfil, because every square White can reach is on ranks 1 to 3 and every
    // square Black's pieces move to is on ranks 6 to 8.
    assert_eq!(perft(&start(), 2), 256);
}

/// Deeper counts, recorded from this engine.
///
/// **These prove stability and nothing else.** There is no second
/// implementation and no published table to agree with, so a shared mistake
/// would be recorded here as confidently as a correct count. Their job is to
/// make a change in move generation impossible to make by accident.
#[test]
fn deeper_counts_do_not_move_without_someone_meaning_it() {
    assert_eq!(perft(&start(), 3), 4_176);
    assert_eq!(perft(&start(), 4), 68_122);
}

#[test]
#[ignore = "seconds, not milliseconds — run with --ignored"]
fn deeper_still() {
    assert_eq!(perft(&start(), 5), 1_164_248);
}

/// Walks the whole tree to a shallow depth, checking things that must be true
/// of *every* position, not of a count.
///
/// This is the part of the suite that could catch a generator which is wrong
/// the same way every time.
#[test]
fn every_position_in_the_tree_is_a_legal_board() {
    fn walk(position: &Position, depth: u32) {
        // Exactly one king a side, always. A generator that let a king be
        // captured would show up here first, and it is the failure that would
        // corrupt a Game rather than merely misplay one.
        for color in Color::ALL {
            assert_eq!(
                position.pieces(color, Role::King).count(),
                1,
                "{} has the wrong number of kings in {}",
                if color == Color::White {
                    "White"
                } else {
                    "Black"
                },
                position.to_fen()
            );
        }

        // Shatranj cannot produce either of these. The opening array has no
        // queen and no bishop, and the only promotion is to a ferz — so if one
        // appears, a chess code path has been reached by a shatranj position.
        assert!(
            position.by_role(Role::Queen).is_empty() && position.by_role(Role::Bishop).is_empty(),
            "a chess piece appeared in {}",
            position.to_fen()
        );

        // No pawn may stand on a promotion rank: reaching one always promotes.
        for color in Color::ALL {
            let pawns = position.pieces(color, Role::Pawn);
            for square in pawns {
                assert_ne!(
                    square.rank(),
                    color.promotion_rank(),
                    "an unpromoted pawn on the last rank in {}",
                    position.to_fen()
                );
            }
        }

        // Shatranj never sets one, because it never generates a double step.
        assert!(
            position.en_passant_square().is_none(),
            "an en passant square in a game with no double step: {}",
            position.to_fen()
        );

        if depth == 0 {
            return;
        }
        for mv in shatranj::legal_moves(position) {
            let next = position.make_move(mv);
            // The rule the legality filter exists to enforce. Stated here as
            // well because a filter that silently stopped filtering would leave
            // every count below unchanged in shape and wrong in value.
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

/// A promoting pawn becomes a ferz and is offered nothing else.
#[test]
fn promotion_is_never_a_choice() {
    let position = Position::from_fen("4k3/1P6/8/8/8/8/8/4K3 w - - 0 1").expect("a legal position");
    let promotions: Vec<_> = shatranj::legal_moves(&position)
        .into_iter()
        .filter(|mv| mv.promotion_role().is_some())
        .collect();

    assert_eq!(promotions.len(), 1, "{promotions:?}");
    assert_eq!(promotions[0].promotion_role(), Some(Role::Ferz));

    let after = position.make_move(promotions[0]);
    assert_eq!(after.by_role(Role::Ferz).count(), 1);
    assert_eq!(after.by_role(Role::Queen).count(), 0, "not a queen");
}

/// A played-out game of shatranj ends — and it is [`rules_chess::Game`] that
/// guarantees it, not the position.
///
/// Worth stating precisely, because the first version of this test asserted
/// the wrong thing and failed. `Shatranj::outcome` looks at a position alone,
/// and a position alone genuinely cannot end a shuffling game: shatranj has no
/// stalemate-by-repetition and no insufficient-material draw, so two players
/// with pieces left can push them about for ever and every position in that
/// sequence is legitimately undecided. Left there, that is two Players staring
/// at a board with no result and no way to get one.
///
/// What stops it is the history-aware layer — fivefold repetition and the
/// move-count rule on `Game` — which shatranj inherits without asking. This
/// plays greedily for captures so material drains, and then keeps shuffling to
/// prove the backstop fires.
#[test]
fn a_played_out_game_reaches_an_ending() {
    use rules_chess::{Game, Rules};

    let mut game = Game::start(Rules::Shatranj);
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
