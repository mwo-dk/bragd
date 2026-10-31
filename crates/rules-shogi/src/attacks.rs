//! Where each piece can reach.
//!
//! Two kinds of piece, and the split is the whole module. **Steppers** — pawn,
//! knight, silver, gold, king and the four promoted pieces that move as gold —
//! reach a fixed set of squares and are tabled at compile time. **Sliders** —
//! lance, bishop, rook, and the horse and dragon built on those two — run until
//! something stops them, so they are walked.
//!
//! Everything here is **side-dependent**, which is the difference from a chess
//! attack table. In chess only the pawn cares which way it is facing; in shogi
//! the pawn, the lance, the knight, the silver, the gold and all four
//! gold-moving promoted pieces do. Getting one of them mirrored produces a game
//! that looks right and is not, so the tables are generated from one direction
//! list per role and the mirroring happens in exactly one place.

use crate::board::{Bitboard, SQUARES, Square};
use crate::piece::{Role, Side};

/// The squares a stepper reaches, per side and per square.
type StepTable = [[Bitboard; SQUARES]; 2];

/// Builds a table from a direction list written **for Black** — up the board.
/// White's is the same list with the rank offsets negated, which is the one
/// place the mirroring happens.
const fn steps(deltas: &[(i16, i16)]) -> StepTable {
    let mut table = [[Bitboard::EMPTY; SQUARES]; 2];
    let mut side = 0;
    while side < 2 {
        let sign: i16 = if side == 0 { 1 } else { -1 };
        let mut index = 0;
        while index < SQUARES {
            let from = Square::at(index as u8);
            let mut mask = 0u128;
            let mut at = 0;
            while at < deltas.len() {
                let (df, dr) = deltas[at];
                if let Some(to) = from.offset(df, dr * sign) {
                    mask |= 1u128 << to.index();
                }
                at += 1;
            }
            table[side][index] = Bitboard(mask);
            index += 1;
        }
        side += 1;
    }
    table
}

/// Straight ahead one square.
const PAWN: StepTable = steps(&[(0, 1)]);
/// Two forward and one to the side, and it jumps.
const KNIGHT: StepTable = steps(&[(-1, 2), (1, 2)]);
/// Forward, and the four diagonals — but **not** sideways or straight back.
const SILVER: StepTable = steps(&[(0, 1), (-1, 1), (1, 1), (-1, -1), (1, -1)]);
/// Forward, the two forward diagonals, sideways and straight back. Not the
/// back diagonals.
const GOLD: StepTable = steps(&[(0, 1), (-1, 1), (1, 1), (-1, 0), (1, 0), (0, -1)]);
/// Every neighbour.
const KING: StepTable = steps(&[
    (0, 1),
    (0, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (1, 1),
    (-1, -1),
    (1, -1),
]);

const ROOK_DIRECTIONS: [(i16, i16); 4] = [(0, 1), (0, -1), (-1, 0), (1, 0)];
const BISHOP_DIRECTIONS: [(i16, i16); 4] = [(-1, 1), (1, 1), (-1, -1), (1, -1)];

/// Every square a slider reaches from `from`, stopping **on** the first
/// occupied square in each direction — that square is included, because a
/// slider may capture what blocks it. The caller removes its own pieces.
fn slide(from: Square, occupied: Bitboard, directions: &[(i16, i16)]) -> Bitboard {
    let mut reachable = Bitboard::EMPTY;
    for &(df, dr) in directions {
        let mut at = from;
        while let Some(next) = at.offset(df, dr) {
            reachable = reachable.with(next);
            if occupied.contains(next) {
                break;
            }
            at = next;
        }
    }
    reachable
}

/// Where a piece of `role` on `from` could move, ignoring whose pieces are in
/// the way — the caller masks off its own.
///
/// `occupied` matters only to the sliders, and is taken for every role so no
/// call site has to know which is which. That is deliberate: a call site that
/// knew would eventually be wrong about the horse.
pub fn reachable(side: Side, role: Role, from: Square, occupied: Bitboard) -> Bitboard {
    let s = side.index();
    let i = from.index() as usize;
    match role {
        Role::Pawn => PAWN[s][i],
        Role::Knight => KNIGHT[s][i],
        Role::Silver => SILVER[s][i],
        // The four that move exactly as a gold. Spelled out rather than folded
        // into one arm with a helper, because "these four move as gold" is the
        // fact a reader needs and a helper called `gold_like` would hide it.
        Role::Gold
        | Role::PromotedPawn
        | Role::PromotedLance
        | Role::PromotedKnight
        | Role::PromotedSilver => GOLD[s][i],
        Role::King => KING[s][i],
        // A lance is a rook that only goes forward.
        Role::Lance => slide(from, occupied, &[(0, side.forward())]),
        Role::Bishop => slide(from, occupied, &BISHOP_DIRECTIONS),
        Role::Rook => slide(from, occupied, &ROOK_DIRECTIONS),
        // The horse and the dragon are each a slider plus the king's step in
        // the *other* four directions. Adding the whole king step would be the
        // same set — a bishop already covers its diagonals — and saying it this
        // way makes the piece's actual description true.
        Role::Horse => slide(from, occupied, &BISHOP_DIRECTIONS) | KING[s][i],
        Role::Dragon => slide(from, occupied, &ROOK_DIRECTIONS) | KING[s][i],
    }
}

/// Whether a piece of this role could ever leave `to`, given its side.
///
/// A pawn or lance dropped on the far rank, or a knight on either of the far
/// two, can never move again. Shogi forbids putting one there and forces
/// promotion when one would move there.
pub fn would_be_stuck(side: Side, role: Role, to: Square) -> bool {
    role.is_stuck_on(side, to.rank())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{FILES, RANKS};

    fn at(name: &str) -> Square {
        Square::parse(name).expect("a square")
    }

    fn names(set: Bitboard) -> Vec<String> {
        let mut out: Vec<String> = set.map(|square| square.name()).collect();
        out.sort();
        out
    }

    #[test]
    fn a_pawn_moves_one_square_forward_and_the_sides_are_opposites() {
        let square = at("5e");
        assert_eq!(
            names(reachable(Side::Black, Role::Pawn, square, Bitboard::EMPTY)),
            ["5d"]
        );
        assert_eq!(
            names(reachable(Side::White, Role::Pawn, square, Bitboard::EMPTY)),
            ["5f"]
        );
    }

    /// **The piece most often got wrong.** A shogi knight jumps two forward and
    /// one across, and — unlike a chess knight — it has only those two moves
    /// and cannot go sideways or back.
    #[test]
    fn a_knight_has_exactly_two_moves_and_both_are_forward() {
        let moves = reachable(Side::Black, Role::Knight, at("5e"), Bitboard::EMPTY);
        assert_eq!(names(moves), ["4c", "6c"]);
        assert_eq!(moves.count(), 2);
    }

    /// A silver moves forward and diagonally but **not sideways or straight
    /// back**; a gold moves sideways and straight back but **not diagonally
    /// backwards**. They are near-mirrors of each other and confusing the two
    /// is the classic beginner's error — in a program as much as at a board.
    #[test]
    fn silver_and_gold_differ_in_exactly_the_places_they_should() {
        let silver = names(reachable(
            Side::Black,
            Role::Silver,
            at("5e"),
            Bitboard::EMPTY,
        ));
        let gold = names(reachable(
            Side::Black,
            Role::Gold,
            at("5e"),
            Bitboard::EMPTY,
        ));

        assert_eq!(silver, ["4d", "4f", "5d", "6d", "6f"]);
        assert_eq!(gold, ["4d", "4e", "5d", "5f", "6d", "6e"]);

        // The silver has the back diagonals and the gold does not.
        assert!(silver.contains(&"4f".to_string()) && !gold.contains(&"4f".to_string()));
        // The gold has sideways and straight back, and the silver does not.
        assert!(gold.contains(&"4e".to_string()) && !silver.contains(&"4e".to_string()));
        assert!(gold.contains(&"5f".to_string()) && !silver.contains(&"5f".to_string()));
    }

    #[test]
    fn the_four_gold_movers_move_as_a_gold() {
        let gold = reachable(Side::Black, Role::Gold, at("5e"), Bitboard::EMPTY);
        for role in [
            Role::PromotedPawn,
            Role::PromotedLance,
            Role::PromotedKnight,
            Role::PromotedSilver,
        ] {
            assert_eq!(
                reachable(Side::Black, role, at("5e"), Bitboard::EMPTY),
                gold,
                "{role:?}"
            );
        }
    }

    #[test]
    fn a_lance_runs_forward_until_something_stops_it() {
        let empty = reachable(Side::Black, Role::Lance, at("5i"), Bitboard::EMPTY);
        assert_eq!(empty.count(), 8, "the whole file ahead of it");

        let blocker = Bitboard::from_square(at("5f"));
        let blocked = reachable(Side::Black, Role::Lance, at("5i"), blocker);
        // It reaches the blocker and stops: 5h, 5g, 5f.
        assert_eq!(names(blocked), ["5f", "5g", "5h"]);
    }

    #[test]
    fn a_horse_is_a_bishop_plus_the_four_steps_it_did_not_have() {
        let bishop = reachable(Side::Black, Role::Bishop, at("5e"), Bitboard::EMPTY);
        let horse = reachable(Side::Black, Role::Horse, at("5e"), Bitboard::EMPTY);
        // Everything a bishop reaches, plus exactly the four orthogonal steps.
        assert_eq!(horse.count(), bishop.count() + 4);
        for square in ["5d", "5f", "4e", "6e"] {
            assert!(horse.contains(at(square)), "{square}");
            assert!(!bishop.contains(at(square)), "{square}");
        }
    }

    #[test]
    fn a_dragon_is_a_rook_plus_the_four_diagonal_steps() {
        let rook = reachable(Side::Black, Role::Rook, at("5e"), Bitboard::EMPTY);
        let dragon = reachable(Side::Black, Role::Dragon, at("5e"), Bitboard::EMPTY);
        assert_eq!(dragon.count(), rook.count() + 4);
        for square in ["4d", "6d", "4f", "6f"] {
            assert!(dragon.contains(at(square)), "{square}");
        }
    }

    /// A rook and a bishop are the two pieces that do not care which way they
    /// face, and a table that mirrored them would be a table with a bug in it.
    #[test]
    fn the_sideless_pieces_are_the_same_for_both_sides() {
        for role in [Role::Rook, Role::Bishop, Role::King] {
            assert_eq!(
                reachable(Side::Black, role, at("5e"), Bitboard::EMPTY),
                reachable(Side::White, role, at("5e"), Bitboard::EMPTY),
                "{role:?}"
            );
        }
    }

    /// The property behind all of it: **White's moves are Black's, reflected**.
    /// One test over every stepping role and every square, which is what makes
    /// a single mirrored direction impossible to hide.
    #[test]
    fn white_is_black_reflected_through_the_middle_rank() {
        for role in [
            Role::Pawn,
            Role::Knight,
            Role::Silver,
            Role::Gold,
            Role::King,
            Role::PromotedPawn,
        ] {
            for index in 0..SQUARES as u8 {
                let from = Square::at(index);
                let mirror = |square: Square| {
                    Square::from_file_rank(square.file() as i16, (RANKS - 1 - square.rank()) as i16)
                        .expect("still on the board")
                };
                let black = reachable(Side::Black, role, from, Bitboard::EMPTY);
                let white = reachable(Side::White, role, mirror(from), Bitboard::EMPTY);
                let reflected: Bitboard =
                    black.fold(Bitboard::EMPTY, |set, square| set.with(mirror(square)));
                assert_eq!(white, reflected, "{role:?} from {from}");
            }
        }
    }

    #[test]
    fn nothing_reaches_off_the_board() {
        for role in Role::ALL {
            for side in Side::ALL {
                for index in 0..SQUARES as u8 {
                    let set = reachable(side, role, Square::at(index), Bitboard::EMPTY);
                    assert_eq!(set.0 & !Bitboard::BOARD.0, 0, "{role:?} from {index}");
                    // And a piece never reaches its own square.
                    assert!(!set.contains(Square::at(index)), "{role:?} reaches itself");
                }
            }
        }
    }

    #[test]
    fn the_corner_does_not_wrap() {
        // A king on the right edge has five neighbours, not eight, and none of
        // them is on the far side of the board.
        let king = reachable(Side::Black, Role::King, at("1e"), Bitboard::EMPTY);
        assert_eq!(king.count(), 5);
        for square in king {
            assert!(square.file() >= FILES - 2, "{square} wrapped");
        }
    }
}
