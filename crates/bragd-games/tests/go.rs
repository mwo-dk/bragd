use bragd_games::{Awaiting, GameError, HighlightKind, apply, replay, start_position};

#[test]
fn go_9_plays_passes_and_is_counted() {
    let mut entries = vec!["e5", "c3", "pass", "pass"];
    let scoring = replay("go-9", None, &entries).unwrap();
    assert!(
        scoring.outcome.is_none(),
        "two passes stop play but count nothing yet"
    );
    assert!(scoring.moves.iter().any(|m| m.wire == "dead:"));
    assert!(!scoring.moves.iter().any(|m| m.wire == "pass"));

    entries.extend(["dead:", "accept"]);
    let over = replay("go-9", None, &entries).unwrap();
    let outcome = over.outcome.unwrap();
    assert_eq!(outcome.reason, "counted");
    // One stone each on an otherwise empty board: White wins by the komi.
    assert_eq!(outcome.result, "W+7.5");
    assert_eq!(outcome.winner.unwrap().name, "White");
    assert_eq!(over.awaiting, Awaiting::Nothing);
    assert!(over.moves.is_empty());
}

#[test]
fn go_13_and_go_19_replay_a_short_game() {
    for (key, size, game) in [
        ("go-13", 13, ["d4", "k10", "d10", "k4"]),
        ("go-19", 19, ["d4", "q16", "q4", "d16"]),
    ] {
        let state = replay(key, None, &game).unwrap();
        assert_eq!(state.board.pieces.len(), 4, "{key}");
        assert_eq!((state.board.files, state.board.ranks), (size, size));
        assert_eq!(state.to_move.unwrap().name, "Black", "{key}");
        assert_eq!(state.history, game);
    }
}

#[test]
fn a_capture_takes_the_stone_off() {
    // White's c3 is surrounded on all four sides.
    let state = replay("go-9", None, &["b3", "c3", "d3", "a1", "c2", "a2", "c4"]).unwrap();
    assert!(!state.board.pieces.iter().any(|p| p.square == "c3"));
}

#[test]
fn an_illegal_move_is_refused_with_its_index() {
    let err = apply("go-9", None, &["e5"], "e5").unwrap_err();
    assert!(
        matches!(err, GameError::Refused { index: 1, .. }),
        "{err:?}"
    );
    let err = apply("go-9", None, &["e5"], "z99").unwrap_err();
    assert!(
        matches!(err, GameError::Refused { index: 1, .. }),
        "{err:?}"
    );
    let err = apply("go-9", None, &["e5"], "accept").unwrap_err();
    assert!(
        matches!(err, GameError::Refused { index: 1, .. }),
        "{err:?}"
    );
}

#[test]
fn the_board_is_intersections_with_star_points() {
    let state = replay("go-19", None, &["d4"]).unwrap();
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json["board"]["geometry"], "intersections");
    let stars = state
        .board
        .highlights
        .iter()
        .filter(|h| h.kind == HighlightKind::Marked)
        .count();
    assert_eq!(stars, 9);
    let stone = &state.board.pieces[0];
    assert_eq!((stone.x, stone.y, stone.sprite.as_str()), (3, 3, "go-b"));
    let nine = replay::<&str>("go-9", None, &[]).unwrap();
    assert_eq!(nine.board.highlights.len(), 5);
}

#[test]
fn a_picture_round_trips_and_a_bad_one_is_refused() {
    let state = replay("go-9", None, &["e5"]).unwrap();
    let again = replay::<&str>("go-9", Some(&state.position), &[]).unwrap();
    assert_eq!(again.board.pieces, state.board.pieces);
    assert_eq!(again.to_move.unwrap().name, "White");
    assert!(start_position("go-9").unwrap().ends_with(" b"));
    assert!(matches!(
        replay::<&str>("go-9", Some("XO/OX b"), &[]),
        Err(GameError::BadPosition { .. })
    ));
}
