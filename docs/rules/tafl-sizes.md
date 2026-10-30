# Brandubh (7×7) and tablut (9×9)

[← Rules](README.md)

**Done.** Two more tafl games on the rules crate that already plays hnefatafl,
`crates/rules-tafl`. This page is the record of what the change cost against what
it was estimated at, because the first estimate was wrong by a wide margin.

**What the plan got right:** every step below, in order. The refactor landed
first with no new game and the existing suite as the whole gate; sixty tests
passed unchanged before a new ruleset existed.

**What the plan got wrong:** it assumed the size had to become a value threaded
through everything. It did not. The *index space* stays eleven wide whatever the
board is: a square is still `rank * 11 + file`. So `file()`, `rank()` and the
`u128` bitboard were untouched, and only the five things that depend on where the
**edge** is had to learn about size: `is_edge`, `step`, the board mask, the
throne and the corners.

**What neither saw:** the board drawn on screen. The rules keep every board in an
eleven-wide index space, and a front end draws a `size × size` grid, so a flat
board has to be re-indexed before it is drawn. Using the rules' indices directly
puts a tablut board's pieces at eleven-wide spacings on a nine-wide grid, every
rank shifted further than the last: a diagonal smear rather than an obvious bug.
In Bragd, `bragd-games` does that re-indexing once.

And one thing that had nothing to do with size: the first time somebody played a
tafl board by pointer, a bug showed up. The board decided whose piece it was by
checking for a `w` prefix, which is true of no tafl or xiangqi piece.

## What the code said before the refactor

- `SIZE` was a compile-time constant. Now `board.rs` carries `MAX_SIZE = 11` and
  `SIZES = [7, 9, 11]`, and the size itself is on `Position`.
- `Square::file`, `rank`, `is_edge` and `offset` were defined in terms of it, as
  were `THRONE`, `CORNERS` and `Bitboard::BOARD`.
- `Bitboard`'s `Not` complemented within an 11×11 mask. On 7×7 that sets
  seventy-two bits that are not squares, and anything iterating afterwards walks
  them.
- **`ruleset.rs` (the actual rules) and `game.rs` never mentioned `SIZE`.** They
  touched geometry at fourteen call sites, but through functions. The rules never
  had to learn about size at all.

## The steps

1. **Make the size a value, not a constant.** `Position` carries it; the four
   `Square` methods take it; `Bitboard::BOARD` becomes a function of it, and every
   `!bitboard` becomes a masked complement. The compiler finds every site.
2. **Keep 11×11 behaviour identical and run the existing suite.**
3. **Then** a starting layout, a throne and corners derived from the size, and a
   catalogue row per game.

## What was decided

**Each size needed its own ruleset decision**, in the same shape as the decision
that hnefatafl plays the Copenhagen rules: name the reading, and say where it is
ours.

**Brandubh**: the king is armed, and there is no shieldwall and no exit fort.
Both are modern eleven-square inventions solving eleven-square problems.

**Tablut**: the same two, plus the escape. **Tablut follows Linnaeus.** His 1732
journal is the only contemporary description of any tafl game, and it is a
traveller's note rather than a rulebook. It is ambiguous exactly where it decides
the game: whether the king escapes to a corner or to any edge square. **Bragd
plays the corner escape, for playability rather than scholarship.** Under the
edge reading the king starts four squares from safety in four directions at once,
and sixteen attackers cannot cover thirty-two edge squares, so the defenders win
nearly every game. The Rules page says which reading and why.
