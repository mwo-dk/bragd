//! Hnefatafl and the other tafl games.
//!
//! **A separate crate from chess, deliberately**: one rules crate per game
//! family. The two share almost nothing below "two people take turns":
//!
//! - **There are no colours, only sides**, and they are not symmetric.
//!   Attackers outnumber defenders two to one, start on the edges, and win by
//!   capturing the king. Defenders start in the centre and win by getting him
//!   out. Neither side has the other's pieces, the other's objective, or the
//!   other's starting shape.
//! - **Every piece moves the same way** — orthogonally, any distance, like a
//!   rook. There are no diagonals anywhere in the game.
//! - **Capture is by custody**, not by displacement: a piece is taken by being
//!   sandwiched, and the capturing piece never enters its square. One move can
//!   take several pieces, none of them where the mover landed.
//! - **There is no notation anybody agreed on**, no promotion, and no castling.
//!
//! A shared abstraction over that and chess would be "two people take turns"
//! plus a pile of optional fields, which is why there is not one.
#![forbid(unsafe_code)]

pub mod board;
pub mod game;
pub mod position;
pub mod ruleset;

pub use board::{Bitboard, Direction, Square};
pub use game::{AttackerWin, DefenderWin, DrawReason, Game, MoveError, Outcome, PLY_CAP};
pub use position::{Move, Piece, Position, Role, Side, corners, throne};
pub use ruleset::{BRANDUBH, COPENHAGEN, Escape, FETLAR, KingCapture, Ruleset, Shieldwall, TABLUT};
