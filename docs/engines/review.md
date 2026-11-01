[← README](../../README.md) · [Docs index](../README.md) · [Engines](README.md)

# Game review

A review runs an Engine over every position of a Game in the background and annotates each move. A move that lost ground gets a mark such as `?` and the better line; a strong move gets `!`. In chess, the review also names what the move did: a fork, a pin, a piece left hanging or a missed mate. The marks show after each move in the move list, and the Annotations are listed in Analyse. In F1 this is the `annotations` entry.

![Analyse for chess with a reviewed Game: Review the game with Reviewed by the Engine beside it, the Annotations listed under it, such as Qg5?? Blunder. It leaves a piece hanging, and the move list with ?? after that move](../screenshots/review-chess-1100.png)
*A reviewed Game: its Annotations, and marks in the move list.*

## How to reach it

| Where | What to do |
|---|---|
| Sidebar | A Family with an Engine → **Analyse** |
| On the page | Play or set up the Game, then **Review the game**. The line beside it counts *Reviewing: 12 of 40 moves*, then says *Reviewed by* the Engine |
| Annotations | Listed under the button as they come, such as *14. Bxf7?? Blunder. It leaves a piece hanging. Better was Nc3 d6 Be2.* |
| Move list | A mark after the move. Point at it to read what it means |

The Engine is the one chosen in Analyse's list, or the Variant's default.

## The Position store

Each position's Evaluation comes from the [Position store](../knowledge/position-store.md) when the Engine has judged it before, in any Game or in Analyse. Only the other positions are sent to the Engine, and each result is kept in the store with the Engine's Edition. The Annotations are kept there too. So a Game reviewed twice, or a Game that shares its opening with one reviewed before, is done at once for the shared positions.

## Annotations

### Marks

A mark comes from the Evaluations before and after the move, read from the mover's side. The amounts depend on how the Engine scores:

| Score | Families | `?!` Inaccuracy | `?` Mistake | `??` Blunder | Already won |
|---|---|---|---|---|---|
| Centipawns and mates | Chess, xiangqi, shogi, draughts | lost 0.50 pawns | 1.00 | 3.00 | 3.00 |
| Chance to win | Go | lost 5 percentage points | 10 | 20 | 90 % |
| Equity | Backgammon | lost 0.04 | 0.08 | 0.16 | 1.00 |

| Mark | Means | When |
|---|---|---|
| `!` | Good move | The Engine's own move, played, where its second choice is worse by at least the `?` amount |
| `!!` | Brilliant move | The same, by at least the `??` amount |

Centipawns count only up to 10.00 pawns either way, and a mate counts as 10.00, so a move in a won position is not a blunder for winning a little less. `!` and `!!` are not given when the mover was already winning by the last column's amount. They need the Engine's second choice, so a position taken from the store without it gets no `!`.

### The better line

For a move that is not the Engine's own, the review keeps the Engine's line from the position before. It is shown for a move marked `?!`, `?` or `??`, or one that missed a mate.

### Named motifs

For chess and Chess960:

| Motif | Shown as | When |
|---|---|---|
| Fork | *It forks two pieces.* | The moved piece attacks two or more of the other side's pieces, each the king, worth more than it, or not defended |
| Pin | *It pins a piece.* | The moved bishop, rook or queen attacks a piece that stands in front of the king, or of a piece worth more than both it and the moved piece, on the same line |
| Hangs a piece | *It leaves a piece hanging.* | After a move marked `?!`, `?` or `??`, a piece of the mover's is attacked and either not defended or attacked by a piece worth less, and it was not so before |
| Missed mate | *It misses a forced mate.* | The mover had a forced mate, played another move than the Engine's, and has no forced mate after it |

Piece values: pawn 1, knight 3, bishop 3, rook 5, queen 9.

## What it needs

- An Engine for the Variant ([The Engines page](engines.md)).
- At least one move in the Game.

## Settings and config.toml

| Setting | `config.toml` key | Default | What it does |
|---|---|---|---|
| Engine for each variant | `engines.default` | automatic | The Engine a review uses unless Analyse's list names another |

## Questions

#### How long does a review take?

About 300 ms for each position the store does not have yet, so a new 40-move Game (80 positions) takes about half a minute.

#### Does reviewing the same Game twice take as long?

No. Every position the Engine judged is in the [Position store](../knowledge/position-store.md), so the second review takes nothing from the Engine. Positions another Game reached are not judged again either.

#### Why is a move marked in one Game and not in another?

The mark is kept for the move from that position, by the Engine's Edition. A newer Edition of the Engine, or a deeper Evaluation, can judge it differently, and the newest Edition's Annotation is the one shown.

#### Why are motifs named only in chess?

They are worked out from the board, and so far only chess has them. Every Family with an Engine gets marks and better lines.

#### Why can I not review a backgammon Game?

GNU Backgammon is not set up for reviews in Bragd yet. The button is not shown for backgammon.

#### Where are my reviews from before the Position store?

They were kept per Game in the cache folder and were not carried over. Review the Game again.

#### Where is the evaluation graph?

The evaluation profile shows on the Databases page, beside a Game whose file carries one ([Foreign databases](../knowledge/foreign-databases.md)). Analyse shows the marks in the move list.

---
[← Previous: Analyse](analyse.md) · [Next: Tablebases →](tablebases.md)
