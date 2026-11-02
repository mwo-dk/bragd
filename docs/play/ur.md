[← README](../../README.md) · [Docs index](../README.md) · [Play](README.md)

# Ur

The Royal Game of Ur, in four rule sets: Irving Finkel's reconstruction, James Masters', RoyalUr.net's Blitz, and Aseb on its own board of twenty squares. Each Side races its pieces along a path and off the board. Squares both Sides walk are shared, and landing on an enemy piece there sends it back to the start. Bragd throws the pyramid dice for you and shows how each one landed.

![Ur (Finkel) in Play: the twenty-square board with its five rosettes and both dashed paths, pieces on the way, and the four pyramid dice of the throw beside the turn line](../screenshots/play-ur-1100.png)

*The Royal Game of Ur: Light at the bottom, Dark at the top.*

| Variant | Key | Pieces | Dice | Path | Sides |
|---|---|---|---|---|---|
| Ur (Finkel) | `ur` | 7 | 4 | 14 squares | Light (moves first), Dark |
| Ur (Masters) | `ur-masters` | 7 | 3, and 0 moves four | 16 squares | Light, Dark |
| Ur (Blitz) | `ur-blitz` | 5 | 4 | 16 squares | Light, Dark |
| Aseb (twenty squares) | `ur-aseb` | 5 | 4 | 16 squares | Light, Dark |

Each rule set, its path and its sources: [Ur rule sets](../rules/ur.md). The board draws both Sides' paths as dashed lines, each ending in an arrow.

![Ur (Masters) in Play: both dashed paths cross the far end of the board, and three pyramid dice beside the turn line show two marked tips up](../screenshots/play-ur-masters-1100.png)

*Masters' rules: three dice, and a path of sixteen squares through the far end.*

![Aseb in Play at phone width: four squares to each Side and a middle row of twelve, with four pyramid dice](../screenshots/play-ur-aseb-390.png)

*Aseb on its own board of twenty squares. Blitz: [1100 px](../screenshots/play-ur-blitz-1100.png), [390 px](../screenshots/play-ur-blitz-390.png).*

## How to reach it

Sidebar → **Ur** → **Play**, then pick the rule set under **Variant**. Ur has only **Play**, **Databases** and **Rules** in the sidebar, because it has no outside Engine.

## What it needs

Nothing. Ur has no outside Engine. Bragd's own Bot plays every Ur rule set: set **Opponent** to *Against an Engine* and pick *Bragd's Bot* under **Engine**.

## Controls

| What you want | What you do |
|---|---|
| Throw | **Throw**, under the turn line. The pyramid dice tumble in, a marked tip up glows, and *Thrown: n* shows the steps |
| Play against Bragd's Bot | **Opponent** → *Against an Engine*, **Engine** → *Bragd's Bot*, and pick the Side it plays |
| Enter a piece | Click your tray of waiting pieces, then the square the throw reaches |
| Move a piece | Click it, then the square the throw reaches; or drag it |
| Bear off | Click the piece, then your *Off the board* tray, which is marked when bearing off is legal |
| Or pick the move | Click it under *Legal moves*. Every legal move for the throw is listed |

A throw with no legal move passes the turn by itself, and *No move with n* says so. A throw with one legal move plays it by itself after a short pause ([Forced moves](against-yourself.md#forced-moves)). Board colours: [Board themes](../look/board-themes.md).

## Settings and config.toml

| Setting | `config.toml` key | Default | What it does |
|---|---|---|---|
| The Family on or off | `families.family` | On | `ur = false` under `[families.family]` hides it |
| A rule set on or off | `families.variant` | On | `"ur-blitz" = false` under `[families.variant]` hides Blitz from Play and New game |
| Play forced moves by themselves | `play.forced` | On | A throw that leaves one legal move plays it by itself |

## Questions

#### How are the dice thrown?

Tetrahedral dice, each with two marked tips of four. The throw is the number of dice that land with a marked tip up: 0 to 4 with four dice, and 2 is the most likely throw. Masters' rules throw three dice, and a 0 moves four. Each die on screen shows whether its marked tip is up. The record keeps only the count, so which dice show it is a fixed spread.

#### What do the rosettes do?

Landing on a rosette gives you another throw at once, and this can chain. In Finkel's rules the rosette in the middle of the shared row is safe: a piece on it cannot be taken, and the other Side cannot land there. Aseb keeps its three shared rosettes safe. Masters and Blitz have no safe square, and in Blitz a capture also earns another throw.

#### Why can I not bear this piece off?

Bearing off needs the exact throw: one more than the squares left on the path. A larger throw cannot be spent on that piece.

#### Why can I not land on my own piece?

A square holds one piece. A move onto your own piece is not legal, so it is not offered.

#### Why does Ur differ from the rules I know?

Every Ur rule set is a reconstruction. Bragd plays four, cites the source of each and says which it left out in [Ur rule sets](../rules/ur.md). The points where sources disagree on Finkel's are in [Ur and senet](../rules/ur-and-senet-rules.md).

#### How does Bragd's Bot choose?

It looks one throw ahead: for each move it weighs every next throw by how likely it is and expects the best reply. It plays the same move for the same position and throw.

---
[← Previous: Backgammon](backgammon.md) · [Next: Senet →](senet.md)
