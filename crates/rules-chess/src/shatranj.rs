//! Shatranj: move generation and its three endings.
//!
//! **Not a seventh crate.** Shatranj shares chess's board, its sliding pieces,
//! its check detection and its move encoding, so it lives in this crate beside
//! Chess and Chess960 rather than alongside the tafl and draughts crates. A
//! crate is drawn round a game *family*, and this is the same family.
//!
//! Movement needs no new machinery: the ferz is a leaper table and so is the
//! alfil, built by the same `const fn` that produces the knight's and the
//! king's — the tables that already pass a full published perft. What is
//! genuinely new here is not how the pieces move. It is how the game ends.
//!
//! **There is no perft corpus for shatranj**, and a missing corpus is the
//! normal reason to refuse to build a variant. It is acceptable *here
//! specifically*, and the argument is narrow enough to be worth stating: every
//! piece either moves exactly as it does in chess (pawn forward, knight, rook,
//! king) or is a fixed offset table checkable by construction. Nothing here
//! generates a move by a rule the perft gate has not already exercised. The
//! endings, which the gate would not cover in any case, are tested directly
//! below.

use crate::attacks;
use crate::chess_move::{Move, MoveKind, MoveList};
use crate::movegen;
use crate::piece::Role;
use crate::position::Position;

/// The opening array: chess's, with the queen replaced by a ferz and the
/// bishops by alfils.
///
/// The castling field is `-` and always will be — shatranj has no castling, so
/// no position it produces can carry a right. That is also why nothing in this
/// module touches [`crate::castling`].
pub const STARTING_POSITION: &str = "rnefkenr/pppppppp/8/8/8/8/PPPPPPPP/RNEFKENR w - - 0 1";

/// Every legal move for the side to move.
///
/// Same shape as chess: generate pseudo-legal moves, then drop the ones that
/// leave the mover's own king attacked. The legality filter is shared because
/// "you may not move into check" is shared, and [`Position::attackers_to`]
/// already knows about both new pieces.
pub fn legal_moves(position: &Position) -> MoveList {
    let mut moves = pseudo_legal_moves(position);
    let us = position.side_to_move();
    moves.retain(|mv| !position.make_move(mv).is_check_against(us));
    moves
}

/// Everything that obeys piece movement, ignoring whether it leaves the king
/// attacked.
///
/// There is no castling generator to call. That absence is the rule.
fn pseudo_legal_moves(position: &Position) -> MoveList {
    let mut moves = MoveList::new();
    // Chess's pawn without its double step, promoting to a ferz and offering no
    // choice about it. Shared with chaturanga, whose padati is the same piece
    // under a different name and promotes to the same `Role` — the two were
    // byte-identical copies, and neither game has a published corpus that would
    // have noticed them coming apart (see `movegen::stepping_pawn_moves`).
    movegen::stepping_pawn_moves(position, &mut moves, Role::Ferz);
    generate_piece_moves(position, &mut moves);
    moves
}

/// The five roles shatranj has besides the pawn.
///
/// No queen and no bishop appear, and no position this module can reach
/// contains one — the opening array has neither and the only promotion is to a
/// ferz. The rook and the knight are chess's exactly.
fn generate_piece_moves(position: &Position, moves: &mut MoveList) {
    let us = position.side_to_move();
    let occupied = position.occupied();
    let ours = position.by_color(us);
    let enemies = position.by_color(us.opponent());

    for role in [
        Role::Knight,
        Role::Rook,
        Role::King,
        Role::Ferz,
        Role::Alfil,
    ] {
        for from in position.pieces(us, role) {
            let targets = match role {
                Role::Knight => attacks::knight(from),
                Role::Rook => attacks::rook(from, occupied),
                Role::King => attacks::king(from),
                Role::Ferz => attacks::ferz(from),
                Role::Alfil => attacks::alfil(from),
                _ => unreachable!("shatranj has no other piece to generate here"),
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

/// Whether the side to move can leave its opponent with nothing but a king,
/// in one move.
///
/// This is the *deferred* half of the baring rule and the reason baring cannot
/// be decided from material alone: it is not a question about the position, it
/// is a question about the position's replies. Asked only when the side to move
/// is itself already bare, so the loop is over a small move list.
pub fn can_bare_opponent(position: &Position, moves: &MoveList) -> bool {
    let them = position.side_to_move().opponent();
    moves
        .into_iter()
        .any(|mv| position.make_move(mv).is_bare(them))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piece::Color;
    use crate::square::Square;
    use crate::variant::{Shatranj, Variant};

    fn position(fen: &str) -> Position {
        Position::from_fen(fen).expect("test FEN should parse")
    }

    fn uci_moves(fen: &str) -> Vec<String> {
        let mut moves: Vec<String> = legal_moves(&position(fen))
            .into_iter()
            .map(|mv| mv.to_uci(Shatranj.castling_notation()))
            .collect();
        moves.sort();
        moves
    }

    #[test]
    fn the_opening_array_is_chesss_with_two_pieces_swapped() {
        let start = Shatranj.starting_position();
        assert_eq!(start.to_fen(), STARTING_POSITION);
        // Every chess piece except the queen and the bishops, in the same places.
        assert_eq!(start.by_role(Role::Rook).count(), 4);
        assert_eq!(start.by_role(Role::Knight).count(), 4);
        assert_eq!(start.by_role(Role::Pawn).count(), 16);
        // And none of the two chess pieces shatranj does not have.
        assert_eq!(start.by_role(Role::Queen).count(), 0);
        assert_eq!(start.by_role(Role::Bishop).count(), 0);
        assert_eq!(start.by_role(Role::Ferz).count(), 2);
        assert_eq!(start.by_role(Role::Alfil).count(), 4);
    }

    /// Sixteen: eight single pawn steps, four knight moves — and **four alfil
    /// moves**, which is the part worth a test.
    ///
    /// Chess has twenty. Removing the eight double steps would leave twelve,
    /// and twelve was the wrong answer: the alfils leap out over their own
    /// unmoved pawns on move one, because a leaper does not consult the square
    /// it passes over. A bishop on c1 has nowhere to go behind that pawn wall;
    /// an alfil on c1 reaches a3 and e3 immediately. That is the clearest
    /// single demonstration that these two pieces are not bishops wearing a
    /// different hat.
    #[test]
    fn the_opening_position_has_sixteen_moves() {
        let moves = uci_moves(STARTING_POSITION);
        assert_eq!(moves.len(), 16, "{moves:?}");
        assert!(!moves.contains(&"e2e4".to_string()), "no double step");
        assert!(moves.contains(&"e2e3".to_string()));
        assert!(moves.contains(&"b1c3".to_string()));
        for leap in ["c1a3", "c1e3", "f1d3", "f1h3"] {
            assert!(moves.contains(&leap.to_string()), "{leap} over the pawns");
        }
    }

    #[test]
    fn the_ferz_moves_one_square_diagonally_and_nothing_else() {
        let moves = uci_moves("4k3/8/8/8/3F4/8/8/4K3 w - - 0 1");
        let ferz: Vec<&String> = moves.iter().filter(|m| m.starts_with("d4")).collect();
        assert_eq!(ferz.len(), 4, "{ferz:?}");
        for square in ["c5", "e5", "c3", "e3"] {
            assert!(moves.contains(&format!("d4{square}")), "{square}");
        }
    }

    /// The alfil leaps: the intervening square is not consulted, which is what
    /// distinguishes it from a bishop of range two.
    #[test]
    fn the_alfil_leaps_over_whatever_stands_between() {
        let blocked = "4k3/8/8/8/3E4/2PPP3/8/4K3 w - - 0 1";
        let moves = uci_moves(blocked);
        for square in ["b6", "f6", "b2", "f2"] {
            assert!(
                moves.contains(&format!("d4{square}")),
                "the alfil should reach {square} over its own pawns"
            );
        }
    }

    /// Colourbound to eight squares, by construction rather than by a rule.
    #[test]
    fn an_alfil_can_never_leave_its_eight_squares() {
        let mut seen = crate::Bitboard::EMPTY;
        let mut frontier = vec![Square::new(27)]; // d4
        while let Some(from) = frontier.pop() {
            if seen.contains(from) {
                continue;
            }
            seen |= crate::Bitboard::from_square(from);
            for to in attacks::alfil(from) {
                frontier.push(to);
            }
        }
        assert_eq!(seen.count(), 8, "an alfil's whole world is eight squares");
    }

    #[test]
    fn a_pawn_promotes_to_a_ferz_and_is_given_no_choice() {
        let moves = uci_moves("4k3/1P6/8/8/8/8/8/4K3 w - - 0 1");
        let promotions: Vec<&String> = moves.iter().filter(|m| m.starts_with("b7b8")).collect();
        assert_eq!(promotions, vec!["b7b8f"], "one promotion, to a ferz");
    }

    #[test]
    fn there_is_no_castling_however_the_board_is_arranged() {
        // A king and rook on their chess squares with everything between them
        // empty: the one position where chess would offer a castle.
        let moves = uci_moves("4k3/8/8/8/8/8/8/4K2R w - - 0 1");
        assert!(!moves.contains(&"e1g1".to_string()));
        assert!(!moves.contains(&"e1h1".to_string()));
    }

    /// The rule that most distinguishes shatranj from chess: the position that
    /// draws a game of chess wins a game of shatranj, for the side that
    /// produced it.
    #[test]
    fn stalemate_wins_rather_than_draws() {
        // Black king a8, boxed by a king on a6 and a rook on the b-file. Not in
        // check, and with nowhere legal to go.
        let stalemate = position("k7/8/K7/8/8/8/8/1R6 b - - 0 1");
        assert!(legal_moves(&stalemate).is_empty(), "no legal move");
        assert!(
            !stalemate.is_check(),
            "and not in check — this is stalemate"
        );
        assert_eq!(
            Shatranj.outcome(&stalemate),
            Some(crate::Outcome::Win {
                winner: Color::White,
                reason: crate::WinReason::Stalemate,
            }),
            "in chess this is a draw"
        );
    }

    #[test]
    fn baring_the_king_wins_when_it_cannot_be_answered() {
        // Black has only a king, and no way to reach White's rook.
        let bared = position("k7/8/8/8/8/8/R7/4K3 b - - 0 1");
        assert!(!legal_moves(&bared).is_empty(), "the king can still move");
        assert_eq!(
            Shatranj.outcome(&bared),
            Some(crate::Outcome::Win {
                winner: Color::White,
                reason: crate::WinReason::BareKing,
            })
        );
    }

    /// The one-ply-deferred ending, and the reason baring cannot be decided
    /// from material: the same *material* is a loss above and a draw here. Only
    /// the replies tell them apart.
    #[test]
    fn baring_is_drawn_when_the_bared_king_bares_back() {
        // Black is bare, but White's last piece is a knight standing next to
        // the black king and defended by nobody.
        let mutual = position("8/8/8/3kN3/8/8/8/6K1 b - - 0 1");
        assert!(mutual.is_bare(Color::Black));
        assert!(!mutual.is_bare(Color::White), "White still has a knight");
        assert_eq!(
            Shatranj.outcome(&mutual),
            Some(crate::Outcome::Draw {
                reason: crate::DrawReason::BothKingsBared,
            }),
            "Kxe5 bares White in reply, so this is drawn and not lost"
        );
    }

    #[test]
    fn two_lone_kings_are_drawn() {
        let nothing = position("k7/8/8/8/8/8/8/7K w - - 0 1");
        assert_eq!(
            Shatranj.outcome(&nothing),
            Some(crate::Outcome::Draw {
                reason: crate::DrawReason::BothKingsBared,
            })
        );
    }

    /// A bare king is never rescued by the insufficient-material draw, because
    /// shatranj does not have one — thin material is not a dead game here, it
    /// is a decided one.
    #[test]
    fn there_is_no_insufficient_material_draw() {
        for fen in [
            "k7/8/8/8/8/8/8/7K w - - 0 1",
            "k7/8/8/8/8/8/R7/4K3 b - - 0 1",
        ] {
            assert!(!Shatranj.is_insufficient_material(&position(fen)), "{fen}");
        }
    }

    #[test]
    fn checkmate_is_still_a_win() {
        // Back-rank mate by two rooks: a8 covers the rank the king stands on,
        // b7 covers the one it would flee to. Black is *also* bare here, which
        // is the point — mate is asked first, so this is Checkmate and not
        // BareKing.
        let mate = position("R5k1/1R6/8/8/8/8/8/4K3 b - - 0 1");
        assert_eq!(
            Shatranj.outcome(&mate),
            Some(crate::Outcome::Win {
                winner: Color::White,
                reason: crate::WinReason::Checkmate,
            })
        );
    }
}
