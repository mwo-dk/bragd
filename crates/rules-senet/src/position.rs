//! The position, and the rules for moving from it.
//!
//! Senet has the most intricate rules of any game in Bragd, and almost all of
//! the intricacy is in the last five houses. Four things here have no counterpart
//! anywhere else in `crates/`:
//!
//! **Nothing is captured.** Landing on a lone enemy *exchanges* the two pieces —
//! yours goes forward, theirs goes back to the house you left. Material never
//! changes and no piece is ever taken off by contact.
//!
//! **A piece can be forced backwards.** If no forward move is legal, a backward
//! one must be played. That is why a move carries a direction and why
//! [`Position::moves_for`] answers in two passes rather than one.
//!
//! **House 26 cannot be passed.** A piece below it must land on it exactly before
//! going further, which means any piece standing on 26 blockades everything
//! behind it — *including its own side*. It is positional rather than a
//! once-per-piece flag: a piece exchanged back below 26 owes the stop again.
//!
//! **The water freezes an army.** A piece on house 27 stops every other piece of
//! its owner's from moving at all, until it escapes on an exact 4 or is rescued
//! to house 15.
//!
//! Every one of those is a reading of Kendall rather than a fact about history,
//! and `docs/rules/ur-and-senet-rules.md` names each one and the alternatives.

use crate::board::{
    BEAUTY, HOUSES, LAST, PIECES, RE_ATOUM, REBIRTH, Side, THREE_TRUTHS, WATER, adjacent,
    exact_exit,
};
use crate::sticks::Throw;

/// Where a piece is.
///
/// A piece is on a house or it is home. There is no "waiting to enter" — senet
/// starts with every piece already on the board, which is why this has two
/// states where Ur's has three.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Spot {
    /// On this house, 1..=30.
    House(u8),
    /// Borne off, and out of the game.
    Off,
}

impl Spot {
    pub fn house(self) -> Option<u8> {
        match self {
            Spot::House(house) => Some(house),
            Spot::Off => None,
        }
    }
}

/// Which way a piece is being moved.
///
/// Backwards is not a choice a player makes for fun: it is compulsory, and only
/// when nothing forward is legal. It travels on the move so that the record says
/// which happened, because the same two houses can be joined either way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Way {
    Forward,
    Backward,
}

/// A move: a piece leaves one house and arrives somewhere.
///
/// **Its two ends and its direction, and nothing else.** Whether it exchanges,
/// whether it drowns somebody, whether the turn continues — all of those are
/// facts about the position it is played from, so they live on [`Offer`] and
/// never in the record. Ur's engine learned that the hard way and the note is in
/// `crates/rules-ur/src/position.rs`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Move {
    pub from: u8,
    pub to: Spot,
    pub way: Way,
}

impl Move {
    /// The wire spelling: `from-to`, with `off` for a piece leaving the board and
    /// a leading `<` for the backward moves.
    ///
    /// The marker is on the move rather than left to be inferred, because a
    /// backward move from 12 to 9 and a forward move from 12 to 9 are both
    /// spellable and only one of them is what happened.
    pub fn to_wire(self) -> String {
        let arrow = if self.way == Way::Backward { "<" } else { "" };
        match self.to {
            Spot::Off => format!("{arrow}{}-off", self.from),
            Spot::House(house) => format!("{arrow}{}-{house}", self.from),
        }
    }

    /// Reads a move back. Does not judge legality — that is `moves_for`'s job.
    pub fn parse(text: &str) -> Option<Move> {
        let (way, rest) = match text.strip_prefix('<') {
            Some(rest) => (Way::Backward, rest),
            None => (Way::Forward, text),
        };
        let (from, to) = rest.split_once('-')?;
        let from = house_from(from)?;
        let to = if to == "off" {
            Spot::Off
        } else {
            Spot::House(house_from(to)?)
        };
        Some(Move { from, to, way })
    }
}

/// A house number, read strictly: digits only, in range, and no leading zero.
fn house_from(text: &str) -> Option<u8> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if text.len() > 1 && text.starts_with('0') {
        return None;
    }
    let house: u8 = text.parse().ok()?;
    (1..=HOUSES as u8).contains(&house).then_some(house)
}

/// A legal move, with everything it would do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Offer {
    pub chosen: Move,
    /// Whether an enemy piece is exchanged by this move.
    pub exchanges: bool,
    /// Whether the enemy piece exchanged is sent to the water rather than to the
    /// house this move came from.
    pub drowns: bool,
    /// Whether this move puts the mover's own piece in the water.
    pub into_the_water: bool,
    /// Whether this move takes a piece off the board.
    pub bears_off: bool,
}

/// The whole board.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Position {
    white: [Spot; PIECES as usize],
    black: [Spot; PIECES as usize],
    to_move: Side,
}

impl Position {
    /// The opening array: seven a side, alternating along houses 1 to 14.
    ///
    /// **The evens move first**, and take the seat storage calls white. Kendall
    /// paints the odds white and the evens black; the seats here are named for
    /// who moves first, as they are for every other family, and the front end calls
    /// them what the variant calls them.
    pub fn starting() -> Position {
        let mut white = [Spot::Off; PIECES as usize];
        let mut black = [Spot::Off; PIECES as usize];
        for index in 0..PIECES as usize {
            // Evens 2..=14 to the first mover, odds 1..=13 to the other.
            white[index] = Spot::House((index as u8 + 1) * 2);
            black[index] = Spot::House((index as u8) * 2 + 1);
        }
        Position {
            white,
            black,
            to_move: Side::White,
        }
    }

    pub fn to_move(&self) -> Side {
        self.to_move
    }

    pub fn pieces(&self, side: Side) -> &[Spot] {
        match side {
            Side::White => &self.white,
            Side::Black => &self.black,
        }
    }

    fn pieces_mut(&mut self, side: Side) -> &mut [Spot] {
        match side {
            Side::White => &mut self.white,
            Side::Black => &mut self.black,
        }
    }

    fn normalise(&mut self) {
        self.white.sort_unstable();
        self.black.sort_unstable();
    }

    /// How many of a side's pieces are home.
    pub fn off(&self, side: Side) -> u8 {
        self.pieces(side)
            .iter()
            .filter(|spot| **spot == Spot::Off)
            .count() as u8
    }

    /// Who, if anyone, stands on this house.
    pub fn occupant(&self, house: u8) -> Option<Side> {
        [Side::White, Side::Black]
            .into_iter()
            .find(|&side| self.pieces(side).contains(&Spot::House(house)))
    }

    /// Which house, if any, holds a side's piece in the water.
    pub fn drowned(&self, side: Side) -> bool {
        self.occupant(WATER) == Some(side)
    }

    /// Whether the piece on this house is protected from being exchanged.
    ///
    /// Protected means a piece of its own side stands on a house adjacent along
    /// the path — bends included, which is a reading rather than a lookup (see
    /// [`crate::board::adjacent`]).
    ///
    /// **A piece in the water is never protected**, whatever stands beside it.
    /// The transcription strips protection from the drowned piece only, so a
    /// piece standing *beside* the water still confers it outward — which is why
    /// this checks the subject's own house rather than the neighbour's.
    pub fn protected(&self, house: u8) -> bool {
        if house == WATER {
            return false;
        }
        let Some(side) = self.occupant(house) else {
            return false;
        };
        self.pieces(side).iter().any(|spot| match spot {
            Spot::House(other) => adjacent(*other, house),
            Spot::Off => false,
        })
    }

    /// Whether a side may land on this house.
    ///
    /// Empty, or a lone enemy who is not protected. Never your own piece: senet
    /// has no stacking.
    fn may_land(&self, side: Side, house: u8) -> bool {
        match self.occupant(house) {
            None => true,
            Some(owner) if owner == side => false,
            Some(_) => !self.protected(house),
        }
    }

    /// The legal moves for a throw.
    ///
    /// **Two passes, and the second only if the first found nothing.** Moving is
    /// compulsory, and a backward move is not an option a player weighs against a
    /// forward one — it is what happens when there is no forward one. Offering
    /// both together would let a player retreat by choice, which is a different
    /// game.
    ///
    /// An empty answer means the turn passes, and it is a legal outcome rather
    /// than an error.
    pub fn moves_for(&self, throw: Throw) -> Vec<Offer> {
        if self.winner().is_some() {
            return Vec::new();
        }
        let side = self.to_move;

        // **The water freezes everything else.** While a piece of this side is on
        // house 27, no other piece of theirs may move at all — the only actions
        // are the water's own, and they are not ordinary moves.
        if self.drowned(side) {
            return self.water_moves(side, throw);
        }

        let forward = self.moves_one_way(side, throw, Way::Forward);
        if !forward.is_empty() {
            return forward;
        }
        self.moves_one_way(side, throw, Way::Backward)
    }

    /// What a side with a piece in the water may do.
    ///
    /// Two actions and no others: escape on an exact four, or — if the House of
    /// Rebirth is empty — be lifted onto it without throwing. The rescue is not a
    /// function of the throw, so it is offered whatever was thrown; the caller
    /// asks for moves after throwing either way, and Kendall's text makes the
    /// rescue "instead of throwing", which comes to the same thing once a throw
    /// exists.
    fn water_moves(&self, side: Side, throw: Throw) -> Vec<Offer> {
        let mut moves = Vec::new();

        // The exact four, which is `31 - 27` and the same arithmetic as every
        // other exit.
        if Some(throw.steps()) == exact_exit(WATER) {
            moves.push(Offer {
                chosen: Move {
                    from: WATER,
                    to: Spot::Off,
                    way: Way::Forward,
                },
                exchanges: false,
                drowns: false,
                into_the_water: false,
                bears_off: true,
            });
        }

        // The rescue, which needs house 15 to be empty of *anybody* — protection
        // does not enter into it, because nothing is being landed on.
        if self.occupant(REBIRTH).is_none() {
            moves.push(Offer {
                chosen: Move {
                    from: WATER,
                    to: Spot::House(REBIRTH),
                    // Backward, and truthfully so: 27 to 15 is a retreat, and
                    // spelling it forward would make the record claim a piece
                    // advanced twelve houses.
                    way: Way::Backward,
                },
                exchanges: false,
                drowns: false,
                into_the_water: false,
                bears_off: false,
            });
        }

        let _ = side;
        moves
    }

    /// Every legal move in one direction.
    fn moves_one_way(&self, side: Side, throw: Throw, way: Way) -> Vec<Offer> {
        let steps = throw.steps();
        let mut moves = Vec::new();

        for &spot in self.pieces(side) {
            let Spot::House(from) = spot else { continue };

            // **Houses 28 and 29 never move forward.** A piece there leaves the
            // board on its exact throw and does nothing else — it cannot advance
            // to 29 or 30, and it cannot be pushed along by a bigger number.
            let stuck = matches!(from, THREE_TRUTHS | RE_ATOUM);

            let to = match way {
                Way::Forward => {
                    // Bearing off, for the houses that do it.
                    let exits = match from {
                        LAST => true,
                        house if house >= BEAUTY => exact_exit(house) == Some(steps),
                        _ => false,
                    };
                    if exits {
                        moves.push(Offer {
                            chosen: Move {
                                from,
                                to: Spot::Off,
                                way,
                            },
                            exchanges: false,
                            drowns: false,
                            into_the_water: false,
                            bears_off: true,
                        });
                        continue;
                    }
                    if stuck {
                        continue;
                    }
                    let landing = from as u16 + steps as u16;
                    if landing > HOUSES as u16 {
                        continue;
                    }
                    // **The House of Beauty cannot be passed.** A piece below it
                    // must land on it exactly first, so any move that would carry
                    // one beyond is simply not there.
                    if from < BEAUTY && landing > BEAUTY as u16 {
                        continue;
                    }
                    landing as u8
                }
                Way::Backward => {
                    let landing = from as i16 - steps as i16;
                    if landing < 1 {
                        continue;
                    }
                    landing as u8
                }
            };

            if !self.may_land(side, to) {
                continue;
            }

            let exchanges = self.occupant(to) == Some(side.other());
            // **An exchange into the last three houses drowns the victim.** It
            // goes to the water rather than to the house the mover came from —
            // unless the water is taken, in which case the ordinary exchange
            // applies and it goes back to the House of Beauty.
            let drowns = exchanges
                && matches!(to, THREE_TRUTHS | RE_ATOUM | LAST)
                && self.occupant(WATER).is_none();

            moves.push(Offer {
                chosen: Move {
                    from,
                    to: Spot::House(to),
                    way,
                },
                exchanges,
                drowns,
                into_the_water: to == WATER,
                bears_off: false,
            });
        }

        moves.sort_by_key(|offer| (offer.chosen.from, offer.chosen.to));
        moves.dedup_by_key(|offer| (offer.chosen.from, offer.chosen.to));
        moves
    }

    /// Plays a move that [`Self::moves_for`] offered.
    ///
    /// Returns whether the mover keeps the turn, which is **not** simply what the
    /// throw says: landing in the water ends a turn whatever was thrown, and so
    /// does a rescue.
    pub fn play(&mut self, chosen: Move, throw: Throw) -> bool {
        let side = self.to_move;
        let mover = side;

        // The exchange happens first, while the mover's origin is still free to
        // receive the displaced piece.
        if let Spot::House(to) = chosen.to
            && self.occupant(to) == Some(side.other()) {
                let enemy = side.other();
                // Where the displaced piece goes: the water if this is a drowning,
                // the House of Beauty if the water is taken, otherwise the house
                // the mover has just left.
                let sent = if matches!(to, THREE_TRUTHS | RE_ATOUM | LAST) {
                    if self.occupant(WATER).is_none() {
                        WATER
                    } else {
                        BEAUTY
                    }
                } else {
                    chosen.from
                };
                for spot in self.pieces_mut(enemy) {
                    if *spot == Spot::House(to) {
                        *spot = Spot::House(sent);
                        break;
                    }
                }
            }

        for spot in self.pieces_mut(mover) {
            if *spot == Spot::House(chosen.from) {
                *spot = chosen.to;
                break;
            }
        }

        // **A turn that touched the water is over**, whatever was thrown: a piece
        // landing on 27 ends it at once, and a rescue consumes the whole turn.
        let touched_water = chosen.to == Spot::House(WATER)
            || (chosen.from == WATER && chosen.to == Spot::House(REBIRTH));
        let again = throw.throws_again() && !touched_water;
        if !again {
            self.to_move = side.other();
        }
        self.normalise();
        again
    }

    /// Passes the turn, which is what a throw with no move does.
    pub fn pass(&mut self) {
        self.to_move = self.to_move.other();
    }

    /// The side that has brought all seven pieces home, if either has.
    pub fn winner(&self) -> Option<Side> {
        [Side::White, Side::Black]
            .into_iter()
            .find(|&side| self.off(side) == PIECES)
    }

    /// How far a side still has to travel, counting a piece on house *n* as
    /// `31 - n`. The one number that says who is ahead.
    pub fn distance_left(&self, side: Side) -> u16 {
        self.pieces(side)
            .iter()
            .map(|spot| match spot {
                Spot::House(house) => 31 - *house as u16,
                Spot::Off => 0,
            })
            .sum()
    }

    /// The position, written down.
    ///
    /// Thirty characters in **house order**, then the two borne-off counts and
    /// the side to move. House order rather than grid order, deliberately: the
    /// path is the game, and a picture in grid order would need the reader to
    /// know which way the middle row runs before it meant anything.
    pub fn to_picture(&self) -> String {
        let mut board = String::with_capacity(HOUSES);
        for house in 1..=HOUSES as u8 {
            board.push(match self.occupant(house) {
                Some(Side::White) => 'W',
                Some(Side::Black) => 'B',
                None => '.',
            });
        }
        format!(
            "{board} {}{} {}",
            self.off(Side::White),
            self.off(Side::Black),
            if self.to_move == Side::White {
                'w'
            } else {
                'b'
            }
        )
    }

    /// Reads a position back, refusing anything that does not add up.
    pub fn from_picture(text: &str) -> Option<Position> {
        let mut parts = text.split_whitespace();
        let board = parts.next()?;
        let off = parts.next()?;
        let mover = parts.next()?;
        if parts.next().is_some() || board.len() != HOUSES || off.len() != 2 {
            return None;
        }

        let digit = |text: &str, at: usize| -> Option<u8> {
            let byte = text.as_bytes().get(at)?;
            byte.is_ascii_digit().then(|| byte - b'0')
        };
        let borne = [digit(off, 0)?, digit(off, 1)?];

        let mut white = Vec::new();
        let mut black = Vec::new();
        for (index, glyph) in board.chars().enumerate() {
            let house = index as u8 + 1;
            match glyph {
                '.' => continue,
                'W' => white.push(Spot::House(house)),
                'B' => black.push(Spot::House(house)),
                _ => return None,
            }
        }

        let mut position = Position {
            white: [Spot::Off; PIECES as usize],
            black: [Spot::Off; PIECES as usize],
            to_move: match mover {
                "w" => Side::White,
                "b" => Side::Black,
                _ => return None,
            },
        };

        for (side, mut spots, borne) in [
            (Side::White, white, borne[0]),
            (Side::Black, black, borne[1]),
        ] {
            for _ in 0..borne {
                spots.push(Spot::Off);
            }
            if spots.len() != PIECES as usize {
                return None;
            }
            position.pieces_mut(side).copy_from_slice(&spots);
        }

        position.normalise();
        Some(position)
    }
}
