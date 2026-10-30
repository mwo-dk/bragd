# The cheap chess variants

[← Rules](README.md)

Seven Variants live on `crates/rules-chess`, in three tiers by what they cost.
None needs a new crate, and all are 8×8. **Two of the seven are built**: King of
the Hill and Racing Kings. The entries below say so where they are, so that
nobody builds one twice.

## Tier one: a termination hook each

**King of the Hill. Built.** Win by getting your king to one of the four centre
squares. The estimate was one extra check in `outcome`, position-local and
fitting the `Variant` trait without a new method. The trait did not grow, but the
estimate was low. `crates/rules-chess/src/variant.rs` writes `outcome` out in
full rather than delegating to `Chess::outcome`, because chess's
insufficient-material arm would end a live race as a draw: two lone kings are
dead in chess and a game here, since either king can walk to the middle. So it
overrides three methods: `outcome`, `is_insufficient_material` (always false)
and `can_possibly_mate` (always true).

**Three-check.** Win by giving check three times. **Not position-local**: it
needs a count carried in the Game, like the repetition rules, and the count has
to survive a FEN round trip. **Not built.**

**Racing Kings. Built.** Both kings race to the eighth rank, and no check is
allowed at all. The estimate held: "no checks" is a filter over generated moves,
and the starting array is its own.

## Tier two: rewrites move generation

**Atomic.** A capture destroys a 3×3 area. Every capture becomes a multi-square
update, and the king can die without being checkmated, so `outcome` changes
shape as well.

**Horde.** One side is all pawns. The pawns may double-step from the first rank,
so the pawn generator needs a variant branch. That is exactly what shatranj
avoided by giving its pieces their own roles, so it needs the same care.

**Antichess.** Captures are compulsory and losing all your pieces wins.
Compulsory capture inverts the generator: generate captures, and only if there
are none generate the rest.

## Tier three: new state entirely

**Crazyhouse.** Captured pieces change sides and can be dropped. That is new
state (the hands), a new move category and new notation. The widening of `Move`
it was once costed with is **already done**: `MoveKind` in
`crates/rules-chess/src/chess_move.rs` is a five-bit field running to code 19,
widened so a pawn could promote to a hawk or an elephant. Drops would still need
kinds of their own, but there is room for them.

## What they share

These are cheap to build. Each one is still a Variant somebody has to want to
play, so the next one waits until a player asks for it.
