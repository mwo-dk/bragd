//! Shogi.
//!
//! Its own crate, and the reason is not the board size — nine by nine would fit
//! the tafl engine's shape perfectly well. It is that **captured pieces change
//! sides and come back**, which nothing else in Bragd does and which
//! reaches every part of an engine: a position carries two hands, a move is
//! either a board move or a drop, and three of the rules are about drops
//! specifically.
//!
//! Grafting that onto a crate built for games where a captured piece is gone
//! would have made every one of those games carry a hand it never uses
//! (a crate is drawn round a board; this is the first case where the line is
//! drawn somewhere else, and the reason is written down here so the next
//! reader does not think it was an accident).
#![forbid(unsafe_code)]

pub mod attacks;
pub mod board;
pub mod game;
pub mod perft;
pub mod piece;
pub mod position;
pub mod record;

pub use board::{Bitboard, Square};
pub use game::{DrawReason, Game, MoveError, Outcome, WinReason};
pub use piece::{Piece, Role, Side};
pub use position::{Move, Position};
