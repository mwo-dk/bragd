# Capablanca chess

[← Rules](README.md)

Ten files by eight ranks, with two compound pieces. **Built**, in
`crates/rules-capablanca`. In Bragd it is a Variant of the Chess family. This
page records what was decided and where the estimate was wrong.

## Decided and built

- **Its own rules crate**, because eighty squares do not fit in a `u64`, and
  widening `crates/rules-chess` would rewrite the type that carries chess's perft
  gate. Xiangqi hit the same wall at ninety points, with the same answer: `u128`.
- Opening array `RNABQKBCNR`: archbishop on c, chancellor on h, king on f.
- **Castling moves the king three squares**, not two: f1 to i1 with the rook to
  h1, or f1 to c1 with the rook to d1. Two would leave the king on h1 with a rook
  still on j1 and a gap between them.
- Six promotion choices, en passant, FEN with two-digit empty runs, outcomes.
- Artwork composed from Cburnett's bishop, rook and knight.

## A correction, before anybody estimates something like it

The plan said the remaining work was wiring. **It understated it**, because it
was written without reading what a Game needs beyond a position. Chess's `Game`
(`crates/rules-chess/src/game.rs`) carries:

- **The move history and one Zobrist key per position**, which is what
  threefold repetition is counted from. Capablanca had no Zobrist table, so that
  was new work rather than a port.
- **SAN**, for the move list.
- **Draw claims**, the fifty-move rule, resignation, flagging, and the error
  types for refused moves and claims.

So the order was: **a Game layer first, then the wiring.**

## How each step was answered

1. **A `Game` for Capablanca**: done, in `crates/rules-capablanca/src/game.rs`
   and `zobrist.rs`. History, keys, repetition, the move-count rule, resignation
   and flagging, with a random number per piece per square (8 roles by 80
   squares), plus side to move, castling and en passant. Keys live in memory only
   and are never stored, so changing the table is harmless.
2. **SAN, or a decision not to have it**: decided the second way. Ten files means
   the disambiguation rules have more files to disambiguate across, and a move
   list that is subtly wrong is worse than one that is verbose. Capablanca moves
   are shown in long algebraic form (`e2e4`), and the Rules page says so.

## What gates it

The **weakest gate of the rules crates**, and its test file says so at the top.
Shatranj could argue that every piece moves by a rule chess's published perft
already exercises; this crate shares no code with that one. The counts are
self-recorded. The weight is on a property: no move may join two squares no piece
here could join. A ten-file board invites arithmetic that wraps off the j-file
onto the a-file, which looks perfectly legal.

## The one open question, and its answer

Nothing structural was in doubt. The open question was whether the composed
compound pieces read at board size on a small screen. It was first recorded as
yes. **It was no, and both pieces were redrawn.** The archbishop had been a whole
bishop with a small horse inside its mitre, and the chancellor a rook with a horse
in the doorway: legible at two hundred pixels, and at forty-four pixels a bishop
with a smudge and a rook with a smudge.

The convention now is a **base graft**, the knight's head on the other piece's
foot. It is close to universal: Unicode's chess symbols, the Wikimedia set this
artwork descends from, chess.com and merida all use it.

The failure is worth remembering: the question was asked, answered from a picture
at the wrong size, and written down as settled.
