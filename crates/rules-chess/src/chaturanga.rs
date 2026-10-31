//! Chaturanga: move generation, and three endings that are not shatranj's.
//!
//! # Which chaturanga
//!
//! There is no living chaturanga and no surviving rulebook. What exists is a
//! set of *reconstructions* built from Sanskrit references, from the Arabic
//! game that descended from it, and from the South-East Asian games that did
//! survive. They disagree, and the disagreements are not decorative — they
//! change which piece moves where and who wins a drawn-looking position.
//!
//! That was once a reason not to build this game. The objection was never that the rules are unknowable, it was
//! that there is nothing to be *correct against*. Naming a reconstruction
//! supplies one. This module implements a single named reconstruction and the
//! rules document says so, out loud, with the places it is contested.
//!
//! **The reconstruction implemented here** is the one Murray sets out in *A
//! History of Chess* (1913) for the Indian game, taking the elephant's move in
//! the form that survived into makruk and sittuyin:
//!
//! - **Gaja (elephant)** — one square straight forward, or one square in any
//!   diagonal direction. *Contested.* The alternative is the Arabic alfil, two
//!   squares diagonally and leaping; that reading makes chaturanga a shatranj
//!   reskin, which is a thing to notice about the reading rather than an
//!   argument against it.
//! - **Mantri (counsellor)** — one square diagonally. Same move as shatranj's
//!   ferz, and the same [`Role::Ferz`] here.
//! - **Ratha, ashva, raja** — the rook, the knight and the king of chess.
//! - **Padati (foot soldier)** — one square forward, captures diagonally. No
//!   double step, therefore no en passant. Promotes to a mantri and nothing
//!   else.
//! - **No castling.**
//!
//! # Where it differs from shatranj, which is the part that matters
//!
//! If it differed only in the elephant it would be worth a footnote on the
//! shatranj rules rather than a game of its own. It differs in how it *ends*:
//!
//! - **Stalemate is a win for the stalemated player** — the opposite of
//!   shatranj, where stalemating wins. This is the rule that reached England
//!   and survived there into the nineteenth century, and it is the strongest
//!   single reason to have both games in Bragd: the same position, reached
//!   the same way, is scored in opposite directions.
//! - **Baring the king wins outright**, with no reply. Shatranj's "unless they
//!   bare yours next move" is an Arabic refinement; this game does not have it,
//!   so the ending is decidable from material alone.
//!
//! # What gates it
//!
//! No perft corpus, as with shatranj, and the same narrow argument applies:
//! every piece is either chess's exactly (pawn forward, knight, rook, king) or
//! a fixed offset table built by the same `const fn` that produces the knight's
//! and the king's — the tables that already pass a full published perft. The
//! gaja is the one genuinely new table and it is checked directly below, both
//! for its shape and for the asymmetry that makes it a role of its own.

use crate::attacks;
use crate::chess_move::{Move, MoveKind, MoveList};
use crate::movegen;
use crate::piece::Role;
use crate::position::Position;

/// The opening array: chess's, with the queen replaced by a mantri and the
/// bishops by elephants.
///
/// Identical in shape to shatranj's — the two games were the same game — and
/// the castling field is `-` for the same reason: there is no castling, so no
/// position this module produces can carry a right.
pub const STARTING_POSITION: &str = "rngfkgnr/pppppppp/8/8/8/8/PPPPPPPP/RNGFKGNR w - - 0 1";

/// Every legal move for the side to move.
pub fn legal_moves(position: &Position) -> MoveList {
    let mut moves = pseudo_legal_moves(position);
    let us = position.side_to_move();
    moves.retain(|mv| !position.make_move(mv).is_check_against(us));
    moves
}

fn pseudo_legal_moves(position: &Position) -> MoveList {
    let mut moves = MoveList::new();
    // The padati: chess's pawn without its double step, promoting to a mantri
    // and offering no choice about it. **`Role::Ferz` is the mantri** — one
    // square diagonally is one square diagonally, and the reconstruction says
    // so in as many words — which is what makes this the same call shatranj makes rather
    // than a second copy of it (see `movegen::stepping_pawn_moves`).
    movegen::stepping_pawn_moves(position, &mut moves, Role::Ferz);
    generate_piece_moves(position, &mut moves);
    moves
}

/// The five roles chaturanga has besides the padati.
///
/// The gaja is the only one that has to be told whose it is. Everything else
/// here is a colourless table or a slider, which is why this is the first
/// generator in the crate that reads `us` for anything but the piece set.
fn generate_piece_moves(position: &Position, moves: &mut MoveList) {
    let us = position.side_to_move();
    let occupied = position.occupied();
    let ours = position.by_color(us);
    let enemies = position.by_color(us.opponent());

    for role in [Role::Knight, Role::Rook, Role::King, Role::Ferz, Role::Gaja] {
        for from in position.pieces(us, role) {
            let targets = match role {
                Role::Knight => attacks::knight(from),
                Role::Rook => attacks::rook(from, occupied),
                Role::King => attacks::king(from),
                Role::Ferz => attacks::ferz(from),
                Role::Gaja => attacks::gaja(us, from),
                _ => unreachable!("chaturanga has no other piece to generate here"),
            } & !ours;

            for to in targets {
                let kind = if enemies.contains(to) {
                    MoveKind::Capture
                } else {
                    MoveKind::Quiet
                };
                moves.push(Move::new(from, to, kind));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piece::Color;
    use crate::square::Square;
    use crate::variant::{Chaturanga, Variant};

    fn position(fen: &str) -> Position {
        Position::from_fen(fen).expect("test FEN should parse")
    }

    fn uci_moves(fen: &str) -> Vec<String> {
        let mut moves: Vec<String> = legal_moves(&position(fen))
            .into_iter()
            .map(|mv| mv.to_uci(Chaturanga.castling_notation()))
            .collect();
        moves.sort();
        moves
    }

    #[test]
    fn the_opening_array_has_a_mantri_and_four_elephants() {
        let start = Chaturanga.starting_position();
        assert_eq!(start.to_fen(), STARTING_POSITION);
        assert_eq!(start.by_role(Role::Rook).count(), 4);
        assert_eq!(start.by_role(Role::Knight).count(), 4);
        assert_eq!(start.by_role(Role::Pawn).count(), 16);
        assert_eq!(start.by_role(Role::Ferz).count(), 2);
        assert_eq!(start.by_role(Role::Gaja).count(), 4);
        // Not shatranj's elephant, and not chess's two.
        assert_eq!(start.by_role(Role::Alfil).count(), 0);
        assert_eq!(start.by_role(Role::Queen).count(), 0);
        assert_eq!(start.by_role(Role::Bishop).count(), 0);
    }

    /// Twelve, where shatranj has sixteen — and the four missing ones are the
    /// whole difference between the two elephants.
    ///
    /// A gaja on c1 has five squares in an open field, and every one of them is
    /// behind or beside its own unmoved pawns. It cannot leap, so on move one it
    /// has nothing at all; the alfil in the same place reaches a3 and e3 over
    /// the pawn wall. Eight pawn steps and four knight moves is the entire
    /// opening.
    #[test]
    fn the_opening_position_has_twelve_moves() {
        let moves = uci_moves(STARTING_POSITION);
        assert_eq!(moves.len(), 12, "{moves:?}");
        assert!(!moves.contains(&"e2e4".to_string()), "no double step");
        assert!(moves.iter().all(|mv| !mv.starts_with("c1")));
        assert!(moves.iter().all(|mv| !mv.starts_with("f1")));
    }

    /// Five squares, not eight: forward and the four diagonals, with no
    /// sideways and no backward step.
    #[test]
    fn the_gaja_moves_forward_or_diagonally_and_no_other_way() {
        let moves = uci_moves("4k3/8/8/8/3G4/8/8/4K3 w - - 0 1");
        let gaja: Vec<&String> = moves.iter().filter(|m| m.starts_with("d4")).collect();
        assert_eq!(gaja.len(), 5, "{gaja:?}");
        for square in ["d5", "c5", "e5", "c3", "e3"] {
            assert!(moves.contains(&format!("d4{square}")), "{square}");
        }
        for square in ["d3", "c4", "e4"] {
            assert!(
                !moves.contains(&format!("d4{square}")),
                "{square} is not a gaja move"
            );
        }
    }

    /// The asymmetry, stated as a test because it is the reason this piece
    /// could not be a colourless table: Black's forward is the other way.
    #[test]
    fn black_gajas_face_the_other_way() {
        let moves = uci_moves("4k3/8/8/3g4/8/8/8/4K3 b - - 0 1");
        assert!(
            moves.contains(&"d5d4".to_string()),
            "Black's gaja steps down"
        );
        assert!(!moves.contains(&"d5d6".to_string()), "and never up");
    }

    /// Unlike the alfil, it is not colourbound and reaches the whole board —
    /// which is what makes it a piece worth having rather than a curiosity.
    #[test]
    fn a_gaja_can_reach_every_square() {
        let mut seen = crate::Bitboard::EMPTY;
        let mut frontier = vec![Square::new(27)]; // d4
        while let Some(from) = frontier.pop() {
            if seen.contains(from) {
                continue;
            }
            seen |= crate::Bitboard::from_square(from);
            for to in attacks::gaja(Color::White, from) {
                frontier.push(to);
            }
        }
        assert_eq!(seen.count(), 64);
    }

    /// A gaja gives check, and the check is found by `attackers_to` rather than
    /// by the generator — the one place where getting the colour inversion
    /// backwards would be a wrong *result* and not merely a wrong move list.
    #[test]
    fn a_gaja_checks_the_king_in_front_of_it() {
        // White gaja d4, Black king d5: straight ahead, so it is check.
        let checked = position("8/8/8/3k4/3G4/8/8/4K3 b - - 0 1");
        assert!(checked.is_check(), "a gaja attacks the square ahead of it");
        // And the king one square behind it is not attacked, because a gaja
        // has no backward step.
        let safe = position("8/8/8/8/3G4/3k4/8/4K3 b - - 0 1");
        assert!(!safe.is_check(), "and nothing behind it");
    }

    #[test]
    fn a_pawn_promotes_to_a_mantri_and_is_given_no_choice() {
        let moves = uci_moves("4k3/1P6/8/8/8/8/8/4K3 w - - 0 1");
        let promotions: Vec<&String> = moves.iter().filter(|m| m.starts_with("b7b8")).collect();
        assert_eq!(promotions, vec!["b7b8f"]);
    }

    #[test]
    fn there_is_no_castling() {
        let moves = uci_moves("4k3/8/8/8/8/8/8/4K2R w - - 0 1");
        assert!(!moves.contains(&"e1g1".to_string()));
        assert!(!moves.contains(&"e1h1".to_string()));
    }

    /// The rule that separates this game from shatranj, and the one most likely
    /// to surprise: the *stalemated* player wins.
    #[test]
    fn being_stalemated_wins_the_game() {
        // Black to move, not in check, and with no legal move.
        let stalemate = position("k7/8/K7/8/8/8/8/1R6 b - - 0 1");
        assert!(legal_moves(&stalemate).is_empty());
        assert!(!stalemate.is_check());
        assert_eq!(
            Chaturanga.outcome(&stalemate),
            Some(crate::Outcome::Win {
                winner: Color::Black,
                reason: crate::WinReason::Stalemate,
            }),
            "shatranj gives this to White and chess calls it a draw"
        );
    }

    #[test]
    fn checkmate_is_still_a_win_for_the_mating_side() {
        let mate = position("R5k1/1R6/8/8/8/8/8/4K3 b - - 0 1");
        assert_eq!(
            Chaturanga.outcome(&mate),
            Some(crate::Outcome::Win {
                winner: Color::White,
                reason: crate::WinReason::Checkmate,
            })
        );
    }

    /// No reply, no reprieve: the same position shatranj draws.
    #[test]
    fn baring_the_king_wins_even_when_it_can_be_answered() {
        let mutual = position("8/8/8/3kN3/8/8/8/6K1 b - - 0 1");
        assert!(mutual.is_bare(Color::Black));
        assert_eq!(
            Chaturanga.outcome(&mutual),
            Some(crate::Outcome::Win {
                winner: Color::White,
                reason: crate::WinReason::BareKing,
            }),
            "shatranj draws this because Kxe5 bares back"
        );
    }

    #[test]
    fn two_lone_kings_are_drawn() {
        let nothing = position("k7/8/8/8/8/8/8/7K w - - 0 1");
        assert_eq!(
            Chaturanga.outcome(&nothing),
            Some(crate::Outcome::Draw {
                reason: crate::DrawReason::BothKingsBared,
            })
        );
    }
}
