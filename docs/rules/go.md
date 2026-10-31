# Go

[← Rules](README.md)

**Built.** Three boards (9×9, 13×13 and 19×19) under Chinese rules, in
`crates/rules-go`. The size is a parameter, so the three are three *Variants*
above the rules crate and one game inside it.

## What was ordinary

**361 points do not fit a `u128`**, which cost nothing. The board is an array of
points, and the questions a bitboard made cheap are a flood fill. `has_liberty`
stops at the first liberty, which is what the rules ask, and the full chain walk
only runs when something is being captured.

**Placement rather than movement** is one line in a move type. A move with no
origin was already needed for shogi's drops.

**Passing is a move**, and two passes end the play.

The one worth remembering: the order inside `Position::after` is the whole game.
**Place, lift the enemy chains with no liberties, then ask whether your own has
any.** The other order makes a legal capture look like suicide. It is the most
common Go bug there is, and it has its own test.

## What was as hard as claimed: the ending is a negotiation

Two passes are not a result. The players have to agree which stones are dead, and
that agreement decides the score. In short:

- The record is a list of **entries**, not only moves: `dead:b5,q16`, `accept`,
  `resume`. The agreed dead-stone set decides the score, and a record without it
  cannot be replayed to its own result. Backgammon's dice are entries for the
  same reason.
- A proposal carries the **whole** set, never a change to the last one.
- On a resumption, **the turn simply stands**. The passes were moves and turn
  order never broke, and every alternative hands somebody a free move.
- **The clock is the timeout.** A proposal is never accepted by silence; the
  player who stops answering runs out of time.

Entries are opaque text to everything above the rules crate, so `dead:b5,q16` is
stored and replayed the same way as a move.

## Also decided

**Komi is stored doubled**, as an integer. The half point exists so the game
cannot be tied, and a rule whose purpose is "these two numbers are never equal"
deserves exact arithmetic. Chinese komi is 7.5 against Japanese 6.5, because area
scoring runs about a point in Black's favour.

**Chinese rules in Bragd; both in the rules crate.** The reason is the ending.
Under area scoring a stone played in your own territory costs nothing, so resuming
play to settle a dead-stone dispute is *free*. Under territory scoring the same
honest insistence costs a point a stone, and a protocol whose use is taxed is a
protocol people avoid.

**Positional superko**, not situational: the board alone, not the board and whose
turn it is. Stricter than the AGA's wording, and the difference shows only in
positions almost nobody reaches. One rule is better than two.

**Ko lives inside move generation**, unlike every other repetition rule here. A
ko-forbidden point is not in the legal move list, so a front end needs no
"forbidden" list.

**Three sizes are three Variants.** A nine-point board and a nineteen are the same
rules and not the same skill.

**Komi does not move with the size**, and that is a simplification, not a rule.
`Ruleset::on` changes the board and nothing else, so all three boards are played
with 7.5: the standard figure for 19×19 area scoring, and not what 9×9 is usually
played with. The Rules page says so.

**Byo-yomi belongs to Go.** A Go player choosing a time control expects periods.
Byo-yomi cannot be faked with a Fischer increment: an increment banks time, so a
long endgame gets easier, and byo-yomi never banks. A period finished inside is
handed back full, only overrunning spends one, and overrunning the last loses on
time. The counting phase has no clock of its own, so proposals, acceptances and
"play on" are each made inside a period. The clock is outside the rules crate,
which reads no clock.

## Handicap stones

Handicap stones let a beginner and a strong player sit down to a game either of
them might lose. The rules crate takes any starting position, so a handicap is a
New game choice. The work is in four questions.

**Where the stones go, which is not "the star points".** The handicap list and the
star point list disagree in two ways. A 9×9 handicap is laid out on the **third**
line, not the fourth: nine's star lines are 3, 5 and 7, where thirteen's and
nineteen's are 4 and up. From six stones up, the points used on 13×13 and 9×9 are
**not marked at all**: only nineteen carries the full three-by-three grid of
stars.

The order matters up to four stones, because a three-stone board is a *shape*. It
is taken from two implementations that agree, GNU Go's `place_fixed_handicap`
and Sabaki's `getHandicapPlacement`. Where they disagree, on which corner the
third stone takes, two out of three won: GNU Go puts it upper left, Sabaki has
that order behind a `tygem` flag rather than as its default, and Wikipedia's table
of the traditional pattern says lower right.

Tengen is the fifth stone and the seventh, and **not** the sixth or the eighth: an
even handicap is drawn symmetrically and the centre cannot be half of a pair. The
eight-stone board shows the full grid with an empty star point in the middle.

**What komi becomes.** A handicap game is played at **`stones − 0.5`**: 1.5 at
two stones, 8.5 at nine. That is the AGA's rule. Under Japanese rules a handicap
game has no komi, because the stones are the whole compensation. Under area
scoring they are not: each of Black's free stones also *occupies a point that
counts for him*, which White never got to answer. Leave it out, and a nine-stone
game is eight points more generous than agreed, and the score looks ordinary.
Every value is odd in halves, so a handicap game cannot be tied either. (Eight
stones comes out at 7.5, the same as an even game: `2 × 8 − 1` is 15 halves.)

**The komi travels inside the initial position**, as a third field: `<picture>
<colour> <komi in halves>`. Absent, it means the ruleset's own komi. Handicap
games are not offered in Bragd yet; this is the format they need. A record that does not
carry its komi replays to a **different winner**. Halves rather than a decimal,
because halves are what `Ruleset::komi_halves` holds.

**Who moves first: White.** Black has already had his moves. The colour field of
the position says so, and a move list drawn two to a line must leave the first
cell empty, or every stone is listed under the Player who did not play it.

Two refusals. **A handicap game must name who takes Black**: drawing for it hands
the stones to the stronger player half the time. And **one stone is not offered**,
though the arithmetic handles it: it is a real level (no stones placed, Black
plays first against half a point), but its only trace is a number in the count.

## What is left

1. **Japanese rules as two more Variants**, if ever. The rules crate plays them.
   The reason to hold off is the seki hole below, which is only a hole under
   Japanese counting.

## Not done

**Seki.** Territory counting treats a seki's shared liberties as dame correctly,
but a one-point eye inside a seki is counted as territory, where Japanese rules
score nothing in seki. Fixing it needs a life-and-death judgement the rules crate
deliberately does not make, so the answer has to come through the dead-stone
entries: the players fix it, as they do at a real board. **Under the Chinese
rules Bragd plays, this is not wrong.**

**Japanese double ko is not "no result".** Under Japanese rules a double ko cycle
voids the game; here simple ko has nothing to say about it and the cycle is legal.
Chinese rules close it, which the superko test shows.
