//! The position, and the rules for moving from it.
//!
//! A position is fourteen counters' worth of information and nothing else: where
//! each side's seven pieces stand, and whose throw it is. There is no history in
//! it, no throw in it, and no outcome in it — a throw belongs to the record and
//! an outcome is read off the board.

use crate::board::{Rules, Side, Spot};
use crate::dice::Throw;

/// One legal move: a piece at `from` goes to `to`.
///
/// `from` and `to` are **steps along the mover's own path**, not board squares,
/// because that is the only spelling in which a move is unambiguous — both sides
/// have a step 8 and they are the same square, and both have a step 1 and they
/// are not.
///
/// **A move is its two ends and nothing else.** Whether it captures, and whether
/// it earns another throw, are facts about the position it is played from rather
/// than about the move — so they live on [`Offer`], which is what
/// [`Position::moves_for`] hands out, and never in the record. An earlier version
/// put them here and the property suite found it within a minute: the wire
/// carries two numbers, so a record read back had them both `false` and no longer
/// equalled the game it came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Move {
    pub from: Spot,
    pub to: Spot,
}

/// A legal move, with what it would do.
///
/// This is what a player is shown and what a board draws from. It is deliberately
/// not what is stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Offer {
    pub chosen: Move,
    /// Whether this move sends an enemy piece back to the start.
    pub captures: bool,
    /// Whether this move earns another throw: it lands on a rosette, or (where the
    /// rule set says so) it captures.
    pub again: bool,
}

impl Move {
    /// The wire spelling: the two step numbers, separated by a dash.
    ///
    /// `0` means waiting to enter and one past the path length means borne off,
    /// which are the two ends of [`Rules::distance`]. Spelling them as numbers rather than as words
    /// keeps the parser total — every entry is two integers and a dash, whatever
    /// it means.
    pub fn to_wire(self, rules: &Rules) -> String {
        format!("{}-{}", rules.distance(self.from), rules.distance(self.to))
    }

    /// Reads a move back. Does not check that it is legal — that is
    /// [`Position::moves_for`]'s job, and a parser that also judged legality
    /// would have to be given a position and would then be two things.
    pub fn parse(text: &str) -> Option<(u8, u8)> {
        let (from, to) = text.split_once('-')?;
        // Rejecting a leading `+` or whitespace, which `u8::from_str` would take.
        if !from.bytes().all(|b| b.is_ascii_digit()) || !to.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if from.is_empty() || to.is_empty() {
            return None;
        }
        Some((from.parse().ok()?, to.parse().ok()?))
    }
}

/// The whole board.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Position {
    /// The rule set this position is played under.
    rules: &'static Rules,
    /// Where each side's pieces are, in no particular order — pieces are
    /// interchangeable, so this is a multiset that happens to be stored as a list.
    white: Vec<Spot>,
    black: Vec<Spot>,
    to_move: Side,
}

impl Position {
    /// The opening position: every piece waiting, and the given side to throw.
    ///
    /// **The engine does not decide who starts.** Ur has no opening throw that
    /// settles it — the players agree, or a coin does — so seating is a fact
    /// about the Game rather than about the position, exactly as backgammon's is.
    pub fn starting_with(rules: &'static Rules, side: Side) -> Position {
        Position {
            rules,
            white: vec![Spot::Waiting; rules.pieces as usize],
            black: vec![Spot::Waiting; rules.pieces as usize],
            to_move: side,
        }
    }

    /// The rule set this position is played under.
    pub fn rules(&self) -> &'static Rules {
        self.rules
    }

    /// Puts each side's pieces back into their canonical order.
    ///
    /// Pieces are interchangeable, so the array is a multiset and its order
    /// carries no information. Keeping it sorted is what makes two positions that
    /// differ only in which slot holds which piece compare equal — which the
    /// replay property depends on, and which is how this function came to exist.
    fn normalise(&mut self) {
        self.white.sort_unstable();
        self.black.sort_unstable();
    }

    /// Whose throw it is.
    pub fn to_move(&self) -> Side {
        self.to_move
    }

    /// A side's pieces, as spots.
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

    /// How many of a side's pieces are waiting to enter.
    pub fn waiting(&self, side: Side) -> u8 {
        self.pieces(side)
            .iter()
            .filter(|s| **s == Spot::Waiting)
            .count() as u8
    }

    /// How many of a side's pieces are home.
    pub fn off(&self, side: Side) -> u8 {
        self.pieces(side)
            .iter()
            .filter(|s| **s == Spot::Off)
            .count() as u8
    }

    /// Which side, if any, has a piece on this board square.
    pub fn occupant(&self, square: usize) -> Option<Side> {
        [Side::White, Side::Black].into_iter().find(|&side| {
            self.pieces(side)
                .iter()
                .any(|&spot| self.rules.square_at(side, spot) == Some(square))
        })
    }

    /// Whether this side already stands on this step of its own path.
    fn own_piece_at(&self, side: Side, spot: Spot) -> bool {
        matches!(spot, Spot::Step(_)) && self.pieces(side).contains(&spot)
    }

    /// The legal moves for a throw.
    ///
    /// **An empty answer is a legal outcome, not an error.** A throw of nought
    /// moves nothing by definition, and a throw of two with every landing square
    /// blocked moves nothing either. Both pass the turn, and the caller must be
    /// able to tell them apart from "you have not thrown yet" — which is what
    /// [`crate::game::Game::needs`] is for.
    pub fn moves_for(&self, throw: Throw) -> Vec<Offer> {
        let rules = self.rules;
        let steps = rules.steps(throw);
        if steps == 0 || throw.marks() > rules.dice || self.winner().is_some() {
            return Vec::new();
        }
        let side = self.to_move;
        let mut moves: Vec<Offer> = Vec::new();

        for &from in self.pieces(side) {
            if from == Spot::Off {
                continue;
            }
            // **Bearing off needs the exact number.** A piece two steps from home
            // is not brought home by a throw of three. Finkel's reading, and the
            // one the rules document names: it is what makes the last stretch a real
            // obstacle rather than a formality, and the alternative — any throw
            // that reaches or passes the end — would make the final rosette's
            // extra throw almost always winning.
            let Some(to) = rules.advance(from, steps) else {
                continue;
            };

            // Your own piece is in the way. Ur has no stacking: a square holds one
            // piece or none.
            if self.own_piece_at(side, to) {
                continue;
            }

            let mut captures = false;
            if let Some(square) = rules.square_at(side, to)
                && let Some(other) = self.occupant(square) {
                    if other == side {
                        // Unreachable — `own_piece_at` caught it — but a square
                        // occupied by a side that is not the enemy is not a
                        // capture, and saying so here means this stays correct if
                        // the guard above ever moves.
                        continue;
                    }
                    // **A piece can only be taken on a shared square, and never on
                    // a safe rosette.** Off the shared squares the two sides never
                    // meet; and where the rule set protects rosettes, a shared
                    // rosette is the square the rules protect.
                    if !rules.is_shared(square) || rules.is_safe(square) {
                        continue;
                    }
                    captures = true;
                }

            let again = self.earns_again(side, to, captures);
            moves.push(Offer {
                chosen: Move { from, to },
                captures,
                again,
            });
        }

        // Two pieces waiting to enter are the same piece, and two pieces on the
        // same step cannot happen. Deduplicating by the pair of ends is therefore
        // exactly deduplicating identical moves, and it keeps `moves_for` from
        // offering the player seven ways to make one move at the start.
        moves.sort_by_key(|offer| (offer.chosen.from, offer.chosen.to));
        moves.dedup_by_key(|offer| (offer.chosen.from, offer.chosen.to));
        moves
    }

    /// Whether landing at `to` earns another throw: a rosette, or a capture where
    /// the rule set rewards one.
    fn earns_again(&self, side: Side, to: Spot, captures: bool) -> bool {
        let rosette = self.rules.square_at(side, to).is_some_and(|s| self.rules.is_rosette(s));
        rosette || (captures && self.rules.capture_again)
    }

    /// Plays a move that [`Self::moves_for`] offered.
    ///
    /// Returns whether the mover throws again. The caller is trusted to have
    /// taken the move from `moves_for` — [`crate::game::Game`] is the layer that
    /// checks, so that the check happens once and in the place that also owns the
    /// record.
    pub fn play(&mut self, chosen: Move) -> bool {
        let side = self.to_move;
        let rules = self.rules;
        let mut captures = false;

        if let Some(square) = rules.square_at(side, chosen.to)
            && self.occupant(square) == Some(side.other()) {
                let enemy = side.other();
                captures = true;
                for spot in self.pieces_mut(enemy) {
                    if rules.square_at(enemy, *spot) == Some(square) {
                        *spot = Spot::Waiting;
                        break;
                    }
                }
            }

        // The first piece standing at `from` — they are interchangeable, so any
        // of them is the right one, and for `Waiting` there may be several.
        for spot in self.pieces_mut(side) {
            if *spot == chosen.from {
                *spot = chosen.to;
                break;
            }
        }

        // **Worked out here rather than taken from the caller.** Whether the turn
        // continues is a fact about the square landed on, and a caller that
        // passed in its own answer would be a second source of truth for it —
        // which is the bug a replayed record would introduce, since the record
        // spells only the two ends.
        let again = self.earns_again(side, chosen.to, captures);
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
            .find(|&side| self.off(side) == self.rules.pieces)
    }

    /// How many steps a side still has to travel — the pip count, and the one
    /// number that says who is ahead.
    pub fn distance_left(&self, side: Side) -> u16 {
        self.pieces(side)
            .iter()
            .map(|&spot| (self.rules.path_len() as u16 + 1) - self.rules.distance(spot) as u16)
            .sum()
    }

    /// The position as a string, in this family's own notation.
    ///
    /// Twenty characters for the board, then the two waiting counts, then the two
    /// borne-off counts, then the side to move. **Ur has no standard notation and
    /// inventing one is unavoidable**, so this one is at least readable: `.` is an
    /// empty square, `W` and `B` are pieces, and the squares are in the order
    /// [`crate::board::BOARD`] gives them.
    pub fn to_picture(&self) -> String {
        let squares = self.rules.board.len();
        let mut board = String::with_capacity(squares);
        for square in 0..squares {
            board.push(match self.occupant(square) {
                Some(Side::White) => 'W',
                Some(Side::Black) => 'B',
                None => '.',
            });
        }
        format!(
            "{board} {}{} {}{} {}",
            self.waiting(Side::White),
            self.waiting(Side::Black),
            self.off(Side::White),
            self.off(Side::Black),
            if self.to_move == Side::White {
                'w'
            } else {
                'b'
            }
        )
    }

    /// Reads a position back from its picture.
    ///
    /// Hostile input is expected — this is reached from a wire — so every count is
    /// checked against the board rather than trusted, and a picture whose pieces
    /// do not add up to the rule set's pieces a side is refused rather than repaired.
    pub fn from_picture(rules: &'static Rules, text: &str) -> Option<Position> {
        let mut parts = text.split_whitespace();
        let board = parts.next()?;
        let waiting = parts.next()?;
        let off = parts.next()?;
        let mover = parts.next()?;
        if parts.next().is_some() || board.len() != rules.board.len() {
            return None;
        }

        let mut position = Position {
            rules,
            white: Vec::new(),
            black: Vec::new(),
            to_move: match mover {
                "w" => Side::White,
                "b" => Side::Black,
                _ => return None,
            },
        };

        let digit = |text: &str, at: usize| -> Option<u8> {
            let byte = text.as_bytes().get(at)?;
            byte.is_ascii_digit().then(|| byte - b'0')
        };
        if waiting.len() != 2 || off.len() != 2 {
            return None;
        }
        let waiting = [digit(waiting, 0)?, digit(waiting, 1)?];
        let borne = [digit(off, 0)?, digit(off, 1)?];

        // Two named locals rather than an array indexed by the side, because the
        // only thing that would make `found[side]` work is the enum's
        // discriminant happening to be nought and one, and that is a coincidence
        // of declaration order rather than something this parser knows.
        let mut on_board_white: Vec<Spot> = Vec::new();
        let mut on_board_black: Vec<Spot> = Vec::new();
        for (square, glyph) in board.chars().enumerate() {
            let side = match glyph {
                '.' => continue,
                'W' => Side::White,
                'B' => Side::Black,
                _ => return None,
            };
            // Which step of that side's path this square is. A square not on the
            // side's own path cannot hold its piece, and a picture that says
            // otherwise is refused.
            let step = rules.path(side).iter().position(|&s| s == square)? + 1;
            let spot = Spot::Step(step as u8);
            match side {
                Side::White => on_board_white.push(spot),
                Side::Black => on_board_black.push(spot),
            }
        }

        for (side, mut spots, waiting, borne) in [
            (Side::White, on_board_white, waiting[0], borne[0]),
            (Side::Black, on_board_black, waiting[1], borne[1]),
        ] {
            for _ in 0..waiting {
                spots.push(Spot::Waiting);
            }
            for _ in 0..borne {
                spots.push(Spot::Off);
            }
            if spots.len() != rules.pieces as usize {
                return None;
            }
            match side {
                Side::White => position.white = spots,
                Side::Black => position.black = spots,
            }
        }

        position.normalise();
        Some(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{ALL, ASEB, BLITZ, FINKEL, MASTERS};

    const PIECES: u8 = FINKEL.pieces;
    const SQUARES: usize = 20;
    const SAFE: usize = 9;
    const PATH: usize = 14;
    fn square_at(side: Side, spot: Spot) -> Option<usize> {
        FINKEL.square_at(side, spot)
    }
    fn is_shared(square: usize) -> bool {
        FINKEL.is_shared(square)
    }

    fn throw(marks: u8) -> Throw {
        Throw::new(marks).unwrap()
    }

    #[test]
    fn the_opening_position_has_everything_waiting() {
        let position = Position::starting_with(&FINKEL, Side::White);
        assert_eq!(position.to_move(), Side::White);
        for side in [Side::White, Side::Black] {
            assert_eq!(position.waiting(side), PIECES);
            assert_eq!(position.off(side), 0);
            assert_eq!(
                position.distance_left(side),
                (PIECES as u16) * (PATH as u16 + 1)
            );
        }
        assert_eq!(position.winner(), None);
        for square in 0..SQUARES {
            assert_eq!(position.occupant(square), None);
        }
    }

    #[test]
    fn a_nought_offers_nothing_and_every_other_throw_opens_the_game() {
        let position = Position::starting_with(&FINKEL, Side::White);
        assert!(position.moves_for(throw(0)).is_empty());
        for marks in 1..=4 {
            let moves = position.moves_for(throw(marks));
            assert_eq!(
                moves.len(),
                1,
                "a throw of {marks} should offer exactly one entry from the opening"
            );
            assert_eq!(moves[0].chosen.from, Spot::Waiting);
            assert_eq!(moves[0].chosen.to, Spot::Step(marks));
            assert!(!moves[0].captures);
            assert_eq!(moves[0].again, marks == 4, "only step four is a rosette");
        }
    }

    #[test]
    fn landing_on_a_rosette_throws_again_and_does_not_pass_the_turn() {
        let mut position = Position::starting_with(&FINKEL, Side::White);
        let moves = position.moves_for(throw(4));
        assert_eq!(moves.len(), 1);
        assert!(moves[0].again);
        let again = position.play(moves[0].chosen);
        assert!(again, "a rosette earns another throw");
        assert_eq!(position.to_move(), Side::White, "the mover keeps the turn");
    }

    #[test]
    fn an_ordinary_landing_passes_the_turn() {
        let mut position = Position::starting_with(&FINKEL, Side::White);
        let moves = position.moves_for(throw(1));
        let again = position.play(moves[0].chosen);
        assert!(!again);
        assert_eq!(position.to_move(), Side::Black);
    }

    #[test]
    fn a_piece_may_not_land_on_its_own() {
        let mut position = Position::starting_with(&FINKEL, Side::White);
        // White to step 1, then back to White by playing Black elsewhere.
        position.play(position.moves_for(throw(1))[0].chosen);
        position.pass(); // Black does nothing
        assert_eq!(position.to_move(), Side::White);
        // Entering again with a throw of 1 would land on the occupied step 1.
        let moves = position.moves_for(throw(1));
        assert!(
            !moves
                .iter()
                .any(|m| m.chosen.from == Spot::Waiting && m.chosen.to == Spot::Step(1)),
            "entering onto your own piece should not be offered"
        );
    }

    #[test]
    fn a_capture_is_offered_on_the_shared_row_and_sends_the_piece_back() {
        // Moves are chosen by what they do, never by their index — a throw of one
        // from here offers both entering and advancing, and taking `[0]` silently
        // tested the wrong one.
        let advance = |position: &Position, throw, from, to| {
            position
                .moves_for(throw)
                .into_iter()
                .find(|m| m.chosen.from == from && m.chosen.to == to)
                .unwrap_or_else(|| panic!("{from:?} to {to:?} should be legal"))
        };

        // Black walks a piece to step 5, the first shared square.
        let mut position = Position::starting_with(&FINKEL, Side::Black);
        let entry = advance(&position, throw(4), Spot::Waiting, Spot::Step(4));
        assert!(
            position.play(entry.chosen),
            "step 4 is a rosette and throws again"
        );
        let onward = advance(&position, throw(1), Spot::Step(4), Spot::Step(5));
        assert!(!position.play(onward.chosen));
        assert_eq!(position.to_move(), Side::White);

        let contested = square_at(Side::Black, Spot::Step(5)).unwrap();
        assert_eq!(position.occupant(contested), Some(Side::Black));
        assert!(is_shared(contested), "step 5 should be on the shared row");

        // White reaches the same square. Both sides' step 5 is the same square,
        // which is the only reason a capture is possible at all.
        let entry = advance(&position, throw(4), Spot::Waiting, Spot::Step(4));
        assert!(position.play(entry.chosen));
        assert_eq!(square_at(Side::White, Spot::Step(5)), Some(contested));
        let capture = advance(&position, throw(1), Spot::Step(4), Spot::Step(5));
        assert!(
            capture.captures,
            "landing on an enemy in the shared row is a capture"
        );
        position.play(capture.chosen);
        assert_eq!(
            position.waiting(Side::Black),
            PIECES,
            "the taken piece goes back to waiting"
        );
        assert_eq!(position.occupant(contested), Some(Side::White));
    }

    #[test]
    fn nothing_can_be_taken_on_the_middle_rosette() {
        let mut position = Position::starting_with(&FINKEL, Side::Black);
        position.play(position.moves_for(throw(4))[0].chosen); // Black step 4, again
        position.play(position.moves_for(throw(4))[0].chosen); // Black step 8 — the safe rosette, again
        assert_eq!(square_at(Side::Black, Spot::Step(8)), Some(SAFE));
        position.pass(); // hand the turn over deliberately
        assert_eq!(position.to_move(), Side::White);

        position.play(position.moves_for(throw(4))[0].chosen); // White step 4, again
        let moves = position.moves_for(throw(4));
        assert!(
            !moves.iter().any(|m| m.chosen.to == Spot::Step(8)),
            "the safe square must not be enterable while an enemy holds it"
        );
    }

    #[test]
    fn bearing_off_needs_the_exact_throw() {
        let mut position = Position::starting_with(&FINKEL, Side::White);
        position.white[0] = Spot::Step(PATH as u8);
        for marks in 2..=4 {
            let moves = position.moves_for(throw(marks));
            assert!(
                !moves.iter().any(|m| m.chosen.to == Spot::Off),
                "a throw of {marks} should not bear off from the last step"
            );
        }
        let moves = position.moves_for(throw(1));
        let home = moves
            .iter()
            .find(|m| m.chosen.to == Spot::Off)
            .expect("one exactly bears off");
        position.play(home.chosen);
        assert_eq!(position.off(Side::White), 1);
    }

    #[test]
    fn seven_home_wins_and_nothing_less_does() {
        let mut position = Position::starting_with(&FINKEL, Side::White);
        for index in 0..PIECES as usize - 1 {
            position.white[index] = Spot::Off;
        }
        assert_eq!(position.winner(), None, "six home is not a win");
        position.white[PIECES as usize - 1] = Spot::Off;
        assert_eq!(position.winner(), Some(Side::White));
        assert!(
            position.moves_for(throw(2)).is_empty(),
            "a finished game offers no moves"
        );
    }

    #[test]
    fn a_picture_survives_the_round_trip() {
        let mut position = Position::starting_with(&FINKEL, Side::White);
        assert_eq!(
            Position::from_picture(&FINKEL, &position.to_picture()).as_ref(),
            Some(&position)
        );

        position.play(position.moves_for(throw(4))[0].chosen);
        position.play(position.moves_for(throw(4))[0].chosen);
        position.pass();
        position.play(position.moves_for(throw(2))[0].chosen);
        let picture = position.to_picture();
        assert_eq!(Position::from_picture(&FINKEL, &picture).as_ref(), Some(&position));
        assert_eq!(
            Position::from_picture(&FINKEL, &picture).unwrap().to_picture(),
            picture
        );
    }

    #[test]
    fn a_picture_that_does_not_add_up_is_refused() {
        let good = Position::starting_with(&FINKEL, Side::White).to_picture();
        assert!(Position::from_picture(&FINKEL, &good).is_some());
        for bad in [
            "",
            "....................",
            ".................... 77 00",
            ".................... 77 00 x",
            ".................... 7 00 w",
            "................... 77 00 w",
            "..................... 77 00 w",
            ".................... 77 00 w extra",
            ".................... 66 00 w",
            "W................... 77 00 w",
            "..X................. 77 00 w",
            ".................... aa 00 w",
        ] {
            assert_eq!(
                Position::from_picture(&FINKEL, bad),
                None,
                "{bad:?} should be refused"
            );
        }
        // A White piece cannot stand on a square only Black walks.
        let black_only = {
            let mut glyphs = vec!['.'; SQUARES];
            glyphs[14] = 'W';
            format!("{} 66 00 w", glyphs.into_iter().collect::<String>())
        };
        assert_eq!(
            Position::from_picture(&FINKEL, &black_only),
            None,
            "a side cannot stand off its own path"
        );
    }

    #[test]
    fn a_move_survives_the_wire() {
        let chosen = Move {
            from: Spot::Waiting,
            to: Spot::Step(4),
        };
        assert_eq!(chosen.to_wire(&FINKEL), "0-4");
        assert_eq!(Move::parse("0-4"), Some((0, 4)));
        assert_eq!(Move::parse("14-15"), Some((14, 15)));
        for bad in ["", "-", "4", "4-", "-4", "a-b", "1-2-3", " 1-2", "+1-2"] {
            assert_eq!(Move::parse(bad), None, "{bad:?} should not parse");
        }
    }

    /// A picture with the given squares held: `(square, 'W' or 'B')`.
    fn picture(rules: &'static Rules, held: &[(usize, char)], waiting: [u8; 2], mover: char) -> Position {
        let mut glyphs = vec!['.'; 20];
        for &(square, glyph) in held {
            glyphs[square] = glyph;
        }
        let text = format!("{} {}{} 00 {mover}", glyphs.into_iter().collect::<String>(), waiting[0], waiting[1]);
        Position::from_picture(rules, &text).unwrap_or_else(|| panic!("{text} should read"))
    }

    fn offers(position: &Position, marks: u8) -> Vec<(u8, u8, bool, bool)> {
        let rules = position.rules();
        position
            .moves_for(throw(marks))
            .iter()
            .map(|o| (rules.distance(o.chosen.from), rules.distance(o.chosen.to), o.captures, o.again))
            .collect()
    }

    #[test]
    fn masters_moves_four_on_a_nought_and_three_dice_cannot_throw_four() {
        let position = Position::starting_with(&MASTERS, Side::White);
        assert_eq!(offers(&position, 0), [(0, 4, false, true)], "a nought enters onto the first rosette");
        assert_eq!(offers(&position, 3), [(0, 3, false, false)]);
        assert!(position.moves_for(throw(4)).is_empty(), "three dice cannot show four marks");
    }

    #[test]
    fn masters_crosses_the_far_end_and_its_rosettes_are_not_safe() {
        // Black at its step 16, the far rosette of its own row (square 18), which
        // White walks at its step 12. White at step 11 takes it with a 1.
        let position = picture(&MASTERS, &[(12, 'W'), (18, 'B')], [6, 6], 'w');
        assert_eq!(MASTERS.square_at(Side::White, Spot::Step(11)), Some(12));
        assert!(offers(&position, 1).contains(&(11, 12, true, true)), "{:?}", offers(&position, 1));
        // The same square in Finkel is Black's own and White never reaches it.
        assert!(!FINKEL.is_shared(18));
        // Bearing off from step 16 needs exactly 1 (or a nought for 4 from step 13).
        let home = picture(&MASTERS, &[(4, 'W')], [6, 7], 'w');
        assert_eq!(offers(&home, 1), [(0, 1, false, false), (16, 17, false, false)]);
        let near = picture(&MASTERS, &[(19, 'W')], [6, 7], 'w');
        assert_eq!(MASTERS.square_at(Side::White, Spot::Step(13)), Some(19));
        assert!(offers(&near, 0).contains(&(13, 17, false, false)));
    }

    #[test]
    fn finkel_keeps_its_middle_rosette_safe_and_blitz_does_not() {
        // Black on square 9, the middle rosette; White two steps before it.
        for (rules, takes) in [(&FINKEL, false), (&BLITZ, true)] {
            let position = picture(rules, &[(7, 'W'), (9, 'B')], [rules.pieces - 1, rules.pieces - 1], 'w');
            let taking = offers(&position, 2).iter().any(|&(from, to, captures, _)| from == 6 && to == 8 && captures);
            assert_eq!(taking, takes, "{}", rules.name);
        }
    }

    #[test]
    fn a_capture_earns_another_throw_only_in_blitz() {
        for (rules, again) in [(&BLITZ, true), (&MASTERS, false)] {
            let mut position = picture(rules, &[(6, 'W'), (7, 'B')], [rules.pieces - 1, rules.pieces - 1], 'w');
            let capture = position.moves_for(throw(1)).into_iter().find(|o| o.captures).expect("a capture");
            assert_eq!(capture.again, again, "{}", rules.name);
            assert_eq!(position.play(capture.chosen), again);
            assert_eq!(position.to_move() == Side::White, again);
            assert_eq!(position.waiting(Side::Black), rules.pieces);
        }
    }

    #[test]
    fn aseb_runs_twelve_shared_squares_with_three_safe_rosettes() {
        let position = Position::starting_with(&ASEB, Side::Black);
        assert_eq!(offers(&position, 4), [(0, 4, false, true)]);
        assert_eq!(ASEB.square_at(Side::Black, Spot::Step(16)), Some(15));
        // Black on square 11, a shared rosette: safe. On square 10 it is taken.
        let safe = picture(&ASEB, &[(9, 'W'), (11, 'B')], [4, 4], 'w');
        assert!(!offers(&safe, 2).iter().any(|o| o.0 == 10), "a safe rosette held by the enemy blocks");
        let open = picture(&ASEB, &[(9, 'W'), (10, 'B')], [4, 4], 'w');
        assert!(offers(&open, 1).contains(&(10, 11, true, false)));
    }

    #[test]
    fn five_pieces_home_win_where_the_rule_set_has_five() {
        for rules in ALL {
            let mut position = Position::starting_with(rules, Side::White);
            assert_eq!(position.waiting(Side::White), rules.pieces);
            for index in 0..rules.pieces as usize - 1 {
                position.white[index] = Spot::Off;
            }
            assert_eq!(position.winner(), None, "{}", rules.name);
            position.white[rules.pieces as usize - 1] = Spot::Off;
            assert_eq!(position.winner(), Some(Side::White), "{}", rules.name);
            assert_eq!(
                position.distance_left(Side::Black),
                rules.pieces as u16 * (rules.path_len() as u16 + 1)
            );
        }
    }
}
