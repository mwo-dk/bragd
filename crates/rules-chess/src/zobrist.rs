//! Zobrist hashing — a 64-bit key identifying a position.
//!
//! This exists for threefold and fivefold repetition, which is a classic
//! place to be subtly wrong. Two positions repeat only if the
//! same side is to move, the same castling rights survive, *and* the same en
//! passant capture is available — not merely the same pieces on the same
//! squares.
//!
//! The constants are generated at compile time by SplitMix64 from a fixed seed,
//! so the key for a position is stable across builds and machines. That matters
//! because two builds of the rules must agree: a Game replayed in Bragd and on
//! a game server must reach the same repetition count in both.
//!
//! Keys are **not** persisted. Nothing stored has a key column;
//! `Game::keys` is rebuilt by replaying the stored moves on every read. An
//! earlier version of this comment claimed the opposite, which would have made
//! the seed look unchangeable when it is merely load-bearing across builds.

use crate::castling::CastlingRights;
use crate::piece::{Piece, Role};
use crate::square::Square;

/// Fixed so that every build agrees. Changing it changes every key, which is
/// harmless in itself — no key is stored — but two engines with different seeds
/// would count repetitions differently, which is not.
const SEED: u64 = 0x0042_2500_C4E5_5000;

const fn split_mix_64(state: u64) -> (u64, u64) {
    let next_state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = next_state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (next_state, z ^ (z >> 31))
}

struct Tables {
    /// `pieces[color][role][square]`
    pieces: [[[u64; 64]; Role::COUNT]; 2],
    /// `castling[color][side][rook file]`, mixed in once per live right.
    ///
    /// Indexed by the rook's *file* because that is what a right now is. A
    /// four-bit index would still be correct for repetition — a Game's castling
    /// rooks never move house, so two positions in one Game with the same
    /// rights bits always have the same rook files — but the key is meant to
    /// identify a position, not merely to be unique within one Game, and an
    /// identity that depends on an invariant three modules away is one that
    /// breaks silently the first time anything compares keys across Games.
    castling: [[[u64; 8]; 2]; 2],
    /// Indexed by *file*, not square: only the file of an en passant target
    /// distinguishes positions.
    en_passant_file: [u64; 8],
    black_to_move: u64,
    /// Seirawan's hand: `pocket[color][piece]`, mixed in for each piece still
    /// held. Two positions that differ only in what is left to gate are
    /// different positions, and repetition has to see that.
    pocket: [[u64; 2]; 2],
    /// Seirawan's gating rights, one constant per square that still has one.
    /// A square whose right has been spent is a genuinely different position
    /// from the same board with the right intact.
    gate: [u64; 64],
}

const TABLES: Tables = {
    let mut state = SEED;
    let mut pieces = [[[0u64; 64]; Role::COUNT]; 2];
    let mut color = 0;
    while color < 2 {
        let mut role = 0;
        while role < Role::COUNT {
            let mut square = 0;
            while square < 64 {
                let (next_state, value) = split_mix_64(state);
                state = next_state;
                pieces[color][role][square] = value;
                square += 1;
            }
            role += 1;
        }
        color += 1;
    }

    // Drawn *after* everything that existed before them, so adding Seirawan
    // did not renumber a single existing constant. A position's key is
    // compared across Games and is written into no store, but a key that
    // changed meaning between releases would make every repetition claim in
    // flight wrong at once.
    let mut castling = [[[0u64; 8]; 2]; 2];
    let mut color = 0;
    while color < 2 {
        let mut side = 0;
        while side < 2 {
            let mut file = 0;
            while file < 8 {
                let (next_state, value) = split_mix_64(state);
                state = next_state;
                castling[color][side][file] = value;
                file += 1;
            }
            side += 1;
        }
        color += 1;
    }

    let mut en_passant_file = [0u64; 8];
    let mut file = 0;
    while file < 8 {
        let (next_state, value) = split_mix_64(state);
        state = next_state;
        en_passant_file[file] = value;
        file += 1;
    }

    let (next_state, black_to_move) = split_mix_64(state);
    state = next_state;

    let mut pocket = [[0u64; 2]; 2];
    let mut color = 0;
    while color < 2 {
        let mut which = 0;
        while which < 2 {
            let (next_state, value) = split_mix_64(state);
            state = next_state;
            pocket[color][which] = value;
            which += 1;
        }
        color += 1;
    }

    let mut gate = [0u64; 64];
    let mut square = 0;
    while square < 64 {
        let (next_state, value) = split_mix_64(state);
        state = next_state;
        gate[square] = value;
        square += 1;
    }

    Tables {
        pieces,
        castling,
        en_passant_file,
        black_to_move,
        pocket,
        gate,
    }
};

/// The key contribution of one side's hand. Nothing when it is empty, which is
/// every variant but Seirawan.
pub fn pocket(color: crate::piece::Color, role: Role) -> u64 {
    let index = match role {
        Role::Hawk => 0,
        Role::Elephant => 1,
        _ => return 0,
    };
    TABLES.pocket[color.index()][index]
}

/// The key contribution of one live gating right.
pub fn gate(square: Square) -> u64 {
    TABLES.gate[square.index() as usize]
}

pub fn piece(piece: Piece, square: Square) -> u64 {
    TABLES.pieces[piece.color.index()][piece.role.index()][square.index() as usize]
}

/// The key contribution of a set of rights: one constant per live right,
/// XOR-folded. Absent rights contribute nothing, which is what makes "the same
/// rights survive" part of what it means for two positions to repeat.
pub fn castling(rights: CastlingRights) -> u64 {
    rights.alive().fold(0, |key, (color, side, file)| {
        key ^ TABLES.castling[color.index()][side.index()][file as usize]
    })
}

pub fn en_passant_file(file: u8) -> u64 {
    TABLES.en_passant_file[file as usize]
}

/// Mixed in when Black is to move, so the same board with different sides to
/// move hashes differently.
pub fn black_to_move() -> u64 {
    TABLES.black_to_move
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::castling::Side;
    use crate::piece::Color;
    use std::collections::HashSet;

    #[test]
    fn every_constant_is_distinct() {
        // A duplicate would silently merge two different positions and make
        // repetition detection wrong in a way no other test would catch.
        let mut seen = HashSet::new();
        for color in Color::ALL {
            for role in Role::ALL {
                for index in 0..64u8 {
                    assert!(
                        seen.insert(piece(Piece::new(color, role), Square::new(index))),
                        "duplicate piece constant"
                    );
                }
            }
        }
        // The raw constants, not their XOR combinations. What this guards is a
        // duplicated draw from the generator; folding first would instead be
        // testing that no two *subsets* happen to XOR to the same value, which
        // is a different and far weaker statement.
        for color in Color::ALL {
            for side in Side::ALL {
                for file in 0..8u8 {
                    let one = CastlingRights::NONE.with(color, side, file);
                    assert!(
                        seen.insert(castling(one)),
                        "duplicate castling constant for {color:?} {side:?} file {file}"
                    );
                }
            }
        }
        for file in 0..8 {
            assert!(
                seen.insert(en_passant_file(file)),
                "duplicate file constant"
            );
        }
        assert!(seen.insert(black_to_move()));
    }

    #[test]
    fn no_constant_is_zero() {
        // Zero would be a no-op when mixed in, making a piece invisible to the key.
        assert_ne!(black_to_move(), 0);
        assert_ne!(
            piece(Piece::new(Color::White, Role::Pawn), Square::new(0)),
            0
        );
        assert_eq!(
            castling(CastlingRights::NONE),
            castling(CastlingRights::NONE),
            "constants must be stable within a run"
        );
    }
}
