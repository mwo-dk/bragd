# Shogi

[← Rules](README.md)

**Built**, in `crates/rules-shogi`. This page records what the two expected
blockers turned out to be worth: one was real and the other was not.

## Blocker one, which was real: a board must show a hand

The plan said the blocker was *rotation*: shogi shows ownership by turning a
piece 180°. That cost nothing. The second Side's piece is the same artwork,
rotated when the sprite is built.

The real blocker sits beside the board. **A shogi player holds pieces**, they are
not on the board, and they can be played. Every other Family answers "what can I
do" with from-and-to pairs. A drop has no square of origin at all.

What that costs:

- A legal move may be a drop, with no origin square, and names the piece dropped.
- A board needs a **second kind of selection**: a piece in hand, as well as a
  square.
- The rules crate lists the legal drops itself. A front end that filtered the
  move list on its own would be re-implementing nifu by accident.

And one thing nobody listed: **shogi spells a square the other way round on both
axes**. `9i` is the file counted from the right and the rank lettered from the
top. A board with the right grid and the wrong spelling draws perfectly and
refuses every move.

## Blocker two, which was not: the perft corpus cannot gate the hard rule

There are no checkmates before depth 7, so uchifuzume (mate by pawn drop) cannot
fire at depth 6 or less. A rules crate with no uchifuzume code at all passes the
published perft numbers for every depth anybody runs.

The answer was to **test the rule directly** rather than hope a node count would
notice it, which is what the drop tests in `crates/rules-shogi/tests/rules.rs`
do. They also carry the one property a hand-written case set cannot: *the second
Side's moves are the first Side's, reflected*, over every stepping role and every
square. In chess only the pawn cares which way it faces. In shogi the pawn,
lance, knight, silver, gold and all four gold-moving promoted pieces do, and one
mirrored direction gives a game that looks right and is not.

The published perft corpus has since been imported, in
`crates/rules-shogi/tests/perft.rs`: the shogi-l "Shogi Perft Numbers" thread and
the TalkChess table, which agree on the starting position through depth six. It
is a check on movement. It is still not the gate for uchifuzume.

## The pieces: kanji, as outlines

Kanji, because that is what players use. They are drawn as **glyph outlines
extracted from Noto Sans CJK**, not as text in a font: a machine without a
Japanese font would draw a board of empty boxes. The pieces cannot be shown as
Western symbols without a second set. That is the right trade, because a shogi
player wants kanji.

## Not built

- **The impasse rule** (jishogi): the point count when both kings reach the far
  side and neither can be attacked. Such a game runs to the move limit and is
  drawn.
- **Handicap games**, which is how shogi is normally played between unequal
  players. The rules crate takes any starting position, so this is a New game
  choice rather than a rules change.
- **Minishogi** (5×5).
- Western notation. USI is the stored form; a display notation can be added on
  top without touching it.
