[← README](../../README.md) · [Docs index](../README.md) · [Reference](README.md)

# Swiss pairing (FIDE Dutch)

Bragd pairs a Swiss round under the FIDE (Dutch) system: FIDE Handbook C.04.3, as applied from 1 February 2026, with the Basic Rules (C.04.1) and General Handling Rules (C.04.2) of the same date. The pairing is pure logic in the `pairing` crate. The tournament screens that call it come with the rest of 0.3.0; this page says what the pairing does and how it was checked.

## Contents

- [What it handles](#what-it-handles)
- [How a round is paired](#how-a-round-is-paired)
- [Colours](#colours)
- [Why a pairing is what it is](#why-a-pairing-is-what-it-is)
- [How it was checked](#how-it-was-checked)
- [Questions](#questions)

## What it handles

| Rule | Where in the Handbook |
|---|---|
| No two players meet twice; a forfeited game does not count as a meeting | C.04.1 2, C.04.2 3.5 |
| One pairing-allocated bye when the number is odd, never twice, never to a player who already scored a win without playing | C.04.1 3-4 |
| The bye goes to the lowest score that still lets everyone else be paired, then to the player with the fewest unplayed rounds | C.04.3 C5, C9 |
| Colour preferences: absolute, strong and mild, from played games only | C.04.3 1.7, C.04.2 3.4 |
| Topscorers in the final round may break the colour limits, as little as possible | C.04.3 1.8, C10, C11 |
| Downfloaters and upfloaters of the last two rounds are protected | C.04.3 1.4, C14-C21 |
| Requested byes, absences and withdrawals: those players are simply not paired | C.04.2 3.2-3.3 |
| Late entries: they join with their unplayed rounds and are paired from their first round | C.04.2 2.4 |
| Boards in publishing order | C.04.2 3.6 |

Accelerated pairings (C.04.7) are not in this version ([#129](https://github.com/mwo-dk/bragd/issues/129)).

## How a round is paired

Players are ranked by score, then by TPN. Brackets are paired from the top scoregroup down. For each bracket the rules want the candidate that best meets criteria C5 to C21, in that order, and among equal candidates the one that comes first in the sequence of transpositions and exchanges (C.04.3 3 and 4).

Bragd finds that candidate in two steps:

1. **The best values.** One weighted matching covers the bracket, every player below it, and the bye when there is one. A complete matching is a completed round, so the round can always be finished (C4). Each criterion is a band of bits in the weights, highest priority first, so the best matching is the best on C5, then C6, and so on to C21.
2. **The first candidate with those values.** Bragd then walks the rules' own order: the set of moved-down players to pair, each one's opponent, the exchange in the remainder, and each S1 player's opponent. At every step it keeps the earliest choice that can still reach the best values.

A round that cannot be paired at all ends with an error, and the Chief Arbiter decides (C.04.3 1.9.3).

## Colours

Colours follow C.04.3 Article 5: both preferences if possible, then the stronger one, then alternation from the last round in which the two players had different colours, then the higher ranked player's preference. Two players with no games between them take colours from the colour drawn before round 1 and the parity of the higher ranked player's TPN.

## Why a pairing is what it is

For each bracket the pairing reports its residents, its moved-down players, the pairs, the downfloaters, whether a transposition or an exchange was needed, and every criterion the chosen candidate does not fully meet. A pairing made by hand can be judged the same way: Bragd names the first bracket where it differs and says whether it is illegal, worse on a named criterion, or as good but later in the rules' order.

## How it was checked

The tests replay whole tournaments round by round: each round is paired from the history before it and compared with what was published. The test data is in `crates/pairing/tests/fixtures/`, each file with its source.

| Source | Rounds | Same as published |
|---|---|---|
| Mario Held, *Mastering the Dutch* 2026, the worked 15-player tournament ([FIDE TEC](https://tec.fide.com/wp-content/uploads/2026/07/Mastering_the_Dutch_2026.pdf)) | 9 | 9 |
| 13 Swiss events played after 1 February 2026, from chess-results.com, 15 to 353 players | 91 | 75 |

Of the 16 rounds that differ, six are round 1, where the final start numbers are not the ones used to pair it. In every one of the 16, judging the published round against the rules gives one of three answers: it is worse on a named criterion (6), it is as good but later in the rules' order (7), or only some colours differ (3). In none is the published round better than Bragd's. From round 5 on, when start numbers can no longer change (C.04.2 2.3), 38 of 39 rounds are the same.

The rule texts are checked directly too: the order of exchanges and of moved-down sets uses the Handbook's own examples, and the colour rules the examples in Held's *Annotated FIDE (Dutch) Rules 2026*.

## Questions

#### Why do some published rounds differ?

A results file holds only the final start numbers. Late entries and rating corrections can renumber players, and that changes which candidate comes first. Arbiters may also change a pairing by hand (C.04.2 4.4), and an event run in Swiss-Manager does not say whether it used Dutch or another FIDE system.

#### Can Bragd be wrong about the rules?

It can, and the replay tests are there to catch it. Every published round Bragd does not reproduce is listed in the test with the reason found for it; a change in behaviour fails the test.

#### How fast is it?

A club tournament pairs in well under a second. The largest test, 353 players, takes up to ten seconds a round.

---
[← Previous: Rating lists](rating-lists.md) · [Next: Rules →](../rules/README.md)
