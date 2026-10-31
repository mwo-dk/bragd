use bragd_games::{Awaiting, GameError, HighlightKind, apply, replay, start_position};

/// Bishops exchanged on 2b, then Sente drops the captured bishop back.
const GAME: &[&str] = &["7g7f", "3c3d", "8h2b+", "3a2b", "B*4e"];

#[test]
fn a_short_game_with_a_drop_replays() {
    let state = replay("shogi", None, GAME).unwrap();
    assert_eq!(state.history, GAME);
    assert_eq!(state.awaiting, Awaiting::Move);
    assert_eq!(state.to_move.unwrap().name, "Gote");
    let dropped = state
        .board
        .pieces
        .iter()
        .find(|p| p.square == "4e")
        .unwrap();
    assert_eq!(
        (dropped.x, dropped.y, dropped.sprite.as_str()),
        (5, 4, "shogi-bB")
    );
    // Gote took a promoted bishop and holds a plain one.
    let gote = &state.board.hands[1];
    assert_eq!(gote.pieces.len(), 1);
    assert_eq!(
        (gote.pieces[0].sprite.as_str(), gote.pieces[0].count),
        ("shogi-wB", 1)
    );
    assert!(
        state.board.hands[0].pieces.is_empty(),
        "Sente's bishop is back on the board"
    );
}

#[test]
fn drops_are_offered_from_the_hand() {
    let state = replay("shogi", None, &GAME[..4]).unwrap();
    let drop = state.moves.iter().find(|m| m.wire == "B*5e").unwrap();
    assert_eq!(drop.from, None);
    assert_eq!(drop.to.as_deref(), Some("5e"));
    assert_eq!(drop.drop.as_deref(), Some("shogi-bB"));
}

#[test]
fn a_promoted_piece_has_its_own_sprite() {
    let state = replay("shogi", None, &GAME[..3]).unwrap();
    let horse = state
        .board
        .pieces
        .iter()
        .find(|p| p.square == "2b")
        .unwrap();
    assert_eq!(horse.sprite, "shogi-bH");
    assert!(
        state
            .board
            .highlights
            .iter()
            .any(|h| h.kind == HighlightKind::LastMove && (h.x, h.y) == (7, 7))
    );
}

#[test]
fn an_illegal_move_is_refused_with_its_index() {
    let err = apply("shogi", None, &GAME[..2], "7f7d").unwrap_err();
    assert!(
        matches!(err, GameError::Refused { index: 2, .. }),
        "{err:?}"
    );
    // A drop of a piece not in hand.
    let err = apply("shogi", None, &GAME[..1], "R*5e").unwrap_err();
    assert!(
        matches!(err, GameError::Refused { index: 1, .. }),
        "{err:?}"
    );
}

#[test]
fn the_board_is_nine_by_nine_from_sentes_side() {
    let state = replay::<&str>("shogi", None, &[]).unwrap();
    assert_eq!((state.board.files, state.board.ranks), (9, 9));
    assert_eq!(state.board.pieces.len(), 40);
    // Sente's king on 5i stands at the bottom centre.
    let king = state
        .board
        .pieces
        .iter()
        .find(|p| p.square == "5i")
        .unwrap();
    assert_eq!((king.x, king.y, king.sprite.as_str()), (4, 0, "shogi-bK"));
    assert_eq!(state.to_move.unwrap().name, "Sente");
    let start = start_position("shogi").unwrap();
    assert_eq!(
        replay::<&str>("shogi", Some(&start), &[]).unwrap().position,
        start
    );
    assert!(matches!(
        replay::<&str>("shogi", Some("nonsense"), &[]),
        Err(GameError::BadPosition { .. })
    ));
}

/// A replay generates each position's legal moves once: one per ply and one for the start.
/// Counted, not timed.
#[test]
fn a_replay_generates_the_legal_moves_once_a_ply() {
    let line = [
        "7g7f", "3c3d", "2g2f", "8c8d", "2f2e", "8d8e", "6i7h", "4a3b", "2e2d", "2c2d", "2h2d",
        "P*2c",
    ];
    let before = rules_shogi::position::generated();
    let state = bragd_games::replay("shogi", None, &line).unwrap();
    assert_eq!(rules_shogi::position::generated() - before, line.len() as u64 + 1);
    assert_eq!(state.history.len(), line.len());
}

/// Shogi numbers each move, as a kifu does.
#[test]
fn the_shogi_move_list_numbers_each_move() {
    let state = replay("shogi", None, &GAME[..3]).unwrap();
    assert_eq!(state.numbers, ["1", "2", "3"]);
}
