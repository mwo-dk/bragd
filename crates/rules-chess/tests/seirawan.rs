//! Seirawan chess.
//!
//! **There is no perft corpus for this variant**, so the gate is built rather
//! than downloaded — the same position `crates/rules-tafl` is in, and for a weaker
//! reason: Seirawan's rules are not contested, they are simply young enough
//! that nobody has published node counts.
//!
//! **That sentence was the whole of it and no counts were ever built.** Every
//! other variant with pieces of its own has a variant-aware `perft` in its own
//! test file — `tests/shatranj.rs` and `tests/chaturanga.rs` both do, each with
//! an honest note about what a self-recorded count is worth. This file had
//! behaviour tests and a claim. That is a gate weaker than it says it is, and
//! it was weakest for the variant with the most novelty in it: gating is a
//! whole move *category*, which is more than either of those two adds.
//!
//! So the counts are below now, on the same terms shatranj states: two that can
//! be derived by hand, the rest recorded from this engine so that a change to
//! move generation has to be made on purpose. They are evidence of stability,
//! not of correctness. The properties beside them are worth more, because they
//! are the thing a self-recorded count cannot be — a check that would fail if
//! the generator were consistently wrong.
//!
//! What is checked here is the part that is not chess. The movement is chess's
//! generator, which has perft behind it; the hawk and the elephant are unions
//! of tables that already have it. Everything below is about **gating**, which
//! is the whole variant and the only place a bug can hide.

use rules_chess::Game;
use rules_chess::chess_move::{CastlingNotation, Move};
use rules_chess::outcome::{DrawReason, Outcome, WinReason};
use rules_chess::piece::{Color, Role};
use rules_chess::position::Position;
use rules_chess::square::Square;
use rules_chess::variant::{Seirawan, Variant};

/// A square by name. The tests read as positions, not as indices.
fn sq(name: &str) -> Square {
    Square::parse(name).unwrap_or_else(|| panic!("{name} is not a square"))
}

/// The gating rights to open on a contrived board, named one square at a time.
///
/// `with_seirawan_hands` takes them rather than deriving them, because a board
/// cannot say which back-rank pieces have moved — so every test that starts
/// from a FEN has to say what it is claiming, and a test claiming a right on an
/// empty square is told rather than humoured.
fn gates(names: &[&str]) -> rules_chess::Bitboard {
    names.iter().fold(rules_chess::Bitboard::EMPTY, |set, name| {
        set | rules_chess::Bitboard::from_square(sq(name))
    })
}

fn start() -> Position {
    Seirawan.starting_position()
}

/// Perft over **Seirawan's** move list, not chess's.
///
/// Its own function for the reason `tests/shatranj.rs` has one:
/// `rules_chess::perft::perft` walks `Position::legal_moves`, which is chess's
/// generator, and counting that tree here would count a game nobody is playing
/// — the gating moves and the two extra promotions are exactly what is missing
/// from it, so a gate built on it would be blind to the whole variant.
fn perft(position: &Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = Seirawan.legal_moves(position);
    if depth == 1 {
        return moves.len() as u64;
    }
    moves
        .into_iter()
        .map(|mv| perft(&position.make_move(mv), depth - 1))
        .sum()
}

fn uci(position: &Position, text: &str) -> Move {
    Seirawan
        .legal_moves(position)
        .into_iter()
        .find(|mv| mv.to_uci(CastlingNotation::KingDestination) == text)
        .unwrap_or_else(|| panic!("{text} is not legal here"))
}

/// A move plus its gate, found by what it does rather than by its encoding.
fn gated(position: &Position, from_to: &str, role: Role) -> Move {
    Seirawan
        .legal_moves(position)
        .into_iter()
        .find(|mv| {
            mv.to_uci(CastlingNotation::KingDestination)
                .starts_with(from_to)
                && mv.gate() == Some(role)
        })
        .unwrap_or_else(|| panic!("no {from_to} gating a {role:?}"))
}

#[test]
fn the_opening_position_is_a_chessboard() {
    let position = start();
    // Every opening anybody knows still applies, which is the point of the
    // variant. Thirty-two pieces, none of them a hawk or an elephant.
    assert_eq!(position.occupied().count(), 32);
    assert!(position.by_role(Role::Hawk).is_empty());
    assert!(position.by_role(Role::Elephant).is_empty());
    // And both pieces are in both hands.
    for color in Color::ALL {
        assert!(position.in_hand(color, Role::Hawk));
        assert!(position.in_hand(color, Role::Elephant));
    }
}

#[test]
fn the_opening_offers_chess_moves_and_their_gates() {
    let moves: Vec<Move> = Seirawan.legal_moves(&start()).into_iter().collect();
    // Chess has twenty. Sixteen of them leave a back-rank square — the four
    // knight moves and, no, only the knights: the sixteen pawn moves leave the
    // *second* rank. So four moves gate, two ways each.
    let plain = moves.iter().filter(|mv| mv.gate().is_none()).count();
    let gates = moves.iter().filter(|mv| mv.gate().is_some()).count();
    assert_eq!(plain, 20, "the plain moves are chess's");
    assert_eq!(
        gates, 8,
        "four knight moves, a hawk or an elephant behind each"
    );
}

#[test]
fn gating_puts_the_piece_on_the_square_the_mover_left() {
    let position = start();
    let after = Seirawan.make_move(&position, gated(&position, "g1f3", Role::Hawk));

    assert_eq!(after.piece_at(sq("g1")).map(|p| p.role), Some(Role::Hawk));
    assert_eq!(after.piece_at(sq("f3")).map(|p| p.role), Some(Role::Knight));
    // And it is out of the hand.
    assert!(!after.in_hand(Color::White, Role::Hawk));
    assert!(after.in_hand(Color::White, Role::Elephant));
}

/// **The rule that makes the opening tense.** Moving a back-rank piece without
/// gating spends the square for ever. Getting this wrong would let a player
/// hold both pieces back and drop them in at leisure, which is a different and
/// much worse game.
#[test]
fn moving_without_gating_spends_the_square() {
    let position = start();
    let after = Seirawan.make_move(&position, uci(&position, "g1f3"));

    assert!(after.in_hand(Color::White, Role::Hawk), "still in hand");
    assert!(
        !after.gate_squares(Color::White).contains(sq("g1")),
        "g1 must be spent"
    );

    // Bring the knight home again: the square is still spent.
    let black = Seirawan.make_move(&after, uci(&after, "g8f6"));
    let back = Seirawan.make_move(&black, uci(&black, "f3g1"));
    assert!(
        !back.gate_squares(Color::White).contains(sq("g1")),
        "a right does not come back"
    );
    assert!(
        Seirawan
            .legal_moves(&back)
            .into_iter()
            .all(|mv| mv.from() != sq("g1") || mv.gate().is_none()),
        "and nothing may gate there"
    );
}

#[test]
fn a_piece_may_be_gated_only_once() {
    let mut position = start();
    position = Seirawan.make_move(&position, gated(&position, "g1f3", Role::Hawk));
    position = Seirawan.make_move(&position, uci(&position, "g8f6"));
    // The hawk is gone from the hand, so no move offers it again.
    assert!(
        Seirawan
            .legal_moves(&position)
            .into_iter()
            .all(|mv| mv.gate() != Some(Role::Hawk)),
        "the hawk cannot be gated twice"
    );
}

/// Castling vacates two squares and either may be gated, which is what makes it
/// the most valuable move in the opening.
#[test]
fn castling_offers_both_vacated_squares() {
    // Cleared for kingside castling, nothing else moved.
    let position = Position::from_fen("rnbqk2r/pppppppp/5n2/8/8/5N2/PPPPPPPP/RNBQK2R w KQkq - 4 4")
        .expect("legal")
        // f1 and g1 are empty here, so neither can carry a right — naming them
        // is refused rather than quietly ignored.
        .with_seirawan_hands(gates(&["e1", "h1"]));

    let castles: Vec<Move> = Seirawan
        .legal_moves(&position)
        .into_iter()
        .filter(|mv| {
            mv.to_uci(CastlingNotation::KingDestination)
                .starts_with("e1")
        })
        .filter(|mv| matches!(mv.kind(), rules_chess::chess_move::MoveKind::KingsideCastle))
        .collect();

    // One plain castle, and four gated: two pieces × two squares.
    assert_eq!(castles.len(), 5, "{castles:?}");
    assert_eq!(castles.iter().filter(|mv| mv.gate().is_none()).count(), 1);
    assert_eq!(castles.iter().filter(|mv| mv.gates_at_rook()).count(), 2);

    // The one that gates at the rook puts the piece on h1, not on e1.
    let at_rook = *castles
        .iter()
        .find(|mv| mv.gates_at_rook() && mv.gate() == Some(Role::Elephant))
        .expect("an elephant on the rook's square");
    let after = Seirawan.make_move(&position, at_rook);
    assert_eq!(
        after.piece_at(sq("h1")).map(|p| p.role),
        Some(Role::Elephant)
    );
    assert_eq!(after.piece_at(sq("e1")), None);
    // And castling spends *both* squares.
    assert!(
        after.gate_squares(Color::White).is_empty() || {
            let gates = after.gate_squares(Color::White);
            !gates.contains(sq("e1")) && !gates.contains(sq("h1"))
        }
    );
}

/// **A gate cannot rescue a move that was not legal without it.** A pinned
/// back-rank piece stays pinned even though the piece arriving behind it would
/// block the pin — that is the published rule, and it falls out of the
/// generator only extending moves that were already legal.
///
/// The test this replaces walked the *opening* position asserting that nobody
/// left their own king in check. White is never in check there and nothing is
/// pinned, so it could not fail and guarded nothing at all.
#[test]
fn a_pinned_piece_may_not_move_even_though_the_gate_would_block_the_pin() {
    // Black's rook on a1, White's knight on b1, White's king on e1, with c1 and
    // d1 empty: the knight is pinned along the rank. A hawk gated onto b1 would
    // block the pin perfectly, and that does not make the knight's move legal.
    let position = Position::from_fen("4k3/8/8/8/8/8/8/rN2K3 w - - 0 1")
        .expect("a legal position")
        .with_seirawan_hands(gates(&["b1"]));
    assert!(
        position.in_hand(Color::White, Role::Hawk),
        "a piece to gate"
    );
    assert!(
        position.gate_squares(Color::White).contains(sq("b1")),
        "and a live square to gate it onto"
    );
    assert!(
        Seirawan
            .legal_moves(&position)
            .into_iter()
            .all(|mv| mv.from() != sq("b1")),
        "the knight is pinned, gate or no gate"
    );
}

/// The opening array parses.
///
/// It had ten pawns on the seventh rank and nothing noticed, because Seirawan
/// is the only variant that builds its start from `Position::starting()` rather
/// than from its own const — so this string was never handed to the parser that
/// would have rejected it. Every other variant's is, with an `.expect`, so the
/// next person to follow the house pattern here would have got a panic.
#[test]
fn the_opening_array_is_a_position_that_parses() {
    let parsed = Position::from_fen(rules_chess::seirawan::STARTING_POSITION)
        .expect("the opening array is a legal position");
    assert_eq!(parsed.occupied().count(), 32);
    // And it is chess's, which is the whole claim the const makes.
    assert_eq!(parsed, Position::starting());
}

/// **Where a gated piece lands is not always in the move.**
///
/// Seirawan does not override the castling notation, so a castle is spelled by
/// the king's destination: `e1g1E` says nothing about h1. The seam in `bragd-games` has
/// to tell a front end which square the elephant arrives on, and this is the one
/// place that answer is written down.
#[test]
fn the_square_a_gate_lands_on_is_asked_of_the_position() {
    let position = start();
    let plain = uci(&position, "g1f3");
    let with_hawk = gated(&position, "g1f3", Role::Hawk);
    assert_eq!(
        position.gate_square(plain),
        None,
        "a plain move gates nothing"
    );
    assert_eq!(position.gate_square(with_hawk), Some(sq("g1")));

    let castling = Position::from_fen("rnbqk2r/pppppppp/5n2/8/8/5N2/PPPPPPPP/RNBQK2R w KQkq - 4 4")
        .expect("legal")
        .with_seirawan_hands(gates(&["e1", "h1"]));
    let at_rook = Seirawan
        .legal_moves(&castling)
        .into_iter()
        .find(|mv| mv.gates_at_rook() && mv.gate() == Some(Role::Elephant))
        .expect("an elephant on the rook's square");
    assert_eq!(
        at_rook.to_uci(CastlingNotation::KingDestination),
        "e1g1E",
        "and h1 appears nowhere in it"
    );
    assert_eq!(castling.gate_square(at_rook), Some(sq("h1")));
    // Which is where `make_move` actually puts it — the point of asking one
    // function rather than writing the derivation twice.
    assert_eq!(
        Seirawan
            .make_move(&castling, at_rook)
            .piece_at(sq("h1"))
            .map(|p| p.role),
        Some(Role::Elephant)
    );
}

/// The Black king stands on h6 and not on h8, which is not arbitrary: h8 is on
/// the hawk's own long diagonal, so that board is Black in check with White to
/// move and the FEN reader now refuses it. It read as legal for as long as
/// `attackers_to` did not know the piece this test is about.
#[test]
fn the_hawk_moves_as_a_bishop_or_a_knight() {
    let position = Position::from_fen("8/8/7k/8/3H4/8/8/7K w - - 0 1")
        .expect("a lone hawk is a legal position");
    let from = sq("d4");
    let destinations: Vec<String> = Seirawan
        .legal_moves(&position)
        .into_iter()
        .filter(|mv| mv.from() == from)
        .map(|mv| mv.to().to_string())
        .collect();

    // Eight knight jumps plus thirteen diagonal squares on an empty board.
    assert_eq!(destinations.len(), 8 + 13, "{destinations:?}");
    for square in ["e6", "c6", "b5", "b3", "c2", "e2", "f3", "f5"] {
        assert!(
            destinations.contains(&square.to_string()),
            "knight to {square}"
        );
    }
    for square in ["a1", "h8", "a7", "g1"] {
        assert!(
            destinations.contains(&square.to_string()),
            "bishop to {square}"
        );
    }
    // And not like a rook.
    assert!(!destinations.contains(&"d8".to_string()));
}

#[test]
fn the_elephant_moves_as_a_rook_or_a_knight() {
    let position = Position::from_fen("7k/8/8/8/3S4/8/8/7K w - - 0 1")
        .expect("a lone elephant is a legal position");
    let from = sq("d4");
    let destinations: Vec<String> = Seirawan
        .legal_moves(&position)
        .into_iter()
        .filter(|mv| mv.from() == from)
        .map(|mv| mv.to().to_string())
        .collect();

    assert_eq!(destinations.len(), 8 + 14, "{destinations:?}");
    for square in ["d8", "d1", "a4", "h4"] {
        assert!(
            destinations.contains(&square.to_string()),
            "rook to {square}"
        );
    }
    for square in ["e6", "b3"] {
        assert!(
            destinations.contains(&square.to_string()),
            "knight to {square}"
        );
    }
    assert!(!destinations.contains(&"h8".to_string()), "not diagonally");
}

/// **A hand is part of where the game is.** Two positions differing only in
/// what is left to gate are different positions, and repetition has to see
/// that — otherwise a player who gates could claim a repetition against a
/// position that no longer exists.
#[test]
fn the_hand_is_part_of_the_position_key() {
    let position = start();
    let plain = position.make_move(uci(&position, "g1f3"));
    let with_hawk = position.make_move(gated(&position, "g1f3", Role::Hawk));
    assert_ne!(plain.key(), with_hawk.key());

    // And so is a spent gating right, even when the board is identical.
    let a = start();
    let b = a.make_move(uci(&a, "g1f3"));
    let c = b.make_move(uci(&b, "g8f6"));
    let d = c.make_move(uci(&c, "f3g1"));
    let e = d.make_move(uci(&d, "f6g8"));
    // Same board as the start, two gating rights gone.
    assert_ne!(
        a.key(),
        e.key(),
        "spent rights make it a different position"
    );
}

/// A lone king with a piece still to gate is **not** a dead position: it can
/// produce mating material out of nothing, which nothing else in Bragd can
/// do. Calling it insufficient would end a game somebody was about to win.
#[test]
fn a_piece_in_hand_is_material() {
    let bare = Position::from_fen("7k/8/8/8/8/8/8/K7 w - - 0 1")
        .expect("legal")
        .with_seirawan_hands(gates(&["a1"]));
    assert!(!Seirawan.is_insufficient_material(&bare));
    assert!(Seirawan.can_possibly_mate(&bare, Color::White));

    // With no gating rights left, it is chess's answer again.
    let spent = Position::from_fen("7k/8/8/8/8/8/8/K7 w - - 0 1").expect("legal");
    assert!(Seirawan.is_insufficient_material(&spent));
}

#[test]
fn a_game_can_be_played_and_the_pieces_arrive() {
    let mut game = Game::start(Seirawan);
    for wanted in ["g1f3", "g8f6", "b1c3", "b8c6"] {
        let legal: Vec<Move> = game.legal_moves().into_iter().collect();
        // Prefer the gated version, so the pieces actually arrive — which is
        // what this test is for.
        let mv = legal
            .iter()
            .find(|mv| {
                // `starts_with`, because a gated move now spells its gate as a
                // fifth character — `g1f3h`. Written as `==` first, which found
                // only the plain moves and gated nothing at all.
                mv.to_uci(CastlingNotation::KingDestination)
                    .starts_with(wanted)
                    && mv.gate().is_some()
            })
            .or_else(|| {
                legal
                    .iter()
                    .find(|mv| mv.to_uci(CastlingNotation::KingDestination) == wanted)
            })
            .copied()
            .expect("a legal move");
        game.play(mv).expect("legal");
    }
    // Two gates each: every hand is empty and four new pieces are on the board.
    let position = game.position();
    assert_eq!(
        (position.by_role(Role::Hawk) | position.by_role(Role::Elephant)).count(),
        4,
        "four gated pieces"
    );
    assert!(game.outcome().is_none());
    assert_eq!(game.moves().len(), 4);
}

/// **Every legal move must have a distinct spelling**, or the authority cannot
/// tell which one a client meant. Seirawan is the first variant where three
/// different moves share a from and a to — plain, gating a hawk, gating an
/// elephant — and a castle can offer five.
#[test]
fn no_two_legal_moves_spell_the_same_thing() {
    let mut seen = std::collections::HashSet::new();
    let mut position = start();
    // Walk a few plies, gating where possible, so castling and both hands are
    // exercised rather than only the opening.
    for _ in 0..8 {
        seen.clear();
        for mv in Seirawan.legal_moves(&position) {
            let wire = mv.to_uci(CastlingNotation::KingDestination);
            assert!(seen.insert(wire.clone()), "two moves both spell {wire}");
        }
        let Some(next) = Seirawan
            .legal_moves(&position)
            .into_iter()
            .find(|mv| mv.gate().is_some())
            .or_else(|| Seirawan.legal_moves(&position).into_iter().next())
        else {
            break;
        };
        position = Seirawan.make_move(&position, next);
    }
}

/// **Two alphabets, on purpose.**
///
/// The wire spells the elephant `e`, which is what a Seirawan player writes and
/// is unambiguous *within a game* — there is no alfil in Seirawan. The FEN
/// spells it `S`, because FENs from every variant share one stored column and
/// `E` there is already shatranj's alfil, in games that must keep reading back
/// for ever.
///
/// The rule: notation is per-game and follows the game; storage is shared and
/// needs a globally unique letter.
#[test]
fn a_gate_is_spelled_by_its_piece_and_its_case() {
    let position = start();
    let hawk = gated(&position, "g1f3", Role::Hawk);
    let elephant = gated(&position, "g1f3", Role::Elephant);
    assert_eq!(hawk.to_uci(CastlingNotation::KingDestination), "g1f3h");
    assert_eq!(elephant.to_uci(CastlingNotation::KingDestination), "g1f3e");
    // And the same piece is `S` in a FEN. See the note above.
    let after = Seirawan.make_move(&position, elephant);
    // The knight is on f3 and an elephant, written `S`, stands on g1.
    assert!(
        after
            .to_fen()
            .starts_with("rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKBSR"),
        "{}",
        after.to_fen()
    );

    // Uppercase means the rook's square, which only a castle can offer.
    let castling = Position::from_fen("rnbqk2r/pppppppp/5n2/8/8/5N2/PPPPPPPP/RNBQK2R w KQkq - 4 4")
        .expect("legal")
        .with_seirawan_hands(gates(&["e1", "h1"]));
    let at_rook = Seirawan
        .legal_moves(&castling)
        .into_iter()
        .find(|mv| mv.gates_at_rook() && mv.gate() == Some(Role::Hawk))
        .expect("a hawk on the rook's square");
    assert_eq!(at_rook.to_uci(CastlingNotation::KingDestination), "e1g1H");
}

/// **A pawn promotes six ways here, not four**, and every published
/// description of the game says so: a queen, a rook, a bishop, a knight, a hawk
/// or an elephant. It offered four, which is chess's list, because Seirawan
/// takes its movement from chess's generator and promotion came along with it.
#[test]
fn a_pawn_may_promote_to_a_hawk_or_an_elephant() {
    // A pawn on b7 with a8 empty and a knight on b8 to capture: one quiet
    // promotion and one capturing, so twelve moves off this one pawn.
    let position = Position::from_fen("1n2k3/P7/8/8/8/8/8/4K3 w - - 0 1")
        .expect("legal")
        .with_seirawan_hands(gates(&["e1"]));

    let promotions: Vec<String> = Seirawan
        .legal_moves(&position)
        .into_iter()
        .filter(|mv| mv.from() == sq("a7"))
        .map(|mv| mv.to_uci(CastlingNotation::KingDestination))
        .collect();
    assert_eq!(
        promotions,
        [
            "a7a8q", "a7a8r", "a7a8b", "a7a8n", "a7a8h", "a7a8e", //
            "a7b8q", "a7b8r", "a7b8b", "a7b8n", "a7b8h", "a7b8e"
        ],
        "chess's four, then the two this game adds"
    );

    // And the piece that arrives is the one that was asked for.
    let hawk = uci(&position, "a7a8h");
    assert_eq!(
        Seirawan.make_move(&position, hawk).piece_at(sq("a8")),
        Some(rules_chess::piece::Piece::new(Color::White, Role::Hawk))
    );
}

/// **Not drawn from the hand.** A player who gated their hawk on move one may
/// still promote a pawn to a second one, exactly as a chess player with a queen
/// on the board may promote to a second queen. The alternative reading — that
/// the two pieces are a supply of one each — is a narrower game than the rules
/// describe, and nothing published attaches the proviso.
#[test]
fn promoting_to_a_hawk_does_not_need_one_in_hand() {
    // Both hands are empty: this position was parsed from a FEN and never given
    // any. The promotions are offered all the same.
    let spent = Position::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").expect("legal");
    assert!(!spent.in_hand(Color::White, Role::Hawk));
    let letters: Vec<char> = Seirawan
        .legal_moves(&spent)
        .into_iter()
        .filter(|mv| mv.from() == sq("a7"))
        .filter_map(|mv| mv.to_uci(CastlingNotation::KingDestination).chars().nth(4))
        .collect();
    assert_eq!(letters, ['q', 'r', 'b', 'n', 'h', 'e']);
}

/// **A hawk on the board is mating material, and it was not counted.**
///
/// `outcome` asks about insufficient material before anything else, and the
/// test it asked knows chess's six roles. King and hawk against a lone king
/// therefore read as nothing at all and the Game was declared a dead draw with
/// a piece on the board that mates a bare king comfortably — reachable by
/// gating on move one and trading everything else off, at which point the empty
/// hand cannot rescue the verdict either.
#[test]
fn a_gated_piece_still_on_the_board_is_material() {
    let hawk = Position::from_fen("7k/8/8/8/8/8/8/K6H w - - 0 1").expect("legal");
    assert!(Seirawan.can_possibly_mate(&hawk, Color::White));
    assert!(!Seirawan.is_insufficient_material(&hawk));
    assert_eq!(Seirawan.outcome(&hawk), None, "this game is not over");

    // On g1 rather than h1, because an elephant is a rook and h1 puts the Black
    // king in check with White to move — an impossible board, which this FEN
    // was until `attackers_to` learned the piece.
    let elephant = Position::from_fen("7k/8/8/8/8/8/8/K5S1 w - - 0 1").expect("legal");
    assert!(!Seirawan.is_insufficient_material(&elephant));

    // Two bare kings with nothing held and nowhere to gate is still a draw,
    // which is what says the clause above is about the pieces and not a blanket
    // "Seirawan is never dead".
    let nothing = Position::from_fen("7k/8/8/8/8/8/8/K7 w - - 0 1").expect("legal");
    assert!(Seirawan.is_insufficient_material(&nothing));
}

/// **The material claim above, actually demonstrated.** `can_possibly_mate`
/// says a lone compound is enough to mate a bare king, and `Game::flag` turns
/// that sentence into a stored result. So it is worth one board each rather
/// than an appeal to the literature: if either of these stops being mate, a
/// flag-fall is awarding a game nobody could have won.
#[test]
fn a_lone_compound_really_does_mate_a_bare_king() {
    // Hawk on c7 leaps to a8; the king on b6 takes a7 and b7 away; b8 is the
    // hawk's own diagonal.
    let by_hawk = Position::from_fen("k7/2H5/1K6/8/8/8/8/8 b - - 0 1").expect("legal");
    assert_eq!(
        Seirawan.outcome(&by_hawk),
        Some(Outcome::Win {
            winner: Color::White,
            reason: WinReason::Checkmate,
        })
    );

    // The elephant mates along the back rank exactly as a rook does.
    let by_elephant = Position::from_fen("S6k/8/7K/8/8/8/8/8 b - - 0 1").expect("legal");
    assert_eq!(
        Seirawan.outcome(&by_elephant),
        Some(Outcome::Win {
            winner: Color::White,
            reason: WinReason::Checkmate,
        })
    );
}

/// **The compounds attack, and for a while they only moved.**
///
/// `attackers_to` is one definition of "attacks" shared by every variant, and
/// the hawk and the elephant were missing from it while `movegen` walked them
/// both. A piece that moves and never attacks breaks every rule downstream at
/// once: it gives no check, so it cannot mate, so the enemy king is offered
/// squares it covers, so this generator offers the capture of a king — and
/// `to_san` then plays that move and asks the resulting board for a king that
/// is not there, which aborts a release build rather than merely losing a
/// game. The promotions made it ordinary: before them a compound needed a
/// gating right to reach the board at all.
#[test]
fn a_hawk_and_an_elephant_give_check() {
    // Hawk on f6, by the knight half of it.
    let hawk = Position::from_fen("4k3/8/5H2/8/8/8/8/4K3 b - - 0 1").expect("legal");
    assert!(hawk.is_check_against(Color::Black), "the hawk leaps to e8");

    // And the king is only offered the squares the hawk does not cover. Every
    // one of d7, e7 and d8 was on this list while the attack went unseen.
    let escapes = uci_set(&Seirawan.legal_moves(&hawk));
    assert_eq!(
        escapes,
        ["e8f7", "e8f8"].map(String::from).to_vec(),
        "no flight square the hawk covers"
    );

    // Elephant on f6, by the rook half of it.
    let elephant = Position::from_fen("4k3/8/5S2/8/8/8/8/4K3 b - - 0 1").expect("legal");
    assert!(
        elephant.is_check_against(Color::Black),
        "the elephant is a rook on the f-file and a knight to e8"
    );
}

/// **A position where a compound bears on the enemy king is not a position at
/// all**, and the FEN reader is where that is caught. It parsed while
/// `attackers_to` was blind, and every rule after it inherited a board with the
/// side not to move already in check.
#[test]
fn a_board_where_a_compound_attacks_the_idle_king_is_impossible() {
    assert!(
        Position::from_fen("3k4/8/5H2/8/8/8/8/4K3 w - - 0 1").is_err(),
        "a hawk on f6 attacks d8, so Black cannot be waiting"
    );
    assert!(
        Position::from_fen("5k2/8/5S2/8/8/8/8/4K3 w - - 0 1").is_err(),
        "an elephant on f6 attacks f8 along the file"
    );
}

/// **No move in this variant may ever land on the enemy king.** The property
/// rather than the position: a generator that offers one has stopped agreeing
/// with the rule that a king in check must answer it, and a Library would
/// keep the board it produced for ever.
#[test]
fn no_generated_move_captures_a_king() {
    fn walk(position: &Position, depth: u32) {
        let enemy = position.king_square(position.side_to_move().opponent());
        for mv in Seirawan.legal_moves(position) {
            assert_ne!(
                mv.to(),
                enemy,
                "{} captures a king in {}",
                mv.to_uci(CastlingNotation::KingTakesRook),
                position.to_fen()
            );
            if depth > 0 {
                walk(&Seirawan.make_move(position, mv), depth - 1);
            }
        }
    }
    walk(&start(), 3);
    walk(&crowded(), 2);
}

/// **The two boards that measure what this generator can emit.**
///
/// Recorded because the comment on `seirawan::legal_moves` used to reason about
/// the `MoveList` bound with chess's 218-move record, which is the wrong
/// quantity — what this emits is chess's count *plus* two per move leaving a
/// gated square *plus* two per promoting pawn step, and none of that is bounded
/// by chess's own maximum. The second board overflowed a 256-slot list, and
/// `MoveList::push` asserts in release too, so it aborted the app.
///
/// These are evidence of stability on the same terms as the perft counts above:
/// a change to move generation that moves them has to be made on purpose.
#[test]
fn the_widest_move_lists_this_generator_can_emit() {
    // Eight pawns on the seventh against a full enemy back rank, every White
    // home square still gated. The shape the old comment had in mind.
    let promoting = Position::from_fen("rnbq1bnr/PPPPPPPP/8/8/4k3/8/8/RNBQKBNR w KQ - 0 1")
        .expect("legal")
        .with_seirawan_hands(white_home());
    assert_eq!(promoting.legal_moves().len(), 97, "chess's own count");
    assert_eq!(Seirawan.legal_moves(&promoting).len(), 213);

    // Seven promoted queens, an untouched back rank and nothing gated yet. The
    // one that overflowed: 173 chess moves become 267.
    let queens = Position::from_fen("2Q4Q/5Q2/1Q3Q2/8/1Q2Q3/6k1/1Q6/RNBQKBNR w KQ - 0 1")
        .expect("legal")
        .with_seirawan_hands(white_home());
    assert_eq!(queens.legal_moves().len(), 173, "chess's own count");
    assert_eq!(Seirawan.legal_moves(&queens).len(), 267);
}

/// Every White home square, for the two boards above. Black's rank 8 cannot be
/// included on either of them — it holds White pieces on one and gaps on the
/// other, and `with_seirawan_hands` refuses both.
fn white_home() -> rules_chess::Bitboard {
    gates(&["a1", "b1", "c1", "d1", "e1", "f1", "g1", "h1"])
}

/// The move list as UCI strings, for tests that assert on the whole of it.
fn uci_set(moves: &rules_chess::MoveList) -> Vec<String> {
    moves
        .as_slice()
        .iter()
        .map(|mv| mv.to_uci(CastlingNotation::KingTakesRook))
        .collect()
}

/// **A gating right cannot be read off a board**, so the function that opens
/// them refuses to guess. This is a Library hazard the parameter exists for:
/// a rook that went out and came home stands on a1 looking exactly like one
/// that never left, and the old version handed back four restored rights and a
/// hawk that could appear on a square nobody could gate.
#[test]
fn opening_a_gate_on_a_square_that_cannot_carry_one_is_refused() {
    let midgame =
        Position::from_fen("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1").expect("legal");

    // b1 is empty — the knight that started there has gone.
    assert!(
        std::panic::catch_unwind(|| midgame.with_seirawan_hands(gates(&["b1"]))).is_err(),
        "an empty square cannot carry a gating right"
    );
    // The squares whose piece never left are accepted.
    assert!(
        std::panic::catch_unwind(|| midgame.with_seirawan_hands(gates(&["a1", "e1", "h1"])))
            .is_ok(),
        "the squares that are still occupied are fine"
    );
    // **There is no colour to get wrong, which is why nothing here tests for
    // it.** `gates` is one colourless bitboard and the rank decides the owner —
    // `theirs = gates & home_rank(color)` — so a bit on c8 *is* a Black right by
    // construction. It is refused here for the same reason b1 is, that the
    // square is empty, and not for whose it is.
    assert!(
        std::panic::catch_unwind(|| midgame.with_seirawan_hands(gates(&["c8"]))).is_err(),
        "an empty square is refused on either home rank"
    );
    // And nothing off a home rank, whoever owns it.
    assert!(
        std::panic::catch_unwind(|| midgame.with_seirawan_hands(gates(&["a2"]))).is_err(),
        "only a home rank ever carries a right"
    );
}

#[test]
fn the_variant_is_reachable_by_key() {
    use rules_chess::Rules;
    assert_eq!(Rules::from_key("seirawan"), Some(Rules::Seirawan));
    assert_eq!(Rules::Seirawan.name(), Seirawan.name());
    assert_eq!(
        Rules::Seirawan.starting_position().to_fen(),
        Seirawan.starting_position().to_fen()
    );
}

#[test]
fn checkmate_is_still_checkmate() {
    // Fool's mate, reached in a Seirawan game where nobody gated.
    let position =
        Position::from_fen("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3")
            .expect("legal")
            // Fifteen squares, not sixteen: the queen left d8 to give the mate,
            // which spends that right and empties that square. Handing over all
            // sixteen is refused, and that refusal is the whole point of the
            // parameter — a board cannot be asked which pieces have moved.
            .with_seirawan_hands(gates(&[
                "a1", "b1", "c1", "d1", "e1", "f1", "g1", "h1", //
                "a8", "b8", "c8", "e8", "f8", "g8", "h8",
            ]));
    assert_eq!(
        Seirawan.outcome(&position),
        Some(Outcome::Win {
            winner: Color::Black,
            reason: WinReason::Checkmate
        })
    );
    let _ = DrawReason::Stalemate;
}
/// The two counts that can be argued rather than measured.
#[test]
fn the_first_two_plies_are_derivable_by_hand() {
    // Twenty-eight: chess's twenty, plus the four knight moves offered again
    // with a hawk behind them and again with an elephant. Nothing else on the
    // back rank can move at all, and a pawn leaves the *second* rank, which
    // never carried a right.
    assert_eq!(perft(&start(), 1), 28);

    // 784 = 28 x 28, and the square is the argument. No first move can reach
    // anything of Black's — every square White touches is on ranks 1 to 4 and
    // every square Black's pieces move to is on ranks 5 to 8 — and gating
    // spends a White right, never a Black one. So Black's reply count cannot
    // depend on what White chose, gate or no gate.
    assert_eq!(perft(&start(), 2), 784);
}

/// A promoting pawn is worth **twelve moves, not eight**, and that is the whole
/// of the rule this variant adds to chess besides gating.
///
/// Hand-derivable, which is why it is here rather than among the recorded
/// counts: the pawn on a7 promotes onto a8 or by capturing the knight on b8,
/// six ways each; the king on e1 has five squares, and e1 still carries a right,
/// so each of them is offered three ways. 12 + 15 = 27.
#[test]
fn a_promotion_is_six_moves_and_the_count_says_so() {
    let position = Position::from_fen("1n2k3/P7/8/8/8/8/8/4K3 w - - 0 1")
        .expect("a legal position")
        .with_seirawan_hands(gates(&["e1"]));
    assert_eq!(perft(&position, 1), 27);
}

/// Deeper counts, recorded from this engine.
///
/// **These prove stability and nothing else.** There is no second
/// implementation and no published table to agree with, so a shared mistake
/// would be recorded here as confidently as a correct count. Their job is to
/// make a change in move generation impossible to make by accident — which is
/// the job nothing was doing for this variant until they were written.
#[test]
fn deeper_counts_do_not_move_without_someone_meaning_it() {
    assert_eq!(perft(&start(), 3), 24_830);

    // A position with every category on offer at once: a castle that can gate
    // either of two squares, both pieces still in hand, and a pawn one step
    // from six promotions on each of three squares. The opening tree reaches
    // none of that within the depths above.
    assert_eq!(perft(&crowded(), 1), 57);
    assert_eq!(perft(&crowded(), 2), 1_075);
    assert_eq!(perft(&crowded(), 3), 53_848);
}

/// **782_599, and it was recorded as 782_611 before `attackers_to` knew the
/// compounds.** Worth keeping the derivation, because a moved perft count is
/// the one thing this project treats as never accidental.
///
/// The whole difference is twelve moves, in four positions, all of them the
/// same mistake. Both lines gate a hawk and then step it out to the edge —
/// `b1c3h … b1a3` and `g1f3h … g1h3` — where its bishop half rakes a long
/// diagonal at the enemy king: a3 bears on e7, h3 on d7. Black had just opened
/// that square with a pawn, and the old generator let the king walk onto it.
///
/// Four king steps, twelve moves, because e8 still carries Black's gating
/// right: `e8e7` is offered plain and with either piece in hand behind it. That
/// factor of three is the variant being itself, and it is why an illegal king
/// step costs three times what it would in chess.
///
/// Nothing above this depth moves — `perft(&start(), 3)` is 24_830 either way —
/// because a gated hawk needs a move of its own to reach a square that bears on
/// anything.
#[test]
#[ignore = "seconds, not milliseconds — run with --ignored"]
fn deeper_still() {
    assert_eq!(perft(&start(), 4), 782_599);
}

/// White may castle and gate on either vacated square, and the b7 pawn reaches
/// the last rank three ways: pushing to the empty b8, and capturing on a8 or
/// c8. Six pieces each, so eighteen promotions off one pawn.
///
/// f1 and g1 are empty, so neither is named: `with_seirawan_hands` refuses a
/// right on a square that cannot carry one.
fn crowded() -> Position {
    Position::from_fen("r1nqkbnr/pPpppppp/8/8/8/5N2/PPPPPPPP/RNBQK2R w KQkq - 0 1")
        .expect("a legal position")
        .with_seirawan_hands(gates(&["a1", "b1", "c1", "d1", "e1", "h1"]))
}

/// Walks the whole tree to a shallow depth, checking what must be true of
/// *every* position rather than of a count.
///
/// This is the part that could catch a generator wrong the same way every time,
/// which a self-recorded count cannot be. Every assertion below is about the
/// two things this variant adds and chess's perft therefore never exercised:
/// where a gated piece may appear, and what a pawn may become.
#[test]
fn every_position_in_the_tree_obeys_the_rules_gating_adds() {
    /// `ceiling` is how many of each compound a side may have on the board:
    /// one from the hand, and one more for every promotion it has played on the
    /// way here. Carried down the walk rather than fixed at one, because the
    /// crowded position promotes on its first ply — and a fixed one would have
    /// been a true-looking assertion that the promotion rule then falsified,
    /// which is how a test ends up being deleted rather than understood.
    fn walk(position: &Position, depth: u32, ceiling: [u32; 2]) {
        for color in Color::ALL {
            assert_eq!(
                position.pieces(color, Role::King).count(),
                1,
                "the wrong number of kings in {}",
                position.to_fen()
            );
            // **A hand holds one hawk and one elephant.** One more of either on
            // the board than the hand and the promotions can account for means
            // a gate that spent nothing, which is the bug that turns the whole
            // opening into a different game.
            for role in [Role::Hawk, Role::Elephant] {
                assert!(
                    position.pieces(color, role).count() <= ceiling[color.index()],
                    "more {role:?}s than were held or promoted in {}",
                    position.to_fen()
                );
            }
        }

        if depth == 0 {
            return;
        }
        let gates_before = position.gates();
        for mv in Seirawan.legal_moves(position) {
            let us = position.side_to_move();
            let next = position.make_move(mv);
            assert!(
                !next.is_check_against(us),
                "{mv} leaves the mover in check in {}",
                position.to_fen()
            );

            match mv.gate() {
                None => {}
                Some(role) => {
                    // **A gate only ever lands where a right was**, which is the
                    // rule the whole variant turns on and the one a generator
                    // that reasoned about squares rather than rights would break
                    // silently.
                    let square = position
                        .gate_square(mv)
                        .expect("a gating move names its square");
                    assert!(
                        gates_before.contains(square),
                        "{mv} gates onto {square}, which carries no right, in {}",
                        position.to_fen()
                    );
                    assert!(
                        position.in_hand(us, role),
                        "{mv} gates a {role:?} that is not held in {}",
                        position.to_fen()
                    );
                    assert_eq!(
                        next.piece_at(square).map(|piece| piece.role),
                        Some(role),
                        "{mv} did not put the piece where it said"
                    );
                    assert!(!next.in_hand(us, role), "{mv} gated without spending");
                }
            }

            // **The right is spent whether it was used or not.** Getting this
            // wrong is the difference between the published game and one where
            // both pieces can be held back indefinitely.
            assert!(
                !next.gates().contains(mv.from()),
                "{mv} left a right standing on the square it vacated in {}",
                position.to_fen()
            );

            let mut next_ceiling = ceiling;
            if matches!(mv.promotion_role(), Some(Role::Hawk) | Some(Role::Elephant)) {
                next_ceiling[us.index()] += 1;
            }
            walk(&next, depth - 1, next_ceiling);
        }
    }

    walk(&start(), 3, [1, 1]);
    walk(&crowded(), 2, [1, 1]);
}

/// **Six pieces, all six distinct, and every one of them reachable.** Chess
/// offers four; the count test above pins the number and this pins the pieces,
/// because a generator that emitted the queen twice would satisfy the count.
#[test]
fn a_promoting_pawn_is_offered_all_six_pieces_on_every_square_it_can_reach() {
    let position = crowded();
    for destination in ["a8", "b8", "c8"] {
        let mut roles: Vec<Role> = Seirawan
            .legal_moves(&position)
            .into_iter()
            .filter(|mv| mv.from() == sq("b7") && mv.to() == sq(destination))
            .map(|mv| {
                mv.promotion_role()
                    .expect("reaching the last rank promotes")
            })
            .collect();
        let offered = roles.len();
        roles.sort();
        roles.dedup();
        assert_eq!(offered, 6, "{destination} offered {offered}");
        assert_eq!(roles.len(), 6, "{destination} offered a piece twice");
        assert!(roles.contains(&Role::Hawk) && roles.contains(&Role::Elephant));
    }
}
