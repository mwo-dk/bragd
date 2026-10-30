//! The doubling cube: a second game running beside the first.
//!
//! Nothing else in Bragd has anything like it. The cube is state that is not
//! the board, it changes hands, it multiplies the score, and **a game that ends
//! by a declined double did not end by a move** — the loser conceded the current
//! stake rather than being beaten on the board. Modelling it as a kind of
//! resignation would lose the two things that matter about it: what the stake
//! was, and who was holding the cube when it happened.
//!
//! The rules, which are short and are the whole of this module:
//!
//! - The cube starts in the middle at 1, owned by nobody, and **either player
//!   may double** while it is there.
//! - After that only its **owner** may double, and doubling **passes ownership
//!   to whoever accepts**. That is the entire mechanism: a player who doubles
//!   hands the opponent the right to double back at twice the stake, which is
//!   what stops doubling being free.
//! - A double is offered before rolling, on your own turn. The engine enforces
//!   the turn; the platform enforces "before rolling" by the shape of its
//!   messages, because [`crate::Game::play`] takes the roll and the turn
//!   together and there is no state in between for a roll to sit in.
//! - Declining ends the game at the value **before** the double. Accepting
//!   doubles it.
//!
//! Match play adds the Crawford rule, automatic doubles and the Jacoby rule.
//! None of them are here: each is a property of a *match* rather than of a game,
//! and a match is not something this crate models.

use crate::board::Side;

/// The cube stops here. Money play has no ceiling, but 64 is where match play
/// stops and a cube past it is a bug rather than a game.
pub const MAX_CUBE: u16 = 64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cube {
    value: u16,
    /// `None` while the cube is centred, which is the only time either player
    /// may double.
    owner: Option<Side>,
    /// A double on the table, from this side, waiting to be taken or dropped.
    offer: Option<Side>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CubeError {
    /// The cube belongs to the other player, so this one may not double.
    NotYours,
    /// A double is already on the table.
    AlreadyOffered,
    /// Nothing has been offered, so there is nothing to take or drop.
    NoOffer,
    /// The offer is not this player's to answer — you cannot take your own
    /// double.
    NotYoursToAnswer,
    /// The cube is at [`MAX_CUBE`].
    AtMaximum,
}

impl Default for Cube {
    fn default() -> Cube {
        Cube::centred()
    }
}

impl Cube {
    pub const fn centred() -> Cube {
        Cube {
            value: 1,
            owner: None,
            offer: None,
        }
    }

    pub const fn value(self) -> u16 {
        self.value
    }

    pub const fn owner(self) -> Option<Side> {
        self.owner
    }

    /// Who has a double on the table, if anybody.
    pub const fn offer(self) -> Option<Side> {
        self.offer
    }

    pub const fn is_offered(self) -> bool {
        self.offer.is_some()
    }

    /// What the game becomes worth if the double on the table is taken, or
    /// `None` when nothing is on the table.
    ///
    /// Two lines, and they are here rather than in a front end for the reason every
    /// other rule is: the answer is not always `value * 2` — the cube stops at
    /// [`MAX_CUBE`], so there is a ceiling to hit — and a player deciding whether
    /// to take is deciding about exactly this number. Working it out in a
    /// renderer would be cube arithmetic in a language where nothing checks it.
    pub const fn offered_value(self) -> Option<u16> {
        match self.offer {
            None => None,
            Some(_) => Some(self.value * 2),
        }
    }

    /// Whether `side` may double now: the cube is theirs or centred, nothing is
    /// already offered, and there is somewhere left to go.
    pub fn may_double(self, side: Side) -> bool {
        self.offer.is_none()
            && self.value < MAX_CUBE
            && self.owner.is_none_or(|owner| owner == side)
    }

    pub fn offer_by(&mut self, side: Side) -> Result<(), CubeError> {
        if self.offer.is_some() {
            return Err(CubeError::AlreadyOffered);
        }
        if self.value >= MAX_CUBE {
            return Err(CubeError::AtMaximum);
        }
        if !self.owner.is_none_or(|owner| owner == side) {
            return Err(CubeError::NotYours);
        }
        self.offer = Some(side);
        Ok(())
    }

    /// Takes the double. **The stake doubles and the cube changes hands** — the
    /// accepter now owns it and is the only one who may double next.
    pub fn accept_by(&mut self, side: Side) -> Result<(), CubeError> {
        match self.offer {
            None => Err(CubeError::NoOffer),
            Some(doubler) if doubler == side => Err(CubeError::NotYoursToAnswer),
            Some(_) => {
                self.value *= 2;
                self.owner = Some(side);
                self.offer = None;
                Ok(())
            }
        }
    }

    /// Declines, and returns the stake the game is now worth: **the value
    /// before the double**, because a player who drops pays what was on the
    /// table when the double was offered.
    pub fn decline_by(&mut self, side: Side) -> Result<u16, CubeError> {
        match self.offer {
            None => Err(CubeError::NoOffer),
            Some(doubler) if doubler == side => Err(CubeError::NotYoursToAnswer),
            Some(_) => {
                self.offer = None;
                Ok(self.value)
            }
        }
    }
}

impl core::fmt::Display for CubeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            CubeError::NotYours => "the cube is not yours to double",
            CubeError::AlreadyOffered => "a double is already on the table",
            CubeError::NoOffer => "nothing has been doubled",
            CubeError::NotYoursToAnswer => "that double is not yours to answer",
            CubeError::AtMaximum => "the cube is already at sixty-four",
        })
    }
}

impl std::error::Error for CubeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_centred_cube_may_be_doubled_by_either_side() {
        let cube = Cube::centred();
        assert_eq!(cube.value(), 1);
        assert_eq!(cube.owner(), None);
        assert!(cube.may_double(Side::White));
        assert!(cube.may_double(Side::Black));
    }

    #[test]
    fn doubling_passes_ownership_to_the_accepter() {
        // The whole mechanism in four lines: White doubles, Black takes, and it
        // is now Black who holds the cube and may double back at four.
        let mut cube = Cube::centred();
        cube.offer_by(Side::White).expect("the cube is centred");
        assert_eq!(cube.offer(), Some(Side::White));
        // What Black is being asked to play for, which is the number the whole
        // decision is about.
        assert_eq!(cube.offered_value(), Some(2));
        cube.accept_by(Side::Black).expect("Black takes");

        assert_eq!(cube.value(), 2);
        assert_eq!(cube.owner(), Some(Side::Black));
        assert!(!cube.is_offered());
        assert_eq!(cube.offered_value(), None, "nothing is on the table now");
        assert!(cube.may_double(Side::Black));
        assert!(!cube.may_double(Side::White), "White gave the cube away");
    }

    #[test]
    fn only_the_owner_may_double() {
        let mut cube = Cube::centred();
        cube.offer_by(Side::White).expect("centred");
        cube.accept_by(Side::Black).expect("taken");
        assert_eq!(cube.offer_by(Side::White), Err(CubeError::NotYours));
        cube.offer_by(Side::Black).expect("Black owns it");
    }

    #[test]
    fn a_double_cannot_be_answered_by_the_player_who_offered_it() {
        let mut cube = Cube::centred();
        cube.offer_by(Side::White).expect("centred");
        assert_eq!(
            cube.accept_by(Side::White),
            Err(CubeError::NotYoursToAnswer)
        );
        assert_eq!(
            cube.decline_by(Side::White),
            Err(CubeError::NotYoursToAnswer)
        );
        assert_eq!(cube.offer_by(Side::White), Err(CubeError::AlreadyOffered));
    }

    #[test]
    fn declining_costs_the_value_before_the_double() {
        // Not the doubled value. A player who drops a double from 2 to 4 pays 2,
        // which is why doubling out of a lost position is worth doing.
        let mut cube = Cube::centred();
        cube.offer_by(Side::White).expect("centred");
        cube.accept_by(Side::Black).expect("taken");
        cube.offer_by(Side::Black).expect("Black owns it");
        assert_eq!(cube.decline_by(Side::White), Ok(2));
        assert_eq!(cube.value(), 2, "a declined double never doubles anything");
    }

    #[test]
    fn the_cube_stops_at_sixty_four() {
        let mut cube = Cube::centred();
        let mut taker = Side::Black;
        while cube.value() < MAX_CUBE {
            cube.offer_by(taker.other()).expect("the owner doubles");
            cube.accept_by(taker).expect("taken");
            taker = taker.other();
        }
        assert_eq!(cube.value(), MAX_CUBE);
        assert!(!cube.may_double(Side::White));
        assert!(!cube.may_double(Side::Black));
        assert_eq!(cube.offer_by(taker.other()), Err(CubeError::AtMaximum));
    }

    #[test]
    fn there_is_nothing_to_take_until_something_is_offered() {
        let mut cube = Cube::centred();
        assert_eq!(cube.accept_by(Side::White), Err(CubeError::NoOffer));
        assert_eq!(cube.decline_by(Side::Black), Err(CubeError::NoOffer));
    }
}
