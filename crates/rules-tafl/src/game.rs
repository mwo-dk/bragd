//! A game, and how it ends.
//!
//! Everything genuinely contested about Copenhagen lives here rather than in
//! [`crate::ruleset`], because what is contested is not *whether* the rules
//! exist but what they mean. The rules text is unambiguous about movement and
//! capture and vague about three endings — the exit fort, encirclement, and
//! perpetual repetition — and a vague rule still has to do something. Each
//! decision below is written down with its argument, because a future reader
//! will otherwise assume the code is the rule.
//!
//! One structural thing is worth saying first, because it removes an entire
//! category of work that chess needs: **there is no check.** No move is illegal
//! for exposing the king, there are no pins, and there is nothing to evade.
//! Every pseudo-legal move is legal, so move generation is geometry and the
//! restricted-square rule, and nothing else.

use crate::board::{Bitboard, Direction, Square};
use crate::position::{Move, Position, Side};
use crate::ruleset::Ruleset;
use core::fmt;

/// How a game ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Attackers(AttackerWin),
    Defenders(DefenderWin),
    Draw(DrawReason),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttackerWin {
    /// Surrounded on all four sides.
    KingCaptured,
    /// The defenders are sealed in: no square they can reach touches an edge,
    /// so the king can never reach a corner.
    Encirclement,
    /// The defenders have no legal move. In tafl that is a loss, not a draw.
    DefendersStuck,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DefenderWin {
    /// The king reached a corner.
    Escape,
    /// The king is on an edge inside a wall the attackers cannot break.
    ExitFort,
    /// The attackers have no legal move. Vanishingly rare, and still a loss.
    AttackersStuck,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawReason {
    /// The ply cap. **An engine policy, not a rule** — see [`PLY_CAP`].
    TooLong,
    Agreement,
}

impl Outcome {
    pub fn winner(self) -> Option<Side> {
        match self {
            Outcome::Attackers(_) => Some(Side::Attackers),
            Outcome::Defenders(_) => Some(Side::Defenders),
            Outcome::Draw(_) => None,
        }
    }
}

/// When a game is called a draw for going on too long.
///
/// **This is not a rule of Copenhagen.** Copenhagen §10 says a game is drawn
/// "if it is not possible to end the game, fx. because both sides have too few
/// pieces left", which has no computable meaning: three attackers cannot
/// capture the king by custody but can still win by encirclement or shieldwall,
/// and nobody has ever written down a threshold.
///
/// So there is no material draw, and instead a cap — because something has to
/// terminate a game or a server hangs. Four hundred plies is roughly twice the
/// length of the longest games in the public records. Being honest that this
/// is engine policy rather than law is the point of saying so here.
pub const PLY_CAP: usize = 400;

/// A game in progress: the position, its history, and how it ended if it has.
#[derive(Clone)]
pub struct Game {
    rules: Ruleset,
    position: Position,
    /// Every position that has occurred, the current one last.
    ///
    /// Stored in full rather than hashed. A tafl position is three machine
    /// words, four hundred of them is a few kilobytes, and an exact comparison
    /// cannot collide — which matters when a repetition is not a draw offer but
    /// an illegal move.
    seen: Vec<Position>,
    moves: Vec<Move>,
    outcome: Option<Outcome>,
}

impl Game {
    pub fn new(rules: Ruleset) -> Game {
        Game::from_position(rules, rules.starting_position())
    }

    pub fn from_position(rules: Ruleset, position: Position) -> Game {
        let mut game = Game {
            rules,
            position,
            seen: vec![position],
            moves: Vec::new(),
            outcome: None,
        };
        // A position handed in from outside may already be over.
        game.outcome = game.decide(position.to_move().other());
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

    /// Every move the side to move may make.
    ///
    /// **Repetition is enforced here, as illegality.** Copenhagen bans
    /// perpetual repetition, and its three primary sources describe three
    /// different mechanics for it; only one is implementable. "The player who
    /// caused the repetition loses" is not a function of the position — in a
    /// mutual forced sequence neither side chose it — so what is enforced is
    /// the hnefatafl.org restatement: **a defender's move that would repeat a
    /// position is not available.** If that leaves the defenders with nothing,
    /// they lose by the stalemate rule, which is what the literal text says
    /// happens anyway.
    ///
    /// It binds the defenders only, and deliberately: the ban exists because a
    /// besieged side can shuffle for ever and the attackers cannot force the
    /// issue, so it is the besieged side that must make progress.
    ///
    /// **And it binds them only where the ruleset has the ban**, which this
    /// read of [`Ruleset::repetition_ban`] is the whole of. It was enforced
    /// unconditionally for a while, and the flag had no reader anywhere in the
    /// crate — so Fetlar, whose entire reason for being here is that Copenhagen
    /// is Fetlar plus exactly three additions, was played with one of the three
    /// silently turned on. The visible cost was a fortress: a Fetlar position
    /// where a wall of defenders stands round the king and neither side can
    /// make progress is a draw at the ply cap, and under an unasked-for ban an
    /// attacker who shuffles one man back and forth forces the defenders to
    /// dismantle their own wall on the fourth ply, because every move that does
    /// not repeat takes a stone out of it.
    pub fn legal_moves(&self) -> Vec<Move> {
        let all = self.rules.legal_moves(&self.position);
        if !self.rules.repetition_ban || self.position.to_move() != Side::Defenders {
            return all;
        }
        all.into_iter()
            .filter(|&mv| {
                let after = self.after(mv);
                !self.seen.contains(&after)
            })
            .collect()
    }

    /// The position a move produces, captures resolved.
    fn after(&self, mv: Move) -> Position {
        let mut next = self.position.slide(mv);
        for captured in self.rules.all_captures(&next, mv.to) {
            next = next.capture(captured);
        }
        next
    }

    pub fn play(&mut self, mv: Move) -> Result<(), MoveError> {
        if let Some(outcome) = self.outcome {
            return Err(MoveError::AlreadyOver(outcome));
        }
        if !self.legal_moves().contains(&mv) {
            return Err(MoveError::NotLegal(mv));
        }
        let mover = self.position.to_move();
        self.position = self.after(mv);
        self.seen.push(self.position);
        self.moves.push(mv);
        self.outcome = self.decide(mover);
        Ok(())
    }

    pub fn play_wire(&mut self, text: &str) -> Result<(), MoveError> {
        let mv = Move::parse(text).ok_or_else(|| MoveError::NotUnderstood(text.to_string()))?;
        self.play(mv)
    }

    /// Whether the game is over, given who just moved.
    ///
    /// **The mover matters**, and that is a decision rather than a detail. The
    /// rules are silent on when the exit fort and encirclement are tested, so
    /// each is evaluated only after the move of the side it works *against*:
    /// the fort after the attackers move, the encirclement after the defenders
    /// move. That guarantees the losing side always gets one move to break the
    /// formation, and stops a player losing to a shape their opponent built and
    /// they never had a chance to answer.
    fn decide(&self, mover: Side) -> Option<Outcome> {
        // Immediate, and checked first: an escape or a capture ends the game
        // wherever it happens.
        if self.rules.king_has_escaped(&self.position) {
            return Some(Outcome::Defenders(DefenderWin::Escape));
        }
        if self.rules.king_is_captured(&self.position) {
            return Some(Outcome::Attackers(AttackerWin::KingCaptured));
        }

        match mover {
            Side::Attackers if self.rules.exit_fort && self.is_exit_fort() => {
                return Some(Outcome::Defenders(DefenderWin::ExitFort));
            }
            Side::Defenders if self.rules.encirclement && self.is_encircled() => {
                return Some(Outcome::Attackers(AttackerWin::Encirclement));
            }
            _ => {}
        }

        // A player who cannot move **loses**. The opposite of chess, where it
        // is a draw, and it applies to both sides — the attackers being stuck
        // is vanishingly rare and is not a special case.
        if self.legal_moves().is_empty() {
            return Some(match self.position.to_move() {
                Side::Defenders => Outcome::Attackers(AttackerWin::DefendersStuck),
                Side::Attackers => Outcome::Defenders(DefenderWin::AttackersStuck),
            });
        }

        if self.moves.len() >= PLY_CAP {
            return Some(Outcome::Draw(DrawReason::TooLong));
        }
        None
    }

    /// Whether the defenders are sealed in.
    ///
    /// A flood fill from the king through everything that is not an attacker.
    /// If nothing it reaches touches the board's edge, the king can never reach
    /// a corner and the attackers have won — which is exactly the rule's own
    /// stated justification, "they have prevented the king from escaping".
    ///
    /// Preferred over looking for a literal unbroken ring of attackers, which
    /// is under-inclusive: a ring anchored on the edge seals the defenders in
    /// just as completely and is not a loop.
    pub fn is_encircled(&self) -> bool {
        let Some(king) = self.position.king_square() else {
            return false;
        };
        let attackers = self.position.side(Side::Attackers);
        let mut reached = Bitboard::from_square(king);
        let mut frontier = vec![king];
        while let Some(square) = frontier.pop() {
            if square.is_edge(self.position.size()) {
                return false;
            }
            for direction in Direction::ALL {
                let Some(next) = square.step(direction, 1, self.position.size()) else {
                    continue;
                };
                if attackers.contains(next) || reached.contains(next) {
                    continue;
                }
                reached = reached.with(next);
                frontier.push(next);
            }
        }
        true
    }

    /// Whether the defenders have built a fort the attackers cannot break.
    ///
    /// Three conditions, and the third is the one that matters:
    ///
    /// 1. The king stands on an edge square.
    /// 2. He has at least one legal move — a fort he cannot move inside is not
    ///    a fort, it is a coffin.
    /// 3. The wall around him **cannot be broken**, tested by trying: every
    ///    attacker move is played and the wall re-examined.
    ///
    /// Condition 3 is why the naive version is wrong. Two defenders lying flat
    /// on the edge with attackers already pressing them pass a flood fill and
    /// are trivially taken by a shieldwall — and the shieldwall was added to
    /// Copenhagen precisely to punish that shape, so scoring it as a win
    /// inverts what the rule is for.
    ///
    /// It is a **one-move** test, not a proof. Proving no sequence of attacker
    /// moves can ever break a fort is not implementable and is not what human
    /// players adjudicate either.
    pub fn is_exit_fort(&self) -> bool {
        let Some(king) = self.position.king_square() else {
            return false;
        };
        if !king.is_edge(self.position.size()) {
            return false;
        }
        if self.rules.destinations(&self.position, king).is_empty() {
            return false;
        }

        let Some(wall) = self.enclosing_wall(king) else {
            return false;
        };

        // Try to break it. Every attacker move, played out, with the wall
        // re-examined afterwards.
        let attackers_turn = {
            let mut probe = self.position;
            if probe.to_move() != Side::Attackers {
                probe = probe.pass_to(Side::Attackers);
            }
            probe
        };
        for mv in self.rules.legal_moves(&attackers_turn) {
            let mut after = attackers_turn.slide(mv);
            let taken = self.rules.all_captures(&after, mv.to);
            if !(taken & wall).is_empty() {
                return false;
            }
            for captured in taken {
                after = after.capture(captured);
            }
        }
        true
    }

    /// The defenders enclosing the king, or `None` if he is not enclosed.
    ///
    /// Floods the empty squares the king can reach. If that region ever touches
    /// an attacker, there is no fort. What bounds it is defenders and the board
    /// edge, and the defenders that bound it are the wall.
    fn enclosing_wall(&self, king: Square) -> Option<Bitboard> {
        let attackers = self.position.side(Side::Attackers);
        let defenders = self.position.side(Side::Defenders).without(king);

        let mut inside = Bitboard::from_square(king);
        let mut wall = Bitboard::EMPTY;
        let mut frontier = vec![king];
        while let Some(square) = frontier.pop() {
            for direction in Direction::ALL {
                let Some(next) = square.step(direction, 1, self.position.size()) else {
                    // The board's edge bounds the fort, which is the whole
                    // point of standing on one.
                    continue;
                };
                if inside.contains(next) {
                    continue;
                }
                if attackers.contains(next) {
                    // An attacker inside, or touching the inside. No fort.
                    return None;
                }
                if defenders.contains(next) {
                    wall = wall.with(next);
                    continue;
                }
                inside = inside.with(next);
                frontier.push(next);
            }
        }
        // A king alone on an edge with no defenders round him is not a fort,
        // however unbreakable his loneliness is.
        (!wall.is_empty()).then_some(wall)
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
    use crate::ruleset::{COPENHAGEN, FETLAR};

    fn game(layout: [&str; 11], to_move: Side) -> Game {
        Game::from_position(COPENHAGEN, Position::from_picture(&layout, to_move))
    }

    #[test]
    fn a_new_game_has_the_attackers_to_move_and_no_outcome() {
        let game = Game::new(COPENHAGEN);
        assert_eq!(game.position().to_move(), Side::Attackers);
        assert_eq!(game.outcome(), None);
        assert!(!game.legal_moves().is_empty());
    }

    #[test]
    fn reaching_a_corner_wins_for_the_defenders() {
        let mut game = game(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "K..........",
                "...........",
                "...........",
                "...........",
                "....X......",
            ],
            Side::Defenders,
        );
        game.play_wire("a5a1").expect("the king runs the file");
        assert_eq!(
            game.outcome(),
            Some(Outcome::Defenders(DefenderWin::Escape))
        );
    }

    #[test]
    fn surrounding_the_king_wins_for_the_attackers() {
        let mut game = game(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "....X......",
                "...XK......",
                "...........",
                "...........",
                "...........",
                "...........",
                "....X......",
            ],
            Side::Attackers,
        );
        // West, north and the empty throne to his east are already his. This
        // closes the south, which is the fourth.
        game.play_wire("e1e5").expect("legal");
        assert_eq!(
            game.outcome(),
            Some(Outcome::Attackers(AttackerWin::KingCaptured))
        );
    }

    #[test]
    fn a_side_with_no_move_loses_rather_than_drawing() {
        // The opposite of chess, where it is a draw. Here the king is the only
        // defender left, he is safe on the edge — Copenhagen says a king on an
        // edge cannot be captured at all — and he has nowhere to go. The
        // attackers win without ever taking him.
        let stuck = game(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "X..........",
                "KX.........",
                "X..........",
                "...........",
                "...........",
                "...........",
                "...........",
            ],
            Side::Defenders,
        );
        assert!(stuck.legal_moves().is_empty(), "boxed in on the a-file");
        assert!(
            !COPENHAGEN.king_is_captured(stuck.position()),
            "and not captured — he is on the edge"
        );
        assert_eq!(
            stuck.outcome(),
            Some(Outcome::Attackers(AttackerWin::DefendersStuck))
        );
    }

    /// A board where both sides have one piece with room to shuffle.
    fn shuffling() -> Game {
        game(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                ".....K.....",
                "...........",
                "...O.......",
                "...........",
                "...........",
                ".X.........",
            ],
            Side::Defenders,
        )
    }

    #[test]
    fn the_defenders_may_not_repeat_a_position() {
        // Enforced as illegality rather than as a loss, because "who caused the
        // repetition" is not a function of the position — in a mutual forced
        // sequence neither side chose it. What *can* be a rule is "that move is
        // not available".
        //
        // Note what it takes: one side going back and forth is not a
        // repetition, because the other side's piece has moved too. The ban
        // bites when the whole cycle closes.
        let mut game = shuffling();
        game.play_wire("d4d5").expect("defender up");
        game.play_wire("b1b2").expect("attacker up");
        game.play_wire("d5d4")
            .expect("defender back — not yet a repeat");
        game.play_wire("b2b1")
            .expect("attacker back, closing the cycle");

        assert!(
            !game.legal_moves().contains(&Move::parse("d4d5").unwrap()),
            "going up again would restore a position already seen"
        );
        assert!(
            game.legal_moves().contains(&Move::parse("d4d6").unwrap()),
            "but anywhere new is fine"
        );
    }

    #[test]
    fn a_fetlar_defender_may_return_to_a_position() {
        // The other half of the rule, and the half that had no reader. Fetlar
        // is here to prove the ruleset is data rather than one set of constants
        // wrapped in a struct, so a flag Fetlar turns off has to be a flag the
        // engine reads — otherwise "Copenhagen is Fetlar plus three additions"
        // is a sentence in a doc comment and nothing else.
        //
        // The same four plies as the Copenhagen test above, which is the point:
        // one board, one sequence, two rulesets, and the fifth move is legal in
        // exactly one of them. What it protects is the fortress — a Fetlar wall
        // round a king is a draw at the ply cap, and under a ban the defenders
        // do not have, an attacker shuffling one man back and forth would force
        // them to take their own wall apart to find a move that does not repeat.
        let mut game = Game::from_position(FETLAR, *shuffling().position());
        game.play_wire("d4d5").expect("defender up");
        game.play_wire("b1b2").expect("attacker up");
        game.play_wire("d5d4").expect("defender back");
        game.play_wire("b2b1")
            .expect("attacker back, closing the cycle");

        assert!(
            game.legal_moves().contains(&Move::parse("d4d5").unwrap()),
            "Fetlar has no repetition ban, so going up again is legal"
        );
        assert_eq!(
            game.legal_moves().len(),
            FETLAR.legal_moves(game.position()).len(),
            "and nothing whatever is filtered out"
        );
    }

    #[test]
    fn a_fetlar_fort_is_not_shuffled_apart() {
        // The board from "The fort that wins nothing", character for character,
        // and the reason the flag had to grow a reader rather than the lesson a
        // correction. The king is walled in on d1 by seven of his own men and
        // the attackers cannot break it; the front end says so and invites the
        // reader to play it out.
        //
        // Under a ban the defenders do not have, playing it out gives the
        // opposite answer. An attacker steps a man off and back — d6d7, d7d6 —
        // and on the fourth ply the king's return to d1 restores a position
        // already seen and is therefore gone, and every defender move that
        // remains takes a stone out of the wall. A promised fortress that the
        // rules demolish on demand is worse than no promise at all, because the
        // reader who checks is the reader the rules document was written for.
        let fort = Position::from_picture(
            &[
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...X.......",
                ".X.X.X..X..",
                "..X.X......",
                "..OOO......",
                "..O.O......",
                "..OKO......",
            ],
            Side::Attackers,
        );
        let mut game = Game::from_position(FETLAR, fort);
        game.play_wire("d6d7").expect("attacker steps off");
        game.play_wire("d1d2").expect("the king's one square");
        game.play_wire("d7d6").expect("attacker steps back");

        assert!(
            game.legal_moves().contains(&Move::parse("d2d1").unwrap()),
            "the king may step home again — nothing in Fetlar forbids the repeat"
        );
    }

    #[test]
    fn the_attackers_are_not_bound_by_the_repetition_ban() {
        // Deliberately one-sided. The ban exists because the besieged side can
        // shuffle for ever while the attackers cannot force the issue, so it is
        // the besieged side that must make progress.
        //
        // Stated as "the filter is not applied" rather than by constructing an
        // attacker repetition, because one cannot be constructed: a position
        // repeats only when *both* sides return, and the defenders are already
        // forbidden from doing so. That is worth knowing — the ban on one side
        // very nearly bans repetition outright.
        let mut game = shuffling();
        game.play_wire("d4d5").expect("defender up");
        game.play_wire("b1b2").expect("attacker up");
        game.play_wire("d5d6").expect("defender on");

        assert_eq!(game.position().to_move(), Side::Attackers);
        assert_eq!(
            game.legal_moves().len(),
            COPENHAGEN.legal_moves(game.position()).len(),
            "nothing is filtered out on the attackers' turn"
        );
    }

    #[test]
    fn a_sealed_in_king_is_encircled() {
        // No square the defenders can reach touches an edge, so the king can
        // never reach a corner. A ring anchored on the board's edge counts,
        // which is why this is a flood fill rather than a search for a loop.
        let sealed = game(
            [
                "...........",
                "...........",
                "...........",
                "..XXXXX....",
                "..X...X....",
                "..X.K.X....",
                "..X...X....",
                "..XXXXX....",
                "...........",
                "...........",
                "...........",
            ],
            Side::Attackers,
        );
        assert!(sealed.is_encircled());

        let one_gap = game(
            [
                "...........",
                "...........",
                "...........",
                "..XXXXX....",
                "..X...X....",
                "..X.K.X....",
                "..X...X....",
                "..XX.XX....",
                "...........",
                "...........",
                "...........",
            ],
            Side::Attackers,
        );
        assert!(!one_gap.is_encircled(), "one gap and the seal is broken");
    }

    #[test]
    fn a_bare_king_on_an_edge_is_not_a_fort() {
        // However unbreakable his loneliness is. A fort needs a wall.
        let alone = game(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "K..........",
                "...........",
                "...........",
                "...........",
                "...........",
                ".........X.",
            ],
            Side::Attackers,
        );
        assert!(!alone.is_exit_fort());
    }

    #[test]
    fn a_fort_whose_wall_can_be_taken_is_not_a_fort() {
        // The case the naive flood fill gets wrong, and the reason the test
        // tries every attacker move rather than looking at the shape. Two
        // defenders flat on the edge with attackers already pressing them are
        // a shieldwall waiting to happen — and the shieldwall rule was added
        // to punish exactly this.
        let flat = game(
            [
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...........",
                "...XXX.....",
                "..XOKO....X",
            ],
            Side::Attackers,
        );
        assert!(
            !flat.is_exit_fort(),
            "a shieldwall takes the wall out from under him"
        );
    }

    #[test]
    fn a_game_that_goes_on_for_ever_is_drawn_rather_than_hanging() {
        // Engine policy rather than a rule of Copenhagen, and named as such.
        assert_eq!(PLY_CAP, 400);
    }
}
