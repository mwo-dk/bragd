//! The rules of chess and the six Variants that share its board.
//!
//! This crate is the **authority** for what is and is not a legal move, in
//! Bragd and on a future game server. Two implementations of the rules would
//! eventually disagree, and the disagreement would surface as a disputed Game
//! in front of a Player.
//!
//! Two properties hold throughout and are worth stating once:
//!
//! - **It is pure.** Nothing here reads a clock, performs I/O, or allocates a
//!   random number. Times are passed in by the caller. That is what makes a
//!   Game replayable exactly, years later, from stored moves.
//! - **It has no dependencies.** It is pure Rust, so it builds anywhere and no
//!   dependency's licence binds it.
//!
//! Seven variants implement [`Variant`]: chess, Chess960, shatranj,
//! chaturanga, King of the Hill, Racing Kings and Seirawan. They are one
//! *family* and share a board, a move encoding and a definition of check.
//! Capablanca, draughts and the tafl games are separate crates rather than
//! further variants, because they do not share the board.

#![forbid(unsafe_code)]

pub mod attacks;
pub mod bitboard;
pub mod castling;
pub mod chaturanga;
pub mod chess_move;
pub mod fen;
pub mod game;
pub mod motifs;
pub mod movegen;
pub mod outcome;
pub mod perft;
pub mod piece;
pub mod position;
pub mod racing;
pub mod san;
pub mod seirawan;
pub mod shatranj;
pub mod square;
pub mod variant;
pub mod zobrist;

pub use bitboard::Bitboard;
pub use castling::{CastlingRights, Side};
pub use chess_move::{CastlingNotation, Move, MoveKind, MoveList};
pub use fen::FenError;
pub use game::{ClaimError, Game, MoveError};
pub use outcome::{DrawClaim, DrawReason, Outcome, WinReason};
pub use piece::{Color, Piece, Role};
pub use position::Position;
pub use san::{from_san, from_san_loose, to_san, to_san_chess};
pub use square::Square;
pub use variant::{
    Chess, Chess960, KingOfTheHill, RacingKings, Rules, Seirawan, Shatranj, Variant,
};
