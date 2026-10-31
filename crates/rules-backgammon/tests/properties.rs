//! The correctness gate, and why it is not perft.
//!
//! **There is no perft corpus for backgammon, and the idea does not transfer.**
//! Perft counts the leaves of a move tree from a fixed position, which works
//! because chess's tree is a function of the position alone. Backgammon's is
//! not: every ply branches over twenty-one rolls *before* it branches over
//! turns, so a "node count to depth 5" would be counting a different object, and
//! there is no published file of them for anybody to agree with. The published
//! numbers this game does have — rollout equities — measure a bot's judgement
//! rather than a generator's correctness.
//!
//! So the gate is built rather than downloaded, and it is made of properties:
//! things that must hold in every position a real game can reach, checked over
//! thousands of positions reached by playing. Each one below is a bug this
//! engine could plausibly have.
//!
//! The generator lives **here and only here**. The crate itself has no
//! randomness, must never grow any, and has no dependencies at all — but a
//! harness that plays games needs some, and this file is the
//! line. It is seeded from a constant so any failure is reproducible from its
//! seed.

use rules_backgammon::board::CHECKERS;
use rules_backgammon::game::{Entry, Needed, PlayError, WinReason};
use rules_backgammon::{Destination, Game, Origin, Position, Roll, Side, Turn};

/// SplitMix64, the same generator the tafl harness uses.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn die(&mut self) -> u8 {
        (self.below(6) + 1) as u8
    }

    fn roll(&mut self) -> Roll {
        let (one, two) = (self.die(), self.die());
        Roll::new(one, two).expect("two dice")
    }
}

/// A safety valve for the harness, not a rule of the game. A backgammon game
/// under random play ends in a few hundred plies; anything past this means the
/// engine has stopped making progress, which is itself a failure.
const PLY_CAP: usize = 2000;

/// Plays a game out at random, handing every position and every roll to `check`
/// before the turn is chosen. Returns the finished game.
///
/// **Played the way the platform plays it**, in two steps: ask the game what it
/// wants, throw for it, write the roll down, and only then choose a turn. That
/// is the split the record is built on, so it is the path every property below
/// runs over — a harness that used the combined door would leave the platform's
/// own sequence untested.
fn play_out(seed: u64, check: &mut impl FnMut(&Game, Roll, &[Turn])) -> Game {
    let mut rng = Rng(seed);
    let mut game = Game::new();
    let mut plies = 0;
    while !game.is_over() {
        let mover = game.position().to_move();
        assert_eq!(
            game.needs(),
            Some(Needed::Roll { side: mover }),
            "the game did not ask to be thrown for\n{:?}",
            game.position()
        );
        let roll = rng.roll();
        game.roll(roll).expect("nothing is on the table");
        assert_eq!(
            game.needs(),
            Some(Needed::Turn { side: mover, roll }),
            "the dice are showing and the game does not know it"
        );
        assert_eq!(game.pending_roll(), Some(roll));

        let turns = game.turns();
        assert!(
            !turns.is_empty(),
            "no turn at all for {roll}, not even a pass\n{:?}",
            game.position()
        );
        check(&game, roll, &turns);
        let turn = turns[rng.below(turns.len())].clone();
        game.play_turn(&turn).expect("a turn from the legal list");
        plies += 1;
        assert!(
            plies < PLY_CAP,
            "a game that will not end\n{:?}",
            game.position()
        );
    }
    game
}

#[test]
fn fifteen_checkers_a_side_exist_at_all_times() {
    // The bookkeeping invariant, and the one that catches a hit written to the
    // wrong bar or a bear-off that forgets to take the checker off its point.
    // A checker is on a point, on the bar, or borne off; there is no fourth
    // place, so the three must always add to fifteen.
    for seed in 0..60 {
        play_out(seed, &mut |game, _, _| {
            let position = game.position();
            for side in Side::ALL {
                assert_eq!(
                    position.checkers(side),
                    CHECKERS,
                    "{side} has the wrong number of checkers\n{position:?}"
                );
            }
        });
    }
}

#[test]
fn every_generated_turn_uses_as_many_dice_as_possible() {
    // **The rule that catches implementations out.** It is checked against an
    // independent search rather than against the generator's own filtering: the
    // harness does its own depth-first walk over the dice using only the
    // single-die generator, takes the deepest it can reach, and insists every
    // offered turn is that deep.
    for seed in 100..140 {
        play_out(seed, &mut |game, roll, turns| {
            let deepest = deepest_reachable(game.position(), &roll.dice());
            for turn in turns {
                assert_eq!(
                    turn.dice_used(),
                    deepest,
                    "{turn} spends {} of the {deepest} dice available for {roll}\n{:?}",
                    turn.dice_used(),
                    game.position()
                );
            }
        });
    }
}

/// How many of `dice` can be played in a row, found by trying. Uses only
/// [`Position::plays_for_die`], which is the raw movement generator and knows
/// nothing about the maximum-dice rule.
fn deepest_reachable(position: &Position, dice: &[u8]) -> usize {
    let mut best = 0;
    let mut tried: Vec<u8> = Vec::new();
    for (index, &die) in dice.iter().enumerate() {
        if tried.contains(&die) {
            continue;
        }
        tried.push(die);
        let mut rest = dice.to_vec();
        rest.remove(index);
        for play in position.plays_for_die(die) {
            best = best.max(1 + deepest_reachable(&position.after_play(play), &rest));
        }
    }
    best
}

#[test]
fn when_only_one_die_can_be_used_it_is_always_the_larger() {
    // The other half of the maximum-dice rule, and the half that is silently
    // dropped most often. Checked wherever random play throws up a position
    // that can only spend one die: if the larger one is playable at all, no
    // turn may spend the smaller.
    let mut seen = 0;
    for seed in 200..320 {
        play_out(seed, &mut |game, roll, turns| {
            if roll.is_double() || turns[0].dice_used() != 1 {
                return;
            }
            let position = game.position();
            if position.plays_for_die(roll.high()).is_empty() {
                return;
            }
            seen += 1;
            for turn in turns {
                let play = turn.plays()[0];
                assert!(
                    position.plays_for_die(roll.high()).contains(&play),
                    "{turn} spent the smaller die with the larger one playable\n{position:?}"
                );
            }
        });
    }
    assert!(seen > 0, "random play never produced a one-die turn");
}

#[test]
fn applying_a_turn_hands_the_board_to_the_other_side() {
    // Including the pass. A shut-out player's turn moves nothing, and the one
    // thing it must still do is end.
    for seed in 400..440 {
        play_out(seed, &mut |game, _, turns| {
            let before = game.position().to_move();
            for turn in turns {
                assert_eq!(
                    game.position().after(turn).to_move(),
                    before.other(),
                    "{turn} left the same side to move"
                );
            }
        });
    }
}

#[test]
fn every_generated_turn_round_trips_through_its_notation() {
    // Backgammon's own notation, so a record written in it can be read by
    // somebody who has never seen Bragd — which is only true if what goes
    // out comes back unchanged, `*` and `bar` and `off` included.
    for seed in 500..540 {
        play_out(seed, &mut |_, _, turns| {
            for turn in turns {
                let written = turn.to_string();
                assert_eq!(
                    Turn::parse(&written).as_ref(),
                    Some(turn),
                    "{written} did not read back"
                );
            }
        });
    }
}

#[test]
fn a_turn_never_moves_a_checker_the_wrong_way() {
    // Movement is the part no ruleset disagrees about, so it is the part that
    // can be checked absolutely: every play shortens the mover's journey, by
    // between one and six pips, and a play from the bar starts at twenty-five.
    for seed in 600..640 {
        play_out(seed, &mut |_, roll, turns| {
            for turn in turns {
                for play in turn.plays() {
                    let from = match play.from {
                        Origin::Bar => 25,
                        Origin::Point(number) => number,
                    };
                    match play.to {
                        Destination::Point(number) => {
                            assert!(number < from, "{play} went backwards");
                            assert!(
                                (1..=6).contains(&(from - number)),
                                "{play} is not one die's worth"
                            );
                        }
                        // Bearing off can spend a die larger than the point, so
                        // the only thing to assert is that the point is a home
                        // point at all.
                        Destination::Off => assert!(from <= 6, "{play} bore off from outside home"),
                    }
                }
                assert!(turn.dice_used() <= roll.count());
            }
        });
    }
}

#[test]
fn the_pip_count_only_ever_falls_and_reaches_zero_for_the_winner() {
    // A hit puts a checker back to twenty-five, so a *side's* count can rise —
    // but the mover's own count is strictly non-increasing across its own turn,
    // and the winner's is zero at the end.
    for seed in 700..740 {
        let mut game = Game::new();
        let mut rng = Rng(seed);
        while !game.is_over() {
            let roll = rng.roll();
            let mover = game.position().to_move();
            let before = game.position().pip_count(mover);
            let turns = game.turns_for(roll);
            let turn = turns[rng.below(turns.len())].clone();
            game.play(roll, &turn).expect("legal");
            let after = game.position().pip_count(mover);
            assert!(
                after <= before,
                "{turn} put {mover} further from home, {before} to {after}"
            );
        }
        let winner = game.outcome().expect("a finished game").winner;
        assert_eq!(game.position().pip_count(winner), 0);
        assert_eq!(game.position().off(winner), CHECKERS);
    }
}

#[test]
fn a_game_always_ends_and_never_ends_twice() {
    for seed in 800..860 {
        let mut game = play_out(seed, &mut |_, _, _| {});
        let outcome = game.outcome().expect("a finished game");
        assert!(outcome.points >= 1);
        // And nothing can be played afterwards.
        assert_eq!(game.needs(), None);
        assert!(game.turns_for(Roll::new(3, 1).unwrap()).is_empty());
        assert_eq!(
            game.roll(Roll::new(3, 1).unwrap()),
            Err(PlayError::GameOver)
        );
        assert_eq!(game.play_wire("8/5 6/5"), Err(PlayError::GameOver));
        assert_eq!(game.offer_double(Side::White), Err(PlayError::GameOver));
    }
}

#[test]
fn a_gammon_really_means_the_loser_bore_off_nothing() {
    // Cross-checking the score against the board, because the multipliers are
    // the part of backgammon a rules implementation gets subtly wrong and the
    // part that costs real points.
    let mut kinds = Vec::new();
    for seed in 900..1000 {
        let game = play_out(seed, &mut |_, _, _| {});
        let outcome = game.outcome().expect("a finished game");
        let loser = outcome.winner.other();
        let position = game.position();
        match outcome.reason {
            WinReason::Single => assert!(position.off(loser) > 0, "a single with nothing off"),
            WinReason::Gammon => {
                assert_eq!(position.off(loser), 0);
                assert_eq!(position.bar(loser), 0, "that is a backgammon");
                for number in 1..=6 {
                    assert!(
                        !position.point(outcome.winner, number).held_by(loser),
                        "a checker in the winner's home: that is a backgammon"
                    );
                }
                assert_eq!(outcome.points, 2);
            }
            WinReason::Backgammon => {
                assert_eq!(position.off(loser), 0);
                let stranded = position.bar(loser) > 0
                    || (1..=6).any(|n| position.point(outcome.winner, n).held_by(loser));
                assert!(stranded, "a backgammon with nothing stranded");
                assert_eq!(outcome.points, 3);
            }
            other => panic!("random play produced {other:?}"),
        }
        kinds.push(outcome.reason);
    }
    // Not a rules check — a check that the endings are reachable at all. An
    // engine that could never score a gammon would pass everything above.
    assert!(
        kinds.iter().any(|kind| matches!(kind, WinReason::Single)),
        "no game was ever a plain win"
    );
    assert!(
        kinds.iter().any(|kind| matches!(kind, WinReason::Gammon)),
        "no game was ever a gammon: {kinds:?}"
    );
}

#[test]
fn replaying_a_recorded_game_reproduces_it_exactly() {
    // **The whole reason the rolls are in the record.** Without them a replay
    // would produce a different game, silently, which is the failure a pure
    // rules crate exists to prevent and the reason this crate throws no dice.
    for seed in 1100..1140 {
        let mut game = play_out(seed, &mut |_, _, _| {});
        let replayed = Game::replay(*game.initial_position(), game.entries())
            .expect("a recorded game replays");
        assert_eq!(replayed, game, "the replay diverged");
        assert_eq!(replayed.outcome(), game.outcome());
        assert_eq!(replayed.position(), game.position());

        // **The record is in its split form**, and this is what says so: the
        // entries strictly alternate roll, turn, roll, turn — nothing else can
        // appear in a game played out without the cube — so a turn always has
        // the dice it was played with immediately before it and never glued on
        // to the front of it.
        for pair in game.entries().chunks(2) {
            assert!(
                matches!(pair[0], Entry::Roll(_)),
                "a turn with no dice in front of it: {:?}",
                pair[0]
            );
            assert!(matches!(pair[1], Entry::Turn(_)), "{:?}", pair[1]);
        }
        assert_eq!(game.entries().len() % 2, 0, "a roll nobody played with");

        // And the record survives a trip through the wire, which is how the
        // platform actually stores it: a list of opaque strings.
        let written: Vec<String> = game.entries().iter().map(Entry::to_wire).collect();
        let read: Vec<Entry> = written
            .iter()
            .map(|text| Entry::parse(text).unwrap_or_else(|| panic!("{text} did not parse")))
            .collect();
        assert_eq!(read, game.entries());
        let from_wire =
            Game::replay(*game.initial_position(), &read).expect("the written record replays");
        assert_eq!(from_wire, game);

        // Guard against the test passing vacuously.
        assert!(game.entries().len() > 10);
        game.resign(Side::White, rules_backgammon::Concession::Single)
            .expect_err("a finished game cannot be resigned");
    }
}

/// **A game abandoned with the dice showing still says what they showed.**
///
/// The case the split exists for. Under the old schema a roll only reached the
/// record attached to the turn it was played with, so a player who threw and
/// then walked away left nothing behind — and Bragd could be accused of
/// having rerolled, with nothing to disprove it. Here the roll is the last
/// entry, it replays back as the same pending roll, and the resumed game asks
/// for a turn rather than for a second throw.
#[test]
fn a_game_abandoned_between_the_throw_and_the_turn_remembers_the_throw() {
    let mut rng = Rng(31_337);
    let mut game = Game::new();
    for _ in 0..7 {
        let roll = rng.roll();
        game.roll(roll).expect("thrown");
        let turns = game.turns();
        let turn = turns[rng.below(turns.len())].clone();
        game.play_turn(&turn).expect("legal");
    }
    let showing = rng.roll();
    game.roll(showing).expect("thrown, and then nobody moved");

    let written: Vec<String> = game.entries().iter().map(Entry::to_wire).collect();
    assert_eq!(
        written.last().map(String::as_str),
        Some(showing.to_string()).as_deref()
    );

    let read: Vec<Entry> = written
        .iter()
        .map(|text| Entry::parse(text).expect("an entry"))
        .collect();
    let resumed = Game::replay(*game.initial_position(), &read).expect("it replays");
    assert_eq!(resumed, game);
    assert_eq!(resumed.pending_roll(), Some(showing));
    assert_eq!(
        resumed.needs(),
        Some(Needed::Turn {
            side: resumed.position().to_move(),
            roll: showing
        }),
        "a resumed game must not be thrown for again"
    );
}

#[test]
fn a_game_with_the_cube_in_it_replays_too() {
    // The cube entries are the ones with no side stored on them — the doubler is
    // whoever was to move and the answer comes from the other player — so a
    // replay that got the derivation wrong would be rejected rather than
    // silently wrong. This is what checks it.
    let mut game = Game::new();
    let mut rng = Rng(4242);
    game.offer_double(Side::White).expect("White is to move");
    game.take(Side::Black).expect("Black takes");
    // An odd number of turns, so that it comes round to Black — who now holds
    // the cube and is the only one who may double next.
    for _ in 0..9 {
        let roll = rng.roll();
        let turns = game.turns_for(roll);
        let turn = turns[rng.below(turns.len())].clone();
        game.play(roll, &turn).expect("legal");
    }
    game.offer_double(Side::Black).expect("Black owns the cube");
    game.take(Side::White).expect("White takes");

    assert_eq!(game.cube().value(), 4);
    assert_eq!(game.cube().owner(), Some(Side::White));

    let replayed =
        Game::replay(*game.initial_position(), game.entries()).expect("the record replays");
    assert_eq!(replayed, game);
    assert_eq!(replayed.cube(), game.cube());
}

/// **The blocking rule**, which is the whole shape of the game.
///
/// Two or more of a side's checkers make a point, and nothing of the other side
/// may land there. Not asserted anywhere above, and it is the rule that a
/// movement generator gets wrong most quietly: a turn that lands on a made
/// point is not obviously illegal from a distance, and the game it produces is
/// simply a different game.
#[test]
fn a_checker_never_lands_on_a_point_the_other_side_holds() {
    for seed in 900..940 {
        play_out(seed, &mut |game, _, turns| {
            let mover = game.position().to_move();
            for turn in turns {
                // Walked play by play against the *evolving* board, not against
                // the position at the start of the turn — a point can be made,
                // vacated or hit part-way through a turn, and checking only the
                // starting position would pass turns that are illegal in the
                // middle.
                let mut position = *game.position();
                for play in turn.plays() {
                    if let Destination::Point(number) = play.to {
                        let point = position.point(mover, number);
                        assert!(
                            point.is_open_to(mover),
                            "{play} landed on a point held by the other side"
                        );
                    }
                    position = position.after_play(*play);
                }
            }
        });
    }
}

/// **A hit puts exactly one checker on the bar**, and it is the opponent's.
///
/// The interesting half is the count. A blot is one checker by definition, so a
/// hit that sent two would mean the generator had hit a made point — and a hit
/// that sent none would mean the notation said `*` about nothing, which is
/// worse than it sounds because a Library is written in that notation.
#[test]
fn a_hit_moves_one_checker_and_only_one_to_the_bar() {
    let mut hits = 0;
    for seed in 1000..1040 {
        play_out(seed, &mut |game, _, turns| {
            let mover = game.position().to_move();
            for turn in turns {
                let mut position = *game.position();
                for play in turn.plays() {
                    let before = position.bar(mover.other());
                    // A blot **of the other side's**. `is_blot` only counts
                    // checkers, and a single checker of your own standing on a
                    // point is a blot you are allowed to land on — which is
                    // what the first version of this test got wrong.
                    let hit = match play.to {
                        Destination::Point(number) => {
                            let point = position.point(mover, number);
                            point.owner() == Some(mover.other()) && point.count() == 1
                        }
                        Destination::Off => false,
                    };
                    position = position.after_play(*play);
                    let after = position.bar(mover.other());
                    if hit {
                        hits += 1;
                        assert_eq!(
                            after,
                            before + 1,
                            "{play} hit and sent {} to the bar",
                            after - before
                        );
                    } else {
                        assert_eq!(
                            after, before,
                            "{play} sent somebody to the bar without a hit"
                        );
                    }
                }
            }
        });
    }
    // The property is worthless if no hit ever happened in forty games.
    assert!(hits > 0, "no hit occurred, so nothing above was tested");
}

/// **You must come in off the bar before anything else.**
///
/// The one rule that overrides every other choice on the board, and the one a
/// generator forgets because it is a precondition rather than a move.
#[test]
fn nothing_else_moves_while_a_checker_is_on_the_bar() {
    let mut seen = 0;
    for seed in 1100..1160 {
        play_out(seed, &mut |game, _, turns| {
            let mover = game.position().to_move();
            if game.position().bar(mover) == 0 {
                return;
            }
            seen += 1;
            for turn in turns {
                let mut position = *game.position();
                for play in turn.plays() {
                    if position.bar(mover) > 0 {
                        assert!(
                            matches!(play.from, Origin::Bar),
                            "{play} moved a checker while {mover:?} still had one on the bar"
                        );
                    }
                    position = position.after_play(*play);
                }
            }
        });
    }
    assert!(
        seen > 0,
        "nobody was ever on the bar, so nothing above was tested"
    );
}
