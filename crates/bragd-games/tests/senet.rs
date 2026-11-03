use bragd_games::{Awaiting, GameError, apply, replay};

#[test]
fn a_short_game_replays_with_its_throws() {
    let mut record: Vec<String> = Vec::new();
    for throw in ["2", "3", "2"] {
        record.push(throw.to_string());
        let state = replay("senet", None, &record).unwrap();
        if state.awaiting == Awaiting::Throw {
            continue; // the throw allowed nothing and passed
        }
        assert_eq!(state.awaiting, Awaiting::Move);
        let chosen = state.moves.first().expect("a legal move").wire.clone();
        record.push(chosen);
    }
    let state = replay("senet", None, &record).unwrap();
    assert_eq!(state.history, record);
    assert!(state.outcome.is_none());
    assert!(record.len() > 3, "moves were played: {record:?}");
}

#[test]
fn evens_throw_first() {
    let state = replay::<&str>("senet", None, &[]).unwrap();
    assert_eq!(state.awaiting, Awaiting::Throw);
    assert_eq!(state.to_move.unwrap().name, "Evens");
    assert_eq!(state.throws, ["1", "2", "3", "4", "5"]);
}

#[test]
fn an_illegal_move_and_an_early_throw_are_refused() {
    let err = apply("senet", None, &["1"], "14-20").unwrap_err();
    assert!(
        matches!(err, GameError::Refused { index: 1, .. }),
        "{err:?}"
    );
    let err = apply("senet", None, &["1"], "2").unwrap_err();
    assert!(
        matches!(err, GameError::Refused { index: 1, .. }),
        "{err:?}"
    );
}

#[test]
fn the_houses_snake_and_are_numbered() {
    let state = replay::<&str>("senet", None, &[]).unwrap();
    let board = &state.board;
    assert_eq!(board.labels.len(), 30);
    let house = |n: &str| board.pieces.iter().find(|p| p.square == n).unwrap();
    assert_eq!(
        (house("1").x, house("1").y, house("1").sprite.as_str()),
        (0, 2, "senet-o")
    );
    assert_eq!(
        (house("2").x, house("2").y, house("2").sprite.as_str()),
        (1, 2, "senet-e")
    );
    // House 11 sits directly under house 10.
    assert_eq!((house("11").x, house("11").y), (9, 1));
    assert_eq!(board.pieces.len(), 14);
}

#[test]
fn senet_reports_a_lone_move_as_forced_and_a_choice_as_not() {
    // A first throw of 2 has one move; a 1 has seven.
    assert_eq!(replay("senet", None, &["2"]).unwrap().forced.as_deref(), Some("14-16"));
    assert_eq!(replay("senet", None, &["1"]).unwrap().forced, None);
    assert_eq!(replay::<&str>("senet", None, &[]).unwrap().forced, None, "nothing is forced before a throw");
}

#[test]
fn the_move_list_shows_a_turn_as_its_throw_and_its_move() {
    let first = replay("senet", None, &["2"]).unwrap().moves[0].wire.clone();
    let record = ["2", first.as_str()];
    let state = replay("senet", None, &record).unwrap();
    assert_eq!(state.numbers, ["1", ""]);
    assert_eq!(state.written, ["2:", first.as_str()]);
}

#[test]
fn chess_writes_its_moves_as_the_record_does() {
    let state = replay("chess", None, &["e2e4", "e7e5"]).unwrap();
    assert_eq!(state.written, state.history);
}
