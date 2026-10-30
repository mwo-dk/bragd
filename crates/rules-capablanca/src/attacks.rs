//! Attack tables for a ten-by-eight board.
//!
//! The same construction as `crates/rules-chess`: classical ray attacks for the
//! sliders, precomputed leaper tables for the rest, everything built at compile
//! time. Magic bitboards would be faster and much harder to read, and this
//! crate is the authority for real Games.
//!
//! The two compound pieces need no code of their own. An archbishop is
//! `bishop | knight` and a chancellor is `rook | knight`, evaluated at the call
//! site — a union, not a new movement rule, which is the whole reason this game
//! is cheap once the board is wide enough.

use crate::board::{Bitboard, FILES, RANKS, SQUARES, Square};
use crate::piece::Color;

/// The eight ray directions as (file step, rank step), clockwise from north.
const DIRECTIONS: [(i16, i16); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];

const ROOK_DIRECTIONS: [usize; 4] = [0, 2, 4, 6];
const BISHOP_DIRECTIONS: [usize; 4] = [1, 3, 5, 7];

/// Whether a direction increases the square index, which decides which end of
/// the blocker set is the nearest blocker.
const fn is_positive(direction: usize) -> bool {
    let (file_step, rank_step) = DIRECTIONS[direction];
    rank_step > 0 || (rank_step == 0 && file_step > 0)
}

/// `RAYS[direction][square]` — every square in that direction, to the edge.
const RAYS: [[Bitboard; SQUARES]; 8] = {
    let mut rays = [[Bitboard::EMPTY; SQUARES]; 8];
    let mut direction = 0;
    while direction < 8 {
        let (file_step, rank_step) = DIRECTIONS[direction];
        let mut origin = 0;
        while origin < SQUARES {
            let mut file = (origin % FILES as usize) as i16 + file_step;
            let mut rank = (origin / FILES as usize) as i16 + rank_step;
            let mut ray = 0u128;
            while file >= 0 && file < FILES as i16 && rank >= 0 && rank < RANKS as i16 {
                ray |= 1u128 << (rank * FILES as i16 + file);
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
///
/// The bounds check is on file and rank rather than on the resulting index,
/// which is the difference between a knight on the j-file staying put and one
/// reappearing on the a-file of the next rank.
const fn leaper_table(offsets: &[(i16, i16)]) -> [Bitboard; SQUARES] {
    let mut table = [Bitboard::EMPTY; SQUARES];
    let mut origin = 0;
    while origin < SQUARES {
        let file = (origin % FILES as usize) as i16;
        let rank = (origin / FILES as usize) as i16;
        let mut attacks = 0u128;
        let mut i = 0;
        while i < offsets.len() {
            let (file_step, rank_step) = offsets[i];
            let target_file = file + file_step;
            let target_rank = rank + rank_step;
            if target_file >= 0
                && target_file < FILES as i16
                && target_rank >= 0
                && target_rank < RANKS as i16
            {
                attacks |= 1u128 << (target_rank * FILES as i16 + target_file);
            }
            i += 1;
        }
        table[origin] = Bitboard(attacks);
        origin += 1;
    }
    table
}

const KNIGHT: [Bitboard; SQUARES] = leaper_table(&[
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
]);

const KING: [Bitboard; SQUARES] = leaper_table(&[
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
]);

/// `PAWN[color][square]` — where a pawn of that colour *attacks*, which is not
/// where it moves.
const PAWN: [[Bitboard; SQUARES]; 2] = [
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

/// Squares attacked along one ray, stopping at and including the first occupied
/// square.
fn ray(direction: usize, from: Square, occupied: Bitboard) -> Bitboard {
    let full = RAYS[direction][from.index() as usize];
    let blockers = full & occupied;
    if blockers.is_empty() {
        return full;
    }
    let nearest = if is_positive(direction) {
        blockers.0.trailing_zeros()
    } else {
        127 - blockers.0.leading_zeros()
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

pub fn queen(from: Square, occupied: Bitboard) -> Bitboard {
    bishop(from, occupied) | rook(from, occupied)
}

/// Bishop plus knight. The union is the definition, and there is nothing else
/// to it — an archbishop is not a piece with its own geometry.
pub fn archbishop(from: Square, occupied: Bitboard) -> Bitboard {
    bishop(from, occupied) | knight(from)
}

/// Rook plus knight.
pub fn chancellor(from: Square, occupied: Bitboard) -> Bitboard {
    rook(from, occupied) | knight(from)
}

/// The squares strictly between two squares that share a rank, file or
/// diagonal; empty otherwise. Used for the castling path.
pub fn between(a: Square, b: Square) -> Bitboard {
    for rays in &RAYS {
        let from_a = rays[a.index() as usize];
        if from_a.contains(b) {
            return from_a & !rays[b.index() as usize] & !Bitboard::from_square(b);
        }
    }
    Bitboard::EMPTY
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(name: &str) -> Square {
        let bytes = name.as_bytes();
        Square::from_file_rank((bytes[0] - b'a') as i16, (bytes[1] - b'1') as i16)
            .expect("a square on the board")
    }

    fn names(bitboard: Bitboard) -> Vec<String> {
        bitboard.map(|s| s.to_string()).collect()
    }

    /// The bug a wider board invites, and the reason the tables check file and
    /// rank rather than the index: on an eight-wide board these offsets would
    /// wrap, and here they must not.
    #[test]
    fn nothing_wraps_around_the_j_file() {
        assert_eq!(names(knight(square("a1"))), ["c2", "b3"]);
        assert_eq!(names(knight(square("j8"))), ["i6", "h7"]);
        assert_eq!(names(king(square("j1"))), ["i1", "i2", "j2"]);
        // A rook on the j-file sees nine squares along rank 1, all of them west.
        let along = rook(square("j1"), Bitboard::EMPTY) & !Bitboard::from_square(square("j8"));
        assert!(
            !along.contains(square("a2")),
            "rank 1 does not reach rank 2"
        );
    }

    #[test]
    fn a_knight_in_the_middle_has_eight_squares() {
        assert_eq!(knight(square("e4")).count(), 8);
    }

    /// The compound pieces are unions and nothing else. Checked as an identity
    /// rather than by listing squares, because that identity *is* the piece.
    #[test]
    fn the_compounds_are_exactly_their_two_halves() {
        let occupied = Bitboard::EMPTY.with(square("e4")).with(square("c6"));
        for index in 0..SQUARES as u8 {
            let from = Square::at(index);
            assert_eq!(
                archbishop(from, occupied),
                bishop(from, occupied) | knight(from)
            );
            assert_eq!(
                chancellor(from, occupied),
                rook(from, occupied) | knight(from)
            );
        }
    }

    /// An archbishop reaches every square of the board given enough moves,
    /// unlike the bishop half of it — worth stating, because the bishop's
    /// colour-boundedness is the thing people expect it to inherit.
    #[test]
    fn an_archbishop_is_not_colourbound() {
        let mut seen = Bitboard::EMPTY;
        let mut frontier = vec![square("a1")];
        while let Some(from) = frontier.pop() {
            if seen.contains(from) {
                continue;
            }
            seen |= Bitboard::from_square(from);
            for to in archbishop(from, Bitboard::EMPTY) {
                frontier.push(to);
            }
        }
        assert_eq!(seen.count(), SQUARES as u32);
    }

    #[test]
    fn sliders_stop_at_the_first_blocker_but_include_it() {
        let occupied = Bitboard::EMPTY.with(square("a4"));
        let attacks = rook(square("a1"), occupied);
        assert!(attacks.contains(square("a4")), "the blocker is attackable");
        assert!(!attacks.contains(square("a5")), "nothing beyond it");
        assert!(
            attacks.contains(square("j1")),
            "and the open direction runs"
        );
    }

    /// If a queen on `a` attacks `b`, a queen on `b` attacks `a`. A ray table
    /// with one direction miswired fails this and very little else.
    #[test]
    fn sliders_are_symmetric() {
        let occupied = Bitboard::EMPTY.with(square("d1")).with(square("e5"));
        for index in 0..SQUARES as u8 {
            let from = Square::at(index);
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
        assert_eq!(
            names(between(square("f1"), square("j1"))),
            ["g1", "h1", "i1"]
        );
        assert_eq!(
            names(between(square("f1"), square("a1"))),
            ["b1", "c1", "d1", "e1"]
        );
        assert_eq!(between(square("f1"), square("g1")), Bitboard::EMPTY);
        assert_eq!(between(square("a1"), square("b8")), Bitboard::EMPTY);
    }
}
