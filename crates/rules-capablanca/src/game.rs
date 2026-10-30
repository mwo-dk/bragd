//! A Game: a position, the moves that reached it, and the endings a position
//! alone cannot decide.
//!
//! [`crate::position::Position`] answers everything that is true of a board.
//! Repetition and the move-count rule are not: they are facts about a *history*,
//! and this is the layer that has one.
//!
//! Its own type rather than `crates/rules-chess`'s for the same reason as everything
//! else in this crate — that one is generic over a `Variant` whose positions are
//! sixty-four squares wide. What is shared instead is the *shape*: the same
//! division between claimable and automatic draws, the same thresholds, and the
//! same rule that a move arriving on the wire is matched against the legal moves
//! rather than parsed. A client is not trusted.
//!
//! **No SAN yet, deliberately.** The wire carries long algebraic, which is what
//! a Library already stores for finished Games. Chess's SAN disambiguation has
//! more files to disambiguate across on a ten-file board, and getting it wrong
//! would produce a move list that is wrong in a way nobody notices; a front end
//! showing `e2e4` where it could show `e4` is a smaller loss and an honest one.

use crate::chess_move::{Move, MoveList};
use crate::outcome::{DrawReason, Outcome, WinReason, outcome as position_outcome};
use crate::piece::Color;
use crate::position::Position;
use crate::zobrist;

/// Five occurrences end the Game whether anybody asks or not.
const AUTOMATIC_REPETITIONS: usize = 5;
/// Three make it claimable.
const CLAIMABLE_REPETITIONS: usize = 3;
/// A hundred and fifty half-moves — seventy-five full moves — with no capture
/// and no pawn move ends it automatically.
const AUTOMATIC_HALFMOVES: u16 = 150;
/// Half that is claimable: the fifty-move rule.
const CLAIMABLE_HALFMOVES: u16 = 100;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawClaim {
    ThreefoldRepetition,
    FiftyMoveRule,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MoveError {
    /// Carries the outcome, because the caller almost always wants to say what
    /// the result was rather than merely that there was one.
    AlreadyOver(Outcome),
    NotLegal(Move),
    NotUnderstood(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClaimError {
    AlreadyOver(Outcome),
    NotAvailable(DrawClaim),
}

#[derive(Clone, Debug)]
pub struct Game {
    initial: Position,
    position: Position,
    moves: Vec<Move>,
    /// One key per position, `keys[0]` being the initial one — so it is always
    /// exactly one longer than `moves`.
    keys: Vec<u64>,
    outcome: Option<Outcome>,
}

impl Game {
    pub fn start() -> Game {
        Game::from_position(Position::starting())
    }

    pub fn from_position(position: Position) -> Game {
        // A Game handed a finished position is finished, rather than a Game
        // that ends on the first move somebody tries to make.
        let outcome = position_outcome(&position);
        Game {
            initial: position,
            position,
            moves: Vec::new(),
            keys: vec![zobrist::key(&position)],
            outcome,
        }
    }

    // --- Reading -------------------------------------------------------------

    pub fn position(&self) -> &Position {
        &self.position
    }

    pub fn initial_position(&self) -> &Position {
        &self.initial
    }

    pub fn moves(&self) -> &[Move] {
        &self.moves
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    pub fn is_over(&self) -> bool {
        self.outcome.is_some()
    }

    pub fn side_to_move(&self) -> Color {
        self.position.side_to_move()
    }

    /// The legal moves, or none at all once the Game is over.
    ///
    /// Returning an empty list rather than the position's moves is what stops a
    /// resigned Game offering a board somebody can still play on.
    pub fn legal_moves(&self) -> MoveList {
        if self.is_over() {
            return MoveList::new();
        }
        self.position.legal_moves()
    }

    /// How many times the current position has occurred, itself included.
    pub fn repetition_count(&self) -> usize {
        let key = *self.keys.last().expect("a game always has one key");
        self.keys.iter().filter(|&&other| other == key).count()
    }

    pub fn available_claims(&self) -> Vec<DrawClaim> {
        if self.is_over() {
            return Vec::new();
        }
        let mut claims = Vec::new();
        if self.repetition_count() >= CLAIMABLE_REPETITIONS {
            claims.push(DrawClaim::ThreefoldRepetition);
        }
        if self.position.halfmove_clock() >= CLAIMABLE_HALFMOVES {
            claims.push(DrawClaim::FiftyMoveRule);
        }
        claims
    }

    // --- Playing -------------------------------------------------------------

    pub fn play(&mut self, mv: Move) -> Result<(), MoveError> {
        if let Some(outcome) = self.outcome {
            return Err(MoveError::AlreadyOver(outcome));
        }
        if !self.position.legal_moves().contains(mv) {
            return Err(MoveError::NotLegal(mv));
        }

        self.position = self.position.make_move(mv);
        self.moves.push(mv);
        self.keys.push(zobrist::key(&self.position));
        self.outcome = self.automatic_outcome();
        Ok(())
    }

    /// Plays a move given in long algebraic, as it arrives on the wire.
    ///
    /// Matched against the legal moves rather than parsed into one, which is
    /// what makes this safe to call on anything a client sends: an unrecognised
    /// string cannot become a move, only an error.
    pub fn play_uci(&mut self, uci: &str) -> Result<(), MoveError> {
        if let Some(outcome) = self.outcome {
            return Err(MoveError::AlreadyOver(outcome));
        }
        match self
            .position
            .legal_moves()
            .into_iter()
            .find(|mv| mv.to_uci() == uci)
        {
            Some(mv) => self.play(mv),
            None => Err(MoveError::NotUnderstood(uci.to_string())),
        }
    }

    pub fn claim_draw(&mut self, claim: DrawClaim) -> Result<(), ClaimError> {
        if let Some(outcome) = self.outcome {
            return Err(ClaimError::AlreadyOver(outcome));
        }
        if !self.available_claims().contains(&claim) {
            return Err(ClaimError::NotAvailable(claim));
        }
        // The **claimed** spellings, which are not the automatic ones. A
        // replay of these moves reaches a game that is still being played with
        // a claim on offer, so the ending has to be stored rather than derived,
        // and `Outcome::is_by_the_rules` is what says so.
        self.outcome = Some(Outcome::Draw {
            reason: match claim {
                DrawClaim::ThreefoldRepetition => DrawReason::ThreefoldRepetition,
                DrawClaim::FiftyMoveRule => DrawReason::FiftyMoveRule,
            },
        });
        Ok(())
    }

    /// Ends the Game for a reason that is not a move — resignation, a flag, an
    /// agreed draw. Refuses if it is already over, returning the outcome it
    /// already has, because the first ending is the real one.
    pub fn end(&mut self, outcome: Outcome) -> Result<(), Outcome> {
        if let Some(already) = self.outcome {
            return Err(already);
        }
        self.outcome = Some(outcome);
        Ok(())
    }

    pub fn resign(&mut self, resigning: Color) -> Result<(), Outcome> {
        self.end(Outcome::Win {
            winner: resigning.opponent(),
            reason: WinReason::Resignation,
        })
    }

    /// A flag-fall. **A draw rather than a loss when the opponent could not
    /// have mated**, which is the rule people are most surprised by and the
    /// reason this asks the position rather than simply awarding the win.
    pub fn flag(&mut self, flagged: Color) -> Result<(), Outcome> {
        let opponent = flagged.opponent();
        if crate::outcome::can_possibly_mate(&self.position, opponent) {
            self.end(Outcome::Win {
                winner: opponent,
                reason: WinReason::Flag,
            })
        } else {
            // **A Clock drew this, not the board.** The material happens to be
            // insufficient, but the ending is the flag — nothing in the move
            // list says time ran out, so this must not be spelled the same way
            // as a board that is dead on its own.
            self.end(Outcome::Draw {
                reason: DrawReason::FlagWithInsufficientMaterial,
            })
        }
    }

    fn automatic_outcome(&self) -> Option<Outcome> {
        if let Some(outcome) = position_outcome(&self.position) {
            return Some(outcome);
        }
        if self.repetition_count() >= AUTOMATIC_REPETITIONS {
            return Some(Outcome::Draw {
                reason: DrawReason::FivefoldRepetition,
            });
        }
        if self.position.halfmove_clock() >= AUTOMATIC_HALFMOVES {
            return Some(Outcome::Draw {
                reason: DrawReason::SeventyFiveMoveRule,
            });
        }
        None
    }
}

impl Default for Game {
    fn default() -> Game {
        Game::start()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Knights out and back, which returns the exact starting position —
    /// castling rights and all — every four plies. The files are b and i
    /// because this board has ten of them.
    const SHUFFLE: [&str; 4] = ["b1c3", "b8c6", "c3b1", "c6b8"];

    fn play_all(game: &mut Game, moves: impl IntoIterator<Item = &'static str>) {
        for uci in moves {
            game.play_uci(uci)
                .unwrap_or_else(|e| panic!("{uci}: {e:?}"));
        }
    }

    #[test]
    fn a_new_game_is_the_opening_position_and_is_not_over() {
        let game = Game::start();
        assert_eq!(game.position().to_fen(), crate::position::STARTING_POSITION);
        assert!(!game.is_over());
        assert_eq!(game.legal_moves().len(), 28);
        assert_eq!(game.repetition_count(), 1);
    }

    #[test]
    fn a_move_that_is_not_legal_is_refused_rather_than_played() {
        let mut game = Game::start();
        assert!(matches!(
            game.play_uci("e2e5"),
            Err(MoveError::NotUnderstood(_))
        ));
        assert_eq!(game.moves().len(), 0);
    }

    #[test]
    fn repetition_is_counted_across_the_history() {
        let mut game = Game::start();
        assert_eq!(game.repetition_count(), 1);
        play_all(&mut game, SHUFFLE);
        assert_eq!(game.repetition_count(), 2);
        play_all(&mut game, SHUFFLE);
        assert_eq!(game.repetition_count(), 3);
        assert_eq!(
            game.available_claims(),
            vec![DrawClaim::ThreefoldRepetition]
        );
        assert!(!game.is_over(), "claimable is not automatic");
    }

    #[test]
    fn the_fifth_repetition_ends_it_without_anybody_asking() {
        let mut game = Game::start();
        for _ in 0..4 {
            play_all(&mut game, SHUFFLE);
        }
        assert_eq!(game.repetition_count(), 5);
        assert_eq!(
            game.outcome(),
            Some(Outcome::Draw {
                reason: DrawReason::FivefoldRepetition
            })
        );
        assert!(
            game.outcome().expect("an ending").is_by_the_rules(),
            "nobody had to ask for it, so a replay finds it too"
        );
        assert!(game.legal_moves().is_empty(), "a finished game offers none");
    }

    #[test]
    fn a_claim_can_be_taken_and_only_when_it_is_available() {
        let mut game = Game::start();
        assert_eq!(
            game.claim_draw(DrawClaim::ThreefoldRepetition),
            Err(ClaimError::NotAvailable(DrawClaim::ThreefoldRepetition))
        );
        play_all(&mut game, SHUFFLE);
        play_all(&mut game, SHUFFLE);
        assert!(game.claim_draw(DrawClaim::ThreefoldRepetition).is_ok());
        assert_eq!(
            game.outcome(),
            Some(Outcome::Draw {
                reason: DrawReason::ThreefoldRepetition
            })
        );
        // The claim is the ending, and a claim is not a move. Replaying these
        // moves gives a game still being played with the claim on offer, so
        // the stored outcome has to win over the replayed one.
        assert!(!game.outcome().expect("an ending").is_by_the_rules());
    }

    #[test]
    fn resigning_gives_it_to_the_other_side_and_only_once() {
        let mut game = Game::start();
        assert!(game.resign(Color::White).is_ok());
        assert_eq!(
            game.outcome(),
            Some(Outcome::Win {
                winner: Color::Black,
                reason: WinReason::Resignation
            })
        );
        // The first ending is the real one.
        assert!(game.resign(Color::Black).is_err());
        assert!(matches!(
            game.play_uci("e2e4"),
            Err(MoveError::AlreadyOver(_))
        ));
    }

    /// A flag against somebody who could not have mated is a draw, not a win.
    #[test]
    fn a_flag_is_a_draw_when_the_opponent_could_not_have_mated() {
        let bare = Game::from_position(
            Position::from_fen("5k4/10/10/10/10/10/10/5K4 w - - 0 1").expect("legal"),
        );
        // Two lone kings is already a draw by material, so the Game is over
        // before the flag — which is itself the right answer.
        assert!(bare.is_over());

        // Black has a rook and White only a knight, so the Game is live —
        // Black could still mate — and White could not have. This is the
        // position where the rule actually does something.
        let mut thin = Game::from_position(
            Position::from_fen("4rk4/10/10/10/10/10/10/5K3N w - - 0 1").expect("legal"),
        );
        assert!(!thin.is_over(), "Black can still mate, so it is not dead");
        assert!(thin.flag(Color::Black).is_ok());
        assert_eq!(
            thin.outcome(),
            Some(Outcome::Draw {
                reason: DrawReason::FlagWithInsufficientMaterial
            }),
            "White has only a knight, so Black's flag is a draw"
        );
        // Spelled apart from a board that is dead on its own, because a Clock
        // decided this one and no Clock is in the move list.
        assert!(!thin.outcome().expect("an ending").is_by_the_rules());
    }

    #[test]
    fn a_flag_against_a_side_that_could_mate_is_a_loss() {
        let mut game = Game::start();
        assert!(game.flag(Color::Black).is_ok());
        assert_eq!(
            game.outcome(),
            Some(Outcome::Win {
                winner: Color::White,
                reason: WinReason::Flag
            })
        );
    }

    /// A Library stores a starting position and a move list, so a Game has to
    /// be reconstructible from exactly that.
    #[test]
    fn a_game_replays_from_its_moves() {
        let mut played = Game::start();
        play_all(&mut played, SHUFFLE);
        play_all(&mut played, ["e2e4", "e7e5", "d2d4"]);

        let mut replayed = Game::from_position(*played.initial_position());
        for mv in played.moves() {
            replayed.play(*mv).expect("a played move replays");
        }

        assert_eq!(replayed.position().to_fen(), played.position().to_fen());
        assert_eq!(replayed.repetition_count(), played.repetition_count());
        assert_eq!(replayed.outcome(), played.outcome());
    }
}
