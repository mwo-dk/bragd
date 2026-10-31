use bragd_games::{Awaiting, GameError, apply, replay, start_position};

const SMALL: &[&str] = &["23-19", "10-14", "22-17", "11-15", "17x10:14", "7x14:10"];
/// The same game turned half a turn: Black moves first in English checkers.
const CHECKERS: &[&str] = &["10-14", "23-19", "11-16", "22-18", "16x23:19", "26x19:23"];

/// A short legal game in each Variant, each with a capture on both sides.
const GAMES: &[(&str, &[&str])] = &[
    ("checkers", CHECKERS),
    ("draughts", &["33-29", "18-22", "32-27", "19-23", "27x18:22", "13x22:18"]),
    ("draughts-brazilian", SMALL),
    ("draughts-russian", SMALL),
];

#[test]
fn a_short_game_with_captures_replays_in_every_draughts_variant() {
    for (key, moves) in GAMES {
        let state = replay(key, None, moves).unwrap_or_else(|e| panic!("{key}: {e}"));
        assert_eq!(state.history.len(), moves.len(), "{key}");
        assert_eq!(state.history[4], moves[4].split(':').next().unwrap(), "{key}");
        assert_eq!(state.awaiting, Awaiting::Move, "{key}");
        // Six plies later the first Side is to move again: Black in checkers, else White.
        let first = if *key == "checkers" { "Black" } else { "White" };
        assert_eq!(state.to_move.unwrap().name, first, "{key}");
        // Each side lost one man.
        let count = |sprite: &str| state.board.pieces.iter().filter(|p| p.sprite == sprite).count();
        let men = if *key == "draughts" { 19 } else { 11 };
        assert_eq!((count("draughts-wM"), count("draughts-bM")), (men, men), "{key}");
    }
}

#[test]
fn an_illegal_move_is_refused_with_its_index() {
    for (key, moves) in GAMES {
        let err = apply(key, None, &moves[..1], moves[0]).unwrap_err();
        assert!(matches!(err, GameError::Refused { index: 1, .. }), "{key}: {err:?}");
    }
    // A capture written without what it takes is not the wire form.
    let err = apply("checkers", None, &CHECKERS[..4], "16x23").unwrap_err();
    assert!(matches!(err, GameError::Refused { index: 4, .. }), "{err:?}");
}

#[test]
fn the_board_is_numbered_as_printed() {
    let state = replay::<&str>("draughts", None, &[]).unwrap();
    assert_eq!(state.board.labels.len(), 50);
    let corner = state.board.labels.iter().find(|l| (l.x, l.y) == (0, 0)).unwrap();
    assert_eq!(corner.text, "46");
    assert_eq!(state.board.pieces.len(), 40);
    let first = state.board.pieces.iter().find(|p| p.square == "46").unwrap();
    assert_eq!(first.sprite, "draughts-wM");
    assert_eq!(state.targets["31"], ["27", "26"]);
    assert_eq!(state.position, start_position("draughts").unwrap());
}

#[test]
fn only_the_opening_array_is_an_initial_position() {
    let start = start_position("checkers").unwrap();
    assert!(start.starts_with("B:W21,22,"), "{start}");
    assert!(start.ends_with(":B1,2,3,4,5,6,7,8,9,10,11,12"), "{start}");
    assert!(replay::<&str>("checkers", Some(&start), &[]).is_ok());
    assert!(matches!(
        replay::<&str>("checkers", Some("B:W32:B1"), &[]),
        Err(GameError::BadPosition { .. })
    ));
}
