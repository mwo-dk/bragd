//! Xiangqi and janggi — Chinese chess and Korean chess.
//!
//! Its own engine, and the reasons reach the bottom of it: pieces stand on
//! ninety **intersections** rather than sixty-four squares, the palaces
//! constrain what may move where, the cannon leaps a screen, and — in xiangqi —
//! the river divides the board and the two generals may not face each other
//! down an open file.
//!
//! Two things in it are unlike anything in the other engines here:
//!
//! - **Attacks are not symmetric.** A horse is blocked at its orthogonal step —
//!   "hobbling the horse's leg" — so whether A attacks B depends on what is
//!   standing beside A, and B attacking A does not follow.
//! - **A stalemated player loses.** As in draughts and tafl, and unlike chess.
//!   Janggi is the exception even to that, because janggi lets you pass.
//!
//! # Why two games in one crate
//!
//! There is one rules crate per *board*, not one per game, which is why the tafl crate holds
//! hnefatafl, brandubh and tablut at three sizes. Janggi is played on this
//! board: nine files by ten ranks, pieces on the intersections, a three-by-three
//! palace at each end, characters on discs. Everything the renderer, the wire
//! format, the notation and the perft harness had to learn for xiangqi is
//! exactly what janggi needs, and a second crate would have duplicated all of
//! it to express a different set of *rules* — which is what a [`Ruleset`] is
//! for.
//!
//! What janggi does **not** share is how anything moves. Of the seven pieces
//! only the horse survives the crossing unchanged. The differences are listed
//! where they are enforced, in [`position`] and [`game`]; the four that will
//! surprise a xiangqi player most are the elephant's 2-3 leap, the palace
//! diagonals being lines that any piece may travel, a cannon that cannot move
//! at all without a screen, and a legal pass.
#![forbid(unsafe_code)]

pub mod board;
pub mod game;
pub mod position;
pub mod ruleset;

pub use board::{Bitboard, Point};
pub use game::{DrawReason, Game, MoveError, Outcome, QUIET_LIMIT, REPETITIONS, WinReason};
pub use position::{Move, Piece, Position, Role, START, Side};
pub use ruleset::{HANDICAP, JANGGI_START, Ruleset, Score, Setup, Wing};
