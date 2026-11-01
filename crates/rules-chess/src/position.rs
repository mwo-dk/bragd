//! A chess position, and the rules for moving from one to the next.
//!
//! `Position` is a value: `make_move` takes `&self` and returns the position
//! that follows, rather than mutating in place with an undo. There is no
//! unmake path to get wrong, replay from stored history is a fold over moves,
//! and the type stays a pure function of its inputs — which is what the whole
//! crate requires, so that a stored Game replays exactly. A position is under a hundred bytes, so
//! copy-make costs less than the bug surface of an undo stack.
//!
//! Nothing here reads a clock or performs I/O.

use crate::attacks;
use crate::bitboard::{Bitboard, RANKS};
use crate::castling::{CastlingRights, Side};
use crate::chess_move::{Move, MoveKind, MoveList};
use crate::movegen;
use crate::piece::{Color, Piece, Role};
use crate::square::Square;
use crate::zobrist;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Position {
    by_color: [Bitboard; 2],
    by_role: [Bitboard; Role::COUNT],
    side_to_move: Color,
    castling: CastlingRights,
    /// The square a pawn just passed over, as FEN records it — set after every
    /// double push, whether or not anyone can actually capture there.
    en_passant: Option<Square>,
    /// Plies since the last capture or pawn move, for the fifty-move rule.
    halfmove_clock: u32,
    fullmove_number: u32,
    /// Seirawan chess's two pieces in hand, per colour.
    ///
    /// Bit 0 is the hawk and bit 1 the elephant. **Zero for every other
    /// variant**, which is what lets this live on the shared position: an empty
    /// hand contributes nothing to the key, writes nothing to a FEN, and
    /// generates no moves, so chess is bit-for-bit unaffected.
    ///
    /// On the position rather than beside it because a hand is part of *where
    /// the game is*: two positions that differ only in what is left to gate are
    /// different positions, and repetition has to see that.
    pockets: [u8; 2],
    /// Squares a Seirawan piece may still be gated onto.
    ///
    /// A back-rank square keeps its right until the piece that started there
    /// moves — gating or not. Empty for every other variant.
    gates: Bitboard,
    key: u64,
}

impl Position {
    /// The standard starting position.
    pub fn starting() -> Position {
        Position::from_fen(crate::fen::STARTING_POSITION)
            .expect("the starting position is a valid FEN")
    }

    /// Assembles a position from its parts and computes its key.
    ///
    /// Used by FEN parsing; the caller is responsible for the position being
    /// legal, which `crate::fen` checks.
    pub(crate) fn from_parts(
        squares: [Option<Piece>; 64],
        side_to_move: Color,
        castling: CastlingRights,
        en_passant: Option<Square>,
        halfmove_clock: u32,
        fullmove_number: u32,
    ) -> Position {
        let mut position = Position {
            by_color: [Bitboard::EMPTY; 2],
            by_role: [Bitboard::EMPTY; Role::COUNT],
            side_to_move,
            castling,
            en_passant,
            halfmove_clock,
            fullmove_number,
            // Empty unless a variant fills them in. Every position this
            // function builds is parsed from a FEN, and only Seirawan's FEN
            // carries either — see `Position::with_seirawan`.
            pockets: [0; 2],
            gates: Bitboard::EMPTY,
            key: 0,
        };
        for (index, square) in squares.iter().enumerate() {
            if let Some(piece) = square {
                position.put_piece(*piece, Square::new(index as u8));
            }
        }
        position.key = position.compute_key();
        position
    }

    // --- Reading the board ---------------------------------------------------

    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    pub fn castling_rights(&self) -> CastlingRights {
        self.castling
    }

    pub fn en_passant_square(&self) -> Option<Square> {
        self.en_passant
    }

    pub fn halfmove_clock(&self) -> u32 {
        self.halfmove_clock
    }

    pub fn fullmove_number(&self) -> u32 {
        self.fullmove_number
    }

    // --- Seirawan's hand and gates -------------------------------------------
    //
    // Every one of these is a no-op for the other variants: an empty hand and
    // no gate rights, which is what `from_parts` builds. They live here rather
    // than in `crate::seirawan` because they are facts about a position, and a
    // position that carried them somewhere else could be copied without them.

    /// Whether `color` still holds `role` in hand.
    pub fn in_hand(&self, color: Color, role: Role) -> bool {
        self.pockets[color.index()] & Position::pocket_bit(role) != 0
    }

    /// The squares `color` may still gate a piece onto.
    pub fn gate_squares(&self, color: Color) -> Bitboard {
        self.gates & self.by_color[color.index()] & Position::home_rank(color)
    }

    /// Every square that still carries a gating right, either colour.
    pub fn gates(&self) -> Bitboard {
        self.gates
    }

    /// Where a gating move puts the piece it brings in, or `None` if it gates
    /// nothing.
    ///
    /// The square the mover leaves, for every move but a castle. A castle
    /// vacates two — the king's and the rook's — and the move's rook bit says
    /// which was chosen.
    ///
    /// Public because the seam in `bragd-games` has to answer the same question, and
    /// **cannot**: Seirawan spells a castle by the king's destination, so
    /// `e1g1H` does not contain h1 anywhere and a front end told only the wire form
    /// can say no more than "the rook's square". Writing the derivation there
    /// as well would be two copies of one rule, free to disagree about exactly
    /// the case nobody gets right. So `make_move` reads this too, and there is
    /// one definition — the same discipline `attackers_to` is given.
    ///
    /// Asked of the position before the move, because the rook's origin is a
    /// castling right and the move spends it.
    pub fn gate_square(&self, mv: Move) -> Option<Square> {
        mv.gate()?;
        if !mv.gates_at_rook() {
            return Some(mv.from());
        }
        let side = match mv.kind() {
            MoveKind::KingsideCastle => Side::Kingside,
            MoveKind::QueensideCastle => Side::Queenside,
            // The rook bit is only ever set on a castle. A move that carries it
            // without being one is malformed rather than merely unusual, and
            // saying so with `None` is better than guessing at a square.
            _ => return None,
        };
        self.castling.rook_origin(self.side_to_move, side)
    }

    /// Puts both pieces in both hands and opens exactly the squares in `gates`.
    ///
    /// Called once, when a Seirawan game starts. Separate from `from_parts`
    /// because a FEN cannot say "this is Seirawan" — the variant knows, and the
    /// variant is what calls this.
    ///
    /// **The squares are a parameter because they cannot be derived, and the
    /// version that derived them was a loaded gun.** This used to open all
    /// sixteen back-rank squares on whatever board it was handed. A gate right
    /// means "the piece that started here has never moved", and no board can
    /// answer that: a rook that went out and came home stands on a1 looking
    /// exactly like one that never left. So a midgame position run through the
    /// old version came back with four spent rights silently restored, and the
    /// first symptom would have been a hawk appearing on a square nobody could
    /// gate — in a Game a Library keeps for ever.
    ///
    /// Nothing did that yet, which is why this is a narrowing rather than a bug
    /// fix. What made it reachable is the obvious next feature: resuming a
    /// Seirawan Game from its stored FEN. **That is still not possible and this
    /// signature is how it says so** — the caller has to produce the surviving
    /// rights from somewhere, and the only place they exist is the move list,
    /// which means replaying rather than resuming.
    ///
    /// # Panics
    /// If `gates` names a square that is not on a home rank, or one that is not
    /// occupied by the piece of the colour whose home rank it is. Neither can
    /// ever carry a gating right, so a caller asking for one has misunderstood
    /// what it is asking for and is told rather than humoured.
    pub fn with_seirawan_hands(mut self, gates: Bitboard) -> Position {
        // The rank first, so that a square off both home ranks gets the message
        // about the rank rather than the one about the piece standing on it.
        assert!(
            (gates & !Position::home_ranks()).is_empty(),
            "only a home-rank square can carry a gating right"
        );
        for color in Color::ALL {
            let theirs = gates & Position::home_rank(color);
            assert!(
                (theirs & !self.by_color[color.index()]).is_empty(),
                "a gate square must hold a piece of its own colour"
            );
        }
        self.pockets = [0b11, 0b11];
        self.gates = gates;
        self.key = self.compute_key();
        self
    }

    /// Both home ranks: the whole of what a Seirawan game opens.
    ///
    /// Here rather than spelled as two constants at the call site because
    /// [`Position::with_seirawan_hands`] refuses anything outside it, and a
    /// caller should not have to reconstruct the set it is being measured
    /// against.
    pub const fn home_ranks() -> Bitboard {
        Bitboard(Position::home_rank(Color::White).0 | Position::home_rank(Color::Black).0)
    }

    const fn pocket_bit(role: Role) -> u8 {
        match role {
            Role::Hawk => 0b01,
            Role::Elephant => 0b10,
            _ => 0,
        }
    }

    const fn home_rank(color: Color) -> Bitboard {
        match color {
            Color::White => Bitboard(0x0000_0000_0000_00ff),
            Color::Black => Bitboard(0xff00_0000_0000_0000),
        }
    }

    /// The Zobrist key. Equal keys mean the same position for the purposes of
    /// repetition; see [`crate::zobrist`].
    pub fn key(&self) -> u64 {
        self.key
    }

    pub fn by_color(&self, color: Color) -> Bitboard {
        self.by_color[color.index()]
    }

    pub fn by_role(&self, role: Role) -> Bitboard {
        self.by_role[role.index()]
    }

    pub fn pieces(&self, color: Color, role: Role) -> Bitboard {
        self.by_color(color) & self.by_role(role)
    }

    pub fn occupied(&self) -> Bitboard {
        self.by_color[0] | self.by_color[1]
    }

    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        if !self.occupied().contains(square) {
            return None;
        }
        let color = if self.by_color(Color::White).contains(square) {
            Color::White
        } else {
            Color::Black
        };
        let role = Role::ALL
            .into_iter()
            .find(|&role| self.by_role(role).contains(square))
            .expect("an occupied square has a role");
        Some(Piece::new(color, role))
    }

    /// # Panics
    /// If the side has no king. Every legal position has one, and a position
    /// without one is corrupt rather than merely unusual.
    pub fn king_square(&self, color: Color) -> Square {
        self.pieces(color, Role::King).single()
    }

    // --- Attacks and check ---------------------------------------------------

    /// Every piece of either colour that attacks `square`, given an occupancy.
    ///
    /// The occupancy is a parameter rather than `self.occupied()` because
    /// callers sometimes need to ask about a board that does not exist yet —
    /// most importantly, whether a king would still be attacked once it has
    /// stepped off the square the attacking ray passes through.
    pub fn attackers_to(&self, square: Square, occupied: Bitboard) -> Bitboard {
        let pawns = self.by_role(Role::Pawn);
        let bishops_and_queens = self.by_role(Role::Bishop) | self.by_role(Role::Queen);
        let rooks_and_queens = self.by_role(Role::Rook) | self.by_role(Role::Queen);
        let gajas = self.by_role(Role::Gaja);

        // A White pawn attacks `square` exactly when it stands where a Black
        // pawn on `square` would capture. The colours look inverted and are not.
        (attacks::pawn(Color::Black, square) & pawns & self.by_color(Color::White))
            | (attacks::pawn(Color::White, square) & pawns & self.by_color(Color::Black))
            | (attacks::knight(square) & self.by_role(Role::Knight))
            | (attacks::king(square) & self.by_role(Role::King))
            | (attacks::bishop(square, occupied) & bishops_and_queens)
            | (attacks::rook(square, occupied) & rooks_and_queens)
            // Shatranj's two. Both bitboards are empty in every chess and
            // Chess960 position, so these terms contribute nothing there — the
            // cost is two table lookups and two ANDs, and the perft suite is
            // run either side of this to say what that actually costs.
            // Putting them here rather than in a
            // variant-aware copy of this function is the deliberate trade: one
            // definition of "attacks" cannot drift between variants, and check
            // detection is the one place a divergence would be a wrong result
            // rather than a wrong move list.
            | (attacks::ferz(square) & self.by_role(Role::Ferz))
            | (attacks::alfil(square) & self.by_role(Role::Alfil))
            // Seirawan's two, and they were missing while `movegen` walked them
            // — the hawk and the elephant *moved* and never *attacked*. Every
            // rule that asks "is the king attacked" inherited it: a hawk gave no
            // check, so it could not mate, so the enemy king walked onto squares
            // it covered and this generator offered the capture of a king. The
            // first thing to notice was not a wrong result but an abort —
            // `to_san` plays the move and asks the next position for a king that
            // is no longer there.
            //
            // No colour inversion here, unlike the pawn and the gaja below.
            // Both compounds are unions of symmetric tables — a bishop, a rook
            // and a knight — so "what attacks `square`" and "what `square`
            // attacks" are the same set, and asking from the target is sound.
            | (attacks::hawk(square, occupied) & self.by_role(Role::Hawk))
            | (attacks::elephant(square, occupied) & self.by_role(Role::Elephant))
            // Chaturanga's elephant, inverted per colour exactly like the pawn
            // above and for the same reason: it has a forward, so "attacks
            // `square`" is asked of the *opposite* colour's table.
            | (attacks::gaja(Color::Black, square) & gajas & self.by_color(Color::White))
            | (attacks::gaja(Color::White, square) & gajas & self.by_color(Color::Black))
    }

    pub fn is_attacked_by(&self, square: Square, attacker: Color, occupied: Bitboard) -> bool {
        !(self.attackers_to(square, occupied) & self.by_color(attacker)).is_empty()
    }

    /// Whether `color`'s king is currently attacked.
    pub fn is_check_against(&self, color: Color) -> bool {
        self.is_attacked_by(self.king_square(color), color.opponent(), self.occupied())
    }

    /// Whether the side to move is in check.
    pub fn is_check(&self) -> bool {
        self.is_check_against(self.side_to_move)
    }

    /// Whether `color` has nothing left but its king.
    ///
    /// Here rather than in the variant that needed it first because two of them
    /// now decide a Game with it — shatranj's baring rule and chaturanga's —
    /// and mechanism moves out to the shared crate once the *second* consumer
    /// exists, never in anticipation of one. It says nothing about what baring means; the two variants
    /// disagree about that, and each says so itself.
    pub fn is_bare(&self, color: Color) -> bool {
        (self.by_color(color) & !self.by_role(Role::King)).is_empty()
    }

    // --- Moves ---------------------------------------------------------------

    /// Every move the side to move may legally play.
    pub fn legal_moves(&self) -> MoveList {
        movegen::legal_moves(self)
    }

    /// Whether a move is one this position permits. Every move arriving from
    /// outside the engine goes through this — a client is not trusted to send a
    /// move that exists.
    pub fn is_legal(&self, mv: Move) -> bool {
        self.legal_moves().contains(mv)
    }

    /// The position after a null move (`--` in a game file): the turn passes,
    /// en passant expires, the halfmove clock counts on and
    /// the move number too after Black. None when the side to move is in
    /// check, where passing would leave its king to be taken.
    pub fn pass(&self) -> Option<Position> {
        if self.is_check() {
            return None;
        }
        let mut next = *self;
        next.halfmove_clock = self.halfmove_clock + 1;
        if self.side_to_move == Color::Black {
            next.fullmove_number = self.fullmove_number.saturating_add(1);
        }
        next.side_to_move = self.side_to_move.opponent();
        next.en_passant = None;
        next.key = next.compute_key();
        Some(next)
    }

    /// The position after `mv`.
    ///
    /// # Panics
    /// If `mv` does not describe something the board supports — no piece on the
    /// origin, or a capture with nothing to capture. Legality is *not* checked
    /// here; call [`Position::is_legal`] first for moves from outside.
    pub fn make_move(&self, mv: Move) -> Position {
        let us = self.side_to_move;
        let them = us.opponent();
        let (from, to, kind) = (mv.from(), mv.to(), mv.kind());
        let moving = self
            .piece_at(from)
            .expect("no piece on the move's origin square");
        debug_assert_eq!(moving.color, us, "moving an opponent's piece");

        let mut next = *self;

        // The old en passant opportunity expires whatever happens.
        if let Some(file) = self.en_passant_key_file() {
            next.key ^= zobrist::en_passant_file(file);
        }
        next.en_passant = None;

        if moving.role == Role::Pawn || kind.is_capture() {
            next.halfmove_clock = 0;
        } else {
            next.halfmove_clock += 1;
        }

        if kind == MoveKind::EnPassant {
            // The captured pawn is beside the mover, not under it.
            let captured_square = to
                .offset(-us.forward())
                .expect("an en passant target always has a square behind it");
            next.remove_piece(Piece::new(them, Role::Pawn), captured_square);
        } else if kind.is_capture() {
            let captured = self
                .piece_at(to)
                .expect("a capture with nothing on the destination");
            debug_assert_eq!(captured.color, them, "capturing one's own piece");
            next.remove_piece(captured, to);
        }

        let castling_side = match kind {
            MoveKind::KingsideCastle => Some(Side::Kingside),
            MoveKind::QueensideCastle => Some(Side::Queenside),
            _ => None,
        };

        if let Some(side) = castling_side {
            // **Both pieces come off the board before either goes back on.**
            //
            // In standard chess the king and rook can never want each other's
            // square, so lifting the king and placing it was safe. In Chess960
            // they routinely do: king f1 with the rook on g1 castles by
            // swapping, and a king on g1 with the rook on h1 castles without
            // moving at all. Placing the king first would put it on an occupied
            // square — a debug panic, and in a release build two pieces
            // silently sharing one square, which is a corrupt board that
            // outlives the move.
            let rook = Piece::new(us, Role::Rook);
            let rook_from = self
                .castling
                .rook_origin(us, side)
                .expect("a castling move implies the right that names its rook");
            next.remove_piece(moving, from);
            next.remove_piece(rook, rook_from);
            next.put_piece(moving, side.king_destination(us));
            next.put_piece(rook, side.rook_destination(us));
        } else {
            next.remove_piece(moving, from);
            next.put_piece(
                match kind.promotion_role() {
                    Some(role) => Piece::new(us, role),
                    None => moving,
                },
                to,
            );
        }

        // A right dies when its king moves — wherever it was standing, which is
        // a question about the role rather than about e1 — and when its rook
        // leaves the square the right names or is captured on it. Hence passing
        // the destination too: that is the perft-position-5 case.
        let rights = next
            .castling
            .after_move(us, moving.role == Role::King, from, to);
        if rights != next.castling {
            next.key ^= zobrist::castling(next.castling) ^ zobrist::castling(rights);
            next.castling = rights;
        }

        if kind == MoveKind::DoublePawnPush {
            next.en_passant = from.offset(us.forward());
        }

        // --- Seirawan's gating ------------------------------------------------
        //
        // Nothing here runs for any other variant: `gates` is empty, so the
        // first condition is false and the loop below has nothing to clear.
        if !next.gates.is_empty() {
            // **The right is spent by the piece leaving, gate or no gate.** A
            // back-rank piece that moves without gating gives up the square for
            // good — that is the whole tension of the opening, and forgetting it
            // would let a player hold both pieces back indefinitely and drop
            // them in at leisure.
            //
            // Castling vacates two squares and spends both rights, which is why
            // this asks about squares rather than about the move's origin.
            for square in [from, to] {
                if next.gates.contains(square) {
                    next.gates = next.gates.without(square);
                    next.key ^= zobrist::gate(square);
                }
            }
            if let Some(side) = castling_side {
                let rook_from = self
                    .castling
                    .rook_origin(us, side)
                    .expect("a castling move implies the right that names its rook");
                if next.gates.contains(rook_from) {
                    next.gates = next.gates.without(rook_from);
                    next.key ^= zobrist::gate(rook_from);
                }
            }

            if let Some(role) = mv.gate() {
                // Onto the square the mover left, or the rook's when a castle
                // chose that one. Asked of `gate_square` rather than worked out
                // here, because the seam in `bragd-games` needs the same answer and two
                // copies of it would be free to disagree about the castle.
                let square = self
                    .gate_square(mv)
                    .expect("a move that gates names the square it gates onto");
                debug_assert!(
                    self.gates.contains(square),
                    "gating onto a square with no right"
                );
                debug_assert!(self.in_hand(us, role), "gating a piece not in hand");
                next.put_piece(Piece::new(us, role), square);
                next.pockets[us.index()] &= !Position::pocket_bit(role);
                next.key ^= zobrist::pocket(us, role);
            }
        }

        next.side_to_move = them;
        next.key ^= zobrist::black_to_move();
        // Computed after the side flips: whether the opportunity is real
        // depends on who is now to move.
        if let Some(file) = next.en_passant_key_file() {
            next.key ^= zobrist::en_passant_file(file);
        }

        if us == Color::Black {
            next.fullmove_number += 1;
        }

        debug_assert_eq!(
            next.key,
            next.compute_key(),
            "incremental key drifted from the computed one after {mv:?}"
        );
        next
    }

    // --- Internals -----------------------------------------------------------

    fn put_piece(&mut self, piece: Piece, square: Square) {
        debug_assert!(!self.occupied().contains(square), "square already occupied");
        self.by_color[piece.color.index()] |= Bitboard::from_square(square);
        self.by_role[piece.role.index()] |= Bitboard::from_square(square);
        self.key ^= zobrist::piece(piece, square);
    }

    fn remove_piece(&mut self, piece: Piece, square: Square) {
        debug_assert_eq!(
            self.piece_at(square),
            Some(piece),
            "removing the wrong piece"
        );
        self.by_color[piece.color.index()] &= !Bitboard::from_square(square);
        self.by_role[piece.role.index()] &= !Bitboard::from_square(square);
        self.key ^= zobrist::piece(piece, square);
    }

    /// The en passant file that belongs in the key, if any.
    ///
    /// FEN records an en passant square after every double push, but two
    /// positions only differ for repetition purposes when the capture is
    /// actually on offer. Hashing the file unconditionally would make a
    /// harmless double push look like a new position and quietly break
    /// threefold detection.
    fn en_passant_key_file(&self) -> Option<u8> {
        let target = self.en_passant?;
        let capturers = attacks::pawn(self.side_to_move.opponent(), target)
            & self.pieces(self.side_to_move, Role::Pawn);
        (!capturers.is_empty()).then(|| target.file())
    }

    fn compute_key(&self) -> u64 {
        let mut key = 0;
        for square in self.occupied() {
            key ^= zobrist::piece(self.piece_at(square).expect("occupied"), square);
        }
        key ^= zobrist::castling(self.castling);
        // Nothing for every variant but Seirawan: an empty hand and no gate
        // rights contribute zero, so the keys of every existing position are
        // bit-for-bit what they were.
        for color in Color::ALL {
            for role in [Role::Hawk, Role::Elephant] {
                if self.in_hand(color, role) {
                    key ^= zobrist::pocket(color, role);
                }
            }
        }
        for square in self.gates {
            key ^= zobrist::gate(square);
        }
        if let Some(file) = self.en_passant_key_file() {
            key ^= zobrist::en_passant_file(file);
        }
        if self.side_to_move == Color::Black {
            key ^= zobrist::black_to_move();
        }
        key
    }

    /// Whether the position could arise in a legal game, to the extent that is
    /// cheap to check. Used to reject FEN from outside.
    pub(crate) fn is_structurally_valid(&self) -> bool {
        // Pawns cannot stand on the first or last rank.
        if !(self.by_role(Role::Pawn) & (RANKS[0] | RANKS[7])).is_empty() {
            return false;
        }
        for color in Color::ALL {
            if self.pieces(color, Role::King).count() != 1 {
                return false;
            }
            // Castling rights the board cannot support.
            //
            // Asked of the *stored* rook square rather than of a corner, and of
            // the king's back rank rather than of e1 — a Chess960 king starts
            // anywhere between its rooks. What is still checked is the part
            // that makes a right meaningful: its rook is where the right says
            // it is, the king is on the back rank, and the rook is on the side
            // of the king the right claims. Anything less would let a FEN
            // assert a right that movegen would then try to execute.
            let king = self.king_square(color);
            for side in Side::ALL {
                let Some(rook_file) = self.castling.rook_file(color, side) else {
                    continue;
                };
                let rook_home = Square::from_file_rank(rook_file, color.back_rank());
                if king.rank() != color.back_rank()
                    || self.piece_at(rook_home) != Some(Piece::new(color, Role::Rook))
                {
                    return false;
                }
                let correct_side = match side {
                    Side::Kingside => rook_file > king.file(),
                    Side::Queenside => rook_file < king.file(),
                };
                if !correct_side {
                    return false;
                }
            }
        }
        // The side that just moved cannot have left its own king en prise.
        !self.is_check_against(self.side_to_move.opponent())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess_move::CastlingNotation;
    use crate::square::squares::*;

    fn position(fen: &str) -> Position {
        Position::from_fen(fen).expect("test FEN should parse")
    }

    #[test]
    fn the_starting_position_is_what_it_should_be() {
        let start = Position::starting();
        assert_eq!(start.side_to_move(), Color::White);
        assert_eq!(start.castling_rights(), CastlingRights::STANDARD);
        assert_eq!(start.occupied().count(), 32);
        assert_eq!(
            start.piece_at(E1),
            Some(Piece::new(Color::White, Role::King))
        );
        assert_eq!(start.piece_at(D4), None);
        assert!(!start.is_check());
        assert_eq!(start.halfmove_clock(), 0);
        assert_eq!(start.fullmove_number(), 1);
    }

    #[test]
    fn keys_ignore_move_order_but_not_position() {
        // Two ways to reach the same position must hash the same, or repetition
        // detection misses transpositions.
        let start = Position::starting();
        let by_one_route = start
            .make_move(mv(&start, "g1f3"))
            .then("g8f6")
            .then("f3g1")
            .then("f6g8");
        assert_eq!(
            by_one_route.key(),
            start.key(),
            "returning to the start must return to its key"
        );

        // Same board, different side to move: different key.
        let after_white = start.make_move(mv(&start, "e2e4"));
        assert_ne!(after_white.key(), start.key());
    }

    #[test]
    fn a_double_push_only_changes_the_key_when_the_capture_is_real() {
        // The same board with the en passant square forgotten. If the key
        // differs, the double push counted as making a new position.
        fn without_the_opportunity(position: &Position) -> Position {
            let mut stripped = *position;
            stripped.en_passant = None;
            stripped.key = stripped.compute_key();
            stripped
        }

        // Nobody can capture, so the opportunity is not part of the position
        // and must not be part of the key — otherwise a harmless double push
        // would look new and threefold detection would quietly miss it.
        let quiet = position("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1");
        let pushed = quiet.make_move(mv(&quiet, "e2e4"));
        assert_eq!(pushed.en_passant_square(), Some(E3), "FEN still records it");
        assert_eq!(pushed.key(), without_the_opportunity(&pushed).key());

        // With a Black pawn on d4 the capture is genuinely on offer, so the
        // position really is different and the key must say so.
        let contested = position("4k3/8/8/8/3p4/8/4P3/4K3 w - - 0 1");
        let pushed = contested.make_move(mv(&contested, "e2e4"));
        assert_ne!(pushed.key(), without_the_opportunity(&pushed).key());
    }

    #[test]
    fn en_passant_removes_the_pawn_beside_the_destination() {
        let before = position("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1");
        let after = before.make_move(mv(&before, "e5d6"));
        assert_eq!(
            after.piece_at(D6),
            Some(Piece::new(Color::White, Role::Pawn))
        );
        assert_eq!(after.piece_at(D5), None, "the captured pawn was on d5");
        assert_eq!(after.by_role(Role::Pawn).count(), 1);
    }

    #[test]
    fn castling_moves_the_rook_too() {
        let before = position("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
        let after = before.make_move(mv(&before, "e1h1"));
        assert_eq!(
            after.piece_at(G1),
            Some(Piece::new(Color::White, Role::King))
        );
        assert_eq!(
            after.piece_at(F1),
            Some(Piece::new(Color::White, Role::Rook))
        );
        assert_eq!(after.piece_at(H1), None);
        assert!(!after.castling_rights().has(Color::White, Side::Kingside));
        assert!(!after.castling_rights().has(Color::White, Side::Queenside));
        assert!(after.castling_rights().has(Color::Black, Side::Kingside));
    }

    #[test]
    fn the_halfmove_clock_resets_on_pawn_moves_and_captures() {
        let before = position("4k3/8/8/8/8/5b2/4P3/4K2N w - - 10 20");
        assert_eq!(before.make_move(mv(&before, "e2e4")).halfmove_clock(), 0);
        assert_eq!(before.make_move(mv(&before, "h1g3")).halfmove_clock(), 11);
        assert_eq!(before.make_move(mv(&before, "e2f3")).halfmove_clock(), 0);
    }

    #[test]
    fn the_fullmove_number_advances_after_black_moves() {
        let start = Position::starting();
        let after_white = start.make_move(mv(&start, "e2e4"));
        assert_eq!(after_white.fullmove_number(), 1);
        assert_eq!(after_white.then("e7e5").fullmove_number(), 2);
    }

    #[test]
    fn structural_validation_rejects_impossible_positions() {
        assert!(
            Position::from_fen("4k3/8/8/8/8/8/8/8 w - - 0 1").is_err(),
            "no White king"
        );
        assert!(
            Position::from_fen("4k3/8/8/8/8/8/8/4K2R w K - 0 1").is_ok(),
            "a rook at home supports the right"
        );
        assert!(
            Position::from_fen("4k3/8/8/8/8/8/8/4K3 w K - 0 1").is_err(),
            "a castling right with no rook"
        );
        assert!(
            Position::from_fen("4k3/8/8/8/8/8/8/R3K3 w Q - 0 1").is_ok(),
            "queenside rook at home"
        );
        assert!(
            Position::from_fen("4k3/4Q3/8/8/8/8/8/4K3 w - - 0 1").is_err(),
            "Black cannot have just moved into check"
        );
        assert!(
            Position::from_fen("4k3/4Q3/8/8/8/8/8/4K3 b - - 0 1").is_ok(),
            "the same board is fine when it is the checked side's turn"
        );
        assert!(Position::from_fen("4k3/8/8/8/8/8/P7/4K3 w - - 0 1").is_ok());
        assert!(
            Position::from_fen("P3k3/8/8/8/8/8/8/4K3 w - - 0 1").is_err(),
            "a pawn on the eighth rank"
        );
        assert!(
            Position::from_fen("4k3/8/8/8/8/8/8/P3K3 w - - 0 1").is_err(),
            "a pawn on the first rank"
        );
    }

    // --- helpers -------------------------------------------------------------

    fn mv(position: &Position, uci: &str) -> Move {
        position
            .legal_moves()
            .into_iter()
            .find(|m| m.to_uci(CastlingNotation::KingTakesRook) == uci)
            .unwrap_or_else(|| panic!("{uci} is not legal here"))
    }

    impl Position {
        fn then(&self, uci: &str) -> Position {
            self.make_move(mv(self, uci))
        }
    }

    const D4: Square = Square::new(27);
    const D5: Square = Square::new(35);
    const D6: Square = Square::new(43);
    const E3: Square = Square::new(20);

    #[test]
    fn a_pass_hands_over_the_turn() {
        let p = position("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        let passed = p.pass().unwrap();
        assert_eq!(passed.to_fen(), "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 1 2");
        assert_eq!(passed.key(), position(&passed.to_fen()).key());
        assert_eq!(position("4k3/8/8/8/8/8/8/4K3 w - - 100 9").pass().unwrap().halfmove_clock(), 101);
        assert!(position("4k3/8/8/8/8/8/8/4K2r w - - 0 1").pass().is_none(), "in check");
    }
}
