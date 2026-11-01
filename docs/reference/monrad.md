[← README](../../README.md) · [Docs index](../README.md) · [Reference](README.md)

# Monrad and Nordic Swiss

The Monrad system is how most Danish club tournaments are paired. Bragd follows the Danish Chess Union's (DSU) own text, *Monrads turneringssystem*, and its arbiter course *TD3 Monradsystemet* (November 2025). The pairing is pure logic in the `pairing` crate; [club Tournaments](../clubs/tournaments.md) use it, and the Nordic Swiss below.

## Contents

- [How a round is paired](#how-a-round-is-paired)
- [Colours](#colours)
- [Byes, absences and withdrawals](#byes-absences-and-withdrawals)
- [How it was checked](#how-it-was-checked)
- [Nordic Swiss](#nordic-swiss)
- [Questions](#questions)

## How a round is paired

1. Players are numbered by lot before round 1. Rating plays no part.
2. Before each round they are placed: by points, then by their placement at the previous pairing. Before round 2 the draw number decides within a score.
3. From the top, each player meets the nearest player below them whom they have not met. Round 1 is therefore 1 against 2, 3 against 4, and so on.
4. When the players at the bottom cannot all get new opponents, the last pair made is broken up and its higher placed player takes the next one down, then the pair before it, and so on: "always from the top, never from the bottom". Bragd checks ahead, so it finds the same pairing without the trial and error.
5. Meeting twice is the only rule that cannot be broken. A round that cannot be paired at all ends with an error; DSU advises checking the next round before starting one late in the tournament.

## Colours

The player who has had White fewer times has White. With equal counts the lower placed player has White, so in round 1 number 2 has White against number 1. There is no limit on the same colour in a row.

## Byes, absences and withdrawals

| Case | What Bragd does | DSU's rule |
|---|---|---|
| An odd number of players | A dummy is placed last. Whoever meets it has the bye and its point. | "NN, hr. Oversidder" placed last, his opponent scores 1 point |
| A game not yet played | The caller counts it as a draw until the result is known | Postponed games count as a draw for the pairing |
| A player sends apologies for one round | They stay in the pairing unless the director leaves them out | Monrad has no rule for leaving them out |
| A player withdraws | They are no longer paired | Leavers are paired among themselves, or one is placed last when the number is odd |

## How it was checked

The tests replay real DSU tournaments from DSU's own tournament system, round by round, colours included. The data is in `crates/pairing/tests/fixtures/`, each file with its source.

| Source | Players | Rounds | Same as published |
|---|---|---|---|
| Skak og grill i Maribo 2024 ([DSU](https://turnering.skak.dk/TournamentActive/Details?tourId=28955)) | 18 | 7 | 7 |
| Frem Skakklubs åbne klubmesterskab i lynskak 2025 ([DSU](https://turnering.skak.dk/TournamentActive/Details?tourId=29338)) | 28 | 9 | 9 |

The rule texts are checked directly too: the breaking up of the last pair uses the course's own example, and the six-player round that cannot be paired is refused.

## Nordic Swiss

The Nordic Swiss is what Danish clubs call "Nordisk"; the Danish Chess Union named it *Dansk Schweizer*. Its rules are in DSU's Skakhåndbogen, section 4.9.3, with the arbiter guide 4.9.4 (version 15.5.2, 2015; unchanged in 15.10, 2019, the last handbook). DSU has since withdrawn the handbook and no longer lists the system, but clubs still run it, and DSU's tournament system still pairs it.

1. Start numbers go by strength, number 1 the strongest. There is no draw.
2. Before each round the players are split into score pools, paired from the top. Inside a pool, players stand in start-number order.
3. A pool is halved into a strong half a and a weak half b. The top player of a meets the top player of b, the second meets the second, and so on.
4. In a pool with an odd number of players the middle player floats. Before the next pool is paired, each floater meets the nearest lower player not yet met.
5. When two players have met, the players of b move up, undoing the pair before when needed; then players move between a and b; then other floaters are chosen; at last the whole pool floats. A pool is undone only when the players after it cannot be paired at all.
6. Meeting twice is never allowed, nor a pair that leaves a strong colour preference unmet, except between two players over 50 % in the last round.
7. With an odd number a player X is added as the weakest. Whoever meets X has the bye, scores a win and counts as having had White.

| Colours | Rule |
|---|---|
| Round 1 | The weakest player of b has White, and the colours alternate up through b |
| Later rounds | Fewer Whites has White; with equal counts, fewer Whites over the last round, then the last two, and so on |
| Still equal | The player with more points, or the stronger on equal points, has the colour opposite to his last |
| Strong preference | Two more games with one colour, or the same colour in the last two rounds |

Rule 5.2 of the 2011-2019 text asks for the pairing that meets most colour preferences. DSU's own system takes the first pairing found, and so does Bragd.

The tests use the guide's own worked examples: the floating middle player, moving b players up (examples 1 to 3), moving a player between the halves (4), undoing a floater's pair (7 and 8), an alternative floater (9), and the colour example. Six events DSU's system paired as Dansk Schweizer are replayed as well, from [turnering.skak.dk](https://turnering.skak.dk). Their first rounds agree with the rules; in later rounds DSU's system often departs from the text, for example by moving a floater down where the guide moves one up. The test records which rounds agree, so a change shows.

| Source | Rounds | Same as published |
|---|---|---|
| Midtjysk EMT 2018, 2019, 2020 and 2022 | 28 | 13 |
| Midtjysk hurtigskak 2019 | 8 | 2 |
| Haderslev Efterårsturnering 2015 | 9 | 1 |

## Questions

#### Why does my "modified Monrad" event pair differently?

Clubs change the system for their own events, for example by pairing round 1 by rating. Bragd follows DSU's text. Such events are not used as tests.

#### Which should I use, Monrad or Nordic Swiss?

Monrad needs no ratings and suits a field of equals. The Nordic Swiss seeds by strength, so the strongest players meet late, much like the FIDE Swiss. DSU's current rules name only Monrad and the FIDE Swiss.

#### Why is rating not used?

The system dates from before everyone had a rating, so DSU's text starts from the draw. A director who wants the first round by rating can number the players by rating instead of by lot.

---
[← Previous: Round-robin, knockout and Scheveningen](round-robin-and-knockout.md) · [Next: Drop-in tournament →](drop-in.md)
