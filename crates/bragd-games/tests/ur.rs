use bragd_games::{Awaiting, GameError, HighlightKind, apply, replay};

#[test]
fn a_short_game_replays_with_its_throws() {
    let mut record: Vec<String> = Vec::new();
    // Light enters onto its rosette and throws again; Dark throws nought and passes.
    for (throw, mover, after) in [
        ("4", "Light", Awaiting::Throw),
        ("2", "Light", Awaiting::Throw),
        ("0", "Dark", Awaiting::Throw),
    ] {
        record.push(throw.to_string());
        let state = replay("ur", None, &record).unwrap();
        if throw == "0" {
            assert_eq!(state.awaiting, after);
            assert_eq!(state.to_move.unwrap().name, "Light", "a nought passes");
            continue;
        }
        assert_eq!(state.awaiting, Awaiting::Move);
        assert_eq!(state.to_move.unwrap().name, mover);
        let chosen = state.moves.first().expect("a legal move").wire.clone();
        record.push(chosen);
        let state = replay("ur", None, &record).unwrap();
        assert_eq!(state.history, record);
        if throw == "4" {
            assert_eq!(state.awaiting, after, "a rosette earns another throw");
            assert_eq!(state.to_move.unwrap().name, "Light");
        }
    }
}

#[test]
fn an_illegal_move_and_an_early_throw_are_refused() {
    let err = apply("ur", None, &["4"], "0-3").unwrap_err();
    assert!(
        matches!(err, GameError::Refused { index: 1, .. }),
        "{err:?}"
    );
    let err = apply("ur", None, &["4"], "2").unwrap_err();
    assert!(
        matches!(err, GameError::Refused { index: 1, .. }),
        "{err:?}"
    );
}

#[test]
fn dark_can_throw_first() {
    let state = replay::<&str>("ur", Some("dark"), &[]).unwrap();
    assert_eq!(state.to_move.unwrap().name, "Dark");
}

#[test]
fn the_board_is_drawn_on_its_own_grid() {
    let state = replay("ur", None, &["4", "0-4"]).unwrap();
    let piece = &state.board.pieces[0];
    assert_eq!((piece.x, piece.y, piece.sprite.as_str()), (0, 0, "ur-l"));
    let marked: Vec<_> = state
        .board
        .highlights
        .iter()
        .filter(|h| h.kind == HighlightKind::Marked)
        .collect();
    assert_eq!(marked.len(), 5, "five rosettes");
    assert!(marked.iter().any(|h| (h.x, h.y) == (0, 0)));
    assert_eq!(state.board.hands[0].pieces[0].count, 6);
    assert_eq!(state.board.hands[1].pieces[0].count, 7);
}
