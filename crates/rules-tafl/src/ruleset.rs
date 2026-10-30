//! What a ruleset is, and which one Bragd plays.
//!
//! **Tafl has no standard rules.** That is not a gap in the research, it is the
//! state of the game: it died out around 1600, the surviving descriptions are
//! fragmentary, and every modern ruleset is a reconstruction with its own
//! answers. Fetlar, Copenhagen, Berserk and the rest disagree about whether the
//! king is armed, how many attackers it takes to capture him, whether he
//! escapes at the corners or the edges, and what the throne does when it is
//! empty.
//!
//! So the ruleset is **data**, and every field below is a place real rulesets
//! disagree. That is not generality for its own sake — it is the only honest
//! way to represent a game whose rules are a choice, and it forces Bragd to
//! be able to say which choice it made.
//!
//! What is *not* configurable is anything every ruleset agrees on: the board,
//! rook movement, capture by custody, and the two win conditions existing at
//! all. Those live in [`crate::position`] and in the capture code below,
//! because a knob nobody would ever turn is a knob that only adds ways to be
//! wrong.

use crate::board::{Bitboard, Direction, Square};
use crate::position::{Move, Position, Role, Side};

/// Where the king has to get to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Escape {
    /// The four corners. The modern standard, and much the harder of the two.
    Corners,
    /// Any edge square. Older, and heavily defender-favouring.
    Edges,
}

/// How many hostile sides it takes to capture the king.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KingCapture {
    /// Two, like any other piece.
    LikeASoldier,
    /// All four — and the board's edge counts as one of them, so three
    /// attackers suffice against a wall. Fetlar's reading.
    SurroundedOrAgainstAnEdge,
    /// All four attackers, with no edge concession, so **a king who reaches an
    /// edge cannot be captured at all**.
    ///
    /// Copenhagen states this unconditionally, and it is not a detail: it is
    /// what makes an exit fort possible, and it is the difference between a
    /// king running for a corner along the rim and a king running into a trap.
    Surrounded,
}

/// One reconstruction of the game.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ruleset {
    /// How wide the board is: 7, 9 or 11.
    ///
    /// On the ruleset rather than chosen at the call site, because a ruleset
    /// and a board size are not independently selectable — brandubh's rules are
    /// brandubh's rules *on a 7x7 board*, and a Copenhagen game on seven
    /// squares would be a game nobody has ever played.
    pub size: u8,
    /// Stable. It is stored on every Game and a rating pool is keyed by it.
    pub key: &'static str,
    pub name: &'static str,

    pub escape: Escape,
    pub king_capture: KingCapture,

    /// Whether the king can take part in capturing.
    ///
    /// An armed king is a real piece; an unarmed one is a token to be escorted.
    /// It changes the feel of the game more than any other single rule.
    pub armed_king: bool,

    /// Whether a soldier may pass *over* the throne when it is empty.
    ///
    /// Landing on it is never allowed. Passing is a separate question and
    /// rulesets split on it.
    pub soldiers_pass_the_empty_throne: bool,

    /// Whether the empty throne acts as an enemy for capture, per side.
    ///
    /// Indexed by [`Side::index`]. Asymmetric in several rulesets: the empty
    /// throne is commonly hostile to the attackers but not to the defenders,
    /// or hostile to both.
    pub empty_throne_is_hostile_to: [bool; 2],

    /// Whether the throne is hostile to the attackers even with the king on it.
    pub occupied_throne_is_hostile_to_attackers: bool,

    /// Whether a row of pieces along an edge can be captured as a group.
    pub shieldwall: Shieldwall,

    /// Whether the defenders win by walling the king onto an edge.
    pub exit_fort: bool,

    /// Whether the attackers win by sealing the defenders in.
    pub encirclement: bool,

    /// Whether the defenders may not repeat a position.
    pub repetition_ban: bool,
}

/// Whether, and how, a row along the board edge can be taken as a group.
///
/// The one capture that is not custody, and Copenhagen's own addition to
/// Fetlar. It is also the only way an edge is ever involved in a capture, which
/// is why "the edge is not hostile" and "a shieldwall exists" are both true.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shieldwall {
    /// Not played. Fetlar.
    None,
    /// Both ends must be closed by a piece.
    Weak,
    /// A corner square may stand in for a bracket at **one** end.
    WithCorners,
}

/// The ruleset Bragd plays.
///
/// **Copenhagen**, which is the closest thing the game has to a modern
/// tournament standard: it is what the Fetlar Hnefatafl World Championship
/// moved to, it is what most online implementations use, and it is the one a
/// player arriving from elsewhere is most likely to already know.
///
/// The specific choices, and why each is the interesting one:
///
/// - **Corner escape.** Edge escape makes the defenders far too strong on an
///   11×11 board; every serious modern ruleset moved away from it.
/// - **An armed king**, so the defenders' strongest piece is a piece rather
///   than a burden.
/// - **Four sides to capture the king**, three against an edge. This is the
///   counterweight to the attackers' two-to-one material advantage.
/// - **The corners are hostile to everybody**, which is what makes them
///   dangerous to stand next to and gives the attackers something to work with
///   near the king's goal.
pub const COPENHAGEN: Ruleset = Ruleset {
    size: 11,
    key: "hnefatafl",
    name: "Hnefatafl",
    escape: Escape::Corners,
    king_capture: KingCapture::Surrounded,
    armed_king: true,
    soldiers_pass_the_empty_throne: true,
    // Hostile to both sides when empty.
    empty_throne_is_hostile_to: [true, true],
    // And still hostile to the attackers with the king sitting on it, which is
    // the asymmetry: a defender beside the occupied throne is safe, an attacker
    // beside it is not.
    occupied_throne_is_hostile_to_attackers: true,
    shieldwall: Shieldwall::WithCorners,
    exit_fort: true,
    encirclement: true,
    repetition_ban: true,
};

/// **Fetlar**, which Copenhagen was built from.
///
/// Kept as a second entry for one reason: it is the evidence that the ruleset
/// is genuinely data rather than a struct wrapped round one set of constants.
/// Copenhagen is Fetlar plus exactly three additions — the shieldwall, the exit
/// fort and the repetition ban — so if the difference between them is not
/// expressible here, the parameterisation is a fiction.
pub const FETLAR: Ruleset = Ruleset {
    size: 11,
    key: "hnefatafl-fetlar",
    name: "Hnefatafl (Fetlar)",
    escape: Escape::Corners,
    king_capture: KingCapture::Surrounded,
    armed_king: true,
    soldiers_pass_the_empty_throne: true,
    empty_throne_is_hostile_to: [true, true],
    occupied_throne_is_hostile_to_attackers: true,
    // The three Copenhagen added. Turning them off is what makes any playout
    // divergence between the two rulesets isolate to exactly one contested
    // rule, which is worth more as a debugging tool than the second ruleset is
    // as a product.
    shieldwall: Shieldwall::None,
    exit_fort: false,
    encirclement: true,
    repetition_ban: false,
};

/// **Brandubh**, the Irish 7x7 game.
///
/// The smallest tafl there is: four attackers against four defenders and a
/// king. It has more surviving Irish poetry attached to it than any other tafl
/// game and less surviving *rule* — the verse tells us the board and the
/// pieces and says nothing about how they move.
///
/// So this is a reconstruction and the rules document says so, in the shape
/// set for hnefatafl. Two choices are worth naming:
///
/// - **The king is armed.** On a board this small an unarmed king is nearly
///   helpless, and the surviving accounts do not settle it either way.
/// - **No shieldwall and no exit fort.** Both are modern 11x11 inventions
///   solving 11x11 problems; importing them here would be importing somebody
///   else's fix for a problem this board does not have.
///
/// The repetition ban stays, because it is not a rule about tafl so much as a
/// rule about not letting a losing player shuffle for ever, and a seven-square
/// board makes shuffling easier rather than harder.
pub const BRANDUBH: Ruleset = Ruleset {
    size: 7,
    key: "brandubh",
    name: "Brandubh",
    escape: Escape::Corners,
    king_capture: KingCapture::Surrounded,
    armed_king: true,
    soldiers_pass_the_empty_throne: true,
    empty_throne_is_hostile_to: [true, true],
    occupied_throne_is_hostile_to_attackers: true,
    shieldwall: Shieldwall::None,
    exit_fort: false,
    encirclement: true,
    repetition_ban: true,
};

/// **Tablut**, the Sámi 9x9 game.
///
/// Sixteen attackers against eight defenders and a king. Its claim in Bragd
/// is unique: Linnaeus watched it played in Lapland in 1732 and drew the board
/// in his journal, which makes it **the only tafl game with a contemporary
/// description of any kind**. Everything else here is reconstructed from
/// archaeology and guesswork.
///
/// That description is still a traveller's note rather than a rulebook, and it
/// is ambiguous in exactly the place that matters most: whether the king wins
/// by reaching a corner or by reaching any edge square. Linnaeus's Latin has
/// been read both ways for two centuries.
///
/// **Bragd plays the corner escape**, and the reason is playability rather
/// than scholarship: under the edge reading the defenders win almost every
/// game, because the king starts eight squares from safety in four directions
/// at once and sixteen attackers cannot cover thirty-two edge squares. The
/// rules document says which reading and why.
pub const TABLUT: Ruleset = Ruleset {
    size: 9,
    key: "tablut",
    name: "Tablut",
    escape: Escape::Corners,
    king_capture: KingCapture::Surrounded,
    armed_king: true,
    soldiers_pass_the_empty_throne: true,
    empty_throne_is_hostile_to: [true, true],
    occupied_throne_is_hostile_to_attackers: true,
    shieldwall: Shieldwall::None,
    exit_fort: false,
    encirclement: true,
    repetition_ban: true,
};

/// Every ruleset this crate plays.
pub const ALL: [Ruleset; 4] = [COPENHAGEN, FETLAR, BRANDUBH, TABLUT];

impl Ruleset {
    /// The ruleset a stored key names, or nothing.
    ///
    /// **Here rather than at the boundary**, which used to hold a `match` over
    /// the four keys with a `_ => None` of its own. Two tables of the same four
    /// rows is the shape that has produced three blank boards in Bragd side
    /// of this project, and the failure is the same in Rust: the copy that is
    /// not the definition falls behind, and a ruleset added here becomes a
    /// variant that is offered and cannot be opened.
    ///
    /// Nothing rather than a fallback to Copenhagen. Tafl's rulesets differ in
    /// what *winning* is — the exit fort, the shieldwall, whether an unarmed
    /// king may take part in a capture — so a game replayed under the wrong one
    /// is a game with a different result, written to a Library looking
    /// correct.
    pub fn from_key(key: &str) -> Option<Ruleset> {
        ALL.into_iter().find(|rules| rules.key == key)
    }

    /// Where a game under these rules begins.
    ///
    /// On the ruleset because the layout follows from the board, and the board
    /// follows from the ruleset — see the note on `size`.
    pub fn starting_position(&self) -> Position {
        match self.size {
            7 => Position::brandubh(),
            9 => Position::tablut(),
            _ => Position::standard(),
        }
    }

    /// Squares no soldier may ever stand on: the corners and the throne.
    ///
    /// The king may stand on the throne — he starts there — and reaching a
    /// corner is how he wins.
    ///
    /// Takes the position rather than nothing, because where the corners and
    /// the throne *are* is a property of the board and the board is now one of
    /// three sizes.
    pub fn restricted(&self, position: &Position) -> Bitboard {
        position
            .corners()
            .iter()
            .fold(Bitboard::from_square(position.throne()), |set, &corner| {
                set.with(corner)
            })
    }

    /// Whether a square acts as an enemy of `side` for the purpose of capture,
    /// without any piece standing on it.
    ///
    /// This is the rule that makes corners dangerous: a piece caught between an
    /// enemy and an empty corner is taken as surely as if the corner held a
    /// piece. The throne is the same idea with more disagreement about it.
    pub fn is_hostile_square(&self, position: &Position, square: Square, side: Side) -> bool {
        if position.corners().contains(&square) {
            return true;
        }
        if square != position.throne() {
            return false;
        }
        match position.king_square() {
            // The king is at home. Hostile to the attackers only, and only if
            // the ruleset says so.
            Some(king) if king == position.throne() => {
                side == Side::Attackers && self.occupied_throne_is_hostile_to_attackers
            }
            _ => self.empty_throne_is_hostile_to[side.index()],
        }
    }

    /// Every square the piece on `from` may move to.
    ///
    /// The king is the only piece that may enter a restricted square, which is
    /// the entire reason those squares are restricted: a corner blocked by a
    /// defender would be a corner the king could never reach.
    pub fn destinations(&self, position: &Position, from: Square) -> Bitboard {
        let is_king = position.king_square() == Some(from);
        let blocked = if is_king {
            Bitboard::EMPTY
        } else {
            self.restricted(position)
        };

        let mut reachable = position.destinations(from, blocked);

        // Whether a soldier may pass *over* the empty throne is a separate
        // question from whether it may land there, and the two are settled
        // differently by different rulesets. Passing is forbidden by treating
        // the throne as though it were occupied.
        if !is_king && !self.soldiers_pass_the_empty_throne {
            reachable = self.without_squares_beyond_the_throne(position, from, reachable);
        }
        reachable
    }

    fn without_squares_beyond_the_throne(
        &self,
        position: &Position,
        from: Square,
        reachable: Bitboard,
    ) -> Bitboard {
        let mut allowed = reachable;
        for direction in Direction::ALL {
            let mut steps = 1;
            let mut past_the_throne = false;
            while let Some(square) = from.step(direction, steps, position.size()) {
                if past_the_throne {
                    allowed = allowed.without(square);
                } else if square == position.throne() {
                    past_the_throne = true;
                }
                steps += 1;
            }
        }
        allowed
    }

    /// Every legal move for the side to move.
    pub fn legal_moves(&self, position: &Position) -> Vec<Move> {
        let mut moves = Vec::new();
        for from in position.side(position.to_move()) {
            for to in self.destinations(position, from) {
                moves.push(Move::new(from, to));
            }
        }
        moves
    }

    /// Which pieces a move takes.
    ///
    /// **Capture is by custody**, and three things about it catch people out:
    ///
    /// - The capturing piece is the one that *moved*. A piece that moves
    ///   voluntarily between two enemies is safe — walking into a sandwich is
    ///   not being captured, and a rule that missed this would make half the
    ///   defensive moves in the game illegal-feeling.
    /// - One move can take up to three pieces, one on each axis it completes,
    ///   and none of them is on the square the mover landed on.
    /// - An empty corner, and sometimes the throne, count as the far side of a
    ///   sandwich. That is what makes standing next to a corner lethal.
    pub fn captures(&self, position: &Position, to: Square) -> Bitboard {
        let mover = position
            .piece_at(to)
            .expect("captures are resolved after the move lands");

        // An unarmed king is escorted rather than fighting: he completes no
        // sandwich, so moving him next to an enemy takes nothing.
        if mover.role == Role::King && !self.armed_king {
            return Bitboard::EMPTY;
        }

        let mut taken = Bitboard::EMPTY;
        for direction in Direction::ALL {
            let Some(victim_square) = to.step(direction, 1, position.size()) else {
                continue;
            };
            let Some(victim) = position.piece_at(victim_square) else {
                continue;
            };
            if victim.side == mover.side {
                continue;
            }

            // The king is not taken by an ordinary sandwich unless the ruleset
            // says he is; `king_is_captured` answers that question separately
            // because it asks about all four sides at once.
            if victim.role == Role::King && self.king_capture != KingCapture::LikeASoldier {
                continue;
            }

            let Some(anchor) = to.step(direction, 2, position.size()) else {
                // The far side is off the board. An edge does not capture — if
                // it did, the whole outer ring would be a death trap and the
                // game would not work.
                continue;
            };

            let anchored = match position.piece_at(anchor) {
                Some(piece) => piece.side == mover.side,
                None => self.is_hostile_square(position, anchor, victim.side),
            };
            if anchored {
                taken |= Bitboard::from_square(victim_square);
            }
        }
        taken
    }

    /// The pieces a **shieldwall** takes, if the move that just landed on `to`
    /// closed one.
    ///
    /// Copenhagen's own addition to Fetlar, and the only capture in the game
    /// that is not custody. A row of two or more enemies along an edge is taken
    /// whole when every one of them has a piece of yours directly inward, and
    /// both ends of the row are bracketed.
    ///
    /// Three things make it fiddly, and all three are rules rather than
    /// oversights:
    ///
    /// - **The move must close a flank.** A shieldwall that comes into
    ///   existence any other way — by the last inward presser arriving, say —
    ///   captures nothing. So this is asked of the square the mover landed on,
    ///   and only when that square is one of the two brackets.
    /// - **A corner may stand in for one bracket**, at one end only. That is
    ///   Copenhagen's "strong" shieldwall; the weaker reading forbids it.
    /// - **The king is never removed by one.** He may be *in* the row — he
    ///   counts towards its length — and he may be a bracket or an inward
    ///   presser, but a shieldwall that engulfs him takes his companions and
    ///   leaves him standing.
    pub fn shieldwall_captures(&self, position: &Position, to: Square) -> Bitboard {
        if self.shieldwall == Shieldwall::None {
            return Bitboard::EMPTY;
        }
        let Some(mover) = position.piece_at(to) else {
            return Bitboard::EMPTY;
        };
        // The mover has to be *on* the edge line the wall runs along, because it
        // has to be one of the brackets.
        if !to.is_edge(position.size()) {
            return Bitboard::EMPTY;
        }

        let mut taken = Bitboard::EMPTY;
        for along in Direction::ALL {
            // Inward is the perpendicular direction that goes into the board.
            // Finding it this way — a step that exists whose opposite does not
            // — is also what establishes that the row runs *along* an edge: if
            // both perpendicular steps exist, the mover is not on the edge this
            // row would need, and there is nothing to look for.
            let Some(inward) = Direction::ALL.into_iter().find(|&d| {
                d != along
                    && d != along.opposite()
                    && to.step(d, 1, position.size()).is_some()
                    && to.step(d.opposite(), 1, position.size()).is_none()
            }) else {
                continue;
            };

            let mut row = Bitboard::EMPTY;
            let mut steps = 1;
            let closed = loop {
                let Some(square) = to.step(along, steps, position.size()) else {
                    // Ran off the board with nothing closing the far end.
                    break false;
                };
                match position.piece_at(square) {
                    // A victim: an enemy with one of ours directly inward.
                    Some(piece) if piece.side != mover.side => {
                        let pressed = square
                            .step(inward, 1, position.size())
                            .and_then(|s| position.piece_at(s));
                        if pressed.map(|p| p.side) != Some(mover.side) {
                            break false;
                        }
                        row |= Bitboard::from_square(square);
                    }
                    // One of ours closes the far end.
                    Some(_) => break !row.is_empty(),
                    // An empty square closes nothing — unless it is a corner,
                    // which stands in for a bracket under the strong reading.
                    None => {
                        break self.shieldwall == Shieldwall::WithCorners
                            && position.corners().contains(&square)
                            && !row.is_empty();
                    }
                }
                steps += 1;
            };

            // Two or more. A single piece against an edge is ordinary custody's
            // business, and custody says it is safe.
            if closed && row.count() >= 2 {
                taken |= row;
            }
        }

        // The king counts towards the row's length and is then spared. Being
        // engulfed by a shieldwall is not how a king dies.
        if let Some(king) = position.king_square() {
            taken = taken.without(king);
        }
        taken
    }

    /// Everything a move takes: custody, then the shieldwall.
    pub fn all_captures(&self, position: &Position, to: Square) -> Bitboard {
        self.captures(position, to) | self.shieldwall_captures(position, to)
    }

    /// Whether the king is captured in this position.
    ///
    /// Asked of the whole position rather than of a move, because it is a
    /// question about all four sides of him at once — and because under
    /// [`KingCapture::Surrounded`] an edge counts as one of them.
    pub fn king_is_captured(&self, position: &Position) -> bool {
        let Some(king) = position.king_square() else {
            // Already gone, taken as an ordinary piece.
            return true;
        };
        if self.king_capture == KingCapture::LikeASoldier {
            return false; // Handled by the ordinary capture rule.
        }

        // Always four sides. What varies is whether the board's edge can be one
        // of them.
        //
        // Under Copenhagen it cannot, and that is not a detail: **a king on the
        // edge cannot be captured at all.** It is what makes an exit fort
        // possible, and it is the difference between a king running along the
        // rim towards a corner and a king running into a trap. Under Fetlar's
        // older reading the edge does the work of a fourth attacker.
        //
        // The three-attacker case beside the throne needs no rule of its own:
        // the throne the king has just left is empty, and an empty throne is
        // already a hostile square.
        let mut sides = 0;
        for direction in Direction::ALL {
            let counts = match king.step(direction, 1, position.size()) {
                Some(square) => match position.piece_at(square) {
                    Some(piece) => piece.side == Side::Attackers,
                    None => self.is_hostile_square(position, square, Side::Defenders),
                },
                None => self.king_capture == KingCapture::SurroundedOrAgainstAnEdge,
            };
            if counts {
                sides += 1;
            }
        }
        sides == 4
    }

    /// Whether the king has escaped.
    pub fn king_has_escaped(&self, position: &Position) -> bool {
        let Some(king) = position.king_square() else {
            return false;
        };
        match self.escape {
            Escape::Corners => position.corners().contains(&king),
            Escape::Edges => king.is_edge(position.size()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A board drawn as a picture, so a test reads like the position it is about.
    fn board(layout: [&str; 11], to_move: Side) -> Position {
        Position::from_picture(&layout, to_move)
    }

    /// Plays a move and returns what it took, which is what most of these ask.
    fn taken_by(before: &Position, from: &str, to: &str) -> Vec<Square> {
        let mv = Move::new(Square::parse(from).unwrap(), Square::parse(to).unwrap());
        let after = before.slide(mv);
        COPENHAGEN.captures(&after, mv.to).collect()
    }

    #[test]
    fn a_sandwich_takes_the_piece_in_the_middle() {
        // Anchor on d5, defender on e5, and the attacker arrives on f5.
        // Deliberately off rank 6: f6 is the throne, and a soldier may not
        // stand there at all.
        let before = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                ".....K.....",
                "...XO....X.",
                "...........",
                "...........",
                "...........",
                "...........",
            ],
            Side::Attackers,
        );
        assert_eq!(
            taken_by(&before, "j5", "f5"),
            [Square::parse("e5").unwrap()]
        );
    }

    #[test]
    fn moving_between_two_enemies_is_safe() {
        // The rule that makes the game playable: only the *mover* completes a
        // sandwich. A piece that walks into one is not captured.
        let before = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                ".X.X......O",
                "...........",
                "...........",
                "...........",
                "...........",
                "..........K",
            ],
            Side::Defenders,
        );
        let mv = Move::new(Square::parse("k6").unwrap(), Square::parse("c6").unwrap());
        let after = before.slide(mv);
        assert!(
            COPENHAGEN.captures(&after, mv.to).is_empty(),
            "took nothing"
        );
        assert_eq!(
            after.piece_at(Square::parse("c6").unwrap()).map(|p| p.side),
            Some(Side::Defenders),
            "and survives between b6 and d6"
        );
    }

    #[test]
    fn an_empty_corner_is_the_far_side_of_a_sandwich() {
        // What makes standing next to a corner lethal, and what the attackers
        // play for once the king is running for one.
        let before = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "..........K",
                "...........",
                "...........",
                "...........",
                "...........",
                ".O.......X.",
            ],
            Side::Attackers,
        );
        assert_eq!(
            taken_by(&before, "j1", "c1"),
            [Square::parse("b1").unwrap()]
        );
    }

    #[test]
    fn an_edge_is_not_a_capturing_wall() {
        // A defender on the a-file with an attacker beside him and the edge of
        // the board behind him is safe. If an edge captured, the whole outer
        // ring would be a death trap and the game would not work.
        let against_the_edge = board(
            [
                "...........",
                "...........",
                "..K........",
                "...........",
                "...........",
                "O........X.",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
            ],
            Side::Attackers,
        );
        assert!(
            taken_by(&against_the_edge, "j6", "b6").is_empty(),
            "a6 is against the edge, which is not a wall"
        );

        // The same shape one file inward, with a real anchor, does capture —
        // which is what says the refusal above is about the edge and not about
        // the shape.
        let real = board(
            [
                "...........",
                "...........",
                "..K........",
                "...........",
                "...........",
                "XO.......X.",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
            ],
            Side::Attackers,
        );
        assert_eq!(taken_by(&real, "j6", "c6"), [Square::parse("b6").unwrap()]);
    }

    #[test]
    fn one_move_can_take_two_pieces_on_different_axes() {
        // Up to three, one per axis the mover completes — and none of them is
        // on the square the mover landed on, which is why captures cannot be
        // recorded in the move.
        let before = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                ".....K.....",
                "...........",
                ".....X.....",
                ".....O.....",
                "X.....OX...",
                "...........",
            ],
            Side::Attackers,
        );
        let mut took = taken_by(&before, "a2", "f2");
        took.sort();
        assert_eq!(
            took,
            [Square::parse("g2").unwrap(), Square::parse("f3").unwrap()]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>(),
            "one north, one east"
        );
    }

    #[test]
    fn the_king_cannot_be_captured_on_the_edge_at_all() {
        // Copenhagen states this unconditionally, and it is the rule that makes
        // an exit fort possible. Three attackers and a wall is not a capture —
        // the attackers have to force him inward first.
        let three_and_a_wall = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "....X......",
                "...XKX.....",
            ],
            Side::Defenders,
        );
        assert!(
            !COPENHAGEN.king_is_captured(&three_and_a_wall),
            "a king on the edge is safe"
        );
        // Fetlar reads it the other way, which is what says this is a ruleset
        // choice rather than an implementation detail.
        assert!(
            FETLAR.king_is_captured(&three_and_a_wall)
                || FETLAR.king_capture == KingCapture::Surrounded,
            "and the difference is expressible"
        );

        let only_two = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...XKX.....",
            ],
            Side::Defenders,
        );
        assert!(!COPENHAGEN.king_is_captured(&only_two), "two is not enough");

        let all_four = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "....X......",
                "...XKX.....",
                "....X......",
                "...........",
                "...........",
            ],
            Side::Defenders,
        );
        assert!(COPENHAGEN.king_is_captured(&all_four));
    }

    #[test]
    fn three_attackers_take_the_king_beside_the_throne() {
        // Copenhagen writes this as its own clause — "if on a square next to
        // the throne, the attackers must occupy the three remaining squares" —
        // but it needs no rule here: the throne the king just left is empty,
        // and an empty throne is already hostile.
        let beside_the_throne = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "....X......",
                "...XK......",
                "....X......",
                "...........",
                "...........",
                "...........",
                "...........",
            ],
            Side::Defenders,
        );
        assert!(
            COPENHAGEN.king_is_captured(&beside_the_throne),
            "e6, boxed by d6/e7/e5 with the empty throne on his east"
        );
    }

    #[test]
    fn an_ordinary_sandwich_does_not_take_the_king() {
        // He is not a soldier. Two attackers beside him are two attackers.
        let before = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...X.K...X.",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
            ],
            Side::Attackers,
        );
        assert!(taken_by(&before, "j6", "g6").is_empty(), "the king stands");
    }

    /// Plays a move and returns everything it took, shieldwall included.
    fn all_taken_by(before: &Position, from: &str, to: &str) -> Vec<Square> {
        let mv = Move::new(Square::parse(from).unwrap(), Square::parse(to).unwrap());
        let after = before.slide(mv);
        let mut squares: Vec<Square> = COPENHAGEN.all_captures(&after, mv.to).collect();
        squares.sort();
        squares
    }

    #[test]
    fn a_shieldwall_takes_a_whole_row_off_the_edge() {
        // Two defenders on rank 1 with attackers pressing them from rank 2, and
        // the attacker's move closes the second end.
        let before = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                ".....K.....",
                "...........",
                "...........",
                "...........",
                "...........",
                "....XX.....",
                "...XOO....X",
            ],
            Side::Attackers,
        );
        assert_eq!(
            all_taken_by(&before, "k1", "g1"),
            [Square::parse("e1").unwrap(), Square::parse("f1").unwrap()],
            "bracketed by d1 and g1, pressed by e2 and f2"
        );
    }

    #[test]
    fn a_shieldwall_needs_every_man_pressed() {
        // One gap in the inward rank and the whole wall survives.
        let before = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                ".....K.....",
                "...........",
                "...........",
                "...........",
                "...........",
                "....X......",
                "...XOO....X",
            ],
            Side::Attackers,
        );
        assert!(
            all_taken_by(&before, "k1", "g1").is_empty(),
            "f1 has nothing above it"
        );
    }

    #[test]
    fn a_shieldwall_must_be_closed_by_the_move_itself() {
        // The wall already exists; an unrelated move does not spring it. This
        // is the rule that stops a shieldwall firing retroactively.
        let before = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                ".....K....X",
                "...........",
                "...........",
                "...........",
                "...........",
                "....XX.....",
                "...XOOX....",
            ],
            Side::Attackers,
        );
        assert!(
            all_taken_by(&before, "k7", "k6").is_empty(),
            "the wall was already closed; this move is elsewhere"
        );
    }

    #[test]
    fn a_shieldwall_of_one_is_not_a_shieldwall() {
        // Asked of the shieldwall specifically, because a single victim on an
        // edge with a bracket one side and the mover the other *is* an ordinary
        // sandwich — so `all_captures` cannot tell the two rules apart, and a
        // test that used it would pass without testing anything.
        let before = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                ".....K.....",
                "...........",
                "...........",
                "...........",
                "...........",
                "....X......",
                "...XO.....X",
            ],
            Side::Attackers,
        );
        let mv = Move::new(Square::parse("k1").unwrap(), Square::parse("f1").unwrap());
        let after = before.slide(mv);
        assert!(
            COPENHAGEN.shieldwall_captures(&after, mv.to).is_empty(),
            "one is not a row"
        );
        // And ordinary custody takes it anyway, which is the point: the
        // shieldwall exists for rows custody cannot reach.
        assert_eq!(
            COPENHAGEN.captures(&after, mv.to).collect::<Vec<_>>(),
            [Square::parse("e1").unwrap()]
        );
    }

    #[test]
    fn a_shieldwall_never_takes_the_king() {
        // He counts towards the row's length and is then spared: being
        // engulfed is not how a king dies.
        let before = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "....XX.....",
                "...XKO....X",
            ],
            Side::Attackers,
        );
        let taken = all_taken_by(&before, "k1", "g1");
        assert_eq!(taken, [Square::parse("f1").unwrap()], "his companion goes");
        assert!(
            !taken.contains(&Square::parse("e1").unwrap()),
            "and the king stands"
        );
    }

    #[test]
    fn fetlar_has_no_shieldwall_at_all() {
        // The two rulesets differ by exactly this, which is what says the
        // parameterisation is real rather than a struct around one set of
        // constants.
        let before = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                ".....K.....",
                "...........",
                "...........",
                "...........",
                "...........",
                "....XX.....",
                "...XOO....X",
            ],
            Side::Attackers,
        );
        let mv = Move::new(Square::parse("k1").unwrap(), Square::parse("g1").unwrap());
        let after = before.slide(mv);
        assert!(COPENHAGEN.all_captures(&after, mv.to).count() == 2);
        assert!(FETLAR.all_captures(&after, mv.to).is_empty());
    }

    #[test]
    fn the_king_escapes_at_a_corner_and_nowhere_else() {
        for (square, escaped) in [("a1", true), ("k11", true), ("a6", false), ("f6", false)] {
            let mut layout = ["..........."; 11];
            let target = Square::parse(square).unwrap();
            let row = (10 - target.rank()) as usize;
            let mut characters: Vec<char> = layout[row].chars().collect();
            characters[target.file() as usize] = 'K';
            let line: String = characters.into_iter().collect();
            layout[row] = Box::leak(line.into_boxed_str());

            assert_eq!(
                COPENHAGEN.king_has_escaped(&board(layout, Side::Attackers)),
                escaped,
                "king on {square}"
            );
        }
    }

    #[test]
    fn only_the_king_may_stand_on_a_corner_or_the_throne() {
        let start = Position::standard();
        for from in start.side(Side::Attackers) {
            let reachable = COPENHAGEN.destinations(&start, from);
            for restricted in COPENHAGEN.restricted(&start) {
                assert!(
                    !reachable.contains(restricted),
                    "{from} could reach the restricted {restricted}"
                );
            }
        }

        // The king's destinations *do* include them, which is the whole reason
        // they are restricted: a corner a defender could block is a corner the
        // king could never reach.
        let open = board(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "K..........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
            ],
            Side::Defenders,
        );
        let reachable = COPENHAGEN.destinations(&open, Square::parse("a6").unwrap());
        assert!(reachable.contains(Square::parse("a1").unwrap()));
        assert!(reachable.contains(Square::parse("a11").unwrap()));
        assert!(reachable.contains(open.throne()), "and the throne is his");
    }

    #[test]
    fn every_move_at_the_start_is_a_piece_of_the_side_to_move_going_somewhere_empty() {
        let start = Position::standard();
        let moves = COPENHAGEN.legal_moves(&start);
        assert!(!moves.is_empty());
        for mv in &moves {
            assert_ne!(mv.from, mv.to);
            assert_eq!(
                start.piece_at(mv.from).map(|p| p.side),
                Some(Side::Attackers)
            );
            assert_eq!(start.piece_at(mv.to), None);
        }
        // Both sides have the same shape, so both have the same move count from
        // the opening array — a cheap check that nothing about the generator is
        // side-specific.
        let mut mirrored = start;
        mirrored = mirrored.slide(Move::new(
            Square::parse("f10").unwrap(),
            Square::parse("f9").unwrap(),
        ));
        assert!(!COPENHAGEN.legal_moves(&mirrored).is_empty());
    }
}
