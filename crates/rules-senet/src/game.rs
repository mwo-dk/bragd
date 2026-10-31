//! The record, and the game built from it.
//!
//! **A Game is its entries**, and an entry is a throw or a move. They alternate
//! only loosely: a one, a four or a five earns another throw, so one side can own
//! six entries in a row, and a throw that nothing can use is followed by no move
//! at all.
//!
//! One entry per supplied input, always. That is not a style choice — the
//! caller hands the rules **one string at a time**, so a crate that turned one throw into two entries would leave the stored record
//! permanently out of step with its own history. Ur's engine was written that way
//! first and the note is in `crates/rules-ur/src/game.rs`.

use crate::board::Side;
use crate::position::{Move, Offer, Position};
use crate::sticks::Throw;

/// One line of the record.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Entry {
    /// What the platform threw.
    Throw(Throw),
    /// What the player did with it.
    Move(Move),
}

impl Entry {
    /// The wire spelling. One entry, one token, no spaces.
    pub fn to_wire(self) -> String {
        match self {
            Entry::Throw(throw) => throw.to_wire(),
            Entry::Move(chosen) => chosen.to_wire(),
        }
    }

    /// Reads an entry back.
    ///
    /// A throw is a bare digit and a move always contains a dash, so the two can
    /// never be confused — which is worth stating, because senet's throws run to
    /// 5 and its houses to 30, and a scheme where `5` might have been a house
    /// would have been ambiguous from the first entry.
    pub fn parse(text: &str) -> Option<Entry> {
        if let Some(throw) = Throw::parse(text) {
            return Some(Entry::Throw(throw));
        }
        Move::parse(text).map(Entry::Move)
    }
}

/// What the game is waiting for.
///
/// The engine answers this rather than the platform inferring it from the shape
/// of the record: throw twice and a player chooses with the wrong number showing,
/// throw never and the Game hangs with nothing logged.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Needed {
    Throw { side: Side },
    Move { side: Side },
    Nothing,
}

impl Needed {
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
    /// All seven pieces off the board.
    Home,
    Resignation,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    pub winner: Side,
    pub reason: WinReason,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlayError {
    Over,
    OutOfTurn,
    Illegal,
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

/// A whole game.
#[derive(Clone, Debug)]
pub struct Game {
    initial: Position,
    position: Position,
    entries: Vec<Entry>,
    pending: Option<Throw>,
    outcome: Option<Outcome>,
}

impl Game {
    /// A new game from the opening array.
    ///
    /// **No seating parameter, unlike Ur's and backgammon's.** Senet's opening
    /// array decides who moves first all by itself — the player on the even
    /// houses starts — so who began is a fact about the *position* rather than
    /// about the Game, and there is nothing for the platform to store.
    pub fn new() -> Game {
        Game::from_position(Position::starting())
    }

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

    pub fn pending_throw(&self) -> Option<Throw> {
        self.pending
    }

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

    /// The moves the throw on the table allows.
    pub fn moves(&self) -> Vec<Offer> {
        self.pending
            .map(|throw| self.position.moves_for(throw))
            .unwrap_or_default()
    }

    /// Records a throw.
    ///
    /// A throw nothing can use hands the turn over in the same call and writes
    /// nothing extra. **The repeat is forfeited in that case**, which is Kendall's
    /// "after otherwise moving": a one that moved nothing does not earn another
    /// one, and this is the only place that distinction can be made, because the
    /// throw alone cannot know whether anything moved.
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

        self.position.play(legal.chosen, throw);
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

    pub fn play_entry(&mut self, entry: Entry) -> Result<(), PlayError> {
        match entry {
            Entry::Throw(throw) => self.throw(throw),
            Entry::Move(chosen) => self.play(chosen),
        }
    }

    pub fn play_wire(&mut self, text: &str) -> Result<(), PlayError> {
        let entry = Entry::parse(text)
            .ok_or_else(|| PlayError::Unreadable(format!("cannot read {text:?} as an entry")))?;
        self.play_entry(entry)
    }

    /// Ends the game by concession.
    ///
    /// Deliberately not an entry, as tafl's and shogi's are not: nothing happened
    /// on the board, so a conceded game replays back as a game still in progress
    /// and the stored outcome is what settles it.
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

    /// Rebuilds a game from its record, entry by entry.
    ///
    /// The engine invents nothing during a replay, which is what keeps the stored
    /// record and the engine's own history the same list.
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

    /// How far each side still has to travel.
    pub fn distances(&self) -> (u16, u16) {
        (
            self.position.distance_left(Side::White),
            self.position.distance_left(Side::Black),
        )
    }

    /// How many pieces each side has borne off.
    pub fn home(&self) -> (u8, u8) {
        (
            self.position.off(Side::White),
            self.position.off(Side::Black),
        )
    }
}

impl Default for Game {
    fn default() -> Game {
        Game::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::PIECES;
    use crate::position::{Spot, Way};

    fn throw(value: u8) -> Throw {
        Throw::new(value).unwrap()
    }

    #[test]
    fn a_new_game_waits_for_a_throw_from_the_side_on_the_even_houses() {
        let game = Game::new();
        assert_eq!(game.needs(), Needed::Throw { side: Side::White });
        assert!(game.moves().is_empty());
        assert!(!game.is_over());
        assert_eq!(game.home(), (0, 0));
        // Every piece is on the board, so both sides owe the same distance.
        let (white, black) = game.distances();
        assert!(white > 0 && black > 0);
    }

    #[test]
    fn a_throw_puts_the_game_in_want_of_a_move() {
        let mut game = Game::new();
        game.throw(throw(1)).unwrap();
        assert_eq!(game.needs(), Needed::Move { side: Side::White });
        assert_eq!(game.pending_throw(), Some(throw(1)));
        assert!(!game.moves().is_empty());
        assert_eq!(game.entries(), &[Entry::Throw(throw(1))]);
    }

    #[test]
    fn throwing_twice_and_moving_early_are_both_refused() {
        let mut game = Game::new();
        game.throw(throw(1)).unwrap();
        assert_eq!(game.throw(throw(2)), Err(PlayError::OutOfTurn));

        let mut fresh = Game::new();
        let chosen = Move {
            from: 14,
            to: Spot::House(15),
            way: Way::Forward,
        };
        assert_eq!(fresh.play(chosen), Err(PlayError::OutOfTurn));
    }

    #[test]
    fn an_illegal_move_is_refused_and_writes_nothing() {
        let mut game = Game::new();
        game.throw(throw(1)).unwrap();
        // House 2 is white's own, and 3 is black's protected piece; either way
        // this exact move is not on offer with a throw of one from 1 — house 1 is
        // black's, not white's.
        let wrong = Move {
            from: 1,
            to: Spot::House(2),
            way: Way::Forward,
        };
        assert_eq!(game.play(wrong), Err(PlayError::Illegal));
        assert_eq!(game.entries().len(), 1, "a refusal writes nothing");
        assert_eq!(game.needs(), Needed::Move { side: Side::White });
    }

    #[test]
    fn a_one_earns_another_throw_and_a_two_does_not() {
        let mut game = Game::new();
        game.throw(throw(1)).unwrap();
        let chosen = game.moves()[0].chosen;
        game.play(chosen).unwrap();
        assert_eq!(
            game.needs(),
            Needed::Throw { side: Side::White },
            "a one repeats"
        );

        let mut game = Game::new();
        game.throw(throw(2)).unwrap();
        let chosen = game.moves()[0].chosen;
        game.play(chosen).unwrap();
        assert_eq!(
            game.needs(),
            Needed::Throw { side: Side::Black },
            "a two ends the turn"
        );
    }

    #[test]
    fn a_throw_nothing_can_use_is_one_entry_and_hands_over() {
        // Contrived, and it took two goes. White has one piece left, on 29, which
        // only an exact two can move forward. **Blocking the forward move is not
        // enough** — with nothing forward available a backward move becomes
        // compulsory, so 29 − 5 = 24 has to be blocked as well, which is what the
        // protected black pair on 23 and 24 is for. The first version of this
        // fixture forgot that and the engine was right to disagree.
        let picture = {
            let mut board = vec!['.'; 30];
            board[29 - 1] = 'W';
            for house in [23usize, 24, 27, 28, 1, 3, 5] {
                board[house - 1] = 'B';
            }
            format!("{} 60 w", board.into_iter().collect::<String>())
        };
        let position = Position::from_picture(&picture).expect("the fixture should read");
        let mut game = Game::from_position(position);

        assert_eq!(game.needs(), Needed::Throw { side: Side::White });
        game.throw(throw(5)).unwrap();
        assert_eq!(
            game.entries(),
            &[Entry::Throw(throw(5))],
            "a throw is one entry, because the platform supplies one string"
        );
        assert_eq!(
            game.needs(),
            Needed::Throw { side: Side::Black },
            "and a five that moved nothing earns no repeat"
        );
    }

    #[test]
    fn a_resignation_ends_it_and_is_not_an_entry() {
        let mut game = Game::new();
        game.throw(throw(2)).unwrap();
        let chosen = game.moves()[0].chosen;
        game.play(chosen).unwrap();
        let before = game.entries().len();
        game.resign(Side::White).unwrap();
        assert_eq!(game.entries().len(), before);
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
    fn bringing_the_last_piece_home_ends_it() {
        let picture = {
            let mut board = vec!['.'; 30];
            board[30 - 1] = 'W';
            for house in [1usize, 3, 5, 7, 9, 11, 13] {
                board[house - 1] = 'B';
            }
            format!("{} 60 w", board.into_iter().collect::<String>())
        };
        let mut game = Game::from_position(Position::from_picture(&picture).unwrap());
        game.throw(throw(3)).unwrap();
        let home = game
            .moves()
            .into_iter()
            .find(|offer| offer.bears_off)
            .expect("30 leaves on anything");
        game.play(home.chosen).unwrap();
        assert!(game.is_over());
        assert_eq!(game.home(), (PIECES, 0));
        assert_eq!(game.needs(), Needed::Nothing);
        assert_eq!(game.throw(throw(2)), Err(PlayError::Over));
    }

    #[test]
    fn an_entry_survives_the_wire_and_rubbish_does_not() {
        let cases = [
            Entry::Throw(throw(1)),
            Entry::Throw(throw(5)),
            Entry::Move(Move {
                from: 14,
                to: Spot::House(15),
                way: Way::Forward,
            }),
            Entry::Move(Move {
                from: 12,
                to: Spot::House(9),
                way: Way::Backward,
            }),
            Entry::Move(Move {
                from: 30,
                to: Spot::Off,
                way: Way::Forward,
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
        for text in ["", " ", "0", "6", "31-4", "4-31", "x", "1-2-3", "--", "<"] {
            assert_eq!(Entry::parse(text), None, "{text:?} should not parse");
        }
    }

    #[test]
    fn a_game_replays_from_its_entries() {
        let mut game = Game::new();
        for value in [1u8, 2, 3, 1, 4, 2, 5, 3] {
            if game.is_over() {
                break;
            }
            game.throw(throw(value)).unwrap();
            if let Needed::Move { .. } = game.needs() {
                let chosen = game.moves()[0].chosen;
                game.play(chosen).unwrap();
            }
        }
        assert!(
            game.entries().len() >= 8,
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
            from: 14,
            to: Spot::House(15),
            way: Way::Forward,
        })];
        let error = Game::replay(Position::starting(), &entries).unwrap_err();
        assert!(matches!(error, PlayError::Unreadable(_)), "got {error:?}");
    }
}
