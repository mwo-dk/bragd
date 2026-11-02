# Ur rule sets

[← Rules](README.md)

Nobody wrote down how the Royal Game of Ur was played while it was played. Every rule set is a reconstruction, and Bragd plays four of them, each a Variant of the Ur Family. They live in `crates/rules-ur` as one `Rules` value each (`FINKEL`, `MASTERS`, `BLITZ`, `ASEB`): a board, two paths, a number of pieces and dice, and three switches. Nothing else differs between them.

| Variant | Key | Board | Path | Pieces | Dice | A throw of 0 | Safe squares | Another throw for |
|---|---|---|---|---|---|---|---|---|
| Ur (Finkel) | `ur` | the board found at Ur, 20 squares | Bell's, 14 squares | 7 | 4 | moves nothing | the middle rosette | a rosette |
| Ur (Masters) | `ur-masters` | the board found at Ur | Masters', 16 squares | 7 | 3 | moves 4 | none | a rosette |
| Ur (Blitz) | `ur-blitz` | the board found at Ur | Masters', 16 squares | 5 | 4 | moves nothing | none | a rosette or a capture |
| Aseb (twenty squares) | `ur-aseb` | 4 + 12 + 4 squares | Aseb's, 16 squares | 5 | 4 | moves nothing | the three shared rosettes | a rosette |

In every rule set a piece bears off only with the exact throw, a square holds one piece, landing on an enemy piece on a shared square sends it back to wait, and a throw with no legal move passes the turn.

## The dice

Each die is a tetrahedron with two of its four tips marked. A throw counts the dice that land with a marked tip pointing up, so four dice give 0 to 4 with weights 1, 4, 6, 4 and 1 out of 16. Masters' rules throw three dice (1, 3, 3, 1 out of 8), and a throw of 0 there moves four, which keeps every throw useful. Bragd throws in Rust (`bragd_core::throw`) from the operating system's random source. The record keeps only the count, so the board shows which dice landed marked tip up in a fixed spread for that count.

## The paths

Columns are counted 1 to 8 (1 to 12 on Aseb's board) from the end where pieces turn into the middle row. Each side has its own row; the middle row lies between them.

- **Bell's path** (Finkel): in along your own row from column 4 to column 1, along the whole middle row, then back into your own row at columns 8 and 7, and off. The middle row is shared. Rosettes fall at steps 4, 8 and 14.
- **Masters' path**: as Bell's to column 7 of the middle row, then across the far end: the other side's columns 7 and 8, the middle row's column 8, your own columns 8 and 7, and off. Twelve squares are shared, including the other side's last two, and a rosette falls on every fourth step.
- **Aseb's path**: in along your own four squares, then the whole middle row of twelve, and off from its end. Rosettes fall on every fourth step; three of them are on the shared row.

## Sources

| Rule set | Source |
|---|---|
| Finkel | Irving Finkel, "On the rules for the Royal Game of Ur", in *Ancient Board Games in Perspective*, ed. I. L. Finkel, British Museum Press, London, 2007. The seven-piece game with the middle rosette safe is the one Finkel played on camera with Tom Scott (British Museum, 2017), described on [Masters Traditional Games: the rules of the Royal Game of Ur](https://www.mastersofgames.com/rules/royal-ur-rules.htm) and [RoyalUr.net: rules](https://royalur.net/rules). |
| Bell's path | R. C. Bell, *Board and Table Games from Many Civilizations*, revised edition, Dover, New York, 1979. Finkel's game walks it. |
| Masters | James Masters, [The Royal Game of Ur and the Game of 20 Squares](https://www.tradgames.org.uk/games/Royal-Game-Ur.htm), tradgames.org.uk: the 16-square path with a rosette every fourth square, and three binary lots giving 0 to 3. The full rule set (0 moves four, rosettes not safe, exact throw to bear off) as written on [Masters Traditional Games](https://www.mastersofgames.com/rules/royal-ur-rules.htm) and [RoyalUr.net](https://royalur.net/rules). |
| Blitz | [RoyalUr.net: rules](https://royalur.net/rules), RoyalUr.net's own rule set for shorter games: Masters' path, five pieces, no safe rosettes, and a capture earns another throw. |
| Aseb | W. Crist, A. E. Dunn-Vaturi and A. de Voogt, *Ancient Egyptians at Play: Board Games Across Borders*, Bloomsbury Academic, London, 2016, for the board of twenty squares and its path. The pieces, dice and rosette rules as RoyalUr.net plays it ([RoyalUr.net: rules](https://royalur.net/rules)). |

## Left out

| Rule set | Why not |
|---|---|
| R. C. Bell's own rules | His path is Finkel's, so it is here. His scoring needs a throw of 5 to enter and to re-enter, and a fine paid into a pool on each rosette. Bragd plays no stakes, so the rosettes would mean nothing, and the piece count is not in a source we could check. |
| The Tournament Engine (société internationale d'UR, playur.org) | Pieces stack on rosettes and move together, and pieces may move backwards. A square holding a stack is a different position model, not a switch on this one. |
| Murray's path | H. J. R. Murray (*A History of Board-games Other Than Chess*, 1952) gives a path of 27 squares through the middle row and back, but no rule set to play it with. |
| Skiriuk's path | A 2021 blog post with a path and no published rule set behind it. |

## Bragd's Bot

Ur has no outside Engine, so Bragd's own Bot plays it (`rules_ur::bot`). For each legal move it weighs every next throw by how likely it is, assumes whoever throws picks their best reply, and counts how far each side's pieces have come, with a little extra for a piece home or on a safe square. It asks the rule set everything, so it plays all four Variants. The same position and throw always give the same move.

## Questions

#### Why is Finkel's the default?

It is the British Museum's game and the one most players know. The others are offered beside it, not instead of it.

#### Why does Masters throw three dice?

Masters reads the game as played with three binary lots. Three dice can show no marked tip one time in eight, so 0 moves four rather than wasting the turn.

#### Is the tablet from 177 BC the source of Finkel's rules?

Not directly. The tablet (BM 33333B) describes a later game of twenty squares with betting. Finkel's playable rules draw on it but are his own reconstruction, as RoyalUr.net and Masters both point out.
