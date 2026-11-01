//! A position: the board, the two hands, and whose turn it is.
//!
//! The hands are the reason this crate exists. A captured piece is not gone —
//! it changes sides, reverts to its unpromoted form and can be put back on the
//! board as a move. So a position is *not* fully described by where the pieces
//! are, and anything that treats it as though it were — a repetition check, a
//! transposition table, a "have we been here before" — is wrong in a way that
//! only shows up in long games.

use crate::attacks;
use crate::board::{Bitboard, FILES, RANKS, SQUARES, Square};
use crate::piece::{Piece, Role, Side};
use core::fmt;

std::thread_local! {
    static GENERATED: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

/// How many times this thread has generated a position's legal moves. A cost
/// meter: tests prove a replay generates them once a ply by counting, not by
/// timing.
pub fn generated() -> u64 {
    GENERATED.with(|n| n.get())
}

/// A move: something from the board, or something from the hand.
///
/// Two shapes rather than one with an optional origin. A drop has no origin, no
/// capture and no promotion, and three fields that are always absent on one
/// variant is the sort of type that invites a bug — the compiler cannot tell
/// you that you forgot a drop can never capture.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Move {
    /// A piece already on the board.
    Board {
        from: Square,
        to: Square,
        /// Whether it promotes on arrival. Forced when the piece would
        /// otherwise be stuck — see [`Position::legal_moves`].
        promote: bool,
    },
    /// A piece from the hand, placed on an empty square.
    Drop { role: Role, to: Square },
}

impl Move {
    pub fn to(self) -> Square {
        match self {
            Move::Board { to, .. } | Move::Drop { to, .. } => to,
        }
    }

    /// The wire form, which is USI's — the notation every shogi program speaks.
    ///
    /// `7g7f` for a board move, `7g7f+` when it promotes, and `P*5e` for a
    /// drop. Following USI rather than inventing one: unlike tafl, shogi has an
    /// established machine notation, and a Game stored in it can be replayed
    /// by anything.
    pub fn to_usi(self) -> String {
        match self {
            Move::Board { from, to, promote } => {
                format!(
                    "{}{}{}",
                    from.name(),
                    to.name(),
                    if promote { "+" } else { "" }
                )
            }
            Move::Drop { role, to } => format!("{}*{}", role.letter(), to.name()),
        }
    }

    pub fn parse(text: &str) -> Option<Move> {
        // **Refused before anything is sliced.** USI is ASCII, and the two
        // slices below are byte indices — so a four-*byte* emoji passes a
        // `len() == 4` check and then panics halfway through a character. The
        // text arrives from a wire message or a stored row, which makes that a
        // panic in the authority for a live game rather than a parse failure.
        if !text.is_ascii() {
            return None;
        }
        if let Some((role, square)) = text.split_once('*') {
            let role = Role::from_letter(role)?;
            // Only the seven that can be held. `K*5e` is not a typo to be
            // tolerated; it is a move that cannot exist.
            if !Role::IN_HAND.contains(&role) {
                return None;
            }
            return Some(Move::Drop {
                role,
                to: Square::parse(square)?,
            });
        }
        let promote = text.ends_with('+');
        let squares = text.strip_suffix('+').unwrap_or(text);
        if squares.len() != 4 {
            return None;
        }
        Some(Move::Board {
            from: Square::parse(&squares[..2])?,
            to: Square::parse(&squares[2..])?,
            promote,
        })
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_usi())
    }
}

/// How many of each holdable role a side has in hand.
pub type Hand = [u8; 7];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    by_side: [Bitboard; 2],
    by_role: [Bitboard; Role::COUNT],
    /// `hands[side][index into Role::IN_HAND]`.
    hands: [Hand; 2],
    to_move: Side,
    /// Plies played. Shogi has no fifty-move rule; this is for the move number
    /// in SFEN and for nothing else.
    ply: u32,
}

/// The opening array, in SFEN.
pub const START: &str = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";

impl Position {
    /// The opening array, which is the same for both sides and symmetric.
    pub fn starting() -> Position {
        // Written as a picture, rank 9 down to rank 1 — the way a board is
        // printed and the way SFEN writes it. Lowercase is White, who sits at
        // the top and moves down.
        Position::from_sfen(START)
            .expect("the opening array is a valid SFEN")
    }

    pub fn to_move(&self) -> Side {
        self.to_move
    }

    pub fn ply(&self) -> u32 {
        self.ply
    }

    pub fn side(&self, side: Side) -> Bitboard {
        self.by_side[side.index()]
    }

    pub fn by_role(&self, role: Role) -> Bitboard {
        self.by_role[role.index()]
    }

    pub fn pieces(&self, side: Side, role: Role) -> Bitboard {
        self.by_side[side.index()] & self.by_role[role.index()]
    }

    pub fn occupied(&self) -> Bitboard {
        self.by_side[0] | self.by_side[1]
    }

    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        let side = if self.by_side[0].contains(square) {
            Side::Black
        } else if self.by_side[1].contains(square) {
            Side::White
        } else {
            return None;
        };
        let role = Role::ALL
            .into_iter()
            .find(|&role| self.by_role[role.index()].contains(square))?;
        Some(Piece::new(side, role))
    }

    /// How many of `role` are in `side`'s hand.
    pub fn in_hand(&self, side: Side, role: Role) -> u8 {
        match Role::IN_HAND.iter().position(|&held| held == role) {
            Some(index) => self.hands[side.index()][index],
            None => 0,
        }
    }

    pub fn hand(&self, side: Side) -> Hand {
        self.hands[side.index()]
    }

    /// Where `side`'s king is, or `None` if it has been taken — which only
    /// happens in a position built by hand, since a real game ends first.
    pub fn king_square(&self, side: Side) -> Option<Square> {
        self.pieces(side, Role::King).first()
    }

    /// Whether `side`'s king is attacked.
    pub fn is_check_against(&self, side: Side) -> bool {
        let Some(king) = self.king_square(side) else {
            return false;
        };
        self.is_attacked(king, side.other())
    }

    pub fn is_check(&self) -> bool {
        self.is_check_against(self.to_move)
    }

    /// Whether `by` attacks `square`.
    ///
    /// Asked by walking `by`'s pieces rather than by radiating out from the
    /// square. Slower, and correct for every role without a second table: the
    /// reverse trick needs each piece's *inverse* attack set, and in shogi
    /// almost every piece is asymmetric, so the inverse table is a second place
    /// to get the mirroring wrong.
    pub fn is_attacked(&self, square: Square, by: Side) -> bool {
        let occupied = self.occupied();
        for role in Role::ALL {
            for from in self.pieces(by, role) {
                if attacks::reachable(by, role, from, occupied).contains(square) {
                    return true;
                }
            }
        }
        false
    }

    /// Every legal move, drops included.
    ///
    /// Legality here means the full thing: a move that leaves your own king
    /// attacked is not generated, and neither is a drop that breaks one of the
    /// three drop rules.
    pub fn legal_moves(&self) -> Vec<Move> {
        GENERATED.with(|n| n.set(n.get() + 1));
        self.pseudo_legal()
            .into_iter()
            .filter(|&mv| !self.after(mv).is_check_against(self.to_move))
            .filter(|&mv| !self.is_drop_mate(mv))
            .collect()
    }

    /// Moves that follow the movement rules, before the king is considered.
    fn pseudo_legal(&self) -> Vec<Move> {
        let us = self.to_move;
        let ours = self.side(us);
        let occupied = self.occupied();
        let mut moves = Vec::new();

        for role in Role::ALL {
            for from in self.pieces(us, role) {
                let targets = Bitboard(attacks::reachable(us, role, from, occupied).0 & !ours.0);
                for to in targets {
                    let can_promote = role.promoted().is_some()
                        && (us.promotion_zone().contains(to) || us.promotion_zone().contains(from));
                    // **Forced when the piece would otherwise be stuck.** A
                    // pawn moving to the last rank must promote; there is no
                    // legal move that leaves it there.
                    let must_promote = attacks::would_be_stuck(us, role, to);
                    if !must_promote {
                        moves.push(Move::Board {
                            from,
                            to,
                            promote: false,
                        });
                    }
                    if can_promote {
                        moves.push(Move::Board {
                            from,
                            to,
                            promote: true,
                        });
                    }
                }
            }
        }

        for (index, &role) in Role::IN_HAND.iter().enumerate() {
            if self.hands[us.index()][index] == 0 {
                continue;
            }
            for to in occupied.complement() {
                // A dropped piece never promotes, and one that could never move
                // again may not be dropped at all.
                if attacks::would_be_stuck(us, role, to) {
                    continue;
                }
                // **Nifu**: never two of your own unpromoted pawns on one file.
                // Promoted pawns do not count, which is why this asks about the
                // role rather than about the file's contents.
                if role == Role::Pawn
                    && !(self.pieces(us, Role::Pawn) & Bitboard::file(to.file())).is_empty()
                {
                    continue;
                }
                moves.push(Move::Drop { role, to });
            }
        }

        moves
    }

    /// **Uchifuzume**: a pawn may not be dropped to give immediate checkmate.
    ///
    /// The strangest rule in the game and the only one that requires looking a
    /// move ahead — you may drop a pawn to give check, and you may move a pawn
    /// to give mate; you may not *drop* one to give *mate*. Checked last,
    /// because it is the only rule here that costs a full move generation.
    fn is_drop_mate(&self, mv: Move) -> bool {
        let Move::Drop {
            role: Role::Pawn, ..
        } = mv
        else {
            return false;
        };
        let after = self.after(mv);
        if !after.is_check_against(after.to_move) {
            return false;
        }
        // Mate if the side now to move has no way out. Their own legality is
        // what decides it, minus this rule — which cannot recurse, because
        // their escape is never a pawn drop giving mate to a side already
        // mated.
        after
            .pseudo_legal()
            .into_iter()
            .all(|reply| after.after(reply).is_check_against(after.to_move))
    }

    /// The position after `mv`, without checking that it is legal.
    pub fn after(&self, mv: Move) -> Position {
        let us = self.to_move;
        let mut next = *self;

        match mv {
            Move::Board { from, to, promote } => {
                let moving = self.piece_at(from).expect("no piece on the origin");
                if let Some(captured) = self.piece_at(to) {
                    next.remove(captured, to);
                    // **The one place demotion happens.** A dragon taken from
                    // the other player becomes a rook in your hand.
                    next.add_to_hand(us, captured.role.demoted());
                }
                next.remove(moving, from);
                let landed = if promote {
                    moving.role.promoted().unwrap_or(moving.role)
                } else {
                    moving.role
                };
                next.put(Piece::new(us, landed), to);
            }
            Move::Drop { role, to } => {
                next.take_from_hand(us, role);
                next.put(Piece::new(us, role), to);
            }
        }

        next.to_move = us.other();
        next.ply += 1;
        next
    }

    fn put(&mut self, piece: Piece, square: Square) {
        self.by_side[piece.side.index()] = self.by_side[piece.side.index()].with(square);
        self.by_role[piece.role.index()] = self.by_role[piece.role.index()].with(square);
    }

    fn remove(&mut self, piece: Piece, square: Square) {
        self.by_side[piece.side.index()] = self.by_side[piece.side.index()].without(square);
        self.by_role[piece.role.index()] = self.by_role[piece.role.index()].without(square);
    }

    fn add_to_hand(&mut self, side: Side, role: Role) {
        if let Some(index) = Role::IN_HAND.iter().position(|&held| held == role) {
            self.hands[side.index()][index] += 1;
        }
    }

    fn take_from_hand(&mut self, side: Side, role: Role) {
        let index = Role::IN_HAND
            .iter()
            .position(|&held| held == role)
            .expect("only holdable roles are dropped");
        let count = &mut self.hands[side.index()][index];
        assert!(*count > 0, "dropping a {role:?} that is not in hand");
        *count -= 1;
    }

    // --- SFEN ---------------------------------------------------------------

    /// Reads SFEN, shogi's FEN. Four fields: board, side, hands, move number.
    pub fn from_sfen(text: &str) -> Option<Position> {
        let mut fields = text.split_whitespace();
        let board = fields.next()?;
        let side = fields.next()?;
        let hands = fields.next()?;
        let ply: u32 = fields.next().unwrap_or("1").parse().ok()?;

        let mut position = Position {
            by_side: [Bitboard::EMPTY; 2],
            by_role: [Bitboard::EMPTY; Role::COUNT],
            hands: [[0; 7]; 2],
            to_move: match side {
                "b" => Side::Black,
                "w" => Side::White,
                _ => return None,
            },
            ply: ply.max(1),
        };

        let ranks: Vec<&str> = board.split('/').collect();
        if ranks.len() != RANKS as usize {
            return None;
        }
        for (index, row) in ranks.iter().enumerate() {
            // SFEN writes rank 9 — White's back rank — first.
            let rank = (RANKS - 1 - index as u8) as i16;
            let mut file: i16 = 0;
            let mut promoted = false;
            for character in row.chars() {
                if let Some(skip) = character.to_digit(10) {
                    if promoted {
                        return None;
                    }
                    file += skip as i16;
                    continue;
                }
                if character == '+' {
                    promoted = true;
                    continue;
                }
                let base = Role::from_letter(&character.to_ascii_uppercase().to_string())?;
                let role = if promoted { base.promoted()? } else { base };
                promoted = false;
                let side = if character.is_ascii_uppercase() {
                    Side::Black
                } else {
                    Side::White
                };
                // SFEN's files run right to left, so the first character of a
                // rank is file 9 — internal file 0.
                let square = Square::from_file_rank(file, rank)?;
                position.put(Piece::new(side, role), square);
                file += 1;
            }
            if file != FILES as i16 {
                return None;
            }
        }

        if hands != "-" {
            let mut count: u8 = 0;
            for character in hands.chars() {
                if let Some(digit) = character.to_digit(10) {
                    count = count * 10 + digit as u8;
                    continue;
                }
                let role = Role::from_letter(&character.to_ascii_uppercase().to_string())?;
                if !Role::IN_HAND.contains(&role) {
                    return None;
                }
                let side = if character.is_ascii_uppercase() {
                    Side::Black
                } else {
                    Side::White
                };
                let index = Role::IN_HAND.iter().position(|&held| held == role)?;
                position.hands[side.index()][index] += count.max(1);
                count = 0;
            }
        }

        Some(position)
    }

    pub fn to_sfen(&self) -> String {
        let mut board = String::new();
        for rank in (0..RANKS).rev() {
            let mut empty = 0;
            for file in 0..FILES {
                let square =
                    Square::from_file_rank(file as i16, rank as i16).expect("on the board");
                match self.piece_at(square) {
                    Some(piece) => {
                        if empty > 0 {
                            board.push_str(&empty.to_string());
                            empty = 0;
                        }
                        board.push_str(&piece.to_string());
                    }
                    None => empty += 1,
                }
            }
            if empty > 0 {
                board.push_str(&empty.to_string());
            }
            if rank > 0 {
                board.push('/');
            }
        }

        let mut hands = String::new();
        for side in Side::ALL {
            for (index, &role) in Role::IN_HAND.iter().enumerate() {
                let count = self.hands[side.index()][index];
                if count == 0 {
                    continue;
                }
                if count > 1 {
                    hands.push_str(&count.to_string());
                }
                let letter = role.letter();
                hands.push_str(&match side {
                    Side::Black => letter.to_string(),
                    Side::White => letter.to_ascii_lowercase(),
                });
            }
        }
        if hands.is_empty() {
            hands.push('-');
        }

        format!(
            "{board} {} {hands} {}",
            match self.to_move {
                Side::Black => "b",
                Side::White => "w",
            },
            self.ply
        )
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..RANKS).rev() {
            for file in 0..FILES {
                let square =
                    Square::from_file_rank(file as i16, rank as i16).expect("on the board");
                match self.piece_at(square) {
                    Some(piece) => write!(f, "{piece:>2}")?,
                    None => f.write_str(" .")?,
                }
            }
            writeln!(f)?;
        }
        writeln!(f, "{:?} to move", self.to_move)?;
        write!(f, "{}", self.to_sfen())
    }
}

/// How many squares there are, re-exported so a caller need not reach into the
/// board module for a loop bound.
pub const BOARD_SQUARES: usize = SQUARES;
