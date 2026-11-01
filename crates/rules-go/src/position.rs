//! A position, and what playing a stone does to it.
//!
//! Two things here are unlike every other engine in Bragd.
//!
//! **A move is a point, not a from and a to.** Nothing moves. A stone is placed
//! and then never moves again, and the only thing that happens to it afterwards
//! is being lifted off. That is why there is no `slide` here and no origin on a
//! [`Move`].
//!
//! **Passing is a move.** Not a resignation, not a claim — a legal, ordinary,
//! frequently correct move, and the way the game ends. Two of them in a row and
//! there is nothing left to play for.
//!
//! The order inside [`Position::after`] is the whole of Go's difficulty and is
//! written out there.

use crate::board::{Board, Colour, Point};
use core::fmt;

/// A move: a stone on a point, or a pass.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Move {
    Play(Point),
    Pass,
}

impl Move {
    /// The wire form: `d4`, or `pass`.
    ///
    /// Go has no SAN and needs none — a move is a point, so the point's own
    /// name is the notation, and the only thing that had to be invented is a
    /// word for passing. `pass` is what every Go server and every SGF-adjacent
    /// tool already writes.
    pub fn to_wire(self) -> String {
        match self {
            Move::Play(point) => point.name(),
            Move::Pass => "pass".to_string(),
        }
    }

    pub fn parse(text: &str) -> Option<Move> {
        if text.eq_ignore_ascii_case("pass") {
            return Some(Move::Pass);
        }
        Point::parse(text).map(Move::Play)
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_wire())
    }
}

/// Why a stone cannot go there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Illegal {
    OffBoard,
    Occupied,
    /// The stone, and everything it joins, would have no liberty left — and it
    /// takes nothing off the board to earn one.
    Suicide,
    /// The point is the one the ko rule forbids this turn.
    Ko,
}

impl fmt::Display for Illegal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Illegal::OffBoard => "that point is not on the board",
            Illegal::Occupied => "there is already a stone there",
            Illegal::Suicide => "that would be suicide",
            Illegal::Ko => "the ko rule forbids that point this turn",
        })
    }
}

/// A position: the stones, whose turn it is, the ko point, and what has been
/// taken.
///
/// The prisoner counts are carried because territory scoring needs them and
/// they cannot be recovered from the final board — a stone that was captured on
/// move forty leaves no trace at all. Area scoring does not need them, which is
/// part of why the two scorings disagree about who won.
#[derive(Clone, PartialEq, Eq)]
pub struct Position {
    board: Board,
    to_move: Colour,
    ko: Option<Point>,
    prisoners: [u16; 2],
}

impl Position {
    /// An empty board with Black to play.
    pub fn empty(size: u8) -> Option<Position> {
        Some(Position {
            board: Board::new(size)?,
            to_move: Colour::Black,
            ko: None,
            prisoners: [0, 0],
        })
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    pub fn size(&self) -> u8 {
        self.board.size()
    }

    pub fn to_move(&self) -> Colour {
        self.to_move
    }

    /// The point the ko rule forbids **this turn only**, if there is one.
    pub fn ko(&self) -> Option<Point> {
        self.ko
    }

    /// Its Zobrist key ([`crate::zobrist`]): the stones, the side to move and the ko point.
    pub fn zobrist(&self) -> u64 {
        let size = self.size();
        // The key numbers rows from the top, as a game record does.
        let mut stones = Vec::with_capacity(self.board.intersections());
        for row in (0..size).rev() {
            for column in 0..size {
                stones.push(match self.board.at(Point::at(column, row)) {
                    None => 0,
                    Some(Colour::Black) => 1,
                    Some(Colour::White) => 2,
                });
            }
        }
        let ko = self.ko.map(|p| (p.column(), size - 1 - p.row()));
        crate::zobrist::key(size, size, &stones, self.to_move == Colour::Black, ko)
    }

    /// How many stones of each colour have been taken off, indexed by
    /// [`Colour::index`] — **the colour of the stone taken**, not of the player
    /// who took it. Japanese scoring wants the other reading, and adding the
    /// two the wrong way round hands each player the other's prisoners.
    pub fn prisoners(&self) -> [u16; 2] {
        self.prisoners
    }

    /// The position after a move, or why the move cannot be played.
    ///
    /// **The order is the rule.** A stone goes down; then every enemy chain
    /// touching it that now has no liberty comes off; and only *then* is the
    /// stone's own chain examined for suicide. Done the other way round — own
    /// group first — a move that fills your last liberty in order to capture
    /// looks like suicide and is rejected, which is the commonest bug in the
    /// game and one that only shows up in real play, because it is a shape
    /// nobody sets up on purpose.
    ///
    /// It cannot go wrong in the other direction: a capture always frees a
    /// point next to the stone that made it, so a move that takes something can
    /// never leave itself dead. The check is written after the captures anyway,
    /// because the sequence is what a reader needs to see.
    pub fn after(&self, mv: Move) -> Result<Position, Illegal> {
        let point = match mv {
            // A pass clears the ko. The ban exists to stop an immediate
            // retake, and the retake is no longer immediate.
            Move::Pass => {
                return Ok(Position {
                    board: self.board.clone(),
                    to_move: self.to_move.other(),
                    ko: None,
                    prisoners: self.prisoners,
                });
            }
            Move::Play(point) => point,
        };

        if !self.board.contains(point) {
            return Err(Illegal::OffBoard);
        }
        if self.board.at(point).is_some() {
            return Err(Illegal::Occupied);
        }
        // Ko is enforced here, inside move generation, and not as a claim after
        // the fact. A ko-forbidden point is simply not somewhere a stone may
        // go — which is unlike every other repetition rule in Bragd.
        if self.ko == Some(point) {
            return Err(Illegal::Ko);
        }

        let mover = self.to_move;
        let mut board = self.board.clone();
        board.set(point, Some(mover));

        let mut prisoners = self.prisoners;
        let mut taken = 0usize;
        let mut taken_at = None;
        for neighbour in board.neighbours(point) {
            // Re-read the point each time: two neighbours can belong to one
            // chain, and the second is empty by the time we reach it.
            if board.at(neighbour) != Some(mover.other()) || board.has_liberty(neighbour) {
                continue;
            }
            let chain = board.chain(neighbour).expect("a stone stands there");
            for &stone in chain.stones() {
                board.set(stone, None);
            }
            taken += chain.stones().len();
            taken_at = Some(chain.stones()[0]);
        }
        prisoners[mover.other().index()] += taken as u16;

        if !board.has_liberty(point) {
            return Err(Illegal::Suicide);
        }

        // A ko is not "a stone was taken". It is the position that can be
        // restored by taking straight back: exactly one stone off, and the
        // stone that took it standing alone with exactly one liberty — the
        // point it just cleared. Every clause matters. Drop the "alone" and a
        // snapback becomes a ko, and the player who set the snapback up is told
        // they may not have it.
        let ko = match (taken, taken_at) {
            (1, Some(taken_at)) => {
                let placed = board.chain(point).expect("the stone just played");
                (placed.stones().len() == 1 && placed.liberties() == 1).then_some(taken_at)
            }
            _ => None,
        };

        Ok(Position {
            board,
            to_move: mover.other(),
            ko,
            prisoners,
        })
    }

    /// Every move that is legal *in this position*, passing included.
    ///
    /// Simple ko is applied here, because a position knows its own ko point.
    /// Superko is not, because it is a fact about the game's whole history and
    /// a position does not have one — it is applied in
    /// [`crate::game::Game::legal_moves`], which does. Both are still
    /// enforced inside move generation; they are enforced at the layer that
    /// holds the evidence.
    pub fn plays(&self) -> Vec<Move> {
        let mut moves: Vec<Move> = self
            .board
            .points()
            .filter(|&point| self.after(Move::Play(point)).is_ok())
            .map(Move::Play)
            .collect();
        moves.push(Move::Pass);
        moves
    }

    /// A position drawn as a picture: `X` black, `O` white, `.` empty, top row
    /// first — the way a board is printed.
    ///
    /// Positions in tests are written this way because Go is a game about
    /// *shape*, and a ko or a snapback or an eye is recognisable as a diagram
    /// and unreadable as a list of coordinates.
    ///
    /// # Panics
    /// If the picture is not square, or has a character in it that is not a
    /// stone or an empty point.
    pub fn from_picture(rows: &[&str], to_move: Colour) -> Position {
        let size = rows.len();
        let mut board = Board::new(size as u8).expect("a picture of a board that exists");
        for (offset, line) in rows.iter().enumerate() {
            let row = (size - 1 - offset) as u8;
            assert_eq!(line.chars().count(), size, "row {row} is the wrong length");
            for (column, character) in line.chars().enumerate() {
                let colour = match character {
                    '.' | '+' => None,
                    'X' | 'x' | '#' => Some(Colour::Black),
                    'O' | 'o' => Some(Colour::White),
                    other => panic!("{other:?} is not a stone"),
                };
                board.set(Point::at(column as u8, row), colour);
            }
        }
        Position {
            board,
            to_move,
            ko: None,
            prisoners: [0, 0],
        }
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}{} to play", self.board, self.to_move.name())?;
        match self.ko {
            Some(ko) => writeln!(f, ", ko at {ko}"),
            None => writeln!(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(text: &str) -> Point {
        Point::parse(text).expect("a point")
    }

    fn play(position: &Position, text: &str) -> Result<Position, Illegal> {
        position.after(Move::parse(text).expect("a move"))
    }

    #[test]
    fn a_move_round_trips_through_the_wire_for_every_point_and_for_passing() {
        let board = Board::new(19).expect("a board");
        for point in board.points() {
            let mv = Move::Play(point);
            assert_eq!(Move::parse(&mv.to_wire()), Some(mv), "{mv}");
        }
        assert_eq!(Move::parse("pass"), Some(Move::Pass));
        assert_eq!(Move::Pass.to_wire(), "pass");
        assert_eq!(Move::parse("PASS"), Some(Move::Pass));
        assert_eq!(Move::parse("passe"), None);
    }

    #[test]
    fn a_stone_with_no_liberties_is_lifted() {
        // A white stone in the corner with one liberty left. Black takes it.
        let position = Position::from_picture(
            &[
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                "X........",
                "O........",
            ],
            Colour::Black,
        );
        let after = play(&position, "b1").expect("a capture");
        assert_eq!(after.board().at(point("a1")), None, "the stone came off");
        assert_eq!(after.prisoners()[Colour::White.index()], 1);
    }

    #[test]
    fn filling_your_own_last_liberty_is_suicide() {
        let position = Position::from_picture(
            &[
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                "O........",
                ".O.......",
            ],
            Colour::Black,
        );
        assert_eq!(play(&position, "a1"), Err(Illegal::Suicide));
    }

    #[test]
    fn suicide_by_joining_a_group_is_also_illegal() {
        // Multi-stone suicide. Japanese and Chinese rules agree it is
        // forbidden; **Ing and New Zealand allow it**, which is the one place
        // the four big rule sets split on what a legal move is rather than on
        // how to count. The common rule is what is implemented, and the
        // divergence is written down here rather than made a parameter,
        // because no ruleset Bragd offers wants the other answer.
        let position = Position::from_picture(
            &[
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                "O........",
                "XO.......",
                ".O.......",
            ],
            Colour::Black,
        );
        assert_eq!(play(&position, "a1"), Err(Illegal::Suicide));
    }

    #[test]
    fn a_capture_is_resolved_before_your_own_group_is_checked() {
        // The single most common Go bug, as a test. Black plays a2: the point
        // has no liberty of its own — a1, a3 and b2 are all white — so a
        // suicide check run *first* rejects it. Run second, after the white
        // stone at a1 has come off for having no liberties, it is an ordinary
        // capture and the stone breathes through the point it just cleared.
        let position = Position::from_picture(
            &[
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                "O........",
                ".O.......",
                "OX.......",
            ],
            Colour::Black,
        );
        let after = play(&position, "a2").expect("a capture, not a suicide");
        assert_eq!(after.board().at(point("a1")), None);
        assert_eq!(after.board().at(point("a2")), Some(Colour::Black));
        assert_eq!(after.prisoners()[Colour::White.index()], 1);
    }

    /// The standard ko shape: a white stone at d3 with one liberty, and Black
    /// about to take it.
    fn ko() -> Position {
        Position::from_picture(
            &[
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                "..OX.....",
                ".O.OX....",
                "..OX.....",
                ".........",
            ],
            Colour::Black,
        )
    }

    #[test]
    fn the_ko_point_may_not_be_taken_straight_back() {
        let taken = play(&ko(), "c3").expect("Black takes the ko");
        assert_eq!(taken.board().at(point("d3")), None);
        assert_eq!(taken.ko(), Some(point("d3")));
        assert_eq!(
            play(&taken, "d3"),
            Err(Illegal::Ko),
            "White would restore the position exactly"
        );

        // Anywhere else is fine, and one move later the ko is there to be had.
        let elsewhere = play(&taken, "j9").expect("a move somewhere else");
        assert_eq!(elsewhere.ko(), None);
        let answer = play(&elsewhere, "a9").expect("Black answers");
        play(&answer, "d3").expect("now White may take it back");
    }

    #[test]
    fn a_pass_clears_the_ko() {
        let taken = play(&ko(), "c3").expect("Black takes the ko");
        let passed = taken.after(Move::Pass).expect("passing is always legal");
        assert_eq!(passed.ko(), None);
    }

    #[test]
    fn a_snapback_is_not_a_ko() {
        // White gives a stone away at a1; Black takes it with a2 and by doing
        // so puts three stones of their own on one liberty; White plays a1
        // again and takes all three. The retake is legal because the chain that
        // captured was three stones, not one — which is the clause in the ko
        // test that exists for exactly this.
        let position = Position::from_picture(
            &[
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                "OO.......",
                ".XO......",
                ".XO......",
            ],
            Colour::White,
        );
        let sacrificed = play(&position, "a1").expect("White gives one away");
        let taken = play(&sacrificed, "a2").expect("Black takes it");
        assert_eq!(taken.prisoners()[Colour::White.index()], 1);
        assert_eq!(taken.ko(), None, "the capturing chain was three stones");

        let snapped = play(&taken, "a1").expect("and White takes all three back");
        assert_eq!(snapped.prisoners()[Colour::Black.index()], 3);
        for text in ["a2", "b1", "b2"] {
            assert_eq!(snapped.board().at(point(text)), None, "{text} came off");
        }
    }

    #[test]
    fn a_corner_group_with_two_eyes_cannot_be_taken() {
        // Six black stones in the corner, walled in by White, with the only two
        // liberties left being separate one-point eyes at a1 and c1. Neither
        // can be filled — a white stone in either has no liberty and captures
        // nothing, because the group still breathes through the other — and
        // that is the whole of what "alive" means.
        let position = Position::from_picture(
            &[
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                ".........",
                "OOOO.....",
                "XXXXO....",
                ".X.XO....",
            ],
            Colour::White,
        );
        let group = position.board().chain(point("b2")).expect("a stone");
        assert_eq!(group.stones().len(), 6);
        assert_eq!(group.liberties(), 2, "two eyes and nothing else");

        assert_eq!(play(&position, "a1"), Err(Illegal::Suicide));
        assert_eq!(play(&position, "c1"), Err(Illegal::Suicide));

        // And not by any other move either: whatever White plays, the group is
        // still there afterwards.
        for mv in position.plays() {
            let after = position.after(mv).expect("a legal move");
            assert_eq!(
                after.board().chain(point("b2")).map(|chain| chain.colour()),
                Some(Colour::Black),
                "{mv} took the group off"
            );
        }
    }

    #[test]
    fn every_empty_point_of_an_empty_board_is_a_move_and_so_is_passing() {
        let position = Position::empty(9).expect("a board");
        assert_eq!(position.plays().len(), 81 + 1);
    }
}
