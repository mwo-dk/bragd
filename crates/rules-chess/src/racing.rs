//! Racing Kings: get your king to the eighth rank, and no checks at all.
//!
//! The pieces move exactly as they do in chess and there are no pawns, so
//! nothing here touches move generation. What it does touch is *legality*, and
//! that is the whole variant: **a move that gives check is illegal**, in either
//! direction. You may not check your opponent any more than you may expose your
//! own king, which turns the board into a traffic problem rather than a fight.
//!
//! That one rule is why the position is a race and not a massacre. Without it
//! White simply checks Black's king backwards for as long as it takes.
//!
//! The endings live in [`crate::variant::RacingKings`]. Everything in this
//! module is the filter and the opening array.

use crate::chess_move::MoveList;
use crate::piece::{Color, Role};
use crate::position::Position;

/// The starting array.
///
/// Both armies stand on the first two ranks with no pawns at all: kings and
/// queens on the a and h files, then rooks, bishops and knights. Black is on
/// the left half of each rank and White on the right, which is what makes the
/// race symmetric — neither colour starts nearer the eighth rank than the
/// other.
///
/// No castling rights, because there is nothing to castle: a king that has
/// moved is not the issue, the rooks are simply not where castling needs them
/// and the rule would be a rule about a position that cannot arise.
pub const STARTING_POSITION: &str = "8/8/8/8/8/8/krbnNBRK/qrbnNBRQ w - - 0 1";

/// The legal moves: chess's, minus every move that gives check.
///
/// Filtering after generation rather than teaching the generator a new rule.
/// That is deliberate and it is the same judgement made for shatranj:
/// the generator is the most heavily tested code in the crate and a variant
/// branch inside it puts every other variant's correctness at risk to save a
/// loop over at most a few dozen moves.
///
/// The cost is one `make_move` per candidate. A position here has fewer than
/// forty moves and no pawn promotions to multiply them, so this is a rounding
/// error next to the search that will one day call it.
pub fn legal_moves(position: &Position) -> MoveList {
    let mut moves = MoveList::default();
    for mv in position.legal_moves() {
        // `is_check` asks about the side to move in the *resulting* position,
        // which after a move is the opponent — exactly the side that must not
        // be left in check.
        if !position.make_move(mv).is_check() {
            moves.push(mv);
        }
    }
    moves
}

/// Whether `color`'s king has finished the race.
///
/// The eighth rank is rank index 7 for both sides. Not flipped per colour:
/// unlike promotion, both kings are running to the *same* end of the board,
/// which is what makes the position a race rather than two separate ones.
pub fn has_arrived(position: &Position, color: Color) -> bool {
    position.king_square(color).rank() == 7
}

/// Whether either side still has a piece that could ever matter.
///
/// Used for the flag rule. There is no such thing as insufficient material in a
/// race — a lone king can still run — so this is always true and exists to say
/// so in one place rather than as a bare `true` in the variant.
pub fn can_still_race(position: &Position, color: Color) -> bool {
    // A king is never captured and never leaves the board, so this holds for
    // every position that can occur.
    !position.pieces(color, Role::King).is_empty()
}
