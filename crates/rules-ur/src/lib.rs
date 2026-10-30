//! The Royal Game of Ur.
//!
//! Its own crate for the ordinary reason — one rules crate per family. It
//! shares nothing with backgammon except that both are races settled by dice, and
//! that is a resemblance rather than a rule in common.
//!
//! **A throw is an input, not an event.** The same rule backgammon is built on,
//! and for the same reason: the rules are a pure function so that a Game can
//! be stored as a position plus a list of entries and replayed years
//! later, and dice are not a pure function. The caller throws, writes the
//! result into the record, and passes it in. This crate has no dependencies,
//! contains no generator, and cannot grow one by accident.
//!
//! **The rules are Irving Finkel's reading, and that is part of the product.**
//! The game was played for three thousand years and nobody wrote the rules down
//! until a Babylonian tablet did, partially, around 177 BC; Finkel's
//! reconstruction from BM 33333b is the one with a named argument behind it. So
//! this engine does not implement "the Royal Game of Ur" — it implements a
//! reconstruction that is specific enough to be wrong, which is the only way there
//! is anything to be correct against. The contested points are listed
//! on the rules document and in `docs/rules/ur.md`, not hidden in here.
//!
//! **The two players do not walk the same board.** Twelve of the twenty squares
//! are private, six to a side, and only the middle eight are shared. Every rule
//! that distinguishes this from a simple race is a rule about which kind of square
//! a piece is standing on: a piece can only be taken on the shared row, and not
//! even there on the rosette in the middle of it. So [`board`] writes the two
//! paths down once and everything else asks it.
//!
//! **A move is one piece, and a turn may be several moves.** Landing on a rosette
//! earns another throw, so a side can move four times before the other side moves
//! at all. Nothing above this crate needs to know that — [`Game::needs`] says
//! whose turn it is after every entry, and the platform obeys it rather than
//! assuming the sides alternate.
//!
//! There is no perft corpus for Ur and there cannot usefully be one: the tree
//! branches over five throws before it branches over moves. The correctness gate
//! is properties instead, in `tests/properties.rs`, in the same spirit as
//! backgammon's and tafl's.
#![forbid(unsafe_code)]

pub mod board;
pub mod dice;
pub mod game;
pub mod position;

pub use board::{BOARD, PATH, PIECES, ROSETTES, SAFE, SQUARES, Side, Spot, Square};
pub use dice::{DICE, Throw, WAYS};
pub use game::{Entry, Game, Needed, Outcome, PlayError, WinReason};
pub use position::{Move, Offer, Position};
