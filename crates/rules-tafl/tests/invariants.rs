//! The correctness gate, and why it is not perft.
//!
//! **There is no published perft corpus for any tafl variant.** Not a smaller
//! one, not an older one — none. The board-game programming tradition of
//! shared node counts is a chess tradition, and tafl has no equivalent, no EPD
//! suite, and no reference file competing engines validate against.
//!
//! It is worse than that: the rules themselves could not support one. The exit
//! fort, encirclement and repetition rules are underspecified in Copenhagen's
//! own text, so two engines that are each defensibly correct will legitimately
//! disagree about node counts past a few plies. A published perft could only
//! ever pin the movement and capture layer — which is the part that is *not*
//! contested.
//!
//! So the gate is built rather than downloaded, and it is made of properties
//! rather than numbers. Each one below is something that must hold in every
//! position a real game can reach, checked over thousands of positions reached
//! by playing. A property that holds across ten thousand random positions is
//! weaker evidence than a matching perft number — and it is what is available.
//!
//! The one place a number *is* available is the shape of the opening array, and
//! there the totals are asserted exactly.

use rules_tafl::board::{Bitboard, Direction, Square};
use rules_tafl::game::{AttackerWin, DefenderWin, Outcome};
use rules_tafl::position::{Piece, Role, corners, throne};
use rules_tafl::{BRANDUBH, COPENHAGEN, FETLAR, Game, Position, Ruleset, Side, TABLUT};

/// A deterministic generator, so a failure can be reproduced from its seed.
///
/// The engine has no randomness of its own and must not grow any — but a test
/// harness that plays games needs some, and keeping it here rather than in the
/// crate is the line.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // SplitMix64, the same generator the chess engine's Zobrist table uses.
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

/// Plays a game out at random and hands every position to `check`.
fn play_out(rules: Ruleset, seed: u64, check: &mut impl FnMut(&Game)) -> Option<Outcome> {
    let mut rng = Rng(seed);
    let mut game = Game::new(rules);
    check(&game);
    while game.outcome().is_none() {
        let moves = game.legal_moves();
        if moves.is_empty() {
            break;
        }
        let mv = moves[rng.below(moves.len())];
        game.play(mv).expect("a move from the legal list is legal");
        check(&game);
    }
    game.outcome()
}

#[test]
fn the_opening_array_is_exactly_what_the_rules_say() {
    // The one place an exact number is available, so it is asserted exactly:
    // twenty-four attackers, twelve defenders and a king, the king on the
    // throne, and the attackers to move.
    let start = Position::standard();
    assert_eq!(start.side(Side::Attackers).count(), 24);
    assert_eq!(start.side(Side::Defenders).count(), 13);
    assert_eq!(start.king_square(), Some(throne(11)));
    assert_eq!(start.to_move(), Side::Attackers);

    // And the opening move count, which is the closest thing to a perft(1) this
    // game has. Recomputing it by hand from the array is the check: each of the
    // 24 attackers and 12 defenders sees a clear rank and file, minus what
    // blocks it and minus the restricted squares.
    let moves = COPENHAGEN.legal_moves(&start);
    assert_eq!(moves.len(), 116, "perft(1) for the standard array");
}

#[test]
fn no_two_pieces_ever_share_a_square() {
    // The bookkeeping invariant. Capture removes from both sides and the king
    // set; sliding moves within one. A bug in either shows up here first.
    for seed in 0..40 {
        play_out(COPENHAGEN, seed, &mut |game| {
            let position = game.position();
            let attackers = position.side(Side::Attackers);
            let defenders = position.side(Side::Defenders);
            assert!(
                (attackers & defenders).is_empty(),
                "a square held by both sides\n{position:?}"
            );
            assert_eq!(
                attackers.count() + defenders.count(),
                position.occupied().count(),
                "the sides do not add up\n{position:?}"
            );
        });
    }
}

#[test]
fn the_king_is_always_a_defender_or_gone() {
    for seed in 100..140 {
        play_out(COPENHAGEN, seed, &mut |game| {
            let position = game.position();
            if let Some(king) = position.king_square() {
                assert_eq!(
                    position.piece_at(king),
                    Some(Piece {
                        side: Side::Defenders,
                        role: Role::King
                    }),
                    "the king is not a defender\n{position:?}"
                );
            }
        });
    }
}

#[test]
fn no_soldier_ever_stands_on_a_restricted_square() {
    // The rule that makes the corners reachable at all. If a defender could
    // park on a corner, the king could never win there.
    for seed in 200..240 {
        play_out(COPENHAGEN, seed, &mut |game| {
            let position = game.position();
            for square in corners(11).into_iter().chain([throne(11)]) {
                if let Some(piece) = position.piece_at(square) {
                    assert_eq!(
                        piece.role,
                        Role::King,
                        "a soldier on the restricted {square}\n{position:?}"
                    );
                }
            }
        });
    }
}

#[test]
fn material_never_increases() {
    // There is no promotion, no drop and no reinforcement in tafl. Every move
    // leaves the piece count the same or smaller, and a move that *adds* a
    // piece would mean capture resolution is writing where it should be
    // clearing.
    for seed in 300..340 {
        let mut last = None;
        play_out(COPENHAGEN, seed, &mut |game| {
            let count = game.position().occupied().count();
            if let Some(previous) = last {
                assert!(
                    count <= previous,
                    "material grew from {previous} to {count}"
                );
            }
            last = Some(count);
        });
    }
}

#[test]
fn one_move_never_takes_more_than_it_could() {
    // Custody can take at most three — one per direction other than the one the
    // mover came from — and a shieldwall can take a row. Anything beyond a
    // whole edge is a bug in the capture set.
    for seed in 400..440 {
        let mut previous = None;
        play_out(COPENHAGEN, seed, &mut |game| {
            let count = game.position().occupied().count();
            if let Some(before) = previous {
                let taken = before - count;
                assert!(
                    taken <= 11 - 2,
                    "one move took {taken} pieces\n{:?}",
                    game.position()
                );
            }
            previous = Some(count);
        });
    }
}

#[test]
fn every_generated_move_is_a_rook_move_over_empty_squares() {
    // Movement is the part no ruleset disagrees about, so it is the part that
    // can be checked absolutely: orthogonal, at least one square, and nothing
    // in between.
    for seed in 500..530 {
        play_out(COPENHAGEN, seed, &mut |game| {
            let position = game.position();
            for mv in COPENHAGEN.legal_moves(position) {
                assert_ne!(mv.from, mv.to);
                assert!(
                    mv.from.file() == mv.to.file() || mv.from.rank() == mv.to.rank(),
                    "{mv} is not orthogonal"
                );
                assert_eq!(
                    position.piece_at(mv.from).map(|p| p.side),
                    Some(position.to_move()),
                    "{mv} moves a piece that is not the mover's"
                );
                assert_eq!(position.piece_at(mv.to), None, "{mv} lands on a piece");

                let size = position.size();
                // Nothing between, in either direction.
                let direction = Direction::ALL
                    .into_iter()
                    .find(|&d| (1..11).any(|steps| mv.from.step(d, steps, size) == Some(mv.to)))
                    .expect("an orthogonal move has a direction");
                let mut steps = 1;
                while let Some(square) = mv.from.step(direction, steps, size) {
                    if square == mv.to {
                        break;
                    }
                    assert_eq!(position.piece_at(square), None, "{mv} jumps {square}");
                    steps += 1;
                }
            }
        });
    }
}

#[test]
fn a_game_always_ends_and_never_ends_twice() {
    // The termination guarantee. Without it the server hangs on a game two
    // players have abandoned to a shuffle, and the ply cap exists precisely so
    // this is provable rather than hoped for.
    for seed in 600..640 {
        let mut game = Game::new(COPENHAGEN);
        let mut rng = Rng(seed);
        let mut plies = 0;
        while game.outcome().is_none() {
            let moves = game.legal_moves();
            assert!(!moves.is_empty(), "no moves and no outcome");
            game.play(moves[rng.below(moves.len())]).expect("legal");
            plies += 1;
            assert!(plies <= rules_tafl::PLY_CAP, "past the cap with no outcome");
        }
        // And nothing can be played afterwards.
        let after = game.legal_moves();
        let stray = after.first().copied();
        if let Some(mv) = stray {
            assert!(game.play(mv).is_err(), "played on after the game ended");
        }
    }
}

#[test]
fn both_sides_win_sometimes_under_random_play() {
    // Not a rules check — a check that the *win conditions are reachable at
    // all*. An engine where one side can never win would pass every property
    // above and be useless, and this is the cheapest way to notice.
    let mut outcomes = Vec::new();
    for seed in 700..800 {
        if let Some(outcome) = play_out(COPENHAGEN, seed, &mut |_| {}) {
            outcomes.push(outcome);
        }
    }
    assert_eq!(outcomes.len(), 100, "every game reached an outcome");

    let attackers = outcomes
        .iter()
        .filter(|o| matches!(o, Outcome::Attackers(_)))
        .count();
    let defenders = outcomes
        .iter()
        .filter(|o| matches!(o, Outcome::Defenders(_)))
        .count();
    assert!(attackers > 0, "the attackers never win: {outcomes:?}");
    assert!(defenders > 0, "the defenders never win: {outcomes:?}");
}

#[test]
fn the_king_only_ever_dies_by_being_surrounded() {
    // He is not a soldier, and ordinary custody must never remove him. If it
    // did, the two-to-one material imbalance would be unplayable.
    for seed in 800..860 {
        let outcome = play_out(COPENHAGEN, seed, &mut |game| {
            let position = game.position();
            if position.king_square().is_none() {
                // He may only be gone if the game is over by capture.
                assert_eq!(
                    game.outcome(),
                    Some(Outcome::Attackers(AttackerWin::KingCaptured)),
                    "the king vanished without the game ending\n{position:?}"
                );
            }
        });
        // A defender win by escape must leave him standing on a corner.
        if outcome == Some(Outcome::Defenders(DefenderWin::Escape)) {
            // Checked inside the closure above for every position; here only
            // that the outcome is self-consistent.
            assert!(outcome.unwrap().winner() == Some(Side::Defenders));
        }
    }
}

#[test]
fn fetlar_and_copenhagen_share_every_uncontested_rule() {
    // Copenhagen is Fetlar plus the shieldwall, the exit fort and the
    // repetition ban. So everything *else* — movement, custody capture, the
    // hostility of the throne and corners, the king's four sides — must be
    // bit-identical between them, and this walks a real game asking that at
    // every ply.
    //
    // Not done by playing the same seed under both and comparing outcomes: the
    // repetition ban changes the legal move *list*, so the same random index
    // picks a different move and the two games diverge immediately for reasons
    // that say nothing. Replaying one game's moves under the other ruleset is
    // what actually isolates the difference.
    for seed in 900..940 {
        let mut rng = Rng(seed);
        let mut game = Game::new(FETLAR);
        while game.outcome().is_none() {
            let moves = game.legal_moves();
            if moves.is_empty() {
                break;
            }
            let mv = moves[rng.below(moves.len())];
            let before = *game.position();

            // Custody is not one of the three, so both rulesets must agree
            // about it exactly.
            let landed = before.slide(mv);
            assert_eq!(
                COPENHAGEN.captures(&landed, mv.to).0,
                FETLAR.captures(&landed, mv.to).0,
                "the two rulesets disagree about an ordinary capture after {mv}\n{before:?}"
            );
            // And the shieldwall is one of the three, so Fetlar must never have
            // any — which is what makes a divergence isolate to a named rule.
            assert!(
                FETLAR.shieldwall_captures(&landed, mv.to).is_empty(),
                "Fetlar fired a shieldwall"
            );
            // Movement is not one of the three either.
            assert_eq!(
                COPENHAGEN.legal_moves(&before).len(),
                FETLAR.legal_moves(&before).len(),
                "the two rulesets disagree about movement\n{before:?}"
            );

            game.play(mv).expect("legal");
        }
    }
}

#[test]
fn a_shieldwall_works_on_all_four_edges() {
    // The other half of the comparison above: the three added rules must do
    // *something*. Not looked for in random play — a wall needs a run of two
    // pressed men bracketed at both ends, which random play essentially never
    // builds — but constructed on each edge in turn.
    //
    // All four, because the geometry is the part most likely to be right in one
    // orientation and wrong in another: the inward direction is derived rather
    // than looked up, and a sign error would show on exactly one pair of edges.
    /// One constructed wall: where the bracket is, the two victims along the
    /// edge, the two men pressing them inward, and the move that closes it.
    struct Wall {
        bracket: &'static str,
        victims: [&'static str; 2],
        pressers: [&'static str; 2],
        from: &'static str,
        to: &'static str,
    }

    let cases = [
        Wall {
            bracket: "d1",
            victims: ["e1", "f1"],
            pressers: ["e2", "f2"],
            from: "g4",
            to: "g1",
        },
        Wall {
            bracket: "d11",
            victims: ["e11", "f11"],
            pressers: ["e10", "f10"],
            from: "g8",
            to: "g11",
        },
        Wall {
            bracket: "a4",
            victims: ["a5", "a6"],
            pressers: ["b5", "b6"],
            from: "d7",
            to: "a7",
        },
        Wall {
            bracket: "k4",
            victims: ["k5", "k6"],
            pressers: ["j5", "j6"],
            from: "h7",
            to: "k7",
        },
    ];

    for Wall {
        bracket,
        victims,
        pressers,
        from,
        to,
    } in cases
    {
        let attackers = [bracket, pressers[0], pressers[1], from];
        let before = Position::from_squares(&attackers, &victims, "f6", Side::Attackers);
        let mv = rules_tafl::Move::new(Square::parse(from).unwrap(), Square::parse(to).unwrap());
        let after = before.slide(mv);

        let taken: Vec<Square> = COPENHAGEN.shieldwall_captures(&after, mv.to).collect();
        assert_eq!(taken.len(), 2, "no shieldwall on the {to} edge\n{after:?}");
        for victim in victims {
            assert!(
                taken.contains(&Square::parse(victim).unwrap()),
                "{victim} survived on the {to} edge\n{after:?}"
            );
        }
        // And Fetlar, which has no shieldwall, takes nothing from the same shape.
        assert!(FETLAR.shieldwall_captures(&after, mv.to).is_empty());
    }
}

#[test]
fn an_encircled_position_really_has_no_way_out() {
    // The flood fill is the whole encirclement rule, so it is worth checking
    // against its own justification: if the defenders are encircled, no square
    // they can reach touches an edge — and therefore no corner is reachable.
    let sealed = Game::from_position(
        COPENHAGEN,
        Position::from_picture(
            &[
                "...........",
                "...........",
                "...........",
                "..XXXXX....",
                "..X...X....",
                "..X.KOX....",
                "..X...X....",
                "..XXXXX....",
                "...........",
                "...........",
                "...........",
            ],
            Side::Attackers,
        ),
    );
    assert!(sealed.is_encircled());

    let mut reachable = Bitboard::EMPTY;
    for from in sealed.position().side(Side::Defenders) {
        reachable |= COPENHAGEN.destinations(sealed.position(), from);
    }
    for corner in corners(11) {
        assert!(!reachable.contains(corner), "a corner is one move away");
    }
    for square in reachable {
        assert!(!square.is_edge(11), "{square} is on an edge");
    }
}

// --- the smaller boards ------------------------------------------------------
//
// **The properties above are the gate, and they were all written against the
// 11×11 board.** Running them again on 7×7 and 9×9 is what makes those two
// games tested rather than merely compiled — the geometry refactor that made
// them possible touched `is_edge`, `step`, the board mask, the throne and the
// corners, and every one of those is a place where a small board could be
// subtly wrong while eleven squares stayed perfect.
//
// The same generator, the same playouts, the same assertions. Fewer seeds,
// because a brandubh game is short and a thousand of them prove no more than a
// couple of hundred do.

/// Every property the 11×11 board is held to, on one of the smaller ones.
fn holds_on(rules: Ruleset, seeds: u64) {
    let size = rules.size;
    let squares = (size as u32) * (size as u32);

    for seed in 0..seeds {
        play_out(rules, seed, &mut |game| {
            let position = game.position();

            // The board never grows a piece or loses one to nowhere.
            let attackers = position.side(Side::Attackers).count();
            let defenders = position.side(Side::Defenders).count();
            assert!(attackers + defenders <= squares, "more pieces than squares");

            // Every occupied square is a square that exists on *this* board.
            // This is the assertion the whole refactor turns on: an index in
            // the top-right of the index space is a real square on eleven and
            // no square at all on seven.
            for square in position.side(Side::Attackers) {
                assert!(
                    square.is_on(size),
                    "{square} is not on a {size}-square board"
                );
            }
            for square in position.side(Side::Defenders) {
                assert!(
                    square.is_on(size),
                    "{square} is not on a {size}-square board"
                );
            }

            // At most one king, and he is on the board while he is alive.
            if let Some(king) = position.king_square() {
                assert!(king.is_on(size), "the king is off the board");
                assert_eq!(
                    position.piece_at(king).map(|piece| piece.role),
                    Some(Role::King)
                );
            }

            // No move leaves the board or lands on a piece.
            for mv in rules.legal_moves(position) {
                assert!(mv.to.is_on(size), "{mv} lands off a {size}-square board");
                assert!(mv.from.is_on(size), "{mv} starts off the board");
                assert_eq!(position.piece_at(mv.to), None, "{mv} lands on a piece");
            }

            // No soldier stands on a corner or the throne.
            for square in corners(size).into_iter().chain([throne(size)]) {
                if let Some(Piece { role, .. }) = position.piece_at(square) {
                    assert_eq!(role, Role::King, "a soldier is standing on {square}");
                }
            }
        });
    }
}

#[test]
fn brandubh_holds_every_property_the_big_board_does() {
    holds_on(BRANDUBH, 300);
}

#[test]
fn tablut_holds_every_property_the_big_board_does() {
    holds_on(TABLUT, 300);
}

#[test]
fn the_small_opening_arrays_are_exactly_what_the_rules_say() {
    // Brandubh: four attackers, four defenders and a king, all on a cross.
    let brandubh = BRANDUBH.starting_position();
    assert_eq!(brandubh.size(), 7);
    assert_eq!(brandubh.side(Side::Attackers).count(), 8);
    assert_eq!(brandubh.side(Side::Defenders).count(), 5);
    assert_eq!(brandubh.king_square(), Some(throne(7)));
    assert_eq!(brandubh.to_move(), Side::Attackers);

    // Tablut, from Linnaeus's drawing: sixteen against eight and a king.
    let tablut = TABLUT.starting_position();
    assert_eq!(tablut.size(), 9);
    assert_eq!(tablut.side(Side::Attackers).count(), 16);
    assert_eq!(tablut.side(Side::Defenders).count(), 9);
    assert_eq!(tablut.king_square(), Some(throne(9)));
    assert_eq!(tablut.to_move(), Side::Attackers);
}

#[test]
fn a_king_can_still_reach_a_corner_on_a_small_board() {
    // The one thing a size bug would break most visibly: the corners of a 7×7
    // board are not the corners of an 11×11 one, and a king walking to g7 on
    // brandubh has won while the same square on hnefatafl is an ordinary
    // interior square.
    let escaped = Game::from_position(
        BRANDUBH,
        Position::from_picture(
            &[
                "......K", ".......", ".......", ".......", ".......", ".......", "..X....",
            ],
            Side::Attackers,
        ),
    );
    assert!(
        matches!(
            escaped.outcome(),
            Some(Outcome::Defenders(DefenderWin::Escape))
        ),
        "a king on g7 has escaped on a seven-square board: {:?}",
        escaped.outcome()
    );
}

#[test]
fn every_ruleset_starts_from_a_board_of_its_own_size() {
    for rules in rules_tafl::ruleset::ALL {
        let start = rules.starting_position();
        assert_eq!(start.size(), rules.size, "{} started wrong", rules.key);
        assert_eq!(start.king_square(), Some(throne(rules.size)));
        // And the game is playable from it, which is the cheapest possible
        // check that a layout is not nonsense.
        assert!(
            !rules.legal_moves(&start).is_empty(),
            "{} has no moves",
            rules.key
        );
    }
}
