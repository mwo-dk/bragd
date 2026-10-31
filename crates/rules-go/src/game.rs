//! A game: the record of what happened, and how it stops.
//!
//! **Two passes do not end a Go game with a result.** They end the *playing* of
//! it, and what follows is a conversation: the players agree which stones are
//! dead, and only then is there a score. That is why this has a third state
//! rather than an outcome — see [`State::Scoring`] — and it is the one place
//! where a game in Bragd cannot be settled by the engine alone. Which
//! stones are dead is never worked out here and never will be
//! ([`crate::score`] says why); it is proposed, countered and agreed, and those
//! are the entries this module records.
//!
//! **The negotiation is part of the record**, which is why the record is a list
//! of [`Entry`] and not a list of [`Move`]. The result depends on the agreed
//! dead set, so a stored game whose record does not contain that set cannot
//! be replayed to its own result — the same class of bug as a Chess960 game
//! that does not record where it started. The moves alone stop at `pass, pass`
//! and have no outcome in them. [`Game::moves`] is a projection of the record
//! for anything that wants the stones on their own; [`Game::entries`] is the
//! record.
//!
//! **There is no scoring clock here, because the negotiation is on the ordinary
//! one.** [`Game::to_move`] answers whose turn it is to act in every phase,
//! scoring included, so a player who walks away while the dead stones are being
//! argued about flags exactly as they would mid-game. A separate timer for the
//! scoring phase would be a second way to lose, running under different rules,
//! for the part of the game where two people are already disagreeing.
//!
//! Superko lives here for the same reason repetition lives on tafl's `Game`: it
//! is a fact about everything that has happened, and a position does not know
//! its own history. It is still enforced as **illegality inside move
//! generation** — a superko point is not a move — which is what makes it a ko
//! rule and not a draw claim. A resumption does not clear it: a position that
//! stood before the passes still forbids itself after them.

use crate::board::{Board, Colour, Point};
use crate::position::{Illegal, Move, Position};
use crate::ruleset::{Repetition, Ruleset, Scoring};
use crate::score::{self, Score};
use core::fmt;

/// Where a game has got to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum State {
    Playing,
    /// Both players have passed. Play is over and the result is not known yet:
    /// it waits on the two of them agreeing which stones are dead. Nothing more
    /// may be played, and no score exists until somebody supplies that set.
    Scoring,
    Over(Outcome),
}

/// How a game ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// Counted out. `winner` is `None` for *jigo*, a tie, which a half-point
    /// komi exists to make impossible and which is therefore only reachable
    /// with a whole-point komi or none at all.
    Counted {
        score: Score,
        winner: Option<Colour>,
    },
    /// Resignation, which is how most serious games actually end — a Go player
    /// who is behind by more than the remaining moves can bridge resigns rather
    /// than playing it out, and no score is ever taken.
    Resigned { winner: Colour },
}

impl Outcome {
    pub fn winner(self) -> Option<Colour> {
        match self {
            Outcome::Counted { winner, .. } => winner,
            Outcome::Resigned { winner } => Some(winner),
        }
    }
}

/// One thing that happened, in order: a move, or a step of the negotiation that
/// follows the last one.
///
/// The side is not stored on any of these because it is never in doubt. Entries
/// arrive in turn order, so a proposal is made by whoever it was turn to act and
/// answered by the other player — see [`Game::to_move`]. Carrying the colour as
/// well would be a second copy of a fact the game already knows, and two copies
/// of a fact eventually disagree.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Entry {
    Move(Move),
    /// A proposal: **the whole set** of stones claimed dead, never a delta.
    ///
    /// Whole, because a delta is only meaningful against the proposal it
    /// answers, and a record read from the middle — which is what a replay and
    /// a resumed session both are — has no such thing. It also makes the
    /// counter-proposal trivial: superseding an offer is one entry that says
    /// what the whole board should look like, not a removal and two additions
    /// that have to be applied in order.
    Mark(Vec<Point>),
    Accept,
    Resume,
}

/// The prefix a proposal is written with. `:` and not a space, so that one
/// entry is one word on the wire and nothing that splits a record on whitespace
/// can tear a proposal in half.
const MARK: &str = "dead:";

impl Entry {
    /// The wire form: `d4`, `pass`, `dead:b5,q16`, `dead:`, `accept`, `resume`.
    ///
    /// **A proposal has exactly one spelling.** The points are sorted and
    /// de-duplicated here rather than left as the caller wrote them, because the
    /// same set offered as `q16,b5` and as `b5,q16` is the same offer, and two
    /// records of the same game have to be byte-identical or a Library has two
    /// answers to "was this game already stored".
    ///
    /// **`dead:` with nothing after it is a real proposal**, not a malformed
    /// one: the empty set, meaning "nothing on this board is dead". It is the
    /// commonest offer there is on a game that was played out to the end, and a
    /// parser that treated the empty tail as an error would refuse the ordinary
    /// case.
    pub fn to_wire(&self) -> String {
        match self {
            Entry::Move(mv) => mv.to_wire(),
            Entry::Mark(dead) => {
                let names: Vec<String> = canonical(dead).iter().map(|point| point.name()).collect();
                format!("{MARK}{}", names.join(","))
            }
            Entry::Accept => String::from("accept"),
            Entry::Resume => String::from("resume"),
        }
    }

    pub fn parse(text: &str) -> Option<Entry> {
        if text.eq_ignore_ascii_case("accept") {
            return Some(Entry::Accept);
        }
        if text.eq_ignore_ascii_case("resume") {
            return Some(Entry::Resume);
        }
        if let Some(head) = text.get(..MARK.len())
            && head.eq_ignore_ascii_case(MARK)
        {
            let tail = &text[MARK.len()..];
            if tail.is_empty() {
                return Some(Entry::Mark(Vec::new()));
            }
            // No `filter`, no `trim`: an empty word between two commas is a
            // typo, and `Point::parse` refuses it. A parser that skipped the
            // blanks would read `dead:b5,,q16` and `dead:b5,q16` as one offer
            // with two spellings, which is the thing the canonical form exists
            // to stop.
            let mut dead = Vec::new();
            for word in tail.split(',') {
                dead.push(Point::parse(word)?);
            }
            return Some(Entry::Mark(canonical(&dead)));
        }
        Move::parse(text).map(Entry::Move)
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_wire())
    }
}

/// A dead-stone set in its one spelling: in the board's own index order, with
/// no point named twice.
///
/// Sorted by row and then column — the order [`crate::Board::index`] walks, the
/// bottom-left corner first — and deliberately **not** by `Point`'s derived
/// `Ord`, which compares the column first and would run up the left-hand edge.
/// Either would be canonical; this one is the order a board is sent to a front end
/// in, so a record and a board array read the same way round.
fn canonical(dead: &[Point]) -> Vec<Point> {
    let mut dead = dead.to_vec();
    dead.sort_by_key(|point| (point.row(), point.column()));
    dead.dedup();
    dead
}

/// The offer on the table: who made it, and which stones it claims are dead.
///
/// There is at most one, and it is the *whole* claim rather than the difference
/// from the last one — see [`Entry::Mark`]. A counter-proposal replaces it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Proposal {
    pub by: Colour,
    pub dead: Vec<Point>,
}

/// Which negotiation entries the side to act may send.
///
/// Three flags and not a `Vec<Entry>`, because one of the three carries an
/// argument: a `Mark` in such a list would need a set inside it, and any set
/// this crate chose would be the engine having an opinion about which stones
/// are dead — the one thing it refuses to do. So what is enumerated is the
/// *forms* that are open, and the set is the caller's to supply.
///
/// All three are false unless the game is in [`State::Scoring`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Available {
    /// A proposal may be made: the first one, or one that supersedes the
    /// standing offer. Always true while scoring — there is no position in
    /// which a player may not say what they think is dead.
    pub mark: bool,
    /// There is an offer to accept. Never true for the player who made it,
    /// because it is not their turn to act.
    pub accept: bool,
    /// Play may be resumed. Only while an offer stands: two passes followed by
    /// a bare `resume` would otherwise be a way to unilaterally take a pass
    /// back.
    pub resume: bool,
}

/// A game in progress: the rules, the position, the record, and every board
/// position that has stood.
#[derive(Clone)]
pub struct Game {
    rules: Ruleset,
    position: Position,
    entries: Vec<Entry>,
    /// Every board that has been on the table, each with its key. Kept in full
    /// rather than as hashes alone because superko makes a repetition an
    /// *illegal move*, and a hash collision would then forbid a move the rules
    /// allow — the same argument the tafl engine makes for storing positions
    /// whole. The key is there so that the comparison happens once rather than
    /// once per board ever played.
    seen: Vec<(u64, Board)>,
    passes: u8,
    /// The offer on the table, if there is one. `None` in every phase but
    /// [`State::Scoring`], and `None` at the start of each negotiation: two
    /// passes put nothing on the table, they only give somebody the floor.
    proposal: Option<Proposal>,
    state: State,
}

impl Game {
    pub fn new(rules: Ruleset) -> Game {
        Game::from_position(rules, rules.starting_position())
    }

    /// A game from a position already on the board. The position's size wins if
    /// it disagrees with the ruleset's — a diagram is a diagram.
    pub fn from_position(rules: Ruleset, position: Position) -> Game {
        let seen = vec![(position.board().key(), position.board().clone())];
        Game {
            rules,
            position,
            entries: Vec::new(),
            seen,
            passes: 0,
            proposal: None,
            state: State::Playing,
        }
    }

    pub fn rules(&self) -> &Ruleset {
        &self.rules
    }

    pub fn position(&self) -> &Position {
        &self.position
    }

    /// The record: everything that happened, in order, including the
    /// negotiation. This is what the platform stores and replays.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The plays alone, for anything that wants the stones without the
    /// argument about them.
    ///
    /// Computed from the record rather than kept beside it. A second list to
    /// keep in step is a second list to get out of step, and the one that would
    /// go stale is the one that decides the result.
    pub fn moves(&self) -> Vec<Move> {
        self.entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Move(mv) => Some(*mv),
                _ => None,
            })
            .collect()
    }

    /// How many passes stand in a row. One means the next pass stops the play.
    ///
    /// Read from here and **not by counting passes off the end of the record**:
    /// a resumption resets this to zero while leaving the two passes in the
    /// record where they happened, so a front end that counted them would announce
    /// that the next pass ends a game that has only just restarted.
    pub fn passes(&self) -> u8 {
        self.passes
    }

    /// The offer on the table, if there is one.
    pub fn proposal(&self) -> Option<&Proposal> {
        self.proposal.as_ref()
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    /// Whose turn it is to act, **in any phase**.
    ///
    /// While the game is being played this is the position's side to move.
    /// While it is being scored it is the player who has to answer the standing
    /// offer — the other one from whoever made it — or, before anybody has
    /// offered anything, the side the position says is to move, which after two
    /// passes is the player who passed first.
    ///
    /// This is what a clock is driven by, which is why it has to be right in
    /// the scoring phase and not only while stones are going down: the
    /// negotiation runs on the players' ordinary time, and that clock is the
    /// protocol's timeout. A boundary that sent `position.to_move()` instead
    /// would run the wrong player's clock for the whole of the argument, and
    /// the player who walked away would be the one it kept waiting for.
    pub fn to_move(&self) -> Colour {
        match &self.proposal {
            Some(proposal) => proposal.by.other(),
            None => self.position.to_move(),
        }
    }

    /// Which negotiation entries are open to the side to act.
    ///
    /// Separate from [`Game::legal_moves`] because a proposal takes a set as
    /// its argument and so cannot be enumerated — see [`Available`]. While the
    /// game is being played, nothing here is open and the moves are the answer;
    /// while it is being scored, the moves are empty and this is.
    pub fn available(&self) -> Available {
        if self.state != State::Scoring {
            return Available {
                mark: false,
                accept: false,
                resume: false,
            };
        }
        let standing = self.proposal.is_some();
        Available {
            mark: true,
            accept: standing,
            resume: standing,
        }
    }

    pub fn outcome(&self) -> Option<Outcome> {
        match self.state {
            State::Over(outcome) => Some(outcome),
            _ => None,
        }
    }

    /// Every move that may be played, passing included.
    ///
    /// Passing is always here while the game is being played. There is no
    /// position in Go where a player must put a stone down, which is why there
    /// is no stalemate in this game and no rule about having nothing to do.
    ///
    /// Empty once the play has stopped, and that is not "no moves available" —
    /// it is "this game is not taking moves". [`Game::available`] is what says
    /// what may be sent then.
    pub fn legal_moves(&self) -> Vec<Move> {
        if self.state != State::Playing {
            return Vec::new();
        }
        let mut moves: Vec<Move> = self
            .position
            .board()
            .points()
            .filter(|&point| match self.position.after(Move::Play(point)) {
                Ok(next) => !self.forbidden_by_superko(next.board()),
                Err(_) => false,
            })
            .map(Move::Play)
            .collect();
        moves.push(Move::Pass);
        moves
    }

    pub fn play(&mut self, mv: Move) -> Result<(), MoveError> {
        if self.state != State::Playing {
            return Err(MoveError::NotPlaying);
        }
        let next = self.position.after(mv).map_err(MoveError::Illegal)?;
        if matches!(mv, Move::Play(_)) && self.forbidden_by_superko(next.board()) {
            return Err(MoveError::Superko);
        }

        self.position = next;
        self.entries.push(Entry::Move(mv));
        match mv {
            // A pass leaves the board exactly as it was, so there is nothing to
            // record and nothing it could repeat. Two of them in a row and the
            // playing is over.
            Move::Pass => {
                self.passes += 1;
                if self.passes >= 2 {
                    self.state = State::Scoring;
                }
            }
            Move::Play(_) => {
                self.passes = 0;
                let board = self.position.board();
                self.seen.push((board.key(), board.clone()));
            }
        }
        Ok(())
    }

    /// Records an entry, whatever phase the game is in, and applies it.
    ///
    /// The routing is the protocol, so it is written out in one place:
    ///
    /// - While the game is being **played**, a move is a move and the three
    ///   negotiation entries are refused. There is nothing to negotiate about
    ///   until the play has stopped.
    /// - On entering **scoring** nothing stands on the table, and the side to
    ///   act — the position's, which after two passes is the player who passed
    ///   first — may only propose. It cannot accept, because there is nothing
    ///   to accept; it cannot resume, because that would be taking its own pass
    ///   back.
    /// - Once an offer stands it is the **other** player's turn, and they may
    ///   accept it, counter it with an offer of their own, or resume play.
    /// - A counter that names exactly the standing set is not refused. It is a
    ///   pointless entry and it hands the turn back, and two players who
    ///   politely re-offer the same set forever will run out of clock, which is
    ///   the right way for that to end and not the engine's business.
    pub fn play_entry(&mut self, entry: Entry) -> Result<(), MoveError> {
        if matches!(self.state, State::Over(_)) {
            return Err(MoveError::NotPlaying);
        }
        match entry {
            Entry::Move(mv) => self.play(mv),
            _ if self.state != State::Scoring => Err(MoveError::NotScoring),
            Entry::Mark(dead) => {
                let dead = canonical(&dead);
                // **Either colour, and the engine judges nothing.** Conceding
                // your own dead groups is ordinary — it is how most games are
                // agreed — so the only thing checked is that there is a stone
                // there to be talked about. A point with nothing on it is a
                // front end with a stale board or a caller with the wrong game, and
                // it would score as nothing at all rather than as an error.
                for &point in &dead {
                    if self.position.board().at(point).is_none() {
                        return Err(MoveError::NotAStone(point));
                    }
                }
                // Read before the offer is replaced: with one standing, the
                // author of the new one is the player answering it.
                let by = self.to_move();
                self.proposal = Some(Proposal {
                    by,
                    dead: dead.clone(),
                });
                self.entries.push(Entry::Mark(dead));
                Ok(())
            }
            Entry::Accept => {
                let Some(proposal) = &self.proposal else {
                    return Err(MoveError::NoProposal);
                };
                // Accepting your own offer is not possible and needs no check:
                // while it stands it is never your turn to act.
                let dead = proposal.dead.clone();
                self.settle(&dead)?;
                self.entries.push(Entry::Accept);
                // The offer is left standing rather than cleared. It is now the
                // *agreed* set — the one the board was counted with — and a
                // finished game that could not say what it was counted with is
                // the whole bug this protocol exists to prevent.
                Ok(())
            }
            Entry::Resume => {
                if self.proposal.is_none() {
                    return Err(MoveError::NoProposal);
                }
                self.proposal = None;
                self.passes = 0;
                self.state = State::Playing;
                self.entries.push(Entry::Resume);
                // **The position's turn simply stands, and that absence is the
                // rule.** Nobody gets a free move for having resumed and
                // nobody is punished for it: the passes were moves, so turn
                // order was never broken, and the player to move now is the one
                // whose turn it was when the second pass ended the play. Every
                // alternative — "the resumer moves", "the opponent of the
                // resumer moves" — hands one player an extra move, and it is
                // the rule most implementations get wrong.
                //
                // `seen` is not cleared either. A board that has stood is still
                // forbidden from standing again; a resumption is not a new
                // game, and superko that forgot its history would make a ko
                // cycle winnable by passing twice.
                Ok(())
            }
        }
    }

    /// Reads an entry off the wire and applies it. The one way in from the
    /// platform, which stores these as opaque strings.
    pub fn play_wire(&mut self, text: &str) -> Result<(), MoveError> {
        let entry = Entry::parse(text).ok_or_else(|| MoveError::NotUnderstood(text.to_string()))?;
        self.play_entry(entry)
    }

    /// Whether a board has stood before, under a ruleset that forbids it
    /// standing again.
    ///
    /// The key rules out almost every position without looking at it; a match
    /// is then confirmed by comparing the boards themselves, because forbidding
    /// a legal move on the strength of a hash collision would be a rules bug
    /// that could not be reproduced.
    fn forbidden_by_superko(&self, board: &Board) -> bool {
        if self.rules.repetition != Repetition::PositionalSuperko {
            return false;
        }
        let key = board.key();
        self.seen
            .iter()
            .any(|(seen, before)| *seen == key && before == board)
    }

    /// Resignation. Allowed while the dead stones are still being argued
    /// about, which is not a loophole: a player who would rather concede than
    /// have the argument is entitled to.
    pub fn resign(&mut self, resigning: Colour) -> Result<Outcome, MoveError> {
        if matches!(self.state, State::Over(_)) {
            return Err(MoveError::NotPlaying);
        }
        let outcome = Outcome::Resigned {
            winner: resigning.other(),
        };
        self.state = State::Over(outcome);
        Ok(outcome)
    }

    /// What the board is worth, given a set of dead stones.
    ///
    /// Answers at any time, not only at the end — Bragd wants a running
    /// estimate, and the arithmetic does not care whether the players have
    /// finished. It is [`Game::settle`] that insists on the game being over.
    pub fn score(&self, dead: &[Point]) -> Score {
        match self.rules.scoring {
            Scoring::Area => score::area(self.position.board(), dead, self.rules.komi_halves),
            Scoring::Territory => score::territory(
                self.position.board(),
                dead,
                self.position.prisoners(),
                self.rules.komi_halves,
            ),
        }
    }

    /// Ends the game on an agreed set of dead stones.
    ///
    /// The moment a Go game acquires an outcome. The ordinary way to reach it
    /// is [`Entry::Accept`], which hands the standing proposal straight to
    /// here. It stays public because a game can also be settled from outside
    /// the negotiation — by an arbiter, or by a server resolving a timeout — but
    /// a game settled that way leaves no entry behind saying which stones were
    /// dead, so its record cannot be replayed to its own result. That is the
    /// bug the negotiation exists to prevent, and this door is the one place it
    /// is still open.
    pub fn settle(&mut self, dead: &[Point]) -> Result<Outcome, MoveError> {
        if self.state != State::Scoring {
            return Err(MoveError::NotScoring);
        }
        let score = self.score(dead);
        let outcome = Outcome::Counted {
            score,
            winner: score.winner(),
        };
        self.state = State::Over(outcome);
        Ok(outcome)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MoveError {
    /// The game is not taking moves: it is being counted, or it is over.
    NotPlaying,
    Illegal(Illegal),
    /// The board has stood exactly like this before, and this ruleset says a
    /// position may not come round twice.
    Superko,
    /// Both players have not passed, so there is nothing to count yet and
    /// nothing to negotiate about.
    NotScoring,
    /// An answer arrived with no offer to answer. Accepting nothing is
    /// meaningless; resuming with nothing on the table is a player taking their
    /// own pass back, which is why it is refused rather than treated as a
    /// change of mind.
    NoProposal,
    /// A proposal named a point with no stone on it. Either colour's stones may
    /// be called dead — conceding your own groups is ordinary — but an empty
    /// point is a stale board rather than a disagreement.
    NotAStone(Point),
    NotUnderstood(String),
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoveError::NotPlaying => f.write_str("this game is not taking moves"),
            MoveError::Illegal(why) => write!(f, "{why}"),
            MoveError::Superko => f.write_str("that would repeat a position the game has had"),
            MoveError::NotScoring => f.write_str("this game has not been played out yet"),
            MoveError::NoProposal => f.write_str("there is no proposal on the table"),
            MoveError::NotAStone(point) => write!(f, "there is no stone on {point} to be dead"),
            MoveError::NotUnderstood(text) => write!(f, "{text:?} is not an entry"),
        }
    }
}

impl std::error::Error for MoveError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn point(text: &str) -> Point {
        Point::parse(text).expect("a point")
    }

    /// A 9×9 split down the middle — a black wall on the d file, a white wall
    /// on the e file — with one white stone at b5 alone inside Black's half.
    ///
    /// The board the whole negotiation is about. Left standing, that stone
    /// makes the entire left-hand side dame and Black has nine points; agreed
    /// dead, the same board gives Black twenty-seven more. Twenty-seven points
    /// of difference, from a set of stones no engine put there.
    fn split() -> Position {
        Position::from_picture(
            &[
                "...XO....",
                "...XO....",
                "...XO....",
                "...XO....",
                ".O.XO....",
                "...XO....",
                "...XO....",
                "...XO....",
                "...XO....",
            ],
            Colour::Black,
        )
    }

    /// The board of [`split`], two passes in, with nobody having offered
    /// anything yet.
    fn played_out() -> Game {
        let mut game = Game::from_position(Ruleset::CHINESE.on(9), split());
        game.play_wire("pass").expect("Black passes");
        game.play_wire("pass").expect("White passes");
        assert_eq!(*game.state(), State::Scoring);
        game
    }

    /// Two ko shapes on one board, far apart: White holds one at d3 and Black
    /// holds the other at h7, each a single stone with a single liberty.
    fn double_ko() -> Position {
        Position::from_picture(
            &[
                ".........",
                "......XO.",
                ".....X.XO",
                "......XO.",
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
    fn a_new_game_has_black_to_play_and_every_point_available() {
        let game = Game::new(Ruleset::CHINESE.on(9));
        assert_eq!(game.position().to_move(), Colour::Black);
        assert_eq!(game.legal_moves().len(), 81 + 1, "and passing");
        assert_eq!(*game.state(), State::Playing);
    }

    #[test]
    fn two_passes_stop_the_play_without_deciding_anything() {
        let mut game = Game::new(Ruleset::CHINESE.on(9));
        game.play(Move::Pass).expect("passing is legal");
        assert_eq!(*game.state(), State::Playing, "one pass is just a move");
        game.play(Move::Pass).expect("and so is the second");

        assert_eq!(*game.state(), State::Scoring);
        assert_eq!(
            game.outcome(),
            None,
            "nobody has said which stones are dead"
        );
        assert!(game.legal_moves().is_empty());
        assert_eq!(game.play(Move::Pass), Err(MoveError::NotPlaying));
    }

    #[test]
    fn passes_have_to_be_consecutive_to_end_the_play() {
        let mut game = Game::new(Ruleset::CHINESE.on(9));
        game.play(Move::Pass).expect("Black passes");
        game.play_wire("d4").expect("White plays");
        game.play(Move::Pass).expect("Black passes again");
        assert_eq!(*game.state(), State::Playing);
    }

    #[test]
    fn the_dead_stones_come_from_outside_and_decide_the_result() {
        // The same finished board scored twice, with and without a white stone
        // agreed dead — which is the whole reason the set is an argument and
        // not something the engine works out.
        let mut game = played_out();
        let alive = game.score(&[]);
        let dead = game.score(&[point("b5")]);
        assert_eq!(alive.black_halves, 2 * 9);
        assert_eq!(dead.black_halves, 2 * 36);

        assert_eq!(
            game.settle(&[point("b5")]).expect("counted"),
            Outcome::Counted {
                score: dead,
                winner: Some(Colour::White),
            }
        );
        assert_eq!(
            game.play(Move::Pass),
            Err(MoveError::NotPlaying),
            "and it is over"
        );

        let mut unfinished = Game::from_position(Ruleset::CHINESE.on(9), double_ko());
        assert_eq!(
            unfinished.settle(&[]),
            Err(MoveError::NotScoring),
            "nothing to count until both have passed"
        );
    }

    #[test]
    fn a_dead_set_has_exactly_one_spelling_however_it_is_written() {
        // Two records of the same game have to come out byte-identical, or a
        // Library has two answers to whether it has already stored a game. So
        // the order the points arrive in is not the order they are written in,
        // and a point named twice is still a set.
        assert_eq!(Entry::Mark(vec![point("q16"), point("b5")]).to_wire(), {
            "dead:b5,q16"
        });
        assert_eq!(Entry::parse("dead:q16,b5"), Entry::parse("dead:b5,q16"));
        assert_eq!(
            Entry::Mark(vec![point("b5"), point("b5")]).to_wire(),
            "dead:b5",
            "a set, not a list"
        );

        // **The empty tail is deliberate.** `dead:` is the proposal that
        // nothing on this board is dead, which is much the commonest offer
        // there is on a game played out to the end — a parser that read the
        // missing points as a malformed entry would refuse the ordinary case.
        assert_eq!(Entry::Mark(Vec::new()).to_wire(), "dead:");
        assert_eq!(Entry::parse("dead:"), Some(Entry::Mark(Vec::new())));

        for entry in [
            Entry::Mark(Vec::new()),
            Entry::Mark(vec![point("b5"), point("q16")]),
            Entry::Accept,
            Entry::Resume,
            Entry::Move(Move::Pass),
            Entry::Move(Move::Play(point("d4"))),
        ] {
            assert_eq!(
                Entry::parse(&entry.to_wire()),
                Some(entry.clone()),
                "{entry}"
            );
        }

        for text in [
            "dead:b5,",
            "dead:,b5",
            "dead:b5,,q16",
            "dead: b5",
            "dead:i5",
            "dead:d04",
            "dead",
            "deadb5",
            "accepted",
            "",
        ] {
            assert_eq!(Entry::parse(text), None, "parsed {text:?}");
        }
    }

    /// **The test the whole design exists for.**
    ///
    /// The result of a Go game is a number that depends on the agreed set of
    /// dead stones. Replaying the *moves* of this game stops at `pass, pass`
    /// and produces no result at all; replaying the *record* reaches the
    /// identical outcome, because the record contains the agreement. A record
    /// that could not do this is the same class of bug as a Chess960 game that
    /// does not say where it started: it looks complete and stores a result
    /// nothing in it can account for.
    #[test]
    fn a_whole_negotiation_replays_from_its_wire_record_to_the_identical_outcome() {
        let mut game = Game::from_position(Ruleset::CHINESE.on(9), split());
        for text in ["pass", "pass", "dead:b5", "accept"] {
            game.play_wire(text)
                .unwrap_or_else(|why| panic!("{text}: {why}"));
        }
        let outcome = game.outcome().expect("the game was counted");

        let record: Vec<String> = game.entries().iter().map(Entry::to_wire).collect();
        assert_eq!(record, ["pass", "pass", "dead:b5", "accept"]);
        assert_eq!(
            game.moves(),
            vec![Move::Pass, Move::Pass],
            "the plays alone are not the record, and this is what they are missing"
        );

        let mut replayed = Game::from_position(Ruleset::CHINESE.on(9), split());
        for text in &record {
            replayed
                .play_wire(text)
                .unwrap_or_else(|why| panic!("{text}: {why}"));
        }
        assert_eq!(replayed.outcome(), Some(outcome));
        assert_eq!(replayed.entries(), game.entries());
    }

    #[test]
    fn the_turn_alternates_across_the_negotiation_so_nobody_answers_their_own_proposal() {
        let mut game = played_out();
        // Nothing stands, so the side to act is the position's — Black, who
        // passed first — and the only thing it may do is offer.
        assert_eq!(game.to_move(), Colour::Black);
        assert_eq!(
            game.available(),
            Available {
                mark: true,
                accept: false,
                resume: false
            }
        );

        game.play_wire("dead:b5").expect("Black offers");
        assert_eq!(game.to_move(), Colour::White, "White has to answer it");
        assert_eq!(
            game.available(),
            Available {
                mark: true,
                accept: true,
                resume: true
            }
        );

        // A counter-proposal supersedes the standing one whole; it is not a
        // difference from it.
        game.play_wire("dead:").expect("White says nothing is dead");
        assert_eq!(
            game.proposal(),
            Some(&Proposal {
                by: Colour::White,
                dead: Vec::new()
            })
        );
        assert_eq!(game.to_move(), Colour::Black, "and now Black must answer");

        // The position has not moved through any of this. `to_move` is not
        // `position.to_move()` while a game is being scored, and a clock driven
        // by the position would have been running Black's time throughout.
        assert_eq!(game.position().to_move(), Colour::Black);

        game.play_wire("accept").expect("Black accepts the counter");
        assert_eq!(
            game.outcome(),
            Some(Outcome::Counted {
                score: game.score(&[]),
                winner: game.score(&[]).winner(),
            }),
            "counted on the counter-proposal, not on the offer it replaced"
        );
    }

    #[test]
    fn resuming_needs_a_proposal_to_resume_from() {
        // Otherwise two passes and a bare `resume` would be a way to take a
        // pass back unilaterally: the play would restart with the opponent
        // having spent a move on nothing.
        let mut game = played_out();
        assert_eq!(game.play_wire("resume"), Err(MoveError::NoProposal));
        assert_eq!(game.play_wire("accept"), Err(MoveError::NoProposal));
        assert_eq!(*game.state(), State::Scoring, "and nothing has changed");
        assert!(
            game.entries()
                .iter()
                .all(|entry| { matches!(entry, Entry::Move(_)) })
        );
    }

    #[test]
    fn resuming_leaves_the_turn_where_the_passes_left_it_and_the_history_still_stands() {
        let mut game = Game::from_position(Ruleset::CHINESE.on(9), double_ko());
        for text in ["c3", "g7", "pass", "d3"] {
            game.play_wire(text)
                .unwrap_or_else(|why| panic!("{text}: {why}"));
        }
        assert_eq!(game.to_move(), Colour::Black);
        assert_eq!(
            game.play_wire("h7"),
            Err(MoveError::Superko),
            "it would bring the whole board round to a position it has had"
        );

        game.play_wire("pass").expect("Black passes");
        game.play_wire("pass").expect("White passes");
        game.play_wire("dead:").expect("Black offers nothing dead");
        game.play_wire("resume")
            .expect("White would rather play on");

        assert_eq!(*game.state(), State::Playing);
        assert_eq!(game.proposal(), None, "the table is cleared");
        assert_eq!(game.passes(), 0, "and the next pass is the first of two");
        // **The turn simply stands.** It is Black's move because it was
        // Black's move: the passes were moves, so turn order was never broken,
        // and neither player gained or lost one by resuming.
        assert_eq!(game.to_move(), Colour::Black);
        // And a position that stood before the passes still forbids itself
        // after them. A resumption is not a new game, so the superko history
        // is not cleared — otherwise passing twice and resuming would be a way
        // to win a ko cycle by forgetting it.
        assert_eq!(game.play_wire("h7"), Err(MoveError::Superko));

        game.play_wire("pass").expect("Black passes again");
        game.play_wire("pass").expect("and so does White");
        assert_eq!(*game.state(), State::Scoring);
        assert_eq!(game.proposal(), None, "a fresh negotiation, from nothing");
    }

    #[test]
    fn a_proposal_may_not_name_a_point_with_nothing_on_it() {
        // The engine makes no life-and-death judgement, so a proposal is barely
        // checked at all — but a point with no stone on it is not a
        // disagreement about life and death, it is a front end working from a board
        // that has moved on, and it would score as nothing whatsoever.
        let mut game = played_out();
        assert_eq!(
            game.play_wire("dead:a1"),
            Err(MoveError::NotAStone(point("a1")))
        );
        assert_eq!(
            game.play_wire("dead:b5,a1"),
            Err(MoveError::NotAStone(point("a1"))),
            "and one bad point spoils the whole proposal"
        );
        assert_eq!(game.proposal(), None, "nothing was put on the table");
    }

    #[test]
    fn conceding_your_own_stones_is_an_ordinary_proposal() {
        // Either colour may be named dead, and the commonest way a real game is
        // agreed is the losing player conceding their own groups. An engine
        // that only let you kill the opponent's stones would make agreement
        // impossible for the person who has to do the conceding — and deciding
        // for itself whose stones may be named would be the life-and-death
        // judgement this crate refuses to make.
        let mut game = played_out();
        assert_eq!(game.to_move(), Colour::Black);
        game.play_wire("dead:d5")
            .expect("Black gives away a stone of its own");
        assert_eq!(
            game.proposal(),
            Some(&Proposal {
                by: Colour::Black,
                dead: vec![point("d5")]
            })
        );

        game.play_wire("accept").expect("White accepts");
        assert_eq!(
            game.outcome(),
            Some(Outcome::Counted {
                score: game.score(&[point("d5")]),
                winner: game.score(&[point("d5")]).winner(),
            })
        );
        assert_eq!(
            game.play_wire("dead:"),
            Err(MoveError::NotPlaying),
            "and the negotiation is closed with it"
        );
    }

    #[test]
    fn nothing_may_be_negotiated_before_the_play_has_stopped() {
        let mut game = Game::from_position(Ruleset::CHINESE.on(9), split());
        for text in ["dead:b5", "accept", "resume"] {
            assert_eq!(game.play_wire(text), Err(MoveError::NotScoring), "{text}");
        }
        game.play_wire("pass").expect("Black passes");
        assert_eq!(
            game.play_wire("dead:b5"),
            Err(MoveError::NotScoring),
            "one pass is just a move"
        );
    }

    #[test]
    fn resigning_ends_a_game_that_was_still_being_played() {
        let mut game = Game::new(Ruleset::JAPANESE.on(9));
        game.play_wire("e5").expect("a move");
        assert_eq!(
            game.resign(Colour::White).expect("White resigns"),
            Outcome::Resigned {
                winner: Colour::Black
            }
        );
        assert!(game.resign(Colour::Black).is_err(), "it cannot end twice");
    }

    #[test]
    fn simple_ko_lets_a_double_ko_come_round_again_and_superko_does_not() {
        // The case that separates the two repetition rules, and it needs two
        // kos to build: with one, the immediate retake is banned and that is
        // the end of it. With two, each player takes their own ko, waits a
        // move, and takes it back — no retake is ever *immediate*, so simple ko
        // never objects, and after four exchanges the whole board is exactly as
        // it started. Under Japanese rules this is a real position and the real
        // answer is that the game is void; here it is simply legal, and the
        // ruleset that forbids it is the one that compares whole boards.
        let opening = double_ko().board().clone();

        let mut japanese = Game::from_position(Ruleset::JAPANESE.on(9), double_ko());
        for text in ["c3", "g7", "pass", "d3", "h7"] {
            japanese
                .play_wire(text)
                .unwrap_or_else(|why| panic!("{text}: {why}"));
        }
        assert_eq!(
            japanese.position().board(),
            &opening,
            "the board has come all the way round"
        );

        let mut chinese = Game::from_position(Ruleset::CHINESE.on(9), double_ko());
        for text in ["c3", "g7", "pass", "d3"] {
            chinese
                .play_wire(text)
                .unwrap_or_else(|why| panic!("{text}: {why}"));
        }
        assert_eq!(chinese.play_wire("h7"), Err(MoveError::Superko));
        assert!(
            !chinese.legal_moves().contains(&Move::Play(point("h7"))),
            "and it is not offered as a move either"
        );
    }

    #[test]
    fn the_ko_ban_is_a_move_that_is_not_offered_rather_than_a_claim() {
        let mut game = Game::from_position(Ruleset::JAPANESE.on(9), double_ko());
        game.play_wire("c3").expect("Black takes the ko");
        assert!(!game.legal_moves().contains(&Move::Play(point("d3"))));
        assert_eq!(
            game.play_wire("d3"),
            Err(MoveError::Illegal(Illegal::Ko)),
            "and playing it anyway is refused"
        );
    }

    /// A small linear congruential generator, seeded from a constant, so that
    /// the random playouts below are the same random playouts on every machine
    /// and every run. It lives in the tests and nowhere else: an engine that
    /// generated numbers would be an engine whose results depended on
    /// something other than its inputs.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 33
        }

        fn below(&mut self, count: usize) -> usize {
            (self.next() % count as u64) as usize
        }
    }

    /// Plays random stones — never a pass — until the board runs out of legal
    /// moves or `moves` have been played, calling `watch` after each one.
    fn playout(rules: Ruleset, seed: u64, moves: usize, mut watch: impl FnMut(&Game)) -> Game {
        let mut game = Game::new(rules);
        let mut rng = Rng(seed);
        for _ in 0..moves {
            let stones: Vec<Move> = game
                .legal_moves()
                .into_iter()
                .filter(|mv| !matches!(mv, Move::Pass))
                .collect();
            if stones.is_empty() {
                break;
            }
            let mv = stones[rng.below(stones.len())];
            game.play(mv).expect("a move that was just offered");
            watch(&game);
        }
        game
    }

    #[test]
    fn no_random_playout_ever_leaves_a_stone_without_a_liberty() {
        // The invariant that stands in for perft here. A group with no
        // liberties on the board means a capture was missed or a suicide was
        // allowed, and either is the kind of bug that would otherwise be found
        // by a player in a real game.
        for seed in [1, 2, 3] {
            playout(Ruleset::CHINESE.on(13), seed, 400, |game| {
                let board = game.position().board();
                for point in board.points() {
                    if board.at(point).is_some() {
                        assert!(
                            board.has_liberty(point),
                            "{point} is on the board with nothing to breathe:\n{board:?}"
                        );
                    }
                }
            });
        }
    }

    #[test]
    fn positional_superko_never_lets_a_whole_board_position_come_round_twice() {
        for seed in [4, 5, 6] {
            let mut boards: HashSet<Board> = HashSet::new();
            let game = playout(Ruleset::CHINESE.on(13), seed, 400, |game| {
                let board = game.position().board().clone();
                assert!(
                    boards.insert(board),
                    "a position repeated:\n{:?}",
                    game.position()
                );
            });
            assert!(
                game.moves().len() > 200,
                "the playout has to actually play, and it played {}",
                game.moves().len()
            );
        }
    }

    #[test]
    fn area_scoring_never_hands_out_more_than_the_board() {
        // The looser half of the invariant that `crate::score` asserts exactly
        // on a constructed board: a random position is full of dame, dame
        // belongs to nobody, and so the two areas fall short of the board
        // rather than overshooting it. Komi is set aside for this, because komi
        // is not a point on the board.
        let rules = Ruleset {
            komi_halves: 0,
            ..Ruleset::CHINESE
        }
        .on(9);
        let game = playout(rules, 7, 200, |_| {});
        let score = game.score(&[]);
        assert!(
            score.black_halves + score.white_halves
                <= 2 * game.position().board().intersections() as i32
        );
    }
}
