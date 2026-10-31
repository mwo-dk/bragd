# Janggi

[← Rules](README.md)

Korean chess. **Built**, inside `crates/rules-xiangqi` as `Ruleset::Janggi`. It
is a Variant of the Xiangqi family in Bragd's catalogue, with pieces of its own.

## Why it was cheap

Janggi shares xiangqi's shape: pieces on intersections, a 9×10 board, palaces, a
cannon that leaps, characters on discs. The board work xiangqi needed (not
chequered, characters rather than shapes, intersections rather than squares) is
exactly what janggi needs. It is a `Ruleset` inside the same crate because one
rules crate serves one *board*, not one game.

Its pieces are not xiangqi's: eight hanja on octagons, at three sizes. So the
Variant has its own sprites, under the Xiangqi family's name.

## Why it is not just xiangqi

- **No river.** Every piece may cross freely, which changes the whole game.
- **The palace diagonals matter for more pieces**, and the general and guards
  move along them.
- **The cannon may not leap another cannon, and may not capture one.** It cannot
  move at all without a screen, where a xiangqi cannon slides freely and only
  needs a screen to capture.
- **The elephant's move is longer**: one orthogonal step, then two diagonal. It
  can be blocked at two points rather than one.
- **A player may pass.** Nothing in the chess family can, and it removes an
  ending: **there is no stalemate in janggi.** A player not in check may always
  pass, so an empty move list means checkmate and nothing else.
- **Bikjang**: if the two generals face each other with nothing between, the game
  is drawn. Xiangqi forbids the position; janggi scores it. Three readings are in
  circulation, and these rules take the simple draw.
- **A game that runs out is ranked, not halved.** Where xiangqi halves the point
  on a repetition or the quiet limit, janggi counts material and produces a
  *winner*. Han's 1.5-point handicap makes a tie impossible, and the crate has a
  test named for it.

The last two change how a game ends, so read them before assuming the two games
share their endings.

## Not built: the setup

**Each side may swap its horses and elephants before play**, so a game begins
from one of four arrangements per side, sixteen per game. That has to be stored
with the Game, the way a Chess960 arrangement is.

The rules crate has it (`Wing`, `Setup`, `Setup::ALL` and
`Position::janggi_from` in `crates/rules-xiangqi/src/ruleset.rs`), and **nothing
offers it yet**: every janggi Game opens from `Setup::WONANGMA`. Offering it is
a New game choice, made before play rather than during it.
