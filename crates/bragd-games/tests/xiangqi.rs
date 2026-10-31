use bragd_games::{Awaiting, GameError, Geometry, apply, replay};

#[test]
fn a_short_xiangqi_game_replays() {
    let state = replay("xiangqi", None, &["h3e3", "h10g8", "h1g3"]).unwrap();
    assert_eq!(state.history, ["h3e3", "h10g8", "h1g3"]);
    assert_eq!(state.to_move.unwrap().name, "Black");
    assert_eq!(state.awaiting, Awaiting::Move);
    assert!(state.outcome.is_none());
    assert!(!state.targets.is_empty());
}

#[test]
fn a_xiangqi_general_cannot_leave_its_palace() {
    let err = apply("xiangqi", None, &["h3e3"], "e10e8").unwrap_err();
    assert!(matches!(err, GameError::Refused { index: 1, .. }), "{err:?}");
}

#[test]
fn the_xiangqi_board_is_points_with_red_at_the_bottom() {
    let state = replay::<&str>("xiangqi", None, &[]).unwrap();
    assert_eq!(state.board.geometry, Geometry::Intersections);
    assert_eq!((state.board.files, state.board.ranks), (9, 10));
    assert_eq!(state.board.pieces.len(), 32);
    let general = state.board.pieces.iter().find(|p| p.square == "e1").unwrap();
    assert_eq!((general.x, general.y, general.sprite.as_str()), (4, 0, "xiangqi-rK"));
    assert_eq!(state.to_move.unwrap().name, "Red");
}

#[test]
fn a_janggi_game_with_passes_replays() {
    let state = replay("janggi", None, &["e2e2", "e9e9", "a4b4"]).unwrap();
    assert_eq!(state.history, ["pass", "pass", "a4b4"]);
    assert_eq!(state.to_move.unwrap().name, "Han");
    let pass = state.moves.iter().find(|m| m.wire == "e9e9").unwrap();
    assert_eq!((pass.from.as_deref(), pass.to.as_deref()), (None, None));
    let general = state.board.pieces.iter().find(|p| p.square == "e2").unwrap();
    assert_eq!((general.x, general.y, general.sprite.as_str()), (4, 1, "janggi-rK"));
}

#[test]
fn a_janggi_soldier_cannot_step_back() {
    let err = apply("janggi", None, &["e2e2", "e9e9"], "a4a3").unwrap_err();
    assert!(matches!(err, GameError::Refused { index: 2, .. }), "{err:?}");
}

#[test]
fn the_xiangqi_move_list_numbers_full_moves() {
    let state = replay("xiangqi", None, &["h3e3", "h10g8", "h1g3"]).unwrap();
    assert_eq!(state.numbers, ["1.", "", "2."]);
}
