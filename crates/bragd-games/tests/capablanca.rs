use bragd_games::{Awaiting, GameError, apply, replay, start_position};

#[test]
fn a_short_capablanca_game_replays() {
    let moves = ["e2e4", "e7e5", "c1d3", "h8g6"];
    let state = replay("capablanca", None, &moves).unwrap();
    assert_eq!(state.history, moves);
    assert_eq!(state.to_move.unwrap().name, "White");
    assert_eq!(state.awaiting, Awaiting::Move);
    assert!(state.outcome.is_none());
    assert_eq!(state.board.highlights.len(), 2, "the last move, from and to");
}

#[test]
fn a_capablanca_illegal_move_is_refused() {
    let err = apply("capablanca", None, &["e2e4"], "e7e4").unwrap_err();
    assert!(matches!(err, GameError::Refused { index: 1, .. }), "{err:?}");
}

#[test]
fn the_capablanca_board_is_ten_files_wide() {
    let state = replay::<&str>("capablanca", None, &[]).unwrap();
    assert_eq!((state.board.files, state.board.ranks), (10, 8));
    assert_eq!(state.board.pieces.len(), 40);
    let archbishop = state.board.pieces.iter().find(|p| p.square == "c1").unwrap();
    assert_eq!((archbishop.x, archbishop.y, archbishop.sprite.as_str()), (2, 0, "chess-wA"));
    assert!(start_position("capablanca").unwrap().starts_with("rnabqkbcnr/"));
}
