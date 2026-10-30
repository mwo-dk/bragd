//! A position, and everything that can legally follow it.
//!
//! Structurally the same as `crates/rules-chess`'s: bitboards by role and by colour,
//! pseudo-legal generation followed by a legality filter that drops moves
//! leaving your own king attacked. What differs is arithmetic — a rank is ten
//! squares — and castling, which is the one rule that genuinely had to be
//! redecided rather than widened.

use crate::attacks;
use crate::board::{Bitboard, FILES, RANKS, Square};
use crate::chess_move::{Move, MoveKind, MoveList};
use crate::piece::{Color, Piece, Role};

/// The opening array Capablanca settled on: `RNABQKBCNR`.
///
/// Archbishop on c, chancellor on h, king on **f** and queen on e. It is not
/// symmetric and that is not a transcription error — Capablanca tried several
/// arrangements over the 1920s and this is the one he published last. The two
/// compound pieces are deliberately unlike: one comes out early beside the
/// knight, the other sits behind the bishop.
pub const STARTING_POSITION: &str =
    "rnabqkbcnr/pppppppppp/10/10/10/10/PPPPPPPPPP/RNABQKBCNR w KQkq - 0 1";

/// Where the king starts, on either side's back rank. Ten files wide, so the
/// king is not in the middle: f is the sixth file of ten.
const KING_FILE: u8 = 5;
/// The rook files, which are the two ends.
const KING_ROOK_FILE: u8 = 9;
const QUEEN_ROOK_FILE: u8 = 0;
/// Where the king lands. **Three squares, not two** — see [`Position::castles`].
const KING_SIDE_KING_FILE: u8 = 8;
const KING_SIDE_ROOK_FILE: u8 = 7;
const QUEEN_SIDE_KING_FILE: u8 = 2;
const QUEEN_SIDE_ROOK_FILE: u8 = 3;

/// Who may still castle which way.
///
/// A plain four-bit set, because unlike Chess960 the rooks start on known files
/// and stay there — there is nothing to remember but whether the right survives.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Castling(u8);

impl Castling {
    const WHITE_KING: u8 = 1;
    const WHITE_QUEEN: u8 = 2;
    const BLACK_KING: u8 = 4;
    const BLACK_QUEEN: u8 = 8;

    pub const NONE: Castling = Castling(0);
    pub const ALL: Castling = Castling(15);

    const fn bit(color: Color, king_side: bool) -> u8 {
        match (color, king_side) {
            (Color::White, true) => Castling::WHITE_KING,
            (Color::White, false) => Castling::WHITE_QUEEN,
            (Color::Black, true) => Castling::BLACK_KING,
            (Color::Black, false) => Castling::BLACK_QUEEN,
        }
    }

    pub const fn has(self, color: Color, king_side: bool) -> bool {
        self.0 & Castling::bit(color, king_side) != 0
    }

    pub const fn with(self, color: Color, king_side: bool) -> Castling {
        Castling(self.0 | Castling::bit(color, king_side))
    }

    pub const fn without(self, color: Color, king_side: bool) -> Castling {
        Castling(self.0 & !Castling::bit(color, king_side))
    }

    fn to_fen(self) -> String {
        let mut text = String::new();
        if self.has(Color::White, true) {
            text.push('K');
        }
        if self.has(Color::White, false) {
            text.push('Q');
        }
        if self.has(Color::Black, true) {
            text.push('k');
        }
        if self.has(Color::Black, false) {
            text.push('q');
        }
        if text.is_empty() {
            text.push('-');
        }
        text
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Position {
    by_role: [Bitboard; Role::COUNT],
    by_color: [Bitboard; 2],
    side_to_move: Color,
    castling: Castling,
    en_passant: Option<Square>,
    halfmove_clock: u16,
    fullmove_number: u16,
}

impl Position {
    pub fn starting() -> Position {
        Position::from_fen(STARTING_POSITION).expect("the opening array is a legal position")
    }

    // --- Reading it ----------------------------------------------------------

    pub fn by_role(&self, role: Role) -> Bitboard {
        self.by_role[role.index()]
    }

    pub fn by_color(&self, color: Color) -> Bitboard {
        self.by_color[color.index()]
    }

    pub fn pieces(&self, color: Color, role: Role) -> Bitboard {
        self.by_role(role) & self.by_color(color)
    }

    pub fn occupied(&self) -> Bitboard {
        self.by_color[0] | self.by_color[1]
    }

    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    pub fn castling(&self) -> Castling {
        self.castling
    }

    pub fn en_passant_square(&self) -> Option<Square> {
        self.en_passant
    }

    pub fn halfmove_clock(&self) -> u16 {
        self.halfmove_clock
    }

    pub fn fullmove_number(&self) -> u16 {
        self.fullmove_number
    }

    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        let color = if self.by_color(Color::White).contains(square) {
            Color::White
        } else if self.by_color(Color::Black).contains(square) {
            Color::Black
        } else {
            return None;
        };
        Role::ALL
            .into_iter()
            .find(|&role| self.by_role(role).contains(square))
            .map(|role| Piece::new(color, role))
    }

    /// # Panics
    /// If that side has no king, which no reachable position allows.
    pub fn king_square(&self, color: Color) -> Square {
        self.pieces(color, Role::King)
            .next()
            .expect("every position has a king a side")
    }

    // --- Attacks and check ---------------------------------------------------

    /// Every piece of either colour attacking `square`, given an occupancy.
    ///
    /// The occupancy is a parameter rather than `self.occupied()` so a caller
    /// can ask about a board that does not exist yet — most importantly whether
    /// a king would still be attacked once it has stepped off the square the
    /// attacking ray runs through.
    pub fn attackers_to(&self, square: Square, occupied: Bitboard) -> Bitboard {
        let pawns = self.by_role(Role::Pawn);
        let knights = self.by_role(Role::Knight);
        // The compound pieces join both of their halves' terms, which is all
        // there is to them. An archbishop is found by the bishop term and by
        // the knight term, and is not looked for separately.
        let bishops =
            self.by_role(Role::Bishop) | self.by_role(Role::Queen) | self.by_role(Role::Archbishop);
        let rooks =
            self.by_role(Role::Rook) | self.by_role(Role::Queen) | self.by_role(Role::Chancellor);
        let leapers = knights | self.by_role(Role::Archbishop) | self.by_role(Role::Chancellor);

        // A White pawn attacks `square` exactly when it stands where a Black
        // pawn on `square` would capture. The colours look inverted and are not.
        (attacks::pawn(Color::Black, square) & pawns & self.by_color(Color::White))
            | (attacks::pawn(Color::White, square) & pawns & self.by_color(Color::Black))
            | (attacks::knight(square) & leapers)
            | (attacks::king(square) & self.by_role(Role::King))
            | (attacks::bishop(square, occupied) & bishops)
            | (attacks::rook(square, occupied) & rooks)
    }

    pub fn is_attacked_by(&self, square: Square, attacker: Color, occupied: Bitboard) -> bool {
        !(self.attackers_to(square, occupied) & self.by_color(attacker)).is_empty()
    }

    pub fn is_check_against(&self, color: Color) -> bool {
        self.is_attacked_by(self.king_square(color), color.opponent(), self.occupied())
    }

    pub fn is_check(&self) -> bool {
        self.is_check_against(self.side_to_move)
    }

    // --- Moves ---------------------------------------------------------------

    pub fn legal_moves(&self) -> MoveList {
        let mut moves = self.pseudo_legal_moves();
        let us = self.side_to_move;
        moves.retain(|mv| !self.make_move(mv).is_check_against(us));
        moves
    }

    pub fn is_legal(&self, mv: Move) -> bool {
        self.legal_moves().contains(mv)
    }

    fn pseudo_legal_moves(&self) -> MoveList {
        let mut moves = MoveList::new();
        self.pawn_moves(&mut moves);
        self.piece_moves(&mut moves);
        self.castles(&mut moves);
        moves
    }

    fn pawn_moves(&self, moves: &mut MoveList) {
        let us = self.side_to_move;
        let forward = us.forward();
        let empty = !self.occupied();
        let enemies = self.by_color(us.opponent());
        let last_rank = us.promotion_rank();

        for from in self.pieces(us, Role::Pawn) {
            let file = from.file() as i16;
            let rank = from.rank() as i16;

            // One forward, and two from the starting rank if both are clear.
            if let Some(to) = Square::from_file_rank(file, rank + forward.signum())
                && empty.contains(to) {
                    self.push_pawn(moves, from, to, false, to.rank() == last_rank);
                    if from.rank() == us.pawn_rank()
                        && let Some(twice) =
                            Square::from_file_rank(file, rank + 2 * forward.signum())
                            && empty.contains(twice) {
                                moves.push(Move::new(from, twice, MoveKind::DoublePush));
                            }
                }

            // Captures, one diagonal at a time. Building the target from file
            // and rank rather than by adding an offset is what stops a capture
            // wrapping off the j-file onto the a-file.
            for sideways in [-1, 1] {
                let Some(to) = Square::from_file_rank(file + sideways, rank + forward.signum())
                else {
                    continue;
                };
                if enemies.contains(to) {
                    self.push_pawn(moves, from, to, true, to.rank() == last_rank);
                } else if self.en_passant == Some(to) {
                    moves.push(Move::new(from, to, MoveKind::EnPassant));
                }
            }
        }
    }

    /// Six promotions where chess offers four.
    fn push_pawn(
        &self,
        moves: &mut MoveList,
        from: Square,
        to: Square,
        capture: bool,
        promoting: bool,
    ) {
        if promoting {
            for role in Role::PROMOTIONS {
                moves.push(Move::new(
                    from,
                    to,
                    MoveKind::Promotion { to: role, capture },
                ));
            }
        } else {
            moves.push(Move::new(
                from,
                to,
                if capture {
                    MoveKind::Capture
                } else {
                    MoveKind::Quiet
                },
            ));
        }
    }

    fn piece_moves(&self, moves: &mut MoveList) {
        let us = self.side_to_move;
        let occupied = self.occupied();
        let ours = self.by_color(us);
        let enemies = self.by_color(us.opponent());

        for role in [
            Role::Knight,
            Role::Bishop,
            Role::Rook,
            Role::Queen,
            Role::King,
            Role::Archbishop,
            Role::Chancellor,
        ] {
            for from in self.pieces(us, role) {
                let targets = match role {
                    Role::Knight => attacks::knight(from),
                    Role::Bishop => attacks::bishop(from, occupied),
                    Role::Rook => attacks::rook(from, occupied),
                    Role::Queen => attacks::queen(from, occupied),
                    Role::King => attacks::king(from),
                    Role::Archbishop => attacks::archbishop(from, occupied),
                    Role::Chancellor => attacks::chancellor(from, occupied),
                    Role::Pawn => unreachable!("pawns are generated separately"),
                } & !ours;

                for to in targets {
                    moves.push(Move::new(
                        from,
                        to,
                        if enemies.contains(to) {
                            MoveKind::Capture
                        } else {
                            MoveKind::Quiet
                        },
                    ));
                }
            }
        }
    }

    /// Castling on a ten-file rank.
    ///
    /// **The king moves three squares, not two.** On an eight-file board "two
    /// squares" puts the king on the g- or c-file with the rook beside it, and
    /// the whole point is that the king ends up tucked behind a pawn shield
    /// with the rook connected to the centre. Ten files stretches that: from f1
    /// the king goes to i1 with the rook to h1, or to c1 with the rook to d1.
    /// Two squares would leave the king on h1 and a rook still on j1 with a gap
    /// between them, which is not castling in any sense that matters.
    ///
    /// The three conditions are chess's exactly: the right survives, everything
    /// between king and rook is empty, and the king is not in check on any
    /// square it stands on along the way — the one it starts on, the ones it
    /// crosses, and the one it lands on.
    fn castles(&self, moves: &mut MoveList) {
        let us = self.side_to_move;
        let rank = us.back_rank() as i16;
        let occupied = self.occupied();
        let king_from = Square::from_file_rank(KING_FILE as i16, rank).expect("on the board");

        // A king already in check may not castle out of it.
        if self.is_attacked_by(king_from, us.opponent(), occupied) {
            return;
        }

        for (king_side, rook_file, king_to_file, kind) in [
            (
                true,
                KING_ROOK_FILE,
                KING_SIDE_KING_FILE,
                MoveKind::CastleKingSide,
            ),
            (
                false,
                QUEEN_ROOK_FILE,
                QUEEN_SIDE_KING_FILE,
                MoveKind::CastleQueenSide,
            ),
        ] {
            if !self.castling.has(us, king_side) {
                continue;
            }
            let rook_from = Square::from_file_rank(rook_file as i16, rank).expect("on the board");
            // The right can outlive the rook if a position is written by hand.
            if !self.pieces(us, Role::Rook).contains(rook_from) {
                continue;
            }
            if !(attacks::between(king_from, rook_from) & occupied).is_empty() {
                continue;
            }

            let king_to = Square::from_file_rank(king_to_file as i16, rank).expect("on the board");
            let crossed = attacks::between(king_from, king_to).with(king_to);
            if crossed
                .into_iter()
                .any(|square| self.is_attacked_by(square, us.opponent(), occupied))
            {
                continue;
            }

            moves.push(Move::new(king_from, king_to, kind));
        }
    }

    // --- Playing one ---------------------------------------------------------

    /// The position after `mv`. Assumes the move is one this position can make;
    /// callers taking moves from outside must check [`Position::is_legal`]
    /// first, because a client is not trusted.
    pub fn make_move(&self, mv: Move) -> Position {
        let mut next = *self;
        let us = self.side_to_move;
        let them = us.opponent();
        let moved = self
            .piece_at(mv.from)
            .expect("a move starts from a piece")
            .role;

        next.en_passant = None;
        next.halfmove_clock += 1;

        // Take the captured piece off first, since en passant takes one from a
        // square the move does not land on.
        match mv.kind {
            MoveKind::EnPassant => {
                let captured = Square::from_file_rank(
                    mv.to.file() as i16,
                    mv.to.rank() as i16 - us.forward().signum(),
                )
                .expect("the pawn it passed is on the board");
                next.remove(them, Role::Pawn, captured);
                next.halfmove_clock = 0;
            }
            _ if mv.is_capture() => {
                let captured = self.piece_at(mv.to).expect("a capture takes something");
                next.remove(them, captured.role, mv.to);
                next.halfmove_clock = 0;
                // A rook taken on its home square ends that side's right.
                next.castling = clear_rights_for(next.castling, them, mv.to);
            }
            _ => {}
        }

        next.remove(us, moved, mv.from);
        match mv.kind {
            MoveKind::Promotion { to, .. } => next.put(us, to, mv.to),
            _ => next.put(us, moved, mv.to),
        }

        if moved == Role::Pawn {
            next.halfmove_clock = 0;
        }
        if mv.kind == MoveKind::DoublePush {
            next.en_passant = Square::from_file_rank(
                mv.from.file() as i16,
                mv.from.rank() as i16 + us.forward().signum(),
            );
        }

        if mv.is_castle() {
            let rank = us.back_rank() as i16;
            let (from_file, to_file) = if mv.kind == MoveKind::CastleKingSide {
                (KING_ROOK_FILE, KING_SIDE_ROOK_FILE)
            } else {
                (QUEEN_ROOK_FILE, QUEEN_SIDE_ROOK_FILE)
            };
            let rook_from = Square::from_file_rank(from_file as i16, rank).expect("on the board");
            let rook_to = Square::from_file_rank(to_file as i16, rank).expect("on the board");
            next.remove(us, Role::Rook, rook_from);
            next.put(us, Role::Rook, rook_to);
        }

        if moved == Role::King {
            next.castling = next.castling.without(us, true).without(us, false);
        }
        if moved == Role::Rook {
            next.castling = clear_rights_for(next.castling, us, mv.from);
        }

        next.side_to_move = them;
        if us == Color::Black {
            next.fullmove_number += 1;
        }
        next
    }

    fn remove(&mut self, color: Color, role: Role, square: Square) {
        self.by_role[role.index()] = self.by_role[role.index()].without(square);
        self.by_color[color.index()] = self.by_color[color.index()].without(square);
    }

    fn put(&mut self, color: Color, role: Role, square: Square) {
        self.by_role[role.index()] = self.by_role[role.index()].with(square);
        self.by_color[color.index()] = self.by_color[color.index()].with(square);
    }

    // --- FEN -----------------------------------------------------------------

    /// Reads a FEN. Ten files per rank, so an empty rank is written `10` and a
    /// run of squares can be two digits — which is the one thing a chess FEN
    /// parser lifted straight over would get wrong.
    pub fn from_fen(text: &str) -> Result<Position, FenError> {
        let mut fields = text.split_whitespace();
        let placement = fields.next().ok_or(FenError::Empty)?;

        let mut position = Position {
            by_role: [Bitboard::EMPTY; Role::COUNT],
            by_color: [Bitboard::EMPTY; 2],
            side_to_move: Color::White,
            castling: Castling::NONE,
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
        };

        let ranks: Vec<&str> = placement.split('/').collect();
        if ranks.len() != RANKS as usize {
            return Err(FenError::WrongRankCount(ranks.len()));
        }
        for (index, row) in ranks.iter().enumerate() {
            let rank = RANKS as usize - 1 - index; // FEN writes rank 8 first.
            let mut file = 0usize;
            let mut digits = String::new();
            for character in row.chars() {
                if character.is_ascii_digit() {
                    digits.push(character);
                    continue;
                }
                if !digits.is_empty() {
                    file += digits.parse::<usize>().map_err(|_| FenError::BadRank)?;
                    digits.clear();
                }
                let piece =
                    Piece::from_fen_char(character).ok_or(FenError::UnknownPiece(character))?;
                let square =
                    Square::from_file_rank(file as i16, rank as i16).ok_or(FenError::BadRank)?;
                position.put(piece.color, piece.role, square);
                file += 1;
            }
            if !digits.is_empty() {
                file += digits.parse::<usize>().map_err(|_| FenError::BadRank)?;
            }
            if file != FILES as usize {
                return Err(FenError::RankDoesNotAddUp(file));
            }
        }

        position.side_to_move = match fields.next() {
            Some("b") => Color::Black,
            Some("w") | None => Color::White,
            Some(_) => return Err(FenError::BadSideToMove),
        };

        if let Some(rights) = fields.next() {
            for character in rights.chars() {
                position.castling = match character {
                    'K' => position.castling.with(Color::White, true),
                    'Q' => position.castling.with(Color::White, false),
                    'k' => position.castling.with(Color::Black, true),
                    'q' => position.castling.with(Color::Black, false),
                    '-' => position.castling,
                    _ => return Err(FenError::BadCastling(character)),
                };
            }
        }

        if let Some(square) = fields.next() {
            position.en_passant = if square == "-" {
                None
            } else {
                Some(parse_square(square).ok_or(FenError::BadEnPassant)?)
            };
        }

        if let Some(clock) = fields.next() {
            position.halfmove_clock = clock.parse().map_err(|_| FenError::BadClock)?;
        }
        if let Some(number) = fields.next() {
            position.fullmove_number = number.parse().map_err(|_| FenError::BadClock)?;
        }

        for color in Color::ALL {
            if position.pieces(color, Role::King).count() != 1 {
                return Err(FenError::WrongKingCount);
            }
        }

        Ok(position)
    }

    pub fn to_fen(&self) -> String {
        let mut placement = String::new();
        for rank in (0..RANKS).rev() {
            let mut empty = 0;
            for file in 0..FILES {
                let square =
                    Square::from_file_rank(file as i16, rank as i16).expect("on the board");
                match self.piece_at(square) {
                    Some(piece) => {
                        if empty > 0 {
                            placement.push_str(&empty.to_string());
                            empty = 0;
                        }
                        placement.push(piece.fen_char());
                    }
                    None => empty += 1,
                }
            }
            if empty > 0 {
                placement.push_str(&empty.to_string());
            }
            if rank > 0 {
                placement.push('/');
            }
        }

        format!(
            "{} {} {} {} {} {}",
            placement,
            if self.side_to_move == Color::White {
                "w"
            } else {
                "b"
            },
            self.castling.to_fen(),
            self.en_passant
                .map(|square| square.to_string())
                .unwrap_or_else(|| "-".to_string()),
            self.halfmove_clock,
            self.fullmove_number
        )
    }
}

/// A rook leaving or being taken on its home square ends that right.
fn clear_rights_for(castling: Castling, color: Color, square: Square) -> Castling {
    if square.rank() != color.back_rank() {
        return castling;
    }
    match square.file() {
        KING_ROOK_FILE => castling.without(color, true),
        QUEEN_ROOK_FILE => castling.without(color, false),
        _ => castling,
    }
}

fn parse_square(text: &str) -> Option<Square> {
    let bytes = text.as_bytes();
    if bytes.len() != 2 {
        return None;
    }
    Square::from_file_rank(
        (bytes[0] as char).to_ascii_lowercase() as i16 - 'a' as i16,
        bytes[1] as i16 - '1' as i16,
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FenError {
    Empty,
    WrongRankCount(usize),
    RankDoesNotAddUp(usize),
    BadRank,
    UnknownPiece(char),
    BadSideToMove,
    BadCastling(char),
    BadEnPassant,
    BadClock,
    WrongKingCount,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(name: &str) -> Square {
        parse_square(name).expect("a square")
    }

    fn uci(position: &Position) -> Vec<String> {
        let mut moves: Vec<String> = position
            .legal_moves()
            .into_iter()
            .map(|mv| mv.to_uci())
            .collect();
        moves.sort();
        moves
    }

    #[test]
    fn the_opening_array_is_capablancas() {
        let start = Position::starting();
        assert_eq!(start.to_fen(), STARTING_POSITION);
        assert_eq!(start.by_role(Role::Pawn).count(), 20, "ten pawns a side");
        assert_eq!(start.by_role(Role::Archbishop).count(), 2);
        assert_eq!(start.by_role(Role::Chancellor).count(), 2);
        assert_eq!(
            start.piece_at(square("f1")).map(|p| p.role),
            Some(Role::King)
        );
        assert_eq!(
            start.piece_at(square("e1")).map(|p| p.role),
            Some(Role::Queen)
        );
        assert_eq!(
            start.piece_at(square("c1")).map(|p| p.role),
            Some(Role::Archbishop)
        );
        assert_eq!(
            start.piece_at(square("h1")).map(|p| p.role),
            Some(Role::Chancellor)
        );
    }

    /// Every FEN this engine writes is one it can read back to the same
    /// position — including the two-digit empty runs a ten-file rank produces.
    #[test]
    fn fen_round_trips_through_two_digit_runs() {
        for fen in [
            STARTING_POSITION,
            "10/10/10/10/10/10/10/K8k w - - 0 1",
            "rnabqkbcnr/pppppppppp/10/10/10/10/PPPPPPPPPP/RNABQKBCNR b Kq j6 3 17",
        ] {
            let position = Position::from_fen(fen).expect(fen);
            assert_eq!(position.to_fen(), fen);
        }
    }

    #[test]
    fn a_malformed_fen_is_an_error_rather_than_a_panic() {
        for fen in [
            "",
            "rnabqkbcnr/pppppppppp/10/10/10/10/PPPPPPPPPP w KQkq - 0 1", // seven ranks
            "rnabqkbcnr/pppppppppp/9/10/10/10/PPPPPPPPPP/RNABQKBCNR w - - 0 1", // short rank
            "rnabqkbcnr/pppppppppp/10/10/10/10/PPPPPPPPPP/RNABQKBXNR w - - 0 1", // no such piece
            "10/10/10/10/10/10/10/10 w - - 0 1",                         // no kings
        ] {
            assert!(Position::from_fen(fen).is_err(), "{fen}");
        }
    }

    /// Twenty pawn moves and eight knight moves, and nothing else can move:
    /// both bishops, both compounds, the queen and the king are behind the pawn
    /// wall, and neither compound can leap out — an archbishop's knight half
    /// starts on c1, whose knight squares are a2, b3, d3 and e2, all occupied
    /// or blocked by its own men. Wait: b3 and d3 are empty.
    #[test]
    fn the_opening_position_has_the_moves_a_hand_count_gives() {
        let moves = uci(&Position::starting());
        // Ten pawns, two steps each.
        assert_eq!(
            moves.iter().filter(|m| m.as_bytes()[1] == b'2').count(),
            20,
            "{moves:?}"
        );
        // The knights on b1 and i1 have two squares each, and the archbishop on
        // c1 and the chancellor on h1 have two apiece as well — their knight
        // halves reach the third rank over the pawns.
        for leap in [
            "b1a3", "b1c3", "i1h3", "i1j3", "c1b3", "c1d3", "h1g3", "h1i3",
        ] {
            assert!(moves.contains(&leap.to_string()), "{leap} in {moves:?}");
        }
        assert_eq!(moves.len(), 28, "{moves:?}");
    }

    /// Three squares, and the rook lands beside the king. The number is the
    /// whole reason castling had to be redecided for this board.
    #[test]
    fn castling_moves_the_king_three_squares() {
        let position =
            Position::from_fen("rnabqkbcnr/pppppppppp/10/10/10/10/PPPPPPPPPP/R4K3R w KQkq - 0 1")
                .expect("a legal position");
        let moves = uci(&position);
        assert!(moves.contains(&"f1i1".to_string()), "kingside: {moves:?}");
        assert!(moves.contains(&"f1c1".to_string()), "queenside: {moves:?}");

        let after = position.make_move(Move::new(
            square("f1"),
            square("i1"),
            MoveKind::CastleKingSide,
        ));
        assert_eq!(
            after.piece_at(square("i1")).map(|p| p.role),
            Some(Role::King)
        );
        assert_eq!(
            after.piece_at(square("h1")).map(|p| p.role),
            Some(Role::Rook)
        );
        assert!(after.piece_at(square("j1")).is_none(), "the rook moved");
        assert!(!after.castling().has(Color::White, true));
        assert!(!after.castling().has(Color::White, false), "both rights go");
    }

    /// Out of, through, and into — three separate refusals, and on this board
    /// "through" is two squares rather than one, because the king crosses g and
    /// h on its way to i.
    #[test]
    fn a_king_may_not_castle_out_of_through_or_into_check() {
        // Black rook on h8, so h1 is attacked: a square the king crosses.
        let through = Position::from_fen("k6r2/10/10/10/10/10/10/R4K3R w KQ - 0 1").expect("legal");
        assert!(
            !uci(&through).contains(&"f1i1".to_string()),
            "through check"
        );
        assert!(
            uci(&through).contains(&"f1c1".to_string()),
            "the other way is clear"
        );

        // Black rook on f8: the king is in check and may not castle either way.
        let out = Position::from_fen("k4r4/10/10/10/10/10/10/R4K3R w KQ - 0 1").expect("legal");
        assert!(!uci(&out).contains(&"f1i1".to_string()));
        assert!(!uci(&out).contains(&"f1c1".to_string()));

        // Black rook on i8: the destination itself is attacked.
        let into = Position::from_fen("k7r1/10/10/10/10/10/10/R4K3R w KQ - 0 1").expect("legal");
        assert!(!uci(&into).contains(&"f1i1".to_string()), "into check");
    }

    #[test]
    fn castling_needs_the_squares_between_king_and_rook_empty() {
        let blocked =
            Position::from_fen("5k4/10/10/10/10/10/10/RN3K2NR w KQ - 0 1").expect("legal");
        let moves = uci(&blocked);
        assert!(!moves.contains(&"f1i1".to_string()));
        assert!(!moves.contains(&"f1c1".to_string()));
    }

    #[test]
    fn a_pawn_promotes_to_any_of_six_pieces() {
        let position = Position::from_fen("5k4/P9/10/10/10/10/10/5K4 w - - 0 1").expect("legal");
        let promotions: Vec<String> = uci(&position)
            .into_iter()
            .filter(|m| m.starts_with("a7a8"))
            .collect();
        assert_eq!(promotions.len(), 6, "{promotions:?}");
        assert!(promotions.contains(&"a7a8c".to_string()), "a chancellor");
        assert!(promotions.contains(&"a7a8a".to_string()), "an archbishop");
    }

    #[test]
    fn en_passant_works_across_a_ten_file_rank() {
        // A White pawn on i5 and a Black pawn stepping j7-j5 beside it.
        let position = Position::from_fen("5k4/9p/10/8P1/10/10/10/5K4 b - - 0 1").expect("legal");
        let double = position
            .legal_moves()
            .into_iter()
            .find(|mv| mv.kind == MoveKind::DoublePush)
            .expect("the pawn can step twice");
        let after = position.make_move(double);
        assert_eq!(after.en_passant_square(), Some(square("j6")));

        let capture = after
            .legal_moves()
            .into_iter()
            .find(|mv| mv.kind == MoveKind::EnPassant)
            .expect("en passant is available");
        let taken = after.make_move(capture);
        assert_eq!(
            taken.by_role(Role::Pawn).count(),
            1,
            "the black pawn is gone"
        );
        assert!(taken.piece_at(square("j5")).is_none());
    }

    /// A chancellor gives check as a rook *and* as a knight, and `attackers_to`
    /// has to find it both ways. This is the one place where missing half of a
    /// compound piece would be a wrong result rather than a short move list: a
    /// king would be allowed to stand in check.
    #[test]
    fn a_compound_checks_with_either_half() {
        // The rook half: chancellor j1, black king j8, nothing between.
        let as_rook = Position::from_fen("9k/10/10/10/10/10/10/5K3C w - - 0 1").expect("legal");
        assert!(
            as_rook.is_check_against(Color::Black),
            "down the open j-file"
        );

        // The knight half: chancellor e6, black king f8. Not on the e-file and
        // not on rank 6 — only a knight's leap reaches it.
        let as_knight = Position::from_fen("5k4/10/4C5/10/10/10/10/5K4 w - - 0 1").expect("legal");
        assert!(as_knight.is_check_against(Color::Black), "as a knight");

        // And the control: a plain rook on the same square does not.
        let plain = Position::from_fen("5k4/10/4R5/10/10/10/10/5K4 w - - 0 1").expect("legal");
        assert!(!plain.is_check_against(Color::Black), "a rook cannot leap");
    }

    #[test]
    fn no_generated_move_leaves_the_mover_in_check() {
        fn walk(position: &Position, depth: u32) {
            for mv in &position.legal_moves() {
                let next = position.make_move(mv);
                assert!(
                    !next.is_check_against(position.side_to_move()),
                    "{} leaves the mover in check in {}",
                    mv.to_uci(),
                    position.to_fen()
                );
                if depth > 0 {
                    walk(&next, depth - 1);
                }
            }
        }
        walk(&Position::starting(), 2);
    }
}
