//! Move generation.
//!
//! Two stages: generate pseudo-legal moves — everything that obeys how the
//! pieces move — then discard those that leave one's own king attacked. The
//! alternative, generating only legal moves directly, is faster and needs pin
//! and check-evasion masks that are hard to get right and harder to keep right
//! across variants. The engine is the authority for real Games, so
//! the version that is obviously correct wins; perft says whether that is still
//! fast enough, and at these depths it is.
//!
//! One case escapes the simple filter and is worth naming: en passant can
//! expose one's own king along a rank, because *two* pawns leave the rank at
//! once. The filter catches it anyway — it tests the position that results,
//! which no longer has either pawn on that rank — which is precisely why the
//! filter is written this way rather than as a pin mask.

use crate::attacks;
use crate::bitboard::{Bitboard, FILES, RANKS};
use crate::castling::Side;
use crate::chess_move::{Move, MoveKind, MoveList};
use crate::piece::{Color, Piece, Role};
use crate::position::Position;

const PROMOTION_ROLES: [Role; 4] = [Role::Queen, Role::Rook, Role::Bishop, Role::Knight];

/// Every legal move for the side to move.
pub fn legal_moves(position: &Position) -> MoveList {
    let mut moves = pseudo_legal_moves(position);
    let us = position.side_to_move();
    moves.retain(|mv| !position.make_move(mv).is_check_against(us));
    moves
}

/// Everything that obeys piece movement, ignoring whether it leaves the king
/// attacked. Castling is the exception: its path-under-attack rules are part of
/// how the move works, so they are enforced here.
pub fn pseudo_legal_moves(position: &Position) -> MoveList {
    let mut moves = MoveList::new();
    generate_pawn_moves(position, &mut moves);
    generate_piece_moves(position, &mut moves);
    generate_castles(position, &mut moves);
    moves
}

fn generate_pawn_moves(position: &Position, moves: &mut MoveList) {
    let us = position.side_to_move();
    let them = us.opponent();
    let forward = us.forward();
    let pawns = position.pieces(us, Role::Pawn);
    let empty = !position.occupied();
    let enemies = position.by_color(them);
    let last_rank = RANKS[us.promotion_rank() as usize];

    // Pushes. A double push is a single push that lands on the third rank and
    // keeps going, which is exactly the rule: the intermediate square must be
    // empty too.
    let single_pushes = pawns.shift(forward) & empty;
    let third_rank = RANKS[(us.pawn_rank() as i8 + forward.signum()) as usize];
    let double_pushes = (single_pushes & third_rank).shift(forward) & empty;

    for to in single_pushes {
        let from = to
            .offset(-forward)
            .expect("a pushed pawn came from somewhere");
        push_pawn_move(moves, from, to, false, last_rank.contains(to));
    }
    for to in double_pushes {
        let from = to
            .offset(-forward * 2)
            .expect("a double-pushed pawn came from somewhere");
        moves.push(Move::new(from, to, MoveKind::DoublePawnPush));
    }

    // Captures, one diagonal at a time. Masking the source file before shifting
    // is what stops a capture wrapping around the board.
    let toward_a_file = forward - 1;
    let toward_h_file = forward + 1;
    for (delta, source_mask) in [(toward_a_file, !FILES[0]), (toward_h_file, !FILES[7])] {
        let targets = (pawns & source_mask).shift(delta);

        for to in targets & enemies {
            let from = to
                .offset(-delta)
                .expect("a capturing pawn came from somewhere");
            push_pawn_move(moves, from, to, true, last_rank.contains(to));
        }

        if let Some(target) = position.en_passant_square()
            && targets.contains(target)
        {
            let from = target
                .offset(-delta)
                .expect("an en passant capturer came from somewhere");
            moves.push(Move::new(from, target, MoveKind::EnPassant));
        }
    }
}

/// A pawn move is four moves when it lands on the last rank.
fn push_pawn_move(
    moves: &mut MoveList,
    from: crate::square::Square,
    to: crate::square::Square,
    capture: bool,
    promoting: bool,
) {
    if promoting {
        for role in PROMOTION_ROLES {
            moves.push(Move::new(from, to, MoveKind::promotion_to(role, capture)));
        }
    } else {
        moves.push(Move::new(
            from,
            to,
            if capture {
                MoveKind::Capture
            } else {
                MoveKind::Quiet
            },
        ));
    }
}

/// The **stepping pawn**: chess's pawn without its double step, promoting to
/// one piece with no choice offered.
///
/// Shatranj's and chaturanga's, and it was written out twice — byte for byte
/// identical, down to the comment about masking the source file. Two copies of
/// twenty lines is not much until one of them is edited, and the edit that would
/// be made to one of them is the interesting one: both games are reconstructions
/// with no published corpus, so each is gated by a perft
/// count recorded from this engine. A change to one copy moves that variant's
/// numbers and the person making it updates them, which is exactly right; the
/// other copy sits there unchanged and equally green, and the two games quietly
/// stop being the same game about pawns.
///
/// # The parameter
///
/// `promotes_to` is a `Role`, not a variant. A *variant* parameter through
/// move generation is refused here on the ground that it makes the branch live
/// in every position of every variant and puts a value on the hot path carrying
/// the correctness gate. Neither applies: chess's generator above is untouched
/// and does not call this, there is no branch — the role is written into the
/// move — and the two callers hand over a constant. What it buys is that the
/// difference between the two games is named at the call site rather than
/// implied by which file you are reading.
///
/// **No double step means no en passant**, which is not a separate omission:
/// en passant exists only to answer the double step, so a position this produces
/// never carries an en passant square at all.
pub(crate) fn stepping_pawn_moves(position: &Position, moves: &mut MoveList, promotes_to: Role) {
    let us = position.side_to_move();
    let forward = us.forward();
    let pawns = position.pieces(us, Role::Pawn);
    let empty = !position.occupied();
    let enemies = position.by_color(us.opponent());
    let last_rank = RANKS[us.promotion_rank() as usize];

    for to in pawns.shift(forward) & empty {
        let from = to
            .offset(-forward)
            .expect("a pushed pawn came from somewhere");
        push_stepping_pawn_move(moves, from, to, false, last_rank.contains(to), promotes_to);
    }

    // Captures, one diagonal at a time. Masking the source file before shifting
    // is what stops a capture wrapping around the board.
    for (delta, source_mask) in [(forward - 1, !FILES[0]), (forward + 1, !FILES[7])] {
        for to in (pawns & source_mask).shift(delta) & enemies {
            let from = to
                .offset(-delta)
                .expect("a capturing pawn came from somewhere");
            push_stepping_pawn_move(moves, from, to, true, last_rank.contains(to), promotes_to);
        }
    }
}

/// One move, never four: the pawn becomes `promotes_to` or it does not promote.
fn push_stepping_pawn_move(
    moves: &mut MoveList,
    from: crate::square::Square,
    to: crate::square::Square,
    capture: bool,
    promoting: bool,
    promotes_to: Role,
) {
    let kind = match (promoting, capture) {
        (true, _) => MoveKind::promotion_to(promotes_to, capture),
        (false, true) => MoveKind::Capture,
        (false, false) => MoveKind::Quiet,
    };
    moves.push(Move::new(from, to, kind));
}

fn generate_piece_moves(position: &Position, moves: &mut MoveList) {
    let us = position.side_to_move();
    let occupied = position.occupied();
    let ours = position.by_color(us);
    let enemies = position.by_color(us.opponent());

    // Seven roles, not five. The hawk and the elephant are here because a
    // piece the generator does not walk generates no moves at all — which is
    // exactly what happened: a lone hawk on an empty board had nothing to play.
    //
    // The two extra entries cost nothing measurable. A variant that has no
    // hawks has an empty bitboard for them, and iterating an empty bitboard is
    // one test that fails immediately. Shatranj's leapers are not here for the
    // same reason they never were: they are generated by their own module,
    // which also filters what a shatranj king may do.
    for role in [
        Role::Knight,
        Role::Bishop,
        Role::Rook,
        Role::Queen,
        Role::King,
        Role::Hawk,
        Role::Elephant,
    ] {
        for from in position.pieces(us, role) {
            let targets = match role {
                Role::Knight => attacks::knight(from),
                Role::Bishop => attacks::bishop(from, occupied),
                Role::Rook => attacks::rook(from, occupied),
                Role::Queen => attacks::queen(from, occupied),
                Role::King => attacks::king(from),
                Role::Ferz => attacks::ferz(from),
                Role::Alfil => attacks::alfil(from),
                Role::Gaja => attacks::gaja(us, from),
                Role::Hawk => attacks::hawk(from, occupied),
                Role::Elephant => attacks::elephant(from, occupied),
                Role::Pawn => unreachable!("pawns are generated separately"),
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

fn generate_castles(position: &Position, moves: &mut MoveList) {
    let us = position.side_to_move();
    let them = us.opponent();
    let rights = position.castling_rights();
    let occupied = position.occupied();

    // A king may not castle out of check. True in every arrangement, and it is
    // also what makes the king safe to leave in the occupancy below.
    if rights.is_empty() || position.is_check() {
        return;
    }

    // Where the king actually is, rather than e1. A live right means the king
    // has not moved, so this *is* its home — but in Chess960 that home is any
    // file from b to g.
    let king_from = position.king_square(us);

    for side in Side::ALL {
        let Some(rook_from) = rights.rook_origin(us, side) else {
            continue;
        };
        // Rights arriving from FEN are not trusted to match the board.
        if position.piece_at(rook_from) != Some(Piece::new(us, Role::Rook)) {
            continue;
        }

        let king_to = side.king_destination(us);
        let rook_to = side.rook_destination(us);

        // **Two different sets of squares, and conflating them is the bug.**
        //
        // Must be EMPTY: everything both pieces travel over or land on — which
        // in Chess960 is not "between the king and the rook". A king on b1
        // castling queenside walks *east* to c1 while its rook goes a1 to d1,
        // so d1 must be clear and `between(b1, a1)` never mentions it. The two
        // castling pieces are exempt from their own test: they are what is
        // moving, and the king may end on the rook's square or the reverse.
        let path = attacks::between(king_from, king_to).with(king_to)
            | attacks::between(rook_from, rook_to).with(rook_to);
        let exempt = Bitboard::from_square(king_from) | Bitboard::from_square(rook_from);
        if !(path & occupied & !exempt).is_empty() {
            continue;
        }

        // Must be UNATTACKED: only the squares the king crosses and lands on.
        // The rook's path may be attacked freely — nothing is ever captured by
        // castling, and a rook is not a king.
        //
        // **The castling rook is lifted out of the occupancy for this test**,
        // and that is not a micro-optimisation. A rook on b1 shields c1 from an
        // enemy rook or queen on a1; after castling it stands on d1 and the
        // shield is gone, so a test against the current occupancy would allow a
        // castle that ends in check. The king, by contrast, does *not* need
        // lifting: for the king to be shielding a square on its own path, the
        // attacker's ray would have to pass through the king's own square —
        // which means it is giving check right now, and castling out of check
        // was refused above.
        let without_rook = occupied & !Bitboard::from_square(rook_from);
        let king_path = attacks::between(king_from, king_to)
            .with(king_to)
            .with(king_from);
        if king_path
            .into_iter()
            .any(|square| position.is_attacked_by(square, them, without_rook))
        {
            continue;
        }

        // Encoded as the king taking its own rook. See `Move::castle`.
        moves.push(Move::new(
            king_from,
            rook_from,
            match side {
                Side::Kingside => MoveKind::KingsideCastle,
                Side::Queenside => MoveKind::QueensideCastle,
            },
        ));
    }
}

/// All squares attacked by `color`, used for board display and later for hints.
pub fn attacked_squares(position: &Position, color: Color) -> Bitboard {
    let occupied = position.occupied();
    let mut attacked = Bitboard::EMPTY;
    for role in Role::ALL {
        for from in position.pieces(color, role) {
            attacked |= match role {
                Role::Pawn => attacks::pawn(color, from),
                Role::Knight => attacks::knight(from),
                Role::Bishop => attacks::bishop(from, occupied),
                Role::Rook => attacks::rook(from, occupied),
                Role::Queen => attacks::queen(from, occupied),
                Role::King => attacks::king(from),
                Role::Ferz => attacks::ferz(from),
                Role::Alfil => attacks::alfil(from),
                Role::Gaja => attacks::gaja(color, from),
                Role::Hawk => attacks::hawk(from, occupied),
                Role::Elephant => attacks::elephant(from, occupied),
            };
        }
    }
    attacked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess_move::CastlingNotation;

    fn position(fen: &str) -> Position {
        Position::from_fen(fen).expect("test FEN should parse")
    }

    fn uci_moves(fen: &str) -> Vec<String> {
        let mut moves: Vec<String> = position(fen)
            .legal_moves()
            .into_iter()
            .map(|mv| mv.to_uci(CastlingNotation::KingTakesRook))
            .collect();
        moves.sort();
        moves
    }

    fn moves_from(fen: &str, square: &str) -> Vec<String> {
        let mut moves: Vec<String> = position(fen)
            .legal_moves()
            .into_iter()
            .filter(|mv| mv.from().to_string() == square)
            .map(|mv| mv.to_uci(CastlingNotation::KingTakesRook))
            .collect();
        moves.sort();
        moves
    }

    #[test]
    fn the_opening_position_has_twenty_moves() {
        assert_eq!(uci_moves(crate::fen::STARTING_POSITION).len(), 20);
    }

    #[test]
    fn a_pinned_piece_may_not_step_out_of_the_pin() {
        // The knight on e2 is pinned against the king by the rook on e8.
        let fen = "4r2k/8/8/8/8/8/4N3/4K3 w - - 0 1";
        assert_eq!(moves_from(fen, "e2"), Vec::<String>::new());
        // A rook on the same file may move along it, though.
        let fen = "4r2k/8/8/8/8/8/4R3/4K3 w - - 0 1";
        assert_eq!(
            moves_from(fen, "e2"),
            ["e2e3", "e2e4", "e2e5", "e2e6", "e2e7", "e2e8"]
        );
    }

    #[test]
    fn en_passant_that_exposes_the_king_is_not_legal() {
        // White king a5, White pawn b5, Black pawn c5 having just double-pushed,
        // Black rook h5 — all on the fifth rank. Capturing en passant clears
        // *two* pawns off that rank at once, and the rook gives check. This is
        // the case a pin mask misses, because neither pawn is pinned on its own.
        let fen = "7k/8/8/KPp4r/8/8/8/8 w - c6 0 1";
        let moves = uci_moves(fen);
        assert!(!moves.contains(&"b5c6".to_string()), "generated: {moves:?}");

        // The same capture with the rook gone is perfectly legal.
        let fen = "7k/8/8/KPp5/8/8/8/8 w - c6 0 1";
        assert!(uci_moves(fen).contains(&"b5c6".to_string()));
    }

    #[test]
    fn castling_respects_every_obstacle() {
        // Internally a castle is the king taking its own rook, so it is spelled
        // by the rook's square here whatever the variant calls it on the wire.
        let clear = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        assert!(moves_from(clear, "e1").contains(&"e1h1".to_string()));
        assert!(moves_from(clear, "e1").contains(&"e1a1".to_string()));

        // A piece in the way, including on b1 which the king never touches.
        let blocked = "r3k2r/8/8/8/8/8/8/RN2K2R w KQkq - 0 1";
        assert!(!moves_from(blocked, "e1").contains(&"e1a1".to_string()));
        assert!(moves_from(blocked, "e1").contains(&"e1h1".to_string()));

        // Out of check.
        let in_check = "r3k2r/8/8/8/8/8/4r3/R3K2R w KQkq - 0 1";
        assert!(!moves_from(in_check, "e1").contains(&"e1h1".to_string()));

        // Through an attacked square.
        let through = "r3k2r/8/8/8/8/8/5r2/R3K2R w KQkq - 0 1";
        assert!(!moves_from(through, "e1").contains(&"e1h1".to_string()));
        assert!(moves_from(through, "e1").contains(&"e1a1".to_string()));

        // Into an attacked square.
        let into = "r3k2r/8/8/8/8/8/6r1/R3K2R w KQkq - 0 1";
        assert!(!moves_from(into, "e1").contains(&"e1h1".to_string()));

        // The rook's path may be attacked — only the king's may not.
        let rook_path = "r3k2r/8/8/8/8/8/1r6/R3K2R w KQkq - 0 1";
        assert!(moves_from(rook_path, "e1").contains(&"e1a1".to_string()));
    }

    #[test]
    fn chess960_castling_shapes_that_standard_chess_cannot_express() {
        // Each of these is a legal castle that no standard position can
        // produce, and each broke a different assumption in the old generator.

        // The king does not move at all: it is already on g1.
        let still = "4k3/8/8/8/8/8/8/6KR w H - 0 1";
        assert!(moves_from(still, "g1").contains(&"g1h1".to_string()));

        // King and rook swap squares.
        let swap = "4k3/8/8/8/8/8/8/5KR1 w G - 0 1";
        assert!(moves_from(swap, "f1").contains(&"f1g1".to_string()));

        // Castling queenside moves the king *east*, towards the h-file.
        let eastward = "4k3/8/8/8/8/8/8/RK6 w A - 0 1";
        assert!(moves_from(eastward, "b1").contains(&"b1a1".to_string()));

        // The rook's destination must be empty even though it lies beyond the
        // king — `between(king, rook)` never mentions d1 here.
        let blocked_beyond = "4k3/8/8/8/8/8/8/RKnB4 w A - 0 1";
        assert!(!moves_from(blocked_beyond, "b1").contains(&"b1a1".to_string()));
    }

    #[test]
    fn the_castling_rook_stops_shielding_the_king_once_it_moves() {
        // The one Chess960 rule that cannot be read off the pre-move board. The
        // rook on b1 blocks the a1 rook's attack on c1 right now; after
        // castling it stands on d1 and the king on c1 is in check. Testing the
        // king's path against the current occupancy would allow it.
        let shielded = "4k3/8/8/8/8/8/8/rR4K1 w B - 0 1";
        assert!(
            !moves_from(shielded, "g1").contains(&"g1b1".to_string()),
            "castling into a discovered check"
        );

        // Take the enemy rook away and the same castle is fine, which is what
        // says the refusal above is about the x-ray and not about the shape.
        let unshielded = "4k3/8/8/8/8/8/8/1R4K1 w B - 0 1";
        assert!(moves_from(unshielded, "g1").contains(&"g1b1".to_string()));
    }

    #[test]
    fn a_pawn_reaching_the_last_rank_produces_four_moves() {
        let moves = moves_from("4k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a7");
        assert_eq!(moves, ["a7a8b", "a7a8n", "a7a8q", "a7a8r"]);

        // And eight when it can also capture into promotion.
        let moves = moves_from("1n2k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a7");
        assert_eq!(moves.len(), 8, "{moves:?}");
    }

    #[test]
    fn double_pushes_need_both_squares_empty() {
        assert_eq!(
            moves_from("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1", "e2"),
            ["e2e3", "e2e4"]
        );
        assert_eq!(
            moves_from("4k3/8/8/8/4n3/8/4P3/4K3 w - - 0 1", "e2"),
            ["e2e3"]
        );
        assert_eq!(
            moves_from("4k3/8/8/8/8/4n3/4P3/4K3 w - - 0 1", "e2"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn pawn_captures_do_not_wrap_around_the_board() {
        // A White pawn on h5 must not "capture" onto a6.
        let moves = moves_from("4k3/8/p7/7P/8/8/8/4K3 w - - 0 1", "h5");
        assert_eq!(moves, ["h5h6"]);
        let moves = moves_from("4k3/8/7p/P7/8/8/8/4K3 w - - 0 1", "a5");
        assert_eq!(moves, ["a5a6"]);
    }

    #[test]
    fn only_moves_that_answer_a_check_are_legal() {
        // The rook on e8 checks along the file. White may step off it or block
        // with the bishop, and nothing else.
        let moves = uci_moves("4r2k/8/8/8/8/8/3B4/4K3 w - - 0 1");
        assert_eq!(moves, ["d2e3", "e1d1", "e1f1", "e1f2"]);
    }

    #[test]
    fn a_king_may_not_walk_along_a_checking_ray() {
        // Stepping from e1 to d1 or f1 stays on the rank the rook attacks. This
        // is the case that fails when the king's own square is left in the
        // occupancy while testing the destination — the king shadows the ray
        // from itself.
        let moves = moves_from("7k/8/8/8/8/8/8/r3K3 w - - 0 1", "e1");
        assert_eq!(moves, ["e1d2", "e1e2", "e1f2"]);
    }

    #[test]
    fn kings_may_not_stand_beside_each_other() {
        let moves = moves_from("8/8/8/4k3/8/4K3/8/8 w - - 0 1", "e3");
        assert_eq!(moves, ["e3d2", "e3d3", "e3e2", "e3f2", "e3f3"]);
    }

    #[test]
    fn attacked_squares_covers_every_role() {
        let position = position("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(attacked_squares(&position, Color::White).count(), 5);
    }

    /// The stepping pawn promotes to **what it is told**, and pushes one move.
    ///
    /// Worth its own test rather than leaning on the two variants' suites,
    /// because both of them ask for a ferz: a generator that ignored the
    /// parameter and hardcoded `Role::Ferz` would pass every shatranj and
    /// chaturanga assertion there is, and would be a trap laid for the third
    /// caller. Asked here with a role neither variant uses, so the answer can
    /// only come from the argument.
    #[test]
    fn a_stepping_pawn_promotes_to_the_role_it_is_given() {
        let board = position("4k3/1P6/8/8/8/8/8/4K3 w - - 0 1");
        for role in [Role::Ferz, Role::Knight] {
            let mut moves = MoveList::new();
            stepping_pawn_moves(&board, &mut moves, role);
            let promotions: Vec<Move> = moves
                .into_iter()
                .filter(|mv| mv.promotion_role().is_some())
                .collect();
            assert_eq!(promotions.len(), 1, "one promotion, never four: {role:?}");
            assert_eq!(promotions[0].promotion_role(), Some(role));
        }
    }

    /// And it never generates the double step, which is what makes en passant
    /// unreachable rather than merely unimplemented.
    #[test]
    fn a_stepping_pawn_has_no_double_step_to_answer() {
        let board = position("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1");
        let mut moves = MoveList::new();
        stepping_pawn_moves(&board, &mut moves, Role::Ferz);
        let squares: Vec<String> = moves
            .into_iter()
            .map(|mv| mv.to_uci(CastlingNotation::KingTakesRook))
            .collect();
        assert_eq!(squares, vec!["e2e3"], "chess would also offer e2e4");
    }
}
