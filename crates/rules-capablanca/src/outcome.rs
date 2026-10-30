//! How a game of Capablanca chess ends.
//!
//! Chess's endings exactly — this is the one part of the game Capablanca did
//! not touch. Kept in its own module rather than borrowed from `crates/rules-chess`
//! for the same reason as everything else here: that crate's types are built on
//! sixty-four-square bitboards, and a shared enum would be the thread by which
//! the rest followed.

use crate::piece::{Color, Role};
use crate::position::Position;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WinReason {
    Checkmate,
    Resignation,
    Flag,
    Abandonment,
}

/// Why a game was drawn.
///
/// **Five reasons became eight, and the three that were added are the three
/// [`Outcome::is_by_the_rules`] cannot answer without.** Each of the old
/// spellings was carrying two endings that a replay treats in opposite ways:
/// `Repetition` was both the fivefold that ends a game with nobody asking and
/// the threefold somebody claimed, `FiftyMoveRule` was both halves of the same
/// pair, and `InsufficientMaterial` was both a dead board and a flag against a
/// side that could not have mated. A collapsed reason cannot be re-derived and
/// cannot be re-derived-from, and a stored result has to be checkable.
///
/// The names are `crates/rules-chess`'s deliberately. The two chess-family engines
/// write their reason into the same stored column, and one calling a fivefold
/// draw `repetition` while the other calls it `fivefoldRepetition` would be two
/// spellings of one ending in one table.
///
/// # Three spellings this engine can no longer produce
///
/// A stored reason is free text, permanent and unvalidated,
/// and the split does not reach backwards into it. Any Capablanca row written
/// before this says `repetition`, which no engine emits any more; and two of
/// the retired spellings survive as new reasons meaning something narrower than
/// they used to — an older `fiftyMoveRule` row is the *automatic* draw at 75
/// moves that nobody asked for, where a newer one is a Player's claim at 50, and
/// an older `insufficientMaterial` row may be either a dead board or a flag
/// against a side that could not mate. Read a reason stored by an older version
/// of these rules with that in mind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawReason {
    Stalemate,
    /// Neither side has material enough to mate. A fact about the board.
    InsufficientMaterial,
    /// The same position, five times. Automatic.
    FivefoldRepetition,
    /// 75 moves by each side with no capture and no pawn move. Automatic.
    SeventyFiveMoveRule,
    /// The same position three times, **and somebody claimed it**.
    ThreefoldRepetition,
    /// 50 moves by each side with no capture and no pawn move, **claimed**.
    FiftyMoveRule,
    /// Both Players agreed.
    Agreement,
    /// The side to move flagged, and the opponent could not possibly have
    /// mated. A Clock decided this, not the board — which is why it is its own
    /// reason rather than [`DrawReason::InsufficientMaterial`].
    FlagWithInsufficientMaterial,
}

impl DrawReason {
    /// Whether a replay of the move list reaches this ending on its own.
    pub fn is_by_the_rules(self) -> bool {
        !matches!(
            self,
            DrawReason::Agreement
                | DrawReason::FlagWithInsufficientMaterial
                | DrawReason::ThreefoldRepetition
                | DrawReason::FiftyMoveRule
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Win { winner: Color, reason: WinReason },
    Draw { reason: DrawReason },
}

impl Outcome {
    /// Whether the rules produced this ending, as opposed to a Player or a
    /// Clock — which is to say whether replaying the stored moves reaches it.
    ///
    /// **The boundary used to answer this in TypeScript by assuming every
    /// outcome this engine returned was a rules ending.** It is not: this
    /// engine is asked to record the resignation, the flag, the agreed draw and
    /// the two claims as well, so the adapter's `true` was a claim about the
    /// caller rather than about the ending. The answer belongs here because
    /// this is the only place that knows which of the eight draw reasons and
    /// which of the four win reasons a Player supplied.
    ///
    /// Nothing reads the answer yet; no verifier consults it, so this is the
    /// crate being made able to answer rather than a corruption being repaired.
    /// The consumer it is for is a replay check — the one that would find a
    /// concession missing from the moves.
    ///
    /// Written as an exclusion list, and not because this crate needs it: all
    /// four win reasons are spelled out just above and `Checkmate` is the only
    /// one the board produces, so `reason == WinReason::Checkmate` would be
    /// exactly right here today. It is the reading that is wrong. `crates/rules-chess`
    /// had this same line and eight win reasons, and four board-decided endings
    /// — shatranj's baring and stalemate, King of the Hill, Racing Kings —
    /// answered `false` for years because the line asks "is it checkmate" and
    /// the question is "did a Player or a Clock do this". Spelled the same way
    /// in both crates so neither can be read as the safe precedent for the other.
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

/// The ending this position produces on its own, if any.
///
/// Position-local, like everything here: repetition and the move-count rules
/// need the history of a Game and are decided a layer up.
pub fn outcome(position: &Position) -> Option<Outcome> {
    if is_insufficient_material(position) {
        return Some(Outcome::Draw {
            reason: DrawReason::InsufficientMaterial,
        });
    }
    if !position.legal_moves().is_empty() {
        return None;
    }
    Some(if position.is_check() {
        Outcome::Win {
            winner: position.side_to_move().opponent(),
            reason: WinReason::Checkmate,
        }
    } else {
        Outcome::Draw {
            reason: DrawReason::Stalemate,
        }
    })
}

pub fn is_insufficient_material(position: &Position) -> bool {
    Color::ALL
        .into_iter()
        .all(|color| !can_possibly_mate(position, color))
}

/// Whether `color` has material enough to mate, on the practical standard
/// rather than a full dead-position test.
///
/// **The two compound pieces count as mating material on their own**, and that
/// is the one place this differs from chess's version rather than merely being
/// widened. A lone bishop or knight cannot mate; a lone archbishop can, and so
/// can a lone chancellor — a chancellor is a rook with extra, and king and
/// archbishop against a bare king is a known forced win. Treating either as a
/// minor piece would draw games that are won.
pub fn can_possibly_mate(position: &Position, color: Color) -> bool {
    let ours = position.by_color(color);
    let minors = (position.by_role(Role::Knight) | position.by_role(Role::Bishop)) & ours;
    let heavy = (position.by_role(Role::Pawn)
        | position.by_role(Role::Rook)
        | position.by_role(Role::Queen)
        | position.by_role(Role::Archbishop)
        | position.by_role(Role::Chancellor))
        & ours;

    if !heavy.is_empty() {
        return true;
    }
    // A lone minor cannot mate. Two can, with cooperation, which is the same
    // standard every chess server applies.
    minors.count() >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(fen: &str) -> Position {
        Position::from_fen(fen).expect("a legal position")
    }

    /// Back-rank mate, with a chancellor doing the work — which is worth using
    /// here rather than a rook, because it is the piece whose two halves the
    /// check detector has to add together.
    #[test]
    fn checkmate_is_a_win_for_the_side_that_is_not_stuck() {
        let mated = position("4rk3C/4ppp3/10/10/10/10/10/5K4 b - - 0 1");
        assert!(mated.is_check());
        assert!(mated.legal_moves().is_empty(), "{}", mated.to_fen());
        assert_eq!(
            outcome(&mated),
            Some(Outcome::Win {
                winner: Color::White,
                reason: WinReason::Checkmate
            })
        );
    }

    #[test]
    fn a_position_with_moves_left_has_no_outcome() {
        assert_eq!(outcome(&Position::starting()), None);
    }

    /// The line that is not chess's: a lone compound piece can mate, so a
    /// position with one is not a dead draw.
    #[test]
    fn a_lone_compound_is_mating_material_and_a_lone_minor_is_not() {
        let cases: &[(&str, bool)] = &[
            ("5k4/10/10/10/10/10/10/5K4 w - - 0 1", false),
            ("5k4/10/10/10/10/10/10/5K3B w - - 0 1", false),
            ("5k4/10/10/10/10/10/10/5K3N w - - 0 1", false),
            ("5k4/10/10/10/10/10/10/5K2NN w - - 0 1", true),
            ("5k4/10/10/10/10/10/10/5K3A w - - 0 1", true),
            ("5k4/10/10/10/10/10/10/5K3C w - - 0 1", true),
            ("5k4/10/10/10/10/10/10/5K3R w - - 0 1", true),
        ];
        for (fen, expected) in cases {
            assert_eq!(
                can_possibly_mate(&position(fen), Color::White),
                *expected,
                "{fen}"
            );
        }
    }

    #[test]
    fn two_lone_kings_are_a_draw() {
        assert_eq!(
            outcome(&position("5k4/10/10/10/10/10/10/5K4 w - - 0 1")),
            Some(Outcome::Draw {
                reason: DrawReason::InsufficientMaterial
            })
        );
    }
}
