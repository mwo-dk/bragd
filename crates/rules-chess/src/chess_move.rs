//! Moves.
//!
//! A move is packed into a `u32`: origin, destination, a kind that says
//! everything else, and — for the one variant that needs it — what was gated
//! onto the square the mover left. The kind is *not* redundant with the
//! position: it records what the move meant when it was generated, so a move
//! read back from a Library years later replays identically without having to
//! re-derive intent from a board.
//!
//! It was sixteen bits for a long time and the doc comments below are written
//! from that history, because the two widenings since were each forced by a
//! variant and each is worth being able to find. Twenty-one bits are used.

use crate::castling::Side;
use crate::piece::{Color, Role};
use crate::square::Square;
use core::fmt;

/// What kind of move this is. The bit layout is load-bearing: bit 2 marks a
/// capture, bit 3 marks a promotion, and the low two bits of a promotion select
/// the piece. Do not reorder.
///
/// **The layout only holds for the sixteen chess left behind.** Every variant
/// promotion since — shatranj's ferz at 6 and 7, Seirawan's two compounds at 16
/// through 19 — sits outside it, which is why [`MoveKind::is_capture`] and
/// [`MoveKind::is_promotion`] enumerate rather than test a bit. Read that as the
/// scheme having run out rather than as an invitation to squeeze one more in:
/// the next promoting piece appends four codes and adds four arms, and nothing
/// else in the crate has to change.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum MoveKind {
    Quiet = 0,
    DoublePawnPush = 1,
    KingsideCastle = 2,
    QueensideCastle = 3,
    Capture = 4,
    /// A pawn capturing onto an empty square. The captured pawn is not on the
    /// destination, which is the entire reason this needs its own kind.
    EnPassant = 5,
    /// Shatranj's only promotion. A pawn reaching the last rank becomes a ferz
    /// and has no say in it, so unlike chess this is not a choice the Player
    /// makes — which is why it needs no dialog and why there is exactly one
    /// kind rather than four.
    ///
    /// Codes 6 and 7 were the two the chess encoding left unused. Taking them
    /// is what kept a [`Move`] at sixteen bits; the cost is that `is_capture`
    /// and `is_promotion` can no longer be single bit tests, since neither 6
    /// nor 7 can carry the promotion bit.
    FerzPromotion = 6,
    FerzPromotionCapture = 7,
    KnightPromotion = 8,
    BishopPromotion = 9,
    RookPromotion = 10,
    QueenPromotion = 11,
    KnightPromotionCapture = 12,
    BishopPromotionCapture = 13,
    RookPromotionCapture = 14,
    QueenPromotionCapture = 15,
    /// Seirawan chess's two compounds, which a pawn may also promote to.
    ///
    /// **This is what made the kind field five bits wide.** Sixteen codes were
    /// all there were, and every one of them was taken — chess's twelve plus
    /// en passant, the two castles and shatranj's two ferz promotions. A hawk
    /// and an elephant, capturing and not, need four more, and there is no
    /// spare bit inside a nibble to find them in.
    ///
    /// Widening the field rather than adding an override flag beside it: the
    /// module's whole premise is that the kind says everything a move means
    /// besides its two squares, and a promotion role stored somewhere else
    /// would make `mv.kind().promotion_role()` and `mv.promotion_role()` two
    /// different answers to one question. The bits were free — a [`Move`] is a
    /// `u32` and was using twenty of them — so the choice cost nothing but
    /// moving the gate field one place up.
    HawkPromotion = 16,
    HawkPromotionCapture = 17,
    ElephantPromotion = 18,
    ElephantPromotionCapture = 19,
}

const CAPTURE_BIT: u8 = 0b0100;
const PROMOTION_BIT: u8 = 0b1000;

impl MoveKind {
    const fn from_bits(bits: u8) -> MoveKind {
        match bits {
            0 => MoveKind::Quiet,
            1 => MoveKind::DoublePawnPush,
            2 => MoveKind::KingsideCastle,
            3 => MoveKind::QueensideCastle,
            4 => MoveKind::Capture,
            5 => MoveKind::EnPassant,
            6 => MoveKind::FerzPromotion,
            7 => MoveKind::FerzPromotionCapture,
            8 => MoveKind::KnightPromotion,
            9 => MoveKind::BishopPromotion,
            10 => MoveKind::RookPromotion,
            11 => MoveKind::QueenPromotion,
            12 => MoveKind::KnightPromotionCapture,
            13 => MoveKind::BishopPromotionCapture,
            14 => MoveKind::RookPromotionCapture,
            15 => MoveKind::QueenPromotionCapture,
            16 => MoveKind::HawkPromotion,
            17 => MoveKind::HawkPromotionCapture,
            18 => MoveKind::ElephantPromotion,
            19 => MoveKind::ElephantPromotionCapture,
            _ => panic!("not a move kind"),
        }
    }

    /// Whether a piece leaves the board. True for en passant, where the
    /// destination square is empty.
    ///
    /// A match rather than `self as u8 & CAPTURE_BIT`, because
    /// [`MoveKind::FerzPromotion`] sits at code 6 and would otherwise read as a
    /// capture. Enumerating the kinds also means a new one cannot silently
    /// inherit an answer from where its number happened to fall.
    pub const fn is_capture(self) -> bool {
        matches!(
            self,
            MoveKind::Capture
                | MoveKind::EnPassant
                | MoveKind::FerzPromotionCapture
                | MoveKind::KnightPromotionCapture
                | MoveKind::BishopPromotionCapture
                | MoveKind::RookPromotionCapture
                | MoveKind::QueenPromotionCapture
                | MoveKind::HawkPromotionCapture
                | MoveKind::ElephantPromotionCapture
        )
    }

    pub const fn is_promotion(self) -> bool {
        matches!(
            self,
            MoveKind::FerzPromotion
                | MoveKind::FerzPromotionCapture
                | MoveKind::KnightPromotion
                | MoveKind::BishopPromotion
                | MoveKind::RookPromotion
                | MoveKind::QueenPromotion
                | MoveKind::KnightPromotionCapture
                | MoveKind::BishopPromotionCapture
                | MoveKind::RookPromotionCapture
                | MoveKind::QueenPromotionCapture
                | MoveKind::HawkPromotion
                | MoveKind::HawkPromotionCapture
                | MoveKind::ElephantPromotion
                | MoveKind::ElephantPromotionCapture
        )
    }

    pub const fn is_castle(self) -> bool {
        matches!(self, MoveKind::KingsideCastle | MoveKind::QueensideCastle)
    }

    /// What the pawn becomes, if anything.
    pub const fn promotion_role(self) -> Option<Role> {
        match self {
            MoveKind::FerzPromotion | MoveKind::FerzPromotionCapture => Some(Role::Ferz),
            MoveKind::HawkPromotion | MoveKind::HawkPromotionCapture => Some(Role::Hawk),
            MoveKind::ElephantPromotion | MoveKind::ElephantPromotionCapture => {
                Some(Role::Elephant)
            }
            _ if !self.is_promotion() => None,
            _ => Some(match self as u8 & 0b11 {
                0 => Role::Knight,
                1 => Role::Bishop,
                2 => Role::Rook,
                _ => Role::Queen,
            }),
        }
    }

    /// The promotion kind for a role, capturing or not.
    ///
    /// The four chess pieces are the bit layout; every variant's piece is a
    /// pair of codes outside it, listed here rather than computed. A table of
    /// four pairs is not worth an arithmetic scheme that the next piece would
    /// break again.
    ///
    /// # Panics
    /// A pawn cannot promote to a pawn, a king, an alfil or a gaja. The alfil
    /// and the gaja are excluded because neither game offers them: a promoting
    /// pawn becomes a ferz in shatranj and a mantri in chaturanga, and is given
    /// no choice about it.
    pub const fn promotion_to(role: Role, capture: bool) -> MoveKind {
        let outside_the_layout = match role {
            Role::Ferz => Some((MoveKind::FerzPromotion, MoveKind::FerzPromotionCapture)),
            Role::Hawk => Some((MoveKind::HawkPromotion, MoveKind::HawkPromotionCapture)),
            Role::Elephant => Some((
                MoveKind::ElephantPromotion,
                MoveKind::ElephantPromotionCapture,
            )),
            _ => None,
        };
        if let Some((quiet, capturing)) = outside_the_layout {
            return if capture { capturing } else { quiet };
        }
        let offset = match role {
            Role::Knight => 0,
            Role::Bishop => 1,
            Role::Rook => 2,
            Role::Queen => 3,
            _ => panic!("a pawn cannot promote to that"),
        };
        let capture_bit = if capture { CAPTURE_BIT } else { 0 };
        MoveKind::from_bits(PROMOTION_BIT | capture_bit | offset)
    }
}

/// A move: origin, destination and kind, in 16 bits.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move(u32);

impl Move {
    /// The kind, in **five** bits at 12.
    ///
    /// Four was the whole encoding once, and shatranj's ferz took the last two
    /// codes chess left. Seirawan's promotions needed four more and there was
    /// no nibble left to hold them, so the field grew by a bit and the gate
    /// moved up one. Nothing outside this file reads a raw move, and no move
    /// integer is stored anywhere — a Library keeps UCI strings — so the
    /// renumbering is invisible past this module's edge.
    const KIND_SHIFT: u32 = 12;
    const KIND_MASK: u32 = 0b1_1111 << Move::KIND_SHIFT;
    /// Where the gate lives: three bits above the kind.
    ///
    /// **A move was sixteen bits and every one of them was spoken for** — six
    /// for each square and four for the kind, with all sixteen kinds used.
    /// Seirawan chess needs one more fact on the move: which piece, if any, was
    /// gated onto the square the mover just left. There was nowhere to put it.
    ///
    /// Widening to `u32` rather than inventing a parallel move type: every
    /// existing accessor keeps the same shift, the perft suite is unchanged by
    /// construction, and a `MoveList` of 256 grows by a kilobyte, which is
    /// nothing. A parallel type would have meant two move representations in
    /// one engine and a conversion between them at every boundary. That
    /// headroom is what let the kind field grow later without an argument.
    const GATE_SHIFT: u32 = 17;
    const GATE_MASK: u32 = 0b111 << Move::GATE_SHIFT;
    /// Set when the gated piece goes on the **rook's** original square rather
    /// than the king's. Castling vacates two squares at once and either may be
    /// gated, which is the one case where "the square the mover left" is
    /// ambiguous.
    const GATE_AT_ROOK: u32 = 1 << 20;

    pub const fn new(from: Square, to: Square, kind: MoveKind) -> Move {
        Move(from.index() as u32 | ((to.index() as u32) << 6) | ((kind as u32) << Move::KIND_SHIFT))
    }

    pub const fn from(self) -> Square {
        Square::new((self.0 & 0x3f) as u8)
    }

    pub const fn to(self) -> Square {
        Square::new(((self.0 >> 6) & 0x3f) as u8)
    }

    pub const fn kind(self) -> MoveKind {
        MoveKind::from_bits(((self.0 & Move::KIND_MASK) >> Move::KIND_SHIFT) as u8)
    }

    /// The same move, with a piece gated onto the square it leaves.
    ///
    /// Seirawan chess only. `role` is the hawk or the elephant; anything else
    /// is a caller bug and is rejected rather than encoded, because a gate of
    /// "queen" would round-trip through the wire as a legal-looking move.
    ///
    /// A promotion is rejected for a sharper reason than tidiness: [`to_uci`]
    /// spells a promotion and a gate with the *same* fifth character, and the
    /// only thing that keeps them apart is that no move is ever both. That is
    /// true of the board — a gate needs the vacated square to carry a gating
    /// right, and only a home rank ever does, while a promoting pawn starts on
    /// the rank before the last and never on its own home rank — and this is
    /// where it stops being merely true and starts being enforced.
    ///
    /// [`to_uci`]: Move::to_uci
    pub const fn with_gate(self, role: Role) -> Move {
        if self.kind().is_promotion() {
            panic!("a promotion cannot also gate: both spell themselves as the fifth character");
        }
        let code = match role {
            Role::Hawk => 1,
            Role::Elephant => 2,
            _ => panic!("only the hawk and the elephant can be gated"),
        };
        Move((self.0 & !Move::GATE_MASK) | (code << Move::GATE_SHIFT))
    }

    /// The same castling move, gating onto the rook's original square.
    pub const fn with_gate_at_rook(self, role: Role) -> Move {
        Move(self.with_gate(role).0 | Move::GATE_AT_ROOK)
    }

    /// Whether the gate goes on the rook's original square. Only ever true on
    /// a castling move.
    pub const fn gates_at_rook(self) -> bool {
        self.0 & Move::GATE_AT_ROOK != 0
    }

    /// The chess move underneath, with the gate stripped off.
    ///
    /// For everything but Seirawan this is the move itself. It exists because
    /// the parts of the engine that were written before the gate had somewhere
    /// to live still reason about moves the *chess* generator produces — and
    /// `Position::is_legal` is one of them, so a gating move fails a test it
    /// should never have been given. See the note in [`crate::san::to_san`].
    pub const fn without_gate(self) -> Move {
        Move(self.0 & !(Move::GATE_MASK | Move::GATE_AT_ROOK))
    }

    /// The piece gated onto the vacated square, if any.
    pub const fn gate(self) -> Option<Role> {
        match (self.0 & Move::GATE_MASK) >> Move::GATE_SHIFT {
            1 => Some(Role::Hawk),
            2 => Some(Role::Elephant),
            _ => None,
        }
    }

    pub const fn is_capture(self) -> bool {
        self.kind().is_capture()
    }

    pub const fn promotion_role(self) -> Option<Role> {
        self.kind().promotion_role()
    }

    /// Long algebraic, as used on the wire and by UCI: `e2e4`, `e7e8q`.
    ///
    /// This is deliberately the *lossy* form — it does not distinguish a
    /// capture from a quiet move, because the receiving side has the position
    /// and must validate the move against it anyway. A client that could
    /// declare its own move kind could declare a capture that is not there.
    ///
    /// Castling is the one move whose spelling depends on the variant; see
    /// [`CastlingNotation`].
    pub fn to_uci(self, notation: CastlingNotation) -> String {
        let to = match (notation, self.kind()) {
            (CastlingNotation::KingDestination, MoveKind::KingsideCastle) => {
                Side::Kingside.king_destination(self.mover_color())
            }
            (CastlingNotation::KingDestination, MoveKind::QueensideCastle) => {
                Side::Queenside.king_destination(self.mover_color())
            }
            _ => self.to(),
        };
        let mut text = format!("{}{}", self.from(), to);
        if let Some(role) = self.promotion_role() {
            text.push(role.letter().unwrap().to_ascii_lowercase());
        }
        // **The gate has to be on the wire**, or three different Seirawan moves
        // spell the same thing: the plain move, the one gating a hawk and the
        // one gating an elephant all share a from and a to. A client would send
        // an ambiguous string and the authority would guess.
        //
        // The letter is the piece; its **case** says which vacated square it
        // lands on. Lowercase is the square the mover left, which is every move
        // but a castle; uppercase is the rook's original square, which only a
        // castle can offer.
        //
        // **The fifth character is shared with a promotion and that is safe**,
        // which stopped being obvious the moment a pawn could promote to a hawk:
        // `a7a8h` is a promotion and `b1c3h` is a gate, and both are `h`. No
        // move is ever both, because a gate needs the vacated square to carry a
        // gating right and only a home rank ever does, while a promoting pawn
        // starts on the rank before the last. `with_gate` refuses a promotion
        // outright so that this stays a fact about the encoding rather than a
        // fact about the generator that happens to hold today.
        if let Some(role) = self.gate() {
            let letter = role.letter().expect("the gated pieces both have letters");
            text.push(if self.gates_at_rook() {
                letter.to_ascii_uppercase()
            } else {
                letter.to_ascii_lowercase()
            });
        }
        text
    }

    /// Which colour is castling, read off the rank the move starts on.
    ///
    /// Only meaningful for a castle, where `from` is the king's square and a
    /// king holding a castling right has not left its back rank.
    fn mover_color(self) -> Color {
        if self.from().rank() == 0 {
            Color::White
        } else {
            Color::Black
        }
    }
}

/// How a castling move is spelled on the wire.
///
/// A property of the **variant**, not of a position, because stored Games are
/// replayed through it: a Library keeps raw UCI strings for ever and replays
/// them on every read of every stored Game. Changing the spelling for a
/// variant would stop its stored Games replaying.
///
/// Internally a castle is *always* the king taking its own rook. This only
/// decides what it is called on the way out and on the way in.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CastlingNotation {
    /// `e1g1` — the king's destination. **Standard chess, frozen.**
    ///
    /// The default, so `Chess` is frozen by omission rather than by
    /// discipline: a variant has to opt *out* of the spelling already stored.
    /// Unambiguous only because a standard king can never reach g1 or c1 in one
    /// ordinary move.
    #[default]
    KingDestination,
    /// `e1h1`, `g1h1`, `b1a1` — the king's square then its own rook's.
    ///
    /// The `UCI_Chess960` convention, and the only spelling that works for
    /// Chess960 at all. King-to-destination breaks twice there: a castle where
    /// the king does not move spells `g1g1`, which is not a move; and a castle
    /// from b1 landing on c1 spells `b1c1`, which a quiet king step also
    /// spells — and `generate_castles` runs last, so the lookup would return
    /// the king step and a Player who asked to castle would authoritatively
    /// get one.
    ///
    /// Collision-free by construction: the destination names a square holding
    /// one's own rook, and every other generator masks its targets with
    /// `& !ours`, so no ordinary move can produce the same pair.
    KingTakesRook,
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The unambiguous spelling. This is for debugging output, where two
        // moves printing the same string is worse than an unfamiliar one.
        write!(f, "{}", self.to_uci(CastlingNotation::KingTakesRook))
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}({:?})",
            self.to_uci(CastlingNotation::KingTakesRook),
            self.kind()
        )
    }
}

/// A stack-allocated move list.
///
/// Move generation is on the hot path of both perft and every move a Player
/// makes, so this avoids an allocation per node.
///
/// **512, and it was 256 on an argument that only held for chess.** 256 sits
/// above chess's record of 218 with room to spare, and every variant here that
/// only rearranges chess's pieces is bounded by the same number. Seirawan is
/// not: it emits chess's list *plus* two moves for every move that leaves a
/// square still carrying a gating right, *plus* two for every promoting pawn
/// step. Those additions are not bounded by 218, and a contrived-but-legal
/// board crosses 256 — see the note on `seirawan::legal_moves`, which carries
/// the measured counts.
///
/// The generous bound is what makes the assert in `push` an assert rather than
/// a live hazard. A side has at most eight gated squares, and they hold the
/// pieces that started on them, whose mobility on an empty board sums to 76; at
/// two pieces in hand that is 152. Eight pawns on the seventh reach at most
/// three squares each, so promotion adds at most 48. Even summing maxima that
/// cannot occur together — 218 + 152 + 48 — the total is 418.
///
/// The cost is 1KB more stack per node, on a type that is already an array
/// rather than a `Vec` precisely because that trade was made deliberately.
#[derive(Clone)]
pub struct MoveList {
    moves: [Move; Self::CAPACITY],
    len: usize,
}

impl MoveList {
    const CAPACITY: usize = 512;

    pub fn new() -> MoveList {
        MoveList {
            moves: [Move(0); Self::CAPACITY],
            len: 0,
        }
    }

    /// # Panics
    /// If more than [`MoveList::CAPACITY`] moves are pushed, which would mean
    /// move generation is producing moves that no position can have. An
    /// `assert!` and not a `debug_assert!`, so this aborts a release build
    /// too — deliberately, because the alternative is silently truncating a
    /// legal move list, but it does mean the bound above has to be real.
    pub fn push(&mut self, mv: Move) {
        assert!(self.len < Self::CAPACITY, "move list overflow");
        self.moves[self.len] = mv;
        self.len += 1;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }

    pub fn contains(&self, mv: Move) -> bool {
        self.as_slice().contains(&mv)
    }

    pub fn retain(&mut self, mut keep: impl FnMut(Move) -> bool) {
        let mut kept = 0;
        for index in 0..self.len {
            if keep(self.moves[index]) {
                self.moves[kept] = self.moves[index];
                kept += 1;
            }
        }
        self.len = kept;
    }
}

impl Default for MoveList {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = Move;
    type IntoIter = core::iter::Copied<core::slice::Iter<'a, Move>>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter().copied()
    }
}

impl IntoIterator for MoveList {
    type Item = Move;
    type IntoIter = IntoIter;

    fn into_iter(self) -> IntoIter {
        IntoIter {
            list: self,
            next: 0,
        }
    }
}

/// Iterates an owned move list. Exists so that `for mv in position.legal_moves()`
/// reads the way it should — the list is a value, not a borrow of anything.
pub struct IntoIter {
    list: MoveList,
    next: usize,
}

impl Iterator for IntoIter {
    type Item = Move;

    fn next(&mut self) -> Option<Move> {
        let mv = *self.list.as_slice().get(self.next)?;
        self.next += 1;
        Some(mv)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.list.len() - self.next;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for IntoIter {}

impl fmt::Debug for MoveList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.as_slice()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::square::squares::*;

    #[test]
    fn packing_round_trips_for_every_field() {
        for kind in [
            MoveKind::Quiet,
            MoveKind::DoublePawnPush,
            MoveKind::KingsideCastle,
            MoveKind::QueensideCastle,
            MoveKind::Capture,
            MoveKind::EnPassant,
            MoveKind::KnightPromotion,
            MoveKind::QueenPromotionCapture,
        ] {
            for from in [A1, E1, H8] {
                for to in [B1, E8, H1] {
                    let mv = Move::new(from, to, kind);
                    assert_eq!(mv.from(), from);
                    assert_eq!(mv.to(), to);
                    assert_eq!(mv.kind(), kind);
                }
            }
        }
    }

    #[test]
    fn capture_and_promotion_bits_agree_with_the_kinds() {
        assert!(
            MoveKind::EnPassant.is_capture(),
            "en passant removes a pawn"
        );
        assert!(!MoveKind::DoublePawnPush.is_capture());
        assert!(!MoveKind::QueensideCastle.is_capture());
        assert!(MoveKind::QueenPromotionCapture.is_capture());
        assert!(MoveKind::QueenPromotionCapture.is_promotion());
        assert!(!MoveKind::Capture.is_promotion());
    }

    /// **Every kind there is, against every other field.** The kind field grew
    /// from four bits to five for Seirawan's promotions, and the failure that
    /// widening invites is silent: a code that overflows into the gate bits
    /// gives a move that reads back as a different move, and the only place it
    /// would show is a position nobody wrote a test for.
    #[test]
    fn every_kind_survives_being_packed_beside_a_gate() {
        for code in 0..=19u8 {
            let kind = MoveKind::from_bits(code);
            for from in [A1, E1, H8] {
                for to in [B1, E8, H1] {
                    let mv = Move::new(from, to, kind);
                    assert_eq!((mv.from(), mv.to(), mv.kind()), (from, to, kind));
                    assert_eq!(mv.gate(), None, "{kind:?} invented a gate");
                    assert!(!mv.gates_at_rook(), "{kind:?}");

                    // And with the fields above it filled in, which is where a
                    // kind that had outgrown its own bits would land.
                    if kind.is_promotion() {
                        continue;
                    }
                    let gated = mv.with_gate(Role::Elephant).with_gate_at_rook(Role::Hawk);
                    assert_eq!(gated.kind(), kind, "the gate ate the kind");
                    assert_eq!(gated.gate(), Some(Role::Hawk));
                    assert!(gated.gates_at_rook());
                    assert_eq!(gated.without_gate(), mv);
                }
            }
        }
    }

    /// **A promotion and a gate would spell themselves the same way**, so the
    /// encoding refuses to hold both rather than leaving the wire form to be
    /// ambiguous. No board can produce one — a gate needs a home-rank square to
    /// have been vacated and a promoting pawn never starts on one — and this is
    /// what keeps that a property of the encoding.
    #[test]
    #[should_panic(expected = "a promotion cannot also gate")]
    fn a_promotion_cannot_carry_a_gate() {
        let a7 = Square::new(48);
        Move::new(a7, A8, MoveKind::promotion_to(Role::Queen, false)).with_gate(Role::Hawk);
    }

    #[test]
    fn promotion_roles_round_trip() {
        for role in [
            Role::Knight,
            Role::Bishop,
            Role::Rook,
            Role::Queen,
            Role::Ferz,
            Role::Hawk,
            Role::Elephant,
        ] {
            for capture in [false, true] {
                let kind = MoveKind::promotion_to(role, capture);
                assert_eq!(kind.promotion_role(), Some(role));
                assert!(kind.is_promotion(), "{kind:?}");
                assert_eq!(kind.is_capture(), capture);
            }
        }
        assert_eq!(MoveKind::Quiet.promotion_role(), None);
    }

    #[test]
    fn uci_matches_the_wire_format() {
        use CastlingNotation::*;
        assert_eq!(
            Move::new(E1, E8, MoveKind::Quiet).to_uci(KingDestination),
            "e1e8"
        );
        assert_eq!(
            Move::new(B8, A8, MoveKind::promotion_to(Role::Knight, true)).to_uci(KingDestination),
            "b8a8n"
        );
    }

    #[test]
    fn a_castle_is_the_king_taking_its_own_rook_and_is_spelled_two_ways() {
        use CastlingNotation::*;
        // Internally, always the rook's square — so `from != to` even when the
        // king does not move, and no ordinary move can share the pair.
        let short = Move::new(E1, H1, MoveKind::KingsideCastle);
        let long = Move::new(E1, A1, MoveKind::QueensideCastle);

        // Standard chess's spelling is frozen: every Library is full of it.
        assert_eq!(short.to_uci(KingDestination), "e1g1");
        assert_eq!(long.to_uci(KingDestination), "e1c1");

        // Chess960 says what actually happened.
        assert_eq!(short.to_uci(KingTakesRook), "e1h1");
        assert_eq!(long.to_uci(KingTakesRook), "e1a1");

        // And the colour comes off the rank, not from a parameter.
        assert_eq!(
            Move::new(E8, H8, MoveKind::KingsideCastle).to_uci(KingDestination),
            "e8g8"
        );
    }

    #[test]
    fn the_king_destination_spelling_is_the_default() {
        // Frozen by omission: a variant has to opt *out* of the spelling
        // already stored in a Library, rather than opt in to it.
        assert_eq!(
            CastlingNotation::default(),
            CastlingNotation::KingDestination
        );
    }

    #[test]
    fn retain_keeps_order_and_length() {
        let mut list = MoveList::new();
        for to in [A1, B1, C1, D1] {
            list.push(Move::new(E1, to, MoveKind::Quiet));
        }
        list.retain(|mv| mv.to() != B1);
        assert_eq!(list.len(), 3);
        assert_eq!(
            list.into_iter().map(|mv| mv.to()).collect::<Vec<_>>(),
            vec![A1, C1, D1]
        );
    }
}
