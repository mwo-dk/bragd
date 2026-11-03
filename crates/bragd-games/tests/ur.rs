use bragd_games::{Awaiting, GameError, HighlightKind, apply, bot_move, replay};

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

#[test]
fn each_variant_draws_its_own_board_path_and_dice() {
    // (key, columns, path length, dice, pieces)
    for (key, files, path, dice, pieces) in [
        ("ur", 8, 14, 4, 7),
        ("ur-masters", 8, 16, 3, 7),
        ("ur-blitz", 8, 16, 4, 5),
        ("ur-aseb", 12, 16, 4, 5),
    ] {
        let state = replay(key, None, &["1"]).unwrap();
        assert_eq!(state.board.files, files, "{key}");
        assert_eq!(state.board.cells.len(), 20, "{key}");
        assert_eq!(state.board.paths.len(), 2, "{key}");
        assert!(state.board.paths.iter().all(|p| p.len() == path), "{key}");
        assert_eq!(state.board.pyramids.len(), dice, "{key}");
        assert_eq!(state.board.pyramids.iter().filter(|&&m| m).count(), 1, "{key}");
        assert_eq!(state.board.hands[0].pieces[0].count, pieces, "{key}");
        let start = replay::<&str>(key, None, &[]).unwrap();
        assert_eq!(start.throws.len(), dice + 1, "{key}: nought to {dice}");
    }
    // Aseb's long middle row reaches column 11.
    let aseb = replay::<&str>("ur-aseb", None, &[]).unwrap();
    assert!(aseb.board.cells.iter().any(|c| (c.x, c.y) == (11, 1)));
}

#[test]
fn masters_throws_three_dice_and_a_nought_moves_four() {
    assert!(apply("ur-masters", None, &[] as &[&str], "4").is_err(), "three dice cannot show four");
    let state = replay("ur-masters", None, &["0"]).unwrap();
    assert_eq!(state.board.dice, [4]);
    assert_eq!(state.moves.iter().map(|m| m.wire.as_str()).collect::<Vec<_>>(), ["0-4"]);
    assert_eq!(state.board.pyramids, [false; 3], "no marked tip is up");
}

#[test]
fn a_lone_legal_move_is_forced_and_a_choice_is_not() {
    // The opening throw allows only the entry: forced.
    let state = replay("ur", None, &["2"]).unwrap();
    assert_eq!(state.forced.as_deref(), Some("0-2"));
    // A piece on the board and another waiting: two moves, a choice.
    let state = replay("ur", None, &["2", "0-2", "0", "1"]).unwrap();
    assert!(state.moves.len() > 1);
    assert_eq!(state.forced, None);
    // Waiting for a throw, nothing is forced.
    assert_eq!(replay::<&str>("ur", None, &[]).unwrap().forced, None);
}

#[test]
fn a_throw_with_no_move_is_reported_as_passed() {
    let state = replay("ur", None, &["0"]).unwrap();
    assert_eq!(state.passed.as_deref(), Some("0"));
    assert_eq!(state.to_move.unwrap().name, "Dark");
    let state = replay("ur", None, &["2"]).unwrap();
    assert_eq!(state.passed, None, "a throw with a move waiting has not passed");
}

#[test]
fn the_bot_answers_a_throw_with_a_legal_move_in_every_variant() {
    for key in ["ur", "ur-masters", "ur-blitz", "ur-aseb"] {
        let entries = ["3"];
        let chosen = bot_move(key, None, &entries).unwrap().expect("a move");
        assert!(apply(key, None, &entries, &chosen).is_ok(), "{key}: {chosen}");
        assert_eq!(bot_move::<&str>(key, None, &[]).unwrap(), None, "{key}: no throw yet");
    }
    assert_eq!(bot_move::<&str>("chess", None, &[]).unwrap(), None);
}

#[test]
fn the_move_list_shows_a_turn_as_its_throw_and_its_move() {
    // Light throws 4 onto its rosette and throws again; Dark throws nought and passes.
    let first = replay("ur", None, &["4"]).unwrap().moves[0].wire.clone();
    let second = replay("ur", None, &["4", first.as_str(), "2"]).unwrap().moves[0].wire.clone();
    let record = ["4", first.as_str(), "2", second.as_str(), "0"];
    let state = replay("ur", None, &record).unwrap();
    assert_eq!(state.history, record, "the record itself is unchanged");
    assert_eq!(state.numbers, ["1", "", "2", "", "3"], "a turn is numbered once, at its throw");
    assert_eq!(state.written, ["4:", first.as_str(), "2:", second.as_str(), "0: –"]);
    // A throw waiting for its move shows as one too.
    let state = replay("ur", None, &["4"]).unwrap();
    assert_eq!((state.numbers, state.written), (vec!["1".to_string()], vec!["4:".to_string()]));
}
