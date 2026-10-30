//! A position, and the moves out of it.
//!
//! The capture generator is the whole of the difficulty, and three rules in it
//! are the ones implementations get wrong. Each is named where it is enforced.

use crate::board::{Bitboard, Direction, Geometry, Square};
use crate::ruleset::{Promotion, Ruleset};
use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Side {
    /// Moves up the board, towards the far rank.
    White,
    /// Moves down.
    Black,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::White, Side::Black];

    pub const fn other(self) -> Side {
        match self {
            Side::White => Side::Black,
            Side::Black => Side::White,
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Side::White => 0,
            Side::Black => 1,
        }
    }

    /// Whether this side's men move towards the higher ranks.
    pub const fn is_upward(self) -> bool {
        matches!(self, Side::White)
    }
}

/// A move: where a piece started, where it stopped, and what it took.
///
/// **The path is deliberately not here.** A capture sequence can wander, and
/// with a flying king it can wander in several ways that end identically — but
/// what survives the turn is only the origin, the destination and the set of
/// captured pieces. Two sequences agreeing on those three produce the same
/// position and are the same move.
///
/// That is not a simplification, it is the definition the published perft
/// tables use when they say "duplicates removed" — and the bug they exist to
/// catch is a generator that emits both.
///
/// **`crowned` is not the path coming back in.** It is one bit of the path's
/// *effect*: whether the man became a king somewhere in the middle of the
/// sequence rather than at the end of it. Russian draughts is the only ruleset
/// here that works that way, and without this bit the move is genuinely
/// under-determined — the same origin, destination and capture set can leave a
/// man or a king on the board, which are different positions.
///
/// It stays out of the "same move" question for every other ruleset by
/// construction: nothing but [`Promotion::MidSequence`] can set it, and only
/// `RUSSIAN` uses that. So the cross-verified International and English perft
/// numbers cannot move because of this field.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub captured: Bitboard,
    /// Whether the mover was crowned **during** the sequence rather than by
    /// stopping on the far rank. Always false outside Russian.
    pub crowned: bool,
}

impl Move {
    pub fn is_capture(self) -> bool {
        !self.captured.is_empty()
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.from.index(),
            if self.is_capture() { "x" } else { "-" },
            self.to.index()
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    geometry: Geometry,
    /// `by_side[side]` — every piece of that side, kings included.
    by_side: [Bitboard; 2],
    /// The kings, of both sides.
    kings: Bitboard,
    to_move: Side,
}

impl Position {
    /// The opening array: men on the first four ranks each for International,
    /// three each for the 8×8 variants, with two empty ranks between.
    pub fn start(geometry: Geometry) -> Position {
        let filled = if geometry.side == 10 { 4 } else { 3 };
        let mut white = Bitboard::EMPTY;
        let mut black = Bitboard::EMPTY;
        for index in 0..geometry.squares() as u8 {
            let square = Square::new(index);
            let (_, rank) = geometry.file_rank(square);
            if rank < filled {
                white = white.with(square);
            } else if rank >= geometry.side - filled {
                black = black.with(square);
            }
        }
        Position {
            geometry,
            by_side: [white, black],
            kings: Bitboard::EMPTY,
            // White moves first in every variant this crate plays.
            to_move: Side::White,
        }
    }

    pub fn geometry(self) -> Geometry {
        self.geometry
    }

    pub fn to_move(self) -> Side {
        self.to_move
    }

    pub fn side(self, side: Side) -> Bitboard {
        self.by_side[side.index()]
    }

    pub fn kings(self) -> Bitboard {
        self.kings
    }

    pub fn occupied(self) -> Bitboard {
        self.by_side[0].union(self.by_side[1])
    }

    pub fn is_king(self, square: Square) -> bool {
        self.kings.contains(square)
    }

    pub fn side_at(self, square: Square) -> Option<Side> {
        Side::ALL
            .into_iter()
            .find(|&side| self.side(side).contains(square))
    }

    /// Applies a move, promoting if the rules say so.
    pub fn play(self, rules: &Ruleset, mv: Move) -> Position {
        let mover = self.to_move;
        let was_king = self.is_king(mv.from);
        let mut next = self;

        next.by_side[mover.index()] = next.by_side[mover.index()].without(mv.from).with(mv.to);
        next.kings = next.kings.without(mv.from);
        if was_king {
            next.kings = next.kings.with(mv.to);
        }

        // **Captured pieces come off now, and not before.** Through the whole
        // sequence they stayed on the board as obstacles; this is the end of
        // the turn, which is when they leave.
        next.by_side[mover.other().index()] =
            next.by_side[mover.other().index()].difference(mv.captured);
        next.kings = next.kings.difference(mv.captured);

        // Promotion, and there are two ways to it.
        //
        // Under International a man that merely *passed* through the far rank
        // mid-sequence is still a man; only stopping there promotes, which is
        // why the usual question is asked of the destination and not of the
        // path. Under Russian, touching the far rank crowns on the spot and the
        // new king carries on jumping — so it can and routinely does finish the
        // turn somewhere else entirely, and asking only about the destination
        // left it a man.
        //
        // That was the bug, and it was total for the one ruleset it applies to:
        // the generator would let the piece fly for the leg it made *from* the
        // crowning square and then take the crown away again when the move was
        // played. `mv.crowned` is the generator's own answer to "did this
        // sequence pass through", which is the one thing the destination cannot
        // say.
        if !was_king
            && rules.promotion != Promotion::Never
            && (mv.crowned || next.geometry.is_promotion_rank(mv.to, mover.is_upward()))
        {
            next.kings = next.kings.with(mv.to);
        }

        next.to_move = mover.other();
        next
    }

    /// Every legal move.
    ///
    /// **Capture is compulsory in every variant here**, so if any capture
    /// exists the quiet moves are not offered at all. What varies is whether
    /// you must take the *most*.
    pub fn legal_moves(self, rules: &Ruleset) -> Vec<Move> {
        let captures = self.captures(rules);
        if !captures.is_empty() {
            return rules.filter_captures(captures);
        }
        self.quiet_moves(rules)
    }

    fn quiet_moves(self, rules: &Ruleset) -> Vec<Move> {
        let mut moves = Vec::new();
        let occupied = self.occupied();
        let upward = self.to_move.is_upward();

        for from in self.side(self.to_move) {
            let king = self.is_king(from);
            for direction in Direction::ALL {
                // A man walks forward only. A king walks any way.
                if !king && !direction.is_forward(upward) {
                    continue;
                }
                let reach = if king && rules.flying_kings {
                    u8::MAX
                } else {
                    1
                };
                let mut steps = 1;
                while steps <= reach {
                    let Some(to) = self.geometry.step(from, direction, steps) else {
                        break;
                    };
                    if occupied.contains(to) {
                        break;
                    }
                    moves.push(Move {
                        from,
                        to,
                        // A quiet move onto the far rank promotes by arriving,
                        // which `play` asks of the destination. There is no
                        // middle of a one-step sequence.
                        crowned: false,
                        captured: Bitboard::EMPTY,
                    });
                    steps = steps.saturating_add(1);
                }
            }
        }
        moves
    }

    /// Every capture sequence, deduplicated.
    ///
    /// Depth-first, carrying the set of pieces taken so far — which are
    /// **ghosts**: still on the board as obstacles, but not capturable again.
    /// That double duty is the rule implementations most often get wrong, and
    /// it is why the occupancy passed down is the original one and the
    /// already-taken set is separate.
    pub fn captures(self, rules: &Ruleset) -> Vec<Move> {
        // Deduplicated on (from, to, captured, crowned), because sequences
        // agreeing on those reach the same position — offering both would put
        // the same move in the list twice.
        //
        // `crowned` is in the key because it is part of the *position* reached,
        // not part of the route: under Russian, one path through the far rank
        // and one around it can take the same men and stop on the same square
        // and leave a king in one case and a man in the other. Outside Russian
        // the field is always false and the key is the published three.
        let found = self.captures_as_generated(rules);
        let mut unique: Vec<Move> = Vec::with_capacity(found.len());
        for mv in found {
            if !unique.iter().any(|seen| {
                seen.from == mv.from
                    && seen.to == mv.to
                    && seen.captured == mv.captured
                    && seen.crowned == mv.crowned
            }) {
                unique.push(mv);
            }
        }
        unique
    }

    /// Every capture sequence, **including ones that reach the same position by
    /// different paths**.
    ///
    /// Only for perft. It exists because the two families of published table
    /// use different conventions — International's remove duplicates, English's
    /// keep them — and an engine that cannot produce both cannot be checked
    /// against both. For play, use [`Position::captures`].
    pub fn captures_as_generated(self, rules: &Ruleset) -> Vec<Move> {
        let mut found = Vec::new();
        for from in self.side(self.to_move) {
            // Not crowned at the start: a man standing on its own promotion
            // rank is a contradiction, because arriving there is what made it a
            // king.
            self.extend(rules, from, from, Bitboard::EMPTY, false, &mut found);
        }
        found
    }

    /// The move list a perft counts, in this variant's published convention.
    pub fn moves_as_counted(self, rules: &Ruleset) -> Vec<Move> {
        if !rules.counts_duplicate_captures {
            return self.legal_moves(rules);
        }
        let captures = self.captures_as_generated(rules);
        if !captures.is_empty() {
            return rules.filter_captures(captures);
        }
        self.quiet_moves(rules)
    }

    /// One step of the search: from `at`, having already taken `taken`, and
    /// `crowned` if the man was made a king earlier in this same sequence.
    fn extend(
        self,
        rules: &Ruleset,
        origin: Square,
        at: Square,
        taken: Bitboard,
        crowned: bool,
        found: &mut Vec<Move>,
    ) {
        let mover = self.to_move;
        let enemies = self.side(mover.other());
        // The mover is no longer on its origin square, so that square is free
        // to land on or pass over — a king circling back over its own start is
        // legal and is one of the ways duplicate sequences arise.
        let blockers = self.occupied().without(origin);
        let upward = mover.is_upward();
        // **Crowning is remembered, not re-asked.** A man that reaches the far
        // rank mid-sequence promotes on the spot in the rulesets that say so —
        // and it stays promoted for the rest of the turn. Asking only whether
        // it is standing on the far rank *now* let it fly for exactly one leg,
        // the one it made from the crowning square, and then demoted it: the
        // second leg was generated as a man again. It is the same mistake `play`
        // was making at the other end of the move.
        let crowned = crowned || self.crowns_here(rules, origin, at, upward);
        let king = self.is_king(origin) || crowned;

        for direction in Direction::ALL {
            // A man captures backwards in most variants, and not in English or
            // Italian. Its *walking* direction is a separate rule.
            if !king && !rules.men_capture_backwards && !direction.is_forward(upward) {
                continue;
            }

            // How far ahead of the victim a flying king may start.
            let reach = if king && rules.flying_kings {
                self.geometry.side
            } else {
                1
            };

            let mut gap = 1;
            let victim = loop {
                if gap > reach {
                    break None;
                }
                let Some(square) = self.geometry.step(at, direction, gap) else {
                    break None;
                };
                if !blockers.contains(square) {
                    gap += 1;
                    continue;
                }
                // The first piece met. It is a victim only if it is an enemy
                // that has not already been taken — a ghost blocks the way but
                // cannot be jumped twice.
                break if enemies.contains(square) && !taken.contains(square) {
                    Some(square)
                } else {
                    None
                };
            };
            let Some(victim) = victim else { continue };

            // Where the piece can land: any empty square beyond the victim for
            // a flying king, exactly one for anything else.
            let mut beyond = 1;
            while let Some(landing) = self.geometry.step(victim, direction, beyond) {
                if blockers.contains(landing) {
                    break;
                }
                let next_taken = taken.with(victim);
                // The landing square may itself be the far rank, so the flag
                // has to be brought up to date before it is either recursed on
                // or recorded — a sequence that ends by *arriving* crowned and
                // one that ends having crowned earlier are both crowned.
                let landed = crowned || self.crowns_here(rules, origin, landing, upward);
                self.extend(rules, origin, landing, next_taken, landed, found);
                // Recording happens at the leaves, so a sequence that could
                // continue is not offered as a stopping point. Capture chains
                // must be played to the end in every variant here.
                if !self.can_extend(rules, origin, landing, next_taken, landed) {
                    found.push(Move {
                        from: origin,
                        to: landing,
                        captured: next_taken,
                        crowned: landed,
                    });
                }
                if !(king && rules.flying_kings) {
                    break;
                }
                beyond += 1;
            }
        }
    }

    /// Whether a sequence could go on from here. Used to refuse stopping early.
    ///
    /// **`crowned` matters here as much as in `extend`.** A newly crowned king
    /// with a flying capture available may not stop, and a probe that had
    /// forgotten the crown would say the sequence was over — which offers a
    /// move the rules forbid rather than merely generating one badly.
    fn can_extend(
        self,
        rules: &Ruleset,
        origin: Square,
        at: Square,
        taken: Bitboard,
        crowned: bool,
    ) -> bool {
        let mut probe = Vec::new();
        self.extend_probe(rules, origin, at, taken, crowned, &mut probe);
        !probe.is_empty()
    }

    /// Whether standing on `at` crowns the piece that started on `origin`.
    ///
    /// One place, because the generator asks it twice and `play` asks the
    /// destination half of it — and the bug this replaces was the two halves
    /// having drifted into disagreeing.
    fn crowns_here(self, rules: &Ruleset, origin: Square, at: Square, upward: bool) -> bool {
        rules.promotion == Promotion::MidSequence
            && !self.is_king(origin)
            && self.geometry.is_promotion_rank(at, upward)
    }

    fn extend_probe(
        self,
        rules: &Ruleset,
        origin: Square,
        at: Square,
        taken: Bitboard,
        crowned: bool,
        found: &mut Vec<()>,
    ) {
        let mover = self.to_move;
        let enemies = self.side(mover.other());
        let blockers = self.occupied().without(origin);
        let upward = mover.is_upward();
        let king = self.is_king(origin) || crowned || self.crowns_here(rules, origin, at, upward);

        for direction in Direction::ALL {
            if !king && !rules.men_capture_backwards && !direction.is_forward(upward) {
                continue;
            }
            let reach = if king && rules.flying_kings {
                self.geometry.side
            } else {
                1
            };
            let mut gap = 1;
            let victim = loop {
                if gap > reach {
                    break None;
                }
                let Some(square) = self.geometry.step(at, direction, gap) else {
                    break None;
                };
                if !blockers.contains(square) {
                    gap += 1;
                    continue;
                }
                break if enemies.contains(square) && !taken.contains(square) {
                    Some(square)
                } else {
                    None
                };
            };
            let Some(victim) = victim else { continue };
            if let Some(landing) = self.geometry.step(victim, direction, 1)
                && !blockers.contains(landing)
            {
                found.push(());
                return;
            }
        }
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..self.geometry.side).rev() {
            for file in 0..self.geometry.side {
                match self.geometry.at(file as i16, rank as i16) {
                    None => f.write_str(" ")?,
                    Some(square) => {
                        let king = self.is_king(square);
                        f.write_str(match self.side_at(square) {
                            Some(Side::White) if king => "W",
                            Some(Side::White) => "w",
                            Some(Side::Black) if king => "B",
                            Some(Side::Black) => "b",
                            None => ".",
                        })?
                    }
                }
            }
            writeln!(f)?;
        }
        write!(f, "{:?} to move", self.to_move)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruleset::{BRAZILIAN, RUSSIAN};

    /// A position built by hand, because the fields are private and the only
    /// other way in is to play a real game to the position you want.
    ///
    /// That is not a convenience: `Promotion::MidSequence` is the one ruleset
    /// flag nothing tested ever set, and the reason is exactly this — there is
    /// no perft table for Russian and no way to state a Russian position from
    /// outside the crate, so the rule that defines the game had no test at all
    /// and was wrong at both ends of the move.
    fn position(white: &[u8], black: &[u8], kings: &[u8], to_move: Side) -> Position {
        let bits = |squares: &[u8]| {
            squares.iter().fold(Bitboard::EMPTY, |board, &index| {
                board.with(Square::new(index))
            })
        };
        Position {
            geometry: Geometry::SMALL,
            by_side: [bits(white), bits(black)],
            kings: bits(kings),
            to_move,
        }
    }

    // The 8×8 numbering, so the squares below can be read: four dark squares to
    // a rank, index 0 at file 0 of rank 0. White moves upward and crowns on
    // rank 7, which is indices 28 to 31.
    const WHITE_MAN: u8 = 20; // file 1, rank 5
    const FIRST_VICTIM: u8 = 25; // file 2, rank 6
    const THE_FAR_RANK: u8 = 29; // file 3, rank 7 — where the crown happens
    const SECOND_VICTIM: u8 = 26; // file 4, rank 6
    const BACK_OFF_THE_RANK: u8 = 22; // file 5, rank 5
    const THIRD_VICTIM: u8 = 13; // file 3, rank 3 — two steps away, so only a
    // flying king can reach it

    /// One White man, three Black men, and one empty square between the second
    /// landing and the third victim.
    ///
    /// **That gap is the whole test.** A man at `BACK_OFF_THE_RANK` sees an
    /// empty square in that direction and stops. A king sees past it and must
    /// take. So the third capture happens if and only if the piece is still
    /// remembered to be a king two legs after it was crowned — which is the
    /// half of the bug that a position crowning on the last leg cannot show.
    fn crowning_position() -> Position {
        position(
            &[WHITE_MAN],
            &[FIRST_VICTIM, SECOND_VICTIM, THIRD_VICTIM],
            &[],
            Side::White,
        )
    }

    #[test]
    fn a_russian_man_crowned_mid_sequence_is_a_king_at_the_end_of_it() {
        let start = crowning_position();
        let moves = start.legal_moves(&RUSSIAN);

        assert!(!moves.is_empty(), "no moves generated at all");
        assert!(
            !moves.iter().any(|mv| mv.to.index() == THE_FAR_RANK),
            "stopping on the far rank was offered, but the new king can still take: {moves:?}"
        );

        for mv in &moves {
            assert!(
                mv.crowned,
                "a sequence through the far rank was not marked crowned: {mv:?}"
            );
            let after = start.play(&RUSSIAN, *mv);
            assert!(
                after.is_king(mv.to),
                "the man passed through the far rank and finished the turn as a man on {}",
                mv.to.index()
            );
        }
    }

    #[test]
    fn and_it_keeps_flying_for_the_rest_of_the_turn() {
        // The subtler half. The crown used to be re-derived from "am I standing
        // on the far rank *now*", so the piece flew for exactly the one leg it
        // made from the crowning square and was generated as a man again on the
        // next — which is a legal capture the rules require, silently not
        // offered.
        let moves = crowning_position().legal_moves(&RUSSIAN);

        // Not *every* move takes three. A flying king may land anywhere beyond
        // its victim, and the far landings put it where nothing more is in
        // reach — those sequences are complete at two, and Russian's "take any
        // capture" makes them legal. What must exist is the one that takes the
        // third, because only a piece still flying two legs after the crown can
        // see past the empty square in front of it.
        assert!(
            moves.iter().any(|mv| mv.captured.into_iter().count() == 3),
            "no sequence took the third man, so the crown was forgotten a leg later: {moves:?}"
        );

        // And stopping on that square is not a choice: from there the capture
        // is available, and a capture available is a capture compulsory.
        assert!(
            !moves.iter().any(|mv| mv.to.index() == BACK_OFF_THE_RANK),
            "the sequence was allowed to stop where only a man could have: {moves:?}"
        );
    }

    #[test]
    fn brazilian_is_the_same_game_without_that_one_rule() {
        // Brazilian is Russian's board, pieces, backward captures and flying
        // kings with `Promotion::OnStopping`. It is the control that makes the
        // two tests above about the rule rather than about the position: the
        // same three men, and the man stops two captures in, off the far rank,
        // still a man.
        let start = crowning_position();
        let moves = start.legal_moves(&BRAZILIAN);
        assert!(!moves.is_empty(), "no moves generated at all");

        for mv in moves {
            assert!(!mv.crowned, "Brazilian crowned mid-sequence: {mv:?}");
            assert_eq!(
                mv.captured.into_iter().count(),
                2,
                "Brazilian took a third man, which only a king could reach: {mv:?}"
            );
            assert_eq!(mv.to.index(), BACK_OFF_THE_RANK);
            assert!(
                !start.play(&BRAZILIAN, mv).is_king(mv.to),
                "Brazilian crowned a man that did not stop on the far rank: {mv:?}"
            );
        }
    }

    #[test]
    fn nothing_outside_russian_ever_sets_the_flag() {
        // The field is only safe to add to the deduplication key because of
        // this. If a fifth ruleset ever sets `MidSequence`, this fails and
        // whoever added it has to think about the published perft tables first.
        for rules in Ruleset::ALL {
            assert_eq!(
                rules.promotion == Promotion::MidSequence,
                rules.key == "draughts-russian",
                "{} sets MidSequence",
                rules.key
            );
        }
    }
}
