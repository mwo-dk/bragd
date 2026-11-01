//! Draughts and checkers.
//!
//! **International 10×10 is the engine**, and the 8×8 variants — English,
//! Russian, Brazilian — fall out as flag combinations from the same generator.
//! That is not generality for its own sake: the variants disagree about four
//! specific things, each of which is a boolean or a small enum, and the
//! disagreements are the reason "checkers" cannot be offered as one game.
//!
//! One genuinely new idea lives here that no other engine in Bragd has: **a
//! capture is a sequence**, so a move is not a pair of squares. What makes it
//! tractable is that the sequence's *path* does not survive the move — only
//! where the piece started, where it ended, and what it took. Two paths with
//! the same three are the same move, which is also exactly what the published
//! perft tables mean by "duplicates removed".
//!
//! Turkish draughts is deliberately **not** here. It is orthogonal rather than
//! diagonal, plays on all sixty-four squares, and removes captured pieces
//! immediately rather than at the end of the turn — three differences that
//! reach the bottom of the engine. It is a different family.
#![forbid(unsafe_code)]

pub mod board;
pub mod game;
pub mod position;
pub mod record;
pub mod ruleset;

pub use board::{Bitboard, Direction, Geometry, Square};
pub use game::{DrawReason, Game, MoveError, Outcome, STALL_LIMIT, WinReason};
pub use position::{Move, Position, Side};
pub use ruleset::{BRAZILIAN, CaptureChoice, ENGLISH, INTERNATIONAL, Promotion, RUSSIAN, Ruleset};
