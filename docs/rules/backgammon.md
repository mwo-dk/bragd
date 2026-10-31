# Backgammon

[← Rules](README.md)

**Built**, in `crates/rules-backgammon`. The only Variant in Bragd with a doubling
cube, and the first with dice.

## What makes it different from everything else here

**A game of backgammon is not determined by its moves.** Every other rules crate is
a pure function, so a Game can be stored as a starting position and a list of
moves and replayed years later. Dice are not a pure function. The answer: **a roll
is an input.** `Position::turns_for(roll)` takes it as an argument, whoever plays
the Game throws it, and the record stores it. The crate has no dependencies and no
way to acquire a random number generator by accident. An RNG in the rules would
make old Games unreplayable *silently*: nothing fails, and the damage shows the
first time somebody opens an old Game and sees a different one.

### The roll is its own entry

The record is `5-3`, then `13/8 8/5`: two entries, not one string.

**A player has to see the roll before choosing a turn.** So something must hold
it. The tempting place is a field beside the record, which would be a second
source of truth for the dice and would be lost on an abandoned Game, where it is
the one thing somebody might want to check. As entries, the dice are written down
the instant they exist, and `Game::replay` is exact over the whole record.

Two mistakes are available and both are quiet. Throw twice and a player plays
their second roll with the first showing. Throw never and the Game hangs with
nobody able to act. So the rules answer `needs()`, and whoever throws asks it
rather than inferring it from the shape of the record.

### A double comes before the roll

A double is offered *before* rolling, because seeing your dice first would make
the decision free, and the rules enforce that. So the dice must not be thrown the
instant a roll becomes legal: while the player to move still holds a cube
decision, the throw waits for them to press Roll. Throwing at once would close
the doubling window before anybody could use it. A front end therefore needs a
Roll action as well as a Double action.

### Who must act is not always the side to move

**A double is offered on your own turn**, so while one waits, the position still
says the *doubler* is to play while the other player owes the answer. The clock
follows `needs()`, not the side to move. Driving it from the side to move would
run the doubler's clock through the opponent's whole cube decision.

### An entry contains a space

`13/8 8/5` is one turn in the notation every backgammon player reads. Every other
Family's entry is a single token. A record of entries must therefore never be
joined or split on spaces: split back, that turn becomes two entries that each
parse, and the Game replays as a different game without any error.

### Who moved first is a fact about the Game

The opening roll is one die each, the higher going first, so who moved first is
not in the position. It is stored with the Game, like a Chess960 arrangement,
because it is the same question: where did this Game begin. A Game should not be
opened without it: entries alone replay into the wrong game about half the time,
and the result looks reasonable.

The opening roll is then thrown separately, **rethrown until the dice differ**.
Who goes first is already settled, so all that is left of the higher-die rule is
the absence of doubles. Leaving it out would hand the first player an opening 6-6
that no backgammon player has ever seen.

### A result is not one point

A gammon is two points and a backgammon is three, times the cube. A concession is
graded the same three ways: conceding a single is one, a gammon two and a
backgammon three, each multiplied by the cube, so a gammon conceded on a cube of
two is four points. A plain "resign" concedes a **single** game; nobody who pressed
one button should be charged three points.

## Not done, and honest about it

**Turns can be built a die at a time.** What is legal depends on both dice
together, because you must use as many as any sequence can, so playing the 5 first
may make the 3 illegal. A board that let you play them one at a time would have to
enforce that itself, which is a rule living in a front end.

**So the rule stays in the rules crate.** `Position::continue_turn(roll, played)`
answers the two questions a board assembling a turn asks (what may I play next,
and is the turn finished) by *filtering the whole legal turns it already
generates*. A play that strands a die never appears in a legal sequence, so it is
never offered, and a caller that sends one anyway is refused outright.

Two things fell out of writing it:

- **A turn is finished exactly when it cannot be extended**, so "what may I play
  next" and "may I stop" are one question. The answer is an enum rather than a list
  plus a flag, because a caller holding the two separately can offer a "done"
  button halfway through a turn.
- **Order is not a turn's identity, but it is a turn's legality.** Turns are
  deduplicated by their placements, since two orderings reach the same board. But
  `13/8 8/5` needs the checker to arrive on the 8-point before it can leave it, so
  the orderings are not interchangeable while a turn is being assembled.

`bragd-games` offers whole turns. Building a turn a die at a time goes through
`continue_turn` when a front end needs it.

**There are no matches.** Every Game is played for its own points, so the Crawford
rule and match equity do not arise.

**No Jacoby rule**, so a gammon counts with the cube in the middle.

**There is no perft gate**, and the roll is why: perft counts the leaves of a tree
that is a function of the position alone, and this tree branches over twenty-one
rolls before it branches over turns. The gate is properties instead, in
`crates/rules-backgammon/tests/properties.rs`. The strongest is that a Game rebuilt
from its entries equals the original in position, cube, entries and outcome. Take
the rolls out of the record and that test fails at once.
