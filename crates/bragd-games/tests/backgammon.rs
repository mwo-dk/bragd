use bragd_games::{Awaiting, GameError, State, apply, replay};

fn offered(state: &State, wire: &str) -> bool {
    state.moves.iter().any(|m| m.wire == wire)
}

#[test]
fn a_short_game_replays_with_its_rolls() {
    let mut record: Vec<&str> = Vec::new();
    let start = replay("backgammon", None, &record).unwrap();
    assert_eq!(start.awaiting, Awaiting::Throw);
    assert_eq!(start.throws.len(), 21);
    assert_eq!(start.to_move.unwrap().name, "White");

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
    let state = replay("backgammon", None, &["double"]).unwrap();
    assert_eq!(state.to_move.unwrap().name, "Black");
    let wires: Vec<_> = state.moves.iter().map(|m| m.wire.as_str()).collect();
    assert_eq!(wires, ["take", "drop"]);
    let taken = replay("backgammon", None, &["double", "take"]).unwrap();
    let cube = taken.board.cube.unwrap();
    assert_eq!((cube.value, cube.owner), (2, Some(1)));
    let dropped = replay("backgammon", None, &["double", "drop"]).unwrap();
    assert_eq!(dropped.outcome.unwrap().winner.unwrap().name, "White");
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
