//! The correctness gate.
//!
//! **There is no perft corpus for Ur and there cannot usefully be one.** Perft
//! counts the leaves of a tree that is a function of the position alone; this
//! tree branches over five throws before it branches over moves, so a node count
//! would be counting a different object — and there is no published corpus of
//! them to compare against anyway. The same argument backgammon makes,
//! and the same answer tafl reached from the other direction: the gate
//! is properties, checked over thousands of games played by a generator that
//! lives here and not in the library.
//!
//! The generator is a SplitMix64 seeded from a constant, so a failure is
//! reproducible from its seed. `crates/rules-ur` itself has no dependencies and no
//! randomness; this file is where that line is drawn.

use rules_ur::board::{PATH, is_shared, square_at};
use rules_ur::{Entry, Game, Needed, PIECES, Position, SAFE, SQUARES, Side, Spot, Throw, WinReason};

/// The dice the library refuses to own.
struct Dice(u64);

impl Dice {
    fn new(seed: u64) -> Dice {
        Dice(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Four dice, each showing a mark or not — which is what makes the 1-4-6-4-1
    /// distribution rather than a uniform one over nought to four.
    fn throw(&mut self) -> Throw {
        let bits = self.next();
        let marks = (0..4).map(|die| ((bits >> die) & 1) as u8).sum();
        Throw::new(marks).expect("four dice cannot show more than four marks")
    }

    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }
}

/// Plays one game to its end, or until the cap, checking everything that must be
/// true at every step along the way. Returns the finished game.
fn play_a_game(seed: u64) -> Game {
    let mut dice = Dice::new(seed);
    let first = if seed.is_multiple_of(2) {
        Side::White
    } else {
        Side::Black
    };
    let mut game = Game::new(first);

    // A cap so a stuck game fails loudly rather than hanging the suite. A real
    // game is a few hundred entries; ten thousand is far past any of them.
    let cap = 10_000;
    let mut steps = 0;

    while !game.is_over() && steps < cap {
        steps += 1;
        match game.needs() {
            Needed::Throw { side } => {
                assert_eq!(
                    game.position().to_move(),
                    side,
                    "needs and the position disagree on the turn"
                );
                assert!(
                    game.moves().is_empty(),
                    "there are moves offered before a throw"
                );
                let throw = dice.throw();
                game.throw(throw)
                    .expect("a throw should be accepted when one is owed");
            }
            Needed::Move { side } => {
                assert_eq!(game.position().to_move(), side);
                let moves = game.moves();
                assert!(!moves.is_empty(), "a move is owed but none is offered");
                let chosen = moves[dice.below(moves.len())];
                game.play(chosen.chosen)
                    .expect("a move taken from moves() should be legal");
            }
            Needed::Nothing => unreachable!("the loop guard already checked this"),
        }
        check_the_position(&game);
    }

    assert!(
        steps < cap,
        "seed {seed} did not finish within {cap} entries"
    );
    game
}

/// Everything that must be true of any position this engine can reach.
fn check_the_position(game: &Game) {
    let position = game.position();

    for side in [Side::White, Side::Black] {
        // **Material never changes.** Seven pieces, always, wherever they are.
        // A capture moves a piece back; it never removes one.
        let pieces = position.pieces(side);
        assert_eq!(
            pieces.len(),
            PIECES as usize,
            "{side:?} does not have seven pieces"
        );

        // Every piece is somewhere legal.
        for &spot in pieces {
            if let Spot::Step(step) = spot {
                assert!(
                    step >= 1 && step as usize <= PATH,
                    "{side:?} has a piece at step {step}"
                );
            }
        }

        // **No two of a side's pieces share a square.** Waiting and Off may
        // repeat; a step may not.
        let mut steps: Vec<u8> = pieces
            .iter()
            .filter_map(|spot| match spot {
                Spot::Step(step) => Some(*step),
                _ => None,
            })
            .collect();
        let before = steps.len();
        steps.sort_unstable();
        steps.dedup();
        assert_eq!(steps.len(), before, "{side:?} has two pieces on one square");

        // The counts agree with the board.
        let on_board = pieces.iter().filter(|s| matches!(s, Spot::Step(_))).count() as u8;
        assert_eq!(
            on_board + position.waiting(side) + position.off(side),
            PIECES
        );
    }

    // **No square holds two pieces.** This is the cross-side version, and it is
    // the one a capture could break.
    for square in 0..SQUARES {
        let holders = [Side::White, Side::Black]
            .into_iter()
            .filter(|&side| {
                position
                    .pieces(side)
                    .iter()
                    .any(|&spot| square_at(side, spot) == Some(square))
            })
            .count();
        assert!(holders <= 1, "square {square} holds pieces of both sides");
    }

    // **Nothing but a shared square can hold an enemy, so nothing but a shared
    // square can ever have been captured on.** Checked as a standing property of
    // the board rather than of the last move.
    for side in [Side::White, Side::Black] {
        for &spot in position.pieces(side) {
            if let Some(square) = square_at(side, spot)
                && !is_shared(square) {
                    assert_eq!(
                        position.occupant(square),
                        Some(side),
                        "a private square is occupied by the wrong side"
                    );
                }
        }
    }

    // The game is over exactly when somebody is home.
    match position.winner() {
        Some(winner) => assert_eq!(position.off(winner), PIECES),
        None => {
            for side in [Side::White, Side::Black] {
                assert!(position.off(side) < PIECES);
            }
        }
    }

    // A finished game offers nothing, and an unfinished one always has something
    // owed. **A game that ends and never ends twice** is the property that keeps
    // a game server from hanging.
    if game.is_over() {
        assert_eq!(game.needs(), Needed::Nothing);
    } else {
        assert!(
            game.needs().side().is_some(),
            "an unfinished game owes nobody anything"
        );
    }
}

#[test]
fn a_thousand_games_hold_every_property() {
    let mut finished = 0;
    for seed in 0..1_000u64 {
        let game = play_a_game(seed.wrapping_mul(0x2545_F491_4F6C_DD1D).wrapping_add(1));
        assert!(game.is_over());
        finished += 1;
    }
    // The loop above can run zero times if the range is ever edited; assert the
    // count so that a silent no-op fails.
    assert_eq!(finished, 1_000, "the property loop did not run");
}

#[test]
fn every_game_replays_to_exactly_itself() {
    let mut checked = 0;
    for seed in 0..300u64 {
        let game = play_a_game(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(7));
        let replayed = Game::replay(game.initial_position().clone(), game.entries())
            .expect("a game this engine produced must replay");

        assert_eq!(
            replayed.position(),
            game.position(),
            "seed {seed} replayed to a different board"
        );
        assert_eq!(
            replayed.entries(),
            game.entries(),
            "seed {seed} replayed to a different record"
        );
        assert_eq!(
            replayed.outcome(),
            game.outcome(),
            "seed {seed} replayed to a different result"
        );
        assert_eq!(replayed.needs(), game.needs());
        checked += 1;
    }
    assert_eq!(checked, 300, "the replay loop did not run");
}

#[test]
fn every_record_survives_the_wire_as_text() {
    let mut checked = 0;
    for seed in 0..200u64 {
        let game = play_a_game(seed.wrapping_mul(0xD1B5_4A32_D192_ED03).wrapping_add(3));
        let wire: Vec<String> = game.entries().iter().map(|entry| entry.to_wire()).collect();
        let read: Vec<Entry> = wire
            .iter()
            .map(|text| Entry::parse(text).unwrap_or_else(|| panic!("{text:?} did not parse back")))
            .collect();
        assert_eq!(read, game.entries(), "seed {seed} did not survive as text");
        for token in &wire {
            assert!(!token.is_empty() && !token.contains(char::is_whitespace));
        }
        checked += 1;
    }
    assert_eq!(checked, 200);
}

#[test]
fn every_position_survives_its_picture() {
    let mut checked = 0;
    for seed in 0..200u64 {
        let mut dice = Dice::new(seed.wrapping_mul(0x94D0_49BB_1331_11EB).wrapping_add(11));
        let mut game = Game::new(Side::White);
        // Stop partway through, so the pictures checked are middlegame boards
        // rather than only opening and finished ones.
        for _ in 0..dice.below(80) {
            if game.is_over() {
                break;
            }
            match game.needs() {
                Needed::Throw { .. } => game.throw(dice.throw()).unwrap(),
                Needed::Move { .. } => {
                    let moves = game.moves();
                    game.play(moves[dice.below(moves.len())].chosen).unwrap()
                }
                Needed::Nothing => break,
            }
            let picture = game.position().to_picture();
            let read = Position::from_picture(&picture).unwrap_or_else(|| {
                panic!("seed {seed} produced an unreadable picture {picture:?}")
            });
            assert_eq!(&read, game.position(), "seed {seed} picture lost something");
            assert_eq!(read.to_picture(), picture);
            checked += 1;
        }
    }
    assert!(
        checked > 1_000,
        "only {checked} pictures were checked, which is too few to mean anything"
    );
}

#[test]
fn the_distribution_of_throws_is_one_four_six_four_one() {
    // Not a rule of the game, but the thing a player is told on the rules document,
    // and the generator in this file is what the property tests lean on.
    let mut dice = Dice::new(0xC0FF_EE12_3456_789A);
    let mut counts = [0u32; 5];
    let rounds = 400_000;
    for _ in 0..rounds {
        counts[dice.throw().steps() as usize] += 1;
    }
    let expected = [1.0, 4.0, 6.0, 4.0, 1.0];
    for (marks, &count) in counts.iter().enumerate() {
        let share = count as f64 / rounds as f64;
        let want = expected[marks] / 16.0;
        assert!(
            (share - want).abs() < 0.01,
            "a throw of {marks} came up {share:.4} of the time, not about {want:.4}"
        );
    }
}

#[test]
fn a_piece_is_never_taken_on_the_safe_square() {
    // The property that most directly encodes a rule. Played over many games,
    // watching the one square where a capture must never happen.
    let mut dice = Dice::new(0x5EED_1234_5678_9ABC);
    let mut times_held = 0;
    for _ in 0..400 {
        let mut game = Game::new(Side::White);
        while !game.is_over() {
            let holder_before = game.position().occupant(SAFE);
            match game.needs() {
                Needed::Throw { .. } => game.throw(dice.throw()).unwrap(),
                Needed::Move { side } => {
                    let moves = game.moves();
                    let chosen = moves[dice.below(moves.len())];
                    game.play(chosen.chosen).unwrap();
                    if let Some(held_by) = holder_before {
                        times_held += 1;
                        let holder_after = game.position().occupant(SAFE);
                        if holder_after != Some(held_by) {
                            // It may have moved off under its own power; it must
                            // not have been replaced by the enemy.
                            assert_ne!(
                                holder_after,
                                Some(side.other()),
                                "a piece was taken on the safe square"
                            );
                        }
                    }
                }
                Needed::Nothing => break,
            }
        }
    }
    assert!(
        times_held > 100,
        "the safe square was only occupied {times_held} times — too few to prove anything"
    );
}

#[test]
fn a_side_that_wins_has_every_piece_home_and_the_other_does_not() {
    let mut wins = [0, 0];
    for seed in 0..400u64 {
        let game = play_a_game(seed.wrapping_mul(0xBF58_476D_1CE4_E5B9).wrapping_add(5));
        let outcome = game.outcome().expect("a finished game has an outcome");
        assert_eq!(outcome.reason, WinReason::Home, "nothing here resigns");
        assert_eq!(game.position().off(outcome.winner), PIECES);
        assert!(game.position().off(outcome.winner.other()) < PIECES);
        match outcome.winner {
            Side::White => wins[0] += 1,
            Side::Black => wins[1] += 1,
        }
    }
    // Both sides win sometimes. A gate that only ever saw one side win would
    // pass with the board mirrored wrongly for the other.
    assert!(wins[0] > 50 && wins[1] > 50, "lopsided results: {wins:?}");
}
