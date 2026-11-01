[← README](../../README.md) · [Docs index](../README.md) · [Reference](README.md)

# Round-robin, knockout and Scheveningen

Besides the Swiss, Bragd pairs the three systems where who meets whom is fixed in advance: everyone against everyone, a knockout bracket, and a match of two teams. Like the Swiss pairing, this is pure logic in the `pairing` crate; [club Tournaments](../clubs/tournaments.md) use it. Every system hands back the same shape: the boards in order, the bye if there is one, and a short account of the rule each group of boards came from, for the director's "why" view.

## Contents

- [Round-robin](#round-robin)
- [Knockout](#knockout)
- [Scheveningen](#scheveningen)
- [How it was checked](#how-it-was-checked)
- [Questions](#questions)

## Round-robin

Players get their numbers by lot, and each round follows FIDE's Berger tables (Handbook C.05 Annex 1). With an odd number of players the highest number counts as a bye, so a field of 7 plays the table for 8 and whoever meets number 8 sits out.

| Option | What it does |
|---|---|
| Single | Every pair meets once. |
| Double | The second cycle repeats the first with colours reversed, as at the FIDE Candidates 2026. |
| Swap the last two rounds | In a double round-robin, the last two rounds of the first cycle change places, as C.05 Annex 1 recommends, so that no one has the same colour three times running. |

## Knockout

Players are seeded from 1, and the bracket is paired as the FIDE World Cup regulations pair it: top half against lower half reversed (1-128, 2-127 and so on).

- The bracket has the next power of two of places. The top seeds take the places left over as byes and start in round 2: 206 players give 50 byes, as in the World Cup 2025.
- In each round, match 1 is the first place against the last, match 2 the second against the last but one. The winner of a match takes its place in the next round, so seeds 1 and 2 can meet only in the final.
- In round 1 the higher seed of match 1 has the colour drawn at the opening, the higher seed of match 2 the other one, and so on down the list. Each round after, every place has the colour opposite to its last. The board gives the colours of the first game.

How many games a match has, its tie-breaks and a match for third place are for the event to decide.

## Scheveningen

Two teams of the same size play, and every player of one team meets every player of the other, in as many rounds as a team has players. Team A's boards are numbered 1 upwards, team B's after them.

In round 1 board 1 of team A meets board 1 of team B, board 2 meets board 2, and so on; each round, team B moves one board on. Team A has White when the two board numbers add up to an even number. So every player has White in half their games, give or take one, and with an even team size every player alternates colours. A double Scheveningen repeats the rounds with colours reversed.

## How it was checked

| Source | What is compared |
|---|---|
| FIDE Handbook C.05 Annex 1, the Berger tables for 3 to 16 players ([FIDE](https://handbook.fide.com/chapter/C05Annex1)) | Every round of every table, board order and colours |
| FIDE Candidates 2026, double round-robin of 8 ([chess-results.com](https://chess-results.com/tnr1384162.aspx?lan=1&art=2&rd=1&turdet=YES)) | All 14 rounds |
| FIDE World Cup 2019 regulations, art. 4.5-4.6 ([FIDE](https://www.fide.com/FIDE/handbook/WorldCup2019Regulations.pdf)) | The 128-player first round and the colours |
| FIDE World Cup 2025 regulations, art. 4.2 ([FIDE](https://handbook.fide.com/files/handbook/WorldCup2025Regulations.pdf)) | The 50 byes, matches 1.1, 1.2 and 1.78, and matches 2.1, 2.2 and 2.64 |

No federation publishes a Scheveningen table, and events on chess-results.com each draw their own, so the tests check what every Scheveningen must do: every pair of opponents once, and colours balanced.

## Questions

#### Why does my double round-robin not swap the last two rounds?

FIDE only recommends it, and its own Candidates 2026 did not. Turn the swap on if your event wants it.

#### Who gets a bye in a knockout that is not a power of two?

The top seeds. FIDE states it for the World Cup's 206 players; Bragd gives the same answer for any field: as many top seeds as there are places left in the bracket.

#### Can the lower seed take the higher seed's colours?

Yes. A winner takes the place of the player they beat, colours included, as the World Cup regulations say.

---
[← Previous: Swiss pairing](swiss-pairing.md) · [Next: Monrad and Nordic Swiss →](monrad.md)
