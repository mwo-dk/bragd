[← README](../../README.md) · [Docs index](../README.md) · [Reference](README.md)

# Drop-in tournament

A Drop-in tournament ("spil når du vil", play when you like) runs over a season of club evenings. Whoever turns up on an evening is paired among themselves, and the standings add up the evenings. The pairing is pure logic in the `pairing` crate; the tournament screens that use it come with the rest of 0.3.0.

## Contents

- [How an evening is paired](#how-an-evening-is-paired)
- [Standings](#standings)
- [Questions](#questions)

## How an evening is paired

1. Only the players present are paired.
2. Each has a pairing score: their points so far, plus half a point for every round they were away. The half point counts for the pairing only, never in the standings, so a player who misses evenings is not dropped to the bottom.
3. Players are ranked by pairing score, then by their number in the tournament.
4. With an odd number present, the bye goes to the lowest ranked of those with the fewest byes so far.
5. From the top, each player meets the highest ranked player below them whom they have met least often. Over a season the same people turn up again and again, so a repeat is allowed when nothing else is left; it is only put off.
6. Colours follow the Monrad rule: the player who has had White fewer times has White; with equal counts, the lower ranked player.

The director can change any pairing, as in every system.

## Standings

| Standing | What counts | When to use it |
|---|---|---|
| Percentage (the default) | Points as a share of the most that could have been scored in the rounds played | Players come on different evenings; a player who came twice and won both is ahead of one who came ten times and won seven |
| Total points | Every point scored | Turning up often should count |
| Best N results | The N best results of each player | A fixed number of evenings counts, say the best 10 |

A round away is not a result: it counts nowhere in the standings. A bye or a forfeit is a result, worth what it scored. Ties go by the number in the tournament.

## Questions

#### Why half a point for an absence?

So that missing an evening does not decide who you meet next time. Ranked by real points only, a strong player who was away would meet the weakest players when they came back.

#### Can two players meet twice?

Yes, in a Drop-in tournament they can, but only when no one they have met less often is left. In a Swiss or Monrad tournament a repeat is never allowed.

#### Is there a federation rule for this?

No. The Danish Chess Union has none; clubs run such evenings by their own rules, often as a "modified Monrad". Bragd's version is the one in Bragd's design, described on this page.

---
[← Previous: Monrad and Nordic Swiss](monrad.md) · [Next: Rules →](../rules/README.md)
