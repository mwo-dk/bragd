//! Which of the two games on this board is being played.
//!
//! Xiangqi and janggi share a board — nine files by ten ranks, pieces on the
//! intersections, a three-by-three palace at each end — and there is one
//! rules crate per board. So janggi lives here, as a ruleset, the way hnefatafl,
//! brandubh and tablut share the tafl crate.
//!
//! **It is an enum and not a struct of knobs, and that is a departure from
//! `tafl::Ruleset` on purpose.** Tafl's ruleset is a dozen independent fields
//! because tafl has no agreed rules: every one of those fields is a place where
//! real, published, currently-played reconstructions disagree, and a
//! half-Copenhagen game is a thing somebody has actually played. Xiangqi and
//! janggi are not two readings of one game. They differ in the river, the
//! elephant, the guards, the palace lines, the cannon, passing, stalemate,
//! bikjang and the scoring — nine fields, of which every combination but two is
//! a game nobody has ever played. Writing those nine booleans out would offer
//! five hundred and ten ways to be wrong in exchange for no expressiveness at
//! all. The wrong answer here is the general one.
//!
//! What is genuinely parameterised is where each rule is *asked*: the board
//! knows about a river and about palace lines, and each game asks only its own
//! questions.

use crate::position::{Position, Role, Side};
use core::fmt;

/// One of the two games played on this board.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Ruleset {
    /// Chinese chess. The river, the hobbled horse, the screened cannon, and
    /// two generals that may not see each other.
    Xiangqi,
    /// Korean chess. No river, a longer elephant, palace diagonals that are
    /// real lines, a cannon that cannot move without a screen, and a legal
    /// pass.
    Janggi,
}

impl Ruleset {
    pub const ALL: [Ruleset; 2] = [Ruleset::Xiangqi, Ruleset::Janggi];

    /// Stable, and stored on every Game. A rating pool is keyed by it.
    pub const fn key(self) -> &'static str {
        match self {
            Ruleset::Xiangqi => "xiangqi",
            Ruleset::Janggi => "janggi",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Ruleset::Xiangqi => "Xiangqi",
            Ruleset::Janggi => "Janggi",
        }
    }

    /// What the two seats are called.
    ///
    /// Indexed by [`Side::index`], and **the names do not agree with the
    /// colours**. Janggi's first player is Cho (초), whose pieces are green or
    /// blue; the second is Han (한), whose pieces are red. This engine's
    /// `Side::Red` is "the side that moves first" — inherited from xiangqi,
    /// where Red does move first — so under janggi `Side::Red` is *Cho* and
    /// `Side::Black` is *Han*, and a renderer that painted `Side::Red` red
    /// would paint Cho the colour of his opponent.
    ///
    /// This is a familiar mistake: a field holding the right thing under a name that lies. The
    /// name is not renamed here because `Side` is xiangqi's too, so instead the
    /// truth is written down and this function is what anything user-facing is
    /// expected to read.
    pub const fn seat_names(self) -> [&'static str; 2] {
        match self {
            Ruleset::Xiangqi => ["Red", "Black"],
            Ruleset::Janggi => ["Cho", "Han"],
        }
    }

    /// Where a game under these rules begins.
    ///
    /// Janggi's answer is a *choice* rather than a constant — see [`Setup`] —
    /// and this returns the arrangement conventionally drawn in diagrams.
    pub fn starting_position(self) -> Position {
        match self {
            Ruleset::Xiangqi => Position::start(),
            Ruleset::Janggi => Position::janggi(),
        }
    }
}

/// How one player's horses and elephants are arranged before play.
///
/// **Janggi begins with a decision.** Each player may transpose a horse with
/// the elephant beside it, on either wing, independently — so each side has
/// four arrangements and a game has sixteen. It is the only choice in the
/// family that is made before a move rather than by one, and like Chess960's
/// it has to be stored with the Game rather than derived from it.
///
/// Modelled as two independent wings because that is the rule: the
/// transposition is a wing at a time, and an enum of four named arrangements
/// would hide the fact that they are two coins rather than one die.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Wing {
    /// Horse outside, elephant inside: horse beside the chariot, elephant
    /// beside the guard.
    HorseByTheChariot,
    /// The transposition.
    ElephantByTheChariot,
}

/// One player's arrangement, named from that player's own seat.
///
/// **Left and right are the player's own**, which means the two seats mirror:
/// Red's left wing is the a-file end and Black's left wing is the i-file end,
/// because the players sit opposite each other rather than beside each other.
/// Naming the wings by absolute file instead would have been simpler to
/// implement and would have described neither player's actual choice.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Setup {
    pub left: Wing,
    pub right: Wing,
}

impl Setup {
    /// Elephants inside on both wings — **원앙마**, *wonangma*, the "mandarin
    /// duck horses", also called 안상차림, the inner-elephant setup.
    ///
    /// This is what a janggi diagram draws when it is not making a point about
    /// setups, and it is what [`Position::janggi`] uses. It is not the most
    /// common arrangement in serious play — that is [`Setup::GWIMA_LEFT`] or its
    /// mirror — but "the one the diagrams show" is the right default for
    /// Players who are mostly meeting the game for the first time.
    pub const WONANGMA: Setup = Setup {
        left: Wing::HorseByTheChariot,
        right: Wing::HorseByTheChariot,
    };

    /// Elephants outside on both wings — **양귀마**, *yanggwima*, "both corner
    /// horses", also called 바깥상차림, the outer-elephant setup. Both horses
    /// stand beside the guards and can reach the palace corners.
    pub const YANGGWIMA: Setup = Setup {
        left: Wing::ElephantByTheChariot,
        right: Wing::ElephantByTheChariot,
    };

    /// One wing each way — **귀마**, *gwima*, with the elephant inside on the
    /// player's left.
    ///
    /// The two mixed arrangements are 왼상차림 and 오른상차림, the left- and
    /// right-elephant setups, and **the sources disagree about which is
    /// which**: the English descriptions read "left elephant inside" for
    /// 왼상차림 while the Korean tables of 상마상마 / 마상마상 read the other
    /// way round, and neither says from whose seat. Rather than pick a side of
    /// that and be quietly wrong, these two are named for the geometry, which
    /// cannot be misread.
    pub const GWIMA_LEFT: Setup = Setup {
        left: Wing::HorseByTheChariot,
        right: Wing::ElephantByTheChariot,
    };

    /// **귀마** with the elephant inside on the player's right. The mirror of
    /// [`Setup::GWIMA_LEFT`].
    pub const GWIMA_RIGHT: Setup = Setup {
        left: Wing::ElephantByTheChariot,
        right: Wing::HorseByTheChariot,
    };

    pub const ALL: [Setup; 4] = [
        Setup::WONANGMA,
        Setup::YANGGWIMA,
        Setup::GWIMA_LEFT,
        Setup::GWIMA_RIGHT,
    ];

    pub const fn key(self) -> &'static str {
        match (self.left, self.right) {
            (Wing::HorseByTheChariot, Wing::HorseByTheChariot) => "wonangma",
            (Wing::ElephantByTheChariot, Wing::ElephantByTheChariot) => "yanggwima",
            (Wing::HorseByTheChariot, Wing::ElephantByTheChariot) => "gwima-left",
            (Wing::ElephantByTheChariot, Wing::HorseByTheChariot) => "gwima-right",
        }
    }

    pub const fn name(self) -> &'static str {
        match (self.left, self.right) {
            (Wing::HorseByTheChariot, Wing::HorseByTheChariot) => "Wonangma (elephants inside)",
            (Wing::ElephantByTheChariot, Wing::ElephantByTheChariot) => {
                "Yanggwima (elephants outside)"
            }
            (Wing::HorseByTheChariot, Wing::ElephantByTheChariot) => {
                "Gwima (elephant inside on the left)"
            }
            (Wing::ElephantByTheChariot, Wing::HorseByTheChariot) => {
                "Gwima (elephant inside on the right)"
            }
        }
    }

    /// The back rank, in file order a–i, as a row of FEN letters for `side`.
    ///
    /// The mirroring lives here and only here: the player's left wing is the
    /// a-file end for Red and the i-file end for Black.
    pub(crate) fn back_rank(self, side: Side) -> String {
        let pair = |wing: Wing| match wing {
            // Written outer first, which is the a-file end.
            Wing::HorseByTheChariot => ['n', 'b'],
            Wing::ElephantByTheChariot => ['b', 'n'],
        };
        let (near_a, near_i) = if side.is_red() {
            (pair(self.left), pair(self.right))
        } else {
            (pair(self.right), pair(self.left))
        };
        let row: String = [
            'r', near_a[0], near_a[1], 'a', '1', 'a', near_i[1], near_i[0], 'r',
        ]
        .into_iter()
        .collect();
        if side.is_red() {
            row.to_ascii_uppercase()
        } else {
            row
        }
    }
}

impl Default for Setup {
    fn default() -> Setup {
        Setup::WONANGMA
    }
}

/// A janggi score, counted in **half-points**.
///
/// Janggi's piece values are whole numbers and the only fraction anywhere in
/// the game is Han's 1.5-point compensation, so half-points make the whole
/// thing an integer. That is worth doing rather than reaching for a float: two
/// scores are compared for order and for equality when a game is ranked, and
/// `f64` equality is a bug waiting for a position with enough pieces on it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Score(u32);

impl Score {
    pub const fn half_points(self) -> u32 {
        self.0
    }
}

impl fmt::Display for Score {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_multiple_of(2) {
            write!(f, "{}", self.0 / 2)
        } else {
            write!(f, "{}.5", self.0 / 2)
        }
    }
}

/// Han's compensation for moving second, in half-points: 1.5.
///
/// Called 덤 (*deom*). It exists because moving first is an advantage worth
/// about that much, and it has a second effect that the engine leans on
/// directly: **it makes a tied score impossible.** Every piece is worth a whole
/// number of points, so Cho's score is always an integer and Han's never is,
/// and a game ranked on material always has a winner.
pub const HANDICAP: u32 = 3;

impl Role {
    /// What this piece is worth when a janggi game is ranked on material.
    ///
    /// The standard table, in whole points. The general is worth nothing, which
    /// is not a shrug — a game in which he could be captured is already over,
    /// so counting him would only add a constant to both sides.
    pub const fn janggi_points(self) -> u32 {
        match self {
            Role::General => 0,
            Role::Chariot => 13,
            Role::Cannon => 7,
            Role::Horse => 5,
            Role::Elephant | Role::Advisor => 3,
            Role::Soldier => 2,
        }
    }
}

impl Position {
    /// What `side` scores if the game is ranked on material.
    ///
    /// A full army is 72 points — two chariots at 13, two cannons at 7, two
    /// horses at 5, two elephants and two guards at 3, five soldiers at 2 — and
    /// Han's handicap takes him to 73.5 against Cho's 72 at the opening
    /// position, which is the pair of numbers every janggi book quotes.
    ///
    /// Asked of any position, under either ruleset, because the arithmetic is
    /// the same and refusing to answer for a xiangqi position would only mean
    /// the caller had to know which game it was holding.
    pub fn janggi_score(&self, side: Side) -> Score {
        let material: u32 = Role::ALL
            .into_iter()
            .map(|role| self.pieces(side, role).count() * role.janggi_points())
            .sum();
        Score(material * 2 + if side.is_red() { 0 } else { HANDICAP })
    }

    /// Who wins a janggi game that is ranked rather than played out.
    ///
    /// Total, with no draw arm, because [`HANDICAP`] makes a tie arithmetically
    /// impossible — see the note there.
    pub fn janggi_leader(&self) -> Side {
        if self.janggi_score(Side::Red) > self.janggi_score(Side::Black) {
            Side::Red
        } else {
            Side::Black
        }
    }
}

/// The opening array with both players in the arrangement the diagrams draw.
///
/// Rank 10 first, uppercase for the side that moves first, as xiangqi's
/// [`crate::START`] is written. The differences from xiangqi's array are the
/// whole of janggi's opening shape: the generals stand on the **centre** of
/// their palaces rather than the back rank, the guards flank them on the back
/// rank instead, and the soldiers stand on the fourth rank with no river ahead
/// of them.
pub const JANGGI_START: &str =
    "rnba1abnr/4k4/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/4K4/RNBA1ABNR w - - 0 1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_diagrammed_setup_is_the_one_the_start_constant_writes() {
        let built = Position::janggi_from(Setup::WONANGMA, Setup::WONANGMA);
        assert_eq!(built.to_fen(), JANGGI_START);
        assert_eq!(built, Position::janggi());
    }

    #[test]
    fn a_wing_is_transposed_independently_of_the_other() {
        // Four arrangements from two coins, and each differs from wonangma on
        // exactly the wings it says it does.
        let back = |setup: Setup| setup.back_rank(Side::Red);
        assert_eq!(back(Setup::WONANGMA), "RNBA1ABNR");
        assert_eq!(back(Setup::YANGGWIMA), "RBNA1ANBR");
        assert_eq!(back(Setup::GWIMA_LEFT), "RNBA1ANBR");
        assert_eq!(back(Setup::GWIMA_RIGHT), "RBNA1ABNR");

        // Every arrangement is a permutation of the same men.
        for setup in Setup::ALL {
            let row = back(setup);
            assert_eq!(row.matches('N').count(), 2, "{}", setup.name());
            assert_eq!(row.matches('B').count(), 2, "{}", setup.name());
        }
    }

    #[test]
    fn left_and_right_are_each_players_own_and_so_the_seats_mirror() {
        // A mixed setup chosen by both players produces back ranks that are
        // reflections of one another, because the two players sit opposite
        // each other. If the wings were named by file instead, these two rows
        // would read the same and one player would have got the setup he did
        // not ask for.
        let red = Setup::GWIMA_LEFT.back_rank(Side::Red);
        let black = Setup::GWIMA_LEFT.back_rank(Side::Black);
        let reflected: String = black.chars().rev().collect();
        assert_eq!(red.to_ascii_lowercase(), reflected);
    }

    #[test]
    fn the_opening_score_is_seventy_three_and_a_half_against_seventy_two() {
        let start = Position::janggi();
        assert_eq!(start.janggi_score(Side::Red).to_string(), "72");
        assert_eq!(start.janggi_score(Side::Black).to_string(), "73.5");
        assert_eq!(
            start.janggi_leader(),
            Side::Black,
            "Han leads from move one, which is what the handicap is for"
        );
    }

    #[test]
    fn a_ranked_janggi_game_can_never_be_tied() {
        // The handicap is an odd number of half-points and every piece is an
        // even number of them, so the two scores have different parity in every
        // position that can ever arise. Checked over a spread of positions
        // rather than argued at, because the argument is the reason the
        // `janggi_leader` has no draw arm.
        let mut position = Position::janggi();
        for _ in 0..40 {
            assert_ne!(
                position.janggi_score(Side::Red),
                position.janggi_score(Side::Black)
            );
            let Some(&mv) = position.legal_moves().first() else {
                break;
            };
            position = position.make_move(mv);
        }
    }

    #[test]
    fn every_setup_is_a_position_the_engine_will_play() {
        for red in Setup::ALL {
            for black in Setup::ALL {
                let position = Position::janggi_from(red, black);
                assert_eq!(position.side(Side::Red).count(), 16);
                assert_eq!(position.side(Side::Black).count(), 16);
                assert!(!position.legal_moves().is_empty());
                assert_eq!(
                    Position::from_fen_with(Ruleset::Janggi, &position.to_fen()),
                    Some(position),
                    "{} against {}",
                    red.name(),
                    black.name()
                );
            }
        }
    }
}
