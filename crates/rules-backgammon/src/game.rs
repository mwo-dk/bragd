//! A game: what was rolled, what was played with it, and what it was worth.
//!
//! **The record is not a list of moves.** It is a list of [`Entry`]s, and the
//! roll is one of them. That is the price of the engine never throwing dice: a game stored as "starting
//! position plus turns" would replay into a *different game* the moment anybody
//! opened it, silently.
//!
//! **The roll is its own entry, separate from the turn played with it.** The
//! record reads `5-3`, `13/8 8/5`, `6-1`, `24/18 18/17`, and not `5-3 13/8 8/5`.
//! It was the other way round while the engine took the roll and the turn in one
//! call, and the guarantee rested on that — *there is no state in between
//! for a roll to sit in unrecorded*. But a player has to **see** the roll before
//! choosing a turn, so something must hold it between the throw and the play,
//! and the only two places it can be held are the record and a field beside it.
//! A field is exactly the roll-outside-the-record to avoid: it is lost
//! when a game is abandoned mid-turn, it is a second source of truth to fall out
//! of step with the entries, and nothing in a stored row would ever show it
//! had existed. As an entry it is written down the instant it is generated, the
//! record stays self-describing, replay is uniform, and a game abandoned with
//! the dice showing still says what they showed.
//!
//! **Who moves first is still not in the entries.** The opening roll is one die
//! each with the higher going first, so seating is a fact about the Game rather
//! than about the position, and it lives in [`Position::starting_with`]. Making
//! it an entry would put the same fact in two places.
//!
//! **Scoring is not one point.** Backgammon settles for the cube's value times
//! one, two or three depending on how badly the loser lost, and the two
//! multipliers are easy to get subtly wrong:
//!
//! - a **gammon** is the loser having borne off **none**, and is worth double;
//! - a **backgammon** is that *and* a checker still on the bar or still in the
//!   winner's home board, and is worth triple.
//!
//! The trap is the second condition. "Still in your own home board" is not it —
//! it is the *winner's* home, the far end of the board, the one place a checker
//! has not begun the journey at all.

use crate::board::{HOME, Side};
use crate::cube::{Cube, CubeError};
use crate::dice::Roll;
use crate::position::{Continuation, Position};
use crate::turn::{Play, Turn};
use core::fmt;

/// One thing that happened, in order.
///
/// The side is not stored on any of these because it is never in doubt. Entries
/// arrive in turn order, so a roll belongs to whoever is to move, a double is
/// offered by that same player and answered by the other one. Carrying the side
/// as well would be a second copy of a fact the position already knows, and two
/// copies of a fact eventually disagree.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Entry {
    /// What the platform threw, written down **before** anybody sees it.
    ///
    /// A double is spelled once — `4-4`, not four entries of `4` — because it is
    /// one throw. That it is worth four moves rather than two is a rule about
    /// the roll and not a fact about the record; [`Roll::dice`] applies it.
    Roll(Roll),
    /// A turn, in the mover's own point numbers: `13/8 8/5`, `bar/20`, `6/off`,
    /// `13/11*` for a hit.
    ///
    /// **A turn that can play nothing is written `(no play)` and is an entry
    /// like any other.** A player who is shut out still consumes a turn, so a
    /// record that quietly skipped it would replay with the sides swapped from
    /// that point on — every checker on a plausible point, nothing to throw, and
    /// no way to tell from the result that it was the wrong game.
    ///
    /// The spelling is [`Turn`]'s own rather than a bare word invented for the
    /// wire. A bare `pass` reads well and was not taken, because it would put
    /// two spellings of one thing in the crate — one for `Turn::to_string`, one
    /// for the record — and the empty turn is precisely the case a parser
    /// written in a hurry gets wrong. `(no play)` is also what a match file
    /// writes, so a record stays legible to somebody who has never seen
    /// Bragd.
    Turn(Turn),
    Double,
    Take,
    Drop,
}

impl Entry {
    /// The wire form: `5-3`, `13/8 8/5`, `(no play)`, `double`, `take`, `drop`.
    pub fn to_wire(&self) -> String {
        match self {
            Entry::Roll(roll) => roll.to_string(),
            Entry::Turn(turn) => turn.to_string(),
            Entry::Double => String::from("double"),
            Entry::Take => String::from("take"),
            Entry::Drop => String::from("drop"),
        }
    }

    /// Reads one entry back.
    ///
    /// The three forms cannot be confused with each other and it is worth
    /// saying why rather than trusting it: a roll always has a `-` between two
    /// digits and never a `/`, a turn always has a `/` in every one of its
    /// tokens, and the three cube words are neither. So the order the branches
    /// are tried in is a matter of taste rather than of correctness — which is
    /// the property that makes this safe to hand a database row.
    pub fn parse(text: &str) -> Option<Entry> {
        match text.trim() {
            "double" => Some(Entry::Double),
            "take" => Some(Entry::Take),
            "drop" => Some(Entry::Drop),
            word => match Roll::parse(word) {
                Some(roll) => Some(Entry::Roll(roll)),
                None => Turn::parse(word).map(Entry::Turn),
            },
        }
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_wire())
    }
}

/// What a game is waiting for, and from whom.
///
/// **This is what the platform reads to decide whether to throw dice**, and
/// there is no safe way to infer it from the shape of the record instead. Both
/// mistakes are silent: throw a second time while a roll is already written down
/// and the player plays the new dice with the old ones showing; never throw and
/// the game sits there with nobody able to act and nothing logged.
///
/// `side` is the player who must act, which is **not** always the side to move.
/// A double is offered on your own turn before you roll, so while one is on the
/// table the position still says the doubler is to play and it is the other
/// player who owes an answer. A clock or a "your turn" light driven by the
/// position would be pointed at the wrong player for the whole of a cube
/// decision.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Needed {
    /// Nothing has been thrown for this turn. The platform throws, writes the
    /// roll into the record as its own entry, and only then shows it to anybody.
    Roll { side: Side },
    /// A roll is on the record and nothing has been played with it. This is the
    /// state that did not exist when the engine took a roll and a turn together,
    /// and the reason the roll is an entry.
    Turn { side: Side, roll: Roll },
    /// A double is on the table and must be taken or dropped before anything
    /// else happens.
    Answer { side: Side },
}

impl Needed {
    /// Whoever has to act. The clock belongs to this player.
    pub fn side(self) -> Side {
        match self {
            Needed::Roll { side } | Needed::Turn { side, .. } | Needed::Answer { side } => side,
        }
    }
}

/// How a game ended, and what it was worth.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    pub winner: Side,
    pub reason: WinReason,
    /// The cube's value times the multiplier for `reason`. The platform stores
    /// this; nothing else needs to know how it was arrived at.
    pub points: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WinReason {
    /// Fifteen off, and the loser has borne off at least one. One times the
    /// cube.
    Single,
    /// Fifteen off and the loser none. Twice the cube.
    Gammon,
    /// A gammon, with the loser still on the bar or still in the winner's home
    /// board. Three times the cube.
    Backgammon,
    /// **A double was declined, so the game ended without a move.** Worth the
    /// cube's value before the double, and never a gammon: nobody bore anything
    /// off, and there is no board result to grade.
    DoubleDeclined,
    /// Conceded, at the grade the loser conceded. Backgammon lets a player
    /// resign a single, a gammon or a backgammon, and the grade is theirs to
    /// choose because it is theirs to pay.
    Resignation(Concession),
}

/// What a resigning player admits to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Concession {
    Single,
    Gammon,
    Backgammon,
}

impl Concession {
    const fn multiplier(self) -> u16 {
        match self {
            Concession::Single => 1,
            Concession::Gammon => 2,
            Concession::Backgammon => 3,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Game {
    initial: Position,
    position: Position,
    cube: Cube,
    entries: Vec<Entry>,
    outcome: Option<Outcome>,
}

impl Game {
    pub fn new() -> Game {
        Game::from_position(Position::starting())
    }

    pub fn from_position(position: Position) -> Game {
        Game {
            initial: position,
            position,
            cube: Cube::centred(),
            entries: Vec::new(),
            outcome: None,
        }
    }

    pub fn position(&self) -> &Position {
        &self.position
    }

    pub fn initial_position(&self) -> &Position {
        &self.initial
    }

    pub fn cube(&self) -> Cube {
        self.cube
    }

    /// The record: everything that happened, in order. This is what the platform
    /// stores and what [`Game::replay`] is given back.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    pub fn is_over(&self) -> bool {
        self.outcome.is_some()
    }

    /// The roll on the table: thrown, written down, and not yet played with.
    ///
    /// Read off the end of the record rather than kept in a field beside it. A
    /// field would be a second copy of the roll to fall out of step with the
    /// entries, and the copy that went stale would be the one the player was
    /// looking at. The last entry is the whole answer because nothing may
    /// happen between a roll and its turn: a double must be offered *before*
    /// rolling, which [`Game::offer_double`] enforces, so a `Roll` at the end of
    /// the record has nothing that could follow it but its own `Turn`.
    pub fn pending_roll(&self) -> Option<Roll> {
        match self.entries.last() {
            Some(Entry::Roll(roll)) => Some(*roll),
            _ => None,
        }
    }

    /// What the game is waiting for, or `None` once it is over.
    ///
    /// The platform reads this and nothing else to decide when to throw — see
    /// [`Needed`] for the two ways inferring it goes wrong.
    pub fn needs(&self) -> Option<Needed> {
        if self.is_over() {
            return None;
        }
        // The cube first, because an unanswered double stops everything —
        // including the roll that would otherwise be the obvious next thing.
        if let Some(doubler) = self.cube.offer() {
            return Some(Needed::Answer {
                side: doubler.other(),
            });
        }
        let side = self.position.to_move();
        match self.pending_roll() {
            Some(roll) => Some(Needed::Turn { side, roll }),
            None => Some(Needed::Roll { side }),
        }
    }

    /// The last turn played and who played it.
    ///
    /// The mover is counted off the record rather than taken from the position,
    /// because the position has moved on: after a turn it is the other side's,
    /// and a game that then ended by a drop or a resignation has entries after
    /// the turn as well. Anything drawing the last turn needs the mover, since
    /// the notation is in that player's own numbers, and deriving it outside the
    /// engine would be a second copy of turn order.
    pub fn last_turn(&self) -> Option<(Side, &Turn)> {
        let mut side = self.initial.to_move();
        let mut last = None;
        for entry in &self.entries {
            if let Entry::Turn(turn) = entry {
                last = Some((side, turn));
                side = side.other();
            }
        }
        last
    }

    /// Every legal turn for the roll on the table, or nothing when there is no
    /// roll on the table.
    ///
    /// Empty means "there is nothing to choose from *yet*", never "you are shut
    /// out" — a shut-out player has exactly one turn, the pass. [`Game::needs`]
    /// is what distinguishes them.
    pub fn turns(&self) -> Vec<Turn> {
        match self.pending_roll() {
            Some(roll) => self.turns_for(roll),
            None => Vec::new(),
        }
    }

    /// Every legal turn for `roll`. Empty once the game is over or while a
    /// double is waiting to be answered — in both cases nothing may be played
    /// whatever was thrown.
    pub fn turns_for(&self, roll: Roll) -> Vec<Turn> {
        if self.is_over() || self.cube.is_offered() {
            return Vec::new();
        }
        self.position.turns_for(roll)
    }

    /// What a player who has tapped out `played` so far may do next, against
    /// the roll on the table.
    ///
    /// The call a board assembling a turn one checker at a time needs; see
    /// [`Position::continue_turn`] for why it is answered from the whole-turn
    /// sequences rather than from the dice.
    ///
    /// `None` for the three ways there is nothing to continue: no roll on the
    /// table, a game that is over, and a half-turn that cannot be finished
    /// legally. The caller is not owed the distinction — every one of them
    /// means "do not put that checker there", and [`Game::needs`] already says
    /// which state the game is in.
    ///
    /// **A double waiting to be answered is the first of those, not a fourth.**
    /// `offer_double` refuses while a roll is showing and then writes an
    /// `Entry::Double`, and `pending_roll` reads only the *last* entry — so an
    /// offer on the table always means no roll on it, and the `?` below is what
    /// actually returns. The `cube.is_offered()` test is kept for the reason
    /// `play_turn` keeps its own: three public doors rely on that invariant and
    /// it is cheap to stop trusting it. It is defence in depth, not a branch
    /// anything reaches.
    pub fn continue_turn(&self, played: &[Play]) -> Option<Continuation> {
        if self.is_over() || self.cube.is_offered() {
            return None;
        }
        self.position.continue_turn(self.pending_roll()?, played)
    }

    /// Writes a roll into the record.
    ///
    /// **The dice come from outside** and this is the door they come
    /// through. Calling it twice in a row is refused rather than tolerated: two
    /// rolls with no turn between them is the platform having thrown while a
    /// roll was already showing, and the player would play the second with the
    /// first still on the board.
    pub fn roll(&mut self, roll: Roll) -> Result<(), PlayError> {
        if self.is_over() {
            return Err(PlayError::GameOver);
        }
        if self.cube.is_offered() {
            return Err(PlayError::DoubleWaiting);
        }
        if self.pending_roll().is_some() {
            return Err(PlayError::AlreadyRolled);
        }
        self.entries.push(Entry::Roll(roll));
        Ok(())
    }

    /// Plays `turn` with the roll on the table, or says why not.
    ///
    /// The turn that gets recorded is the engine's own spelling of it, not the
    /// caller's. They describe the same checkers arriving on the same points —
    /// that is what makes the turn legal — but the caller may have ordered the
    /// plays differently or put the `*` on the other one of two checkers landing
    /// on a blot, and a Library should hold one canonical form.
    pub fn play_turn(&mut self, turn: &Turn) -> Result<(), PlayError> {
        if self.is_over() {
            return Err(PlayError::GameOver);
        }
        // Unreachable in practice — nothing may be rolled while a double is on
        // the table, so there is no roll here to play against — and checked
        // anyway, because the alternative is trusting that invariant from three
        // separate public doors.
        if self.cube.is_offered() {
            return Err(PlayError::DoubleWaiting);
        }
        let roll = self.pending_roll().ok_or(PlayError::NoRoll)?;
        let legal = self.legal_form_of(roll, turn)?;
        self.apply(legal);
        Ok(())
    }

    /// Plays one entry, whatever kind it is. The one way in for a record.
    pub fn play_entry(&mut self, entry: Entry) -> Result<(), PlayError> {
        match entry {
            Entry::Roll(roll) => self.roll(roll),
            Entry::Turn(turn) => self.play_turn(&turn),
            // The doubler is whoever is to move, and the answer comes from the
            // other player. Both are derivable, which is why neither is stored.
            Entry::Double => {
                let side = self.position.to_move();
                self.offer_double(side)
            }
            Entry::Take => {
                let side = self.cube.offer().ok_or(PlayError::NoOffer)?.other();
                self.take(side)
            }
            Entry::Drop => {
                let side = self.cube.offer().ok_or(PlayError::NoOffer)?.other();
                self.drop(side)
            }
        }
    }

    /// Reads one entry off the wire and plays it. The door a stored record comes
    /// through, one line at a time.
    pub fn play_wire(&mut self, text: &str) -> Result<(), PlayError> {
        let entry = Entry::parse(text).ok_or(PlayError::NotUnderstood)?;
        self.play_entry(entry)
    }

    /// Rolls and plays in one call, for a caller that already has both.
    ///
    /// The two entries still go into the record separately — this is a
    /// convenience, not a second schema. It is **atomic**: an illegal turn
    /// leaves no roll behind, so a caller that had both and was wrong about one
    /// of them has not half-committed a throw nobody made. When a roll is
    /// already on the table it must be *this* roll, because a turn checked
    /// against dice other than the ones showing is the bug the record exists to
    /// make impossible.
    pub fn play(&mut self, roll: Roll, turn: &Turn) -> Result<(), PlayError> {
        let pending = self.pending_roll();
        if let Some(pending) = pending
            && pending != roll
        {
            return Err(PlayError::WrongRoll);
        }
        if self.is_over() {
            return Err(PlayError::GameOver);
        }
        if self.cube.is_offered() {
            return Err(PlayError::DoubleWaiting);
        }
        let legal = self.legal_form_of(roll, turn)?;
        if pending.is_none() {
            self.entries.push(Entry::Roll(roll));
        }
        self.apply(legal);
        Ok(())
    }

    /// Offers a double. Only on your own turn, and **only before rolling** —
    /// which is now a thing the engine has to enforce rather than a thing the
    /// shape of the API guaranteed. Splitting the roll off into its own entry
    /// created the state in between that a late double could sit in, and a
    /// double offered with the dice already showing would let a player see what
    /// they had thrown before deciding what the game was worth.
    pub fn offer_double(&mut self, side: Side) -> Result<(), PlayError> {
        if self.is_over() {
            return Err(PlayError::GameOver);
        }
        if side != self.position.to_move() {
            return Err(PlayError::NotYourTurn);
        }
        if self.pending_roll().is_some() {
            return Err(PlayError::AlreadyRolled);
        }
        self.cube.offer_by(side)?;
        self.entries.push(Entry::Double);
        Ok(())
    }

    /// Takes the double: the stake doubles and the cube changes hands.
    pub fn take(&mut self, side: Side) -> Result<(), PlayError> {
        if self.is_over() {
            return Err(PlayError::GameOver);
        }
        self.cube.accept_by(side)?;
        self.entries.push(Entry::Take);
        Ok(())
    }

    /// Declines the double, which **ends the game without a move**.
    pub fn drop(&mut self, side: Side) -> Result<(), PlayError> {
        if self.is_over() {
            return Err(PlayError::GameOver);
        }
        let stake = self.cube.decline_by(side)?;
        self.entries.push(Entry::Drop);
        self.outcome = Some(Outcome {
            winner: side.other(),
            reason: WinReason::DoubleDeclined,
            points: stake,
        });
        Ok(())
    }

    /// Concedes, at the grade the resigning player names.
    ///
    /// Not an [`Entry`], matching the other engines here: a concession is not a
    /// thing that happened on the board, and the record plus this outcome is
    /// what a Library stores.
    pub fn resign(&mut self, side: Side, grade: Concession) -> Result<(), PlayError> {
        if self.is_over() {
            return Err(PlayError::GameOver);
        }
        self.outcome = Some(Outcome {
            winner: side.other(),
            reason: WinReason::Resignation(grade),
            points: self.cube.value() * grade.multiplier(),
        });
        Ok(())
    }

    /// The engine's own spelling of `turn`, if it is a legal turn for `roll`.
    fn legal_form_of(&self, roll: Roll, turn: &Turn) -> Result<Turn, PlayError> {
        let wanted = turn.placements();
        self.position
            .turns_for(roll)
            .into_iter()
            .find(|candidate| candidate.placements() == wanted)
            .ok_or(PlayError::NotLegal)
    }

    /// Moves the checkers, records the turn, and settles the game if that
    /// finished it. The one place a turn is committed, so that the three doors
    /// into it cannot drift apart.
    fn apply(&mut self, turn: Turn) {
        self.position = self.position.after(&turn);
        self.entries.push(Entry::Turn(turn));
        self.outcome = self.settle();
    }

    /// Whether somebody has borne off all fifteen, and what it was worth.
    fn settle(&self) -> Option<Outcome> {
        let winner = self.position.winner()?;
        let loser = winner.other();
        let reason = if self.position.off(loser) > 0 {
            WinReason::Single
        } else if self.loser_is_backgammoned(loser, winner) {
            WinReason::Backgammon
        } else {
            WinReason::Gammon
        };
        let multiplier = match reason {
            WinReason::Single => 1,
            WinReason::Gammon => 2,
            WinReason::Backgammon => 3,
            // Neither can be reached from here: both end the game before a
            // checker is borne off.
            WinReason::DoubleDeclined | WinReason::Resignation(_) => 1,
        };
        Some(Outcome {
            winner,
            reason,
            points: self.cube.value() * multiplier,
        })
    }

    /// A gammoned loser who is also **on the bar or still in the winner's home
    /// board**. The winner's home, not the loser's: it is the far end of the
    /// board, the one place a checker has not begun the journey at all.
    fn loser_is_backgammoned(&self, loser: Side, winner: Side) -> bool {
        if self.position.bar(loser) > 0 {
            return true;
        }
        (1..=HOME).any(|number| self.position.point(winner, number).held_by(loser))
    }

    /// Rebuilds a game from its record. The test that this reproduces the
    /// original exactly is the point of storing the rolls at all.
    pub fn replay(initial: Position, entries: &[Entry]) -> Result<Game, PlayError> {
        let mut game = Game::from_position(initial);
        for entry in entries {
            game.play_entry(entry.clone())?;
        }
        Ok(game)
    }
}

impl Default for Game {
    fn default() -> Game {
        Game::new()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayError {
    /// The turn is not one of the legal ones. In backgammon that very often
    /// means the turn was legal *as far as it went* and stopped short of a die
    /// it was obliged to use.
    NotLegal,
    NotUnderstood,
    GameOver,
    NotYourTurn,
    /// A double is on the table and must be answered before anything else.
    DoubleWaiting,
    /// A take or a drop with nothing doubled.
    NoOffer,
    /// A roll arrived with one already on the table, or a double was offered
    /// after rolling. Both are the same mistake — acting as though the dice were
    /// not already showing.
    AlreadyRolled,
    /// A turn with nothing rolled. Half of what makes a turn checkable is
    /// missing, and guessing a roll to check it against is the thing this crate
    /// exists not to do.
    NoRoll,
    /// A turn offered with dice other than the ones on the table.
    WrongRoll,
    Cube(CubeError),
}

impl From<CubeError> for PlayError {
    fn from(error: CubeError) -> PlayError {
        PlayError::Cube(error)
    }
}

impl fmt::Display for PlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlayError::NotLegal => f.write_str("that is not a legal turn for this roll"),
            PlayError::NotUnderstood => f.write_str("that is not an entry"),
            PlayError::GameOver => f.write_str("this game is over"),
            PlayError::NotYourTurn => f.write_str("it is not your turn"),
            PlayError::DoubleWaiting => f.write_str("answer the double first"),
            PlayError::NoOffer => f.write_str("nothing has been doubled"),
            PlayError::AlreadyRolled => f.write_str("the dice are already on the table"),
            PlayError::NoRoll => f.write_str("nothing has been rolled yet"),
            PlayError::WrongRoll => f.write_str("that is not the roll on the table"),
            PlayError::Cube(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PlayError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Layout;

    fn roll(text: &str) -> Roll {
        Roll::parse(text).expect("a roll")
    }

    /// Rolls and plays a turn written in notation, which is what most of these
    /// tests want to say in one line.
    fn play(game: &mut Game, dice: &str, turn: &str) -> Result<(), PlayError> {
        let turn = Turn::parse(turn).ok_or(PlayError::NotUnderstood)?;
        game.play(roll(dice), &turn)
    }

    /// White has one checker left on the 2-point and fourteen off, so a 2 wins
    /// the game. What Black looks like is the whole point of each test.
    fn about_to_win(black: Layout) -> Game {
        Game::from_position(
            Position::from_parts(
                Layout {
                    points: &[(2, 1)],
                    bar: 0,
                    off: 14,
                },
                black,
                Side::White,
            )
            .expect("a legal position"),
        )
    }

    #[test]
    fn a_new_game_has_no_outcome_and_a_centred_cube() {
        let game = Game::new();
        assert!(!game.is_over());
        assert_eq!(game.cube().value(), 1);
        assert_eq!(game.cube().owner(), None);
        assert!(game.entries().is_empty());
        assert!(!game.turns_for(roll("3-1")).is_empty());
        // Nothing has been thrown, so there is nothing to choose from — which is
        // not the same as being shut out, and `needs` is what says which.
        assert!(game.turns().is_empty());
        assert_eq!(game.needs(), Some(Needed::Roll { side: Side::White }));
    }

    #[test]
    fn the_roll_is_an_entry_of_its_own_and_the_turn_is_another() {
        // The whole schema, in one game: the record reads as the dice and then
        // what was done with them, never the two glued together.
        let mut game = Game::new();
        game.roll(roll("3-1")).expect("the platform throws");
        assert_eq!(game.pending_roll(), Some(roll("3-1")));
        assert_eq!(
            game.needs(),
            Some(Needed::Turn {
                side: Side::White,
                roll: roll("3-1")
            }),
            "the dice are showing and White has to do something with them"
        );

        game.play_wire("8/5 6/5")
            .expect("the best opening there is");
        assert_eq!(
            game.entries()
                .iter()
                .map(Entry::to_wire)
                .collect::<Vec<_>>(),
            ["3-1", "8/5 6/5"]
        );
        assert_eq!(game.pending_roll(), None);
        assert_eq!(
            game.needs(),
            Some(Needed::Roll { side: Side::Black }),
            "and now the platform throws for the other player"
        );
    }

    /// **A turn assembled one tap at a time ends in a turn the game accepts.**
    ///
    /// The half that makes the partial-turn API worth having: the board walks
    /// `continue_turn` until it says `Complete`, then hands that turn straight
    /// to `play_turn`. Nothing in between reassembles it, so there is no second
    /// spelling of one turn for a Library to disagree with.
    #[test]
    fn a_turn_can_be_assembled_one_play_at_a_time_and_then_played() {
        let mut game = Game::new();
        game.roll(roll("3-1")).expect("thrown");

        let mut played: Vec<Play> = Vec::new();
        let finished = loop {
            match game.continue_turn(&played).expect("a turn is available") {
                // Whichever the engine offers first: the point is that every
                // step is legal and the walk terminates, not which turn it
                // arrives at.
                Continuation::More(next) => played.push(next[0]),
                Continuation::Complete(turn) => break turn,
            }
        };
        assert_eq!(finished.dice_used(), 2, "both dice: {finished}");
        game.play_turn(&finished).expect("the engine's own turn");
        assert_eq!(game.entries().len(), 2);
    }

    /// **Nothing to continue is not the same as nothing to play.** All three
    /// states that mean "do not put that checker there" answer `None`, and the
    /// game says which state it is in through `needs` rather than through this.
    ///
    /// The middle assertion is about the *cube leaving no roll behind*, not
    /// about `continue_turn`'s `cube.is_offered()` guard — that guard is
    /// unreachable, because offering a double is only allowed with nothing
    /// showing and then writes an entry of its own. Deleting the guard leaves
    /// this test passing verbatim, which is worth knowing before reading it as
    /// cover for the branch.
    #[test]
    fn there_is_nothing_to_continue_before_the_dice_or_during_a_double() {
        let mut game = Game::new();
        assert_eq!(game.continue_turn(&[]), None, "nothing has been thrown");

        game.offer_double(Side::White).expect("before rolling");
        assert_eq!(
            game.continue_turn(&[]),
            None,
            "a double leaves no roll on the table"
        );
        // Which is the state the game reports, and the door the platform is
        // actually supposed to read.
        assert_eq!(
            game.needs(),
            Some(Needed::Answer { side: Side::Black }),
            "and `needs` is what says so"
        );
        assert_eq!(
            game.roll(roll("3-1")),
            Err(PlayError::DoubleWaiting),
            "nothing may be thrown under an unanswered double"
        );
        game.take(Side::Black).expect("taken");

        game.roll(roll("3-1")).expect("thrown");
        assert!(game.continue_turn(&[]).is_some(), "and now there is");

        game.resign(Side::White, Concession::Single).expect("over");
        assert_eq!(game.continue_turn(&[]), None, "the game is over");
    }

    /// Both ways of getting it wrong, and they are wrong in opposite
    /// directions: a second throw would be played with the first one showing,
    /// and a turn against no throw is a turn checked against dice nobody saw.
    #[test]
    fn a_second_roll_and_a_turn_with_no_roll_are_both_refused() {
        let mut game = Game::new();
        assert_eq!(game.play_wire("8/5 6/5"), Err(PlayError::NoRoll));
        game.roll(roll("3-1")).expect("thrown");
        assert_eq!(game.roll(roll("6-5")), Err(PlayError::AlreadyRolled));
        assert_eq!(game.entries().len(), 1, "and nothing was written down");

        // A turn is checked against the roll on the table, never against one
        // handed in beside it.
        assert_eq!(
            play(&mut game, "6-5", "24/18 18/13"),
            Err(PlayError::WrongRoll)
        );
        play(&mut game, "3-1", "8/5 6/5").expect("the roll that is showing");
    }

    /// A double is offered *before* rolling. That used to be guaranteed by the
    /// shape of the API — there was no state between the roll and the turn — and
    /// splitting the roll off created exactly that state, so it is now a rule
    /// the engine enforces. Otherwise a player could look at the dice and then
    /// decide what the game was worth.
    #[test]
    fn a_double_may_not_be_offered_once_the_dice_are_on_the_table() {
        let mut game = Game::new();
        game.roll(roll("6-6")).expect("thrown");
        assert_eq!(
            game.offer_double(Side::White),
            Err(PlayError::AlreadyRolled),
            "White has seen the best roll in the game and wants to double"
        );
        assert_eq!(game.cube().value(), 1);
        assert_eq!(game.entries().len(), 1);
    }

    #[test]
    fn bearing_off_the_fifteenth_checker_wins_a_single_point() {
        // Black has borne off one, so it is a plain win however far behind the
        // rest of Black's checkers are.
        let mut game = about_to_win(Layout {
            points: &[(6, 14)],
            bar: 0,
            off: 1,
        });
        play(&mut game, "6-5", "2/off 1/off").expect_err("there is only one checker to bear off");
        assert!(game.entries().is_empty(), "and no roll was left behind");
        play(&mut game, "6-5", "2/off").expect("the last checker comes off");

        assert_eq!(
            game.outcome(),
            Some(Outcome {
                winner: Side::White,
                reason: WinReason::Single,
                points: 1,
            })
        );
        assert_eq!(game.needs(), None, "a finished game waits for nothing");
    }

    #[test]
    fn a_gammon_is_two_points() {
        // Black has borne off nothing and is nowhere near White's home, which is
        // exactly a gammon and not a backgammon.
        let mut game = about_to_win(Layout {
            points: &[(6, 15)],
            bar: 0,
            off: 0,
        });
        play(&mut game, "6-5", "2/off").expect("off");
        assert_eq!(
            game.outcome(),
            Some(Outcome {
                winner: Side::White,
                reason: WinReason::Gammon,
                points: 2,
            })
        );
    }

    #[test]
    fn a_backgammon_is_three_points_when_the_loser_is_on_the_bar() {
        let mut game = about_to_win(Layout {
            points: &[(6, 14)],
            bar: 1,
            off: 0,
        });
        play(&mut game, "6-5", "2/off").expect("off");
        assert_eq!(
            game.outcome(),
            Some(Outcome {
                winner: Side::White,
                reason: WinReason::Backgammon,
                points: 3,
            })
        );
    }

    #[test]
    fn a_backgammon_is_three_points_when_the_loser_is_in_the_winners_home() {
        // **The winner's home board, not the loser's.** A Black checker on
        // Black's own 6-point is a gammon; one on White's 6-point — Black's
        // 19-point — is a backgammon, because it has not begun the journey.
        let mut game = about_to_win(Layout {
            points: &[(19, 1), (6, 14)],
            bar: 0,
            off: 0,
        });
        play(&mut game, "6-5", "2/off").expect("off");
        assert_eq!(
            game.outcome().map(|outcome| outcome.reason),
            Some(WinReason::Backgammon)
        );

        // The same shape with that checker at home instead is only a gammon,
        // which is the assertion that stops the two home boards being confused.
        let mut gammon = about_to_win(Layout {
            points: &[(6, 15)],
            bar: 0,
            off: 0,
        });
        play(&mut gammon, "6-5", "2/off").expect("off");
        assert_eq!(
            gammon.outcome().map(|outcome| outcome.reason),
            Some(WinReason::Gammon)
        );
    }

    #[test]
    fn the_cube_multiplies_the_score() {
        let mut game = about_to_win(Layout {
            points: &[(6, 15)],
            bar: 0,
            off: 0,
        });
        game.offer_double(Side::White).expect("White is to move");
        game.take(Side::Black).expect("Black takes");
        play(&mut game, "6-5", "2/off").expect("off");
        assert_eq!(
            game.outcome(),
            Some(Outcome {
                winner: Side::White,
                reason: WinReason::Gammon,
                points: 4,
            }),
            "a gammon on a cube of two"
        );
    }

    #[test]
    fn a_declined_double_ends_the_game_without_a_move() {
        let mut game = Game::new();
        game.offer_double(Side::White).expect("White is to move");
        assert_eq!(
            game.needs(),
            Some(Needed::Answer { side: Side::Black }),
            "and it is Black who owes an answer, not the side to move"
        );
        assert_eq!(
            game.position().to_move(),
            Side::White,
            "the position still says the doubler is to play, which is why `needs` exists"
        );
        assert!(
            game.turns_for(roll("6-5")).is_empty(),
            "nothing may be played while a double is on the table"
        );
        assert_eq!(game.roll(roll("6-5")), Err(PlayError::DoubleWaiting));
        assert_eq!(
            play(&mut game, "6-5", "24/18 13/8"),
            Err(PlayError::DoubleWaiting)
        );

        game.drop(Side::Black).expect("Black drops");
        assert_eq!(
            game.outcome(),
            Some(Outcome {
                winner: Side::White,
                reason: WinReason::DoubleDeclined,
                points: 1,
            }),
            "the value before the double, and never a gammon"
        );
        assert_eq!(game.entries(), &[Entry::Double, Entry::Drop]);
        assert_eq!(game.position(), &Position::starting(), "no checker moved");
    }

    #[test]
    fn only_the_player_to_move_may_double() {
        let mut game = Game::new();
        assert_eq!(game.offer_double(Side::Black), Err(PlayError::NotYourTurn));
        game.offer_double(Side::White).expect("White is to move");
    }

    #[test]
    fn resigning_a_gammon_costs_two_points_times_the_cube() {
        let mut game = Game::new();
        game.offer_double(Side::White).expect("to move");
        game.take(Side::Black).expect("taken");
        game.resign(Side::White, Concession::Gammon)
            .expect("White concedes a gammon");
        assert_eq!(
            game.outcome(),
            Some(Outcome {
                winner: Side::Black,
                reason: WinReason::Resignation(Concession::Gammon),
                points: 4,
            })
        );
        assert_eq!(
            game.resign(Side::Black, Concession::Single),
            Err(PlayError::GameOver)
        );
    }

    #[test]
    fn a_turn_is_recorded_in_the_engines_own_spelling() {
        // The caller wrote the plays in the other order. It is the same turn —
        // the same checkers on the same points — so it is legal, and what goes
        // into a Library is one canonical form rather than whatever arrived.
        let mut game = Game::new();
        play(&mut game, "3-1", "6/5 8/5").expect("the 5-point, spelled backwards");
        let Some((mover, turn)) = game.last_turn() else {
            panic!("no turn was recorded");
        };
        assert_eq!(mover, Side::White);
        assert_eq!(turn.to_string(), "8/5 6/5");
    }

    #[test]
    fn a_turn_that_stops_short_of_a_die_it_must_use_is_rejected() {
        let mut game = Game::new();
        assert_eq!(
            play(&mut game, "6-5", "24/18"),
            Err(PlayError::NotLegal),
            "the 5 has somewhere to go"
        );
        assert!(
            game.entries().is_empty(),
            "a refused turn leaves no half-committed roll behind"
        );
        play(&mut game, "6-5", "24/18 18/13").expect("the lover's leap");
    }

    /// **The entry a parser written in a hurry gets wrong**, checked in both
    /// directions. A shut-out player still takes a turn, so the record has to
    /// contain it or every later entry replays for the wrong side.
    #[test]
    fn a_turn_that_plays_nothing_is_still_an_entry_and_reads_back_as_one() {
        let pass = Entry::Turn(Turn::pass());
        assert_eq!(pass.to_wire(), "(no play)");
        assert_eq!(Entry::parse("(no play)"), Some(pass.clone()));

        for entry in [
            Entry::Roll(roll("5-3")),
            Entry::Roll(roll("4-4")),
            Entry::Turn(Turn::parse("13/8 8/5").expect("a turn")),
            Entry::Turn(Turn::parse("bar/20 13/11*").expect("a turn")),
            Entry::Turn(Turn::parse("6/off").expect("a turn")),
            pass,
            Entry::Double,
            Entry::Take,
            Entry::Drop,
        ] {
            assert_eq!(
                Entry::parse(&entry.to_wire()),
                Some(entry.clone()),
                "{entry} did not read back"
            );
        }

        // A double is one throw and is spelled once. Four entries of `4` would
        // be four throws.
        assert_eq!(Entry::Roll(roll("4-4")).to_wire(), "4-4");
        // And silence is not a pass, for the reason `Turn::parse` refuses it: a
        // truncated record must not read as a shorter game.
        for text in ["", "no play", "(noplay)", "pass", "-", "7-1", "5", "13"] {
            assert_eq!(Entry::parse(text), None, "parsed {text:?}");
        }
    }

    /// A shut-out board played through the record, because the pass is the entry
    /// whose absence would be invisible: the game would go on, with the sides
    /// swapped, and every position after it would look perfectly ordinary.
    #[test]
    fn a_shut_out_turn_goes_into_the_record_and_replays() {
        // Black holds all six points of Black's home board and White is on the
        // bar, so no die brings the checker in.
        let position = Position::from_parts(
            Layout {
                points: &[(13, 14)],
                bar: 1,
                off: 0,
            },
            Layout {
                points: &[(1, 2), (2, 2), (3, 2), (4, 2), (5, 2), (6, 2)],
                bar: 0,
                off: 3,
            },
            Side::White,
        )
        .expect("a legal position");

        let mut game = Game::from_position(position);
        game.roll(roll("6-5")).expect("thrown");
        game.play_wire("(no play)").expect("nothing can be played");
        assert_eq!(
            game.entries()
                .iter()
                .map(Entry::to_wire)
                .collect::<Vec<_>>(),
            ["6-5", "(no play)"]
        );
        assert_eq!(
            game.position().to_move(),
            Side::Black,
            "a player who can do nothing has still had their turn"
        );

        let replayed = Game::replay(position, game.entries()).expect("it replays");
        assert_eq!(replayed, game);
    }

    /// The two doors into a turn have to agree. The platform uses the split one
    /// — throw, show the dice, take a turn — and a bot or a replay may use the
    /// combined one, and a record written by either must be the same record.
    #[test]
    fn rolling_and_playing_separately_records_the_same_game_as_doing_both_at_once() {
        let mut apart = Game::new();
        apart.roll(roll("3-1")).expect("thrown");
        apart.play_wire("8/5 6/5").expect("played");

        let mut together = Game::new();
        play(&mut together, "3-1", "8/5 6/5").expect("both at once");

        assert_eq!(apart, together);
        assert_eq!(apart.entries(), together.entries());
    }

    /// A record that puts a cube action where a turn belongs is refused rather
    /// than absorbed. It would otherwise leave a roll stranded in the middle of
    /// the record, which is the one shape `pending_roll` reads the last entry to
    /// avoid.
    #[test]
    fn a_record_cannot_slip_a_double_in_between_a_roll_and_its_turn() {
        let entries = [Entry::Roll(roll("3-1")), Entry::Double];
        assert_eq!(
            Game::replay(Position::starting(), &entries),
            Err(PlayError::AlreadyRolled)
        );
    }
}
