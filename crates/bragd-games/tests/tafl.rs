use bragd_games::{Awaiting, GameError, HighlightKind, apply, replay, start_position};

const HNEFATAFL: &[&str] = &["d1d4", "f4f3", "a4b4", "e5e2"];

/// A short legal game in each Variant.
const GAMES: &[(&str, &[&str])] = &[
    ("hnefatafl", HNEFATAFL),
    ("hnefatafl-fetlar", HNEFATAFL),
    ("tablut", &["d1d4", "e3c3", "a6c6", "c3c4"]),
    ("brandubh", &["a4a2", "c4c2", "d2e2", "e4e3"]),
];

#[test]
fn a_short_game_replays_in_every_tafl_variant() {
    for (key, moves) in GAMES {
        let state = replay(key, None, moves).unwrap_or_else(|e| panic!("{key}: {e}"));
        assert_eq!(state.history, *moves, "{key}");
        assert_eq!(state.awaiting, Awaiting::Move, "{key}");
        assert_eq!(state.to_move.unwrap().name, "Attackers", "{key}");
        assert!(state.outcome.is_none(), "{key}");
    }
}

#[test]
fn an_illegal_move_is_refused_with_its_index() {
    for (key, moves) in GAMES {
        // The same move again: its piece has gone, and it is the defenders' turn.
        let err = apply(key, None, &moves[..1], moves[0]).unwrap_err();
        assert!(matches!(err, GameError::Refused { index: 1, .. }), "{key}: {err:?}");
    }
}

#[test]
fn brandubh_draws_its_pieces_throne_and_corners() {
    let state = replay("brandubh", None, &["a4a2"]).unwrap();
    assert_eq!(state.board.pieces.len(), 13);
    let king = state.board.pieces.iter().find(|p| p.sprite == "tafl-dK").unwrap();
    assert_eq!((king.x, king.y, king.square.as_str()), (3, 3, "d4"));
    let marked: Vec<_> = state
        .board
        .highlights
        .iter()
        .filter(|h| h.kind == HighlightKind::Marked)
        .map(|h| (h.x, h.y))
        .collect();
    assert_eq!(marked.len(), 5);
    assert!(marked.contains(&(3, 3)) && marked.contains(&(6, 6)) && marked.contains(&(0, 0)));
    assert_eq!(state.to_move.unwrap().name, "Defenders");
}

#[test]
fn a_picture_start_is_honoured_and_a_bad_one_refused() {
    let start = start_position("brandubh").unwrap();
    assert_eq!(start, "...X.../...X.../...O.../XXOKOXX/...O.../...X.../...X...");
    // A king already in the corner it is trying to reach.
    let escaped = replay::<&str>("brandubh", Some("K....../......./......./......X/......./......./......."), &[]);
    assert!(escaped.is_ok(), "{escaped:?}");
    for bad in ["...", "X....../......./......./......./......./......./.......", "Z......"] {
        assert!(
            matches!(replay::<&str>("brandubh", Some(bad), &[]), Err(GameError::BadPosition { .. })),
            "{bad}"
        );
    }
}
