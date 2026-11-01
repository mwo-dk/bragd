[← README](../../README.md) · [Docs index](../README.md)

# Play

Every Variant of every Game family can be played on the board in Bragd: against yourself, or against an Engine where the Family has one. Rust holds the rules. The board offers only the moves the rules crate says are legal, so an illegal move cannot be made. Chess can also be played on Lichess from *Play → Play online* ([Playing online](../online/playing-online.md)).

| Page | What it covers |
|---|---|
| [Playing against yourself](against-yourself.md) | New game, moving, Undo, flipping, the result |
| [Playing against an Engine](against-an-engine.md) | Choosing the Engine's Side, strength, time per move, book moves |
| [Chess and its Variants](chess.md) | Promotion, castling, Chess960, Seirawan gating, Capablanca |
| [Xiangqi and janggi](xiangqi.md) | The palace, the river, janggi's passes |
| [Shogi](shogi.md) | Drops, promotion, the hands |
| [Go](go.md) | Placing stones, passing, marking dead stones, scoring |
| [Draughts](draughts.md) | Captures with several paths, crowning, the four rulesets |
| [Tafl](tafl.md) | Attackers and Defenders, the king, the four sizes |
| [Backgammon](backgammon.md) | Throwing the dice, whole turns, the bar, bearing off, the doubling cube |
| [Ur](ur.md) | Throwing four dice, entering, rosettes |
| [Senet](senet.md) | Throwing the sticks, the special squares, bearing off |

## Controls every Family shares

The bar above the board is the same for every Family. Open it from the sidebar: a Family, then **Play**.

| What you want | What you do |
|---|---|
| Choose the Variant | The Variant list, first in the bar. Choosing one starts a new Game of it. Only Variants that are on show ([Game families and Variants](../getting-started/game-families.md)) |
| Start again | **New game**. The banner at the end of a Game has its own **New game** button |
| Take back a move | **Undo** drops the last entry of the record, a throw of the dice included. Against an Engine it takes back the Engine's reply too |
| Turn the board round | **Flip board**. The trays of pieces in hand swap sides with it |
| Move a piece | Click it, then click a cell it can go to. Or drag it there and let go |
| Use the keyboard | **Tab** moves from cell to cell; **Enter** on a focused cell acts as a click |
| Pick between moves that end on the same cell | A list, *Which move?*, shows each by its notation. **Cancel** closes it |
| See the Game so far | *Moves*, beside the board (under it in a narrow window), in the Variant's own notation |

The line above the move list says whose turn it is, by the Variant's names for its Sides: White and Black, Red and Black, Cho and Han, Sente and Gote, Attackers and Defenders, Light and Dark, Evens and Odds.

---
[← Previous: Tips](../getting-started/tips.md) · [Next: Playing against yourself →](against-yourself.md)
