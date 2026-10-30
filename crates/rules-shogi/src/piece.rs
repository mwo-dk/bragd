//! The pieces, and the two things that make shogi shogi.
//!
//! **Captured pieces change sides.** Nothing else in Bragd does that, and
//! it is why a piece has to know its side separately from where it is: a rook
//! taken from the other player becomes yours, and comes back as a rook you
//! drop.
//!
//! **Most pieces promote**, and promotion is a property of the piece on the
//! board rather than a one-off event: a promoted rook is a *dragon* for as long
//! as it lives, and reverts to a plain rook the moment it is captured. So the
//! role carries the promotion, and demotion happens in exactly one place — the
//! capture.

use core::fmt;

/// Which player. Shogi's own names, and they are not colours.
///
/// **Black moves first and moves up the board**, which is the reverse of what
/// "black" means in every other game in Bragd. Using shogi's names anyway,
/// because a shogi player reading "White to move" at the start of a game would
/// think something had gone wrong — and because the platform stores the two
/// seats as `white` and `black` for chess's sake regardless, so a
/// mapping is needed at the boundary whatever these are called.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Side {
    /// Sente. Moves first, moves up the board, sits at the bottom.
    Black,
    /// Gote. Moves second, moves down the board.
    White,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::Black, Side::White];

    pub const fn other(self) -> Side {
        match self {
            Side::Black => Side::White,
            Side::White => Side::Black,
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Side::Black => 0,
            Side::White => 1,
        }
    }

    /// Which way this side's pawns face: +1 up the board for Black.
    pub const fn forward(self) -> i16 {
        match self {
            Side::Black => 1,
            Side::White => -1,
        }
    }

    /// The three ranks this side promotes in, as a mask.
    pub const fn promotion_zone(self) -> crate::board::Bitboard {
        let mut mask = 0u128;
        let (from, to) = match self {
            Side::Black => (6, 9),
            Side::White => (0, 3),
        };
        let mut rank = from;
        while rank < to {
            let mut file = 0;
            while file < crate::board::FILES {
                mask |= 1u128 << (rank * crate::board::FILES + file);
                file += 1;
            }
            rank += 1;
        }
        crate::board::Bitboard(mask)
    }
}

/// What a piece is, promotion included.
///
/// Fourteen roles rather than eight plus a flag. A promoted silver moves as a
/// gold and has nothing else in common with a silver, so treating promotion as
/// a modifier would mean every movement lookup began by asking about it — and
/// the four pieces that promote to a gold would each need the same branch.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Role {
    Pawn,
    Lance,
    Knight,
    Silver,
    Gold,
    Bishop,
    Rook,
    King,
    /// Tokin. A promoted pawn, and it moves as a gold.
    PromotedPawn,
    PromotedLance,
    PromotedKnight,
    PromotedSilver,
    /// The horse: a bishop that may also step one square in any direction.
    Horse,
    /// The dragon: a rook that may also step one square diagonally.
    Dragon,
}

impl Role {
    pub const COUNT: usize = 14;

    pub const ALL: [Role; Role::COUNT] = [
        Role::Pawn,
        Role::Lance,
        Role::Knight,
        Role::Silver,
        Role::Gold,
        Role::Bishop,
        Role::Rook,
        Role::King,
        Role::PromotedPawn,
        Role::PromotedLance,
        Role::PromotedKnight,
        Role::PromotedSilver,
        Role::Horse,
        Role::Dragon,
    ];

    /// The seven kinds a player can hold in hand, in the order hands are
    /// conventionally written: strongest first.
    ///
    /// A king is never in hand — the game ends before it could be — and a
    /// promoted piece reverts when captured, which is why this list is of
    /// unpromoted roles only.
    pub const IN_HAND: [Role; 7] = [
        Role::Rook,
        Role::Bishop,
        Role::Gold,
        Role::Silver,
        Role::Knight,
        Role::Lance,
        Role::Pawn,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    /// What this becomes on promotion, or `None` if it cannot promote.
    ///
    /// Gold and the king are the two that cannot, and every promoted piece is
    /// already promoted. That is the whole rule.
    pub const fn promoted(self) -> Option<Role> {
        match self {
            Role::Pawn => Some(Role::PromotedPawn),
            Role::Lance => Some(Role::PromotedLance),
            Role::Knight => Some(Role::PromotedKnight),
            Role::Silver => Some(Role::PromotedSilver),
            Role::Bishop => Some(Role::Horse),
            Role::Rook => Some(Role::Dragon),
            _ => None,
        }
    }

    /// What this reverts to when captured.
    ///
    /// **Called on exactly one path — the capture — and that is the point.** A
    /// dragon taken from the other player becomes a rook in your hand, and if
    /// demotion happened anywhere else it would eventually happen twice or not
    /// at all.
    pub const fn demoted(self) -> Role {
        match self {
            Role::PromotedPawn => Role::Pawn,
            Role::PromotedLance => Role::Lance,
            Role::PromotedKnight => Role::Knight,
            Role::PromotedSilver => Role::Silver,
            Role::Horse => Role::Bishop,
            Role::Dragon => Role::Rook,
            other => other,
        }
    }

    pub const fn is_promoted(self) -> bool {
        matches!(
            self,
            Role::PromotedPawn
                | Role::PromotedLance
                | Role::PromotedKnight
                | Role::PromotedSilver
                | Role::Horse
                | Role::Dragon
        )
    }

    /// Whether a piece of this role standing on `rank` could ever move again.
    ///
    /// The rule behind three drop restrictions and one forced promotion: a pawn
    /// or a lance on the furthest rank has no move, and a knight on either of
    /// the two furthest ranks has none. Dropping one there is illegal, and
    /// moving one there promotes whether the player wants it or not.
    pub const fn is_stuck_on(self, side: Side, rank: u8) -> bool {
        let from_far_edge = match side {
            Side::Black => crate::board::RANKS - 1 - rank,
            Side::White => rank,
        };
        match self {
            Role::Pawn | Role::Lance => from_far_edge == 0,
            Role::Knight => from_far_edge <= 1,
            _ => false,
        }
    }

    /// The letter used in the wire format and in Western shogi notation.
    /// Promoted pieces are the base letter with a `+`.
    pub const fn letter(self) -> &'static str {
        match self {
            Role::Pawn => "P",
            Role::Lance => "L",
            Role::Knight => "N",
            Role::Silver => "S",
            Role::Gold => "G",
            Role::Bishop => "B",
            Role::Rook => "R",
            Role::King => "K",
            Role::PromotedPawn => "+P",
            Role::PromotedLance => "+L",
            Role::PromotedKnight => "+N",
            Role::PromotedSilver => "+S",
            Role::Horse => "+B",
            Role::Dragon => "+R",
        }
    }

    pub fn from_letter(text: &str) -> Option<Role> {
        Role::ALL
            .into_iter()
            .find(|role| role.letter().eq_ignore_ascii_case(text))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Piece {
    pub side: Side,
    pub role: Role,
}

impl Piece {
    pub const fn new(side: Side, role: Role) -> Piece {
        Piece { side, role }
    }
}

impl fmt::Display for Piece {
    /// SFEN's spelling: uppercase Black, lowercase White, `+` for promoted.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let letter = self.role.letter();
        match self.side {
            Side::Black => f.write_str(letter),
            Side::White => f.write_str(&letter.to_ascii_lowercase()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_and_demotion_are_inverses_where_both_exist() {
        for role in Role::ALL {
            if let Some(promoted) = role.promoted() {
                assert_eq!(promoted.demoted(), role, "{role:?}");
                assert!(promoted.is_promoted(), "{promoted:?}");
                assert_eq!(promoted.promoted(), None, "{promoted:?} promotes twice");
            }
        }
    }

    #[test]
    fn gold_and_the_king_do_not_promote() {
        assert_eq!(Role::Gold.promoted(), None);
        assert_eq!(Role::King.promoted(), None);
    }

    #[test]
    fn a_capture_always_yields_something_that_can_be_dropped() {
        // Every role demotes to one of the seven a hand can hold — except the
        // king, which is never captured. If this ever failed, a capture would
        // put something in a hand that cannot be dropped.
        for role in Role::ALL {
            if role == Role::King {
                continue;
            }
            assert!(
                Role::IN_HAND.contains(&role.demoted()),
                "{role:?} demotes to {:?}, which cannot be held",
                role.demoted()
            );
        }
    }

    #[test]
    fn the_promotion_zone_is_the_far_three_ranks() {
        assert_eq!(Side::Black.promotion_zone().count(), 27);
        assert_eq!(Side::White.promotion_zone().count(), 27);
        // And they do not overlap, which is the mistake a copied constant makes.
        assert!((Side::Black.promotion_zone() & Side::White.promotion_zone()).is_empty());
        // Black promotes at the top: 1a is in Black's zone, 1i is not.
        let top = crate::board::Square::parse("1a").unwrap();
        let bottom = crate::board::Square::parse("1i").unwrap();
        assert!(Side::Black.promotion_zone().contains(top));
        assert!(!Side::Black.promotion_zone().contains(bottom));
        assert!(Side::White.promotion_zone().contains(bottom));
    }

    #[test]
    fn a_pawn_on_the_far_rank_has_nowhere_to_go() {
        assert!(Role::Pawn.is_stuck_on(Side::Black, 8));
        assert!(!Role::Pawn.is_stuck_on(Side::Black, 7));
        assert!(Role::Pawn.is_stuck_on(Side::White, 0));
        // A knight is stuck two ranks out, because it jumps two.
        assert!(Role::Knight.is_stuck_on(Side::Black, 7));
        assert!(Role::Knight.is_stuck_on(Side::Black, 8));
        assert!(!Role::Knight.is_stuck_on(Side::Black, 6));
        // Nothing else is ever stuck.
        for role in [
            Role::Silver,
            Role::Gold,
            Role::Bishop,
            Role::Rook,
            Role::King,
        ] {
            for rank in 0..crate::board::RANKS {
                assert!(!role.is_stuck_on(Side::Black, rank), "{role:?}");
            }
        }
    }

    #[test]
    fn letters_round_trip() {
        for role in Role::ALL {
            assert_eq!(Role::from_letter(role.letter()), Some(role));
        }
    }
}
