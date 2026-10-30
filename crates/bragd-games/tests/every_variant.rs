use bragd_games::{Awaiting, Family, GameError, VARIANTS, replay, start_position};

#[test]
fn every_variant_starts_and_offers_something_to_do() {
    for info in &VARIANTS {
        let key = info.key;
        let start = start_position(key).unwrap_or_else(|e| panic!("{key}: {e}"));
        assert!(!start.is_empty(), "{key}");
        let state = replay::<&str>(key, None, &[]).unwrap_or_else(|e| panic!("{key}: {e}"));
        assert_eq!(state.variant.key(), key);
        assert!(state.outcome.is_none(), "{key} is over before it began");
        let seat = state.to_move.unwrap_or_else(|| panic!("{key}: nobody to act"));
        assert_eq!(seat.name, info.sides[seat.index as usize], "{key}");
        match state.awaiting {
            Awaiting::Move => assert!(!state.moves.is_empty(), "{key}: no legal moves"),
            Awaiting::Throw => assert!(!state.throws.is_empty(), "{key}: no throws"),
            Awaiting::Nothing => panic!("{key}: awaiting nothing at the start"),
        }
        let board = &state.board;
        assert_eq!((board.files, board.ranks), (info.files, info.ranks), "{key}");
        assert_eq!(board.geometry, info.geometry, "{key}");
        let prefix = format!("{}-", info.family.key());
        for piece in &board.pieces {
            assert!(piece.sprite.starts_with(&prefix), "{key}: {}", piece.sprite);
            assert!(piece.x < board.files.max(26) && piece.count > 0, "{key}: {piece:?}");
        }
        if info.family != Family::Go {
            assert!(!board.pieces.is_empty() || !board.hands.is_empty(), "{key}: empty board");
        }
        serde_json::to_string(&state).unwrap();
    }
}

#[test]
fn the_start_position_round_trips_where_the_family_takes_one() {
    for info in &VARIANTS {
        let key = info.key;
        let start = start_position(key).unwrap();
        let from_text = replay::<&str>(key, Some(&start), &[])
            .unwrap_or_else(|e| panic!("{key}: its own start was refused: {e}"));
        let from_none = replay::<&str>(key, None, &[]).unwrap();
        assert_eq!(from_text.position, from_none.position, "{key}");
    }
}

#[test]
fn an_unknown_variant_is_an_error_not_a_fallback() {
    assert_eq!(
        replay::<&str>("checkers-but-wrong", None, &[]).unwrap_err(),
        GameError::UnknownVariant {
            key: "checkers-but-wrong".into()
        }
    );
}

#[test]
fn every_variant_refuses_nonsense() {
    for info in &VARIANTS {
        let err = replay(info.key, None, &["zz99-nonsense"]).unwrap_err();
        assert!(
            matches!(err, GameError::Refused { index: 0, .. }),
            "{}: {err:?}",
            info.key
        );
    }
}
