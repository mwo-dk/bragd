//! A Game: a position, the moves that reached it, and how it ended.
//!
//! Everything a position cannot answer on its own lives here, and all of it is
//! about history. Repetition needs to know which positions have been seen; the
//! move-count rules need to know how long since anything irreversible happened.
//!
//! Still no clocks and still no I/O — the caller owns those and tells
//! this type what happened by calling [`Game::end`]. That split is the point:
//! everything in here can be replayed exactly from stored moves, which is what
//! makes a disputed Game answerable years later.

use crate::chess_move::{Move, MoveList};
use crate::outcome::{DrawClaim, DrawReason, Outcome, WinReason};
use crate::piece::Color;
use crate::position::Position;
use crate::variant::{Chess, Variant};
use core::fmt;

/// The same position five times ends the Game whether anyone notices or not.
const AUTOMATIC_REPETITIONS: usize = 5;
/// Three times may be claimed.
const CLAIMABLE_REPETITIONS: usize = 3;
/// 75 moves each, in plies.
const AUTOMATIC_HALFMOVES: u32 = 150;
/// 50 moves each, in plies.
const CLAIMABLE_HALFMOVES: u32 = 100;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Game<V: Variant = Chess> {
    variant: V,
    initial: Position,
    position: Position,
    moves: Vec<Move>,
    /// One key per position, `keys[0]` being the initial one. Always one longer
    /// than `moves`.
    keys: Vec<u64>,
    outcome: Option<Outcome>,
    /// The current position's legal moves, generated once when it is reached:
    /// the next move is checked against them and the outcome read from them.
    legal: MoveList,
    /// Each move in SAN, written as it is played from the moves already
    /// generated, so `san_history` replays nothing.
    san: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MoveError {
    /// The Game is already over. Carries the outcome, because the caller
    /// usually wants to tell the Player why rather than just that.
    AlreadyOver(Outcome),
    NotLegal(Move),
    /// Arrived as text and did not name a legal move here — either malformed,
    /// or legal-looking but not available. The two are not distinguished on
    /// purpose: a client is not owed an explanation of why its move was
    /// refused, and telling it more only helps a probing one.
    NotUnderstood(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClaimError {
    AlreadyOver(Outcome),
    /// The conditions for this Claim are not met. Claiming a draw that is not
    /// there is a normal thing for a client to attempt — the position may have
    /// changed between the button appearing and the message arriving.
    NotAvailable(DrawClaim),
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoveError::AlreadyOver(outcome) => write!(f, "the game is over ({outcome})"),
            MoveError::NotLegal(mv) => write!(f, "{mv} is not legal in this position"),
            MoveError::NotUnderstood(text) => {
                write!(f, "{text:?} does not name a legal move here")
            }
        }
    }
}

impl fmt::Display for ClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClaimError::AlreadyOver(outcome) => write!(f, "the game is over ({outcome})"),
            ClaimError::NotAvailable(claim) => write!(f, "{claim:?} cannot be claimed here"),
        }
    }
}

impl std::error::Error for MoveError {}
impl std::error::Error for ClaimError {}

impl Game<Chess> {
    /// A new Game of standard chess from the standard starting position.
    pub fn new() -> Game<Chess> {
        Game::from_position(Chess, Position::starting())
    }
}

impl Default for Game<Chess> {
    fn default() -> Self {
        Game::new()
    }
}

impl<V: Variant> Game<V> {
    pub fn start(variant: V) -> Game<V> {
        let position = variant.starting_position();
        Game::from_position(variant, position)
    }

    /// Starts from an arbitrary position — a Chess960 setup, a puzzle, an
    /// stored Game being resumed.
    pub fn from_position(variant: V, position: Position) -> Game<V> {
        let legal = variant.legal_moves(&position);
        let outcome = variant.outcome_given(&position, &legal);
        Game {
            variant,
            initial: position,
            position,
            moves: Vec::new(),
            keys: vec![position.key()],
            outcome,
            legal,
            san: Vec::new(),
        }
    }

    // --- Reading -------------------------------------------------------------

    pub fn variant(&self) -> &V {
        &self.variant
    }

    pub fn position(&self) -> &Position {
        &self.position
    }

    pub fn initial_position(&self) -> &Position {
        &self.initial
    }

    /// Every move played, in order. This is the record a Library stores and
    /// replay folds over.
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

    pub fn legal_moves(&self) -> MoveList {
        if self.is_over() {
            return MoveList::new();
        }
        self.legal.clone()
    }

    /// The moves as a person would write them: `Nf3`, not `g1f3`.
    ///
    /// Derived by replaying rather tha stored, because notation depends on the
    /// position a move was played in — the same move is `Nf3` in one game and
    /// `Ngf3` in another. Storing it would be storing a fact about a position
    /// alongside the move that left it, which is the sort of thing that goes
    /// stale silently.
    ///
    /// Each is written as its move is played, from the legal moves the Game
    /// generates then anyway; nothing here is kept apart from the moves.
    pub fn san_history(&self) -> Vec<String> {
        self.san.clone()
    }

    /// A legal move of the current position in SAN, from the moves the Game
    /// already has. Only a move that gives check generates its replies, to
    /// tell `+` from `#`.
    pub fn san(&self, mv: Move) -> String {
        crate::san::san_given(&self.position, mv, &self.legal, |after| {
            self.variant.legal_moves(after).is_empty()
        })
    }

    /// How many times the current position has occurred, counting now.
    ///
    /// Only positions since the last irreversible move are searched, which is
    /// not merely an optimisation: a capture or a pawn move makes every earlier
    /// position unreachable, so nothing before it can repeat.
    pub fn repetition_count(&self) -> usize {
        let key = self.position.key();
        let reversible = self.position.halfmove_clock() as usize;
        let from = self.keys.len().saturating_sub(reversible + 1);
        self.keys[from..]
            .iter()
            .filter(|&&seen| seen == key)
            .count()
    }

    /// The draws the side to move may claim right now.
    ///
    /// Note this reports what is claimable *in the current position*, not
    /// FIDE's fuller rule where a Player may also claim by declaring the move
    /// they intend to play. Announcing an unplayed move is a real part of the
    /// rules over the board and a poor fit for a Game where moves arrive as
    /// messages; a Player who wants the draw plays the move and claims after.
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

    /// Plays a move, then applies any ending the rules produce automatically.
    pub fn play(&mut self, mv: Move) -> Result<(), MoveError> {
        if let Some(outcome) = self.outcome {
            return Err(MoveError::AlreadyOver(outcome));
        }
        if !self.legal.contains(mv) {
            return Err(MoveError::NotLegal(mv));
        }

        // `Position::make_move`: every variant's `make_move` is a legality
        // assertion and then this call, and legality was checked just above.
        let after = self.position.make_move(mv);
        let next = self.variant.legal_moves(&after);
        self.san.push(crate::san::san_given(&self.position, mv, &self.legal, |_| next.is_empty()));
        self.position = after;
        self.legal = next;
        self.moves.push(mv);
        self.keys.push(self.position.key());
        self.outcome = self.automatic_outcome();
        Ok(())
    }

    /// Plays a move given in long algebraic form, as it arrives on the wire.
    ///
    /// The text is matched against the legal moves rather than parsed into one,
    /// which is what makes this safe to call on anything a client sends: an
    /// unrecognised string cannot become a move, only an error.
    pub fn play_uci(&mut self, uci: &str) -> Result<(), MoveError> {
        if let Some(outcome) = self.outcome {
            return Err(MoveError::AlreadyOver(outcome));
        }
        match self
            .legal
            .as_slice()
            .iter()
            .copied()
            .find(|mv| mv.to_uci(self.variant.castling_notation()) == uci)
        {
            Some(mv) => self.play(mv),
            None => Err(MoveError::NotUnderstood(uci.to_string())),
        }
    }

    /// Takes a claimable draw.
    pub fn claim_draw(&mut self, claim: DrawClaim) -> Result<(), ClaimError> {
        if let Some(outcome) = self.outcome {
            return Err(ClaimError::AlreadyOver(outcome));
        }
        if !self.available_claims().contains(&claim) {
            return Err(ClaimError::NotAvailable(claim));
        }
        self.outcome = Some(Outcome::Draw {
            reason: claim.reason(),
        });
        Ok(())
    }

    /// Ends the Game for a reason the rules cannot see — resignation,
    /// flag-fall, abandonment, agreement. The caller is the authority for
    /// these; the engine only records them.
    ///
    /// Returns the outcome already recorded if the Game had ended, and does not
    /// overwrite it. That is not defensiveness for its own sake: a resignation
    /// and a flag-fall genuinely can arrive at the same instant, and whichever
    /// the caller processed first is the one that happened.
    pub fn end(&mut self, outcome: Outcome) -> Result<(), Outcome> {
        match self.outcome {
            Some(existing) => Err(existing),
            None => {
                self.outcome = Some(outcome);
                Ok(())
            }
        }
    }

    pub fn resign(&mut self, resigning: Color) -> Result<(), Outcome> {
        self.end(Outcome::Win {
            winner: resigning.opponent(),
            reason: WinReason::Resignation,
        })
    }

    /// Records a flag-fall. A flag alone does not win a Game the winner could
    /// never have won: with no material to mate, it is a draw.
    pub fn flag(&mut self, flagged: Color) -> Result<(), Outcome> {
        let winner = flagged.opponent();
        let outcome = if self.variant.can_possibly_mate(&self.position, winner) {
            Outcome::Win {
                winner,
                reason: WinReason::Flag,
            }
        } else {
            Outcome::Draw {
                reason: DrawReason::FlagWithInsufficientMaterial,
            }
        };
        self.end(outcome)
    }

    /// Endings that apply themselves. Checkmate is tested first: a move that
    /// mates ends the Game even if it also completes the seventy-fifth move.
    fn automatic_outcome(&self) -> Option<Outcome> {
        if let Some(outcome) = self.variant.outcome_given(&self.position, &self.legal) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The knights-out-and-back shuffle, which returns the exact starting
    /// position — castling rights and all — every four plies.
    const SHUFFLE: [&str; 4] = ["g1f3", "g8f6", "f3g1", "f6g8"];

    fn play_all(game: &mut Game, moves: impl IntoIterator<Item = &'static str>) {
        for uci in moves {
            game.play_uci(uci).unwrap_or_else(|e| panic!("{uci}: {e}"));
        }
    }

    fn game_from(fen: &str) -> Game {
        Game::from_position(
            Chess,
            Position::from_fen(fen).expect("test FEN should parse"),
        )
    }

    #[test]
    fn a_new_game_is_the_starting_position_with_no_moves() {
        let game = Game::new();
        assert_eq!(game.moves().len(), 0);
        assert_eq!(game.side_to_move(), Color::White);
        assert_eq!(game.legal_moves().len(), 20);
        assert!(!game.is_over());
        assert_eq!(
            game.repetition_count(),
            1,
            "the start counts as its own first sighting"
        );
    }

    #[test]
    fn repetition_is_claimable_at_three_and_automatic_at_five() {
        let mut game = Game::new();
        play_all(&mut game, SHUFFLE); // second sighting
        assert_eq!(game.repetition_count(), 2);
        assert!(game.available_claims().is_empty());

        play_all(&mut game, SHUFFLE); // third
        assert_eq!(game.repetition_count(), 3);
        assert_eq!(
            game.available_claims(),
            vec![DrawClaim::ThreefoldRepetition]
        );
        assert!(!game.is_over(), "threefold is claimable, never automatic");

        play_all(&mut game, SHUFFLE); // fourth
        assert!(!game.is_over());

        play_all(&mut game, SHUFFLE); // fifth
        assert_eq!(
            game.outcome(),
            Some(Outcome::Draw {
                reason: DrawReason::FivefoldRepetition
            }),
            "fivefold ends it whether anyone notices or not"
        );
        assert!(
            game.available_claims().is_empty(),
            "nothing to claim once it is over"
        );
    }

    #[test]
    fn an_irreversible_move_wipes_the_repetition_history() {
        let mut game = Game::new();
        play_all(&mut game, SHUFFLE);
        play_all(&mut game, SHUFFLE);
        assert_eq!(game.repetition_count(), 3);

        // A pawn move makes every earlier position unreachable.
        play_all(&mut game, ["e2e4"]);
        assert_eq!(game.repetition_count(), 1);
        assert!(game.available_claims().is_empty());
    }

    #[test]
    fn the_fifty_move_rule_is_claimable_and_the_seventy_five_move_rule_is_not() {
        let mut game = game_from("4k3/8/8/8/8/8/8/4KQ2 w - - 99 60");
        play_all(&mut game, ["f1f2"]);
        assert_eq!(game.position().halfmove_clock(), 100);
        assert_eq!(game.available_claims(), vec![DrawClaim::FiftyMoveRule]);
        assert!(!game.is_over());

        let mut game = game_from("4k3/8/8/8/8/8/8/4KQ2 w - - 149 90");
        play_all(&mut game, ["f1f2"]);
        assert_eq!(
            game.outcome(),
            Some(Outcome::Draw {
                reason: DrawReason::SeventyFiveMoveRule
            })
        );
    }

    #[test]
    fn a_claim_that_is_not_there_is_refused() {
        let mut game = Game::new();
        assert_eq!(
            game.claim_draw(DrawClaim::ThreefoldRepetition),
            Err(ClaimError::NotAvailable(DrawClaim::ThreefoldRepetition))
        );
        assert!(!game.is_over(), "a refused claim does not end anything");
    }

    #[test]
    fn claiming_a_draw_that_is_there_ends_the_game() {
        let mut game = Game::new();
        play_all(&mut game, SHUFFLE);
        play_all(&mut game, SHUFFLE);
        assert!(game.claim_draw(DrawClaim::ThreefoldRepetition).is_ok());
        assert_eq!(
            game.outcome(),
            Some(Outcome::Draw {
                reason: DrawReason::ThreefoldRepetition
            })
        );
    }

    #[test]
    fn checkmate_ends_the_game_and_no_move_follows_it() {
        let mut game = Game::new();
        play_all(&mut game, ["f2f3", "e7e5", "g2g4", "d8h4"]);
        assert_eq!(
            game.outcome(),
            Some(Outcome::Win {
                winner: Color::Black,
                reason: WinReason::Checkmate
            })
        );
        assert!(game.legal_moves().is_empty());
        assert!(matches!(
            game.play_uci("e1f2"),
            Err(MoveError::AlreadyOver(_))
        ));
    }

    #[test]
    fn an_illegal_move_changes_nothing() {
        let mut game = Game::new();
        let before = *game.position();
        assert!(matches!(
            game.play_uci("e2e5"),
            Err(MoveError::NotUnderstood(_))
        ));
        assert!(matches!(
            game.play_uci("nonsense"),
            Err(MoveError::NotUnderstood(_))
        ));
        assert_eq!(*game.position(), before);
        assert_eq!(game.moves().len(), 0);
    }

    #[test]
    fn resignation_hands_the_win_to_the_other_side() {
        let mut game = Game::new();
        assert!(game.resign(Color::White).is_ok());
        assert_eq!(
            game.outcome(),
            Some(Outcome::Win {
                winner: Color::Black,
                reason: WinReason::Resignation
            })
        );
    }

    #[test]
    fn the_first_ending_to_arrive_is_the_one_that_happened() {
        // A resignation and a flag-fall genuinely can race.
        let mut game = Game::new();
        game.resign(Color::White).unwrap();
        let second = game.flag(Color::White);
        assert_eq!(
            second,
            Err(Outcome::Win {
                winner: Color::Black,
                reason: WinReason::Resignation
            }),
            "the later ending is refused and reports what already stood"
        );
    }

    #[test]
    fn flagging_against_a_bare_king_is_a_draw_not_a_loss() {
        // White is out of time, but Black has nothing to mate with.
        let mut game = game_from("4k3/8/8/8/8/8/8/4KR2 b - - 0 1");
        game.flag(Color::White).unwrap();
        assert_eq!(
            game.outcome(),
            Some(Outcome::Draw {
                reason: DrawReason::FlagWithInsufficientMaterial
            })
        );

        // With a rook, the same flag-fall is a win.
        let mut game = game_from("4kr2/8/8/8/8/8/8/4K3 b - - 0 1");
        game.flag(Color::White).unwrap();
        assert_eq!(
            game.outcome(),
            Some(Outcome::Win {
                winner: Color::Black,
                reason: WinReason::Flag
            })
        );
    }

    #[test]
    fn a_game_starting_from_a_finished_position_is_already_over() {
        let game = game_from("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(
            game.outcome(),
            Some(Outcome::Draw {
                reason: DrawReason::InsufficientMaterial
            })
        );
        assert!(
            game.legal_moves().is_empty(),
            "a finished game offers no moves"
        );
    }

    #[test]
    fn replaying_the_stored_moves_reproduces_the_position() {
        // The property every stored Game depends on.
        let mut game = Game::new();
        play_all(&mut game, ["e2e4", "e7e5", "g1f3", "b8c6", "f1b5", "a7a6"]);

        let mut replayed = Game::from_position(Chess, *game.initial_position());
        for mv in game.moves() {
            replayed.play(*mv).expect("a stored move must replay");
        }
        assert_eq!(replayed.position(), game.position());
        assert_eq!(replayed.position().key(), game.position().key());
        assert_eq!(replayed.position().to_fen(), game.position().to_fen());
    }

    /// The moves, outcome and SAN a Game keeps from its once-a-ply generation
    /// are exactly what asking the variant afresh gives, in every variant, over
    /// random games played to their end.
    #[test]
    fn what_the_game_keeps_is_what_the_variant_says() {
        use crate::variant::Rules;
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for rules in Rules::ALL {
            for _ in 0..4 {
                let mut game = Game::start(rules);
                let mut position = *game.position();
                let mut written = Vec::new();
                for _ in 0..300 {
                    let fresh = rules.legal_moves(&position);
                    // The variant's own ending comes first; repetition and the
                    // move count only add to it.
                    assert_eq!(game.outcome(), rules.outcome(&position).or(game.outcome()), "{}", rules.key());
                    if game.is_over() {
                        break;
                    }
                    assert_eq!(game.legal_moves(), fresh, "{}", rules.key());
                    let mv = fresh.as_slice()[(next() % fresh.len() as u64) as usize];
                    assert_eq!(game.san(mv), crate::san::to_san(&rules, &position, mv));
                    written.push(crate::san::to_san(&rules, &position, mv));
                    game.play(mv).unwrap();
                    position = rules.make_move(&position, mv);
                    assert_eq!(game.position(), &position);
                }
                assert_eq!(game.san_history(), written, "{}", rules.key());
            }
        }
    }
}
