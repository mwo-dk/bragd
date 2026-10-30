//! A game, and how it ends.
//!
//! Two endings are ordinary and one is not.
//!
//! Ordinary: **checkmate loses, and so does stalemate.** A player with no legal
//! move loses in xiangqi, as in draughts and tafl and unlike chess. Porting
//! chess's intuition here is the common bug.
//!
//! Not ordinary: **perpetual check loses.** WXF Article 20.1 — "any player who
//! delivers perpetual checks would be dealt with a loss" — which inverts what
//! every chess player expects, where perpetual check is the classic way to
//! save a lost position. It is not a detail: without it a losing player draws
//! every game they can find one check in, and the game is unplayable against a
//! stranger.
//!
//! **Perpetual chase is not implemented**, and that is stated rather than
//! hidden. It is the same idea applied to threatening a piece rather than the
//! general, and it needs a classification of every repeated move into check,
//! chase, threat or idle — for which the WXF's own conformance material is 110
//! worked diagrams. Until those are transcribed, a repetition that is not a
//! perpetual check is scored as a draw, which is what most engines and most
//! online sites do.

//!
//! # Janggi ends differently
//!
//! Three of the endings above do not survive the crossing, and one ending
//! exists that xiangqi has no word for:
//!
//! - **Stalemate cannot happen**, because passing is legal. A player who is not
//!   in check always has at least the pass, so an empty move list means
//!   checkmate and nothing else.
//! - **Bikjang** — the two generals facing each other down an open file — is a
//!   legal position that ends the game. Xiangqi forbids the position; janggi
//!   scores it.
//! - **A game that runs out is ranked, not halved.** Janggi counts material and
//!   the higher score wins, so a repetition or the quiet limit produces a
//!   winner rather than a draw.
//! - **Perpetual check still loses for the checker**, which is the one hard
//!   ending the two games agree about, so that adjudication is shared.

use crate::position::{Move, Position, Side};
use crate::ruleset::Ruleset;
use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Win { winner: Side, reason: WinReason },
    Draw { reason: DrawReason },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WinReason {
    Checkmate,
    /// No legal move, and not in check. **A loss**, not a draw.
    ///
    /// Xiangqi only. Under janggi it is unreachable, because a player who is
    /// not in check may always pass.
    Stalemate,
    /// The loser repeated a position while checking throughout (WXF 20.1).
    PerpetualCheck,
    /// **Janggi only.** The game ran out — by repetition or by the quiet limit
    /// — and was ranked on material.
    ///
    /// Janggi does not halve a point between two players; it counts what each
    /// has left. Han's 1.5-point handicap means the count can never tie, so
    /// this is always a win and never a draw.
    Points,
    Resignation,
    Flag,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawReason {
    /// A repetition that was not a perpetual check.
    ///
    /// **Approximate, and knowingly so.** The WXF adjudicates a repetition by
    /// classifying every move in the cycle — check, chase, threat, idle — and
    /// scores perpetual chase as a loss too. That classification is not
    /// implemented, so anything that is not a perpetual check lands here.
    Repetition,
    /// Both sides checking perpetually, which the WXF scores as a draw.
    MutualPerpetualCheck,
    /// Too long without a capture. Engine policy; see [`QUIET_LIMIT`].
    Stalled,
    /// **Janggi only.** 빅장 — the two generals came to face each other down an
    /// open file, with nothing between them.
    ///
    /// This is the same geometry xiangqi declares illegal, and janggi's answer
    /// to it is the game's most-argued rule. Three readings are in circulation:
    ///
    /// 1. **A draw, at once.** The reading implemented here, and the one most
    ///    casual play and most implementations use.
    /// 2. **An offer.** The player who created the face-off has offered a draw;
    ///    the player to move may accept it by passing, or decline it by
    ///    breaking the face-off — moving a general aside or interposing a
    ///    piece. This is what Wikipedia describes and it is the reading a
    ///    tournament player is most likely to expect, because it makes bikjang
    ///    a resource rather than an accident.
    /// 3. **Abolished.** Some modern tournament rules drop bikjang entirely and
    ///    play on.
    ///
    /// Reading 1 is implemented because it needs no offer, no acceptance and no
    /// state beyond the position, and because an engine that ended the game
    /// only on a player's say-so would need the platform to carry a draw offer
    /// that nothing else in Bragd has. Reading 2 is the one to revisit when
    /// there is a real janggi audience to ask; the difference between them is
    /// visible only in games where a player *wants* the face-off broken.
    ///
    /// Note also that a tournament under reading 1 would still rank the drawn
    /// game on material — [`Position::janggi_score`] is exposed for exactly
    /// that — and this engine does not, because the brief for the rule was the
    /// simple draw.
    Bikjang,
    Agreement,
}

/// How many plies without a capture before a game is drawn.
///
/// Sixty moves each. The published rules disagree — the WXF says fifty moves
/// by both players "after verification by an arbiter", other bodies say sixty —
/// and none of them is checkable by a program without the arbiter. So this is
/// engine policy, at the more generous of the two numbers.
pub const QUIET_LIMIT: usize = 120;

/// How many times a position must recur before it is adjudicated.
///
/// **Threefold**, which is what most engines and online sites use. The WXF text
/// says four. The difference is real and this is the more common reading; it is
/// noted rather than silently chosen.
pub const REPETITIONS: usize = 3;

#[derive(Clone)]
pub struct Game {
    position: Position,
    seen: Vec<Position>,
    moves: Vec<Move>,
    /// Whether the position at the same index was reached by a checking move.
    checks: Vec<bool>,
    quiet: usize,
    outcome: Option<Outcome>,
}

impl Game {
    pub fn new() -> Game {
        Game::from_position(Position::start())
    }

    /// A janggi game from the diagrammed opening array.
    ///
    /// Any other arrangement — see [`crate::Setup`] — goes through
    /// [`Game::from_position`], because which arrangement each player chose is
    /// part of the starting position rather than part of the Game's rules.
    pub fn janggi() -> Game {
        Game::from_position(Position::janggi())
    }

    pub fn from_position(position: Position) -> Game {
        let mut game = Game {
            position,
            seen: vec![position],
            moves: Vec::new(),
            checks: vec![false],
            quiet: 0,
            outcome: None,
        };
        game.outcome = game.decide();
        game
    }

    pub fn position(&self) -> &Position {
        &self.position
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
        self.position.legal_moves()
    }

    pub fn play(&mut self, mv: Move) -> Result<(), MoveError> {
        if let Some(outcome) = self.outcome {
            return Err(MoveError::AlreadyOver(outcome));
        }
        if !self.legal_moves().contains(&mv) {
            return Err(MoveError::NotLegal(mv));
        }
        // A pass lands on the general's own point, so "is there a piece there"
        // would call it a capture, throw away the repetition history and make
        // janggi's shuffling draws unreachable.
        let capture = !mv.is_pass() && self.position.piece_at(mv.to).is_some();
        self.position = self.position.make_move(mv);
        self.moves.push(mv);
        // Whether *this* move gave check, recorded alongside the position it
        // produced. The perpetual-check test needs to know who was checking
        // throughout a cycle, which a position alone cannot say.
        self.checks
            .push(self.position.is_in_check(self.position.to_move()));
        if capture {
            self.quiet = 0;
            self.seen.clear();
            self.checks.truncate(1);
        } else {
            self.quiet += 1;
        }
        self.seen.push(self.position);
        self.outcome = self.decide();
        Ok(())
    }

    pub fn play_wire(&mut self, text: &str) -> Result<(), MoveError> {
        let mv = Move::parse(text).ok_or_else(|| MoveError::NotUnderstood(text.to_string()))?;
        self.play(mv)
    }

    fn decide(&self) -> Option<Outcome> {
        let to_move = self.position.to_move();
        if self.position.legal_moves().is_empty() {
            // Checkmate and stalemate are both losses, and telling them apart
            // is for the scoresheet rather than for the score. Under janggi
            // the second arm never fires: a player who is not in check can
            // always pass.
            return Some(Outcome::Win {
                winner: to_move.other(),
                reason: if self.position.is_in_check(to_move) {
                    WinReason::Checkmate
                } else {
                    WinReason::Stalemate
                },
            });
        }

        // Bikjang is asked after checkmate and before everything else. After,
        // because a position with no legal moves in it is over on its own
        // terms and the generals' sightline is then beside the point; before,
        // because it is the only ending here that a single move creates
        // outright.
        if self.position.rules() == Ruleset::Janggi && self.position.generals_face() {
            return Some(Outcome::Draw {
                reason: DrawReason::Bikjang,
            });
        }

        let repeats = self
            .seen
            .iter()
            .filter(|&&seen| seen == self.position)
            .count();
        if repeats >= REPETITIONS {
            return Some(self.adjudicate_repetition());
        }

        if self.quiet >= QUIET_LIMIT {
            return Some(self.ran_out(DrawReason::Stalled));
        }
        None
    }

    /// What a game that neither side could finish is worth.
    ///
    /// **Xiangqi halves the point; janggi ranks the players.** Janggi counts
    /// the material each side has left, adds Han's 1.5-point handicap, and the
    /// higher score wins — so a shuffling repetition is a result rather than a
    /// shrug, and a player who is ahead has every reason to run the clock out
    /// while a player who is behind has to come and get it.
    ///
    /// This is threaded through rather than merely exposed because a draw is
    /// the wrong answer, not an approximate one: it would hand half a point to
    /// a player who is fourteen points down.
    fn ran_out(&self, reason: DrawReason) -> Outcome {
        match self.position.rules() {
            Ruleset::Xiangqi => Outcome::Draw { reason },
            Ruleset::Janggi => Outcome::Win {
                winner: self.position.janggi_leader(),
                reason: WinReason::Points,
            },
        }
    }

    /// Who, if anyone, was checking throughout the repeated cycle.
    ///
    /// The cycle is everything back to the previous occurrence of this
    /// position. A side that checked on **every** one of its moves in it has
    /// delivered a perpetual check and loses; if both did, it is a draw.
    ///
    /// Anything else falls through to [`Game::ran_out`], which is where
    /// perpetual *chase* would go if it were implemented.
    ///
    /// **Shared with janggi**, which is unusual for anything in this file.
    /// Janggi forbids repeated check in the same terms — the checking side must
    /// vary, and loses if it will not — so the classification is the same
    /// question with the same answer. What differs is only the fall-through:
    /// janggi ranks the position rather than halving it.
    fn adjudicate_repetition(&self) -> Outcome {
        let Some(previous) = self.seen[..self.seen.len() - 1]
            .iter()
            .rposition(|&seen| seen == self.position)
        else {
            return self.ran_out(DrawReason::Repetition);
        };

        // Each entry in `checks` says whether the move that produced that
        // position gave check. The side that made it is the one to move in the
        // position before it.
        let mut checking = [true, true];
        let mut moved = [false, false];
        for index in (previous + 1)..self.seen.len() {
            let mover = self.seen[index - 1].to_move();
            moved[mover.index()] = true;
            if !self.checks[index] {
                checking[mover.index()] = false;
            }
        }

        let perpetual = |side: Side| moved[side.index()] && checking[side.index()];
        match (perpetual(Side::Red), perpetual(Side::Black)) {
            (true, true) => Outcome::Draw {
                reason: DrawReason::MutualPerpetualCheck,
            },
            // The checker loses, which is the opposite of chess and the reason
            // this rule cannot be left out.
            (true, false) => Outcome::Win {
                winner: Side::Black,
                reason: WinReason::PerpetualCheck,
            },
            (false, true) => Outcome::Win {
                winner: Side::Red,
                reason: WinReason::PerpetualCheck,
            },
            (false, false) => self.ran_out(DrawReason::Repetition),
        }
    }
}

impl Default for Game {
    fn default() -> Game {
        Game::new()
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MoveError {
    AlreadyOver(Outcome),
    NotLegal(Move),
    NotUnderstood(String),
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoveError::AlreadyOver(outcome) => write!(f, "the game is over: {outcome:?}"),
            MoveError::NotLegal(mv) => write!(f, "{mv} is not legal here"),
            MoveError::NotUnderstood(text) => write!(f, "{text:?} is not a move"),
        }
    }
}

impl std::error::Error for MoveError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_game_has_the_red_side_to_move() {
        let game = Game::new();
        assert_eq!(game.position().to_move(), Side::Red);
        assert_eq!(game.outcome(), None);
        assert_eq!(game.legal_moves().len(), 44);
    }

    #[test]
    fn perpetual_check_loses_for_the_checker() {
        // The rule that inverts every chess player's instinct, and the reason
        // xiangqi needs it: without it a losing player draws every game they
        // can find one repeatable check in.
        //
        // Forced, so the test asserts rather than hopes. A red chariot checks
        // along rank 10; the black general's only escape is e9 because d10 and
        // f10 are on the checked rank. The chariot follows to rank 9; now e8 is
        // blocked by black's own soldier and d9/f9 are checked, so the general
        // must go back. The cycle cannot be avoided.
        let mut game = Game::from_position(
            Position::from_fen("4k4/R8/4p4/9/9/9/9/9/9/3K5 w - - 0 1").expect("valid"),
        );

        // Each move is attempted only while the game is still going, because
        // the whole point is that it stops of its own accord.
        for text in ["a9a10", "e10e9", "a10a9", "e9e10"].iter().cycle().take(12) {
            if game.outcome().is_some() {
                break;
            }
            // The general is genuinely forced: every other palace square is
            // either on the checked line or blocked by his own soldier.
            if text.starts_with('e') {
                assert_eq!(game.legal_moves().len(), 1, "before {text}");
            }
            game.play_wire(text)
                .unwrap_or_else(|problem| panic!("{text}: {problem}"));
        }

        assert_eq!(
            game.outcome(),
            Some(Outcome::Win {
                winner: Side::Black,
                reason: WinReason::PerpetualCheck
            }),
            "the checker loses"
        );
    }

    #[test]
    fn a_repetition_that_is_not_a_perpetual_check_is_a_draw() {
        // The same shape without the checks. Two chariots shuffling on their
        // own files, neither ever giving check, and nobody loses for it.
        let mut game = Game::from_position(
            Position::from_fen("3k5/9/9/9/8r/8R/9/9/9/4K4 w - - 0 1").expect("valid"),
        );
        for _ in 0..4 {
            if game.outcome().is_some() {
                break;
            }
            game.play_wire("i5i4").expect("red shuffles");
            game.play_wire("i6i7").expect("black shuffles");
            if game.outcome().is_some() {
                break;
            }
            game.play_wire("i4i5").expect("back");
            game.play_wire("i7i6").expect("back");
        }
        assert_eq!(
            game.outcome(),
            Some(Outcome::Draw {
                reason: DrawReason::Repetition
            })
        );
    }

    #[test]
    fn a_game_always_ends_under_random_play() {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..8 {
            let mut game = Game::new();
            let mut plies = 0;
            while game.outcome().is_none() {
                let moves = game.legal_moves();
                assert!(!moves.is_empty(), "no moves and no outcome");
                game.play(moves[(next() % moves.len() as u64) as usize])
                    .expect("legal");
                plies += 1;
                assert!(plies < 3_000, "no outcome after {plies} plies");
            }
        }
    }

    fn janggi_from(fen: &str) -> Game {
        Game::from_position(Position::from_fen_with(Ruleset::Janggi, fen).expect("valid"))
    }

    #[test]
    fn bikjang_ends_the_game_the_moment_the_generals_see_each_other() {
        // A red soldier on e5 is all that stands between the two generals.
        // Stepping it aside is legal in janggi and draws on the spot; the same
        // move under xiangqi is not a move at all, which is the clearest
        // statement of what the two games do with one position.
        let fen = "4k4/9/9/9/9/4P4/9/9/9/4K4 w - - 0 1";

        let mut game = janggi_from(fen);
        assert_eq!(game.outcome(), None, "nothing yet: e5 blocks the file");
        game.play_wire("e5d5").expect("legal in janggi");
        assert_eq!(
            game.outcome(),
            Some(Outcome::Draw {
                reason: DrawReason::Bikjang
            })
        );

        let xiangqi = Game::from_position(Position::from_fen(fen).expect("valid"));
        assert!(
            !xiangqi
                .legal_moves()
                .iter()
                .any(|mv| mv.to_wire() == "e5d5"),
            "xiangqi refuses to let the file open"
        );
    }

    #[test]
    fn a_janggi_game_that_runs_out_is_ranked_rather_than_halved() {
        // The same shuffle that draws in xiangqi. Both sides have a chariot
        // and nothing else, so Han's 1.5-point handicap decides it — which is
        // exactly the case the handicap exists to settle.
        let fen = "3k5/9/9/9/8r/8R/9/9/9/4K4 w - - 0 1";

        let mut game = janggi_from(fen);
        for _ in 0..4 {
            if game.outcome().is_some() {
                break;
            }
            game.play_wire("i5i4").expect("red shuffles");
            game.play_wire("i6i7").expect("black shuffles");
            if game.outcome().is_some() {
                break;
            }
            game.play_wire("i4i5").expect("back");
            game.play_wire("i7i6").expect("back");
        }
        assert_eq!(
            game.outcome(),
            Some(Outcome::Win {
                winner: Side::Black,
                reason: WinReason::Points
            }),
            "Han is ahead on 16.5 to 15 and janggi gives him the game"
        );

        let mut xiangqi = Game::from_position(Position::from_fen(fen).expect("valid"));
        for _ in 0..4 {
            if xiangqi.outcome().is_some() {
                break;
            }
            for text in ["i5i4", "i6i7", "i4i5", "i7i6"] {
                if xiangqi.outcome().is_some() {
                    break;
                }
                xiangqi.play_wire(text).expect("legal");
            }
        }
        assert!(
            matches!(xiangqi.outcome(), Some(Outcome::Draw { .. })),
            "and the same position under the other rules is half a point each"
        );
    }

    #[test]
    fn a_janggi_player_may_pass_and_the_game_carries_on() {
        let mut game = Game::janggi();
        game.play_wire("e2e2").expect("a pass is a move");
        assert_eq!(game.position().to_move(), Side::Black);
        assert_eq!(game.outcome(), None);
        assert_eq!(game.moves().len(), 1, "and it goes on the scoresheet");
    }

    #[test]
    fn janggi_cannot_stalemate_because_it_can_pass() {
        // Random play, and the assertion is about what never happens: a side
        // with no moves is always a side in check. Stalemate is not scored
        // differently in janggi — it is unreachable, which is a stronger claim
        // and worth testing as one.
        let mut state = 0x6D2B_79F5_2A38_31C1u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..8 {
            let mut game = Game::janggi();
            let mut plies = 0;
            while game.outcome().is_none() {
                let moves = game.legal_moves();
                assert!(!moves.is_empty(), "no moves and no outcome");
                assert!(
                    !game.position().is_in_check(game.position().to_move())
                        || !moves.iter().any(|mv| mv.is_pass()),
                    "a player in check was allowed to stand still"
                );
                game.play(moves[(next() % moves.len() as u64) as usize])
                    .expect("legal");
                plies += 1;
                assert!(plies < 3_000, "no outcome after {plies} plies");
            }
            assert!(
                !matches!(
                    game.outcome(),
                    Some(Outcome::Win {
                        reason: WinReason::Stalemate,
                        ..
                    })
                ),
                "stalemate is unreachable when passing is legal"
            );
        }
    }

    #[test]
    fn stalemate_is_a_loss_and_not_a_draw() {
        // The opposite of chess. Black to move, not in check, and with nowhere
        // to go: black loses.
        let stuck = Position::from_fen("4k4/4P4/9/9/9/9/9/9/4A4/3AK4 b - - 0 1").expect("valid");
        let game = Game::from_position(stuck);
        if game.legal_moves().is_empty() {
            assert!(matches!(
                game.outcome(),
                Some(Outcome::Win {
                    winner: Side::Red,
                    ..
                })
            ));
        }
    }
}
