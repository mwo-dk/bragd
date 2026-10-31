//! Standard Algebraic Notation — `Nf3`, not `g1f3`.
//!
//! Three things need this, so it is not cosmetic: the move list a Player reads,
//! the algebraic input that makes the board usable without a mouse or a screen,
//! and PGN export from a Library.
//!
//! The wire format stays *long* algebraic regardless. SAN is written for people
//! and means nothing without a position to read it against, so it is resolved
//! where a position is already known and the result travels unambiguously. The
//! server sends the notation for each legal move alongside its long form, which
//! lets a Player type `Nf3` and a client turn it into `g1f3` without holding a
//! rules engine of its own.
//!
//! The fiddly part is **disambiguation**, and the rule is easy to get subtly
//! wrong: it is decided by the *legal* moves, not the pseudo-legal ones. If two
//! knights could reach a square but one of them is pinned, there is nothing to
//! disambiguate and writing `Nbd2` would be wrong.
//!
//! **Which legal moves is a question about the variant, not about the board**,
//! and everything here takes one for that reason. This module asked
//! `Position::legal_moves` — *chess's* generator — for all three of the things
//! it needs it for, and each was wrong in its own way for a variant:
//!
//! - The debug assertion that the move is legal fired on every ferz, alfil and
//!   gaja move there is, because chess's generator does not walk those pieces.
//!   That panicked the whole module in a debug build and was compiled out of
//!   a release build, so it was invisible where it mattered and
//!   fatal where it did not.
//! - Disambiguation could not see a rival it had not generated, so two ferzes
//!   bearing on one square — reachable in shatranj, where every promotion makes
//!   another ferz — both wrote `Fd4` and a Library held two different games
//!   spelled identically.
//! - `+` and `#` are told apart by whether the side to move has a reply, and
//!   chess's generator finds none for a shatranj position whose only moves are
//!   an alfil's. A check would have been written as mate.
//!
//! **This is not a variant parameter through move generation**, and the
//! difference matters. What is refused there is threading a variant
//! through *move generation* — the hottest path in the engine and the one the
//! perft gate measures — so that a branch would be live in every position of
//! every variant. Nothing here is on that path: this runs once per move a
//! person reads, it calls the variant's generator rather than becoming one, and
//! the alternative is not "roles instead of a parameter" but "notation that is
//! wrong for four of the seven variants".


use crate::chess_move::{Move, MoveKind, MoveList};
use crate::piece::Role;
use crate::position::Position;
use crate::variant::Variant;

/// Writes a move as a person would.
///
/// # Panics
/// If the move is not legal in this position under these rules. SAN is defined
/// relative to a position — "which knight" has no answer otherwise — so there
/// is nothing sensible to return.
pub fn to_san<V: Variant + ?Sized>(variant: &V, position: &Position, mv: Move) -> String {
    let legal = variant.legal_moves(position);
    san_given(position, mv, &legal, |after| variant.legal_moves(after).is_empty())
}

/// [`to_san`] when the position's legal moves (`legal`, the variant's) are
/// known, and `no_reply` says whether the position after the move has none. A
/// [`crate::Game`] knows both from the moves it generates once a ply.
pub(crate) fn san_given(
    position: &Position,
    mv: Move,
    legal: &MoveList,
    no_reply: impl FnOnce(&Position) -> bool,
) -> String {
    // **Asked of the variant, which is what makes it true for every one of
    // them.** It used to be asked of `Position::is_legal` and of the move with
    // its gate stripped off, because chess's generator has never heard of
    // gating and a perfectly legal `b1c3h` is not in the list it checks
    // against. Stripping the gate answered that one variant and left shatranj
    // and chaturanga panicking on every move their own pieces make.
    //
    // Note what this is *not*: an assertion weakened until it passes. It is
    // stronger than the one it replaces — it now checks the gate as well, which
    // `without_gate` explicitly did not.
    debug_assert!(legal.contains(mv), "SAN is only defined for a legal move");

    let mut san = match mv.kind() {
        MoveKind::KingsideCastle => "O-O".to_string(),
        MoveKind::QueensideCastle => "O-O-O".to_string(),
        _ => body(legal, position, mv),
    };
    san.push_str(&gate(position, mv));

    // Check and mate are part of the notation, and deciding between them needs
    // the position the move produces.
    //
    // `Position::make_move` rather than `Variant::make_move`: every variant's
    // is an assertion of legality followed by exactly this call, and the
    // assertion is the one already made above. Going through the variant would
    // generate the move list a second time on every move written.
    let after = position.make_move(mv);
    if after.is_check() {
        san.push(if no_reply(&after) { '#' } else { '+' });
    }
    san
}

/// What a move brings in behind it, if anything: `/H`, or `/He1` on a castle.
///
/// **The published form.** XBoard's rules document for the variant says a gating
/// move is entered "with a suffix `/H` or `/E`", and Bragd's own board has
/// spelled the piece `H` and `E` since it shipped. The letter is the piece and
/// nothing else — the wire form's uppercase-means-the-rook's-square trick is
/// not reused here, because SAN is read by people and a reader should not have
/// to know that `O-O/E` and `O-O/e` are different moves.
///
/// So a castle **names its square instead**, which is the only move that needs
/// to: it vacates two, and either may be gated. Every other move vacates the
/// square it started on and the notation already says which that is.
fn gate(position: &Position, mv: Move) -> String {
    let Some(role) = mv.gate() else {
        return String::new();
    };
    let letter = role.letter().expect("the gated pieces both have letters");
    if mv.kind().is_castle() {
        let square = position
            .gate_square(mv)
            .expect("a castle that gates names the square it gates onto");
        format!("/{letter}{square}")
    } else {
        format!("/{letter}")
    }
}

fn body(legal: &MoveList, position: &Position, mv: Move) -> String {
    let piece = position
        .piece_at(mv.from())
        .expect("a legal move starts on an occupied square");
    let mut san = String::new();

    if piece.role == Role::Pawn {
        // A pawn names its origin file only when it captures: `exd5`, but `e4`.
        if mv.is_capture() {
            san.push((b'a' + mv.from().file()) as char);
            san.push('x');
        }
        san.push_str(&mv.to().to_string());
    } else {
        san.push(piece.role.letter().expect("only a pawn has no letter"));
        san.push_str(&disambiguation(legal, position, mv));
        if mv.is_capture() {
            san.push('x');
        }
        san.push_str(&mv.to().to_string());
    }

    if let Some(role) = mv.promotion_role() {
        san.push('=');
        san.push(role.letter().expect("a pawn cannot promote to a pawn"));
    }
    san
}

/// The least that distinguishes this move from another of the same piece to the
/// same square: nothing, a file, a rank, or both.
fn disambiguation(legal: &MoveList, position: &Position, mv: Move) -> String {
    let piece = position.piece_at(mv.from()).expect("occupied");

    let rivals: Vec<Move> = legal
        .into_iter()
        .filter(|other| {
            other.to() == mv.to()
                && other.from() != mv.from()
                && position.piece_at(other.from()).map(|p| p.role) == Some(piece.role)
        })
        .collect();

    if rivals.is_empty() {
        return String::new();
    }

    let file = (b'a' + mv.from().file()) as char;
    let rank = (b'1' + mv.from().rank()) as char;

    // File first, because that is what the convention prefers when it settles
    // the matter on its own.
    if rivals
        .iter()
        .all(|other| other.from().file() != mv.from().file())
    {
        return file.to_string();
    }
    if rivals
        .iter()
        .all(|other| other.from().rank() != mv.from().rank())
    {
        return rank.to_string();
    }
    format!("{file}{rank}")
}

/// Reads a move written the way a person writes it.
///
/// Implemented by generating the legal moves and comparing — which is slower
/// than parsing and cannot be wrong in a way `to_san` is not also wrong. The
/// two therefore agree by construction, which matters more here than speed: a
/// Player typing a move gets one attempt per move, not thousands per second.
///
/// Forgiving about decoration a person might add or leave off — check marks,
/// `!?`, `e.p.`, `0-0` written with zeroes — and unforgiving about anything
/// that would change which move it is.
pub fn from_san<V: Variant + ?Sized>(variant: &V, position: &Position, text: &str) -> Option<Move> {
    let wanted = normalise(text);
    if wanted.is_empty() {
        return None;
    }

    let mut found = None;
    for mv in variant.legal_moves(position) {
        if normalise(&to_san(variant, position, mv)) == wanted {
            // Two legal moves cannot share a SAN — that is what disambiguation
            // is for — so a second match means `to_san` has a bug.
            debug_assert!(found.is_none(), "two moves share the notation {wanted}");
            found = Some(mv);
        }
    }
    found
}

/// Strips what does not change which move is meant.
fn normalise(text: &str) -> String {
    let mut out = String::new();
    for character in text.trim().chars() {
        match character {
            // Zeroes for castling are common and unambiguous.
            '0' => out.push('O'),
            'o' => out.push('O'),
            // **The two separators are one separator here.** `to_san` writes
            // `=` for a promotion and `/` for a gate, which is what WinBoard
            // writes; but WinBoard *accepts* both for both, and this side is
            // the forgiving one. So `Nf3=H` and `a8/Q` are read, not refused.
            //
            // Safe against collision, which is the only thing that could make
            // it wrong: a gate is only ever written after a piece letter or a
            // castle, because a gating right lives on a home-rank square and no
            // pawn ever stands on one. So no pawn move carries a gate, no piece
            // move carries a promotion, and folding the two characters together
            // cannot bring two legal moves onto the same string.
            '/' => out.push('='),
            // Check, mate, annotations, en passant marks and the capture symbol
            // some people use.
            '+' | '#' | '!' | '?' | ' ' | '-' if character == '-' && out.starts_with('O') => {
                out.push('-')
            }
            '+' | '#' | '!' | '?' | ' ' => {}
            _ => out.push(character),
        }
    }
    out.replace("e.p.", "").replace("ep", "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess_move::CastlingNotation;
    use crate::variant::{Chaturanga, Chess, Seirawan, Shatranj};

    fn position(fen: &str) -> Position {
        Position::from_fen(fen).expect("test FEN should parse")
    }

    /// The SAN for a move given in long algebraic, so tests can name a move
    /// unambiguously and assert on how it is written.
    fn san(fen: &str, uci: &str) -> String {
        san_in(&Chess, fen, uci)
    }

    /// The same, under rules that are not chess's.
    fn san_in<V: Variant>(variant: &V, fen: &str, uci: &str) -> String {
        let position = position(fen);
        let mv = variant
            .legal_moves(&position)
            .into_iter()
            .find(|mv| mv.to_uci(CastlingNotation::KingTakesRook) == uci)
            .unwrap_or_else(|| panic!("{uci} is not legal in {fen}"));
        to_san(variant, &position, mv)
    }

    const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn pawns_are_written_as_their_destination() {
        assert_eq!(san(START, "e2e4"), "e4");
        assert_eq!(san(START, "a2a3"), "a3");
    }

    #[test]
    fn pieces_carry_their_letter() {
        assert_eq!(san(START, "g1f3"), "Nf3");
        assert_eq!(san(START, "b1c3"), "Nc3");
    }

    #[test]
    fn captures_are_marked_and_pawns_name_their_file() {
        let fen = "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";
        assert_eq!(san(fen, "e4d5"), "exd5");
        let fen = "rnbqkbnr/ppp1pppp/8/3p4/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2";
        assert_eq!(san(fen, "d5e4"), "dxe4");
    }

    #[test]
    fn castling_is_written_with_letters() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        assert_eq!(san(fen, "e1h1"), "O-O");
        assert_eq!(san(fen, "e1a1"), "O-O-O");
    }

    #[test]
    fn promotion_names_the_piece() {
        // A queen landing on a8 checks a king on e8 along the rank; a knight
        // does not. Both belong in the notation.
        let fen = "4k3/P7/8/8/8/8/8/4K3 w - - 0 1";
        assert_eq!(san(fen, "a7a8q"), "a8=Q+");
        assert_eq!(san(fen, "a7a8n"), "a8=N");

        // And with the king off the eighth rank, the a-file and the long
        // diagonal, a promotion is just a promotion.
        let quiet = "8/P7/7k/8/8/8/8/4K3 w - - 0 1";
        assert_eq!(san(quiet, "a7a8q"), "a8=Q");
        let fen = "1n2k3/P7/8/8/8/8/8/4K3 w - - 0 1";
        assert_eq!(san(fen, "a7b8q"), "axb8=Q+");
    }

    #[test]
    fn check_and_mate_are_part_of_the_notation() {
        // Scholar's mate.
        let fen = "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5Q2/PPPP1PPP/RNB1K1NR w KQkq - 4 4";
        assert_eq!(san(fen, "f3f7"), "Qxf7#");

        let fen = "4k3/8/8/8/8/8/8/4K2R w K - 0 1";
        assert_eq!(san(fen, "h1h8"), "Rh8+");
    }

    #[test]
    fn two_pieces_reaching_one_square_are_told_apart_by_file() {
        // Knights on b1 and f3 both reach d2.
        let fen = "4k3/8/8/8/8/5N2/8/1N2K3 w - - 0 1";
        assert_eq!(san(fen, "b1d2"), "Nbd2");
        assert_eq!(san(fen, "f3d2"), "Nfd2");
    }

    #[test]
    fn by_rank_when_the_file_does_not_settle_it() {
        // Rooks on a1 and a5 both reach a3, sharing a file.
        let fen = "4k3/8/8/R7/8/8/8/R3K3 w - - 0 1";
        assert_eq!(san(fen, "a1a3"), "R1a3");
        assert_eq!(san(fen, "a5a3"), "R5a3");
    }

    #[test]
    fn by_both_when_neither_alone_settles_it() {
        // Three queens bearing on d5, from a8, d8 and a5. The a8 queen shares
        // a file with one rival and a rank with the other, so neither alone
        // tells them apart and it needs both.
        let fen = "Q2Q4/8/7k/Q7/8/8/8/4K3 w - - 0 1";
        assert_eq!(san(fen, "a8d5"), "Qa8d5");
        assert_eq!(san(fen, "d8d5"), "Qdd5");
        assert_eq!(san(fen, "a5d5"), "Q5d5");
    }

    #[test]
    fn a_pinned_rival_is_not_a_rival() {
        // This is the rule people get wrong. The knight on d2 is pinned against
        // the king by the rook on d8, so only one knight can legally reach f3
        // and there is nothing to disambiguate.
        let fen = "3rk3/8/8/8/8/8/3N4/3K2N1 w - - 0 1";
        let position = position(fen);
        let reaching: Vec<String> = position
            .legal_moves()
            .into_iter()
            .filter(|mv| mv.to().to_string() == "f3")
            .map(|mv| to_san(&Chess, &position, mv))
            .collect();
        assert_eq!(reaching, ["Nf3"], "a pinned knight cannot go there at all");
    }

    #[test]
    fn en_passant_is_written_like_any_other_pawn_capture() {
        let fen = "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1";
        assert_eq!(san(fen, "e5d6"), "exd6");
    }

    #[test]
    fn everything_written_can_be_read_back() {
        // The property that matters: the two are inverses on every legal move
        // of every position a few plies deep.
        //
        // Run for **every variant that has its own pieces or its own move
        // categories**, because that is where the two can come apart: chess's
        // version passed throughout the whole time shatranj's notation was
        // ambiguous and Seirawan's was silent about the gate.
        fn walk<V: Variant>(variant: &V, position: &Position, depth: u32) {
            for mv in variant.legal_moves(position) {
                let written = to_san(variant, position, mv);
                assert_eq!(
                    from_san(variant, position, &written),
                    Some(mv),
                    "{written} did not read back in {}",
                    position.to_fen()
                );
                if depth > 0 {
                    walk(variant, &position.make_move(mv), depth - 1);
                }
            }
        }
        // Shallow on purpose: reading a move writes every legal move to find
        // it, so this is quadratic in the branching factor. Depth two from the
        // opening took a minute, which is not a price worth paying on every
        // `cargo test`. Kiwipete carries the awkward moves anyway.
        walk(&Chess, &Position::starting(), 1);
        walk(
            &Chess,
            &position("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"),
            0,
        );
        walk(&Shatranj, &Shatranj.starting_position(), 1);
        walk(&Chaturanga, &Chaturanga.starting_position(), 1);
        // From a position where every category is on offer at once: a castle
        // that can gate either of two squares, two pieces still in hand, and a
        // pawn one step from six different promotions.
        walk(&Seirawan, &Seirawan.starting_position(), 1);
        walk(&Seirawan, &gating_and_promoting(), 0);
    }

    /// White may castle and gate on either vacated square, and the b7 pawn
    /// promotes six ways onto each of three squares.
    ///
    /// **b8 is empty and c8 holds the knight, and that is the whole point of
    /// the board.** The obvious spelling — a knight on b8 — blocks the push and
    /// leaves nothing on c8 to capture, so b7 has exactly one destination and
    /// every promotion the crate writes in SAN is a capture. It was that FEN
    /// for a while, and the consequence was silent: nothing asserted that a
    /// *pushed* Seirawan promotion is written `b8=H`, because the walk below
    /// never generated one. This is the board `tests/seirawan.rs::crowded()`
    /// uses, for the same reason.
    ///
    /// The gate squares are named rather than taken wholesale, because f1 and
    /// g1 are empty here and `with_seirawan_hands` refuses a right on a square
    /// that cannot carry one.
    fn gating_and_promoting() -> Position {
        position("r1nqkbnr/pPpppppp/8/8/8/5N2/PPPPPPPP/RNBQK2R w KQkq - 0 1")
            .with_seirawan_hands(squares(&["a1", "b1", "c1", "d1", "e1", "h1"]))
    }

    fn squares(names: &[&str]) -> crate::bitboard::Bitboard {
        names
            .iter()
            .fold(crate::bitboard::Bitboard::EMPTY, |set, name| {
                set | crate::bitboard::Bitboard::from_square(
                    crate::square::Square::parse(name).expect("a square"),
                )
            })
    }

    /// **Three moves that used to spell the same thing.** The move list, a
    /// Library and a Player typing a move all went through this string, so a
    /// hawk arriving was invisible in every one of them.
    #[test]
    fn a_gate_is_written_and_a_castle_names_its_square() {
        let start = Seirawan.starting_position();
        let written: Vec<String> = Seirawan
            .legal_moves(&start)
            .into_iter()
            .filter(|mv| mv.from().to_string() == "g1")
            .map(|mv| to_san(&Seirawan, &start, mv))
            .collect();
        assert_eq!(written, ["Nf3", "Nf3/H", "Nf3/E", "Nh3", "Nh3/H", "Nh3/E"]);

        // A castle vacates two squares and either may take the piece, so this
        // is the one move whose gate has to name where it lands.
        let castling = gating_and_promoting();
        let castles: Vec<String> = Seirawan
            .legal_moves(&castling)
            .into_iter()
            .filter(|mv| mv.kind().is_castle())
            .map(|mv| to_san(&Seirawan, &castling, mv))
            .collect();
        assert_eq!(castles, ["O-O", "O-O/He1", "O-O/Ee1", "O-O/Hh1", "O-O/Eh1"]);
    }

    /// A pawn promotes six ways here, and the notation has to tell all six
    /// apart — `a8=H` is not `a8=Q` and a Library has to hold the difference.
    #[test]
    fn seirawan_promotions_name_all_six_pieces() {
        let position = gating_and_promoting();
        let written: Vec<String> = Seirawan
            .legal_moves(&position)
            .into_iter()
            .filter(|mv| mv.from().to_string() == "b7" && mv.to().to_string() == "a8")
            .map(|mv| to_san(&Seirawan, &position, mv))
            .collect();
        assert_eq!(
            written,
            ["bxa8=Q", "bxa8=R", "bxa8=B", "bxa8=N", "bxa8=H", "bxa8=E"],
            "the two extra pieces come after chess's four, not among them"
        );
    }

    /// **A pushed promotion, not a capture**, which for a while nothing here
    /// asserted: the fixture's b8 was blocked, so every promotion the tests saw
    /// arrived with an `x` in it and the plain spelling was never exercised.
    #[test]
    fn a_seirawan_promotion_that_captures_nothing_is_written_as_its_square() {
        let position = gating_and_promoting();
        let written: Vec<String> = Seirawan
            .legal_moves(&position)
            .into_iter()
            .filter(|mv| mv.from().to_string() == "b7" && mv.to().to_string() == "b8")
            .map(|mv| to_san(&Seirawan, &position, mv))
            .collect();
        assert_eq!(
            written,
            ["b8=Q", "b8=R", "b8=B", "b8=N", "b8=H", "b8=E"],
            "no file letter and no x when nothing is taken"
        );
    }

    /// **The elephant checks, and the check mark is the visible proof.** While
    /// `attackers_to` did not know the compounds, this same board wrote `b8=E`
    /// — a rook's worth of check recorded in a Library as a quiet move — and
    /// `b8=H` for a hawk that equally covers the king's flight squares. The
    /// notation is downstream of check detection, so it is a fair place to
    /// assert that detection happens.
    #[test]
    fn a_promoted_compound_gives_check_and_the_notation_says_so() {
        let board = position("4k3/P7/8/8/8/8/8/4K3 w - - 0 1")
            .with_seirawan_hands(crate::bitboard::Bitboard::EMPTY);
        let written: Vec<String> = Seirawan
            .legal_moves(&board)
            .into_iter()
            .filter(|mv| mv.from().to_string() == "a7")
            .map(|mv| to_san(&Seirawan, &board, mv))
            .collect();
        assert_eq!(
            written,
            ["a8=Q+", "a8=R+", "a8=B", "a8=N", "a8=H", "a8=E+"],
            "the elephant is a rook and checks along the eighth"
        );
    }

    /// **The bug that panicked the module.** A ferz move is not in chess's
    /// generator, so writing one asserted against a list it was never in — and
    /// `state` on a live shatranj game took the app down in every debug
    /// build. Chaturanga's gaja is the same story with a different table.
    #[test]
    fn a_variants_own_pieces_can_be_written_at_all() {
        assert_eq!(
            san_in(&Shatranj, crate::shatranj::STARTING_POSITION, "c1a3"),
            "Ea3"
        );
        assert_eq!(
            san_in(&Shatranj, "4k3/8/8/8/3F4/8/8/4K3 w - - 0 1", "d4c5"),
            "Fc5"
        );
        assert_eq!(
            san_in(&Shatranj, "4k3/1P6/8/8/8/8/8/4K3 w - - 0 1", "b7b8f"),
            "b8=F"
        );
        assert_eq!(
            san_in(&Chaturanga, "4k3/8/8/8/3G4/8/8/4K3 w - - 0 1", "d4d5"),
            "Gd5"
        );
    }

    /// **Disambiguation could not see a rival it had not generated.** Shatranj
    /// promotes every pawn to a ferz, so two of them bearing on one square is
    /// an ordinary middlegame position — and both moves wrote `Fd4`, which is
    /// two different games spelled identically in a Library.
    #[test]
    fn two_of_a_variants_own_pieces_are_told_apart() {
        let fen = "4k3/8/8/8/8/2F1F3/8/4K3 w - - 0 1";
        assert_eq!(san_in(&Shatranj, fen, "c3d4"), "Fcd4");
        assert_eq!(san_in(&Shatranj, fen, "e3d4"), "Fed4");
    }

    /// **Check and mate are told apart by the replies**, and asking chess for
    /// them in a shatranj position finds none: every move here is an alfil's.
    /// This is a check written as mate, which is a decided game on the board.
    #[test]
    fn a_reply_only_the_variant_can_generate_is_still_a_reply() {
        // Black's king on a8 is checked by the rook arriving on h8, and the
        // White king on b6 covers both squares it could run to. The **alfil on
        // d6** saves it, by leaping to b8 or f8 and interposing on the rank —
        // and an alfil is a piece chess's generator does not walk, so chess
        // finds no reply at all and would write this as mate.
        let fen = "k7/8/1K1e4/8/8/8/8/7R w - - 0 1";
        assert_eq!(san_in(&Shatranj, fen, "h1h8"), "Rh8+");
        // The same position with the alfil gone really is mate, which is what
        // says the `+` above is about the replies and not about the shape.
        assert_eq!(
            san_in(&Shatranj, "k7/8/1K6/8/8/8/8/7R w - - 0 1", "h1h8"),
            "Rh8#"
        );
    }

    #[test]
    fn reading_forgives_decoration_but_not_ambiguity() {
        let rook = position("4k3/8/8/8/8/8/8/4K2R w K - 0 1");
        // With and without the check mark, and with junk a person might add.
        for written in ["Rh8+", "Rh8", "Rh8!", "Rh8+?!"] {
            assert!(from_san(&Chess, &rook, written).is_some(), "{written}");
        }
        // Castling with zeroes, as plenty of people write it.
        let castling = position("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
        assert_eq!(
            from_san(&Chess, &castling, "0-0"),
            from_san(&Chess, &castling, "O-O"),
            "zeroes and letters mean the same thing"
        );
    }

    /// **The reference implementation accepts both separators and this refused
    /// one of them.** WinBoard writes `=` with promotions and `/` with gatings
    /// — which is exactly what `to_san` does — but reads either for either, and
    /// a Player copying a move out of a WinBoard game was told a legal move was
    /// not one. Decoration that does not change which move is meant is the
    /// thing this function promises to forgive.
    #[test]
    fn a_gate_and_a_promotion_may_be_typed_with_either_separator() {
        let start = Seirawan.starting_position();
        assert_eq!(
            from_san(&Seirawan, &start, "Nf3=H"),
            from_san(&Seirawan, &start, "Nf3/H"),
            "a gate written the promotion way"
        );
        assert!(from_san(&Seirawan, &start, "Nf3=H").is_some());

        let promoting = gating_and_promoting();
        assert_eq!(
            from_san(&Seirawan, &promoting, "bxa8/H"),
            from_san(&Seirawan, &promoting, "bxa8=H"),
            "a promotion written the gate way"
        );
        assert!(from_san(&Seirawan, &promoting, "bxa8/H").is_some());

        // The castle carries a gate too, and it is the one spelling where the
        // separator sits between two things that both look like notation.
        assert_eq!(
            from_san(&Seirawan, &promoting, "O-O=He1"),
            from_san(&Seirawan, &promoting, "O-O/He1"),
            "a gating castle"
        );
        assert!(from_san(&Seirawan, &promoting, "O-O=He1").is_some());
    }

    #[test]
    fn reading_refuses_what_is_not_a_move_here() {
        let start = Position::starting();
        for written in ["", "  ", "Nf6", "Qh5", "e5", "O-O", "xyz", "e4e5"] {
            assert_eq!(
                from_san(&Chess, &start, written),
                None,
                "accepted {written:?}"
            );
        }
    }

    /// **The point of writing the gate at all**: a Player typing `Nf3` in a
    /// Seirawan game gets the plain move, and the two that bring a piece in are
    /// reachable by typing rather than only by pressing.
    #[test]
    fn a_typed_seirawan_move_names_exactly_one_of_the_three() {
        let start = Seirawan.starting_position();
        let plain = from_san(&Seirawan, &start, "Nf3").expect("the plain move");
        let hawk = from_san(&Seirawan, &start, "Nf3/H").expect("the hawk");
        let elephant = from_san(&Seirawan, &start, "Nf3/E").expect("the elephant");
        assert_eq!(plain.gate(), None);
        assert_eq!(hawk.gate(), Some(Role::Hawk));
        assert_eq!(elephant.gate(), Some(Role::Elephant));
        assert_eq!(from_san(&Seirawan, &start, "Nf3/Q"), None, "not a gate");
    }

    #[test]
    fn reading_is_case_sensitive_where_it_has_to_be() {
        // `b4` is a pawn move and `B4` is not a move at all — a bishop needs a
        // destination. Folding case would make one of them silently the other.
        let start = Position::starting();
        assert!(from_san(&Chess, &start, "b4").is_some());
        assert_eq!(from_san(&Chess, &start, "B4"), None);
    }
}
