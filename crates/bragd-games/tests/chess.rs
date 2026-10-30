use bragd_games::{Awaiting, GameError, apply, replay, start_position};

/// A short legal game in each Variant, and what it must end in.
const GAMES: &[(&str, &[&str], &str)] = &[
    ("chess", &["f2f3", "e7e5", "g2g4", "d8h4"], "checkmate"),
    ("chess960", &["e2e4", "e7e5", "g1f3", "b8c6"], ""),
    ("shatranj", &["e2e3", "e7e6", "c1a3", "f8d6"], ""),
    ("chaturanga", &["e2e3", "e7e6", "b1a3"], ""),
    ("kingofthehill", &["d2d3", "a7a6", "e1d2", "a6a5", "d2e3", "a5a4", "e3d4"], "kingOfTheHill"),
    ("racingkings", &["h2h3", "a2a3"], ""),
    ("seirawan", &["b1c3h", "g8f6e", "e2e4"], ""),
];

#[test]
fn a_short_game_replays_in_every_chess_variant() {
    for (key, moves, ending) in GAMES {
        let state = replay(key, None, moves).unwrap_or_else(|e| panic!("{key}: {e}"));
        assert_eq!(state.history.len(), moves.len(), "{key}");
        match state.outcome {
            Some(outcome) => assert_eq!(&outcome.reason, ending, "{key}"),
            None => {
                assert!(ending.is_empty(), "{key} should have ended by {ending}");
                assert_eq!(state.awaiting, Awaiting::Move);
                assert!(!state.moves.is_empty(), "{key}");
                assert!(!state.targets.is_empty(), "{key}");
            }
        }
    }
}

#[test]
fn an_illegal_move_is_refused_with_its_index() {
    for (key, moves, _) in GAMES {
        let err = apply(key, None, &moves[..1], "e2e5").unwrap_err();
        assert!(
            matches!(err, GameError::Refused { index: 1, .. }),
            "{key}: {err:?}"
        );
    }
}

#[test]
fn the_board_names_its_pieces_with_the_family() {
    let state = replay::<&str>("chess", None, &[]).unwrap();
    assert_eq!(state.board.pieces.len(), 32);
    let e1 = state.board.pieces.iter().find(|p| p.square == "e1").unwrap();
    assert_eq!((e1.x, e1.y, e1.sprite.as_str()), (4, 0, "chess-wK"));
    assert_eq!(state.to_move.unwrap().name, "White");
    assert!(state.moves.iter().any(|m| m.wire == "g1f3" && m.notation == "Nf3"));
    assert_eq!(state.targets["g1"], ["f3", "h3"]);
}

#[test]
fn seirawan_shows_its_hands_and_gates() {
    let state = replay("seirawan", None, &["b1c3h"]).unwrap();
    let white = &state.board.hands[0];
    assert_eq!(white.pieces[0].sprite, "chess-wH");
    assert_eq!(white.pieces[0].count, 0, "the hawk has been gated");
    assert_eq!(white.pieces[1].count, 1, "the elephant is still held");
    assert_eq!(state.history, ["Nc3/H"]);
}

#[test]
fn a_fen_start_is_honoured_and_a_bad_one_refused() {
    let state = replay::<&str>("chess", Some("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1"), &[]).unwrap();
    let outcome = state.outcome.unwrap();
    assert_eq!((outcome.winner, outcome.reason.as_str()), (None, "stalemate"));
    assert!(matches!(
        replay::<&str>("chess", Some("not a fen"), &[]),
        Err(GameError::BadPosition { .. })
    ));
    assert!(start_position("chess").unwrap().starts_with("rnbqkbnr/pppppppp"));
    assert_eq!(bragd_games::chess960_start(518), start_position("chess").unwrap());
}

#[test]
fn the_state_serialises_for_the_front_end() {
    let state = replay("chess", None, &["e2e4"]).unwrap();
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json["variant"], "chess");
    assert_eq!(json["toMove"]["name"], "Black");
    assert_eq!(json["board"]["geometry"], "squares");
    assert_eq!(json["board"]["highlights"][0]["kind"], "lastMove");
}
