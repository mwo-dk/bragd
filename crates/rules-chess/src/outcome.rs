//! How a Game ends.
//!
//! One enum covers every ending, including the ones the engine cannot detect
//! for itself. Resignation, flag-fall and abandonment are decided outside the
//! rules — by a Player, or by a Clock the engine deliberately cannot read
//! — but they end the *same* Game and are stored in the *same*
//! record, so they belong in the same type. The alternative, splitting
//! rules-endings from other endings, only pushes the join into a Library.
//!
//! The vocabulary here is `CONTEXT.md`'s: **Flag**, **Resign**, **Claim**.

use crate::piece::Color;
use core::fmt;

/// A finished Game.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Win { winner: Color, reason: WinReason },
    Draw { reason: DrawReason },
}

impl Outcome {
    pub fn winner(self) -> Option<Color> {
        match self {
            Outcome::Win { winner, .. } => Some(winner),
            Outcome::Draw { .. } => None,
        }
    }

    pub fn is_draw(self) -> bool {
        matches!(self, Outcome::Draw { .. })
    }

    /// The score for a side: 1, ½ or 0. Returned in halves to stay integral —
    /// Tournament standings are summed and compared, and a half-point that
    /// arrives as `0.5` invites floating-point comparison somewhere downstream.
    pub fn half_points_for(self, color: Color) -> u32 {
        match self {
            Outcome::Draw { .. } => 1,
            Outcome::Win { winner, .. } if winner == color => 2,
            Outcome::Win { .. } => 0,
        }
    }

    /// The result as it is written on a scoresheet and in a Library.
    pub fn scoreline(self) -> &'static str {
        match self {
            Outcome::Win {
                winner: Color::White,
                ..
            } => "1-0",
            Outcome::Win {
                winner: Color::Black,
                ..
            } => "0-1",
            Outcome::Draw { .. } => "1/2-1/2",
        }
    }

    /// Whether the rules produced this ending, as opposed to a Player or a
    /// Clock. Only these can be re-derived from stored moves alone, which
    /// matters when verifying a stored Game.
    ///
    /// **Both arms name what is *not* in the moves, and that direction is the
    /// whole of the correctness here.** The win arm read
    /// `reason == WinReason::Checkmate` for a long time, which is chess's
    /// answer written down as though it were the family's: four of the eight
    /// reasons below are decided by the board exactly as checkmate is —
    /// shatranj's [`WinReason::Stalemate`] and [`WinReason::BareKing`],
    /// [`WinReason::KingOfTheHill`] and [`WinReason::RaceWon`] — and every one
    /// of them answered `false`. Racing Kings has no checkmate at all, a move
    /// that gives check being illegal, so that variant had no ending whatever
    /// that could be re-derived from its own moves.
    ///
    /// Written as an exclusion list so a win reason added for a future variant
    /// is right by default: the three endings here that a replay cannot reach
    /// are the three that come from outside the board, and somebody adding a
    /// fourth such ending has to say so, where somebody adding a fifth *rules*
    /// ending has to do nothing.
    pub fn is_by_the_rules(self) -> bool {
        match self {
            Outcome::Win { reason, .. } => !matches!(
                reason,
                WinReason::Resignation | WinReason::Flag | WinReason::Abandonment
            ),
            Outcome::Draw { reason } => reason.is_by_the_rules(),
        }
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.scoreline())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WinReason {
    Checkmate,
    /// The loser resigned.
    Resignation,
    /// The loser's Clock reached zero — *flagged*, in the glossary's terms.
    /// The engine never decides this; the caller does.
    Flag,
    /// The loser never appeared, or left and did not return within the window.
    /// Awarded on a Claim by the opponent, never automatically.
    Abandonment,
    /// The loser had no legal move and was **not** in check.
    ///
    /// Shatranj only. The same position that draws a game of chess wins a game
    /// of shatranj, which is why this is a third decisive ending rather than a
    /// flavour of [`WinReason::Checkmate`]: leaving your opponent with nothing
    /// to play is a way of winning, not a way of failing to.
    Stalemate,
    /// The loser was reduced to a lone king and could not do the same in reply.
    ///
    /// Shatranj only. See [`DrawReason::BothKingsBared`] for the half of this
    /// rule that is a draw.
    BareKing,
    /// The winner's king reached one of the four centre squares.
    ///
    /// King of the Hill only, and the whole variant: the pieces and their moves
    /// are chess's exactly, and this one extra way to win changes every plan on
    /// the board. Checkmate still wins, so this is an *additional* ending
    /// rather than a replacement.
    KingOfTheHill,
    /// The winner's king reached the eighth rank.
    ///
    /// Racing Kings only. See [`DrawReason::BothKingsHome`] for the half of
    /// this rule that is a draw — White moves first, so Black is always given
    /// the reply that makes it fair.
    RaceWon,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawReason {
    /// No legal move, and not in check.
    Stalemate,
    /// Neither side has material enough to force mate.
    InsufficientMaterial,
    /// The same position, five times. Automatic — this is what stops a losing
    /// Player shuffling forever, so it must not require anyone to notice.
    FivefoldRepetition,
    /// 75 moves by each side with no capture and no pawn move. Automatic, for
    /// the same reason.
    SeventyFiveMoveRule,
    /// The same position, three times, and someone claimed it.
    ThreefoldRepetition,
    /// 50 moves by each side with no capture and no pawn move, and someone
    /// claimed it.
    FiftyMoveRule,
    /// Both Players agreed.
    Agreement,
    /// The side to move flagged, but the opponent could not possibly mate.
    /// A flag alone does not win a Game the winner could never have won.
    FlagWithInsufficientMaterial,
    /// Shatranj: a king was bared, and the bared side bared the other one
    /// straight back.
    ///
    /// **This is the one ending in Bragd that a position cannot answer on
    /// its own.** Every other reason here is a property of the board as it
    /// stands; this one is a property of the board's replies, so deciding it
    /// requires generating them. Worth naming, because it is the reason
    /// baring is checked after move generation rather than from material.
    BothKingsBared,
    /// Racing Kings: White's king reached the eighth rank and Black's king
    /// reached it in reply.
    ///
    /// Without this the variant would be decided by the coin toss that hands
    /// out colours, because White simply moves first. The rule is that Black
    /// always gets the reply, and a race both sides finish is drawn.
    BothKingsHome,
}

impl DrawReason {
    pub fn is_by_the_rules(self) -> bool {
        !matches!(
            self,
            DrawReason::Agreement
                | DrawReason::FlagWithInsufficientMaterial
                // **A claim is a Player's act, so it is not in the moves.**
                // This read `true` for both of these, which is the mistake the
                // question is easiest to get wrong: threefold and the
                // fifty-move rule *are* rules, and the position that allows
                // them *is* recoverable from the move list — but the ending is
                // not, because [`DrawClaim`] exists precisely so that a Player
                // who is winning may decline it. Replay a Game drawn on a
                // threefold claim and you get a Game still in progress with a
                // claim available, which is the same silent wrong answer a
                // flag would give.
                | DrawReason::ThreefoldRepetition
                | DrawReason::FiftyMoveRule
        )
    }
}

/// A draw a Player may claim but is not forced to take.
///
/// FIDE makes threefold and the fifty-move rule *claimable* and their fivefold
/// and seventy-five-move counterparts *automatic*. Modelling the claimable ones
/// as an offer rather than an event is not a nicety: a player who is winning
/// may well have repeated a position three times on the way, and taking the
/// draw away from them automatically would be wrong.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawClaim {
    ThreefoldRepetition,
    FiftyMoveRule,
}

impl DrawClaim {
    pub fn reason(self) -> DrawReason {
        match self {
            DrawClaim::ThreefoldRepetition => DrawReason::ThreefoldRepetition,
            DrawClaim::FiftyMoveRule => DrawReason::FiftyMoveRule,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_result_is_worth_two_halves_a_draw_one() {
        let white_wins = Outcome::Win {
            winner: Color::White,
            reason: WinReason::Checkmate,
        };
        assert_eq!(white_wins.half_points_for(Color::White), 2);
        assert_eq!(white_wins.half_points_for(Color::Black), 0);
        assert_eq!(white_wins.scoreline(), "1-0");

        let draw = Outcome::Draw {
            reason: DrawReason::Stalemate,
        };
        assert_eq!(draw.half_points_for(Color::White), 1);
        assert_eq!(draw.half_points_for(Color::Black), 1);
        assert_eq!(draw.scoreline(), "1/2-1/2");
    }

    #[test]
    fn every_ending_gives_both_sides_two_halves_between_them() {
        for outcome in [
            Outcome::Win {
                winner: Color::White,
                reason: WinReason::Flag,
            },
            Outcome::Win {
                winner: Color::Black,
                reason: WinReason::Resignation,
            },
            Outcome::Draw {
                reason: DrawReason::Agreement,
            },
        ] {
            assert_eq!(
                outcome.half_points_for(Color::White) + outcome.half_points_for(Color::Black),
                2,
                "{outcome:?} does not distribute a whole point"
            );
        }
    }

    #[test]
    fn only_rules_endings_can_be_re_derived_from_the_moves() {
        assert!(
            Outcome::Win {
                winner: Color::White,
                reason: WinReason::Checkmate
            }
            .is_by_the_rules()
        );
        assert!(
            !Outcome::Win {
                winner: Color::White,
                reason: WinReason::Flag
            }
            .is_by_the_rules(),
            "a flag is a Clock's decision, and Clocks are not in the moves"
        );
        assert!(
            Outcome::Draw {
                reason: DrawReason::FivefoldRepetition
            }
            .is_by_the_rules()
        );
        assert!(
            !Outcome::Draw {
                reason: DrawReason::Agreement
            }
            .is_by_the_rules()
        );
    }

    /// The pair the automatic ones are easiest to be confused with.
    ///
    /// Fivefold and the seventy-five-move rule end a Game on their own, so a
    /// replay reaches them; threefold and the fifty-move rule only ever end one
    /// because somebody claimed, and a replay reaches a Game that is still
    /// being played. Asserted beside each other because the four look like one
    /// family of reasons and split exactly down the middle.
    #[test]
    fn a_claimed_draw_is_not_in_the_moves_and_an_automatic_one_is() {
        for automatic in [
            DrawReason::FivefoldRepetition,
            DrawReason::SeventyFiveMoveRule,
        ] {
            assert!(
                Outcome::Draw { reason: automatic }.is_by_the_rules(),
                "{automatic:?} ends a game with nobody asking, so a replay finds it"
            );
        }
        for claimed in [DrawReason::ThreefoldRepetition, DrawReason::FiftyMoveRule] {
            assert!(
                !Outcome::Draw { reason: claimed }.is_by_the_rules(),
                "{claimed:?} needs a Player to claim it, and a claim is not a move"
            );
        }
    }

    /// Every win reason, on the side of the line it belongs on.
    ///
    /// The half of the question chess cannot ask. Checkmate alone can never
    /// tell a "was it the rules" predicate from an "is it checkmate" one, and
    /// asking only about checkmate is how four variants came to report their
    /// signature ending as un-re-derivable: shatranj wins by baring and by
    /// leaving the opponent with nothing to play, King of the Hill wins by
    /// arriving in the centre, and Racing Kings wins by reaching the eighth
    /// rank and has no mate to fall back on. All four are properties of the
    /// board that replaying the move list recovers exactly.
    ///
    /// Enumerated rather than sampled, so a ninth reason added for a future
    /// variant fails here until somebody says which side it is on.
    #[test]
    fn the_board_produces_five_wins_and_a_player_or_a_clock_produces_three() {
        for by_the_rules in [
            WinReason::Checkmate,
            WinReason::Stalemate,
            WinReason::BareKing,
            WinReason::KingOfTheHill,
            WinReason::RaceWon,
        ] {
            assert!(
                Outcome::Win {
                    winner: Color::White,
                    reason: by_the_rules
                }
                .is_by_the_rules(),
                "{by_the_rules:?} is decided by the board, so a replay of the moves finds it"
            );
        }
        for from_outside in [
            WinReason::Resignation,
            WinReason::Flag,
            WinReason::Abandonment,
        ] {
            assert!(
                !Outcome::Win {
                    winner: Color::White,
                    reason: from_outside
                }
                .is_by_the_rules(),
                "{from_outside:?} is not a move, so a replay reaches a game still in progress"
            );
        }
    }

    #[test]
    fn claims_map_to_their_reasons() {
        assert_eq!(
            DrawClaim::ThreefoldRepetition.reason(),
            DrawReason::ThreefoldRepetition
        );
        assert_eq!(DrawClaim::FiftyMoveRule.reason(), DrawReason::FiftyMoveRule);
    }
}
