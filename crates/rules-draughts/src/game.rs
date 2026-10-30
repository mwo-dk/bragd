//! A game, and how it ends.
//!
//! Endings here are simpler than tafl's and stranger than chess's. **A player
//! with no legal move loses** — every piece captured, or every piece blocked,
//! is the same result. There is no stalemate-is-a-draw rule, and porting that
//! intuition from chess is the common bug.
//!
//! Draws are the part that genuinely diverges between variants, and the part
//! this engine deliberately does not pretend to settle. International has FMJD
//! counters for king-only endings; English has its own forty-move convention;
//! Russian and Italian have others again. They are **not** shared, and none of
//! them is a rule about the position — they are all about how long something
//! has gone on. So what is here is one honest counter and a name that says it
//! is ours.

use crate::position::{Move, Position, Side};
use crate::ruleset::Ruleset;
use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// The side that ran out of moves is the loser, always.
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
    /// Every piece taken.
    NoPieces,
    /// Pieces left, none of them able to move. The same result, and worth
    /// telling apart because it is the one people find surprising.
    Blocked,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawReason {
    /// The threefold rule. Positions repeat far more readily here than in
    /// chess once both sides are down to kings.
    Repetition,
    /// Too long without a capture or a man moving.
    ///
    /// **This engine's counter, not a federation's.** The real rules differ
    /// per variant and are about king-only endings with their own move counts;
    /// this is one number that terminates a game, and calling it what it is
    /// beats implementing one federation's rule and labelling it the others'.
    Stalled,
    Agreement,
}

/// How many plies without a capture or a man's move before a game is drawn.
///
/// Fifty moves each. Chosen to be clearly longer than any real manoeuvre and
/// clearly finite; it is engine policy, and [`DrawReason::Stalled`] says so.
pub const STALL_LIMIT: usize = 100;

#[derive(Clone)]
pub struct Game {
    rules: Ruleset,
    position: Position,
    seen: Vec<Position>,
    moves: Vec<Move>,
    /// Plies since the last capture or man's move.
    quiet: usize,
    outcome: Option<Outcome>,
}

impl Game {
    pub fn new(rules: Ruleset) -> Game {
        Game::from_position(rules, Position::start(rules.geometry))
    }

    pub fn from_position(rules: Ruleset, position: Position) -> Game {
        let mut game = Game {
            rules,
            position,
            seen: vec![position],
            moves: Vec::new(),
            quiet: 0,
            outcome: None,
        };
        game.outcome = game.decide();
        game
    }

    pub fn position(&self) -> &Position {
        &self.position
    }

    pub fn rules(&self) -> &Ruleset {
        &self.rules
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    pub fn moves(&self) -> &[Move] {
        &self.moves
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        if self.outcome.is_some() {
            return Vec::new();
        }
        self.position.legal_moves(&self.rules)
    }

    pub fn play(&mut self, mv: Move) -> Result<(), MoveError> {
        if let Some(outcome) = self.outcome {
            return Err(MoveError::AlreadyOver(outcome));
        }
        if !self.legal_moves().contains(&mv) {
            return Err(MoveError::NotLegal(mv));
        }
        // A capture or a man's move makes the position irreversible, which is
        // what the stall counter is really measuring.
        let irreversible = mv.is_capture() || !self.position.is_king(mv.from);
        self.position = self.position.play(&self.rules, mv);
        self.moves.push(mv);
        if irreversible {
            self.quiet = 0;
            self.seen.clear();
        } else {
            self.quiet += 1;
        }
        self.seen.push(self.position);
        self.outcome = self.decide();
        Ok(())
    }

    fn decide(&self) -> Option<Outcome> {
        let to_move = self.position.to_move();
        if self.position.side(to_move).is_empty() {
            return Some(Outcome::Win {
                winner: to_move.other(),
                reason: WinReason::NoPieces,
            });
        }
        if self.position.legal_moves(&self.rules).is_empty() {
            // Pieces on the board and nowhere to go. A loss, not a draw — the
            // opposite of chess, and the bug people port from it.
            return Some(Outcome::Win {
                winner: to_move.other(),
                reason: WinReason::Blocked,
            });
        }
        if self
            .seen
            .iter()
            .filter(|&&seen| seen == self.position)
            .count()
            >= 3
        {
            return Some(Outcome::Draw {
                reason: DrawReason::Repetition,
            });
        }
        if self.quiet >= STALL_LIMIT {
            return Some(Outcome::Draw {
                reason: DrawReason::Stalled,
            });
        }
        None
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MoveError {
    AlreadyOver(Outcome),
    NotLegal(Move),
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoveError::AlreadyOver(outcome) => write!(f, "the game is over: {outcome:?}"),
            MoveError::NotLegal(mv) => write!(f, "{mv:?} is not legal here"),
        }
    }
}

impl std::error::Error for MoveError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruleset::{ENGLISH, INTERNATIONAL};

    #[test]
    fn a_new_game_has_moves_and_no_outcome() {
        for rules in [INTERNATIONAL, ENGLISH] {
            let game = Game::new(rules);
            assert_eq!(game.outcome(), None);
            assert!(!game.legal_moves().is_empty());
            assert_eq!(game.position().to_move(), Side::White);
        }
    }

    #[test]
    fn a_game_always_ends_under_random_play() {
        // The termination guarantee. Without it a server hangs on two kings
        // shuffling, which draughts reaches far more readily than chess.
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..20 {
            let mut game = Game::new(ENGLISH);
            let mut plies = 0;
            while game.outcome().is_none() {
                let moves = game.legal_moves();
                assert!(!moves.is_empty(), "no moves and no outcome");
                let index = (next() % moves.len() as u64) as usize;
                game.play(moves[index]).expect("legal");
                plies += 1;
                assert!(plies < 5_000, "no outcome after {plies} plies");
            }
        }
    }

    #[test]
    fn having_no_move_is_a_loss_and_not_a_draw() {
        // The opposite of chess, and the bug ported from it. Reached by play
        // rather than constructed, because a constructed position would be
        // asserting what the generator already believes.
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut blocked = 0;
        for _ in 0..200 {
            let mut game = Game::new(ENGLISH);
            while game.outcome().is_none() {
                let moves = game.legal_moves();
                let index = (next() % moves.len() as u64) as usize;
                game.play(moves[index]).expect("legal");
            }
            if matches!(
                game.outcome(),
                Some(Outcome::Win {
                    reason: WinReason::Blocked,
                    ..
                })
            ) {
                blocked += 1;
            }
            // Whatever the reason, somebody won or it was drawn — never a
            // "stalemate" in between.
            assert!(game.outcome().is_some());
        }
        assert!(blocked > 0, "no game ended by blockade in 200 tries");
    }

    #[test]
    fn both_sides_win_sometimes() {
        // Not a rules check — a check that both win conditions are reachable.
        // An engine where one side can never win passes every other test here.
        let mut state = 0xBF58_476D_1CE4_E5B9u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut wins = [0, 0];
        for _ in 0..120 {
            let mut game = Game::new(ENGLISH);
            while game.outcome().is_none() {
                let moves = game.legal_moves();
                let index = (next() % moves.len() as u64) as usize;
                game.play(moves[index]).expect("legal");
            }
            if let Some(Outcome::Win { winner, .. }) = game.outcome() {
                wins[winner.index()] += 1;
            }
        }
        assert!(wins[0] > 0 && wins[1] > 0, "one-sided: {wins:?}");
    }

    #[test]
    fn material_never_grows() {
        let mut state = 0x94D0_49BB_1331_11EBu64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..20 {
            let mut game = Game::new(INTERNATIONAL);
            let mut last = game.position().occupied().count();
            while game.outcome().is_none() {
                let moves = game.legal_moves();
                let index = (next() % moves.len() as u64) as usize;
                game.play(moves[index]).expect("legal");
                let now = game.position().occupied().count();
                assert!(now <= last, "material grew from {last} to {now}");
                last = now;
            }
        }
    }
}
