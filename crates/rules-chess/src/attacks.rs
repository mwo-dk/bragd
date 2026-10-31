//! Attack tables.
//!
//! Everything here is computed at compile time. Sliding pieces use classical
//! ray attacks: a precomputed ray per square per direction, masked at the first
//! blocker. Magic bitboards are perhaps three times faster and considerably
//! harder to get right; at perft depths this is fast enough, and being able to
//! read the code is worth more than the constant factor when this crate is the
//! authority for real Games.

use crate::bitboard::Bitboard;
use crate::piece::Color;
use crate::square::Square;

/// The eight ray directions as (file step, rank step), ordered clockwise from
/// north. Indices 0..8 are used directly as ray table indices.
const DIRECTIONS: [(i32, i32); 8] = [
    (0, 1),   // 0 north
    (1, 1),   // 1 north-east
    (1, 0),   // 2 east
    (1, -1),  // 3 south-east
    (0, -1),  // 4 south
    (-1, -1), // 5 south-west
    (-1, 0),  // 6 west
    (-1, 1),  // 7 north-west
];

const ROOK_DIRECTIONS: [usize; 4] = [0, 2, 4, 6];
const BISHOP_DIRECTIONS: [usize; 4] = [1, 3, 5, 7];

/// Whether a direction increases the square index. Determines which end of the
/// blocker set is the *nearest* blocker.
const fn is_positive(direction: usize) -> bool {
    let (file_step, rank_step) = DIRECTIONS[direction];
    rank_step > 0 || (rank_step == 0 && file_step > 0)
}

/// `RAYS[direction][square]` — every square in that direction, to the edge.
const RAYS: [[Bitboard; 64]; 8] = {
    let mut rays = [[Bitboard::EMPTY; 64]; 8];
    let mut direction = 0;
    while direction < 8 {
        let (file_step, rank_step) = DIRECTIONS[direction];
        let mut origin = 0;
        while origin < 64 {
            let mut file = (origin % 8) as i32 + file_step;
            let mut rank = (origin / 8) as i32 + rank_step;
            let mut ray = 0u64;
            while file >= 0 && file < 8 && rank >= 0 && rank < 8 {
                ray |= 1u64 << (rank * 8 + file);
                file += file_step;
                rank += rank_step;
            }
            rays[direction][origin] = Bitboard(ray);
            origin += 1;
        }
        direction += 1;
    }
    rays
};

/// Builds a leaper's table from a list of (file, rank) offsets.
const fn leaper_table(offsets: &[(i32, i32)]) -> [Bitboard; 64] {
    let mut table = [Bitboard::EMPTY; 64];
    let mut origin = 0;
    while origin < 64 {
        let file = (origin % 8) as i32;
        let rank = (origin / 8) as i32;
        let mut attacks = 0u64;
        let mut i = 0;
        while i < offsets.len() {
            let (file_step, rank_step) = offsets[i];
            let target_file = file + file_step;
            let target_rank = rank + rank_step;
            if target_file >= 0 && target_file < 8 && target_rank >= 0 && target_rank < 8 {
                attacks |= 1u64 << (target_rank * 8 + target_file);
            }
            i += 1;
        }
        table[origin] = Bitboard(attacks);
        origin += 1;
    }
    table
}

const KNIGHT: [Bitboard; 64] = leaper_table(&[
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
]);

const KING: [Bitboard; 64] = leaper_table(&[
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
]);

/// The shatranj counsellor: one square diagonally, and nothing else.
///
/// Literally the king's table masked to the diagonals, which is worth saying
/// because it is the reason shatranj needs no new movement machinery: this is
/// built from the same `leaper_table` that already passes a full published
/// perft as the knight and the king.
const FERZ: [Bitboard; 64] = leaper_table(&[(1, 1), (1, -1), (-1, -1), (-1, 1)]);

/// The shatranj elephant: exactly two squares diagonally, leaping over
/// whatever stands between.
///
/// A *fixed offset table*, not a ray — the intervening square is not consulted,
/// so occupancy is not a parameter. That is also why the alfil is colourbound
/// to eight squares of the board and can never attack the other fifty-six; the
/// table says so by construction and no rule has to enforce it.
const ALFIL: [Bitboard; 64] = leaper_table(&[(2, 2), (2, -2), (-2, -2), (-2, 2)]);

/// `GAJA[color][square]` — chaturanga's elephant: one square straight forward,
/// or one square in any diagonal direction.
///
/// The only piece here besides the pawn whose table depends on whose it is, and
/// for the same reason: it has a forward. That asymmetry is the whole reason it
/// is a role of its own rather than an alfil under another name — a colourless
/// table could not express it.
const GAJA: [[Bitboard; 64]; 2] = [
    leaper_table(&[(0, 1), (1, 1), (1, -1), (-1, -1), (-1, 1)]),
    leaper_table(&[(0, -1), (1, 1), (1, -1), (-1, -1), (-1, 1)]),
];

/// `PAWN[color][square]` — the squares a pawn of that colour *attacks*, which
/// is not where it moves. Kept separate from pushes for exactly that reason.
const PAWN: [[Bitboard; 64]; 2] = [
    leaper_table(&[(-1, 1), (1, 1)]),
    leaper_table(&[(-1, -1), (1, -1)]),
];

pub fn knight(from: Square) -> Bitboard {
    KNIGHT[from.index() as usize]
}

pub fn king(from: Square) -> Bitboard {
    KING[from.index() as usize]
}

pub fn pawn(color: Color, from: Square) -> Bitboard {
    PAWN[color.index()][from.index() as usize]
}

pub fn ferz(from: Square) -> Bitboard {
    FERZ[from.index() as usize]
}

pub fn alfil(from: Square) -> Bitboard {
    ALFIL[from.index() as usize]
}

pub fn gaja(color: Color, from: Square) -> Bitboard {
    GAJA[color.index()][from.index() as usize]
}

/// Squares attacked along one ray, stopping at (and including) the first
/// occupied square.
fn ray(direction: usize, from: Square, occupied: Bitboard) -> Bitboard {
    let full = RAYS[direction][from.index() as usize];
    let blockers = full & occupied;
    if blockers.is_empty() {
        return full;
    }
    // The nearest blocker is the lowest set bit going up the board, the highest
    // going down. Everything beyond it is shadowed.
    let nearest = if is_positive(direction) {
        blockers.0.trailing_zeros()
    } else {
        63 - blockers.0.leading_zeros()
    };
    full & !RAYS[direction][nearest as usize]
}

pub fn bishop(from: Square, occupied: Bitboard) -> Bitboard {
    let mut attacks = Bitboard::EMPTY;
    for direction in BISHOP_DIRECTIONS {
        attacks |= ray(direction, from, occupied);
    }
    attacks
}

pub fn rook(from: Square, occupied: Bitboard) -> Bitboard {
    let mut attacks = Bitboard::EMPTY;
    for direction in ROOK_DIRECTIONS {
        attacks |= ray(direction, from, occupied);
    }
    attacks
}

/// Seirawan's hawk: a bishop and a knight at once.
///
/// A union of two existing tables rather than a table of its own. The compound
/// pieces are the one case where that is exactly right — a hawk *is* the union,
/// by definition, and a separate table would be a copy of two others that could
/// drift from them.
pub fn hawk(from: Square, occupied: Bitboard) -> Bitboard {
    bishop(from, occupied) | knight(from)
}

/// Seirawan's elephant: a rook and a knight at once.
pub fn elephant(from: Square, occupied: Bitboard) -> Bitboard {
    rook(from, occupied) | knight(from)
}

pub fn queen(from: Square, occupied: Bitboard) -> Bitboard {
    bishop(from, occupied) | rook(from, occupied)
}

/// The squares strictly between two squares, if they share a rank, file or
/// diagonal; empty otherwise. Used for castling path checks.
pub fn between(a: Square, b: Square) -> Bitboard {
    for rays in &RAYS {
        let from_a = rays[a.index() as usize];
        if from_a.contains(b) {
            // Everything a sees in this direction, minus b and everything past it.
            return from_a & !rays[b.index() as usize] & !Bitboard::from_square(b);
        }
    }
    Bitboard::EMPTY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::square::squares::*;

    fn squares(bitboard: Bitboard) -> Vec<String> {
        bitboard.into_iter().map(|s| s.to_string()).collect()
    }

    // Expected squares are in index order — a1 first, h8 last — because that
    // is how a bitboard iterates, not alphabetically.

    #[test]
    fn knights_do_not_wrap_around_the_board() {
        assert_eq!(squares(knight(A1)), ["c2", "b3"]);
        assert_eq!(squares(knight(H8)), ["g6", "f7"]);
        assert_eq!(knight(Square::parse("e4").unwrap()).count(), 8);
    }

    #[test]
    fn kings_in_a_corner_have_three_squares() {
        assert_eq!(squares(king(A1)), ["b1", "a2", "b2"]);
        assert_eq!(king(E1).count(), 5);
    }

    #[test]
    fn pawn_attacks_are_diagonal_and_directional() {
        let e4 = Square::parse("e4").unwrap();
        assert_eq!(squares(pawn(Color::White, e4)), ["d5", "f5"]);
        assert_eq!(squares(pawn(Color::Black, e4)), ["d3", "f3"]);
        // Edge files attack one square, not two.
        assert_eq!(pawn(Color::White, A1).count(), 1);
    }

    #[test]
    fn sliders_stop_at_the_first_blocker_but_include_it() {
        let occupied = Bitboard::EMPTY.with(Square::parse("a4").unwrap());
        let attacks = rook(A1, occupied);
        assert!(
            attacks.contains(Square::parse("a4").unwrap()),
            "blocker itself is attackable"
        );
        assert!(
            !attacks.contains(Square::parse("a5").unwrap()),
            "nothing beyond the blocker"
        );
        assert!(
            attacks.contains(H1),
            "an unblocked direction reaches the edge"
        );
    }

    #[test]
    fn sliders_are_symmetric() {
        // If a bishop on a attacks b, a bishop on b attacks a. A ray table with
        // one direction miswired fails this and little else.
        let occupied = Bitboard::EMPTY.with(D1).with(Square::parse("e5").unwrap());
        for from in 0..64u8 {
            let from = Square::new(from);
            for to in queen(from, occupied) {
                assert!(
                    queen(to, occupied).contains(from),
                    "{from} attacks {to} but not the reverse"
                );
            }
        }
    }

    #[test]
    fn between_excludes_both_ends() {
        assert_eq!(squares(between(E1, H1)), ["f1", "g1"]);
        assert_eq!(squares(between(H1, E1)), ["f1", "g1"]);
        assert_eq!(
            between(E1, F1),
            Bitboard::EMPTY,
            "adjacent squares have nothing between"
        );
        assert_eq!(
            between(A1, B8),
            Bitboard::EMPTY,
            "unaligned squares have nothing between"
        );
        assert_eq!(squares(between(A1, D1)), ["b1", "c1"]);
    }
}
