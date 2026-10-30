//! Zobrist hashing — a 64-bit key identifying a position.
//!
//! Its own table rather than `crates/rules-chess`'s, for the reason everything else
//! here is its own: that one indexes sixty-four squares and six roles, and this
//! board has eighty and eight. Sharing it would mean the crate carrying chess's
//! perft gate owned a table sized for a game it does not play.
//!
//! This exists for **repetition**, and repetition is a classic place to be
//! subtly wrong. Two positions repeat only if the same side is to move, the
//! same castling rights survive, *and* the same en passant capture is
//! available — not merely the same pieces on the same squares.
//!
//! The constants are generated at compile time by SplitMix64 from a fixed seed,
//! so a key is stable across builds and machines. That matters because a Game
//! replayed in Bragd and on a game server must reach the same repetition count
//! in both.
//!
//! **Keys are never persisted.** They are rebuilt by replaying the stored moves,
//! which is what makes the seed load-bearing across builds and harmless to
//! change in itself.

use crate::board::{SQUARES, Square};
use crate::piece::{Color, Piece, Role};
use crate::position::{Castling, Position};

/// Fixed so that every build agrees. Deliberately not the same number as the
/// chess engine's: the two tables are different sizes and must never be
/// mistaken for interchangeable.
const SEED: u64 = 0x00CA_9AB1_A0CA_0080;

const fn split_mix_64(state: u64) -> (u64, u64) {
    let next_state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = next_state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (next_state, z ^ (z >> 31))
}

struct Tables {
    /// `pieces[color][role][square]`
    pieces: [[[u64; SQUARES]; Role::COUNT]; 2],
    /// `castling[color][king side]`, mixed in once per surviving right.
    ///
    /// Four numbers, where the chess engine needs one per rook *file* because
    /// Chess960 moves its rooks about. This game's rooks start on a and j and
    /// stay there, so the right is the whole story.
    castling: [[u64; 2]; 2],
    /// One per file. Only the file matters: the rank an en passant square can
    /// be on is decided by whose turn it is, which is already in the key.
    en_passant: [u64; crate::board::FILES as usize],
    black_to_move: u64,
}

const TABLES: Tables = {
    let mut state = SEED;
    let mut pieces = [[[0u64; SQUARES]; Role::COUNT]; 2];
    let mut color = 0;
    while color < 2 {
        let mut role = 0;
        while role < Role::COUNT {
            let mut square = 0;
            while square < SQUARES {
                let (next, value) = split_mix_64(state);
                state = next;
                pieces[color][role][square] = value;
                square += 1;
            }
            role += 1;
        }
        color += 1;
    }

    let mut castling = [[0u64; 2]; 2];
    let mut color = 0;
    while color < 2 {
        let mut side = 0;
        while side < 2 {
            let (next, value) = split_mix_64(state);
            state = next;
            castling[color][side] = value;
            side += 1;
        }
        color += 1;
    }

    let mut en_passant = [0u64; crate::board::FILES as usize];
    let mut file = 0;
    while file < crate::board::FILES as usize {
        let (next, value) = split_mix_64(state);
        state = next;
        en_passant[file] = value;
        file += 1;
    }

    let (_, black_to_move) = split_mix_64(state);

    Tables {
        pieces,
        castling,
        en_passant,
        black_to_move,
    }
};

pub fn piece(piece: Piece, square: Square) -> u64 {
    TABLES.pieces[piece.color.index()][piece.role.index()][square.index() as usize]
}

pub fn castling(rights: Castling) -> u64 {
    let mut key = 0;
    for color in Color::ALL {
        for king_side in [true, false] {
            if rights.has(color, king_side) {
                key ^= TABLES.castling[color.index()][usize::from(king_side)];
            }
        }
    }
    key
}

pub fn en_passant(square: Square) -> u64 {
    TABLES.en_passant[square.file() as usize]
}

pub fn black_to_move() -> u64 {
    TABLES.black_to_move
}

/// The key for a position, computed from scratch.
///
/// Recomputed rather than updated incrementally as moves are made. Incremental
/// update is faster and is how an engine that searches millions of positions
/// does it; this one is asked for a key once per move actually played, where the
/// cost is nothing and the risk of an update that drifts out of step with the
/// board is real.
pub fn key(position: &Position) -> u64 {
    let mut key = 0;
    for square in 0..SQUARES as u8 {
        let square = Square::at(square);
        if let Some(occupant) = position.piece_at(square) {
            key ^= piece(occupant, square);
        }
    }
    key ^= castling(position.castling());
    if let Some(square) = position.en_passant_square() {
        key ^= en_passant(square);
    }
    if position.side_to_move() == Color::Black {
        key ^= black_to_move();
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess_move::{Move, MoveKind};

    fn square(name: &str) -> Square {
        let bytes = name.as_bytes();
        Square::from_file_rank((bytes[0] - b'a') as i16, (bytes[1] - b'1') as i16)
            .expect("a square")
    }

    #[test]
    fn every_constant_is_distinct() {
        // A table with a repeat in it silently makes two different positions
        // equal, which shows up as a draw claim nobody can explain.
        let mut all = Vec::new();
        for color in 0..2 {
            for role in 0..Role::COUNT {
                for square in 0..SQUARES {
                    all.push(TABLES.pieces[color][role][square]);
                }
            }
        }
        for color in 0..2 {
            for side in 0..2 {
                all.push(TABLES.castling[color][side]);
            }
        }
        all.extend(TABLES.en_passant);
        all.push(TABLES.black_to_move);

        let before = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), before, "a Zobrist constant is repeated");
        assert!(!all.contains(&0), "a zero constant would hash to nothing");
    }

    #[test]
    fn the_same_position_has_the_same_key() {
        assert_eq!(key(&Position::starting()), key(&Position::starting()));
    }

    /// The three things that are not the pieces, each checked on its own —
    /// because a key that ignored one of them would be right about the board
    /// and wrong about the game.
    #[test]
    fn side_to_move_castling_and_en_passant_are_all_in_the_key() {
        let base = Position::from_fen("5k4/10/10/10/10/10/10/R4K3R w KQ - 0 1").expect("legal");
        let other_side =
            Position::from_fen("5k4/10/10/10/10/10/10/R4K3R b KQ - 0 1").expect("legal");
        assert_ne!(key(&base), key(&other_side), "side to move");

        let fewer_rights =
            Position::from_fen("5k4/10/10/10/10/10/10/R4K3R w K - 0 1").expect("legal");
        assert_ne!(key(&base), key(&fewer_rights), "castling rights");

        let with_ep = Position::from_fen("5k4/10/10/10/10/10/10/R4K3R w KQ c6 0 1").expect("legal");
        assert_ne!(key(&base), key(&with_ep), "en passant");
    }

    /// A position reached two different ways is one position. This is what
    /// repetition actually needs, and a key that failed it would count a
    /// repetition that never happened — or miss one that did.
    #[test]
    fn a_position_reached_two_ways_has_one_key() {
        let start = Position::starting();
        // Knights out and back, in the two orders.
        let one = start
            .make_move(Move::new(square("b1"), square("c3"), MoveKind::Quiet))
            .make_move(Move::new(square("b8"), square("c6"), MoveKind::Quiet))
            .make_move(Move::new(square("c3"), square("b1"), MoveKind::Quiet))
            .make_move(Move::new(square("c6"), square("b8"), MoveKind::Quiet));
        let two = start
            .make_move(Move::new(square("i1"), square("h3"), MoveKind::Quiet))
            .make_move(Move::new(square("i8"), square("h6"), MoveKind::Quiet))
            .make_move(Move::new(square("h3"), square("i1"), MoveKind::Quiet))
            .make_move(Move::new(square("h6"), square("i8"), MoveKind::Quiet));

        assert_eq!(key(&one), key(&two));
        // And the same as the start, which is what makes it a repetition —
        // except for the clocks, which are deliberately not in the key.
        assert_eq!(key(&one), key(&start));
    }
}
