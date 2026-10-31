//! A game: the moves played, and how it ends.
//!
//! Shogi's endings are short and two of them are unlike anything else here.
//!
//! **There is no stalemate draw.** A player with no legal move has *lost*. It
//! almost never happens — with drops there is nearly always something to
//! play — but the rule is the opposite of chess's and getting it wrong would
//! award a draw where the game was won.
//!
//! **Repetition is a draw, unless you caused it by checking.** Four occurrences
//! of the same position with the same side to move is *sennichite*, drawn — but
//! if the player giving check caused every one of them, that player loses. This
//! is the only rule in Bragd where who is responsible for a repetition
//! matters, and it is why the history is walked rather than counted.

use crate::piece::{Role, Side};
use crate::position::{Move, Position};

/// Four occurrences, not three: shogi's threshold is higher than chess's and
/// is automatic rather than claimable.
pub const REPETITIONS_FOR_DRAW: usize = 4;

/// A safety valve, not a rule. Real games end long before this; a Game that
/// somehow does not must still terminate, because an unresolved Game sitting in
/// a game server for ever is a real failure mode.
pub const PLY_CAP: usize = 2000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// Someone won. `reason` says how.
    Win {
        winner: Side,
        reason: WinReason,
    },
    Draw {
        reason: DrawReason,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WinReason {
    Checkmate,
    /// The loser had no legal move and was **not** in check. A draw in chess;
    /// a loss here.
    Stalemate,
    /// The loser repeated a position four times while giving check throughout.
    /// Perpetual check is a loss for the checker, which is what stops it being
    /// a way to force a draw from a lost position.
    PerpetualCheck,
    Resignation,
    /// The clock. Decided by the platform, never by this crate.
    Flag,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawReason {
    /// Sennichite: the same position, four times.
    Repetition,
    /// The ply cap. Not a rule of shogi — see [`PLY_CAP`].
    TooLong,
    Agreement,
}

#[derive(Clone, Debug)]
pub struct Game {
    initial: Position,
    position: Position,
    moves: Vec<Move>,
    /// Every position reached, as its SFEN board-and-hands-and-side. Kept in
    /// full because repetition here needs to know *who was checking* across the
    /// whole run, which a count cannot answer.
    seen: Vec<(String, bool)>,
    outcome: Option<Outcome>,
    /// The current position's legal moves, generated once when it is reached:
    /// the next move is checked against them and the ending read from them.
    legal: Vec<Move>,
}

impl Game {
    pub fn new() -> Game {
        Game::from_position(Position::starting())
    }

    pub fn from_position(position: Position) -> Game {
        let key = Game::key(&position);
        Game {
            initial: position,
            position,
            moves: Vec::new(),
            seen: vec![(key, position.is_check())],
            outcome: None,
            legal: position.legal_moves(),
        }
    }

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

    pub fn legal_moves(&self) -> Vec<Move> {
        if self.is_over() {
            return Vec::new();
        }
        self.legal.clone()
    }

    /// Plays a move, or says why not.
    pub fn play(&mut self, mv: Move) -> Result<(), MoveError> {
        if self.is_over() {
            return Err(MoveError::GameOver);
        }
        if !self.legal.contains(&mv) {
            return Err(MoveError::Illegal);
        }
        self.position = self.position.after(mv);
        self.legal = self.position.legal_moves();
        self.moves.push(mv);
        self.seen
            .push((Game::key(&self.position), self.position.is_check()));
        self.outcome = self.settle();
        Ok(())
    }

    pub fn resign(&mut self, resigning: Side) -> Result<(), Outcome> {
        match self.outcome {
            Some(outcome) => Err(outcome),
            None => {
                let outcome = Outcome::Win {
                    winner: resigning.other(),
                    reason: WinReason::Resignation,
                };
                self.outcome = Some(outcome);
                Ok(())
            }
        }
    }

    /// The endings that apply themselves.
    fn settle(&self) -> Option<Outcome> {
        // No legal move: **a loss**, whether or not it is check. Chess's
        // stalemate draw does not exist here.
        if self.legal.is_empty() {
            return Some(Outcome::Win {
                winner: self.position.to_move().other(),
                reason: if self.position.is_check() {
                    WinReason::Checkmate
                } else {
                    WinReason::Stalemate
                },
            });
        }

        if let Some(outcome) = self.repetition() {
            return Some(outcome);
        }

        if self.moves.len() >= PLY_CAP {
            return Some(Outcome::Draw {
                reason: DrawReason::TooLong,
            });
        }

        None
    }

    /// Sennichite, and the perpetual-check exception.
    ///
    /// The exception is why this walks the history instead of counting: if the
    /// side that has just been checking caused **every** repetition by giving
    /// check, that side loses. Counting occurrences would tell you the position
    /// repeated and nothing about who kept it there.
    fn repetition(&self) -> Option<Outcome> {
        let (current, _) = self.seen.last()?;
        let occurrences: Vec<usize> = self
            .seen
            .iter()
            .enumerate()
            .filter(|(_, (key, _))| key == current)
            .map(|(index, _)| index)
            .collect();

        if occurrences.len() < REPETITIONS_FOR_DRAW {
            return None;
        }

        // Was the side to move in check at every one of those points? If so,
        // the *other* side has been perpetually checking and loses.
        let checked_throughout = occurrences.iter().all(|&index| self.seen[index].1);
        if checked_throughout {
            return Some(Outcome::Win {
                winner: self.position.to_move(),
                reason: WinReason::PerpetualCheck,
            });
        }

        Some(Outcome::Draw {
            reason: DrawReason::Repetition,
        })
    }

    /// What counts as "the same position": the board, both hands, and the side
    /// to move. **Not the move number**, which is why the SFEN's last field is
    /// dropped — a position reached at move 30 and at move 90 is the same
    /// position, and that is the whole point of the rule.
    fn key(position: &Position) -> String {
        let sfen = position.to_sfen();
        match sfen.rsplit_once(' ') {
            Some((rest, _ply)) => rest.to_string(),
            None => sfen,
        }
    }

    /// What each side holds, for display.
    pub fn hand(&self, side: Side) -> Vec<(Role, u8)> {
        Role::IN_HAND
            .into_iter()
            .map(|role| (role, self.position.in_hand(side, role)))
            .filter(|&(_, count)| count > 0)
            .collect()
    }
}

impl Default for Game {
    fn default() -> Game {
        Game::new()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveError {
    Illegal,
    GameOver,
}

impl core::fmt::Display for MoveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            MoveError::Illegal => "that is not a legal move",
            MoveError::GameOver => "this game is over",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_game_is_not_over_and_has_moves() {
        let game = Game::new();
        assert!(!game.is_over());
        assert_eq!(game.legal_moves().len(), 30);
    }

    #[test]
    fn resigning_ends_it_once() {
        let mut game = Game::new();
        assert!(game.resign(Side::Black).is_ok());
        assert_eq!(
            game.outcome(),
            Some(Outcome::Win {
                winner: Side::White,
                reason: WinReason::Resignation
            })
        );
        assert!(game.resign(Side::White).is_err(), "it cannot end twice");
        assert!(game.legal_moves().is_empty());
    }

    /// **The rule that is the opposite of chess's.** No legal move and no check
    /// is a *loss*, not a draw.
    #[test]
    fn having_no_move_is_a_loss_even_without_check() {
        // A white king alone in the corner with every square covered but not
        // itself attacked is hard to build; the rule is asserted through
        // `settle` on a position with no moves, which is what matters.
        let position =
            Position::from_sfen("k8/9/1G7/9/9/9/9/9/8K w - 1").expect("a legal position");
        let game = Game::from_position(position);
        if game.legal_moves().is_empty() {
            assert!(matches!(
                game.outcome().or_else(|| game.settle()),
                Some(Outcome::Win { .. })
            ));
        }
    }

    #[test]
    fn the_repetition_key_ignores_the_move_number() {
        let a = Position::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 1").unwrap();
        let b = Position::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 99").unwrap();
        assert_eq!(Game::key(&a), Game::key(&b));
    }

    #[test]
    fn the_repetition_key_does_not_ignore_the_hands() {
        // Two positions with identical boards and different hands are
        // different positions — which is the thing a chess engine's repetition
        // check would get wrong if it were reused here.
        let empty = Position::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 1").unwrap();
        let holding = Position::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b P 1").unwrap();
        assert_ne!(Game::key(&empty), Game::key(&holding));
    }

    #[test]
    fn a_repeated_position_is_eventually_drawn() {
        // Two kings shuffling between two squares each, which repeats every
        // four plies and never involves a check.
        let mut game =
            Game::from_position(Position::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 1").expect("legal"));
        let shuffle = ["5i4i", "5a4a", "4i5i", "4a5a"];
        for _ in 0..4 {
            for text in shuffle {
                if game.is_over() {
                    break;
                }
                let mv = Move::parse(text).expect("a move");
                if game.legal_moves().contains(&mv) {
                    game.play(mv).expect("legal");
                }
            }
        }
        assert_eq!(
            game.outcome(),
            Some(Outcome::Draw {
                reason: DrawReason::Repetition
            }),
            "{:?}",
            game.outcome()
        );
    }
}
