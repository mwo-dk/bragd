# Seirawan chess

[← Rules](README.md)

**Built.** Chess plus two compound pieces that **enter from off the board**. It
fits 8×8, so it lives in `crates/rules-chess` beside chess, Chess960, shatranj
and chaturanga.

## What makes it interesting

Gating: each side holds a hawk (bishop and knight) and an elephant (rook and
knight) off the board. The first time a back-rank piece moves, its owner may drop
one of them onto the square just vacated. That is a **move category**, not a move
rule.

## Two decisions the first build left open

### A gate is written `/H`, and a castle names its square

`to_san` once returned the same `Nc3` for a plain move and for both of its gating
twins, so the move list was silent about the one decision that defines the
Variant, and a Player typing `Nc3` named three moves at once.

The suffix is the published form: XBoard's rules page for the Variant says a
gating move is entered "with a suffix `/H` or `/E`", and S-chess players already
write both letters.

**A castle names the square as well** (`O-O/He1` against `O-O/Hh1`), because it
is the one move that vacates two squares and either may take the piece. The wire
form (`e1g1H` against `e1g1h`) settles that with the *case* of the letter. That
trick is not reused in SAN: SAN is read by people, and a notation where `O-O/E`
and `O-O/e` are different moves has a trap in it.

### A pawn promotes six ways

Queen, rook, bishop, knight, hawk or elephant. Every published description says
so (chessvariants.com, musketeerchess.net, XBoard's rules page), and
Fairy-Stockfish lists all six.

**Not drawn from the hand.** No source says so, and reading it that way would
make the two compounds a supply of one each for the whole game. A player who
gated their hawk on move one may still promote a pawn to a second one, as a chess
player with a queen may promote to another.

This changed the move encoding. `MoveKind` was a full 4-bit enum (chess's twelve,
en passant, two castles and shatranj's two ferz promotions), so four more codes
meant a **five-bit kind field**, with the gate bits moving up one place. It fits
because `Move` is a `u32` and was using twenty bits. No move integer is stored
anywhere (Games keep UCI strings), so the renumbering stays inside
`chess_move.rs`.

## How it was built

1. **Widen `Move`**. Gating and gating-with-a-capture needed their own codes;
   the six promotions later pushed the kind field to five bits.
   `crates/rules-chess/src/chess_move.rs` records why.
2. **Off-board state on `Position`**: what each side still holds and which
   back-rank squares can still bring a piece in. It is part of the position key
   (`Position::in_hand`, `gate_square`, `gate_squares`).
3. **Two new `Role`s**, appended so existing indices do not move: `Role::Hawk`
   and `Role::Elephant`.
4. **Move generation, outcomes, a self-recorded perft with properties**, in
   `crates/rules-chess/src/seirawan.rs` and `crates/rules-chess/tests/seirawan.rs`.

## The hand and the gate squares are drawn

The board shows the two pieces each side still holds and every back-rank square
that can still bring one in. No squares left means they stay in hand for the rest
of the game, a fact nothing else on the board shows. In Bragd the hand comes
through `bragd-games` as the board's hands, and a gating move names the piece it
drops.

A FEN does not carry the hands, so a Seirawan Game started from a FEN has none.
Teaching positions for Seirawan are therefore lines of moves from the start.

## Gating as a move of its own

Gating could be a separate move, or a flag on the move that triggered it. The
second is fewer codes and a worse move list: a player choosing to gate is making
a decision, and a decision should be a move they can see. **Settled as a separate
move**: three moves share a from and a to, and each is offered with its own
notation.

## The artwork

A hawk is bishop and knight and an elephant is rook and knight, the same pairs as
Capablanca's archbishop and chancellor, but **neither piece reuses those
drawings**. The hawk is its own drawing: a raptor's head in profile on Cburnett's
bishop foot and collar. The elephant is **chaturanga's gaja**, shared. It was
briefly its own drawing, a rook's tower with a howdah, and at forty-four pixels it
read as a rook with a smudge, the same failure Capablanca's compounds had.

## Typing a move

`from_san` reads `=` and `/` interchangeably, so `Nf3=H` and `bxa8/H` both
resolve. That is WinBoard's behaviour, and it is safe: a gate only follows a
piece letter or a castle, and a promotion is only a pawn move, so no two legal
moves collapse onto one string. A typed castle with zeros and a gate (`0-0/Hh1`)
must be read as the castle it is.

## The check bug

`Position::attackers_to` is the one definition of "attacks" every Variant shares,
and the hawk and the elephant were once missing from it while move generation
walked them. A compound **moved and never attacked**: it gave no check, so it
could not mate; the enemy king was offered squares it covered; and the generator
offered the capture of a king, after which `to_san` asked the board for a king
that was not there and panicked.

Two of the crate's own test fixtures turned out to be illegal boards that only
parsed because of it, and the FEN reader now refuses them. Two more rule changes
came with the fix: a pawn promotes six ways instead of four, and a compound on
the board is no longer insufficient material.
