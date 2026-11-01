[← README](../../README.md) · [Docs index](../README.md)

# Rules

Bragd plays 25 Variants in nine Game families, on ten rules crates. Each rules
crate is pure Rust with no dependencies, and it is the authority for what is a
legal move, in Bragd and on a future game server. `crates/bragd-games` is the one
seam over all of them: start, replay, legal moves, the board to draw and the
result.

These pages are records rather than plans. Each says what was built, what was
decided along the way and why, which sources a reconstruction follows, and what is
still open.

## The Variants

| Game family | Variant | Key | Board | Rules crate | Page | Notes |
|---|---|---|---|---|---|---|
| Chess | Chess | `chess` | 8×8 | `rules-chess` | | The one everything else is compared to. |
| Chess | Chess960 | `chess960` | 8×8 | `rules-chess` | | 960 starting arrays; only castling needed new code. |
| Chess | Shatranj | `shatranj` | 8×8 | `rules-chess` | | Ferz and alfil; stalemate wins, a bared king loses. |
| Chess | Chaturanga | `chaturanga` | 8×8 | `rules-chess` | | A named reconstruction. Being stalemated wins. |
| Chess | King of the Hill | `kingofthehill` | 8×8 | `rules-chess` | [Chess variants](chess-variants.md) | Chess plus one ending. No material is ever insufficient. |
| Chess | Racing Kings | `racingkings` | 8×8 | `rules-chess` | [Chess variants](chess-variants.md) | No pawns, no checks allowed, both kings race to rank 8. |
| Chess | Seirawan chess | `seirawan` | 8×8 | `rules-chess` | [Seirawan](seirawan.md) | A hawk and an elephant gated in; a pawn promotes six ways. |
| Chess | Capablanca chess | `capablanca` | 10×8 | `rules-capablanca` | [Capablanca](capablanca.md) | Archbishop and chancellor; the king castles three squares. |
| Xiangqi | Xiangqi | `xiangqi` | 9×10 | `rules-xiangqi` | [Xiangqi](xiangqi.md) | The perpetual-chase rules are not implemented. |
| Xiangqi | Janggi | `janggi` | 9×10 | `rules-xiangqi` | [Janggi](janggi.md) | Korean chess on the same board. |
| Shogi | Shogi | `shogi` | 9×9 | `rules-shogi` | [Shogi](shogi.md) | Captures come back as drops. |
| Go | Go 9×9 | `go-9` | 9×9 | `rules-go` | [Go](go.md) | Chinese rules; the ending is negotiated. |
| Go | Go 13×13 | `go-13` | 13×13 | `rules-go` | [Go](go.md) | |
| Go | Go 19×19 | `go-19` | 19×19 | `rules-go` | [Go](go.md) | |
| Draughts | English checkers | `checkers` | 8×8 | `rules-draughts` | [Draughts](draughts.md) | English rules. |
| Draughts | International draughts | `draughts` | 10×10 | `rules-draughts` | [Draughts](draughts.md) | The FMJD game, and a different game rather than a larger one. |
| Draughts | Brazilian draughts | `draughts-brazilian` | 8×8 | `rules-draughts` | [Draughts](draughts.md) | International's rules on the small board. |
| Draughts | Russian draughts | `draughts-russian` | 8×8 | `rules-draughts` | [Draughts](draughts.md) | Any capture; a man crowned mid-jump keeps jumping as a king. |
| Tafl | Hnefatafl (Copenhagen) | `hnefatafl` | 11×11 | `rules-tafl` | | Copenhagen rules. |
| Tafl | Hnefatafl (Fetlar) | `hnefatafl-fetlar` | 11×11 | `rules-tafl` | | Copenhagen minus the shieldwall, the exit fort and the repetition ban. |
| Tafl | Tablut | `tablut` | 9×9 | `rules-tafl` | [Tafl sizes](tafl-sizes.md) | Linnaeus, 1732: the only contemporary account of any tafl. |
| Tafl | Brandubh | `brandubh` | 7×7 | `rules-tafl` | [Tafl sizes](tafl-sizes.md) | The Irish game. The thinnest evidence of the three. |
| Backgammon | Backgammon | `backgammon` | 24 points | `rules-backgammon` | [Backgammon](backgammon.md) | Dice as inputs, and a doubling cube. |
| Ur | The Royal Game of Ur | `ur` | 20 squares | `rules-ur` | [Ur and senet](ur-and-senet-rules.md) | Finkel's reconstruction; four tetrahedral dice. |
| Senet | Senet | `senet` | 3×10 | `rules-senet` | [Ur and senet](ur-and-senet-rules.md) | Kendall's reconstruction; throwsticks, and nothing is captured. |

**One rules crate per board, not per Variant.** Seven of the 25 share
`crates/rules-chess`, because they share a board and a piece set. Capablanca has
its own because eighty squares do not fit in a `u64`. The four tafl rulesets share
one because the index space is eleven wide whatever the board is. Janggi shares
xiangqi's because it is played on xiangqi's board.

## The pages

- [The cheap chess variants](chess-variants.md): King of the Hill and Racing Kings
  are built; Three-check, Atomic, Horde, Antichess and Crazyhouse are not.
- [Seirawan chess](seirawan.md): gating is new state, and it is on `Position`.
- [Capablanca chess](capablanca.md): the rules crate that exists because of
  arithmetic rather than rules.
- [Xiangqi](xiangqi.md): the perpetual-chase rules are the one thing missing, and
  the page has the position where a player meets it.
- [Janggi](janggi.md): built inside `crates/rules-xiangqi`. The pre-game setup is
  the one part that is not offered.
- [Shogi](shogi.md): drops and rotated pieces.
- [Go](go.md): the ending is a negotiation, and handicap stones.
- [Draughts](draughts.md): a move is an origin, a destination and a captured set
  with **no path in it**, which makes the duplicate-capture bug unrepresentable.
- [The tafl sizes](tafl-sizes.md): what adding brandubh and tablut cost against
  the estimate.
- [Backgammon](backgammon.md): a roll is an input, not an event.
- [Ur and senet](ur-and-senet-rules.md): two named reconstructions, with every
  point where the sources disagree.

## How the rules are checked

Where a published perft corpus exists, it is the gate: chess, Chess960, xiangqi,
shogi, and International and English draughts. Capablanca, shatranj, chaturanga
and Seirawan have self-recorded counts backed by property tests. The shallow depths
run under `cargo test`; the deep ones are `#[ignore]`d and run with
`cargo test --release -- --ignored`. Families with dice (backgammon, Ur, senet) and
tafl have no useful perft, and are gated by property tests instead.

## What the catalogue is missing, which is not age

Bragd already plays old games: Go is about two and a half thousand years old,
backgammon descends from Roman *tabula*, the tafl rulesets are Viking, shatranj is
seventh-century and chaturanga older than that. The modern entries are Chess960,
King of the Hill, Racing Kings, Capablanca and Seirawan: five, all of them chess.

What is thin is **mechanism**:

| Mechanic | Variants | Which |
|---|---|---|
| Checkmate | 11 | chess ×7, Capablanca, xiangqi, janggi, shogi |
| Jump-capture | 4 | the draughts rulesets |
| Territory | 3 | the Go boards |
| Siege and escape | 4 | the tafl rulesets |
| Race | 3 | backgammon, Ur, senet |

Five mechanics in 25 Variants, and eleven of them are the same one. A candidate
for a new Family has to bring a mechanic nothing here shares.

### Shortlisted, and not built

- **Mølle (nine men's morris)**: mill and alignment, the largest hole in the table
  above. Living rules, no reconstruction needed, and boards scratched into Egyptian
  roofing slabs, Roman forums and the deck of the Gokstad ship. Its 24 points map
  onto a 7×7 grid, so unplayable cells (as in draughts), placement with no origin
  (as in shogi's drops) and pieces on intersections (as in xiangqi and Go) already
  exist. It also brings a phase Bragd has never had: you build the position before
  you play it.
- **Bagha-chall, or fox and geese**: the hunt. Goats are placed, tigers jump, and
  the goats win by **immobilising** rather than capturing, a win condition nothing
  here has. Not a tafl re-skin, though it looks like one.

**Considered and declined: oware and the mancala family.** It is the other new
mechanic, and it was declined on cost rather than merit. A pit holds a *count*,
not a piece, and the board model and the sprites assume one piece per cell.
Recorded here so it is not proposed again as though it were cheap.

---
[← Previous: Swiss pairing](../reference/swiss-pairing.md) · [Next: Changelog →](../changelog.md)
