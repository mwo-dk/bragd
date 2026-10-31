//! The rules of senet, one at a time.
//!
//! Kept out of `src/position.rs` and given a file of its own because there are
//! more of them than any other family here has, and because almost every one is
//! a *reading* of Kendall rather than a fact — so each test is really a claim
//! about what Bragd plays, and `docs/rules/ur-and-senet-rules.md` is where
//! the argument for it lives.

use rules_senet::board::adjacent;
use rules_senet::position::{Offer, Way};
use rules_senet::{
    BEAUTY, LAST, Move, PIECES, Position, RE_ATOUM, REBIRTH, Side, Spot, THREE_TRUTHS, Throw, WATER,
};

fn throw(value: u8) -> Throw {
    Throw::new(value).expect("a throw of one to five")
}

/// A position from a picture, so a test can say what it means rather than play
/// twenty moves to get there.
fn at(board: &str, off: &str, mover: char) -> Position {
    let picture = format!("{board} {off} {mover}");
    Position::from_picture(&picture)
        .unwrap_or_else(|| panic!("the fixture {picture:?} should be a legal picture"))
}

/// Thirty dots, with the named houses filled in.
fn empty() -> Vec<char> {
    vec!['.'; 30]
}

fn put(board: &mut [char], house: u8, glyph: char) {
    board[house as usize - 1] = glyph;
}

fn picture(board: &[char]) -> String {
    board.iter().collect()
}

fn offer_for(position: &Position, throw: Throw, from: u8) -> Option<Offer> {
    position
        .moves_for(throw)
        .into_iter()
        .find(|offer| offer.chosen.from == from)
}

#[test]
fn the_opening_array_is_seven_a_side_alternating_on_houses_one_to_fourteen() {
    let position = Position::starting();
    for house in 1..=14u8 {
        let who = position.occupant(house);
        let expected = if house % 2 == 0 {
            Side::White
        } else {
            Side::Black
        };
        assert_eq!(who, Some(expected), "house {house} holds the wrong side");
    }
    for house in 15..=30u8 {
        assert_eq!(
            position.occupant(house),
            None,
            "house {house} should be empty"
        );
    }
    // The House of Rebirth begins empty, which the water rule depends on.
    assert_eq!(position.occupant(REBIRTH), None);
    for side in [Side::White, Side::Black] {
        assert_eq!(position.pieces(side).len(), PIECES as usize);
        assert_eq!(position.off(side), 0);
    }
    // **The evens move first**, and they are the seat storage calls white.
    assert_eq!(position.to_move(), Side::White);
    assert_eq!(position.winner(), None);
}

#[test]
fn a_piece_is_protected_by_a_neighbour_along_the_path() {
    let mut board = empty();
    put(&mut board, 5, 'B');
    put(&mut board, 6, 'B');
    put(&mut board, 20, 'B');
    put(&mut board, 21, 'B');
    put(&mut board, 12, 'B');
    // Seven black: 5, 6, 20, 21, 12 and two more out of the way.
    put(&mut board, 1, 'B');
    put(&mut board, 3, 'B');
    put(&mut board, 25, 'W');
    let mut white = 1;
    for house in [2u8, 4, 8, 10, 14, 16] {
        put(&mut board, house, 'W');
        white += 1;
    }
    assert_eq!(white, PIECES, "the fixture must give white seven pieces");
    let position = at(&picture(&board), "00", 'w');

    assert!(position.protected(5), "5 and 6 are neighbours");
    assert!(position.protected(6));
    // **The bends protect**, which is the reading Bragd takes.
    assert!(adjacent(20, 21));
    assert!(
        position.protected(20),
        "20 and 21 are a step apart along the path"
    );
    assert!(position.protected(21));
    // A piece on its own is not.
    assert!(
        !position.protected(12),
        "12 has no neighbour of its own side"
    );
    assert!(!position.protected(7), "an empty house is not protected");
}

#[test]
fn landing_on_a_lone_enemy_exchanges_the_two_and_takes_nothing_off() {
    let mut board = empty();
    put(&mut board, 4, 'W');
    put(&mut board, 7, 'B');
    for house in [10u8, 12, 14, 16, 18, 20] {
        put(&mut board, house, 'W');
    }
    for house in [1u8, 3, 22, 24, 5, 9] {
        put(&mut board, house, 'B');
    }
    let mut position = at(&picture(&board), "00", 'w');

    let offer = offer_for(&position, throw(3), 4).expect("4 to 7 should be legal");
    assert!(offer.exchanges, "landing on a lone enemy is an exchange");
    assert!(!offer.drowns, "only the last three houses drown");

    let before = position.pieces(Side::Black).len();
    position.play(offer.chosen, throw(3));
    assert_eq!(position.occupant(7), Some(Side::White), "the mover arrives");
    assert_eq!(
        position.occupant(4),
        Some(Side::Black),
        "the exchanged piece goes back to the house the mover left"
    );
    assert_eq!(
        position.pieces(Side::Black).len(),
        before,
        "senet takes nothing off the board by contact"
    );
    assert_eq!(position.off(Side::Black), 0);
}

#[test]
fn a_protected_enemy_cannot_be_landed_on() {
    let mut board = empty();
    put(&mut board, 4, 'W');
    put(&mut board, 7, 'B');
    put(&mut board, 8, 'B');
    for house in [10u8, 12, 14, 16, 18, 20] {
        put(&mut board, house, 'W');
    }
    for house in [1u8, 3, 22, 24, 5] {
        put(&mut board, house, 'B');
    }
    let position = at(&picture(&board), "00", 'w');

    assert!(position.protected(7), "the fixture must actually protect 7");
    assert!(
        offer_for(&position, throw(3), 4).is_none(),
        "a protected enemy must not be landed on"
    );
}

#[test]
fn your_own_piece_is_never_a_landing_square() {
    let position = Position::starting();
    // White is on every even house from 2 to 14, so a throw of two from 2 lands
    // on 4, which is white's own.
    assert!(
        offer_for(&position, throw(2), 2).is_none(),
        "senet does not stack"
    );
    // A throw of one from 14 reaches 15, which is empty.
    let offer = offer_for(&position, throw(1), 14).expect("14 to 15 should be legal");
    assert_eq!(offer.chosen.to, Spot::House(15));
}

#[test]
fn the_house_of_beauty_cannot_be_passed() {
    let mut board = empty();
    put(&mut board, 24, 'W');
    for house in [2u8, 4, 6, 8, 10, 12] {
        put(&mut board, house, 'W');
    }
    for house in [1u8, 3, 5, 7, 9, 11, 13] {
        put(&mut board, house, 'B');
    }
    let position = at(&picture(&board), "00", 'w');

    // Two reaches 26 exactly and is legal; three would carry it to 27 past 26.
    let landing = offer_for(&position, throw(2), 24).expect("24 to 26 should be legal");
    assert_eq!(landing.chosen.to, Spot::House(BEAUTY));
    for steps in [3u8, 4, 5] {
        let offer = position
            .moves_for(throw(steps))
            .into_iter()
            .find(|offer| offer.chosen.from == 24);
        assert!(
            offer.is_none(),
            "a throw of {steps} from 24 would pass the House of Beauty"
        );
    }
}

#[test]
fn a_piece_on_the_house_of_beauty_blockades_its_own_side_too() {
    // The consequence worth saying out loud, and a real feature of the game
    // rather than an accident of the rule.
    let mut board = empty();
    put(&mut board, BEAUTY, 'W');
    put(&mut board, 24, 'W');
    for house in [2u8, 4, 6, 8, 10] {
        put(&mut board, house, 'W');
    }
    for house in [1u8, 3, 5, 7, 9, 11, 13] {
        put(&mut board, house, 'B');
    }
    let position = at(&picture(&board), "00", 'w');

    assert!(
        offer_for(&position, throw(2), 24).is_none(),
        "24 cannot reach 26, because its own piece is standing there"
    );
    for steps in [3u8, 4, 5] {
        assert!(
            offer_for(&position, throw(steps), 24).is_none(),
            "and it cannot go round: a throw of {steps} would pass 26"
        );
    }
}

#[test]
fn the_exits_need_their_exact_throws_and_thirty_takes_anything() {
    for (house, needed) in [(BEAUTY, 5u8), (THREE_TRUTHS, 3), (RE_ATOUM, 2)] {
        let mut board = empty();
        put(&mut board, house, 'W');
        for other in [2u8, 4, 6, 8, 10, 12] {
            put(&mut board, other, 'W');
        }
        for other in [1u8, 3, 5, 7, 9, 11, 13] {
            put(&mut board, other, 'B');
        }
        let position = at(&picture(&board), "00", 'w');

        let offer = offer_for(&position, throw(needed), house)
            .unwrap_or_else(|| panic!("{house} should bear off on {needed}"));
        assert!(offer.bears_off);
        assert_eq!(offer.chosen.to, Spot::Off);

        for wrong in (1..=5u8).filter(|value| *value != needed) {
            let offer = offer_for(&position, throw(wrong), house);
            let bears = offer.map(|offer| offer.bears_off).unwrap_or(false);
            assert!(!bears, "{house} must not bear off on {wrong}");
        }
    }

    // House 30 leaves on anything at all.
    let mut board = empty();
    put(&mut board, LAST, 'W');
    for other in [2u8, 4, 6, 8, 10, 12] {
        put(&mut board, other, 'W');
    }
    for other in [1u8, 3, 5, 7, 9, 11, 13] {
        put(&mut board, other, 'B');
    }
    let position = at(&picture(&board), "00", 'w');
    for steps in 1..=5u8 {
        let offer = offer_for(&position, throw(steps), LAST)
            .unwrap_or_else(|| panic!("30 should bear off on {steps}"));
        assert!(offer.bears_off, "30 leaves on any throw");
    }
}

#[test]
fn houses_twenty_eight_and_twenty_nine_never_move_forward() {
    for house in [THREE_TRUTHS, RE_ATOUM] {
        let mut board = empty();
        put(&mut board, house, 'W');
        for other in [2u8, 4, 6, 8, 10, 12] {
            put(&mut board, other, 'W');
        }
        for other in [1u8, 3, 5, 7, 9, 11, 13] {
            put(&mut board, other, 'B');
        }
        let position = at(&picture(&board), "00", 'w');

        for steps in 1..=5u8 {
            if let Some(offer) = offer_for(&position, throw(steps), house) {
                assert!(
                    offer.bears_off,
                    "{house} may only leave the board, never advance — a throw of {steps} moved it to {:?}",
                    offer.chosen.to
                );
            }
        }
    }
}

#[test]
fn a_backward_move_is_played_only_when_nothing_forward_is_legal() {
    // White's one piece sits on 29 and cannot use a throw of four; every other
    // white piece is gone. So the only move left is backwards.
    let mut board = empty();
    put(&mut board, RE_ATOUM, 'W');
    for other in [1u8, 3, 5, 7, 9, 11, 13] {
        put(&mut board, other, 'B');
    }
    let position = at(&picture(&board), "60", 'w');

    let moves = position.moves_for(throw(4));
    assert!(!moves.is_empty(), "something must happen");
    assert!(
        moves.iter().all(|offer| offer.chosen.way == Way::Backward),
        "with nothing forward available, only backward moves are offered"
    );

    // And when something forward *is* available, nothing backward is offered.
    let position = Position::starting();
    let forward = position.moves_for(throw(1));
    assert!(!forward.is_empty());
    assert!(
        forward.iter().all(|offer| offer.chosen.way == Way::Forward),
        "a player may not retreat by choice"
    );
}

#[test]
fn a_piece_landing_in_the_water_ends_the_turn_whatever_was_thrown() {
    let mut board = empty();
    put(&mut board, BEAUTY, 'W');
    for other in [2u8, 4, 6, 8, 10, 12] {
        put(&mut board, other, 'W');
    }
    for other in [1u8, 3, 5, 7, 9, 11, 13] {
        put(&mut board, other, 'B');
    }
    let mut position = at(&picture(&board), "00", 'w');

    // A one from the House of Beauty is the water, and a one normally repeats.
    assert!(throw(1).throws_again(), "a one repeats, ordinarily");
    let offer = offer_for(&position, throw(1), BEAUTY).expect("26 to 27 should be legal");
    assert!(offer.into_the_water);
    let again = position.play(offer.chosen, throw(1));
    assert!(
        !again,
        "touching the water ends the turn whatever was thrown"
    );
    assert_eq!(position.to_move(), Side::Black);
    assert_eq!(position.occupant(WATER), Some(Side::White));
}

#[test]
fn a_drowned_piece_freezes_every_other_piece_its_owner_has() {
    let mut board = empty();
    put(&mut board, WATER, 'W');
    for other in [2u8, 4, 6, 8, 10, 12] {
        put(&mut board, other, 'W');
    }
    for other in [1u8, 3, 5, 7, 9, 11, 13] {
        put(&mut board, other, 'B');
    }
    let position = at(&picture(&board), "00", 'w');
    assert!(position.drowned(Side::White));

    // A throw of two would ordinarily move several pieces. It moves none.
    for steps in [1u8, 2, 3, 5] {
        for offer in position.moves_for(throw(steps)) {
            assert_eq!(
                offer.chosen.from, WATER,
                "a throw of {steps} moved a piece that should be frozen"
            );
        }
    }
}

#[test]
fn the_drowned_escape_on_an_exact_four_or_are_rescued_to_fifteen() {
    let mut board = empty();
    put(&mut board, WATER, 'W');
    for other in [2u8, 4, 6, 8, 10, 12] {
        put(&mut board, other, 'W');
    }
    for other in [1u8, 3, 5, 7, 9, 11, 13] {
        put(&mut board, other, 'B');
    }
    let position = at(&picture(&board), "00", 'w');

    // The exact four bears it off. `31 - 27` is the same arithmetic as every
    // other exit, which is the point of the rule rather than a coincidence.
    let escape = offer_for(&position, throw(4), WATER).expect("an exact four escapes");
    assert!(escape.bears_off);

    // And the rescue, which is available whatever was thrown while 15 is empty.
    for steps in [1u8, 2, 3, 5] {
        let moves = position.moves_for(throw(steps));
        assert!(
            moves
                .iter()
                .any(|offer| offer.chosen.to == Spot::House(REBIRTH)),
            "a throw of {steps} should still allow the rescue to 15"
        );
    }

    // With 15 taken, the rescue is not on offer — by anybody's piece.
    let mut board = empty();
    put(&mut board, WATER, 'W');
    put(&mut board, REBIRTH, 'B');
    for other in [2u8, 4, 6, 8, 10, 12] {
        put(&mut board, other, 'W');
    }
    for other in [1u8, 3, 5, 7, 9, 11, 13] {
        put(&mut board, other, 'B');
    }
    // Black now has eight, counting the one in the water; drop one.
    put(&mut board, 13, '.');
    let blocked = at(&picture(&board), "00", 'w');
    assert!(
        !blocked
            .moves_for(throw(1))
            .iter()
            .any(|offer| offer.chosen.to == Spot::House(REBIRTH)),
        "an occupied House of Rebirth is no rescue"
    );
}

#[test]
fn a_piece_in_the_water_is_never_protected_but_still_protects_its_neighbour() {
    let mut board = empty();
    put(&mut board, WATER, 'W');
    put(&mut board, BEAUTY, 'W');
    for other in [2u8, 4, 6, 8, 10] {
        put(&mut board, other, 'W');
    }
    for other in [1u8, 3, 5, 7, 9, 11, 13] {
        put(&mut board, other, 'B');
    }
    let position = at(&picture(&board), "00", 'w');

    assert!(adjacent(BEAUTY, WATER), "26 and 27 are a step apart");
    assert!(
        !position.protected(WATER),
        "the drowned piece is never protected, whatever stands beside it"
    );
    assert!(
        position.protected(BEAUTY),
        "but it still confers protection outward, which is the letter of the text"
    );
}

#[test]
fn an_exchange_into_the_last_three_houses_drowns_the_victim() {
    let mut board = empty();
    put(&mut board, BEAUTY, 'W');
    put(&mut board, THREE_TRUTHS, 'B');
    for other in [2u8, 4, 6, 8, 10, 12] {
        put(&mut board, other, 'W');
    }
    for other in [1u8, 3, 5, 7, 9, 11] {
        put(&mut board, other, 'B');
    }
    let mut position = at(&picture(&board), "00", 'w');

    let offer = offer_for(&position, throw(2), BEAUTY).expect("26 to 28 should be legal");
    assert!(offer.exchanges);
    assert!(
        offer.drowns,
        "an exchange onto 28 sends the victim to the water"
    );

    position.play(offer.chosen, throw(2));
    assert_eq!(position.occupant(THREE_TRUTHS), Some(Side::White));
    assert_eq!(
        position.occupant(WATER),
        Some(Side::Black),
        "the drowned piece is in the water, not on the mover's origin"
    );
    assert_eq!(position.occupant(BEAUTY), None, "the mover left 26");
}

#[test]
fn a_drowned_piece_protects_the_house_next_to_the_water() {
    // **Found by writing the test below and having it refuse the move.** The
    // drowned piece is never protected itself, but it protects outward like any
    // other — and house 28 is a step from the water. So a piece sitting on the
    // House of the Three Truths while its owner is drowned cannot be touched,
    // which is a real and rather elegant consequence of two rules that were
    // written down separately.
    let mut board = empty();
    put(&mut board, BEAUTY, 'W');
    put(&mut board, THREE_TRUTHS, 'B');
    put(&mut board, WATER, 'B');
    for other in [2u8, 4, 6, 8, 10, 12] {
        put(&mut board, other, 'W');
    }
    for other in [1u8, 3, 5, 7, 9] {
        put(&mut board, other, 'B');
    }
    let position = at(&picture(&board), "00", 'w');

    assert!(adjacent(WATER, THREE_TRUTHS));
    assert!(
        position.protected(THREE_TRUTHS),
        "the drowned piece guards 28"
    );
    assert!(!position.protected(WATER), "and is not guarded itself");
    assert!(
        offer_for(&position, throw(2), BEAUTY).is_none(),
        "26 to 28 must be refused while the water guards it"
    );
}

#[test]
fn a_drowning_with_the_water_taken_falls_back_to_the_house_of_beauty() {
    // The victim is on 30 rather than 28, because a drowned piece guards 28 —
    // see the test above, which is how this fixture came to be written this way.
    let mut board = empty();
    put(&mut board, BEAUTY, 'W');
    put(&mut board, LAST, 'B');
    put(&mut board, WATER, 'B');
    for other in [2u8, 4, 6, 8, 10, 12] {
        put(&mut board, other, 'W');
    }
    for other in [1u8, 3, 5, 7, 9] {
        put(&mut board, other, 'B');
    }
    let mut position = at(&picture(&board), "00", 'w');

    assert!(
        !position.protected(LAST),
        "nothing of black's stands beside 30"
    );
    let offer = offer_for(&position, throw(4), BEAUTY).expect("26 to 30 should be legal");
    assert!(offer.exchanges);
    assert!(
        !offer.drowns,
        "the water is taken, so the ordinary exchange applies"
    );
    position.play(offer.chosen, throw(4));
    assert_eq!(
        position.occupant(LAST),
        Some(Side::White),
        "the mover arrives"
    );
    assert_eq!(
        position.occupant(BEAUTY),
        Some(Side::Black),
        "the displaced piece goes back to 26 rather than into an occupied water"
    );
    assert_eq!(
        position.occupant(WATER),
        Some(Side::Black),
        "the water is unchanged"
    );
}

#[test]
fn seven_home_wins_and_nothing_less_does() {
    let mut board = empty();
    put(&mut board, LAST, 'W');
    for other in [1u8, 3, 5, 7, 9, 11, 13] {
        put(&mut board, other, 'B');
    }
    let mut position = at(&picture(&board), "60", 'w');
    assert_eq!(position.winner(), None, "six home is not a win");

    let offer = offer_for(&position, throw(1), LAST).expect("30 leaves on anything");
    position.play(offer.chosen, throw(1));
    assert_eq!(position.winner(), Some(Side::White));
    assert!(
        position.moves_for(throw(2)).is_empty(),
        "a finished game offers nothing"
    );
}

#[test]
fn a_picture_survives_the_round_trip_and_rubbish_does_not_parse() {
    let position = Position::starting();
    let picture = position.to_picture();
    assert_eq!(Position::from_picture(&picture).as_ref(), Some(&position));
    assert_eq!(
        Position::from_picture(&picture).unwrap().to_picture(),
        picture
    );

    for bad in [
        "",
        "..............................",
        ".............................. 00",
        ".............................. 00 x",
        "............................. 00 w",
        "............................... 00 w",
        ".............................. 0 w",
        ".............................. 00 w extra",
        "W............................. 00 w",
        "X............................. 00 w",
        ".............................. aa w",
    ] {
        assert_eq!(
            Position::from_picture(bad),
            None,
            "{bad:?} should be refused"
        );
    }
}

#[test]
fn a_move_survives_the_wire_in_both_directions() {
    let cases = [
        Move {
            from: 4,
            to: Spot::House(7),
            way: Way::Forward,
        },
        Move {
            from: 12,
            to: Spot::House(9),
            way: Way::Backward,
        },
        Move {
            from: LAST,
            to: Spot::Off,
            way: Way::Forward,
        },
        Move {
            from: WATER,
            to: Spot::House(REBIRTH),
            way: Way::Backward,
        },
    ];
    for chosen in cases {
        let wire = chosen.to_wire();
        assert!(
            !wire.contains(' '),
            "an entry must be one token, got {wire:?}"
        );
        assert_eq!(Move::parse(&wire), Some(chosen), "{wire:?} did not survive");
    }
    // The direction really is carried: the same two houses, both ways.
    assert_ne!(
        Move {
            from: 12,
            to: Spot::House(9),
            way: Way::Backward
        }
        .to_wire(),
        Move {
            from: 12,
            to: Spot::House(9),
            way: Way::Forward
        }
        .to_wire()
    );

    for bad in [
        "", "-", "4", "4-", "-4", "a-b", "0-4", "4-31", "31-4", "1-2-3", "<", "<-", "04-7",
    ] {
        assert_eq!(Move::parse(bad), None, "{bad:?} should not parse");
    }
}
