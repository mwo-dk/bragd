//! Seirawan chess: two extra pieces, held back and let in as the board opens.
//!
//! Yasser Seirawan and Bruce Harper published it in 2007, and its idea is
//! unusually good: the two new pieces are not on the board at the start, so the
//! opening position is a chessboard and every opening you know still applies —
//! right up to the moment somebody develops a piece and something arrives
//! behind it.
//!
//! - The **hawk** moves as a bishop or a knight.
//! - The **elephant** moves as a rook or a knight.
//!
//! Both start *in hand*. When a piece leaves its original square on the back
//! rank, its owner may place one of their held pieces on the square it left, as
//! part of the same move. That is **gating**, and it is the whole game:
//!
//! - It is optional. Moving without gating is legal, and it **spends the
//!   right** — that square can never be gated again. Holding both pieces back
//!   costs you the squares to put them on.
//! - Castling vacates two squares, the king's and the rook's, and either may be
//!   gated. That makes castling the most valuable single move in the opening.
//! - A gated piece is on the board immediately: it can give check, block one,
//!   or be captured on the next move.
//! - If every back-rank square is spent, the pieces stay in hand for ever.
//!
//! # What lives here, and what does not
//!
//! The *state* is on [`Position`] — the hand and the surviving rights — because
//! two positions differing only in what is left to gate are different positions
//! and repetition has to see it. The *gate itself* rides on [`Move`], because
//! it happens as part of a move and never on its own.
//!
//! What lives here is the generator: chess's moves, with the gating variants
//! layered on. Nothing in this module reimplements a chess rule, which is the
//! same discipline `shatranj` and `chaturanga` follow — a variant that forked
//! move generation would be a second engine to keep correct.

use crate::chess_move::{Move, MoveKind, MoveList};
use crate::piece::{Color, Piece, Role};
use crate::position::Position;

/// The opening array: chess's, exactly.
///
/// Deliberately not a FEN with the hands in it. Seirawan's own FEN writes the
/// pieces in hand in brackets, and this engine does not parse that — the hands
/// are put there by [`crate::variant::Seirawan::starting_position`] calling
/// `with_seirawan_hands`, because a hand is a property of the *variant* and a
/// FEN cannot say which variant it belongs to.
///
/// It had **ten pawns on the seventh rank** until this line was read out loud,
/// and nothing caught it: Seirawan is the only variant that builds its opening
/// from `Position::starting()` rather than parsing its own const, so this one
/// was never handed to a parser that would have said `WrongRankWidth`. Every
/// other variant does parse its own with an `.expect`, so the next person to
/// follow the house pattern here would have got a panic. The test below now
/// parses it.
pub const STARTING_POSITION: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// What may be gated, in the order they are offered.
pub const IN_HAND: [Role; 2] = [Role::Hawk, Role::Elephant];

/// What a pawn may become here that it may not become in chess, in the order
/// they are offered.
///
/// **Not conditional on the hand.** A player who has already gated their hawk
/// may still promote a pawn to a second one, exactly as a chess player with a
/// queen on the board may promote to a second queen. Every published
/// description of the game says a pawn promotes to a queen, rook, bishop,
/// knight, hawk or elephant and none of them attaches a proviso; Fairy-Stockfish
/// lists all six in `promotionPieceTypes` for the variant with no reference to
/// what is held. The alternative reading — that promotion draws from the hand —
/// would make the two pieces a supply of one each for the whole game, which is
/// a different and much narrower game than the one the rules describe.
pub const EXTRA_PROMOTIONS: [Role; 2] = [Role::Hawk, Role::Elephant];

/// Every legal move: chess's, plus the gates, plus the two promotions chess
/// does not have.
///
/// For each chess move that vacates a square with a live gating right, this
/// emits the plain move *and* one move per piece still in hand. They are
/// separate moves rather than a flag on one, because they are separate
/// decisions with different consequences and a player has to make the choice.
///
/// The move list can therefore be about three times chess's in the opening —
/// twenty moves becomes forty-four at the start, since sixteen of the twenty
/// leave a back-rank square.
///
/// **The bound is not chess's 218, and reasoning as though it were is how this
/// overflowed.** An earlier version of this comment argued the list was safe
/// because a position with many promotions is "nowhere near the 218-move chess
/// maximum". That compares the wrong quantity. What this emits is
///
/// > chess's count + 2 x (moves leaving a gated square) + 2 x (promoting pawn
/// > steps)
///
/// and neither addend is bounded by chess's record — they are largest on boards
/// where the back rank has never moved, which is exactly where chess's own
/// count is *not* at its record. The two multiply out rather than trading off.
///
/// Two measured worst cases, both recorded in `tests/seirawan.rs` so that a
/// change to move generation which moves them has to be made on purpose:
///
/// - `rnbq1bnr/PPPPPPPP/8/8/4k3/8/8/RNBQKBNR w KQ - 0 1` with all eight of
///   White's home squares still gated: 97 chess moves become **213**. This is
///   the shape the old comment had in mind, and it is a fifth of the headroom
///   that comment implied, not "nowhere near".
/// - `2Q4Q/5Q2/1Q3Q2/8/1Q2Q3/6k1/1Q6/RNBQKBNR w KQ - 0 1`, same rights: 173
///   chess moves become **267**, which overflowed the 256-slot `MoveList` and
///   aborted a release build — a Game in the caller that could never
///   be advanced again. It needs White to promote seven pawns while never
///   moving a back-rank piece and never gating, so it is legal and essentially
///   never played; the capacity is now 512, which the note there shows covers
///   even the sum of maxima that cannot co-occur.
pub fn legal_moves(position: &Position) -> MoveList {
    let plain = position.legal_moves();

    let us = position.side_to_move();
    let held: Vec<Role> = IN_HAND
        .into_iter()
        .filter(|&role| position.in_hand(us, role))
        .collect();
    // **Asked of the move list rather than of the board**, because it is the
    // exact condition: a pawn on the seventh rank with nowhere to go promotes
    // nothing. Cheap either way — this walks a list that has already been
    // generated, where the board version needs a rank mask and an argument
    // about which rank belongs to which colour.
    let promotes = (&plain).into_iter().any(|mv| mv.kind().is_promotion());
    if held.is_empty() && !promotes {
        return plain;
    }

    let gates = position.gates();
    let mut moves = MoveList::default();

    for mv in plain {
        moves.push(mv);
        push_extra_promotions(&mut moves, mv);

        // Castling vacates two squares; every other move vacates one.
        let mut squares = [Some(mv.from()), None];
        if matches!(
            mv.kind(),
            MoveKind::KingsideCastle | MoveKind::QueensideCastle
        ) {
            let side = match mv.kind() {
                MoveKind::KingsideCastle => crate::castling::Side::Kingside,
                _ => crate::castling::Side::Queenside,
            };
            squares[1] = position.castling_rights().rook_origin(us, side);
        }

        for (index, square) in squares.into_iter().enumerate() {
            let Some(square) = square else { continue };
            if !gates.contains(square) {
                continue;
            }
            for &role in &held {
                let gated = if index == 1 {
                    mv.with_gate_at_rook(role)
                } else {
                    mv.with_gate(role)
                };
                // **A gate cannot rescue an illegal move, and this is where
                // that is enforced by construction rather than by testing for
                // it.** The moves being extended come from
                // `position.legal_moves()`, so each is already legal without
                // the gated piece — which is the published rule: a pinned
                // knight stays pinned even though a hawk arriving behind it
                // would block the pin, and a move that does not answer a check
                // is not made legal by what comes in behind it.
                //
                // The test below can therefore never fail, and is kept anyway:
                // adding a friendly piece to a vacated square can only block an
                // enemy ray and never open one, so it is a statement of that
                // fact rather than a filter that removes anything. If a future
                // refactor ever feeds this pseudo-legal moves, this is what
                // stops the wrong ones reaching the board.
                if !position.make_move(gated).is_check_against(us) {
                    moves.push(gated);
                }
            }
        }
    }

    moves
}

/// The hawk and the elephant twins of a promotion, hung off the knight one.
///
/// **Off exactly one of the four, and that is the whole subtlety**: chess's
/// generator writes four moves for one promoting pawn step, and extending each
/// of them would emit the hawk and the elephant four times over. Any of the
/// four would do to fire once; the knight is chosen because chess offers it
/// last, so the six arrive queen, rook, bishop, knight, hawk, elephant — the
/// order a player expects to be offered them in. If chess ever reorders its
/// four the only thing that changes is the order of the offer.
///
/// The twins are legal by construction, for the same reason a gate cannot
/// rescue an illegal move and the reverse of it: legality asks whether *our*
/// king is attacked afterwards, the enemy's attacks depend on occupancy alone,
/// and all six promotions leave the identical occupancy. There is nothing here
/// to filter.
fn push_extra_promotions(moves: &mut MoveList, mv: Move) {
    if mv.promotion_role() != Some(Role::Knight) {
        return;
    }
    for role in EXTRA_PROMOTIONS {
        moves.push(Move::new(
            mv.from(),
            mv.to(),
            MoveKind::promotion_to(role, mv.is_capture()),
        ));
    }
}

/// Whether `color` has anything left to gate and anywhere to put it.
///
/// Used by the endings: a side with a piece in hand and no square left for it
/// is materially down for the rest of the game, which matters for the
/// insufficient-material test.
pub fn can_still_gate(position: &Position, color: Color) -> bool {
    IN_HAND
        .into_iter()
        .any(|role| position.in_hand(color, role))
        && !position.gate_squares(color).is_empty()
}

/// The pieces `color` still holds, for display.
pub fn hand(position: &Position, color: Color) -> Vec<Piece> {
    IN_HAND
        .into_iter()
        .filter(|&role| position.in_hand(color, role))
        .map(|role| Piece::new(color, role))
        .collect()
}
