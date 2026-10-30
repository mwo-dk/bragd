//! A position, and the generation of whole turns from a roll.
//!
//! The board is twenty-four [`Point`]s, a bar for each side and a tray of borne-
//! off checkers for each side. Those three places are exhaustive: a checker is
//! on a point, on the bar, or off, and the fifteen-a-side count is what the
//! property tests hold this crate to.
//!
//! **The unit of legality is the turn**, not the move. Two dice give up to two
//! moves and a double gives four, and the rule that catches implementations out
//! is that you must use as many dice as any legal sequence can — so a move that
//! is perfectly legal on its own is forbidden when playing it strands the other
//! die. There is a second half to it that is easy to miss: when only one die can
//! be played at all, it must be **the larger one**. Both are enforced in
//! [`Position::turns_for`], and they are the reason [`Position::plays_for_die`]
//! is documented as not being legality.
//!
//! Everything below is written in the mover's own point numbers (see
//! [`crate::board`]), because that is what the notation uses and translating at
//! the edges is cheaper than translating in the reader's head.

use crate::board::{BAR_POINT, CHECKERS, HOME, POINTS, Point, Side, index_of, number_at};
use crate::dice::Roll;
use crate::turn::{Destination, Origin, Play, Turn};
use core::fmt;
use std::collections::BTreeSet;

/// Where a half-finished turn can go next.
///
/// **The two cases are exclusive, and that is a rule rather than a convenience.**
/// A turn is finished exactly when it cannot be extended: you must use as many
/// dice as any legal sequence can, so "there is something left to play" and "you
/// may stop here" are the same question asked twice. Written as two fields on a
/// struct — a list of next plays and an optional finished turn — a caller would
/// have to know that, and the caller who did not would offer a "done" button
/// halfway through a turn.
///
/// Note what neither case is: *illegal*. That is [`Position::continue_turn`]
/// returning `None`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Continuation {
    /// More has to be played, and these are the plays that may come next —
    /// deduplicated by where the checker comes from and where it goes.
    ///
    /// Never empty: a prefix with no legal continuation is not a prefix of any
    /// legal sequence, and `continue_turn` says `None` to that.
    More(Vec<Play>),
    /// The turn is whole. This is the engine's own spelling of it, which is
    /// what [`crate::Game::play_turn`] records — a caller that reassembled it
    /// from its own taps would be offering a Library a second spelling of one
    /// turn.
    Complete(Turn),
}

/// One side's checkers, for building a position by hand.
#[derive(Clone, Copy, Debug)]
pub struct Layout<'a> {
    /// `(own point number, how many)`.
    pub points: &'a [(u8, u8)],
    pub bar: u8,
    pub off: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    points: [Point; POINTS],
    /// Indexed by [`Side::index`].
    bar: [u8; 2],
    off: [u8; 2],
    to_move: Side,
}

impl Position {
    /// The opening array, White to move.
    ///
    /// Which side moves first is not a property of the board — it is settled by
    /// the opening roll, one die each, which the platform throws. Use
    /// [`Position::starting_with`] once it knows.
    pub fn starting() -> Position {
        Position::starting_with(Side::White)
    }

    pub fn starting_with(to_move: Side) -> Position {
        // Two on the 24-point, five on the 13, three on the 8 and five on the 6,
        // in each player's own numbering — symmetric, and the same array every
        // ruleset of backgammon uses.
        let layout = Layout {
            points: &[(24, 2), (13, 5), (8, 3), (6, 5)],
            bar: 0,
            off: 0,
        };
        Position::from_parts(layout, layout, to_move).expect("the opening array is legal")
    }

    /// Builds a position, checking that each side has fifteen checkers
    /// somewhere and that no point is claimed by both.
    pub fn from_parts(white: Layout, black: Layout, to_move: Side) -> Option<Position> {
        let mut position = Position {
            points: [Point::EMPTY; POINTS],
            bar: [white.bar, black.bar],
            off: [white.off, black.off],
            to_move,
        };
        for (side, layout) in [(Side::White, white), (Side::Black, black)] {
            let mut total = layout.bar + layout.off;
            for &(number, count) in layout.points {
                if !(1..=24).contains(&number) || count == 0 {
                    return None;
                }
                let index = index_of(side, number);
                if !position.points[index].is_empty() {
                    return None;
                }
                position.points[index] = Point::of(side, count);
                total = total.checked_add(count)?;
            }
            if total != CHECKERS {
                return None;
            }
        }
        Some(position)
    }

    pub fn to_move(&self) -> Side {
        self.to_move
    }

    /// The same board with the other side to play. The platform needs this to
    /// seat the opening roll; nothing in the rules uses it.
    pub fn with_to_move(&self, side: Side) -> Position {
        Position {
            to_move: side,
            ..*self
        }
    }

    /// What stands on `side`'s point `number`. The point may of course be the
    /// opponent's — the numbering is a view, not an ownership claim.
    pub fn point(&self, side: Side, number: u8) -> Point {
        self.points[index_of(side, number)]
    }

    /// What stands on an absolute index, for anything drawing a board.
    pub fn point_at(&self, index: usize) -> Point {
        self.points[index]
    }

    pub fn bar(&self, side: Side) -> u8 {
        self.bar[side.index()]
    }

    pub fn off(&self, side: Side) -> u8 {
        self.off[side.index()]
    }

    /// Every checker `side` owns. It is always [`CHECKERS`].
    pub fn checkers(&self, side: Side) -> u8 {
        let on_board: u8 = self
            .points
            .iter()
            .filter(|point| point.held_by(side))
            .map(|point| point.count())
            .sum();
        on_board + self.bar(side) + self.off(side)
    }

    /// How far `side` still has to travel, in pips. Every board shows it, and
    /// nothing in the rules depends on it.
    pub fn pip_count(&self, side: Side) -> u32 {
        let mut pips = u32::from(self.bar(side)) * u32::from(BAR_POINT);
        for index in 0..POINTS {
            if self.points[index].held_by(side) {
                pips += u32::from(number_at(side, index)) * u32::from(self.points[index].count());
            }
        }
        pips
    }

    /// Whether every one of `side`'s checkers is home or already off, which is
    /// what bearing off requires. **A checker on the bar is not home**, and that
    /// is the case a naive "no checkers above the 6-point" test gets wrong.
    pub fn all_home(&self, side: Side) -> bool {
        self.bar(side) == 0 && (HOME + 1..=24).all(|number| !self.point(side, number).held_by(side))
    }

    /// The furthest of `side`'s checkers from home, in their own numbering.
    fn highest_occupied(&self, side: Side) -> Option<u8> {
        (1..=24)
            .rev()
            .find(|&number| self.point(side, number).held_by(side))
    }

    /// Whoever has borne off all fifteen, if anybody has.
    pub fn winner(&self) -> Option<Side> {
        Side::ALL
            .into_iter()
            .find(|&side| self.off(side) == CHECKERS)
    }

    /// Every play the side to move could make with a single die.
    ///
    /// **This is not legality.** A play listed here may still be forbidden,
    /// because legality belongs to the whole turn: taking this one may strand
    /// the other die, and the rules say you must play both if any sequence can.
    /// Use it to light up a board, and use [`Position::turns_for`] to decide
    /// what may be played.
    pub fn plays_for_die(&self, die: u8) -> Vec<Play> {
        let side = self.to_move;
        let mut plays = Vec::new();

        // **The bar comes first, and it comes first absolutely.** A player with
        // a checker on the bar has exactly one kind of move available until it
        // is back on the board.
        if self.bar(side) > 0 {
            let number = BAR_POINT - die;
            let landing = self.point(side, number);
            if landing.is_open_to(side) {
                plays.push(Play {
                    from: Origin::Bar,
                    to: Destination::Point(number),
                    hit: landing.owner() == Some(side.other()),
                });
            }
            return plays;
        }

        let home = self.all_home(side);
        let furthest = self.highest_occupied(side);
        for number in 1..=24u8 {
            if !self.point(side, number).held_by(side) {
                continue;
            }
            if number > die {
                let target = number - die;
                let landing = self.point(side, target);
                if landing.is_open_to(side) {
                    plays.push(Play {
                        from: Origin::Point(number),
                        to: Destination::Point(target),
                        hit: landing.owner() == Some(side.other()),
                    });
                }
                continue;
            }
            // Bearing off. Only with every checker home, and then: an exact die
            // always lifts a checker, while **a larger die lifts one only when
            // nothing sits on a higher point**. That second clause is the rule
            // implementations get wrong, and it is checked against the board as
            // it stands *now* — clearing the 6-point with the first die makes a
            // 6 legal from the 4-point with the second.
            if home && (number == die || furthest == Some(number)) {
                plays.push(Play {
                    from: Origin::Point(number),
                    to: Destination::Off,
                    hit: false,
                });
            }
        }
        plays
    }

    /// The board after one play, **with the same side still to move**.
    ///
    /// A turn is not finished until [`Position::after`] is called; this is the
    /// step inside it, exposed because turn generation and any board that
    /// animates a turn both need it. The `hit` flag on the play is ignored and
    /// recomputed, so a play carrying the wrong flag cannot corrupt the board.
    pub fn after_play(&self, play: Play) -> Position {
        let side = self.to_move;
        let mut next = *self;

        match play.from {
            Origin::Bar => next.bar[side.index()] -= 1,
            Origin::Point(number) => {
                let index = index_of(side, number);
                let count = next.points[index].count();
                next.points[index] = Point::of(side, count - 1);
            }
        }

        match play.to {
            Destination::Off => next.off[side.index()] += 1,
            Destination::Point(number) => {
                let index = index_of(side, number);
                let landing = next.points[index];
                let standing = if landing.owner() == Some(side.other()) {
                    // A blot, and it goes to the bar. There is no other way for
                    // a checker to leave a point it is standing on.
                    next.bar[side.other().index()] += landing.count();
                    0
                } else {
                    landing.count()
                };
                next.points[index] = Point::of(side, standing + 1);
            }
        }

        next
    }

    /// The board after a whole turn, with the other side now to move.
    pub fn after(&self, turn: &Turn) -> Position {
        let mut next = *self;
        for &play in turn.plays() {
            next = next.after_play(play);
        }
        next.to_move = self.to_move.other();
        next
    }

    /// Every legal turn for `roll`.
    ///
    /// Never empty: a player who can do nothing at all has exactly one turn
    /// available, the pass, and returning an empty list would make "shut out"
    /// indistinguishable from "the game is over".
    ///
    /// Two rules are applied on top of the search, and both are about the turn
    /// as a whole:
    ///
    /// 1. **You must use as many dice as any legal sequence can.** Every shorter
    ///    sequence is dropped, which is what forbids the move that looks fine
    ///    and strands the other die.
    /// 2. **If only one die can be played, it must be the larger.** This only
    ///    ever bites when the maximum is one, and only on a roll that is not a
    ///    double.
    pub fn turns_for(&self, roll: Roll) -> Vec<Turn> {
        // The same turn arrives by several routes — most obviously on a double,
        // where four identical dice make every ordering of the same plays a
        // separate sequence. They are one turn (see [`Turn::placements`]).
        let mut seen = BTreeSet::new();
        let mut turns = Vec::new();
        for sequence in self.legal_sequences(roll) {
            let turn = Turn::new(sequence);
            if seen.insert(turn.placements()) {
                turns.push(turn);
            }
        }
        turns
    }

    /// What may be played next, given what has been played so far.
    ///
    /// **The call a board needs and `turns_for` cannot answer.** A player
    /// assembles a turn by touching one checker at a time, and after each touch
    /// the board has to light up the squares that are still open and say whether
    /// the turn is finished. Neither question is "which whole turns are legal":
    /// the first is about one step of a turn and the second is about a turn that
    /// might be shorter than the dice suggest.
    ///
    /// Nothing here re-derives legality. Both answers are read off the same
    /// legal sequences [`Position::turns_for`] is built from, which is the
    /// point: **the two rules that make a turn legal are about the turn as a
    /// whole** — use as many dice as any sequence can, and play the larger die
    /// when only one fits — so a partial-turn API that decided for itself what
    /// may follow would be a second, weaker copy of them, and the copy would be
    /// the one a player was tapping against. A play that is fine on its own and
    /// strands the other die simply never appears in a sequence, so it never
    /// appears here either.
    ///
    /// **Order matters and is not the same as a turn's identity.** A turn is
    /// deduplicated by its placements, because two orderings reach the same
    /// board; a player tapping checkers walks one particular ordering, and the
    /// orderings are not interchangeable — `13/8 8/5` needs the checker to be
    /// on 8 before it can leave it. So this matches against sequences rather
    /// than against turns.
    ///
    /// `None` means what has been played so far cannot be finished into a legal
    /// turn — an illegal tap, or a legal-looking one that strands a die. The
    /// caller refuses it and leaves the board where it was. That is a different
    /// answer from [`Continuation::Complete`], which is "there is nothing left
    /// to play *because you are done*".
    ///
    /// Matched on **placements**, ignoring the `hit` flag, because a caller
    /// describing a tap knows where the checker came from and where it went and
    /// has no business asserting whether a blot was sent to the bar. The turn
    /// handed back carries the engine's own flags.
    pub fn continue_turn(&self, roll: Roll, played: &[Play]) -> Option<Continuation> {
        let so_far: Vec<(Origin, Destination)> =
            played.iter().map(|play| play.placement()).collect();

        let mut next: Vec<Play> = Vec::new();
        let mut complete = None;

        for sequence in self.legal_sequences(roll) {
            if sequence.len() < played.len() {
                continue;
            }
            let starts_the_same = sequence[..played.len()]
                .iter()
                .map(|play| play.placement())
                .eq(so_far.iter().copied());
            if !starts_the_same {
                continue;
            }
            match sequence.get(played.len()) {
                // **Every legal sequence has the same length**, because the
                // maximal-use rule dropped the short ones. So a prefix that
                // reaches the end of one has reached the end of all of them,
                // and finding a whole turn here means there is nothing left to
                // offer — which is what makes the two cases exclusive.
                None => complete = Some(Turn::new(sequence)),
                Some(&play) => {
                    if !next
                        .iter()
                        .any(|already| already.placement() == play.placement())
                    {
                        next.push(play);
                    }
                }
            }
        }

        match complete {
            Some(turn) => Some(Continuation::Complete(turn)),
            None if next.is_empty() => None,
            None => Some(Continuation::More(next)),
        }
    }

    /// Every legal *ordering* of plays for `roll`.
    ///
    /// What [`Position::turns_for`] is built from, before the orderings of one
    /// turn are collapsed into it, and what [`Position::continue_turn`] matches
    /// a half-finished turn against. The two whole-turn rules are applied here
    /// so that there is exactly one place they live.
    fn legal_sequences(&self, roll: Roll) -> Vec<Vec<Play>> {
        let mut maximal: Vec<Vec<(Play, u8)>> = Vec::new();
        self.extend(&roll.dice(), &mut Vec::new(), &mut maximal);

        let longest = maximal.iter().map(Vec::len).max().unwrap_or(0);
        maximal.retain(|sequence| sequence.len() == longest);

        if longest == 1 && !roll.is_double() {
            let the_larger_can_be_played =
                maximal.iter().any(|sequence| sequence[0].1 == roll.high());
            if the_larger_can_be_played {
                maximal.retain(|sequence| sequence[0].1 == roll.high());
            }
        }

        maximal
            .into_iter()
            .map(|sequence| sequence.into_iter().map(|(play, _)| play).collect())
            .collect()
    }

    /// Depth-first over the dice, collecting only sequences that cannot be
    /// extended. The dice are carried alongside the plays because the
    /// larger-die rule needs to know which die a play spent, and a play's
    /// origin and destination do not always say — bearing off from the 4-point
    /// with a 6-4 can be either.
    fn extend(
        &self,
        remaining: &[u8],
        prefix: &mut Vec<(Play, u8)>,
        out: &mut Vec<Vec<(Play, u8)>>,
    ) {
        let mut extended = false;
        let mut tried: Vec<u8> = Vec::new();
        for (position, &die) in remaining.iter().enumerate() {
            if tried.contains(&die) {
                continue;
            }
            tried.push(die);
            let mut rest = remaining.to_vec();
            rest.remove(position);
            for play in self.plays_for_die(die) {
                extended = true;
                prefix.push((play, die));
                self.after_play(play).extend(&rest, prefix, out);
                prefix.pop();
            }
        }
        if !extended {
            out.push(prefix.clone());
        }
    }
}

impl fmt::Debug for Position {
    /// Each side's checkers in that side's own numbering, which is how a player
    /// would read the position back to you. Not a picture of a board: the two
    /// numberings run opposite ways and a drawn board has to pick one, which
    /// makes it misleading for exactly one of the two players.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for side in Side::ALL {
            write!(f, "{side}:")?;
            for number in (1..=24u8).rev() {
                let point = self.point(side, number);
                if point.held_by(side) {
                    write!(f, " {number}({})", point.count())?;
                }
            }
            writeln!(
                f,
                " bar {} off {} pips {}",
                self.bar(side),
                self.off(side),
                self.pip_count(side)
            )?;
        }
        write!(f, "{} to move", self.to_move)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turns(position: &Position, high: u8, low: u8) -> Vec<String> {
        let mut written: Vec<String> = position
            .turns_for(Roll::new(high, low).expect("a roll"))
            .iter()
            .map(Turn::to_string)
            .collect();
        written.sort();
        written
    }

    /// White has one checker that can move and fourteen that cannot: a stack on
    /// the 1-point, which has nowhere to go while a checker sits outside home.
    fn one_mobile_checker(on: u8, black: &[(u8, u8)]) -> Position {
        let black_on_board: u8 = black.iter().map(|&(_, count)| count).sum();
        Position::from_parts(
            Layout {
                points: &[(on, 1), (1, 14)],
                bar: 0,
                off: 0,
            },
            Layout {
                points: black,
                bar: 0,
                off: CHECKERS - black_on_board,
            },
            Side::White,
        )
        .expect("a legal position")
    }

    #[test]
    fn the_opening_array_is_fifteen_a_side_and_a_hundred_and_sixty_seven_pips() {
        let start = Position::starting();
        for side in Side::ALL {
            assert_eq!(start.checkers(side), CHECKERS);
            assert_eq!(start.bar(side), 0);
            assert_eq!(start.off(side), 0);
            // The number every backgammon player knows by heart, and the
            // cheapest possible check that the array is the right one.
            assert_eq!(start.pip_count(side), 167, "{side}");
        }
        assert_eq!(start.to_move(), Side::White);
        assert!(!start.all_home(Side::White));
    }

    #[test]
    fn the_two_sides_start_facing_each_other() {
        // White's 24-point is Black's 1-point, so the two back checkers of each
        // side start in the other's home board. If the mirror were wrong they
        // would start on the same point and the position would not build.
        let start = Position::starting();
        assert_eq!(start.point(Side::White, 24).owner(), Some(Side::White));
        assert_eq!(start.point(Side::Black, 1).owner(), Some(Side::White));
        assert_eq!(start.point(Side::White, 6).count(), 5);
        assert_eq!(start.point(Side::Black, 6).count(), 5);
    }

    #[test]
    fn you_must_use_both_dice_if_any_sequence_can() {
        // White's only mobile checker is on the 13-point, rolling 6-5. Black has
        // made White's 7-point, so the 6 cannot be played first at all. Play the
        // 5 first and the 6 follows: 13/8, then 8/2.
        //
        // The rule under test is what happens to `13/8` on its own. It is a
        // perfectly legal move that a naive generator offers, and it is against
        // the rules, because a sequence exists that uses both dice.
        let position = one_mobile_checker(13, &[(18, 2), (6, 13)]);
        assert_eq!(turns(&position, 6, 5), vec!["13/8 8/2"]);
    }

    #[test]
    fn when_only_one_die_can_be_used_it_must_be_the_larger() {
        // The same shape, except Black has made White's 2-point instead. Now
        // neither order gets both dice in: after 13/8 the 6 is dead, and after
        // 13/7 the 5 is dead. Two one-die turns exist and only the one spending
        // the 6 is legal.
        let position = one_mobile_checker(13, &[(23, 2), (6, 13)]);
        assert_eq!(turns(&position, 6, 5), vec!["13/7"]);
    }

    #[test]
    fn a_shut_out_player_has_one_turn_and_it_is_a_pass() {
        // Black has made all six points of Black's home board, which is where a
        // White checker on the bar has to enter, so no die brings it in. There
        // is still exactly one turn, and returning none would make this
        // indistinguishable from a finished game.
        let position = Position::from_parts(
            Layout {
                points: &[(13, 14)],
                bar: 1,
                off: 0,
            },
            Layout {
                points: &[(1, 2), (2, 2), (3, 2), (4, 2), (5, 2), (6, 2)],
                bar: 0,
                off: 3,
            },
            Side::White,
        )
        .expect("a legal position");

        for die in 1..=6 {
            assert!(
                position.plays_for_die(die).is_empty(),
                "entered with a {die}"
            );
        }
        assert_eq!(turns(&position, 6, 5), vec!["(no play)"]);
        assert!(
            position.turns_for(Roll::new(3, 3).unwrap())[0].is_pass(),
            "a double is no help against a closed board"
        );
    }

    #[test]
    fn a_checker_on_the_bar_comes_in_before_anything_else() {
        // White has a checker on the bar and a stack that would love to move.
        // Nothing but an entry is on offer, and the entry lands on the point
        // numbered 25 minus the die.
        let position = Position::from_parts(
            Layout {
                points: &[(13, 14)],
                bar: 1,
                off: 0,
            },
            Layout {
                points: &[(6, 15)],
                bar: 0,
                off: 0,
            },
            Side::White,
        )
        .expect("a legal position");

        for die in 1..=6 {
            for play in position.plays_for_die(die) {
                assert_eq!(play.from, Origin::Bar, "{play} left the bar behind");
            }
        }
        // The entry lands on the point numbered twenty-five minus the die, and
        // a 6 does not enter at all here because Black is standing on the
        // 19-point.
        assert_eq!(position.plays_for_die(1)[0].to, Destination::Point(24));
        assert_eq!(position.plays_for_die(5)[0].to, Destination::Point(20));
        assert!(position.plays_for_die(6).is_empty());

        // A whole turn behaves the same way: whatever else it does, it starts
        // by coming in.
        for turn in position.turns_for(Roll::new(5, 5).unwrap()) {
            assert_eq!(turn.plays()[0].from, Origin::Bar, "{turn}");
            assert_eq!(turn.dice_used(), 4, "{turn}");
        }
    }

    #[test]
    fn a_blot_is_hit_to_the_bar() {
        let position = Position::from_parts(
            Layout {
                points: &[(13, 15)],
                bar: 0,
                off: 0,
            },
            // A lone Black checker on White's 11-point — Black's 14-point.
            Layout {
                points: &[(14, 1), (6, 14)],
                bar: 0,
                off: 0,
            },
            Side::White,
        )
        .expect("a legal position");

        let hit = Turn::parse("13/11*").expect("a turn");
        assert!(
            position
                .turns_for(Roll::new(2, 2).unwrap())
                .iter()
                .any(|turn| turn.plays()[0] == hit.plays()[0]),
            "the blot was not offered as a target"
        );

        let after = position.after_play(hit.plays()[0]);
        assert_eq!(after.bar(Side::Black), 1);
        assert_eq!(after.point(Side::White, 11).owner(), Some(Side::White));
        assert_eq!(after.point(Side::White, 11).count(), 1);
        // And nothing was created or destroyed by the hit.
        assert_eq!(after.checkers(Side::Black), CHECKERS);
        assert_eq!(after.checkers(Side::White), CHECKERS);
    }

    #[test]
    fn a_higher_die_bears_off_only_when_nothing_sits_higher() {
        // One checker on the 6-point, two on the 4-point. A 5 may not lift a
        // checker off the 4-point, because the 6-point is higher — it must play
        // 6/1 instead. This is the rule implementations get wrong.
        let blocked = Position::from_parts(
            Layout {
                points: &[(6, 1), (4, 2)],
                bar: 0,
                off: 12,
            },
            Layout {
                points: &[(6, 15)],
                bar: 0,
                off: 0,
            },
            Side::White,
        )
        .expect("a legal position");
        assert_eq!(
            blocked
                .plays_for_die(5)
                .iter()
                .map(Play::to_string)
                .collect::<Vec<_>>(),
            vec!["6/1"]
        );

        // Take the 6-point checker away and the same 5 lifts one off the
        // 4-point, because now nothing sits higher.
        let clear = Position::from_parts(
            Layout {
                points: &[(4, 2)],
                bar: 0,
                off: 13,
            },
            Layout {
                points: &[(6, 15)],
                bar: 0,
                off: 0,
            },
            Side::White,
        )
        .expect("a legal position");
        assert!(
            clear
                .plays_for_die(5)
                .contains(&Play::new(Origin::Point(4), Destination::Off)),
            "nothing is higher than the 4-point any more"
        );
    }

    #[test]
    fn an_exact_die_always_bears_off_however_much_sits_higher() {
        // The counterpart, and the mistake made in the other direction: the
        // "nothing higher" clause applies to a *larger* die only. A 4 lifts a
        // checker off the 4-point with a full stack on the 6-point behind it.
        let position = Position::from_parts(
            Layout {
                points: &[(6, 5), (4, 2)],
                bar: 0,
                off: 8,
            },
            Layout {
                points: &[(6, 15)],
                bar: 0,
                off: 0,
            },
            Side::White,
        )
        .expect("a legal position");
        assert!(
            position
                .plays_for_die(4)
                .contains(&Play::new(Origin::Point(4), Destination::Off))
        );
    }

    #[test]
    fn bearing_off_needs_every_checker_home_and_the_bar_empty() {
        let outfield = Position::from_parts(
            Layout {
                points: &[(8, 1), (4, 2)],
                bar: 0,
                off: 12,
            },
            Layout {
                points: &[(6, 15)],
                bar: 0,
                off: 0,
            },
            Side::White,
        )
        .expect("a legal position");
        assert!(!outfield.all_home(Side::White));
        assert!(
            !outfield
                .plays_for_die(4)
                .iter()
                .any(|play| play.to == Destination::Off),
            "a checker on the 8-point is not home"
        );

        // And a checker on the bar is not home either, which a plain "nothing
        // above the 6-point" test would miss.
        let on_the_bar = Position::from_parts(
            Layout {
                points: &[(4, 2)],
                bar: 1,
                off: 12,
            },
            Layout {
                points: &[(6, 15)],
                bar: 0,
                off: 0,
            },
            Side::White,
        )
        .expect("a legal position");
        assert!(!on_the_bar.all_home(Side::White));
    }

    #[test]
    fn a_double_is_four_moves() {
        let position = Position::from_parts(
            Layout {
                points: &[(13, 15)],
                bar: 0,
                off: 0,
            },
            Layout {
                points: &[(6, 15)],
                bar: 0,
                off: 0,
            },
            Side::White,
        )
        .expect("a legal position");
        for turn in position.turns_for(Roll::new(3, 3).unwrap()) {
            assert_eq!(turn.dice_used(), 4, "{turn} is short");
        }
    }

    #[test]
    fn applying_a_turn_hands_the_board_to_the_other_side() {
        let start = Position::starting();
        let turn = start.turns_for(Roll::new(3, 1).unwrap())[0].clone();
        let after = start.after(&turn);
        assert_eq!(after.to_move(), Side::Black);
        // And a single play does not, which is the distinction that stops a
        // half-played turn being handed to the opponent.
        assert_eq!(start.after_play(turn.plays()[0]).to_move(), Side::White);
    }

    #[test]
    fn the_best_opening_move_is_available() {
        // 3-1 makes the 5-point, which is the one opening every book agrees
        // about. If the numbering were mirrored the wrong way this would not be
        // generated at all.
        let start = Position::starting();
        let expected = Turn::parse("8/5 6/5").expect("a turn").placements();
        assert!(
            start
                .turns_for(Roll::new(3, 1).unwrap())
                .iter()
                .any(|turn| turn.placements() == expected),
            "8/5 6/5 is not on offer"
        );
    }

    // --- Assembling a turn one checker at a time -----------------------------

    /// What `continue_turn` offers next, written the way a play is written.
    fn next_plays(position: &Position, roll: (u8, u8), played: &[&str]) -> Vec<String> {
        let plays: Vec<Play> = played.iter().map(|text| play(text)).collect();
        match position.continue_turn(Roll::new(roll.0, roll.1).expect("a roll"), &plays) {
            Some(Continuation::More(plays)) => plays.iter().map(Play::to_string).collect(),
            Some(Continuation::Complete(turn)) => panic!("already complete: {turn}"),
            None => panic!("{played:?} cannot be finished"),
        }
    }

    /// One play from its notation, so the tests read as a board does.
    fn play(text: &str) -> Play {
        let turn = Turn::parse(text).unwrap_or_else(|| panic!("{text} is not a play"));
        assert_eq!(turn.plays().len(), 1, "{text} is more than one play");
        turn.plays()[0]
    }

    /// The opening: every first tap is on offer, and the turn is not finished
    /// after one of them.
    #[test]
    fn a_turn_being_assembled_offers_its_first_play_and_is_not_yet_complete() {
        let start = Position::starting();
        let first = next_plays(&start, (3, 1), &[]);
        assert!(first.contains(&"8/5".to_string()), "{first:?}");
        assert!(first.contains(&"6/5".to_string()), "{first:?}");

        // Having played 8/5, what remains is the plays that finish a legal turn
        // — and making the 5-point is one of them.
        let after = next_plays(&start, (3, 1), &["8/5"]);
        assert!(after.contains(&"6/5".to_string()), "{after:?}");
    }

    /// **Finished exactly when it cannot be extended**, which is the maximal-use
    /// rule read the other way round. A board that offered a "done" button on
    /// any non-empty turn would let a player strand a die.
    #[test]
    fn a_turn_is_complete_only_once_every_playable_die_is_spent() {
        let start = Position::starting();
        let roll = Roll::new(3, 1).expect("a roll");
        assert!(
            matches!(
                start.continue_turn(roll, &[play("8/5")]),
                Some(Continuation::More(_))
            ),
            "one die of two is not a turn"
        );
        let both = [play("8/5"), play("6/5")];
        let Some(Continuation::Complete(turn)) = start.continue_turn(roll, &both) else {
            panic!("both dice spent is a whole turn");
        };
        assert_eq!(turn.to_string(), "8/5 6/5");
        // And the turn that comes back is one `play_turn` accepts, spelled the
        // engine's way rather than the caller's.
        assert!(
            start
                .turns_for(roll)
                .iter()
                .any(|legal| legal.placements() == turn.placements())
        );
    }

    /// **The whole reason this is not "which single plays are legal".**
    ///
    /// White's one mobile checker stands on the 10-point and Black holds
    /// White's 2-point. With 6-2 the checker can play either die on its own —
    /// 10/4 or 10/8 — and neither order plays both, because both orders finish
    /// on the blocked 2-point. So only one die may be used, and the rules say
    /// it must be the larger.
    ///
    /// `10/8` is therefore a perfectly good play that is against the rules, and
    /// a board offering the single-die list would light it up. It is not
    /// offered here, and a caller that sends it anyway is refused outright
    /// rather than left half-committed.
    #[test]
    fn a_first_play_that_strands_the_other_die_is_never_offered() {
        // Black's 23-point is White's 2-point: the two numberings run opposite
        // ways, so a wall is built in the other player's numbers.
        let position = one_mobile_checker(10, &[(23, 2)]);
        let roll = Roll::new(6, 2).expect("a roll");

        assert_eq!(
            position.plays_for_die(2),
            vec![play("10/8")],
            "the smaller die is playable on its own, which is the trap"
        );
        assert_eq!(next_plays(&position, (6, 2), &[]), ["10/4"]);
        assert_eq!(
            position.continue_turn(roll, &[play("10/8")]),
            None,
            "playing it would strand the 6"
        );
    }

    /// A tap that cannot be finished is refused outright, which is a different
    /// answer from "nothing left to play".
    #[test]
    fn a_half_turn_that_cannot_be_finished_is_refused() {
        let start = Position::starting();
        let roll = Roll::new(3, 1).expect("a roll");
        assert_eq!(
            start.continue_turn(roll, &[play("13/12"), play("13/12")]),
            None,
            "only one 1 was thrown"
        );
        assert_eq!(
            start.continue_turn(roll, &[play("6/2")]),
            None,
            "not a play either die makes"
        );
    }

    /// **The pass is complete before anything is played**, which is what a board
    /// needs to know to offer the one button there is. Empty is never "wait".
    #[test]
    fn a_shut_out_player_is_complete_immediately() {
        // White is on the bar and Black has closed its home board — which is
        // White's 19- to 24-points, so every die enters onto a point Black
        // holds and none of them enters at all.
        let position = Position::from_parts(
            Layout {
                points: &[(1, 14)],
                bar: 1,
                off: 0,
            },
            Layout {
                points: &[(1, 2), (2, 2), (3, 2), (4, 2), (5, 2), (6, 2), (8, 3)],
                bar: 0,
                off: 0,
            },
            Side::White,
        )
        .expect("a legal position");
        let roll = Roll::new(6, 5).expect("a roll");
        assert_eq!(
            position.continue_turn(roll, &[]),
            Some(Continuation::Complete(Turn::pass()))
        );
    }

    /// **Order is not a turn's identity but it is a turn's legality.** `13/8 8/5`
    /// needs the checker to reach the 8-point before it can leave it, so the two
    /// plays cannot be tapped in the other order — and a partial-turn API that
    /// matched against deduplicated *turns* would have allowed it.
    #[test]
    fn the_order_the_checkers_are_tapped_in_has_to_be_legal() {
        // A single checker on the 13-point and nothing on the 8-point, so 8/5
        // is only reachable after 13/8 has put a checker there.
        let position = Position::from_parts(
            Layout {
                points: &[(13, 1), (1, 14)],
                bar: 0,
                off: 0,
            },
            Layout {
                points: &[(6, 15)],
                bar: 0,
                off: 0,
            },
            Side::White,
        )
        .expect("a legal position");
        let roll = Roll::new(5, 3).expect("a roll");
        assert!(
            matches!(
                position.continue_turn(roll, &[play("13/8"), play("8/5")]),
                Some(Continuation::Complete(_))
            ),
            "13/8 then 8/5 is the turn"
        );
        assert_eq!(
            position.continue_turn(roll, &[play("8/5")]),
            None,
            "there is no checker on the 8-point to move first"
        );
    }

    /// Every prefix of every legal turn is itself acceptable, and every play
    /// offered leads somewhere. The property that says the two halves of this
    /// API agree with `turns_for` rather than merely resembling it.
    #[test]
    fn every_legal_turn_can_be_assembled_one_play_at_a_time() {
        let boards = [
            Position::starting(),
            one_mobile_checker(24, &[(21, 2), (18, 2)]),
            Position::from_parts(
                Layout {
                    points: &[(6, 3), (4, 2), (2, 1)],
                    bar: 0,
                    off: 9,
                },
                Layout {
                    points: &[(6, 15)],
                    bar: 0,
                    off: 0,
                },
                Side::White,
            )
            .expect("a legal position"),
        ];
        for position in boards {
            for high in 1..=6 {
                for low in 1..=high {
                    let roll = Roll::new(high, low).expect("a roll");
                    for turn in position.turns_for(roll) {
                        for taken in 0..turn.plays().len() {
                            let prefix = &turn.plays()[..taken];
                            let Some(Continuation::More(offered)) =
                                position.continue_turn(roll, prefix)
                            else {
                                panic!("{turn} is legal but its first {taken} plays are not");
                            };
                            assert!(
                                offered
                                    .iter()
                                    .any(|play| play.placement()
                                        == turn.plays()[taken].placement()),
                                "{turn}: after {taken} plays, {} was not offered",
                                turn.plays()[taken]
                            );
                        }
                        assert!(
                            matches!(
                                position.continue_turn(roll, turn.plays()),
                                Some(Continuation::Complete(_))
                            ),
                            "{turn} is legal but never finishes"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_position_that_does_not_add_up_is_not_a_position() {
        let short = Layout {
            points: &[(6, 5)],
            bar: 0,
            off: 0,
        };
        let full = Layout {
            points: &[(6, 15)],
            bar: 0,
            off: 0,
        };
        assert!(Position::from_parts(short, full, Side::White).is_none());
        // And two sides cannot stand on one point.
        assert!(
            Position::from_parts(
                Layout {
                    points: &[(6, 15)],
                    bar: 0,
                    off: 0
                },
                Layout {
                    points: &[(19, 15)],
                    bar: 0,
                    off: 0
                },
                Side::White,
            )
            .is_none(),
            "White's 6-point is Black's 19-point"
        );
    }
}
