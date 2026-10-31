//! Capablanca chess — ten files by eight ranks.
//!
//! Its own engine, and the reason is one number: **eighty squares do not fit in
//! a `u64`**. `crates/rules-chess` is bitboards all the way down, and every one of
//! them is sixty-four bits wide; widening that type would rewrite the crate
//! carrying chess's published-perft gate in order to add a game that gate says
//! nothing about. Two extra *pieces* were cheap there. Two extra
//! *files* are not the same kind of change.
//!
//! So this follows `crates/rules-xiangqi`, which met the same wall at ninety points
//! and answered it with a `u128`. Same answer, deliberately, rather than a
//! second invention.
//!
//! What the game adds beyond a wider board:
//!
//! - **The archbishop** — bishop plus knight — and the **chancellor** — rook
//!   plus knight. Both are compound pieces: no new movement rule, just the
//!   union of two that already exist, which is what makes them cheap.
//! - **Castling over a ten-file rank**, where "the king moves two squares" stops
//!   being a usable description.
//!
//! Everything else is chess: the pawns, the promotion, the check, the endings.
#![forbid(unsafe_code)]

pub mod attacks;
pub mod board;
pub mod chess_move;
pub mod game;
pub mod outcome;
pub mod piece;
pub mod position;
pub mod zobrist;

pub use board::{Bitboard, FILES, RANKS, SQUARES, Square};
pub use chess_move::{Move, MoveKind, MoveList};
pub use game::{DrawClaim, Game, MoveError};
pub use outcome::{DrawReason, Outcome, WinReason};
pub use piece::{Color, Piece, Role};
pub use position::Position;
