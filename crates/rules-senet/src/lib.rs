//! Senet.
//!
//! Its own crate for the ordinary reason — one rules crate per family — and
//! it shares nothing with the other two race games here beyond being settled by
//! throws. Backgammon and Ur are races; senet is a race with a **haunted last
//! row**, and almost everything unusual about it is in the final five houses.
//!
//! **A throw is an input, not an event.** The caller throws the sticks,
//! writes the result into the record, and passes it in. This crate has no
//! dependencies, contains no generator, and cannot grow one by accident — an RNG
//! behind this line would make a stored Game unreplayable in silence.
//!
//! **The rules are Timothy Kendall's reading, and that is part of the product.**
//! Senet was played for three thousand years and nobody wrote down how. What
//! survives is boards, paintings and a few texts that are about the afterlife
//! rather than about the game, so this engine does not implement "senet" — it
//! implements a reconstruction specific enough to be wrong, which is the only way
//! there is anything to be correct against. The contested points are
//! listed on the rules document and in `docs/rules/ur-and-senet-rules.md`, not hidden
//! in here. Two are worth knowing before reading any of this: Kendall plays
//! **seven pieces a side on houses 1–14**, not the five-on-1–10 that most popular
//! summaries attribute to him, and he has **no three-in-a-row blockade** — that
//! belongs to Jéquier.
//!
//! **Four things here exist nowhere else in `crates/`.** Nothing is ever
//! captured, only exchanged. A piece can be forced *backwards*. One house cannot
//! be passed, so a piece standing on it blockades its own side. And a piece in
//! the water freezes its owner's entire army until it escapes. [`position`] has
//! the detail on each.
//!
//! There is no perft corpus for senet and there cannot usefully be one: the tree
//! branches over five throws before it branches over moves. The gate is
//! properties instead, in `tests/properties.rs`, with the individual rules pinned
//! one at a time in `tests/rules.rs`.
#![forbid(unsafe_code)]

pub mod board;
pub mod game;
pub mod position;
pub mod sticks;

pub use board::{
    BEAUTY, COLUMNS, HOUSES, LAST, PIECES, RE_ATOUM, REBIRTH, ROWS, Side, THREE_TRUTHS, WATER,
};
pub use game::{Entry, Game, Needed, Outcome, PlayError, WinReason};
pub use position::{Move, Offer, Position, Spot, Way};
pub use sticks::{STICKS, Throw, WAYS};
