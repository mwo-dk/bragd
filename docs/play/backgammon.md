[← README](../../README.md) · [Docs index](../README.md) · [Play](README.md)

# Backgammon

Backgammon with two dice and a doubling cube. Each Side races its fifteen checkers round the 24 points and bears them off. A lone checker hit by the other Side goes to the bar and must come in again. Bragd throws the dice for you; the throw is an entry in the record, so a Game replays to the same position.

![Backgammon in Play: the 24 points with the bar in the middle, the dice and the cube on the board, Throw under the turn line and the list of legal moves](../screenshots/play-backgammon-1100.png)

*Backgammon: the board, the dice and the cube.*

| Variant | Key | Sides |
|---|---|---|
| Backgammon | `backgammon` | White, Black |

## How to reach it

Sidebar → **Backgammon** → **Play**. Backgammon has Play, Analyse, Databases, Engines and Rules in the sidebar.

## What it needs

Nothing to play against yourself. Against an Engine: GNU Backgammon, installed or from its image ([Playing against an Engine](against-an-engine.md)).

## Controls

| What you want | What you do |
|---|---|
| Throw the dice | **Throw**, under the turn line. The opening throw is one die for each Side: the higher die moves first and plays both dice, and a tie is thrown again, so the first turn is never a double. A small checker under each die shows whose it is, and the turn line says who starts, such as *White starts: 4 against 2* |
| Double | **Double**, before you throw. It shows only when you may double: you hold the cube or it is in the middle, and the opening throw is over. Analyse has no Double, as there is no one to double or resign to |
| Answer a double | **Take** keeps playing at twice the stake. **Drop** gives up the Game at the stake before the double |
| Move a checker | Click its point, then a marked point (or *off*), or drag it there. The checker moves at once and uses one die, which turns grey. Move the next the same way; when the dice are used, the turn is played |
| Resign | **Resign…**, then the result you give: a single game, a gammon or a backgammon. Only against an Engine, on your turn |
| Take a checker back | **Undo**, before the last die is used. After the turn is played, Undo takes back the whole turn |
| Or pick the whole turn | Click it under *Legal moves*, before you move a checker. Every legal turn for the dice is listed |
| Come in from the bar | Click the bar, then the point to enter on |
| Bear off | Click the checker, then the tray at the right of the board. White bears off into the lower tray, Black into the upper one |

Only checkers that leave a legal way to finish the turn are marked: you must use as many dice as you can, and the larger die when only one fits. Board colours: [Board themes](../look/board-themes.md).

## Point numbers

Each player counts the points from their own side, as backgammon notation does: your home board is 1 to 6, and your back checkers start on your 24-point. The numbers on the board follow the Side to move, so they always match the moves under *Legal moves*. With White to move, White's back checkers stand on 24; when Black is to move, the numbers turn round and Black's back checkers stand on 24. Once the Game is over the board shows White's numbers.

## Settings and config.toml

| Setting | `config.toml` key | Default | What it does |
|---|---|---|---|
| The Family on or off | `families.family` | On | `backgammon = false` under `[families.family]` hides it |
| Default Engine | `engines.default` | First installed | `backgammon = "gnubg"` |

## Questions

#### Why is a point I want not marked?

What is legal depends on both dice together: you must use as many as any sequence allows. A checker move that would leave the other die unplayable is not marked, even if it fits one die. The rules crate checks every whole turn, and the board offers only checkers that start one. See [Backgammon](../rules/backgammon.md).

#### Is the record written one die at a time?

No. The record holds whole turns, such as `13/8 13/7`, written once the dice are used. Checkers moved before that are not saved yet.

#### Who moves first?

The Side that throws the higher die on the opening throw. Bragd throws one die for White and one for Black, again on a tie, and the winner plays those two dice as the first turn. That is why the first turn is never a double. Each die shows a small checker of its Side under it until the first turn is played.

#### Why did the point numbers change?

They follow the Side to move. A move such as `13/8` counts from the mover's side, so Black's `13/8` starts on the point White calls 12. Turning the numbers round keeps the board and the moves in one numbering.

#### Why is there no Throw button after I press Double?

The other Side owes an answer first: **Take** or **Drop**. Then the doubler throws.

#### How many points is a win?

One for a single game, two for a gammon, three for a backgammon, each times the cube. The banner says how many, such as *2 points*.

#### Are the dice fair?

Yes. Each die comes from the operating system's random source, one byte at a time. A byte from the top four values (252 to 255) is thrown away and another taken, so every face is exactly as likely as the others. Ur's dice and Senet's sticks come from the same source.

#### Does the Engine double?

No. GNU Backgammon as an Engine Side always takes a double and never offers one. Backgammon has no Game review yet.

#### Can I resign?

Yes, against an Engine, on your turn. **Resign…** under the turn line asks which result you give: a single game, a gammon or a backgammon. Against yourself there is no one to resign to, so it does not show. You lose one, two or three points, times the cube. The Game ends at once, and the banner shows the points.

---
[← Previous: Tafl](tafl.md) · [Next: Ur →](ur.md)
