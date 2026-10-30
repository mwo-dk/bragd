[← README](../../README.md) · [Docs index](../README.md) · [Look](README.md)

# Board themes

A Board theme is the four colours of every board: light square, dark square, highlight and marker. It is chosen apart from the [Theme](themes.md), or set to *Match theme* to take its colours from the Theme. It never changes how a board is built: the squares, points and lines belong to the Game family. Racing green is the default.

## Contents

- [The Board themes](#the-board-themes)
- [How to reach it](#how-to-reach-it)
- [What it needs](#what-it-needs)
- [Settings and config.toml](#settings-and-configtoml)
- [Every board in every Board theme](#every-board-in-every-board-theme)
- [Questions](#questions)

## The Board themes

| Id | Shown as | Light square | Dark square | Highlight |
|---|---|---|---|---|
| `racing` | Racing green (the default) | `#efe7d6` | `#17553a` | `#d8b74a` |
| `slate` | Slate | `#dfe3e8` | `#68757f` | `#e0b64a` |
| `walnut` | Walnut | `#e9d3b0` | `#9a6c46` | `#d8b74a` |
| `match` | Match theme | The lighter of the Theme's page and text | The first of the Theme's accent, selection, border, quiet text, text and page that is clearly darker than the light square | The Theme's `yellow` |

The fourth colour, the marker, is the dark square's colour at 40 %. It fills marked squares, such as Ur's rosettes.

## How to reach it

- *Settings → Look → Board theme*. The boards change at once and the choice is saved.
- Step 1 of the [install wizard](../getting-started/first-start.md) shows all four as small boards, drawn with the Theme you picked.
- Or write `board_theme = "walnut"` under `[look]` in `config.toml`.

## What it needs

Nothing.

## Settings and config.toml

| Setting | `config.toml` key | Default | What it does |
|---|---|---|---|
| Board theme (`board_theme`) | `look.board_theme` | `"racing"` | `racing`, `slate`, `walnut` or `match`. Any other name gives Racing green |

## Every board in every Board theme

Each Game family's board, at the start of a Game, in each Board theme. The *Match theme* pictures are taken with the Nord Theme.

| Game family | Racing green | Slate | Walnut | Match theme (Nord) |
|---|---|---|---|---|
| Chess | <img src="../screenshots/board-chess-racing.png" width="200" alt="The chess board in Racing green"> | <img src="../screenshots/board-chess-slate.png" width="200" alt="The chess board in Slate"> | <img src="../screenshots/board-chess-walnut.png" width="200" alt="The chess board in Walnut"> | <img src="../screenshots/board-chess-match.png" width="200" alt="The chess board in Match theme, with Nord"> |
| Xiangqi | <img src="../screenshots/board-xiangqi-racing.png" width="200" alt="The xiangqi board in Racing green"> | <img src="../screenshots/board-xiangqi-slate.png" width="200" alt="The xiangqi board in Slate"> | <img src="../screenshots/board-xiangqi-walnut.png" width="200" alt="The xiangqi board in Walnut"> | <img src="../screenshots/board-xiangqi-match.png" width="200" alt="The xiangqi board in Match theme, with Nord"> |
| Shogi | <img src="../screenshots/board-shogi-racing.png" width="200" alt="The shogi board in Racing green"> | <img src="../screenshots/board-shogi-slate.png" width="200" alt="The shogi board in Slate"> | <img src="../screenshots/board-shogi-walnut.png" width="200" alt="The shogi board in Walnut"> | <img src="../screenshots/board-shogi-match.png" width="200" alt="The shogi board in Match theme, with Nord"> |
| Go | <img src="../screenshots/board-go-racing.png" width="200" alt="The go board in Racing green"> | <img src="../screenshots/board-go-slate.png" width="200" alt="The go board in Slate"> | <img src="../screenshots/board-go-walnut.png" width="200" alt="The go board in Walnut"> | <img src="../screenshots/board-go-match.png" width="200" alt="The go board in Match theme, with Nord"> |
| Draughts | <img src="../screenshots/board-draughts-racing.png" width="200" alt="The draughts board in Racing green"> | <img src="../screenshots/board-draughts-slate.png" width="200" alt="The draughts board in Slate"> | <img src="../screenshots/board-draughts-walnut.png" width="200" alt="The draughts board in Walnut"> | <img src="../screenshots/board-draughts-match.png" width="200" alt="The draughts board in Match theme, with Nord"> |
| Tafl | <img src="../screenshots/board-tafl-racing.png" width="200" alt="The tafl board in Racing green"> | <img src="../screenshots/board-tafl-slate.png" width="200" alt="The tafl board in Slate"> | <img src="../screenshots/board-tafl-walnut.png" width="200" alt="The tafl board in Walnut"> | <img src="../screenshots/board-tafl-match.png" width="200" alt="The tafl board in Match theme, with Nord"> |
| Backgammon | <img src="../screenshots/board-backgammon-racing.png" width="200" alt="The backgammon board in Racing green"> | <img src="../screenshots/board-backgammon-slate.png" width="200" alt="The backgammon board in Slate"> | <img src="../screenshots/board-backgammon-walnut.png" width="200" alt="The backgammon board in Walnut"> | <img src="../screenshots/board-backgammon-match.png" width="200" alt="The backgammon board in Match theme, with Nord"> |
| Ur | <img src="../screenshots/board-ur-racing.png" width="200" alt="The ur board in Racing green"> | <img src="../screenshots/board-ur-slate.png" width="200" alt="The ur board in Slate"> | <img src="../screenshots/board-ur-walnut.png" width="200" alt="The ur board in Walnut"> | <img src="../screenshots/board-ur-match.png" width="200" alt="The ur board in Match theme, with Nord"> |
| Senet | <img src="../screenshots/board-senet-racing.png" width="200" alt="The senet board in Racing green"> | <img src="../screenshots/board-senet-slate.png" width="200" alt="The senet board in Slate"> | <img src="../screenshots/board-senet-walnut.png" width="200" alt="The senet board in Walnut"> | <img src="../screenshots/board-senet-match.png" width="200" alt="The senet board in Match theme, with Nord"> |

## Questions

#### Does a dark Theme turn the board dark?

No. A Board theme never inverts with a dark Theme. *Match theme* takes the lighter of the Theme's page and text for its light square, so the light squares stay the lighter ones.

#### Why does go have no light and dark squares?

Go is played on the lines, not the squares. Its board takes the light square's colour as the wood and the dark square's colour for the lines. Each Family draws its own board from the same four colours.

#### What is the highlight for?

It marks the last move, the selected piece and the places it can move to. A king in check is marked in the Theme's red instead.

#### Can I make a Board theme of my own?

Not yet. Pick *Match theme* and make a [Theme of your own](themes.md#your-own-theme): its page, text, accent and `yellow` colour the board.

---
[← Previous: Looks](looks.md) · [Next: Languages →](languages.md)
