//! The correctness gate.
//!
//! **There is no perft corpus for senet and there cannot usefully be one.** The
//! tree branches over five throws before it branches over moves, so a node count
//! would be counting a different object, and no published corpus of them exists
//! to compare against. The same argument backgammon makes and the same
//! answer tafl reached from the other direction.
//!
//! What makes this suite matter more here than for the other two race games is
//! that senet can move a piece **backwards** and can send one to a house nobody
//! chose. So "the game always ends" is a real question rather than an obvious
//! one, and it is asserted below over thousands of games rather than argued.
//!
//! The generator is a SplitMix64 seeded from a constant, so a failure is
//! reproducible from its seed. `crates/rules-senet` has no dependencies and no
//! randomness; this file is where that line is drawn.

use rules_senet::board::adjacent;
use rules_senet::position::Way;
use rules_senet::{
    Entry, Game, HOUSES, Needed, PIECES, Position, Side, Spot, THREE_TRUTHS, Throw, WATER,
    WinReason,
};

struct Sticks(u64);

impl Sticks {
    fn new(seed: u64) -> Sticks {
        Sticks(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Four sticks, each landing marked or plain — and **no marked face is a
    /// five**, which is the whole of senet's dice and is why this is not a
    /// uniform draw over one to five.
    fn throw(&mut self) -> Throw {
        let bits = self.next();
        let marks = (0..4).map(|stick| ((bits >> stick) & 1) as u8).sum();
        Throw::from_marks(marks).expect("four sticks cannot show more than four marks")
    }

    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }
}

/// Plays one game to its end, checking everything that must hold at every step.
fn play_a_game(seed: u64) -> Game {
    let mut sticks = Sticks::new(seed);
    let mut game = Game::new();

    // **A cap, and it has to be generous.** Senet can send a piece backwards and
    // can drown one, so a game genuinely can run long; what must not happen is
    // that it runs *for ever*. Twenty thousand entries is far past any real game
    // and still finite, so a stuck game fails loudly instead of hanging.
    let cap = 20_000;
    let mut steps = 0;

    while !game.is_over() && steps < cap {
        steps += 1;
        match game.needs() {
            Needed::Throw { side } => {
                assert_eq!(
                    game.position().to_move(),
                    side,
                    "needs and the position disagree about the turn"
                );
                assert!(game.moves().is_empty(), "moves are offered before a throw");
                let throw = sticks.throw();
                game.throw(throw)
                    .expect("a throw is always accepted when one is owed");
            }
            Needed::Move { side } => {
                assert_eq!(game.position().to_move(), side);
                let moves = game.moves();
                assert!(!moves.is_empty(), "a move is owed but none is offered");
                let chosen = moves[sticks.below(moves.len())];
                game.play(chosen.chosen)
                    .expect("a move taken from moves() must be legal");
            }
            Needed::Nothing => unreachable!("the loop guard already checked this"),
        }
        check(&game);
    }

    assert!(
        steps < cap,
        "seed {seed} did not finish within {cap} entries"
    );
    game
}

/// Everything that must be true of any position this engine can reach.
fn check(game: &Game) {
    let position = game.position();

    for side in [Side::White, Side::Black] {
        // **Material never changes.** Senet takes nothing off by contact — an
        // exchange moves two pieces and removes neither — so seven is seven until
        // a piece is borne off, and a borne-off piece never comes back.
        let pieces = position.pieces(side);
        assert_eq!(
            pieces.len(),
            PIECES as usize,
            "{side:?} does not have seven pieces"
        );

        for &spot in pieces {
            if let Spot::House(house) = spot {
                assert!(
                    (1..=HOUSES as u8).contains(&house),
                    "{side:?} has a piece on house {house}"
                );
            }
        }

        // No two of a side's pieces share a house.
        let mut houses: Vec<u8> = pieces.iter().filter_map(|spot| spot.house()).collect();
        let before = houses.len();
        houses.sort_unstable();
        houses.dedup();
        assert_eq!(houses.len(), before, "{side:?} has two pieces on one house");

        assert_eq!(houses.len() as u8 + position.off(side), PIECES);
    }

    // No house holds two pieces.
    for house in 1..=HOUSES as u8 {
        let holders = [Side::White, Side::Black]
            .into_iter()
            .filter(|&side| {
                position
                    .pieces(side)
                    .iter()
                    .any(|s| s.house() == Some(house))
            })
            .count();
        assert!(holders <= 1, "house {house} holds pieces of both sides");
    }

    // **At most one piece is ever in the water.** The freeze rule depends on it:
    // two drowned pieces of the same side would be a position the rules have
    // nothing to say about.
    let drowned = [Side::White, Side::Black]
        .into_iter()
        .filter(|&side| position.drowned(side))
        .count();
    assert!(drowned <= 1, "both sides are in the water at once");

    // The drowned piece is never protected, however it got there.
    if position.occupant(WATER).is_some() {
        assert!(
            !position.protected(WATER),
            "a piece in the water is protected"
        );
    }

    match position.winner() {
        Some(winner) => assert_eq!(position.off(winner), PIECES),
        None => {
            for side in [Side::White, Side::Black] {
                assert!(position.off(side) < PIECES);
            }
        }
    }

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
fn a_thousand_games_hold_every_property_and_every_one_of_them_ends() {
    let mut finished = 0;
    for seed in 0..1_000u64 {
        let game = play_a_game(seed.wrapping_mul(0x2545_F491_4F6C_DD1D).wrapping_add(1));
        assert!(game.is_over(), "seed {seed} did not finish");
        finished += 1;
    }
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
        assert_eq!(replayed.outcome(), game.outcome());
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
        let mut sticks = Sticks::new(seed.wrapping_mul(0x94D0_49BB_1331_11EB).wrapping_add(11));
        let mut game = Game::new();
        for _ in 0..sticks.below(120) {
            if game.is_over() {
                break;
            }
            match game.needs() {
                Needed::Throw { .. } => game.throw(sticks.throw()).unwrap(),
                Needed::Move { .. } => {
                    let moves = game.moves();
                    game.play(moves[sticks.below(moves.len())].chosen).unwrap()
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
        "only {checked} pictures were checked, which proves little"
    );
}

#[test]
fn a_piece_below_the_house_of_beauty_never_passes_it() {
    // The rule that shapes the whole endgame, checked as a property of every
    // move actually played rather than of one contrived position.
    let mut sticks = Sticks::new(0xBEA0_1234_5678_9ABC);
    let mut crossings = 0;
    let mut games = 0;
    for _ in 0..300 {
        let mut game = Game::new();
        while !game.is_over() {
            match game.needs() {
                Needed::Throw { .. } => game.throw(sticks.throw()).unwrap(),
                Needed::Move { .. } => {
                    let moves = game.moves();
                    let chosen = moves[sticks.below(moves.len())].chosen;
                    if chosen.way == Way::Forward
                        && let Spot::House(to) = chosen.to {
                            assert!(
                                !(chosen.from < rules_senet::BEAUTY && to > rules_senet::BEAUTY),
                                "a piece jumped from {} to {to}, over the House of Beauty",
                                chosen.from
                            );
                            if to == rules_senet::BEAUTY {
                                crossings += 1;
                            }
                        }
                    game.play(chosen).unwrap();
                }
                Needed::Nothing => break,
            }
        }
        games += 1;
    }
    assert_eq!(games, 300);
    // The loop is only meaningful if pieces actually reached the house it is
    // about. Without this the test could pass over three hundred games in which
    // nothing ever got that far.
    assert!(crossings > 100, "only {crossings} pieces ever landed on 26");
}

#[test]
fn a_drowned_side_moves_nothing_but_the_drowned_piece() {
    let mut sticks = Sticks::new(0x5EED_9876_5432_10FE);
    let mut drownings = 0;
    for _ in 0..300 {
        let mut game = Game::new();
        while !game.is_over() {
            match game.needs() {
                Needed::Throw { .. } => game.throw(sticks.throw()).unwrap(),
                Needed::Move { side } => {
                    let frozen = game.position().drowned(side);
                    let moves = game.moves();
                    if frozen {
                        drownings += 1;
                        for offer in &moves {
                            assert_eq!(
                                offer.chosen.from, WATER,
                                "a frozen side was offered a move from {}",
                                offer.chosen.from
                            );
                        }
                    }
                    let chosen = moves[sticks.below(moves.len())].chosen;
                    game.play(chosen).unwrap();
                }
                Needed::Nothing => break,
            }
        }
    }
    assert!(
        drownings > 20,
        "only {drownings} frozen turns — too few to prove anything"
    );
}

#[test]
fn the_drowned_piece_still_guards_the_house_beside_it() {
    // The consequence two separate rules combine to produce, checked over real
    // games rather than in one fixture: a piece on 28 whose owner is in the water
    // is protected, because 27 and 28 are a step apart.
    assert!(adjacent(WATER, THREE_TRUTHS));
    let mut sticks = Sticks::new(0x2718_2818_2845_9045);
    let mut seen = 0;
    for _ in 0..400 {
        let mut game = Game::new();
        while !game.is_over() {
            let position = game.position().clone();
            if let (Some(drowned), Some(guarded)) =
                (position.occupant(WATER), position.occupant(THREE_TRUTHS))
                && drowned == guarded {
                    assert!(
                        position.protected(THREE_TRUTHS),
                        "28 should be guarded by its own side's drowned piece"
                    );
                    seen += 1;
                }
            match game.needs() {
                Needed::Throw { .. } => game.throw(sticks.throw()).unwrap(),
                Needed::Move { .. } => {
                    let moves = game.moves();
                    game.play(moves[sticks.below(moves.len())].chosen).unwrap()
                }
                Needed::Nothing => break,
            }
        }
    }
    assert!(
        seen > 0,
        "the arrangement never came up, so nothing was proved"
    );
}

#[test]
fn the_distribution_of_throws_is_four_six_four_one_one() {
    let mut sticks = Sticks::new(0xC0FF_EE98_7654_3210);
    let mut counts = [0u32; 6];
    let rounds = 400_000;
    for _ in 0..rounds {
        counts[sticks.throw().steps() as usize] += 1;
    }
    assert_eq!(counts[0], 0, "senet has no throw of nought");
    let expected = [0.0, 4.0, 6.0, 4.0, 1.0, 1.0];
    for value in 1..=5usize {
        let share = counts[value] as f64 / rounds as f64;
        let want = expected[value] / 16.0;
        assert!(
            (share - want).abs() < 0.01,
            "a throw of {value} came up {share:.4} of the time, not about {want:.4}"
        );
    }
}

#[test]
fn a_side_that_wins_has_every_piece_off_and_both_sides_win_sometimes() {
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
    // Both sides win sometimes. A gate that only ever saw one side win would pass
    // with the board mirrored wrongly for the other — and senet's opening array
    // is not symmetric, so this is a real check rather than a formality.
    assert!(wins[0] > 40 && wins[1] > 40, "lopsided results: {wins:?}");
}
