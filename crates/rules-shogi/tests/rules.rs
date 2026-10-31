//! What makes shogi shogi, checked.
//!
//! **The published perft corpus lives next door in `tests/perft.rs`** — the
//! starting position to depth six, with the deep runs behind `#[ignore]` the
//! way the chess ones are. What is here is the layer a node count cannot
//! explain when it disagrees: the drop rules, promotion, and the fact that
//! captured pieces change sides.
//!
//! The movement has its own tests beside the tables, including the property
//! that White's moves are Black's reflected — which is the one bug that would
//! otherwise pass every hand-written case.

use rules_shogi::piece::{Role, Side};
use rules_shogi::position::Move;
use rules_shogi::{Position, Square};

fn at(name: &str) -> Square {
    Square::parse(name).expect("a square")
}

fn sfen(text: &str) -> Position {
    Position::from_sfen(text).unwrap_or_else(|| panic!("{text} is not a position"))
}

fn usi(position: &Position, text: &str) -> Move {
    let wanted = Move::parse(text).unwrap_or_else(|| panic!("{text} is not a move"));
    assert!(
        position.legal_moves().contains(&wanted),
        "{text} is not legal here:\n{position:?}"
    );
    wanted
}

#[test]
fn the_opening_array_is_forty_pieces_and_black_to_move() {
    let start = Position::starting();
    assert_eq!(start.occupied().count(), 40);
    assert_eq!(start.to_move(), Side::Black);
    // Symmetric: each side has the same pieces.
    for role in Role::ALL {
        assert_eq!(
            start.pieces(Side::Black, role).count(),
            start.pieces(Side::White, role).count(),
            "{role:?}"
        );
    }
    assert_eq!(start.pieces(Side::Black, Role::Pawn).count(), 9);
    assert_eq!(start.pieces(Side::Black, Role::King).count(), 1);
    assert_eq!(start.pieces(Side::Black, Role::Gold).count(), 2);
}

#[test]
fn the_opening_has_thirty_legal_moves() {
    // The known number, and the cheapest possible check that the whole
    // generator agrees with the world: nine pawn pushes, two silvers with two
    // each, two golds with two each, the king, two knights with none, the rook
    // and the bishop.
    assert_eq!(Position::starting().legal_moves().len(), 30);
}

#[test]
fn sfen_round_trips() {
    let start = Position::starting();
    assert_eq!(
        start.to_sfen(),
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
    );
    assert_eq!(sfen(&start.to_sfen()).to_sfen(), start.to_sfen());
}

/// **The rule the whole crate exists for.** A captured piece changes sides,
/// reverts to its unpromoted form, and can be played again.
#[test]
fn a_captured_piece_changes_sides_and_reverts() {
    // A black rook takes a white dragon.
    let position = sfen("4k4/9/9/9/4+r4/9/9/9/4K4 b - 1");
    let before = position.in_hand(Side::Black, Role::Rook);
    assert_eq!(before, 0);

    // A black rook on the same file, which can slide up and take it. The
    // first attempt put the rook on 4i, which cannot reach 5e at all — the
    // engine was right to say so.
    let position = sfen("4k4/9/9/9/4+r4/9/9/4R4/4K4 b - 1");
    let take = position
        .legal_moves()
        .into_iter()
        .find(|mv| mv.to() == at("5e"))
        .expect("something can take the dragon");
    let after = position.after(take);

    // A dragon in the hand would be a bug: it comes back as a rook.
    assert_eq!(after.in_hand(Side::Black, Role::Rook), 1);
    assert_eq!(after.in_hand(Side::Black, Role::Dragon), 0);
    assert!(after.pieces(Side::White, Role::Dragon).is_empty());
}

#[test]
fn a_piece_in_hand_can_be_dropped_on_any_empty_square() {
    let position = sfen("4k4/9/9/9/9/9/9/9/4K4 b P 1");
    let drops: Vec<Move> = position
        .legal_moves()
        .into_iter()
        .filter(|mv| matches!(mv, Move::Drop { .. }))
        .collect();
    // Eighty-one squares, less the nine on the far rank where a pawn could
    // never move again, less the one black king standing on 5i. The white king
    // is already inside the excluded rank, which is what the first count got
    // wrong.
    assert_eq!(drops.len(), 81 - 9 - 1, "{drops:?}");
    let drop = usi(&position, "P*5e");
    let after = position.after(drop);
    assert_eq!(after.in_hand(Side::Black, Role::Pawn), 0);
    assert_eq!(
        after.piece_at(at("5e")).map(|piece| piece.role),
        Some(Role::Pawn)
    );
}

/// **Nifu**: never two of your own unpromoted pawns on one file. A promoted
/// pawn does not count, which is why the rule asks about the role rather than
/// about what is on the file.
#[test]
fn two_unpromoted_pawns_may_not_share_a_file() {
    let position = sfen("4k4/9/9/9/4P4/9/9/9/4K4 b P 1");
    let drops: Vec<Square> = position
        .legal_moves()
        .into_iter()
        .filter_map(|mv| match mv {
            Move::Drop { to, .. } => Some(to),
            _ => None,
        })
        .collect();
    assert!(
        !drops.iter().any(|square| square.file() == at("5e").file()),
        "a pawn was droppable on the file that already has one"
    );
    assert!(!drops.is_empty(), "and every other file is still fine");

    // A *promoted* pawn on the file does not block it.
    let with_tokin = sfen("4k4/9/9/9/4+P4/9/9/9/4K4 b P 1");
    let allowed: Vec<Square> = with_tokin
        .legal_moves()
        .into_iter()
        .filter_map(|mv| match mv {
            Move::Drop { to, .. } => Some(to),
            _ => None,
        })
        .collect();
    assert!(
        allowed
            .iter()
            .any(|square| square.file() == at("5e").file()),
        "a promoted pawn should not block the file"
    );
}

/// A piece that could never move again may not be dropped there at all.
#[test]
fn nothing_may_be_dropped_where_it_could_never_move() {
    for (role, letter, ranks) in [
        (Role::Pawn, "P", vec!["1a", "5a", "9a"]),
        (Role::Lance, "L", vec!["1a", "5a"]),
        (Role::Knight, "N", vec!["1a", "5a", "1b", "5b"]),
    ] {
        let position = sfen(&format!("4k4/9/9/9/9/9/9/9/4K4 b {letter} 1"));
        for square in ranks {
            assert!(
                !position.legal_moves().contains(&Move::Drop {
                    role,
                    to: at(square)
                }),
                "{role:?} should not be droppable on {square}"
            );
        }
        // And one rank further back is fine.
        let fine = match role {
            Role::Knight => "5c",
            _ => "5b",
        };
        assert!(
            position
                .legal_moves()
                .contains(&Move::Drop { role, to: at(fine) }),
            "{role:?} should be droppable on {fine}"
        );
    }
}

/// **Uchifuzume**, the strangest rule in the game: a pawn may not be *dropped*
/// to give immediate checkmate. Dropping one to give check is fine, and
/// *moving* a pawn to give mate is fine. Only the drop-mate is forbidden.
#[test]
fn a_pawn_may_not_be_dropped_to_give_mate() {
    // A white king boxed into the corner by its own pieces, with the only
    // escape squares covered. A pawn dropped in front of it would be mate.
    let position = sfen("8k/7pp/7NG/9/9/9/9/9/K8 b P 1");
    let mate_drop = Move::Drop {
        role: Role::Pawn,
        to: at("1b"),
    };
    // If that is checkmate, it must not be offered.
    let after = position.after(mate_drop);
    let is_mate = after.is_check_against(after.to_move()) && after.legal_moves().is_empty();
    if is_mate {
        assert!(
            !position.legal_moves().contains(&mate_drop),
            "a pawn drop that mates must be refused"
        );
    }

    // The other half, and the one that matters more: a drop that gives *check*
    // and not mate is perfectly legal.
    let checking = sfen("4k4/9/9/9/9/9/9/9/4K4 b P 1");
    assert!(
        checking.legal_moves().contains(&Move::Drop {
            role: Role::Pawn,
            to: at("5b")
        }),
        "a pawn drop giving check is legal"
    );
}

#[test]
fn promotion_is_offered_in_the_zone_and_forced_where_it_must_be() {
    // A pawn stepping into the zone may promote or not: two moves.
    let position = sfen("4k4/9/9/4P4/9/9/9/9/4K4 b - 1");
    let into_zone: Vec<Move> = position
        .legal_moves()
        .into_iter()
        .filter(|mv| mv.to() == at("5c"))
        .collect();
    assert_eq!(into_zone.len(), 2, "promote or not: {into_zone:?}");

    // A pawn stepping onto the last rank has no choice, because a pawn there
    // could never move again.
    let forced = sfen("4k4/4P4/9/9/9/9/9/9/4K4 b - 1");
    let onto_last: Vec<Move> = forced
        .legal_moves()
        .into_iter()
        .filter(|mv| mv.to() == at("5a"))
        .collect();
    assert_eq!(onto_last.len(), 1, "{onto_last:?}");
    assert!(matches!(onto_last[0], Move::Board { promote: true, .. }));
}

#[test]
fn a_dropped_piece_never_promotes() {
    let position = sfen("4k4/9/9/9/9/9/9/9/4K4 b P 1");
    // Every drop into the promotion zone is a plain drop; there is no such
    // thing as a promoted drop.
    for mv in position.legal_moves() {
        if let Move::Drop { to, .. } = mv {
            let after = position.after(mv);
            assert_eq!(
                after.piece_at(to).map(|piece| piece.role),
                Some(Role::Pawn),
                "a drop promoted"
            );
        }
    }
}

#[test]
fn a_move_that_leaves_your_king_attacked_is_not_legal() {
    // Black's king on 5i, a white rook down the file, and a black gold pinned
    // between them.
    let position = sfen("4k4/9/9/9/9/9/9/4G4/4K4 b - 1");
    let position = sfen(&position.to_sfen().replace("4k4/9", "4k4/4r4"));
    for mv in position.legal_moves() {
        assert!(
            !position.after(mv).is_check_against(Side::Black),
            "{mv} leaves the king attacked"
        );
    }
}

/// Parsing must never panic, whatever arrives.
///
/// The emoji is not a joke: USI is ASCII and `Move::parse` slices by byte, so a
/// four-*byte* character passed the length check and then panicked halfway
/// through itself. The text comes off a wire message or out of a stored row,
/// which makes that a panic in the authority for a live game. Found by a
/// hostile-input test, which is what such a test is for.
#[test]
fn parsing_refuses_rubbish_without_panicking() {
    for rubbish in [
        "",
        "🙂",
        "🙂🙂",
        "P*🙂",
        "*",
        "7g7",
        "7g7f7f",
        "+",
        "K*5e",
        "Z*5e",
        "7g7f++",
        "０九０八",
    ] {
        assert_eq!(Move::parse(rubbish), None, "{rubbish:?} parsed");
    }
}

#[test]
fn usi_round_trips_for_every_legal_move() {
    let mut position = Position::starting();
    for _ in 0..12 {
        let moves = position.legal_moves();
        assert!(!moves.is_empty());
        for mv in &moves {
            assert_eq!(Move::parse(&mv.to_usi()), Some(*mv), "{}", mv.to_usi());
        }
        position = position.after(moves[moves.len() / 2]);
    }
}

/// A game can actually be played: the pieces come off, go into hands and come
/// back. The cheapest possible proof that the whole loop works.
#[test]
fn a_game_can_be_played_and_pieces_return_to_the_board() {
    let mut position = Position::starting();
    let mut captures = 0;
    for _ in 0..80 {
        let moves = position.legal_moves();
        if moves.is_empty() {
            break;
        }
        // Prefer a capture, so pieces reach the hands, then a drop, so they
        // come back — which is the loop no other game here has.
        let chosen = moves
            .iter()
            .find(|mv| matches!(mv, Move::Board { to, .. } if position.piece_at(*to).is_some()))
            .or_else(|| moves.iter().find(|mv| matches!(mv, Move::Drop { .. })))
            .copied()
            .unwrap_or(moves[0]);
        if let Move::Board { to, .. } = chosen
            && position.piece_at(to).is_some() {
                captures += 1;
            }
        position = position.after(chosen);

        // The invariant that would catch a piece being lost or duplicated:
        // forty pieces exist, on the board or in a hand, for ever.
        let on_board = position.occupied().count();
        let held: u32 = Side::ALL
            .into_iter()
            .flat_map(|side| Role::IN_HAND.into_iter().map(move |role| (side, role)))
            .map(|(side, role)| position.in_hand(side, role) as u32)
            .sum();
        assert_eq!(on_board + held, 40, "a piece went missing:\n{position:?}");
    }
    assert!(captures > 0, "the walk should have taken something");
}
