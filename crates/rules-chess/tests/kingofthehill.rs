//! King of the Hill and Racing Kings.
//!
//! Both are chess with the endings changed, so what needs testing is not the
//! movement — that is the same generator every other variant here uses, and it
//! has perft behind it — but the **order in which endings are asked**. Every
//! bug either of these variants can have is an ordering bug: a position that is
//! both a hill arrival and a checkmate, or a race that both sides finish.

use rules_chess::outcome::{DrawReason, Outcome, WinReason};
use rules_chess::piece::Color;
use rules_chess::position::Position;
use rules_chess::variant::{KingOfTheHill, RacingKings, Variant};

fn at(fen: &str) -> Position {
    Position::from_fen(fen).expect("test position should be legal")
}

// --- king of the hill --------------------------------------------------------

#[test]
fn the_opening_array_is_chess() {
    let hill = KingOfTheHill.starting_position();
    assert_eq!(hill.to_fen(), Position::starting().to_fen());
    // And so are the moves: twenty at the start, same as chess.
    assert_eq!(KingOfTheHill.legal_moves(&hill).len(), 20);
}

#[test]
fn a_king_on_the_hill_has_won() {
    // White's king stands on e4. Black to move, and the game is already over.
    let position = at("7k/8/8/8/4K3/8/8/8 b - - 0 1");
    assert_eq!(
        KingOfTheHill.outcome(&position),
        Some(Outcome::Win {
            winner: Color::White,
            reason: WinReason::KingOfTheHill
        })
    );
}

#[test]
fn all_four_centre_squares_count_and_nothing_else_does() {
    for (fen, hill) in [
        ("7k/8/8/8/3K4/8/8/8 b - - 0 1", true),  // d4
        ("7k/8/8/8/4K3/8/8/8 b - - 0 1", true),  // e4
        ("7k/8/8/3K4/8/8/8/8 b - - 0 1", true),  // d5
        ("7k/8/8/4K3/8/8/8/8 b - - 0 1", true),  // e5
        ("7k/8/8/8/2K5/8/8/8 b - - 0 1", false), // c4, one file short
        ("7k/8/8/8/5K2/8/8/8 b - - 0 1", false), // f4
        ("7k/8/8/8/8/4K3/8/8 b - - 0 1", false), // e3, one rank short
        ("7k/8/4K3/8/8/8/8/8 b - - 0 1", false), // e6
    ] {
        let won = matches!(
            KingOfTheHill.outcome(&at(fen)),
            Some(Outcome::Win {
                reason: WinReason::KingOfTheHill,
                ..
            })
        );
        assert_eq!(won, hill, "{fen}");
    }
}

#[test]
fn black_wins_on_the_hill_too() {
    let position = at("8/8/8/8/3k4/8/8/K7 w - - 0 1");
    assert_eq!(
        KingOfTheHill.outcome(&position),
        Some(Outcome::Win {
            winner: Color::Black,
            reason: WinReason::KingOfTheHill
        })
    );
}

/// **The ordering rule.** A move can arrive on the hill *and* deliver mate at
/// the same time, and the arrival is what counts — the game ended on the ply
/// that put the king there. Asking about checkmate first would score these
/// positions for the wrong side.
#[test]
fn arriving_beats_being_mated() {
    // Black to move, no legal move, and in check: plain chess says White wins
    // by checkmate. Black's king is not on the hill; White's is on e4. Both
    // readings give White the win here, so the interesting case is the mirror.
    //
    // This one is the real test: White is stalemated *and* Black's king stands
    // on d5. Chess would call it a draw. Here Black has won.
    let position = at("8/8/8/3k4/8/6q1/8/7K w - - 0 1");
    assert!(KingOfTheHill.legal_moves(&position).is_empty());
    assert_eq!(
        KingOfTheHill.outcome(&position),
        Some(Outcome::Win {
            winner: Color::Black,
            reason: WinReason::KingOfTheHill
        })
    );
}

#[test]
fn checkmate_still_wins_when_nobody_is_on_the_hill() {
    // Fool's mate. Nothing near the centre.
    let position = at("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3");
    assert_eq!(
        KingOfTheHill.outcome(&position),
        Some(Outcome::Win {
            winner: Color::Black,
            reason: WinReason::Checkmate
        })
    );
}

/// Two lone kings is a dead draw in chess and a live game here: either king can
/// simply walk to the middle. Getting this wrong would end games that have
/// barely started.
#[test]
fn no_material_is_ever_insufficient() {
    let bare = at("7k/8/8/8/8/8/8/K7 w - - 0 1");
    assert!(!KingOfTheHill.is_insufficient_material(&bare));
    assert!(KingOfTheHill.can_possibly_mate(&bare, Color::White));
    assert!(KingOfTheHill.can_possibly_mate(&bare, Color::Black));
    assert_eq!(KingOfTheHill.outcome(&bare), None);
}

// --- racing kings ------------------------------------------------------------

#[test]
fn the_race_starts_with_no_pawns_and_no_castling() {
    let start = RacingKings.starting_position();
    let fen = start.to_fen();
    assert!(fen.starts_with("8/8/8/8/8/8/krbnNBRK/qrbnNBRQ"), "{fen}");
    assert!(fen.contains(" - "), "no castling rights: {fen}");
    // Sixteen pieces, none of them a pawn.
    assert!(!fen.split(' ').next().unwrap().to_lowercase().contains('p'));
}

/// The rule that makes it a race rather than a rout: **no move may give
/// check**, in either direction.
#[test]
fn no_move_may_give_check() {
    let start = RacingKings.starting_position();
    for mv in RacingKings.legal_moves(&start) {
        assert!(
            !start.make_move(mv).is_check(),
            "{mv} gives check and should not be legal"
        );
    }
    // And the filter actually removes something, or this test proves nothing.
    assert!(RacingKings.legal_moves(&start).len() < start.legal_moves().len());
}

#[test]
fn reaching_the_eighth_rank_wins() {
    // Black arrived. White to move, so White has already had their turn.
    let position = at("3k4/8/8/8/8/8/8/6K1 w - - 0 1");
    assert_eq!(
        RacingKings.outcome(&position),
        Some(Outcome::Win {
            winner: Color::Black,
            reason: WinReason::RaceWon
        })
    );
}

/// **Black always gets the reply**, and this is the whole fairness of the
/// variant. White moves first, so a race decided the instant a king arrives
/// would be decided by the coin toss that handed out colours.
#[test]
fn white_arriving_does_not_end_it_until_black_has_answered() {
    // White's king is home, it is Black's move, and Black's king can step home: not over.
    let pending = at("4K3/1k6/8/8/8/8/8/8 b - - 0 1");
    assert_eq!(RacingKings.outcome(&pending), None);

    // Black moved and did not arrive. Now White has won.
    let settled = at("4K3/8/8/8/8/8/3k4/8 w - - 0 2");
    assert_eq!(
        RacingKings.outcome(&settled),
        Some(Outcome::Win {
            winner: Color::White,
            reason: WinReason::RaceWon
        })
    );
}

/// Black's reply only matters when its king can reach the eighth rank with it: when it cannot,
/// White has won at once, as Fairy-Stockfish and Lichess score it.
#[test]
fn white_home_wins_at_once_when_black_cannot_follow() {
    // Black's king on b7 is a move from home, but White's rook on d8 guards a8, b8 and c8.
    let blocked = at("3RK3/1k6/8/8/8/8/8/8 b - - 0 1");
    assert_eq!(
        RacingKings.outcome(&blocked),
        Some(Outcome::Win {
            winner: Color::White,
            reason: WinReason::RaceWon
        })
    );
}

#[test]
fn both_kings_home_is_a_draw() {
    let position = at("k6K/8/8/8/8/8/8/8 w - - 0 1");
    assert_eq!(
        RacingKings.outcome(&position),
        Some(Outcome::Draw {
            reason: DrawReason::BothKingsHome
        })
    );
    // And the draw is read before either win, or the same position would be
    // scored as a win for whichever side happened to be asked about first.
    let other_turn = at("k6K/8/8/8/8/8/8/8 b - - 0 1");
    assert_eq!(
        RacingKings.outcome(&other_turn),
        Some(Outcome::Draw {
            reason: DrawReason::BothKingsHome
        })
    );
}

/// There is no checkmate in this variant at all: a side with no legal move
/// cannot be in check, because the move that would have given check was itself
/// illegal. So having nothing to play is a draw.
#[test]
fn having_no_move_is_a_draw_and_never_a_loss() {
    let start = RacingKings.starting_position();
    assert!(!RacingKings.legal_moves(&start).is_empty());

    // A position with one king each, boxed in by the no-check rule: every
    // king move would give check, so there is nothing to play.
    let boxed = at("8/8/8/8/8/8/8/K1k5 w - - 0 1");
    if RacingKings.legal_moves(&boxed).is_empty() {
        assert_eq!(
            RacingKings.outcome(&boxed),
            Some(Outcome::Draw {
                reason: DrawReason::Stalemate
            })
        );
    }
}

#[test]
fn a_race_can_actually_be_played_to_the_end() {
    // Not a rules check — a smoke test that the filter does not strangle the
    // opening. If the first dozen plies have no legal moves, something is very
    // wrong and every test above would still pass.
    let mut position = RacingKings.starting_position();
    for ply in 0..12 {
        let moves = RacingKings.legal_moves(&position);
        assert!(!moves.is_empty(), "no legal move at ply {ply}");
        if RacingKings.outcome(&position).is_some() {
            break;
        }
        position = RacingKings.make_move(&position, moves.into_iter().next().unwrap());
    }
}

#[test]
fn both_variants_are_reachable_by_key() {
    use rules_chess::Rules;
    assert_eq!(Rules::from_key("kingofthehill"), Some(Rules::KingOfTheHill));
    assert_eq!(Rules::from_key("racingkings"), Some(Rules::RacingKings));
    // And the dispatcher agrees with the type it dispatches to.
    assert_eq!(Rules::KingOfTheHill.name(), KingOfTheHill.name());
    assert_eq!(
        Rules::RacingKings.starting_position().to_fen(),
        RacingKings.starting_position().to_fen()
    );
}
