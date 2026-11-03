use bragd_games::{Awaiting, GameError, State, apply, replay};

fn offered(state: &State, wire: &str) -> bool {
    state.moves.iter().any(|m| m.wire == wire)
}

#[test]
fn a_short_game_replays_with_its_rolls() {
    let mut record: Vec<&str> = Vec::new();
    let start = replay("backgammon", None, &record).unwrap();
    assert_eq!(start.awaiting, Awaiting::Throw);
    // The opening roll is one die each: the 15 rolls that are not doubles.
    assert_eq!(start.throws.len(), 15);
    assert!(!start.throws.contains(&"6-6".to_string()));
    assert!(apply("backgammon", None, &record, "6-6").is_err(), "the rules refuse an opening double");
    assert_eq!(start.to_move.unwrap().name, "White");
    assert!(!offered(&start, "double"), "nobody doubles before the opening throw");
    assert!(apply("backgammon", None, &record, "double").is_err(), "and the rules refuse it");

    for (roll, turn, mover) in [("3-1", "8/5 6/5", "White"), ("6-5", "24/18 18/13", "Black")] {
        record.push(roll);
        let state = replay("backgammon", None, &record).unwrap();
        assert_eq!(state.awaiting, Awaiting::Move);
        assert_eq!(state.to_move.unwrap().name, mover);
        assert!(offered(&state, turn), "{turn} not in {:?}", state.moves);
        record.push(turn);
    }
    let state = replay("backgammon", None, &record).unwrap();
    assert_eq!(state.history, record);
    assert_eq!(state.awaiting, Awaiting::Throw);
    assert_eq!(state.to_move.unwrap().name, "White");
    assert!(offered(&state, "double"), "the cube is in the middle");
    assert_eq!(state.throws.len(), 21, "doubles after the opening");
}

#[test]
fn the_opening_throw_seats_the_higher_die() {
    assert_eq!(bragd_games::backgammon_opening(2, 5), Some(("black".into(), "5-2".into())));
    assert_eq!(bragd_games::backgammon_opening(6, 1), Some(("white".into(), "6-1".into())));
    assert_eq!(bragd_games::backgammon_opening(4, 4), None);
}

#[test]
fn black_can_be_seated_first() {
    let state = replay::<&str>("backgammon", Some("black"), &[]).unwrap();
    assert_eq!(state.to_move.unwrap().name, "Black");
    assert!(replay::<&str>("backgammon", Some("green"), &[]).is_err());
}

#[test]
fn an_illegal_turn_and_an_early_roll_are_refused() {
    let err = apply("backgammon", None, &["3-1"], "13/9").unwrap_err();
    assert!(
        matches!(err, GameError::Refused { index: 1, .. }),
        "{err:?}"
    );
    let err = apply("backgammon", None, &["3-1"], "6-5").unwrap_err();
    assert!(
        matches!(err, GameError::Refused { index: 1, .. }),
        "{err:?}"
    );
}

#[test]
fn a_double_is_answered_before_anything_else() {
    let opening = ["3-1", "8/5 6/5"];
    let with = |more: &[&'static str]| [&opening[..], more].concat();
    let state = replay("backgammon", None, &with(&["double"])).unwrap();
    assert_eq!(state.to_move.unwrap().name, "White");
    let wires: Vec<_> = state.moves.iter().map(|m| m.wire.as_str()).collect();
    assert_eq!(wires, ["take", "drop"]);
    let taken = replay("backgammon", None, &with(&["double", "take"])).unwrap();
    let cube = taken.board.cube.unwrap();
    assert_eq!((cube.value, cube.owner), (2, Some(0)));
    let dropped = replay("backgammon", None, &with(&["double", "drop"])).unwrap();
    assert_eq!(dropped.outcome.unwrap().winner.unwrap().name, "Black");
}

#[test]
fn the_board_counts_points_in_whites_numbering() {
    let state = replay("backgammon", None, &["3-1"]).unwrap();
    assert_eq!(state.board.dice, [3, 1]);
    let at = |x: u8| state.board.pieces.iter().find(|p| p.x == x).unwrap();
    assert_eq!((at(24).sprite.as_str(), at(24).count), ("backgammon-w", 2));
    assert_eq!((at(1).sprite.as_str(), at(1).count), ("backgammon-b", 2));
    assert_eq!((at(6).sprite.as_str(), at(6).count), ("backgammon-w", 5));
    assert_eq!(state.board.pieces.len(), 8);
}

/// The side to act may resign a single game, a gammon or a backgammon; it ends the Game.
#[test]
fn a_side_resigns_at_the_grade_it_names() {
    let start = replay::<&str>("backgammon", None, &[]).unwrap();
    for wire in ["resign", "resign_gammon", "resign_backgammon"] {
        assert!(offered(&start, wire), "{wire} not in {:?}", start.moves);
    }
    let rolled = replay("backgammon", None, &["3-1"]).unwrap();
    assert!(offered(&rolled, "resign"));
    let state = replay("backgammon", None, &["3-1", "resign_gammon"]).unwrap();
    let outcome = state.outcome.unwrap();
    assert_eq!(outcome.winner.unwrap().name, "Black");
    assert_eq!((outcome.reason.as_str(), outcome.result.as_str()), ("resignation", "2 points"));
    assert_eq!(state.history, ["3-1", "resign_gammon"]);
    assert_eq!(state.awaiting, Awaiting::Nothing);
    assert!(state.moves.is_empty());
    let state = replay("backgammon", None, &["3-1", "8/5 6/5", "resign_backgammon"]).unwrap();
    assert_eq!(state.outcome.unwrap().winner.unwrap().name, "White");
    assert!(matches!(
        apply("backgammon", None, &["3-1", "resign"], "8/5 6/5"),
        Err(GameError::Refused { index: 2, .. })
    ));
}

/// #239: the board moves one checker at a time. After 6-5, 13/8 is the five alone: no whole
/// turn starts with it, yet it is a legal first checker.
#[test]
fn a_turn_is_played_one_checker_at_a_time() {
    let thrown = replay("backgammon", None, &["6-5"]).unwrap();
    let step = |from: &str, to: &str| {
        thrown
            .steps
            .iter()
            .any(|m| m.from.as_deref() == Some(from) && m.to.as_deref() == Some(to))
    };
    assert!(step("13", "8"), "13/8 not offered: {:?}", thrown.steps);
    assert!(step("24", "18") && step("13", "7"));

    let half = bragd_games::step("backgammon", None, &["6-5"], &["13/8"]).unwrap();
    assert_eq!(half.entry, None, "half a turn is not an entry yet");
    let count = |s: &State, square: &str| {
        s.board
            .pieces
            .iter()
            .find(|p| p.square == square)
            .map_or(0, |p| p.count)
    };
    assert_eq!(
        (count(&half.state, "13"), count(&half.state, "8")),
        (4, 4),
        "the checker moved"
    );
    assert_eq!(half.state.history, ["6-5"]);
    assert!(
        half.state.steps.iter().all(|m| m.wire != "13/8"),
        "the five is used"
    );
    assert_eq!(thrown.board.dice, [6, 5]);
    assert_eq!(thrown.board.used, [false, false]);
    assert_eq!(half.state.board.dice, [6, 5]);
    assert_eq!(half.state.board.used, [false, true], "the board greys the five");

    let whole = bragd_games::step("backgammon", None, &["6-5"], &["13/8", "13/7"]).unwrap();
    let entry = whole.entry.expect("both dice played make the turn");
    assert_eq!(whole.state.history, ["6-5", entry.as_str()]);
    assert_eq!(whole.state.awaiting, Awaiting::Throw);
    assert_eq!(whole.state.to_move.unwrap().name, "Black");
    assert_eq!(
        (
            count(&whole.state, "13"),
            count(&whole.state, "8"),
            count(&whole.state, "7")
        ),
        (3, 4, 1)
    );

    // A checker that cannot go there is refused, and so is a step before the throw.
    assert!(matches!(
        bragd_games::step("backgammon", None, &["6-5"], &["13/9"]),
        Err(GameError::Refused { .. })
    ));
    assert!(bragd_games::step::<&str>("backgammon", None, &[], &["13/8"]).is_err());
}

/// #248: a double is four dice, and each checker moved uses one of them.
#[test]
fn a_double_shows_four_dice_and_greys_each_one_used() {
    let record = ["3-1", "8/5 6/5", "4-4"];
    let state = replay("backgammon", None, &record).unwrap();
    assert_eq!(state.board.dice, [4, 4, 4, 4]);
    assert_eq!(state.board.used, [false; 4]);
    let half = bragd_games::step("backgammon", None, &record, &["13/9", "13/9"]).unwrap();
    assert_eq!(half.state.board.used, [true, true, false, false]);
}

/// A roll whose next checker move is the only one, or that can play nothing, is reported as
/// forced; any choice is not. Played out from fixed rolls, taking the first turn each time.
#[test]
fn a_roll_with_one_way_to_play_is_forced_and_a_choice_is_not() {
    let words = ["resign", "double", "take", "drop"];
    let mut record: Vec<String> = Vec::new();
    let (mut forced, mut free) = (0, 0);
    let mut seed: u32 = 7;
    for _ in 0..400 {
        let state = replay("backgammon", None, &record).unwrap();
        if state.outcome.is_some() {
            break;
        }
        let turns: Vec<&str> = state.moves.iter().map(|m| m.wire.as_str()).filter(|w| !words.iter().any(|x| w.starts_with(x))).collect();
        if state.awaiting == bragd_games::Awaiting::Throw {
            assert_eq!(state.forced, None, "nothing is forced before the roll");
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let roll = state.throws[(seed >> 16) as usize % state.throws.len()].clone();
            record.push(roll);
            continue;
        }
        if state.steps.len() == 1 {
            assert_eq!(state.forced.as_deref(), Some(state.steps[0].wire.as_str()), "{record:?}");
            forced += 1;
        } else if state.steps.is_empty() && turns.len() == 1 {
            assert_eq!(state.forced.as_deref(), Some(turns[0]), "{record:?}");
            forced += 1;
        } else {
            assert_eq!(state.forced, None, "{record:?}");
            free += 1;
        }
        record.push(turns[0].to_string());
    }
    assert!(forced > 0 && free > 0, "forced {forced}, free {free}");
}

/// The printed number of the point White calls `x`.
fn label(state: &State, x: u8) -> String {
    state.board.labels.iter().find(|l| l.x == x).map(|l| l.text.clone()).unwrap_or_default()
}

#[test]
fn the_points_are_numbered_from_the_side_to_move_as_its_moves_are() {
    let white = replay::<&str>("backgammon", None, &["3-1"]).unwrap();
    assert_eq!((label(&white, 1), label(&white, 24)), ("1".into(), "24".into()));
    assert!(white.moves.iter().any(|m| m.wire == "8/5 6/5"));

    // Black's 24-point is White's 1-point, where Black's back checkers stand.
    let black = replay::<&str>("backgammon", Some("black"), &["5-2"]).unwrap();
    assert_eq!(black.board.labels.len(), 24);
    assert_eq!((label(&black, 1), label(&black, 24), label(&black, 19)), ("24".into(), "1".into(), "6".into()));
    assert!(black.moves.iter().any(|m| m.wire == "13/8 6/4"), "{:?}", black.moves);
}

#[test]
fn the_opening_throw_says_whose_die_is_whose() {
    // Black threw 5 against White's 2, so Black starts with 5-2.
    let state = replay::<&str>("backgammon", Some("black"), &["5-2"]).unwrap();
    assert_eq!(state.board.dice, [5, 2]);
    assert_eq!(state.board.opening, [1, 0], "the 5 is Black's (seat 1), the 2 White's");
    let state = replay::<&str>("backgammon", None, &["4-2"]).unwrap();
    assert_eq!(state.board.opening, [0, 1]);
    // Once the opening turn is played the dice are the next Side's own.
    let after = replay::<&str>("backgammon", None, &["4-2", "8/4 6/4", "6-5"]).unwrap();
    assert!(after.board.opening.is_empty());
    assert!(replay::<&str>("backgammon", None, &[]).unwrap().board.opening.is_empty(), "nothing thrown yet");
}
