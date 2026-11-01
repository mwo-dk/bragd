//! Named motifs of a chess move, for Annotations: what the move does on the board. A fork, a pin,
//! or a piece of the mover's left hanging. "Missed mate" is not here: it needs the Engine's
//! scores, not the board (`bragd_positions::annotation`).
//!
//! Piece values: pawn 1, knight 3, bishop 3, rook 5, queen 9; the king counts as more than all.

use crate::attacks;
use crate::bitboard::Bitboard;
use crate::piece::{Color, Role};
use crate::square::Square;
use crate::{CastlingNotation, Position};

fn value(role: Role) -> u8 {
    match role {
        Role::Pawn => 1,
        Role::Knight | Role::Bishop => 3,
        Role::Rook => 5,
        Role::Queen => 9,
        Role::King => 100,
        _ => 3,
    }
}

fn attacks_from(role: Role, color: Color, square: Square, occupied: Bitboard) -> Bitboard {
    match role {
        Role::Pawn => attacks::pawn(color, square),
        Role::Knight => attacks::knight(square),
        Role::Bishop => attacks::bishop(square, occupied),
        Role::Rook => attacks::rook(square, occupied),
        Role::Queen => attacks::queen(square, occupied),
        Role::King => attacks::king(square),
        _ => Bitboard::EMPTY,
    }
}

fn hanging(position: &Position, mover: Color) -> Bitboard {
    let opponent = mover.opponent();
    let occupied = position.occupied();
    let mut result = Bitboard::EMPTY;
    for square in position.by_color(mover) & !position.pieces(mover, Role::King) {
        let attackers = position.attackers_to(square, occupied) & position.by_color(opponent);
        if attackers.is_empty() {
            continue;
        }
        let defenders = position.attackers_to(square, occupied) & position.by_color(mover);
        let target_value = value(position.piece_at(square).expect("occupied square").role);
        let attacked_by_cheaper = attackers.into_iter().any(|attacker| {
            value(position.piece_at(attacker).expect("attacker square").role) < target_value
        });
        if defenders.is_empty() || attacked_by_cheaper {
            result = result.with(square);
        }
    }
    result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motif {
    /// The moved piece attacks two or more of the other side's pieces, each the king, worth more
    /// than the moved piece, or not defended.
    Fork,
    /// The moved piece, a bishop, rook or queen, attacks a piece of the other side that stands in
    /// front of that side's king, or of a piece worth more than it and than the moved piece, on the
    /// same line.
    Pin,
    /// After the move, a piece of the mover's (not its king) is attacked and either not defended
    /// or attacked by a piece worth less; and it was not so before the move (the moved piece
    /// counts as new on its new square).
    Hangs,
}

/// The motifs of `uci` (a legal move in UCI, castling as the king's two-square move or as king
/// takes rook) played in `before`, in the order Fork, Pin, Hangs. Empty for a move that is not
/// legal there.
pub fn motifs(before: &Position, uci: &str) -> Vec<Motif> {
    let Some(mv) = before.legal_moves().into_iter().find(|mv| {
        mv.to_uci(CastlingNotation::KingDestination) == uci
            || mv.to_uci(CastlingNotation::KingTakesRook) == uci
    }) else {
        return Vec::new();
    };

    let mover = before.side_to_move();
    let opponent = mover.opponent();
    let after = before.make_move(mv);
    let moved = after.piece_at(mv.to()).expect("a legal move places a piece at its destination");
    let to = mv.to();
    let occupied = after.occupied();
    let moved_attacks = attacks_from(moved.role, mover, to, occupied);
    let mut result = Vec::new();

    let fork_targets = moved_attacks & after.by_color(opponent);
    let valuable_or_undefended = fork_targets.into_iter().filter(|&square| {
        let target = after.piece_at(square).expect("fork target is occupied");
        target.role == Role::King
            || value(target.role) > value(moved.role)
            || (after.attackers_to(square, occupied) & after.by_color(opponent)).is_empty()
    }).count();
    if valuable_or_undefended >= 2 {
        result.push(Motif::Fork);
    }

    if matches!(moved.role, Role::Bishop | Role::Rook | Role::Queen) {
        let mut pin = false;
        for piece_square in moved_attacks & after.by_color(opponent) {
            let blockers_removed = occupied.without(piece_square);
            let revealed = attacks_from(moved.role, mover, to, blockers_removed);
            for target_square in revealed & !moved_attacks {
                if !attacks::between(to, target_square).contains(piece_square) {
                    continue;
                }
                if target_square == after.king_square(opponent)
                    || after.piece_at(target_square).is_some_and(|piece| {
                        // Worth more than the pinned piece and than the pinner: a pawn in front of a
                        // knight, pinned by a queen, is no pin worth naming.
                        piece.color == opponent
                            && value(piece.role) > value(after.piece_at(piece_square).expect("attacked piece is occupied").role)
                            && value(piece.role) > value(moved.role)
                    })
                {
                    pin = true;
                    break;
                }
            }
            if pin { break; }
        }
        if pin { result.push(Motif::Pin); }
    }

    let hanging_before = hanging(before, mover);
    let hanging_after = hanging(&after, mover);
    if hanging_after.into_iter().any(|square| {
        !hanging_before.contains(square) || square == mv.to()
    }) {
        result.push(Motif::Hangs);
    }
    result
}
