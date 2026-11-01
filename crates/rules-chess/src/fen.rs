//! Forsyth-Edwards Notation — how a position crosses every boundary.
//!
//! FEN is the interchange format for positions: the seam, a Library, test
//! fixtures, and anything a Player pastes in. Everything here therefore treats
//! its input as hostile and returns an error rather than a panic. A position
//! that parses is structurally valid; see
//! [`Position::is_structurally_valid`](crate::Position).

use crate::castling::{CastlingFieldError, CastlingRights, Side};
use crate::piece::{Color, Piece, Role};
use crate::position::Position;
use crate::square::Square;
use core::fmt;

pub const STARTING_POSITION: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FenError {
    /// Fewer than four fields, or more than six.
    WrongFieldCount(usize),
    /// The board field does not have eight ranks.
    WrongRankCount(usize),
    /// A rank describes more or fewer than eight squares.
    WrongRankWidth {
        rank: usize,
        width: usize,
    },
    UnknownPiece(char),
    /// The side-to-move field was not `w` or `b`.
    BadSideToMove(String),
    BadCastlingRights(String),
    BadEnPassantSquare(String),
    BadMoveNumber(String),
    /// Parsed, but describes a position that cannot occur: a missing king, a
    /// pawn on the back rank, castling rights with no rook, or a side that has
    /// left its own king capturable.
    Impossible,
}

impl fmt::Display for FenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FenError::WrongFieldCount(count) => {
                write!(f, "expected 4 to 6 fields, found {count}")
            }
            FenError::WrongRankCount(count) => write!(f, "expected 8 ranks, found {count}"),
            FenError::WrongRankWidth { rank, width } => {
                write!(f, "rank {} describes {width} squares, not 8", 8 - rank)
            }
            FenError::UnknownPiece(letter) => write!(f, "{letter:?} is not a piece"),
            FenError::BadSideToMove(field) => write!(f, "{field:?} is not 'w' or 'b'"),
            // Holds the reason rather than the field: there are several ways
            // a castling field can be wrong now, and echoing "HAha" back says
            // nothing about which of them happened.
            FenError::BadCastlingRights(reason) => write!(f, "bad castling field: {reason}"),
            FenError::BadEnPassantSquare(field) => {
                write!(f, "{field:?} is not an en passant square")
            }
            FenError::BadMoveNumber(field) => write!(f, "{field:?} is not a move number"),
            FenError::Impossible => write!(f, "the position could not occur in a game"),
        }
    }
}

impl std::error::Error for FenError {}

impl Position {
    pub fn from_fen(fen: &str) -> Result<Position, FenError> {
        let fields: Vec<&str> = fen.split_whitespace().collect();
        if fields.len() < 4 || fields.len() > 6 {
            return Err(FenError::WrongFieldCount(fields.len()));
        }

        let squares = parse_board(fields[0])?;

        let side_to_move = match fields[1] {
            "w" => Color::White,
            "b" => Color::Black,
            other => return Err(FenError::BadSideToMove(other.to_string())),
        };

        let castling = parse_castling_field(fields[2], &squares)
            .map_err(|problem| FenError::BadCastlingRights(problem.to_string()))?;

        let en_passant = match fields[3] {
            "-" => None,
            square => Some(
                Square::parse(square)
                    .filter(|square| square.rank() == 2 || square.rank() == 5)
                    .ok_or_else(|| FenError::BadEnPassantSquare(square.to_string()))?,
            ),
        };

        // The move counters are optional: plenty of published test positions
        // omit them, and rejecting those would make the perft corpus harder to
        // extend than it needs to be.
        let halfmove_clock = parse_number(fields.get(4).copied().unwrap_or("0"))?;
        let fullmove_number = parse_number(fields.get(5).copied().unwrap_or("1"))?.max(1);

        let position = Position::from_parts(
            squares,
            side_to_move,
            castling,
            en_passant,
            halfmove_clock,
            fullmove_number,
        );

        if !position.is_structurally_valid() {
            return Err(FenError::Impossible);
        }
        Ok(position)
    }

    /// A position from a FEN as game files write them: the counters may be
    /// missing, an en passant square no pawn can take on is dropped, and then
    /// castling rights the board cannot back. Refused, beyond what
    /// [`Position::from_fen`] refuses: pieces other than chess's, more than
    /// sixteen men or eight pawns a side, three checkers, the side not to
    /// move in check, a halfmove clock over 100 and a move number of 0. That
    /// is a position a game could be in, so a file's start reads as it did
    /// before this crate read files.
    pub fn from_fen_lenient(fen: &str) -> Option<Position> {
        let fields: Vec<&str> = fen.split_whitespace().collect();
        let mut full = match fields.len() {
            0 => return None,
            1 => vec![fields[0], "w", "-", "-", "0", "1"],
            2 => vec![fields[0], fields[1], "-", "-", "0", "1"],
            3 => vec![fields[0], fields[1], fields[2], "-", "0", "1"],
            4 => vec![fields[0], fields[1], fields[2], fields[3], "0", "1"],
            5 => vec![fields[0], fields[1], fields[2], fields[3], fields[4], "1"],
            _ => fields[..6].to_vec(),
        };
        let counters_ok = full[4].parse::<u32>().is_ok_and(|n| n <= 100) && full[5].parse::<u32>().is_ok_and(|n| n > 0);
        if !counters_ok {
            return None;
        }
        let read = |f: &[&str]| Position::from_fen(&f.join(" ")).ok().filter(Position::could_be_in_a_game);
        if let Some(p) = read(&full) {
            return Some(p);
        }
        full[3] = "-";
        if let Some(p) = read(&full) {
            return Some(p);
        }
        full[2] = "-";
        read(&full)
    }

    /// What [`Position::from_fen_lenient`] asks beyond a valid FEN.
    fn could_be_in_a_game(&self) -> bool {
        let us = self.side_to_move();
        if Role::ALL.iter().any(|r| !Role::CHESS.contains(r) && !self.by_role(*r).is_empty()) {
            return false;
        }
        for color in Color::ALL {
            if self.by_color(color).count() > 16 || self.pieces(color, Role::Pawn).count() > 8 {
                return false;
            }
        }
        let king = self.king_square(us);
        let checkers = self.attackers_to(king, self.occupied()) & self.by_color(us.opponent());
        if checkers.count() > 2 {
            return false;
        }
        let Some(target) = self.en_passant_square() else { return true };
        // The pawn that went past it, and the square it came from.
        let (Some(pawn), Some(source)) = (target.offset(-us.forward()), target.offset(us.forward())) else { return false };
        self.piece_at(pawn) == Some(Piece::new(us.opponent(), Role::Pawn))
            && self.piece_at(target).is_none()
            && self.piece_at(source).is_none()
            // A check now came from that pawn, or through the square it left.
            && checkers.into_iter().all(|c| c == pawn || crate::attacks::between(c, king).contains(source))
    }

    /// The FEN with Shredder's castling field (`HAha`): each right by its
    /// rook's file. Unambiguous for any Chess960 array, where X-FEN's `K` and
    /// `Q` need the reader to find the outermost rook.
    pub fn to_shredder_fen(&self) -> String {
        let mut fields: Vec<String> = self.to_fen().split(' ').map(str::to_string).collect();
        if !self.castling_rights().is_empty() {
            let mut field = String::new();
            for color in Color::ALL {
                for side in Side::ALL {
                    if let Some(file) = self.castling_rights().rook_file(color, side) {
                        let letter = (b'A' + file) as char;
                        field.push(if color == Color::White { letter } else { letter.to_ascii_lowercase() });
                    }
                }
            }
            fields[2] = field;
        }
        fields.join(" ")
    }

    pub fn to_fen(&self) -> String {
        let mut fen = String::with_capacity(80);

        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                match self.piece_at(Square::from_file_rank(file, rank)) {
                    Some(piece) => {
                        if empty > 0 {
                            fen.push_str(&empty.to_string());
                            empty = 0;
                        }
                        fen.push(piece.fen_char());
                    }
                    None => empty += 1,
                }
            }
            if empty > 0 {
                fen.push_str(&empty.to_string());
            }
            if rank > 0 {
                fen.push('/');
            }
        }

        fen.push(' ');
        fen.push(match self.side_to_move() {
            Color::White => 'w',
            Color::Black => 'b',
        });
        fen.push(' ');
        fen.push_str(&write_castling_field(self));
        fen.push(' ');
        match self.en_passant_square() {
            Some(square) => fen.push_str(&square.to_string()),
            None => fen.push('-'),
        }
        fen.push_str(&format!(
            " {} {}",
            self.halfmove_clock(),
            self.fullmove_number()
        ));
        fen
    }
}

/// The back-rank files a colour's rooks stand on, and its king's file.
///
/// Both questions are asked of the *board*, which is why the castling field
/// cannot be read from the string alone any more: X-FEN's `K` means "the
/// outermost rook on that side of the king", and a Shredder letter's side
/// depends on which side of the king that file falls.
fn back_rank(squares: &[Option<Piece>; 64], color: Color) -> (Option<u8>, Vec<u8>) {
    let mut king = None;
    let mut rooks = Vec::new();
    for file in 0..8u8 {
        let square = Square::from_file_rank(file, color.back_rank());
        match squares[square.index() as usize] {
            Some(piece) if piece.color == color && piece.role == Role::King => king = Some(file),
            Some(piece) if piece.color == color && piece.role == Role::Rook => rooks.push(file),
            _ => {}
        }
    }
    (king, rooks)
}

/// Reads a castling field in either dialect.
///
/// One parser for both, because their letter sets are disjoint: `KQkq` is
/// X-FEN, anything else in `A..H` is Shredder. No flag is needed to tell them
/// apart, and both are needed — every GUI emits X-FEN, and the published
/// Chess960 perft corpus is distributed in Shredder.
fn parse_castling_field(
    field: &str,
    squares: &[Option<Piece>; 64],
) -> Result<CastlingRights, CastlingFieldError> {
    if field == "-" {
        return Ok(CastlingRights::NONE);
    }
    if field.is_empty() {
        // "no rights" is spelled `-`. An empty field is a FEN with a field
        // missing, and letting it through would silently accept one.
        return Err(CastlingFieldError::NotACastlingLetter(' '));
    }

    let mut rights = CastlingRights::NONE;
    for letter in field.chars() {
        let color = if letter.is_ascii_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        let (king, rooks) = back_rank(squares, color);
        let king = king.ok_or(CastlingFieldError::NoKing(color))?;

        let (side, file) = match letter.to_ascii_uppercase() {
            'K' => (
                Side::Kingside,
                *rooks
                    .iter()
                    .filter(|&&file| file > king)
                    .max()
                    .ok_or(CastlingFieldError::NoOutermostRook(color, Side::Kingside))?,
            ),
            'Q' => (
                Side::Queenside,
                *rooks
                    .iter()
                    .filter(|&&file| file < king)
                    .min()
                    .ok_or(CastlingFieldError::NoOutermostRook(color, Side::Queenside))?,
            ),
            found @ 'A'..='H' => {
                let file = found as u8 - b'A';
                if !rooks.contains(&file) {
                    return Err(CastlingFieldError::NoRookOnFile(color, file));
                }
                let side = if file > king {
                    Side::Kingside
                } else if file < king {
                    Side::Queenside
                } else {
                    return Err(CastlingFieldError::WrongSideOfTheKing(color, file));
                };
                (side, file)
            }
            other => return Err(CastlingFieldError::NotACastlingLetter(other)),
        };

        if rights.has(color, side) {
            return Err(CastlingFieldError::Repeated(color, side));
        }
        rights = rights.with(color, side, file);
    }
    Ok(rights)
}

/// Writes the castling field in **X-FEN**.
///
/// X-FEN rather than Shredder, and for compatibility rather than taste: X-FEN
/// spells a right `K` or `Q` when its rook is the outermost one on that side of
/// the king, and falls back to a file letter only when an inner rook holds it.
/// Standard chess's rooks are on the a- and h-files, where nothing can be
/// further out — so **every standard position still writes `KQkq`**, which is
/// what every stored FEN, the published standard perft corpus and the
/// round-trip test all depend on. Shredder would spell the ordinary starting
/// position `AHah` and break all three for nothing.
fn write_castling_field(position: &Position) -> String {
    if position.castling_rights().is_empty() {
        return "-".to_string();
    }
    let mut text = String::new();
    for color in Color::ALL {
        for side in Side::ALL {
            let Some(file) = position.castling_rights().rook_file(color, side) else {
                continue;
            };
            let king = position.king_square(color).file();
            let rooks: Vec<u8> = position
                .pieces(color, Role::Rook)
                .into_iter()
                .filter(|square| square.rank() == color.back_rank())
                .map(|square| square.file())
                .collect();
            let outermost = match side {
                Side::Kingside => !rooks.iter().any(|&other| other > file && other > king),
                Side::Queenside => !rooks.iter().any(|&other| other < file && other < king),
            };
            let letter = if outermost {
                match side {
                    Side::Kingside => 'K',
                    Side::Queenside => 'Q',
                }
            } else {
                (b'A' + file) as char
            };
            text.push(if color == Color::White {
                letter
            } else {
                letter.to_ascii_lowercase()
            });
        }
    }
    text
}

fn parse_board(field: &str) -> Result<[Option<Piece>; 64], FenError> {
    let ranks: Vec<&str> = field.split('/').collect();
    if ranks.len() != 8 {
        return Err(FenError::WrongRankCount(ranks.len()));
    }

    let mut squares = [None; 64];
    // FEN writes rank 8 first, so the index counts down from the top.
    for (index, text) in ranks.iter().enumerate() {
        let rank = 7 - index as u8;
        let mut file = 0u8;
        for letter in text.chars() {
            if let Some(skip) = letter.to_digit(10) {
                if skip == 0 {
                    return Err(FenError::UnknownPiece(letter));
                }
                file += skip as u8;
            } else {
                let piece = Piece::from_fen_char(letter).ok_or(FenError::UnknownPiece(letter))?;
                if file >= 8 {
                    return Err(FenError::WrongRankWidth {
                        rank: index,
                        width: file as usize + 1,
                    });
                }
                squares[Square::from_file_rank(file, rank).index() as usize] = Some(piece);
                file += 1;
            }
        }
        if file != 8 {
            return Err(FenError::WrongRankWidth {
                rank: index,
                width: file as usize,
            });
        }
    }
    Ok(squares)
}

fn parse_number(field: &str) -> Result<u32, FenError> {
    field
        .parse()
        .map_err(|_| FenError::BadMoveNumber(field.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_starting_position_round_trips() {
        let position = Position::from_fen(STARTING_POSITION).unwrap();
        assert_eq!(position.to_fen(), STARTING_POSITION);
    }

    #[test]
    fn every_position_reachable_in_a_few_plies_round_trips() {
        // Exercises the awkward fields — en passant appearing and vanishing,
        // rights being lost, counters advancing — without hand-writing them.
        fn walk(position: &Position, depth: u32) {
            let fen = position.to_fen();
            assert_eq!(
                Position::from_fen(&fen).map(|p| p.to_fen()),
                Ok(fen.clone()),
                "did not survive the round trip"
            );
            if depth > 0 {
                for mv in position.legal_moves() {
                    walk(&position.make_move(mv), depth - 1);
                }
            }
        }
        walk(&Position::starting(), 3);
    }

    #[test]
    fn counters_may_be_omitted() {
        let position = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - -").unwrap();
        assert_eq!(position.halfmove_clock(), 0);
        assert_eq!(position.fullmove_number(), 1);
    }

    #[test]
    fn malformed_input_is_an_error_not_a_panic() {
        let cases: &[(&str, FenError)] = &[
            ("", FenError::WrongFieldCount(0)),
            ("4k3/8/8/8/8/8/8/4K3 w", FenError::WrongFieldCount(2)),
            ("4k3/8/8/8/8/8/4K3 w - - 0 1", FenError::WrongRankCount(7)),
            (
                "4k3/8/8/8/8/8/8/4K4 w - - 0 1",
                FenError::WrongRankWidth { rank: 7, width: 9 },
            ),
            (
                "4k3/8/8/8/8/8/8/4K2 w - - 0 1",
                FenError::WrongRankWidth { rank: 7, width: 7 },
            ),
            (
                "4k3/8/8/8/8/8/8/4Kx2 w - - 0 1",
                FenError::UnknownPiece('x'),
            ),
            (
                "4k3/8/8/8/8/8/8/4K3 x - - 0 1",
                FenError::BadSideToMove("x".to_string()),
            ),
            (
                "4k3/8/8/8/8/8/8/4K3 w XY - 0 1",
                FenError::BadCastlingRights("'X' is not a castling letter".to_string()),
            ),
            (
                "4k3/8/8/8/8/8/8/4K3 w - e4 0 1",
                FenError::BadEnPassantSquare("e4".to_string()),
            ),
            (
                "4k3/8/8/8/8/8/8/4K3 w - - x 1",
                FenError::BadMoveNumber("x".to_string()),
            ),
            ("8/8/8/8/8/8/8/4K3 w - - 0 1", FenError::Impossible),
        ];
        for (fen, expected) in cases {
            assert_eq!(
                Position::from_fen(fen).err().as_ref(),
                Some(expected),
                "for {fen:?}"
            );
        }

        // The castling field says which letter it choked on rather than
        // echoing the field, because there are now several ways a field can be
        // wrong and "HAha" tells you nothing about which.
        for (fen, reason) in [
            (
                "4k3/8/8/8/8/8/8/4K3 w XY - 0 1",
                "'X' is not a castling letter",
            ),
            ("4k3/8/8/8/8/8/8/4K3 w KQ - 0 1", "no outermost White rook"),
            (
                "r3k2r/8/8/8/8/8/8/R3K2R w KKkq - 0 1",
                "White Kingside is named twice",
            ),
            (
                "r3k2r/8/8/8/8/8/8/R3K2R w Bkq - 0 1",
                "no White rook on file b",
            ),
        ] {
            let problem = Position::from_fen(fen).unwrap_err();
            assert!(
                format!("{problem}").contains(reason),
                "for {fen:?}: {problem} does not mention {reason:?}"
            );
        }
    }

    #[test]
    fn an_en_passant_square_must_be_on_the_third_or_sixth_rank() {
        assert!(Position::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").is_ok());
        assert!(Position::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d5 0 1").is_err());
    }

    #[test]
    fn a_lenient_read_drops_what_the_board_cannot_back() {
        let read = |fen: &str| Position::from_fen_lenient(fen).map(|p| p.to_fen());
        // Counters missing.
        assert_eq!(read("4k3/8/8/3pP3/8/8/8/4K3 w - d6").as_deref(), Some("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1"));
        assert_eq!(read("4k3/8/8/8/8/8/8/4K3").as_deref(), Some("4k3/8/8/8/8/8/8/4K3 w - - 0 1"));
        // An en passant square with no pawn past it goes; then rights with no rook.
        assert_eq!(read("r3k3/8/8/8/8/8/8/4K2R w Kq e6 0 1").as_deref(), Some("r3k3/8/8/8/8/8/8/4K2R w Kq - 0 1"));
        assert_eq!(read("r3k3/8/8/8/8/8/8/4K2R w KQkq e6 0 1").as_deref(), Some("r3k3/8/8/8/8/8/8/4K2R w - - 0 1"));
        // Positions no game could be in.
        for fen in [
            "4k3/8/8/8/8/8/8/r3K3 b - - 0 1",
            "4k3/8/8/8/8/8/8/4K3 w - - 101 1",
            "4k3/8/8/8/8/8/8/4K3 w - - 0 0",
            "4k3/8/8/8/8/8/8/4KF2 w - - 0 1",
            "4k3/PPPPPPPP/P7/8/8/8/8/4K3 w - - 0 1",
            "no position",
            "",
        ] {
            assert_eq!(read(fen), None, "{fen}");
        }
    }

    #[test]
    fn shredder_names_each_right_by_its_file() {
        let p = Position::from_fen("1r1kr3/8/8/8/8/8/8/1R1KR3 w KQkq - 0 1").unwrap();
        assert_eq!(p.to_shredder_fen(), "1r1kr3/8/8/8/8/8/8/1R1KR3 w EBeb - 0 1");
        assert_eq!(Position::starting().to_shredder_fen(), "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w HAha - 0 1");
        let none = "4k3/8/8/8/8/8/8/4K3 w - - 0 1";
        assert_eq!(Position::from_fen(none).unwrap().to_shredder_fen(), none);
    }
}
