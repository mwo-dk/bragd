//! Go.
//!
//! Its own crate, and not because the board is bigger. Four things Bragd
//! had assumed four times over stop being true here, and each of them reaches
//! all the way through an engine:
//!
//! **Three hundred and sixty-one points do not fit in a `u128`.** Every other
//! board here answered size with a wider integer and that answer runs out at a
//! hundred and twenty-eight. The board is an array of points and the questions
//! a bitboard made cheap are a flood fill instead ([`board`]).
//!
//! **A move is a placement.** There is no origin, nothing slides, and nothing
//! is ever taken by landing on it: stones come off when the last empty point
//! beside them is filled, which can be a stone at the far end of a chain of
//! forty ([`position`]).
//!
//! **Passing is a move**, and two of them in a row end the play.
//!
//! **Ko is not a draw claim.** It is a repetition rule enforced inside move
//! generation — a ko-forbidden point is not somewhere a stone may be put —
//! where every other repetition rule in Bragd is something that happens to
//! a game after the fact.
//!
//! And one thing that is not a rule at all: **the ending is a negotiation.**
//! Two passes stop the play and leave the result waiting on the two players
//! agreeing which stones are dead. This crate takes that set as an input and
//! counts ([`score`]); it never guesses. The offering, countering and agreeing
//! is a protocol, it is in [`game::Entry`], and it is **part of the record** —
//! the result depends on the agreed set, so a game whose record lacks it cannot
//! be replayed to its own outcome.
//!
//! No dependencies, and no clock: the engine is a pure function of what it is
//! handed.
#![forbid(unsafe_code)]

pub mod board;
pub mod game;
pub mod position;
pub mod ruleset;
pub mod score;

pub use board::{Board, Chain, Colour, MAX_SIZE, Point, SIZES, plays};
pub use game::{Available, Entry, Game, MoveError, Outcome, Proposal, State};
pub use position::{Illegal, Move, Position};
pub use ruleset::{Repetition, Ruleset, Scoring};
pub use score::Score;
