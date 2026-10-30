# Ur and senet

[← Rules](README.md)

**Built**, in `crates/rules-ur` and `crates/rules-senet`. This page is the
specification the two rules crates were built from: the Royal Game of Ur as
Irving Finkel reconstructs it, and senet as Timothy Kendall reconstructs it. Every rule below is stated closely enough to
code from, every point where the sources genuinely disagree is named, and each
disagreement ends in a recommendation with its reason beside it — because a
documented choice can be argued with and a silent one can only be discovered.

## Why this was written before it was coded

Neither game has surviving rules. Ur's early rules were never written down —
the one tablet that exists describes the late form of the game — and senet's
"passed into oblivion" (Piccione's phrase) around the advent of the Christian
era. So there is nothing for the rules to be correct against — the same trap
chaturanga sat in until it was played as a *named reconstruction*. Rules that
implement "senet" cannot be checked, but rules that implement *Kendall's 1978
reconstruction* can be, because the claim is now
specific enough to be wrong. The correctness question stops being "did we
guess history right", which is unanswerable, and becomes "does the code do
what this page says", which is ordinary work.

The same principle sets the limit: the test is whether a *published* reconstruction
can be named and its contested points listed. Both games pass. Ur passes
easily — Finkel is the curator who deciphered the one ancient rules tablet
that exists for any board game, and his ruleset has a rigorous modern
documentation trail (RoyalUr.net, which solved it). Senet passes narrowly —
Kendall's booklet is out of print and scarce, so this document also has to
pin *which transcription* of Kendall it follows, and does.

The document is therefore part of the product, the same way chaturanga's
stated source is: it is where the argument is named, so that a future reader
knows the code is not guessing — it is implementing somebody's argument, and
here is whose, and here is where that argument ran out and ours began.
Hnefatafl set the pattern when it chose the Copenhagen rules: where the named
source is vague, a vague rule still has to do something, so each gap is
closed by a decision that is labelled as ours.

---

## The Royal Game of Ur — Finkel's reconstruction

**The named source.** Irving Finkel, "On the Rules for the Royal Game of Ur",
in *Ancient Board Games in Perspective* (British Museum Press, 2007), 16–32,
building on the tablet BM 33333B written by Itti-Marduk-balatu in 177–176 BC —
the oldest known rules document for any board game. The playable ruleset
below is the one Finkel published with the British Museum's replica set and
demonstrated in the museum's 2017 film with Tom Scott, as documented —
unusually rigorously — by RoyalUr.net, whose "Finkel" ruleset is the one they
strongly solved
(275,827,872 positions). Where this document says "Finkel says", it is that
ruleset; where it says "the paper says", it is the 2007 article.

One honesty note the paper itself insists on. The tablet describes the game
in its *late* form: five differing pieces named after birds, astragal dice,
rosettes as betting stations, entry at fixed houses. The third-millennium
game — the one on Woolley's boards from the Royal Cemetery, the one built
here — is reconstructed from the boards, the equipment found with them
(seven identical counters a side, tetrahedral dice), and the tablet's
tradition read backwards. The paper endorses Kendall's 1982 description of
the route as "the most plausible view of the matter" and confirms from the
tablet that marked squares were lucky, and that a piece could be "knocked
off his square and made to start the journey again". The extra throw on a
rosette, the exact throw to leave, and compulsory movement are Finkel's
playable distillation, not sentences on the tablet. That is the argument
being implemented, and it is his.

### The board, and a scheme to address it

Twenty squares: a block of 4×3, a block of 2×3, and a bridge of two squares
joining them along the middle row. Rows are lettered **A** (one player's
edge), **B** (the middle), **C** (the other player's edge); columns run
**1–8** with column 1 at the far end of the large block, columns 5–6 the
bridge, columns 7–8 the small block. Four cells do not exist: **A5, A6, C5,
C6** — the notches beside the bridge, which is where pieces enter and leave.

```
col:  1    2    3    4    5    6    7    8
A   [A1*][A2 ][A3 ][A4 ]          [A7*][A8 ]   row A: player Light
B   [B1 ][B2 ][B3 ][B4*][B5 ][B6 ][B7 ][B8 ]   row B: shared
C   [C1*][C2 ][C3 ][C4 ]          [C7*][C8 ]   row C: player Dark
```

Squares marked `*` are the five rosettes: **A1, C1, B4, A7, C7**. This is the
decoration on BM 120834 itself and the coordinate set RoyalUr.net's board
model uses; only B4 sits on the shared row.

### The path

Each player's route is **14 squares long**: four private squares up their own
edge row away from the bridge, the whole middle row of eight, then two
private squares back on their own edge of the small block. This is the path
R. C. Bell drew and Finkel's ruleset uses (RoyalUr.net calls it the Bell
path); it is the shape the paper's Kendall quotation describes — in on your
own short row toward the marked corner, out along the middle, and, on the
third-millennium board, around the far corner to your own side and off.

| Path step | Light   | Dark    | Notes                                  |
| --------- | ------- | ------- | -------------------------------------- |
| 1         | A4      | C4      | entry square                           |
| 2         | A3      | C3      |                                        |
| 3         | A2      | C2      |                                        |
| 4         | **A1**  | **C1**  | rosette                                |
| 5         | B1      | B1      | shared from here                       |
| 6         | B2      | B2      |                                        |
| 7         | B3      | B3      |                                        |
| 8         | **B4**  | **B4**  | rosette — the one contested square     |
| 9         | B5      | B5      |                                        |
| 10        | B6      | B6      |                                        |
| 11        | B7      | B7      |                                        |
| 12        | B8      | B8      |                                        |
| 13        | A8      | C8      | private again                          |
| 14        | **A7**  | **C7**  | rosette, last square before exit       |
| off       | —       | —       | exact throw only, see rule 9           |

Two properties fall out of the table and both are load-bearing for an
implementation. **Steps 1–4 and 13–14 are private** — six safe squares per player
that the opponent's path never visits, so a piece there can never be
captured and entry can only be blocked by your own piece. **Both players
traverse B1→B8 in the same direction and the same order**, so a Light piece
at path step *p* (5 ≤ *p* ≤ 12) stands on the same board square as a Dark
piece at the same *p*. A piece's whole state is therefore `hand`, `done`, or
a path step 1–14, and the capture test is equality of step numbers in the
shared range — no board-square translation is ever needed. The board
coordinates above exist for the renderer, not the rules.

### The dice

**Four tetrahedral dice, two of four corners marked on each.** The throw is
the count of marked corners pointing up. Each die shows a marked corner with
probability 1/2, so the throw is binomial B(4, 1/2):

| Throw | Ways | Probability | Effect                    |
| ----- | ---- | ----------- | ------------------------- |
| 0     | 1    | 1/16        | no move — the turn passes |
| 1     | 4    | 4/16        | move one square           |
| 2     | 6    | 6/16        | move two                  |
| 3     | 4    | 4/16        | move three                |
| 4     | 1    | 1/16        | move four                 |

The mean is exactly 2. The paper is the authority for the dice themselves
("two corners are distinguished by inlaid points which provide the score
when they fall uppermost, so that tossing a single tetrahedron gives two
chances out of four"); the count of four thrown together is Finkel's
published set, per RoyalUr.net and the British Museum edition.

### The rules

Seven pieces per player, all starting in hand, board empty. Decide the first
player by lot. Then, each turn:

1. **Throw the four dice.** A throw of 0 moves nothing and the turn passes.
2. **Move exactly the throw** — one piece, the full count, no splitting.
   The move is either a piece already on the board advancing along its path,
   or a piece entering from hand: an entering piece on a throw of *n* lands
   on path step *n* (off the board counts as step 0). A throw of 4 therefore
   enters directly onto the rosette at step 4.
3. **A square holds one piece.** A move whose landing square holds your own
   piece is illegal.
4. **Landing on an opponent's piece captures it** — possible only on shared
   steps 5–12 — and the captured piece returns to its owner's hand to start
   the journey again (the paper: "knocked off his square and made to start
   the journey again").
5. **A piece on a rosette cannot be captured.** Only B4 (step 8) can ever be
   attacked, so in play this is one rule: the central rosette is safe — and
   safety means the move does not exist, so an opponent's piece on B4 also
   *blocks*: a move landing there is illegal, exactly as if it were your
   own piece standing in the way.
6. **Landing on a rosette grants an immediate extra throw**, on any of the
   three rosettes, entry included, chaining without limit.
7. **Moving is compulsory.** If any legal move exists you must play one; you
   choose freely which (capturing is never forced as such). Only when no
   move is legal does the turn pass.
8. There is no voluntary pass and no doubling of any kind.
9. **Bearing off needs the exact throw**: from step *p*, only a throw of
   15 − *p* takes the piece off (Wikipedia's phrasing of Finkel: "exactly
   the number of spaces remaining until the end of the course plus one").
   A larger throw may not be spent on that piece.
10. **The first player to bear off all seven pieces wins.** There is no
    draw; captures can prolong a game but the dice terminate it with
    probability 1, so no ply cap is proposed.

### Where sources genuinely disagree, and what Bragd plays

**The path.** Murray and James Masters argue for longer routes that place a
rosette every fourth step and make more or all of the board contested;
Masters' path is 16 squares, Murray's loops back for 27. **Play Finkel's
14-square path.** It is the named reconstruction's own path, it matches the
route the paper endorses, and it is the path of the solved, documented
ruleset. The longer paths are answers to a real observation — see next —
but they are rival reconstructions, and blending them would un-name ours.

**Whether the central rosette is safe.** Masters objects that protection
makes the *private* rosettes decorative, since no opponent can reach them
anyway — which is true and is the honest cost. **Play it safe (Finkel).**
It is Finkel's stated rule; the paper reads the late game the same way
("Rosette Square: immunity from capture"); and it is what makes step 8 the
board's single fought-over square, which is where this game's middlegame
lives.

**Exact bearing off.** RoyalUr.net's Finkel ruleset and Wikipedia's account
of Finkel both require the exact throw; several popular retellings (and one
of the two second-hand reports of the British Museum leaflet — they
contradict each other) let any sufficient throw end the race. **Require the
exact throw.** It is the better-attested reading of Finkel, and the two
leaflet reports cancelling out is itself the reason to stand on the
documented ruleset rather than a memory of a pamphlet.

**Seven pieces or five.** Woolley's third-millennium boards came with seven
counters a side and the paper says seven; the tablet's late game uses five,
and some commercial sets follow it. **Seven.** Five belongs to the late
bird-race, which is a different game on an evolved board.

**Four dice or three.** Bell and Masters throw three tetrahedra and score an
empty throw as 4, removing the lost turn. **Four dice, 0 = lost turn.** The
excavated equipment and the paper describe the two-marked-corner tetrahedra;
1/4/6/4/1 over sixteen is their distribution, and the 1-in-16 wasted turn is
part of the reconstruction, not a defect to engineer away.

### The sources, and where they are

- Finkel 2007, the paper: mirrored at
  `csclub.uwaterloo.ca/~pbarfuss/On_the_Rules_for_the_Royal_Game_of_Ur.pdf`.
- RoyalUr.net — `royalur.net/rules` (the Finkel ruleset, named variants) and
  `royalur.net/solved`; their open-source model
  (`github.com/RoyalUr/RoyalUr-Python`, `royalur/model/path.py`) carries the
  exact Bell-path coordinates the table above was checked against.
- Wikipedia, "Royal Game of Ur", for Finkel's rules in prose and the
  Murray/Masters objections.
- The Masters/Bell alternative conventions: the royal-ur-rules page at
  `mastersofgames.com`, and Damian Walker's leaflet tradition (cyningstan),
  whose derived text circulates as `genjam.org/.../rules.pdf`.

---

## Senet — Kendall's reconstruction

**The named source.** Timothy Kendall, *Passing Through the Netherworld: The
Meaning and Play of Senet, an Ancient Egyptian Funerary Game* (The Kirk Game
Company, 1978) — a boxed game whose booklet is the reconstruction. The
booklet is out of print and no scan is online, so this document pins a
transcription: **Damian Walker's**, the only complete one (published on his
senet site, archived as `WalkersRulesKendal.pdf` in the Brandeis puzzle
pages), cross-checked against Jean-Marie Lhôte's independent summary
(*Histoire des jeux de société*, 1994, as relayed by Catherine Soubeyrand's
Game Cabinet article). The two agree on every point where they overlap;
Walker preserves the interlocking detail Lhôte compresses. Where a popular
account contradicts both — Wikipedia's current "Kendall" section does, in
ways that match phone apps better than the booklet — this document follows
the transcriptions.

### The board and the path

Thirty squares — "houses" — in three rows of ten, walked boustrophedon: the
path snakes, reversing direction each row. Houses are numbered 1–30 along
the path; the rules crate stores houses, and translate to the grid only to
draw.

```
row 1:   1   2   3   4   5   6   7   8   9  10     (left to right)
row 2:  20  19  18  17  16  15  14  13  12  11     (right to left)
row 3:  21  22  23  24  25  26  27  28  29  30     (left to right)
```

For house *n*: row = ⌈*n*/10⌉; within an odd row the column is
*n* − 10(row − 1), within an even row it is 10 · row − *n* + 1. Consecutive
houses are physically adjacent everywhere, including the bends (10 sits
above 11, 20 above 21). Which long edge is drawn on top is a rendering
choice; the path order is the game.

Five houses carry meanings (glyphs for the board artist in brackets):

| House | Name                                    | In one line              |
| ----- | --------------------------------------- | ------------------------ |
| 15    | House of Rebirth [ankh]                 | where the drowned return |
| 26    | House of Beauty [nfr]                   | the mandatory stop       |
| 27    | House of Water [water]                  | the trap                 |
| 28    | House of the Three Truths [three marks] | exit on exactly 3        |
| 29    | House of Re-Atoum [two marks]           | exit on exactly 2        |
| 30    | the last house [one mark]               | exit on any throw        |

Kendall's booklet calls 27 the House of Humiliation; the water name is the
common one and both refer to the same glyph. House 26 is also rendered as
the House of Happiness — same house, same nfr sign. Piccione confirms the
3–2–1 exit sequence was constant on the boards across three thousand years.

### The throwing sticks

**Four two-faced sticks**, one face plain and one marked. The throw is the
count of counting faces up, 1–4 — and a throw showing none of them scores
**5**. There is no zero. With each face equally likely:

| Throw | Faces up | Ways | Probability | Another throw?    |
| ----- | -------- | ---- | ----------- | ----------------- |
| 1     | 1        | 4    | 4/16        | yes, after moving |
| 2     | 2        | 6    | 6/16        | no                |
| 3     | 3        | 4    | 4/16        | no                |
| 4     | 4        | 1    | 1/16        | yes, after moving |
| 5     | 0        | 1    | 1/16        | yes, after moving |

The mean is 37/16 ≈ 2.31. Which physical face "counts" varies between
retellings of Kendall (Walker counts the light faces, Lhôte the plain);
the structure — 1 to 4, with the empty throw repurposed as 5 — does not,
and the structure is all the rules crate sees. **Throws of 1, 4 and 5 earn
another throw after the move is completed; 2 and 3 end the turn** — and any
water event ends it regardless (below). A throw that produces no move earns
nothing: the repeat is "after otherwise moving" (Walker's words).

### The opening position

**Seven pieces per player, all on the board.** They alternate along houses
1–14: the second player's pieces on the odd houses 1, 3, 5, 7, 9, 11, 13,
the first player's on the even houses 2, 4, 6, 8, 10, 12, 14. **The player
on the even houses throws first.** Kendall colours them white (odds) and
black (evens); the rules crate stores Sides, not colours, and per the glossary a
Side is not a colour. House 15 — the ankh, the "starting square" in the
ancient sense — begins empty.

### Movement

1. **Throw, then move one piece forward** (ascending house numbers) exactly
   the throw. Moving is compulsory: if any forward move is legal, one must
   be played, and the player chooses freely which.
2. **A landing square must be empty or hold a lone enemy.** Illegal
   landings: your own piece; an enemy piece that is *protected*, meaning it
   has at least one piece of its own side on an adjacent house. Adjacency is
   **path adjacency** — consecutive house numbers, bends included; the
   booklet's transcriptions say "stood next to", and the path is the game's
   geometry, so 10–11 and 20–21 protect and the physical grid does not
   enter into it. That reading is ours and is named as such.
3. **Landing on a lone enemy exchanges the two pieces**: the enemy piece is
   placed on the house your piece just left. No piece is ever captured off
   the board by contact — senet removes pieces only at the exit and drowns
   them only in the water.
4. **Passing is free.** Kendall as transcribed has no leaping restriction:
   any number of enemy pieces, in any formation, may be passed over. The
   famous rule that three enemies in a row cannot be passed belongs to
   other reconstructions — see the disagreements.
5. **If no forward move is legal, a backward move must be played**: one
   piece moves the full throw in descending houses, landing under the same
   rules — empty, or a lone enemy, who is then exchanged *forward* onto the
   vacated house. If neither a forward nor a backward move is legal, the
   turn passes. (The pass is our completion — Walker's Kendall text stops
   at the backward rule; Jéquier's parallel clause says the turn is lost,
   and something must happen.)

### The five last houses

6. **House 26 cannot be passed.** No forward move may carry a piece from a
   house below 26 to a house beyond it; the piece must land exactly on 26
   first. This is positional, not a once-per-piece flag: a piece exchanged
   back below 26 must land on it again. (Walker and the Masters text both
   phrase it positionally; Wikipedia's "at least once" flag reading is the
   app tradition. Ours is the positional reading, and the theme agrees —
   sent back means the gauntlet is run again.) Consequence worth saying
   out loud: any piece standing on 26 blockades everything behind it, your
   own included.
7. 26 is **not** safe in Kendall: a lone piece there can be exchanged away
   like any other — and then owes the stop on 26 again.
8. **From 26 a piece continues by exact count**: 1 → 27 (the water), 2 → 28,
   3 → 29, 4 → 30, and **5 → off the board**. That throw of 5 is the only
   way a piece skips the water row entirely.
9. **Houses 28 and 29 do not move forward at all.** A piece there leaves
   the board only on an exact throw — 3 from 28, 2 from 29. If the throw
   does not bear it off and the player has no other legal move anywhere,
   the piece **falls back into the water**: it moves to 27 regardless of
   the count and takes the water penalty. Two occupancy cases close this:
   your own piece cannot be in the water when this fires (a drowned piece
   freezes the army, so the 28/29 piece could not be moving at all), and
   if the water holds the *enemy's* drowned piece, the exchange rule
   applies as everywhere else — that piece is freed onto the vacated 28 or
   29, and yours takes its place in the water.
10. **House 30 bears off on any throw.** A piece there always has a legal
    move, so it is never forced backward.
11. **Exchanges into 28–30 drown the victim.** A lone enemy on 28, 29 or 30
    that is landed on (necessarily from 26) is not placed on the attacker's
    origin: it goes to **27**, the water — unless the water is occupied, in
    which case the ordinary exchange applies and it goes back to 26. Its
    owner takes the water penalty — the freeze starts with their own next
    turn, since they are not on move when the drowning happens — and the
    attacker's turn continues normally, repeat throws included.

### The water, in full

12. **A piece that lands on 27 by its own move ends its owner's turn at
    once** — no repeat throw, whatever was thrown.
13. **While a player has a piece in the water, no other piece of theirs may
    move.** Each turn, the owner chooses one of two actions:
    - **Rescue**: if house 15 is empty, place the piece on 15. This
      consumes the whole turn. If 15 is occupied — by any piece, either
      side's, protected or not — this option does not exist.
    - **Throw**: on an exact **4** the piece bears off (27 + 4 is the
      exact exit, the same 31 − *n* arithmetic as houses 26–29), and the
      4 earns its repeat throw as usual, with the army unfrozen. On any
      other throw nothing moves and the turn is over, with no repeat.
      Walker writes "may be borne off"; it is read here as the only
      available action, since nothing else can move and declining buys
      nothing.

    **The rescue happens after the throw here, and Kendall's text says
    instead of it.** This is a departure and is named as one. Walker's
    transcription makes the rescue an alternative *to* throwing, so a player
    choosing it never learns what the sticks would have said; these rules
    throw first and then offer the rescue whatever came up.

    Two reasons, and only the second is about the game. The structural one is
    that every dice Family has exactly one turn shape — a throw is supplied as
    an entry, then the player acts — and a rule that let one game act *without* a throw would put a second
    shape into the one place every family shares, for a case that arises in a
    few turns of a few games. The one about the game is that a blind
    commitment is a worse decision than an informed one: under the letter, a
    player rescuing on what turns out to have been a four has thrown away the
    exit and cannot know it. The cost is that the rescue is never chosen over
    a four, which under the letter it sometimes would be by accident.
14. **A piece in the water is never protected**, whatever stands beside it.
    A piece standing beside the water still confers protection outward —
    the transcription strips protection from the drowned piece only, and
    this document follows the letter.
15. **Landing on the drowned piece rescues it.** An enemy piece that lands
    on 27 (only reachable deliberately from 26 with a 1, or under
    compulsion) exchanges with it as normal — the drowned piece is freed
    onto the attacker's origin, and the attacker is now in the water, its
    owner taking the penalty from now on. Walker calls this "a nice bonus
    for pieces stuck in the water", and it mostly happens under rule 1's
    compulsion.
16. The freeze cannot deadlock the game: only one piece fits in the water,
    the other player is unaffected, and the drowned piece always has the
    exact-4 exit, so the game ends with probability 1. No ply cap is
    proposed.

### Winning

17. **The first player to bear all seven pieces off the board wins.** There
    is no draw and no scoring of margins.

### Where sources genuinely disagree, and what Bragd plays

**Pieces and layout — and a warning for anyone cross-checking.** It is
easy to find the counts reversed: popular summaries regularly attribute
five pieces on houses 1–10 to Kendall. The transcriptions say the
opposite: **Kendall's booklet gives seven per player on houses 1–14**
(Walker and Lhôte independently), and it is the *other* reconstructions —
Jéquier, the Masters "popular conjecture", Piccione's modern set — that
play five on 1–10. History underwrites both: Piccione
documents seven draughtsmen in the Old Kingdom scenes and five from the
Eighteenth Dynasty on. **Play Kendall's seven on 1–14.** It is the named
reconstruction, and it models the earlier, secular game the way the rest
of his rules do; five-on-ten belongs to the New Kingdom box sets and to
other people's arguments.

**What the water does.** The popular account (Lhôte's compression,
Wikipedia, most apps and museum sheets): landing on 27 sends the piece
straight back to 15, or to the first free house below 15 if it is taken.
Walker's transcription of the booklet: the piece *stays* in the water,
its army frozen, until rescued to 15, borne off on an exact 4, or freed
by an enemy landing on it. **Play Walker's trap.** It is the completest
transcription of the booklet, and the deciding argument is interlock:
Kendall's own special cases — never protected in the water, the
rescue-by-exchange bonus, exchanges into 28–30 drowning the victim, the
28/29 fallback — all *reference a piece sitting in the water*, and the
instant-return version orphans every one of them.

**The three-piece blockade.** "Two adjacent pieces cannot be landed on;
three in a row cannot be passed" is real — its first half is Kendall's
protection rule, but its second half is **Jéquier's** (via Botermans),
kept by Piccione and Skiryuk. Both careful transcriptions of Kendall have
protection only and free passing. **Play Kendall faithfully: no passing
restriction.** Blending in the blockade would be exactly the un-naming
naming a reconstruction exists to prevent; and Kendall's game does not lack teeth for it —
the water and the mandatory stop at 26 are where his game bites.

**The empty throw, and which throws repeat.** Kendall: all-plain scores 5;
1, 4, 5 repeat. Jéquier: all-plain scores 6; 1, 4, 6 repeat. Bell rolls a
die instead. **Kendall's 5 and 1/4/5**, for the same reason as everything
else here.

**The first move.** Kendall: the player on the even houses simply moves
first. Jéquier opens with a throwing race — first to throw a 1 takes the
even pieces, moves the piece on the last starting house one step, and
plays on. **Kendall's flat rule.** Who moves first is then a fact of the
Game like any other seat assignment, stored with the Game.

**Leaving house 30.** Kendall: any throw. Piccione's 3–2–1 reading and the
popular sheets: an exact 1. **Kendall's any-throw**, noted because an
implementation of the exact-1 version by reflex would silently change
how often the endgame stalls.

**Rival reconstructions worth knowing exist**, so nobody mistakes them for
variants of this one: Bell's senet is a different game altogether (pieces
enter on the last five houses and race *backwards* to fill rows one and
two, scoring points); Skiryuk's (implemented at ancientgames.org) is a
modern blend with per-house extra-throw rules on 26–29. Neither is built,
and neither should be blended in.

### Readings this document had to choose

Collected from above so they can be argued with in one place, each labelled
ours where Kendall's transcriptions are silent: path adjacency for
protection (rule 2); the positional reading of the stop at 26 (rule 6); the
pass when no move exists at all (rule 5); no repeat throw without a
completed move (sticks table); the water piece confers protection outward
but never receives it (rule 14); a bear-off 4 from the water keeps its
repeat throw (rule 13); the penalty follows the piece's owner, so a bumped
victim's army freezes and the attacker plays on (rule 11), and an attacker
who lands in the water inherits the trap (rule 15).

### The sources, and where they are

- Kendall 1978, via Walker's transcription and Soubeyrand's Game Cabinet
  article (Lhôte 1994) — both sites are dead, both survive as PDFs under
  `cs.brandeis.edu/~storer/JimPuzzles/GAMES/Senet/SenetReferences/`
  (`WalkersRulesKendal.pdf`, `GameCabinetPage.pdf`, and Walker's Jéquier
  and Bell pages beside them).
- Piccione, "In Search of the Meaning of Senet", *Archaeology* 33 (1980)
  55–58, self-hosted at `piccionep.people.charleston.edu/piccione_senet.pdf`
  — the seven-then-five history and the 3–2–1 sequence.
- Jéquier's rules via Botermans, *The World of Games*, in Walker's
  transcription above — the origin of the blockade and the throw-of-6.
- The popular blend: `mastersofgames.com/rules/senet-rules.htm`; Skiryuk's
  reconstruction at `ancientgames.org/senet`.

---

## What both games ask of the rest of Bragd

Nothing new. A throw is an input, not an event: the rules crate takes it as an
argument, whoever plays the Game throws it, and the record stores it as its own
entry. A Game's record is a list of entries, not moves, and that sentence was
written for exactly this. Ur's throw is drawn from {0…4} with weights 1/4/6/4/1;
senet's from {1…5} with weights 4/6/4/1/1. Both distributions live where the dice
are thrown, never in the rules. Senet's repeat throws and Ur's rosette chains mean
`needs()` must be able to answer "the same side throws again", which backgammon's
doubling already required. Neither game can draw, and neither needs a move limit:
the dice end them, which tafl's four-hundred-ply limit could not say.
