# Draughts

[← Rules](README.md)

**Built.** Four rulesets on one rules crate, `crates/rules-draughts`: English
checkers (8×8), International draughts (10×10), Brazilian (8×8) and Russian
(8×8), each a Variant of the Draughts family.

The family was built early, and the reason is at the top of
`crates/rules-draughts/tests/perft.rs`: it is the one family with a **published,
independently cross-verified corpus**. Build the game you can prove while the
harness is young, so that every bug found improves the harness rather than the
game.

## A move is three things, and the path is not one of them

`Move` in `crates/rules-draughts/src/position.rs` is an origin, a destination and
a `Bitboard` of captured squares, and its doc comment says the omission out loud:
**the path is deliberately not there.**

The path does not survive the turn. A flying king can walk round four men
clockwise or anticlockwise and end on the same square having taken the same four.
The two sequences produce the same position, so for choosing a move they are one
move. Recording the route would offer the same move twice, which confuses a
player and makes an engine double-count.

That is what the published perft tables mean by "duplicates removed", and it makes
the classic draughts generator bug unrepresentable rather than merely tested for.
The comment in `perft.rs` names the incident: a published run once gave
1,666,207,133 at International depth eleven against the correct 1,665,861,398, a
gap of 345,735 nodes, and its author diagnosed a generator that did not check for
duplicate moves. Here two paths agreeing on origin, destination and captured set
*are* one `Move`, and `Position::captures` deduplicates on those three fields
before anything sees the list.

The cost lands in one place: **the rules cannot answer a question about what
happened in the middle of a turn.** That is what made Russian draughts' crowning
rule hard; see the last section.

## Four rules, and therefore four games

`crates/rules-draughts/src/ruleset.rs` carries four constants, and its header says
why they cannot be collapsed: unlike tafl, the disagreements are not gaps in a
historical record but **different games with different names**, each with a
federation and a literature.

Four fields separate them:

| Field | English | International | Brazilian | Russian |
|---|---|---|---|---|
| `men_capture_backwards` | no | yes | yes | yes |
| `flying_kings` | no | yes | yes | yes |
| `promotion` | `OnStopping` | `OnStopping` | `OnStopping` | `MidSequence` |
| `capture` | `Any` | `Maximum` | `Maximum` | `Any` |
| `first` | Black | White | White | White |

Whether a man may jump backwards is a separate rule from its walking direction;
conflating them is a real bug that the field's comment warns about. A flying king
is not only a longer move but a branching decision, because it chooses where to
stop beyond its victim.

International and Brazilian differ only in board size. International and English
differ in three of the four rules, so a key that opened the wrong one would not
misplay a game, it would play a different game, and the moves would replay
without error. Russian is Brazilian's board and pieces with two rules changed,
and the one that matters is the crowning.

`first` is the side that makes the first move. English draughts starts with Black,
the dark men on printed squares 1 to 12, so its first move is one like `11-15`.
The other three start with White. The perft counts do not change: the opening
array turned half a turn is the same position with the colours swapped.

`Ruleset::filter_captures` is where "take the most" lives, and it counts pieces
only. A king counts the same as a man, because International has no quality
tiebreak, and adding one would be Italian's rule in International's clothing.

One more field is **not** a rule of the game. `counts_duplicate_captures` says
whether that ruleset's published perft tables count a capture twice when two paths
reach the same end. It lives on the ruleset rather than as a flag on a shared perft
harness: a flag on the harness would be the harness learning a rule. The harness
counts what it is given; the ruleset owns what a move is.

## Where the rules are asked

`&Ruleset` is a parameter of `legal_moves`, `quiet_moves`, `captures`,
`captures_as_generated`, `moves_as_counted`, `extend` and `extend_probe`, and
three of its fields are read inside the innermost loop of the capture search.
Chess went the other way for shatranj, giving its pieces their own roles rather
than threading a variant parameter through move generation, and
`crates/rules-xiangqi` carries its ruleset **on the `Position`** and branches once
into two whole bodies.

Both readings are defensible, and the difference is in what varies. Xiangqi and
janggi share a board and disagree about every piece, so two bodies is the honest
shape. The four draughts rulesets share every piece and disagree about four
predicates, so one body with four predicates in it is the honest shape. Four
near-identical capture searches would be four places for the ghost-capture rule
to drift. No timing was measured for this choice.

## The sequence that cannot be stopped early

Capture is compulsory in every ruleset here, so `Position::legal_moves` returns
the quiet moves only when the capture list is empty. What varies is whether you
must take the *most*.

The capture search is `Position::extend`, and three things in it are where
implementations go wrong.

**Captured pieces are ghosts.** They stay on the board as obstacles for the whole
turn and cannot be taken twice. The search carries the already-taken set apart
from the occupancy, and `Position::play` removes them only at the end of the turn
("captured pieces come off now, and not before"). A flying king cannot pass over a
ghost, which surprises players and trips a naive implementation that removes
pieces eagerly.

**The origin square is empty.** `blockers` is the occupancy without the origin, so
a king may circle back over its own starting point. That is one of the ways two
paths reach the same end.

**A sequence is recorded only at its leaves.** `extend` pushes a `Move` only when
`can_extend` says the chain is finished, so a jump that could continue is never
offered as a stopping point. A jump you can see and want to stop on is simply not
in the list.

`can_extend` probes only the square immediately beyond the victim, even for a
flying king. That is correct: if that square is occupied the king cannot jump at
all, so there is no farther landing to check.

## Two counting conventions, and a harness that is not allowed to know

International's published tables remove duplicate capture paths; English's keep
them. That is a convention of the literature, not a rule of either game, and a
rules crate that produced only one convention could only be checked against one
corpus.

So `Position` offers three lists. `legal_moves` is what a player is offered, and
is always deduplicated. `captures_as_generated` is every sequence including the
ones that coincide, and exists only for perft. `moves_as_counted` picks between
them by reading `counts_duplicate_captures`. The perft harness calls the third and
knows nothing about the choice.

`the_two_counting_conventions_really_do_differ` walks nine plies of International
and asserts that the deduplicated and raw counts diverge somewhere. Without it,
nobody could tell `counts_duplicate_captures` was wrong.

## What gates it

`INTERNATIONAL_COUNTS` runs to depth nine (41,022,423 nodes) and is attributed to
Ed Gilbert and Aart Bik agreeing exactly, with Rein Halbersma's `dctl` confirming
the shallow depths: **two independent implementations, not one mirrored source.**
`ENGLISH_COUNTS` runs to depth eight (845,931), in its own convention.

The shallow halves run under `cargo test`; the full depths are `#[ignore]`d and
run with `cargo test --release -- --ignored`. Both pass.

**There is no corpus for Brazilian or Russian**; see below.

## How a game ends

**A player with no legal move loses.** Every piece captured and every piece blocked
are the same result, told apart on the scoresheet as `WinReason::NoPieces` and
`WinReason::Blocked`. There is no stalemate draw anywhere in the family, and
porting that from chess is the common bug. `having_no_move_is_a_loss_and_not_a_draw`
reaches a blockade by random play rather than by constructing one, and asserts
that a blockade happened at all in two hundred tries.

Draws are the part the rules deliberately do not pretend to settle. A position
that occurs three times is drawn. A hundred plies without a capture or a man
moving is drawn under `DrawReason::Stalled`, and the name is the point: the real
rules differ per Variant, are written about king-only endings, and carry their own
move counts. `STALL_LIMIT = 100` is Bragd's policy, and calling it that beats
implementing one federation's rule and labelling it the others'.

`Game::play` clears the repetition history on any irreversible move, a capture or
a man's move, which keeps the threefold test cheap and correct.

## How a move is written

A draughts move is stored as `31-27` or `31x22:18`: the two squares, then every
captured square. The captured set has to be written: real draughts notation
`31x22` does not say which route, and an ambiguous record cannot be replayed. The
deduplication rule above is what makes the appended set enough.

A move is never parsed. The legal moves are written out and the entry is matched
against them, so an unrecognised string cannot become a move, only a refusal.

Half a draughts board is not part of the game, so the rules crate numbers only the
playable squares, and they are expanded into a grid to be drawn. Dark squares are
on the even files of rank 0 and the odd files of rank 1. Getting the parity wrong
produces a board that looks right and whose diagonals are broken, which is why
`Geometry::file_rank` in `crates/rules-draughts/src/board.rs` has a round-trip
test over every index. Draw from that function rather than a second copy of the
arithmetic.

## Not done, and honest about it

**Russian's mid-jump crowning was half-implemented, and is now whole.** It was
wrong at *both* ends of a move:

- The generator crowned the man for **exactly one leg**, the one it made *from*
  the crowning square. `extend` computed `king` from `is_promotion_rank(at)`, and
  `at` is the square the piece is standing on, so once it landed anywhere else it
  was a man again. Captures the rules compel were silently not offered.
- `Position::play` crowned on the **destination**. A man that touched the far rank
  in passing and landed elsewhere finished the turn as a man.

The second half was measured before it was fixed. Four thousand random Russian
games, comparing `legal_moves` under `RUSSIAN` against the same ruleset with
`promotion` set to `OnStopping`, found 873 positions where the crowning changed
the move list and 1,703 moves that only Russian offers and that end without a
crown. Not one kept it.

**The fix is one bit, and it is deliberately not the path.** `Move` gains
`crowned`, which says whether the man became a king *during* the sequence rather
than by stopping on the far rank. The generator threads it through the recursion
so the piece keeps flying for the rest of the turn, and `play` crowns on it as
well as on arrival. `crowns_here` is the single place that answers "does standing
here crown this piece", because the bug was two halves disagreeing.

It is in the deduplication key too: under Russian, one path through the far rank
and one around it can take the same men and stop on the same square, leaving a king
in one case and a man in the other. Those are different positions and therefore
different moves.

**Nothing outside Russian can be affected.** `crowned` is only set when
`promotion == Promotion::MidSequence`, and a test asserts `RUSSIAN` is the only
ruleset that sets it. International and English deep perft were run in release
after the change and did not move.

Four tests came with it, each mutation-checked against the bug it is for. Their
position: a man one jump from the far rank, a second victim beyond it, and a third
victim **two** steps away with an empty square between. A man sees an empty square
and stops; a king sees past it and must take. So the third capture happens only if
the piece is still a king two legs after it was crowned. A position that crowns on
the last leg cannot show that, and the first version of the test was written that
way and passed against the bug. Brazilian is the control: the same men, one flag
different, and the man stops two captures in, still a man.

**Brazilian and Russian still have no perft table.** Brazilian is International's
four flags on `Geometry::SMALL`, and the geometry is tested at both sizes, so the
argument that it is covered is nearly airtight. For Russian that argument failed
once, because `Promotion::MidSequence` was the one flag no tested ruleset set.
Russian now has four tests of its own; it still has no table.

A table needs a position reader, and **the crate has no way to build a
position.** `Position::start` is the only constructor, there is no FEN reader, and
the fields are private. Tests outside the crate reach their positions by playing;
the crowning tests live *inside* `position.rs`, where a position can be stated.

**Draughts reads no starting position.** Every draughts Game begins from the
opening array, so every draughts teaching position is a line of moves from the
start rather than a board.
