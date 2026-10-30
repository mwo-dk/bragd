# Xiangqi

[← Rules](README.md)

**Built.** Chinese chess, in `crates/rules-xiangqi`, with two teaching positions
in `crates/bragd-games/src/positions.rs`.

It has its own rules crate, and the reasons reach the bottom of it. Pieces stand
on **ninety intersections** rather than sixty-four squares; two palaces confine
three of the seven pieces; a river divides the board and is a boundary rather
than a rank; and three rules (the cannon's screen, the horse's hobbled leg and
the two generals' refusal to face each other) exist in no other Family. Janggi
shares the crate, because one rules crate serves one **board**; see
[janggi's own page](janggi.md). This page is about the Chinese game.

## The shape, and what the board is asked

Ninety points fit in a `u128` with room to spare, so a set of points is one value,
as in the other rules crates. Files run a–i from Red's left and ranks 1–10 from
Red's back line. That is the convention the published perft positions use, so the
crate reads its corpus in the corpus's own coordinates.

The river **occupies no points at all**: ranks 5 and 6 are both playable and
adjacent, and the river is the gap between them. `Point::is_own_half` is the
whole of it, and the test in `board.rs` asserts the absence (one step from a5
reaches a6) rather than where a boundary rank is. Where to paint the water should
be worked out by asking `is_own_half`, so the line drawn and the line an elephant
may not cross cannot drift apart.

The palace is nine points, files d–f and the three ranks nearest a player, asked
as a predicate rather than stored as a mask. The diagonals drawn on it are
**decoration in xiangqi**: the advisor's move happens to trace them and nothing
else may use them, so xiangqi's move generation needs no notion of a line.
`Point::palace_diagonal` exists for janggi. A xiangqi board is still drawn with
its palace diagonals, because without them it does not look like a xiangqi board.

## The flying general is a rule about positions, not moves

Two generals on one file with nothing between them is illegal. The crate enforces
it inside `Position::is_in_check` rather than by filtering moves afterwards, and
that placement is the whole of its correctness. It is not a rule about which moves
are legal; the position itself cannot exist. So a move that creates one is
refused by the same code that refuses moving into check.

Two consequences follow. `is_in_check` returns **true for both sides at once** in
a facing position, which is why the test asserts it of Red and of Black. And no
general ever generates a move down the file: `xiangqi_moves` gives the general
only palace steps. "In practice a general can attack down an open file as though
it were a chariot" describes an effect, not a move. The effect lives entirely in
`is_in_check`.

The other half of the decision is janggi's: the check is guarded on
`self.rules == Ruleset::Xiangqi`, because janggi calls the same geometry
*bikjang* and plays it. Carrying xiangqi's reading across would delete a class of
legal janggi moves and turn every bikjang into checkmate.

## The cannon is two walks, and the horse makes attack asymmetric

The cannon's rules for moving and for capturing do not overlap, so
`xiangqi_moves` writes two loops. It slides over empty points like a chariot;
separately, it walks to the first piece on the line (the screen, of either
colour) and takes the first piece beyond it, if that piece is an enemy. A cannon
on an open file pointing at the general is not check and not even a threat. That
is the hardest thing about the game for a chess player, and it is what the first
teaching position is about.

The horse moves one orthogonal step then one diagonal step outward, and is blocked
at the orthogonal step: hobbling the leg. That is why
`crates/rules-xiangqi/src/lib.rs` opens by saying **attacks are not symmetric
here**: whether A attacks B depends on what stands beside *A*. Nothing else in
Bragd has that property, and it is why `is_attacked` cannot be a table lookup.

So `is_attacked` generates every pseudo-legal move for that side and checks
whether any lands on the point. It is called once per candidate move inside
`legal_moves`, which makes legality filtering quadratic in the move count. That is
a cost, not a bug: the deep perft below runs in about a minute.

## One board, two games, and only the key says which

A janggi position and a xiangqi position are the same ninety points and the same
FEN letters: **the janggi opening array is a legal xiangqi FEN, and the reverse
holds too.** `JANGGI_START` read under xiangqi's rules parses and offers 45 legal
moves, and `START` read under janggi's parses and offers 27. A Game opened under
the wrong key does not fail: it produces a game in which the elephants leap the
wrong distance, and it looks reasonable doing it. So the Variant key decides, and
an unknown key is refused rather than defaulted. Both janggi teaching positions
also open under xiangqi's rules, and mean something else there.

Inside the crate, the ruleset lives **on the `Position`** rather than being passed
at the call site, and `Position::moves_for` branches once into two bodies that
each read as one game's rules. A position and its rules are not independently
selectable, and one body speckled with `if janggi` would make neither game
readable. Compare `crates/rules-draughts`, which took the opposite route for four
rulesets that differ by four predicates: see [the draughts page](draughts.md).

## What gates it

Two positions, to depth five, with capture and check counts broken out
(`crates/rules-xiangqi/tests/perft.rs`). The opening array reaches 133,312,995
nodes; the "xiangqi Kiwipete", a middlegame with cannons live on both sides,
reaches 53,112,976. Both tables are attributed to two independent lineages
agreeing (a 2009 TalkChess post, and a later verification by two different
applications), which is what makes a table worth gating on.

**The breakdowns matter more here than in chess.** A bare node count says a rule
is wrong, not which one. With the columns separated, a capture-count mismatch
points at the cannon and a check-count mismatch points at the facing generals.
Depth one is 44 nodes, 2 captures, 0 checks, and has a test of its own.

The shallow depths run under `cargo test`; the full ones are `#[ignore]`d and run
with `cargo test --release -- --ignored`. They take about a minute.

Perft gates movement, and **it cannot gate anything below it.** Every repetition
rule is judged over a sequence rather than a position, so no node count can see
it. The header of `perft.rs` says so.

## How a game ends

**Checkmate loses, and so does stalemate.** A player with no legal move loses, as
in draughts and tafl and unlike chess; the two are told apart on the scoresheet,
not in the score. Porting chess's intuition is the common bug.

**Perpetual check loses, for the checker.** WXF Article 20.1, quoted in the module
header: "any player who delivers perpetual checks would be dealt with a loss".
Without it a losing player draws every game in which they can find one repeatable
check. If *both* sides are checking perpetually, it is a draw.

The judgement is in `Game::adjudicate_repetition`. It is the only adjudication in
Bragd's rules that turns on what the repeated moves *were*, not only on which
positions recurred. `Game` carries `checks: Vec<bool>` parallel to `seen`,
recording whether the move that produced each position gave check; a position
alone cannot say who was checking. When a position recurs for the third time, the
cycle is everything back to its previous occurrence. A side that moved in that
cycle and checked on **every one** of its moves has delivered a perpetual check
and loses.

Two numbers in the endings are Bragd's rather than a federation's, and both are
labelled in the code. `REPETITIONS = 3`, where the WXF text says four, because
threefold is what most engines and online sites use. `QUIET_LIMIT = 120` (sixty
moves each without a capture), where the WXF says fifty moves each "after
verification by an arbiter" and other bodies say sixty; neither can be checked by
a program with no arbiter in it.

`Game::play` clears the repetition history on a **capture** and nothing else.
Xiangqi has no irreversible quiet move (no pawn that cannot go back, no castling
right to lose), so a capture is the only thing that makes a position unreachable
again.

## Not done

### Perpetual chase

**The perpetual-chase rules are not implemented, and a repetition that is not a
perpetual check is scored a draw.** The module header of
`crates/rules-xiangqi/src/game.rs`, the doc comment on `DrawReason::Repetition`
and the header of `crates/rules-xiangqi/tests/perft.rs` all say so.

**Where it would live**: the `(false, false)` arm of
`Game::adjudicate_repetition`, which today falls through to `ran_out`. The cycle
is already identified, both sides' moves in it are walked, and each move's
classification is looked up in a parallel vector.

**What is missing** is the rest of the classification. The WXF judges a
repetition by sorting every move in the cycle into check, chase, threat or idle,
and scores perpetual chase as a loss too. Nothing in the crate asks what a move
*threatened*. It has `is_attacked`, and no notion of a piece being **protected**,
which is half of what a chase judgement needs, since chasing a defended piece is
not a chase. So the gap is a missing predicate and the data entry behind it. The
WXF's conformance material is said to run to 110 worked diagrams; that figure has
not been confirmed here.

**What a player meets.** Take this position, Black a horse ahead:

```
4k3r/9/9/9/9/2n6/9/9/9/R2K5 w - - 0 1
```

Red plays `a1c1`, attacking the undefended horse up the c-file. Black moves it,
`c5a6`. Red plays `c1a1`, attacking it again up the a-file. Black moves it back,
`a6c5`. Twice round, the position has occurred three times, and the rules return:

```
Draw { reason: Repetition }
```

Red never gave check. He attacked an undefended piece on every move, and half a
point is what that is worth here. Under WXF adjudication it is a loss for Red.
**The rules never ask what the repeated moves were doing**, and a player meeting
this cannot tell that anything was skipped.

### The test that guards nothing

`stalemate_is_a_loss_and_not_a_draw` in `crates/rules-xiangqi/src/game.rs` wraps
its whole body in `if game.legal_moves().is_empty()`, and its position gives
Black three legal moves (Black is in check from the soldier on e9 and escapes to
d10, f10 or by taking it). The body never runs, so nothing fails if xiangqi stops
scoring stalemate as a loss.

The rules do score stalemate correctly. That was checked against
`4k4/3R1R3/9/9/9/9/9/9/9/3K5 b - - 0 1`, where two red chariots on rank 9 cover
d10, f10 and e9 without attacking e10. It reports no legal moves, not in check,
and `Win { winner: Red, reason: Stalemate }`. Putting that position in the test is
the whole fix.

### Positions the reader accepts that the rules do not

`Position::from_fen_with` refuses a FEN that is malformed, has the wrong number of
rows or files, or has no general. It does **not** refuse a position the rules call
illegal. Both of these open:

- `4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1`, the generals facing, which xiangqi says
  cannot exist. It opens with both sides in check and Red holding two legal moves.
- `4k4/4R4/9/9/9/9/9/9/9/3K5 w - - 0 1`, where the side **not** to move is
  already in check, which no legal game reaches.

Nothing in Bragd should produce either. Teaching positions are replayed by the
test in `crates/bragd-games/src/positions.rs`. An illegal starting position is
latent rather than live, but a FEN can also arrive from a Foreign database, so
it is worth knowing.
