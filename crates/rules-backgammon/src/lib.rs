//! Backgammon.
//!
//! Its own crate for the ordinary reason — one rules crate per family, and
//! this family shares nothing with chess below "two people take turns". It is
//! also the one engine here that **breaks a rule the rest of Bragd is built
//! on**, and that belongs at the top rather than buried in `turns_for`.
//!
//! **A roll is an input, not an event.** The rules are a pure function so
//! that a Game can be stored as a position plus a list of moves and
//! replayed years later. Dice are not a pure function, so the dice are not
//! generated here: the caller throws them, writes them into the record
//! beside the turn, and passes them in ([`Position::turns_for`] takes a
//! [`Roll`]). This crate has no dependencies, contains no generator, and cannot
//! grow one by accident. The wrong answer — an RNG in the rules — makes a
//! stored Game unreplayable *silently*: nothing fails, and the damage appears
//! the first time somebody opens an old game and sees a different one.
//!
//! **A turn is not a move.** Two dice give up to two moves and a double gives
//! four, and the whole turn is legal or illegal together. You must use as many
//! dice as any legal sequence can, and when only one can be used it must be the
//! larger — so a move that is fine on its own is against the rules if playing it
//! strands the other die. Generation therefore produces whole [`Turn`]s;
//! [`Position::plays_for_die`] exists for drawing a board and is documented as
//! not being legality.
//!
//! **The doubling cube is a second game.** It is state that is not the board, it
//! changes hands, it multiplies the score, and a game that ends by a declined
//! double did not end by a move. It lives in [`Cube`], separately, for that
//! reason.
//!
//! **The record is a list of entries, not of moves, and the roll is one of
//! them.** `5-3`, then `13/8 8/5`, then `6-1`, then `24/18 18/17` — separate
//! entries, because a player has to *see* the roll before choosing a turn and
//! the only alternative place to hold it between those two moments is a field
//! outside the record. That field is the thing to avoid: it vanishes
//! when a game is abandoned mid-turn and it is a second source of truth for the
//! dice. As entries the dice are written down the instant they exist, and
//! [`Game::replay`] is exact over the whole thing. [`Game::needs`] is what tells
//! the caller when to throw.
//!
//! There is no perft corpus for backgammon and there cannot usefully be one —
//! the branching factor is over a roll as well as over a position. The
//! correctness gate is properties instead, in `tests/properties.rs`, in the same
//! spirit as tafl's and for a different reason.
#![forbid(unsafe_code)]

pub mod board;
pub mod cube;
pub mod dice;
pub mod game;
pub mod position;
pub mod turn;

pub use board::{BAR_POINT, CHECKERS, HOME, POINTS, Point, Side};
pub use cube::{Cube, CubeError, MAX_CUBE};
pub use dice::Roll;
pub use game::{Concession, Entry, Game, Needed, Outcome, PlayError, WinReason};
pub use position::{Continuation, Layout, Position};
pub use turn::{Destination, NO_PLAY, Origin, Play, Turn};
