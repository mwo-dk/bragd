//! The record, and the game built from it.
//!
//! **A Game is its entries.** An entry is a throw or a move, they alternate only
//! loosely — a rosette earns another throw, and a throw of nought is followed by
//! no move at all — and [`Game::replay`] rebuilds the whole thing from them. That
//! is the property a Library rests on, and `tests/properties.rs` asserts it
//! over thousands of played games rather than hoping.

use crate::board::{Side, Spot};
use crate::dice::Throw;
use crate::position::{Move, Offer, Position};

/// One line of the record.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Entry {
    /// What the platform threw.
    Throw(Throw),
    /// What the player did with it.
    Move(Move),
}

impl Entry {
    /// The wire spelling. One entry, one token, no spaces — so a record joins on
    /// whitespace and a reader never has to know how many words an entry is.
    pub fn to_wire(self) -> String {
        match self {
            Entry::Throw(throw) => throw.to_wire(),
            Entry::Move(chosen) => chosen.to_wire(),
        }
    }

    /// Reads an entry back.
    pub fn parse(text: &str) -> Option<Entry> {
        if let Some(throw) = Throw::parse(text) {
            return Some(Entry::Throw(throw));
        }
        let (from, to) = Move::parse(text)?;
        Some(Entry::Move(Move {
            from: spot_at(from)?,
            to: spot_at(to)?,
        }))
    }
}

/// A distance back into a spot, for the parser.
fn spot_at(distance: u8) -> Option<Spot> {
    match distance {
        0 => Some(Spot::Waiting),
        step if (step as usize) <= crate::board::PATH => Some(Spot::Step(step)),
        step if (step as usize) == crate::board::PATH + 1 => Some(Spot::Off),
        _ => None,
    }
}

/// What the game is waiting for.
///
/// **The engine answers this rather than the platform inferring it.** Two
/// mistakes are available and both are silent: throw twice and the player chooses
/// with the wrong number showing; never throw and the Game hangs with nobody able
/// to act and nothing logged.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Needed {
    /// The platform must throw for this side.
    Throw { side: Side },
    /// This side must choose one of the moves the throw allows.
    Move { side: Side },
    /// Nothing; the game is over.
    Nothing,
}

impl Needed {
    /// Whose turn this is, or `None` when the game is over.
    pub fn side(self) -> Option<Side> {
        match self {
            Needed::Throw { side } | Needed::Move { side } => Some(side),
            Needed::Nothing => None,
        }
    }
}

/// How a game ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WinReason {
    /// All seven pieces home.
    Home,
    /// The other side gave up.
    Resignation,
}

/// The result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    pub winner: Side,
    pub reason: WinReason,
}

/// Why an entry was refused.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlayError {
    /// The game is over.
    Over,
    /// A move arrived when a throw was owed, or the other way round.
    OutOfTurn,
    /// The move is not one the position allows with this throw.
    Illegal,
    /// The record could not be read.
    Unreadable(String),
}

impl std::fmt::Display for PlayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayError::Over => write!(formatter, "the game is over"),
            PlayError::OutOfTurn => write!(formatter, "that is not what this game is waiting for"),
            PlayError::Illegal => write!(formatter, "that move is not legal here"),
            PlayError::Unreadable(text) => write!(formatter, "{text}"),
        }
    }
}

/// A whole game: where it started, what has happened, and where that leaves it.
#[derive(Clone, Debug)]
pub struct Game {
    initial: Position,
    position: Position,
    entries: Vec<Entry>,
    /// The throw on the table, if one has been made and not yet used.
    pending: Option<Throw>,
    outcome: Option<Outcome>,
}

impl Game {
    /// A new game, with the given side throwing first.
    pub fn new(first: Side) -> Game {
        Game::from_position(Position::starting_with(first))
    }

    /// A game from a position, for a teaching board.
    pub fn from_position(position: Position) -> Game {
        let outcome = position.winner().map(|winner| Outcome {
            winner,
            reason: WinReason::Home,
        });
        Game {
            initial: position.clone(),
            position,
            entries: Vec::new(),
            pending: None,
            outcome,
        }
    }

    pub fn position(&self) -> &Position {
        &self.position
    }

    pub fn initial_position(&self) -> &Position {
        &self.initial
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    pub fn is_over(&self) -> bool {
        self.outcome.is_some()
    }

    /// The throw waiting to be used, if any.
    pub fn pending_throw(&self) -> Option<Throw> {
        self.pending
    }

    /// What the game is waiting for.
    pub fn needs(&self) -> Needed {
        if self.is_over() {
            return Needed::Nothing;
        }
        let side = self.position.to_move();
        match self.pending {
            Some(_) => Needed::Move { side },
            None => Needed::Throw { side },
        }
    }

    /// The moves the throw on the table allows. Empty when there is no throw, and
    /// empty when the throw allows nothing — [`Self::needs`] tells them apart.
    pub fn moves(&self) -> Vec<Offer> {
        self.pending
            .map(|throw| self.position.moves_for(throw))
            .unwrap_or_default()
    }

    /// Records a throw.
    ///
    /// **A throw that allows nothing passes the turn, and writes nothing extra.**
    ///
    /// An earlier version wrote a second `Pass` entry beside it. That was wrong
    /// for a reason that only shows up at the caller: it supplies inputs
    /// **one string at a time**, so a crate
    /// that turned one supplied throw into two entries would leave the stored
    /// record one shorter than the engine's own history, for ever, and the two
    /// would drift apart entry by entry. The pass earned nothing to pay for
    /// that: replaying the throw alone already passes the turn, because whether
    /// a throw was usable is a function of the position it was thrown into.
    /// Whether it *was* usable is still visible — see `wasted` in the boundary's
    /// state — it is simply derived rather tha stored.
    pub fn throw(&mut self, throw: Throw) -> Result<(), PlayError> {
        if self.is_over() {
            return Err(PlayError::Over);
        }
        if self.pending.is_some() {
            return Err(PlayError::OutOfTurn);
        }
        self.entries.push(Entry::Throw(throw));
        if self.position.moves_for(throw).is_empty() {
            self.position.pass();
        } else {
            self.pending = Some(throw);
        }
        Ok(())
    }

    /// Plays a move against the throw on the table.
    pub fn play(&mut self, chosen: Move) -> Result<(), PlayError> {
        if self.is_over() {
            return Err(PlayError::Over);
        }
        let Some(throw) = self.pending else {
            return Err(PlayError::OutOfTurn);
        };
        let legal = self
            .position
            .moves_for(throw)
            .into_iter()
            .find(|offer| offer.chosen == chosen)
            .ok_or(PlayError::Illegal)?;

        self.position.play(legal.chosen);
        self.entries.push(Entry::Move(legal.chosen));
        self.pending = None;
        if let Some(winner) = self.position.winner() {
            self.outcome = Some(Outcome {
                winner,
                reason: WinReason::Home,
            });
        }
        Ok(())
    }

    /// Plays whatever an entry says, which is what replay does.
    pub fn play_entry(&mut self, entry: Entry) -> Result<(), PlayError> {
        match entry {
            Entry::Throw(throw) => self.throw(throw),
            Entry::Move(chosen) => self.play(chosen),
        }
    }

    /// Plays an entry given as its wire token.
    pub fn play_wire(&mut self, text: &str) -> Result<(), PlayError> {
        let entry = Entry::parse(text)
            .ok_or_else(|| PlayError::Unreadable(format!("cannot read {text:?} as an entry")))?;
        self.play_entry(entry)
    }

    /// Ends the game by concession.
    ///
    /// Deliberately **not** an entry, which is the choice tafl and shogi make: a
    /// conceded game replays back as a game still in progress, and the stored
    /// outcome is what settles it. Ur has no graded concession —
    /// there is one kind of loss and it is worth one game.
    pub fn resign(&mut self, side: Side) -> Result<(), PlayError> {
        if self.is_over() {
            return Err(PlayError::Over);
        }
        self.outcome = Some(Outcome {
            winner: side.other(),
            reason: WinReason::Resignation,
        });
        Ok(())
    }

    /// Rebuilds a game from its record.
    ///
    /// Straightforwardly, entry by entry, because every entry is something a
    /// player or the platform actually did. The engine invents nothing during a
    /// replay, which is what keeps the stored record and the engine's own
    /// history the same list.
    pub fn replay(initial: Position, entries: &[Entry]) -> Result<Game, PlayError> {
        let mut game = Game::from_position(initial);
        for (index, &entry) in entries.iter().enumerate() {
            game.play_entry(entry).map_err(|error| match error {
                PlayError::Unreadable(text) => PlayError::Unreadable(text),
                other => PlayError::Unreadable(format!("entry {}: {other}", index + 1)),
            })?;
        }
        Ok(game)
    }

    /// How many steps each side still has to travel.
    pub fn distances(&self) -> (u16, u16) {
        (
            self.position.distance_left(Side::White),
            self.position.distance_left(Side::Black),
        )
    }

    /// How many pieces each side has brought home.
    pub fn home(&self) -> (u8, u8) {
        (
            self.position.off(Side::White),
            self.position.off(Side::Black),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::PIECES;

    fn throw(marks: u8) -> Throw {
        Throw::new(marks).unwrap()
    }

    #[test]
    fn a_new_game_waits_for_a_throw_from_the_side_that_starts() {
        for first in [Side::White, Side::Black] {
            let game = Game::new(first);
            assert_eq!(game.needs(), Needed::Throw { side: first });
            assert_eq!(game.needs().side(), Some(first));
            assert!(game.moves().is_empty());
            assert!(!game.is_over());
            assert_eq!(game.home(), (0, 0));
        }
    }

    #[test]
    fn a_throw_puts_the_game_in_want_of_a_move() {
        let mut game = Game::new(Side::White);
        game.throw(throw(2)).unwrap();
        assert_eq!(game.needs(), Needed::Move { side: Side::White });
        assert_eq!(game.pending_throw(), Some(throw(2)));
        assert_eq!(game.moves().len(), 1);
        assert_eq!(game.entries(), &[Entry::Throw(throw(2))]);
    }

    #[test]
    fn throwing_twice_is_refused() {
        let mut game = Game::new(Side::White);
        game.throw(throw(2)).unwrap();
        assert_eq!(game.throw(throw(3)), Err(PlayError::OutOfTurn));
    }

    #[test]
    fn moving_before_throwing_is_refused() {
        let mut game = Game::new(Side::White);
        let chosen = Move {
            from: Spot::Waiting,
            to: Spot::Step(1),
        };
        assert_eq!(game.play(chosen), Err(PlayError::OutOfTurn));
    }

    #[test]
    fn an_illegal_move_is_refused_and_changes_nothing() {
        let mut game = Game::new(Side::White);
        game.throw(throw(1)).unwrap();
        let wrong = Move {
            from: Spot::Waiting,
            to: Spot::Step(3),
        };
        assert_eq!(game.play(wrong), Err(PlayError::Illegal));
        assert_eq!(game.entries().len(), 1, "a refusal writes nothing");
        assert_eq!(game.needs(), Needed::Move { side: Side::White });
    }

    #[test]
    fn a_nought_writes_only_itself_and_hands_over() {
        let mut game = Game::new(Side::White);
        game.throw(throw(0)).unwrap();
        assert_eq!(
            game.entries(),
            &[Entry::Throw(throw(0))],
            "a throw is one entry, because the platform supplies one string"
        );
        assert_eq!(game.needs(), Needed::Throw { side: Side::Black });
        assert_eq!(game.pending_throw(), None);
    }

    #[test]
    fn a_rosette_keeps_the_turn() {
        let mut game = Game::new(Side::White);
        game.throw(throw(4)).unwrap();
        let chosen = game.moves()[0];
        assert!(chosen.again);
        game.play(chosen.chosen).unwrap();
        assert_eq!(
            game.needs(),
            Needed::Throw { side: Side::White },
            "a rosette throws again"
        );
    }

    #[test]
    fn an_ordinary_move_hands_over() {
        let mut game = Game::new(Side::White);
        game.throw(throw(1)).unwrap();
        game.play(game.moves()[0].chosen).unwrap();
        assert_eq!(game.needs(), Needed::Throw { side: Side::Black });
    }

    #[test]
    fn a_blocked_throw_passes_without_the_player_doing_anything() {
        // Contrive a position where White's only piece cannot move: six home,
        // one on the last step, and a throw of four cannot bear off.
        let picture = {
            let mut glyphs = vec!['.'; crate::board::SQUARES];
            glyphs[4] = 'W'; // White's step 14
            // Black's seven are all still waiting; a picture whose pieces do not
            // add up to seven a side is refused, which is how this fixture first
            // failed.
            format!("{} 07 60 w", glyphs.into_iter().collect::<String>())
        };
        let position = Position::from_picture(&picture).expect("the contrived picture should read");
        let mut game = Game::from_position(position);
        assert_eq!(game.needs(), Needed::Throw { side: Side::White });
        game.throw(throw(4)).unwrap();
        assert_eq!(
            game.entries(),
            &[Entry::Throw(throw(4))],
            "a throw that cannot be used is recorded, and nothing else is"
        );
        assert_eq!(game.needs(), Needed::Throw { side: Side::Black });
    }

    #[test]
    fn bringing_the_last_piece_home_ends_it() {
        let picture = {
            let mut glyphs = vec!['.'; crate::board::SQUARES];
            glyphs[4] = 'W';
            // Black's seven are all still waiting; a picture whose pieces do not
            // add up to seven a side is refused, which is how this fixture first
            // failed.
            format!("{} 07 60 w", glyphs.into_iter().collect::<String>())
        };
        let mut game = Game::from_position(Position::from_picture(&picture).unwrap());
        game.throw(throw(1)).unwrap();
        let home = game
            .moves()
            .into_iter()
            .find(|m| m.chosen.to == Spot::Off)
            .unwrap();
        game.play(home.chosen).unwrap();
        assert!(game.is_over());
        assert_eq!(
            game.outcome(),
            Some(Outcome {
                winner: Side::White,
                reason: WinReason::Home
            })
        );
        assert_eq!(game.home(), (PIECES, 0));
        assert_eq!(game.needs(), Needed::Nothing);
        assert_eq!(game.needs().side(), None);
        assert_eq!(game.throw(throw(2)), Err(PlayError::Over));
    }

    #[test]
    fn a_resignation_ends_it_and_is_not_an_entry() {
        let mut game = Game::new(Side::White);
        game.throw(throw(2)).unwrap();
        game.play(game.moves()[0].chosen).unwrap();
        let before = game.entries().len();
        game.resign(Side::White).unwrap();
        assert_eq!(
            game.entries().len(),
            before,
            "a concession is not written into the record"
        );
        assert_eq!(
            game.outcome(),
            Some(Outcome {
                winner: Side::Black,
                reason: WinReason::Resignation
            })
        );
        assert_eq!(game.resign(Side::Black), Err(PlayError::Over));
    }

    #[test]
    fn a_game_replays_from_its_entries() {
        let mut game = Game::new(Side::White);
        for (marks, _) in [(4, ()), (1, ()), (0, ()), (2, ()), (3, ()), (1, ())] {
            if game.is_over() {
                break;
            }
            game.throw(throw(marks)).unwrap();
            if let Needed::Move { .. } = game.needs() {
                let chosen = game.moves()[0];
                game.play(chosen.chosen).unwrap();
            }
        }
        assert!(
            game.entries().len() >= 6,
            "the game should have produced entries to replay"
        );

        let replayed = Game::replay(game.initial_position().clone(), game.entries()).unwrap();
        assert_eq!(replayed.position(), game.position());
        assert_eq!(replayed.entries(), game.entries());
        assert_eq!(replayed.outcome(), game.outcome());
        assert_eq!(replayed.needs(), game.needs());
    }

    #[test]
    fn a_record_that_moves_without_throwing_is_refused() {
        let entries = [Entry::Move(Move {
            from: Spot::Waiting,
            to: Spot::Step(1),
        })];
        let error = Game::replay(Position::starting_with(Side::White), &entries).unwrap_err();
        assert!(matches!(error, PlayError::Unreadable(_)), "got {error:?}");
    }

    #[test]
    fn an_entry_survives_the_wire() {
        let cases = [
            Entry::Throw(throw(0)),
            Entry::Throw(throw(4)),
            Entry::Move(Move {
                from: Spot::Waiting,
                to: Spot::Step(4),
            }),
            Entry::Move(Move {
                from: Spot::Step(14),
                to: Spot::Off,
            }),
        ];
        for entry in cases {
            let wire = entry.to_wire();
            assert!(
                !wire.contains(' '),
                "an entry must be one token, got {wire:?}"
            );
            assert_eq!(Entry::parse(&wire), Some(entry), "{wire:?} did not survive");
        }
    }

    #[test]
    fn rubbish_does_not_parse_as_an_entry() {
        for text in [
            "", " ", "5", "16-0", "0-16", "x", "pass ", "PASS", "1-2-3", "--",
        ] {
            assert_eq!(Entry::parse(text), None, "{text:?} should not parse");
        }
    }
}
