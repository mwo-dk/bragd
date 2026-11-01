//! The variant trait and its implementors.
//!
//! Four types implement [`Variant`]: [`Chess`], [`Chess960`], [`Shatranj`] and
//! [`Chaturanga`]. The trait was sized for the second and has taken two more
//! without growing a method, which is the evidence it was sized right. It is still *not* sized
//! for checkers or the tafl games, which are separate engines; a shared
//! abstraction over three rule systems would serve none of them.
//!
//! Note what each implementor actually overrides. Chess960 changes only where a
//! Game starts and how a castle is spelled — it adds no rule. Shatranj is the
//! first to override [`Variant::outcome`] with genuinely different endings, and
//! that is where its whole difference lives: its pieces are leaper tables, but
//! stalemate *wins* and a bared king loses. Chaturanga overrides the same
//! method to disagree with shatranj about both of those.
//!
//! Everything here is position-local. Termination that needs the history of a
//! Game — repetition, the move-count rules — lives on [`crate::Game`], because
//! a position alone genuinely cannot answer it.

use crate::bitboard::Bitboard;
use crate::chess_move::{CastlingNotation, Move, MoveList};
use crate::outcome::{DrawReason, Outcome, WinReason};
use crate::piece::{Color, Piece, Role};
use crate::position::Position;

pub trait Variant {
    /// The name used in URLs, rating pools and Tournament categories. Stable:
    /// changing it orphans a rating pool.
    fn key(&self) -> &'static str;

    /// The name shown to a Player.
    fn name(&self) -> &'static str;

    /// How this variant spells a castling move on the wire.
    ///
    /// Defaulted, so a variant is frozen to standard chess's spelling unless it
    /// deliberately opts out. That direction matters: a Library holds
    /// raw UCI strings for ever, so the wrong default would silently re-spell
    /// castles in Games already stored.
    fn castling_notation(&self) -> CastlingNotation {
        CastlingNotation::KingDestination
    }

    fn starting_position(&self) -> Position;

    fn legal_moves(&self, position: &Position) -> MoveList;

    /// # Panics
    /// If `mv` is not legal here. Callers taking moves from outside must check
    /// [`Variant::legal_moves`] first — a client is not trusted.
    fn make_move(&self, position: &Position, mv: Move) -> Position;

    /// The ending this position produces on its own, if any. Does not consider
    /// history; see [`crate::Game`].
    fn outcome(&self, position: &Position) -> Option<Outcome> {
        self.outcome_given(position, &self.legal_moves(position))
    }

    /// [`Variant::outcome`] when the position's legal moves are already known:
    /// `legal` must be `self.legal_moves(position)`. A [`crate::Game`] has them
    /// for its next move anyway, so asking here generates them only once a ply.
    fn outcome_given(&self, position: &Position, legal: &MoveList) -> Option<Outcome>;

    /// Whether neither side could mate even with the other's cooperation.
    fn is_insufficient_material(&self, position: &Position) -> bool;

    /// Whether `color` has material enough to mate. Asked on its own because a
    /// flag-fall is a draw rather than a loss when the *opponent* could not
    /// have mated in any continuation.
    fn can_possibly_mate(&self, position: &Position, color: Color) -> bool;
}

/// Standard chess.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Chess;

impl Variant for Chess {
    fn key(&self) -> &'static str {
        "chess"
    }

    fn name(&self) -> &'static str {
        "Chess"
    }

    fn starting_position(&self) -> Position {
        Position::starting()
    }

    fn legal_moves(&self, position: &Position) -> MoveList {
        position.legal_moves()
    }

    fn make_move(&self, position: &Position, mv: Move) -> Position {
        assert!(position.is_legal(mv), "{mv} is not legal in this position");
        position.make_move(mv)
    }

    fn outcome_given(&self, position: &Position, legal: &MoveList) -> Option<Outcome> {
        if self.is_insufficient_material(position) {
            return Some(Outcome::Draw {
                reason: DrawReason::InsufficientMaterial,
            });
        }
        if !legal.is_empty() {
            return None;
        }
        // No legal move: mate if the king is attacked, stalemate if not. The
        // distinction is the whole game.
        Some(if position.is_check() {
            Outcome::Win {
                winner: position.side_to_move().opponent(),
                reason: WinReason::Checkmate,
            }
        } else {
            Outcome::Draw {
                reason: DrawReason::Stalemate,
            }
        })
    }

    fn is_insufficient_material(&self, position: &Position) -> bool {
        Color::ALL
            .into_iter()
            .all(|color| !self.can_possibly_mate(position, color))
    }

    /// The practical rule, not FIDE's full "dead position" test.
    ///
    /// A side can mate unless it has only its king, or its king and a single
    /// minor piece. King and two knights is included as *able* to mate: mate is
    /// not forcible, but it is reachable with cooperation, and that is the
    /// standard every chess server applies. King and bishop against king and
    /// bishop on the same colour is handled here too, since neither side can
    /// mate alone.
    ///
    /// Deliberately not implemented: blocked-pawn positions that are dead by
    /// FIDE's definition. Detecting those needs search, and this engine does
    /// none.
    fn can_possibly_mate(&self, position: &Position, color: Color) -> bool {
        let ours = position.by_color(color);
        let minors = (position.by_role(Role::Knight) | position.by_role(Role::Bishop)) & ours;
        let majors_and_pawns = (position.by_role(Role::Pawn)
            | position.by_role(Role::Rook)
            | position.by_role(Role::Queen))
            & ours;

        if !majors_and_pawns.is_empty() {
            return true;
        }
        match minors.count() {
            0 => false,
            // A lone minor cannot mate — with one exception that is not an
            // exception: a lone bishop cannot mate even against a bishop of the
            // opposite colour, because neither side can force it. Sites differ
            // here; treating a lone minor as unable to mate matches FIDE's
            // automatic draw and is the conservative choice, since it never
            // awards a win that could not happen.
            1 => false,
            // Two or more: two bishops mate, bishop and knight mate, two
            // knights can mate with help.
            _ => true,
        }
    }
}

impl Chess {
    /// All squares `color` attacks — for board rendering and, later, move hints.
    pub fn attacked_squares(&self, position: &Position, color: Color) -> Bitboard {
        crate::movegen::attacked_squares(position, color)
    }
}

/// Chess960, also called Fischer Random.
///
/// The same game with a shuffled back rank. **It adds no rules**, and that is
/// the point of it being the second implementor: once a position carries the
/// squares its castling rooks started on, move generation is already general,
/// so this overrides only what is genuinely different — where a Game starts,
/// what it is called, and how a castle is spelled on the wire. Every rule
/// question is delegated to [`Chess`].
///
/// If this type needed a rule of its own, the castling work would have been
/// done in the wrong place.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Chess960;

impl Chess960 {
    /// The number of distinct starting arrangements.
    pub const ARRANGEMENTS: u16 = 960;

    /// Standard chess's arrangement, in the conventional numbering.
    ///
    /// Worth naming: it is the assertion that proves the numbering below is
    /// the one everybody else uses, since arrangement 518 has to come out as
    /// `RNBQKBNR` and nothing else.
    pub const STANDARD: u16 = 518;

    /// The starting position of arrangement `index`, by Scharnagl's numbering.
    ///
    /// The numbering is a mixed-radix decode, and it is worth following because
    /// it is *why* every arrangement it produces is legal by construction
    /// rather than by filtering:
    ///
    /// 1. `index % 4` places one bishop on a light square (there are four).
    /// 2. `index / 4 % 4` places the other on a dark square. Bishops on
    ///    opposite colours therefore cannot be got wrong.
    /// 3. `index / 16 % 6` places the queen on one of the six squares left.
    /// 4. `index / 96` selects one of the ten ways to place two knights in the
    ///    five remaining squares.
    /// 5. The last three squares take rook, king, rook in that order — which is
    ///    what guarantees the king is between its rooks, the invariant castling
    ///    depends on.
    ///
    /// Out-of-range indices wrap, so this is total: an index is data that may
    /// have come from anywhere, and a panic on the authoritative path is worse
    /// than a game of chess nobody asked for.
    pub fn arrangement(index: u16) -> Position {
        let index = index % Self::ARRANGEMENTS;
        let mut back: [Option<Role>; 8] = [None; 8];

        // The light squares of rank 1 are b, d, f, h; the dark ones a, c, e, g.
        let light = [1usize, 3, 5, 7];
        let dark = [0usize, 2, 4, 6];
        back[light[(index % 4) as usize]] = Some(Role::Bishop);
        back[dark[(index / 4 % 4) as usize]] = Some(Role::Bishop);

        let empty = |back: &[Option<Role>; 8], nth: usize| -> usize {
            back.iter()
                .enumerate()
                .filter(|(_, square)| square.is_none())
                .map(|(file, _)| file)
                .nth(nth)
                .expect("the arrangement always leaves enough squares")
        };

        let queen = empty(&back, (index / 16 % 6) as usize);
        back[queen] = Some(Role::Queen);

        // The ten ways to choose two of the five remaining squares, in the
        // conventional order.
        const KNIGHTS: [(usize, usize); 10] = [
            (0, 1),
            (0, 2),
            (0, 3),
            (0, 4),
            (1, 2),
            (1, 3),
            (1, 4),
            (2, 3),
            (2, 4),
            (3, 4),
        ];
        let (first, second) = KNIGHTS[(index / 96) as usize];
        // Both indices count among the *same* five squares — the ones free
        // before either knight is placed. Placing one and then counting again
        // would shift the second index by one and produce a different, and
        // wrong, arrangement.
        let five: Vec<usize> = (0..5).map(|nth| empty(&back, nth)).collect();
        back[five[first]] = Some(Role::Knight);
        back[five[second]] = Some(Role::Knight);

        // Always the *first* square still free, three times: the list shrinks
        // as it fills, so asking for the nth would skip one and then run out.
        // Rook, king, rook in that order is what guarantees the king ends up
        // between its rooks — the invariant castling depends on.
        for role in [Role::Rook, Role::King, Role::Rook] {
            let file = empty(&back, 0);
            back[file] = Some(role);
        }

        let mut fen = String::with_capacity(80);
        for square in back {
            fen.push(Piece::new(Color::Black, square.expect("every file is filled")).fen_char());
        }
        fen.push_str("/pppppppp/8/8/8/8/PPPPPPPP/");
        for square in back {
            fen.push(Piece::new(Color::White, square.expect("every file is filled")).fen_char());
        }
        // Shredder-FEN: the rooks are not on the a- and h-files, so `KQkq`
        // would be a lie in most arrangements. `from_fen` reads both dialects.
        let rooks: Vec<char> = back
            .iter()
            .enumerate()
            .filter(|(_, role)| **role == Some(Role::Rook))
            .map(|(file, _)| (b'A' + file as u8) as char)
            .collect();
        fen.push_str(&format!(
            " w {}{} - 0 1",
            rooks.iter().rev().collect::<String>(),
            rooks
                .iter()
                .rev()
                .map(|letter| letter.to_ascii_lowercase())
                .collect::<String>()
        ));

        Position::from_fen(&fen).expect("every arrangement is a legal position")
    }
}

impl Variant for Chess960 {
    fn key(&self) -> &'static str {
        "chess960"
    }

    fn name(&self) -> &'static str {
        "Chess960"
    }

    /// Arrangement 518, which is standard chess.
    ///
    /// A Chess960 Game is always started from an explicit arrangement, so this
    /// exists only to satisfy the trait. Returning the standard one rather than
    /// a random one is deliberate: this crate has no randomness and must not
    /// grow any — a rules engine that produces a different answer each time it
    /// is asked cannot be a correctness gate.
    fn starting_position(&self) -> Position {
        Chess960::arrangement(Chess960::STANDARD)
    }

    /// The only thing Chess960 changes about how a Game is written down.
    fn castling_notation(&self) -> CastlingNotation {
        CastlingNotation::KingTakesRook
    }

    // Every rule below is standard chess's, unchanged. If any of these needed
    // an override, the castling work would be in the wrong place.
    fn legal_moves(&self, position: &Position) -> MoveList {
        Chess.legal_moves(position)
    }

    fn make_move(&self, position: &Position, mv: Move) -> Position {
        Chess.make_move(position, mv)
    }

    fn outcome_given(&self, position: &Position, legal: &MoveList) -> Option<Outcome> {
        Chess.outcome_given(position, legal)
    }

    fn is_insufficient_material(&self, position: &Position) -> bool {
        Chess.is_insufficient_material(position)
    }

    fn can_possibly_mate(&self, position: &Position, color: Color) -> bool {
        Chess.can_possibly_mate(position, color)
    }
}

/// Shatranj, the game chess grew out of.
///
/// The movement is in [`crate::shatranj`]; what lives here is the part that is
/// actually novel, which is not movement at all. Two of the three ways this
/// game ends have no counterpart in chess:
///
/// - **Stalemate wins** for the side that inflicted it.
/// - **Baring the opponent's king wins**, unless they bare yours in reply, in
///   which case the game is drawn.
///
/// Both are decided in [`Variant::outcome`], and the second is the only ending
/// in Bragd that cannot be read off a position without generating its
/// replies.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Shatranj;

impl Variant for Shatranj {
    fn key(&self) -> &'static str {
        "shatranj"
    }

    fn name(&self) -> &'static str {
        "Shatranj"
    }

    fn starting_position(&self) -> Position {
        Position::from_fen(crate::shatranj::STARTING_POSITION)
            .expect("the shatranj opening array is a legal position")
    }

    fn legal_moves(&self, position: &Position) -> MoveList {
        crate::shatranj::legal_moves(position)
    }

    fn make_move(&self, position: &Position, mv: Move) -> Position {
        assert!(
            crate::shatranj::legal_moves(position).contains(mv),
            "{mv} is not legal in this position"
        );
        position.make_move(mv)
    }

    /// The three decisive endings, in the order they have to be asked.
    ///
    /// No-legal-move is tested first because it settles both checkmate and
    /// stalemate, and because a side with no move cannot bare anyone in reply —
    /// so asking about baring first would have to ask about moves anyway.
    fn outcome_given(&self, position: &Position, legal: &MoveList) -> Option<Outcome> {
        let us = position.side_to_move();
        let them = us.opponent();
        let moves = legal;

        if moves.is_empty() {
            // Both are wins for the opponent. What differs is only the reason,
            // and the reason is the entire point of the variant.
            return Some(Outcome::Win {
                winner: them,
                reason: if position.is_check() {
                    WinReason::Checkmate
                } else {
                    WinReason::Stalemate
                },
            });
        }

        if position.is_bare(us) {
            // Two lone kings: nobody bared anybody first, and neither side can
            // ever do anything again. Drawn rather than won on a technicality
            // of who ran out of pieces in which order.
            if position.is_bare(them) {
                return Some(Outcome::Draw {
                    reason: DrawReason::BothKingsBared,
                });
            }
            // The deferred half: bared, but with a reply that bares back.
            return Some(if crate::shatranj::can_bare_opponent(position, moves) {
                Outcome::Draw {
                    reason: DrawReason::BothKingsBared,
                }
            } else {
                Outcome::Win {
                    winner: them,
                    reason: WinReason::BareKing,
                }
            });
        }

        None
    }

    /// Never. A bare king *loses* in shatranj rather than drawing, so there is
    /// no material so thin that the game is dead — it is simply already
    /// decided, and [`Variant::outcome`] says so.
    fn is_insufficient_material(&self, _position: &Position) -> bool {
        false
    }

    /// Whether `color` has anything at all besides its king.
    ///
    /// Asked only on a flag-fall, to decide whether the Clock running out is a
    /// loss or a draw. Deliberately coarser than chess's version: a lone ferz
    /// or a lone alfil cannot in fact force mate, and probably cannot mate even
    /// with help, but working out which thin material can helpmate in shatranj
    /// needs a search this engine does not do. Erring towards "could have
    /// mated" only ever awards the win to the side that still had pieces on the
    /// board, which is the conservative direction for a flag.
    fn can_possibly_mate(&self, position: &Position, color: Color) -> bool {
        !position.is_bare(color)
    }
}

/// Chaturanga, under one named reconstruction.
///
/// The movement is in [`crate::chaturanga`], along with *which* reconstruction
/// and why naming one was the condition for building this at all. What lives
/// here is again the ending, and again it is where the game is not shatranj:
///
/// - **Stalemate wins for the stalemated side** — the exact opposite of
///   shatranj. The two variants share a board, a piece set and an opening
///   array, and score the same terminal position in opposite directions.
/// - **Baring the king wins outright.** No reply, no one-ply reprieve, so
///   unlike shatranj this ending *can* be read off material alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Chaturanga;

impl Variant for Chaturanga {
    fn key(&self) -> &'static str {
        "chaturanga"
    }

    fn name(&self) -> &'static str {
        "Chaturanga"
    }

    fn starting_position(&self) -> Position {
        Position::from_fen(crate::chaturanga::STARTING_POSITION)
            .expect("the chaturanga opening array is a legal position")
    }

    fn legal_moves(&self, position: &Position) -> MoveList {
        crate::chaturanga::legal_moves(position)
    }

    fn make_move(&self, position: &Position, mv: Move) -> Position {
        assert!(
            crate::chaturanga::legal_moves(position).contains(mv),
            "{mv} is not legal in this position"
        );
        position.make_move(mv)
    }

    /// The three decisive endings, in the order they have to be asked.
    ///
    /// No-legal-move first, as in shatranj — but the stalemate branch hands the
    /// win to the side that cannot move rather than to the side that trapped
    /// it. That single `us` in place of `them` is the whole variant, which is
    /// why it is worth a test of its own rather than a comment.
    fn outcome_given(&self, position: &Position, legal: &MoveList) -> Option<Outcome> {
        let us = position.side_to_move();
        let them = us.opponent();

        if legal.is_empty() {
            return Some(if position.is_check() {
                Outcome::Win {
                    winner: them,
                    reason: WinReason::Checkmate,
                }
            } else {
                Outcome::Win {
                    winner: us,
                    reason: WinReason::Stalemate,
                }
            });
        }

        // Baring, with no deferred half to ask about. Two lone kings are drawn
        // rather than won by whoever was bared second, for the same reason as
        // in shatranj: neither side can ever do anything again.
        if position.is_bare(us) {
            return Some(if position.is_bare(them) {
                Outcome::Draw {
                    reason: DrawReason::BothKingsBared,
                }
            } else {
                Outcome::Win {
                    winner: them,
                    reason: WinReason::BareKing,
                }
            });
        }

        None
    }

    /// Never — a bare king loses here, so no material is too thin to play on
    /// with; the game is decided rather than dead.
    fn is_insufficient_material(&self, _position: &Position) -> bool {
        false
    }

    /// Whether `color` has anything besides its king, for the same reason and
    /// with the same deliberate coarseness as shatranj's.
    fn can_possibly_mate(&self, position: &Position, color: Color) -> bool {
        !position.is_bare(color)
    }
}

/// King of the Hill: chess, plus one more way to win.
///
/// Every piece moves as it does in chess, castling is chess's, promotion is
/// chess's, and checkmate still ends it. The only addition is that **a king
/// standing on d4, e4, d5 or e5 has won**, immediately and regardless of
/// anything else on the board.
///
/// One extra clause, and it rewrites every plan: the king stops being the thing
/// you hide and becomes the thing you run. A sacrifice that opens a path to the
/// centre is now sound, and a fortress is now a loss waiting to happen.
///
/// The rule is *position-local* — it can be read off the board with no history
/// — which is why it fits [`Variant`] without the trait growing a method.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct KingOfTheHill;

impl KingOfTheHill {
    /// The four squares that end it: d4, e4, d5 and e5.
    ///
    /// Written as indices rather than parsed from names so this is a `const`
    /// and costs nothing at run time. a1 is 0, so d4 is 3 + 3×8 = 27.
    const HILL: [u8; 4] = [27, 28, 35, 36];

    fn on_the_hill(position: &Position, color: Color) -> bool {
        let square = position.king_square(color).index();
        Self::HILL.contains(&square)
    }
}

impl Variant for KingOfTheHill {
    fn key(&self) -> &'static str {
        "kingofthehill"
    }

    fn name(&self) -> &'static str {
        "King of the Hill"
    }

    fn starting_position(&self) -> Position {
        Position::starting()
    }

    fn legal_moves(&self, position: &Position) -> MoveList {
        position.legal_moves()
    }

    fn make_move(&self, position: &Position, mv: Move) -> Position {
        assert!(position.is_legal(mv), "{mv} is not legal in this position");
        position.make_move(mv)
    }

    /// The hill is asked **first**, and that order is the rule.
    ///
    /// A king that arrives on the hill has won even if the move that took it
    /// there also delivered checkmate, and even if the side now to move has no
    /// legal reply. Asking about checkmate first would score those positions
    /// backwards — the game was already over when the king landed.
    ///
    /// Only the opponent's king is checked for arrival: the side to move cannot
    /// have a king on the hill, because the game would have ended on the ply
    /// that put it there.
    fn outcome_given(&self, position: &Position, legal: &MoveList) -> Option<Outcome> {
        let them = position.side_to_move().opponent();
        if Self::on_the_hill(position, them) {
            return Some(Outcome::Win {
                winner: them,
                reason: WinReason::KingOfTheHill,
            });
        }

        // **Spelled out rather than delegated to `Chess::outcome`.**
        //
        // Delegating looks right and is wrong: chess's version begins by
        // declaring insufficient material a draw, and *no* material is
        // insufficient here — two lone kings is a dead draw in chess and a live
        // race to the centre in this variant. Calling it would end games that
        // had barely started, and would do it silently, as a draw nobody could
        // argue with.
        //
        // The rest is chess's exactly: no legal move is mate if the king is
        // attacked and stalemate if it is not.
        if !legal.is_empty() {
            return None;
        }
        Some(if position.is_check() {
            Outcome::Win {
                winner: them,
                reason: WinReason::Checkmate,
            }
        } else {
            Outcome::Draw {
                reason: DrawReason::Stalemate,
            }
        })
    }

    /// **Never**, and this is not a shortcut.
    ///
    /// Two lone kings is a dead draw in chess and a live game here: either king
    /// can simply walk to the centre and win. Every "insufficient" material
    /// combination in chess is sufficient in this variant, so the whole idea
    /// does not apply.
    fn is_insufficient_material(&self, _position: &Position) -> bool {
        false
    }

    /// Always. A flag-fall is a loss here whatever is left on the board,
    /// because a bare king is still a king that could have reached the hill.
    fn can_possibly_mate(&self, _position: &Position, _color: Color) -> bool {
        true
    }
}

/// Racing Kings: both kings run for the eighth rank, and nobody may give check.
///
/// The pieces are chess's and the movement is chess's; the two departures are
/// in [`crate::racing`] and here. **No move may give check**, in either
/// direction, which is what stops White from simply harassing Black's king
/// backwards for the whole game. And the race is won on the eighth rank rather
/// than by mate.
///
/// The subtle rule is the fair one. White moves first, so a race decided the
/// instant a king arrives would be decided by the coin toss that handed out
/// colours. So **Black always gets the reply**: if White arrives and Black
/// arrives in answer, the game is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RacingKings;

impl Variant for RacingKings {
    fn key(&self) -> &'static str {
        "racingkings"
    }

    fn name(&self) -> &'static str {
        "Racing Kings"
    }

    fn starting_position(&self) -> Position {
        Position::from_fen(crate::racing::STARTING_POSITION)
            .expect("the racing kings opening array is a legal position")
    }

    fn legal_moves(&self, position: &Position) -> MoveList {
        crate::racing::legal_moves(position)
    }

    fn make_move(&self, position: &Position, mv: Move) -> Position {
        assert!(
            crate::racing::legal_moves(position).contains(mv),
            "{mv} is not legal in this position"
        );
        position.make_move(mv)
    }

    /// The race, in the order the rules require.
    ///
    /// 1. **Both kings home is a draw.** Asked first, because it is the case
    ///    the other two would each answer as a win.
    /// 2. **Black home wins for Black**, whoever is to move. If it is White to
    ///    move then White has already had their turn and did not arrive.
    /// 3. **White home wins for White, but only once Black has answered** —
    ///    that is what `side_to_move == White` means here: Black moved and did
    ///    not arrive. While it is still Black to move the race is not over,
    ///    unless no legal move takes Black's king home: then the answer
    ///    cannot change it, and White has won now.
    /// 4. No legal move is a **draw**, never a loss. A side with nothing to
    ///    play cannot be in check, because a move that gives check is illegal,
    ///    so there is no checkmate in this variant at all.
    fn outcome_given(&self, position: &Position, legal: &MoveList) -> Option<Outcome> {
        let white_home = crate::racing::has_arrived(position, Color::White);
        let black_home = crate::racing::has_arrived(position, Color::Black);

        if white_home && black_home {
            return Some(Outcome::Draw {
                reason: DrawReason::BothKingsHome,
            });
        }
        if black_home {
            return Some(Outcome::Win {
                winner: Color::Black,
                reason: WinReason::RaceWon,
            });
        }
        let black_king = position.king_square(Color::Black);
        let black_can_follow = || legal.as_slice().iter().any(|m| m.from() == black_king && m.to().rank() == 7);
        if white_home && (position.side_to_move() == Color::White || !black_can_follow()) {
            return Some(Outcome::Win {
                winner: Color::White,
                reason: WinReason::RaceWon,
            });
        }

        if legal.is_empty() {
            return Some(Outcome::Draw {
                reason: DrawReason::Stalemate,
            });
        }

        None
    }

    /// Never. There is no material so thin that a king cannot run.
    fn is_insufficient_material(&self, _position: &Position) -> bool {
        false
    }

    fn can_possibly_mate(&self, position: &Position, color: Color) -> bool {
        crate::racing::can_still_race(position, color)
    }
}

/// **Seirawan chess**: chess, plus two pieces waiting to be let in.
///
/// The rules are in [`crate::seirawan`]; what lives here is the shape the
/// platform sees. Three things are worth saying:
///
/// - The **opening position is a chessboard**, so every opening anybody knows
///   still applies. What changes is that developing a piece hands you the
///   option of putting something behind it.
/// - The endings are chess's. Checkmate, stalemate, the fifty-move rule — all
///   unchanged, because nothing about gating changes what it means for a king
///   to be trapped.
/// - **Insufficient material has to account for the hand.** A lone king with an
///   elephant still to gate is not a dead position, and calling it one would
///   end a game the player was about to win.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Seirawan;

impl Variant for Seirawan {
    fn key(&self) -> &'static str {
        "seirawan"
    }

    fn name(&self) -> &'static str {
        "Seirawan chess"
    }

    fn starting_position(&self) -> Position {
        // The hands are put on here rather than parsed from the FEN: a FEN
        // cannot say which variant it belongs to, and a position carrying two
        // pieces in hand is only meaningful in this one.
        //
        // Every back-rank square, and this is the one place in the crate
        // entitled to say so: nothing has moved yet, which is the fact a gating
        // right is about and the fact no board can be asked for.
        Position::starting().with_seirawan_hands(Position::home_ranks())
    }

    fn legal_moves(&self, position: &Position) -> MoveList {
        crate::seirawan::legal_moves(position)
    }

    fn make_move(&self, position: &Position, mv: Move) -> Position {
        assert!(
            crate::seirawan::legal_moves(position).contains(mv),
            "{mv} is not legal in this position"
        );
        position.make_move(mv)
    }

    fn outcome_given(&self, position: &Position, legal: &MoveList) -> Option<Outcome> {
        if self.is_insufficient_material(position) {
            return Some(Outcome::Draw {
                reason: DrawReason::InsufficientMaterial,
            });
        }
        if !legal.is_empty() {
            return None;
        }
        Some(if position.is_check() {
            Outcome::Win {
                winner: position.side_to_move().opponent(),
                reason: WinReason::Checkmate,
            }
        } else {
            Outcome::Draw {
                reason: DrawReason::Stalemate,
            }
        })
    }

    fn is_insufficient_material(&self, position: &Position) -> bool {
        Color::ALL
            .into_iter()
            .all(|color| !self.can_possibly_mate(position, color))
    }

    /// Chess's test, **plus the hand, plus the two pieces themselves**.
    ///
    /// A side holding a hawk or an elephant with a square left to gate it onto
    /// can still produce mating material out of nothing, which is a thing no
    /// other variant in Bragd can do. Declaring that position dead would
    /// end a game somebody was one move from winning.
    ///
    /// **And a hawk on the board counts, which it did not.** Chess's test knows
    /// six roles and neither of these is one of them, so king and hawk against
    /// a lone king read as no material at all — and since `outcome` asks about
    /// insufficient material *first*, the Game was declared a dead draw with a
    /// piece on the board that mates a bare king comfortably. Reachable in
    /// ordinary play: gate a hawk on move one, trade everything else off, and
    /// the hands are empty so the clause above cannot rescue it either. Adding
    /// the promotions gave it a second route in.
    ///
    /// **This clause is only true because `attackers_to` knows the two pieces,
    /// and for a while it did not.** A hawk that cannot give check cannot mate,
    /// so "a piece that mates a bare king comfortably" would have been a false
    /// sentence and this would have turned a wrong draw into a wrong *win*:
    /// `Game::flag` asks exactly this function, so a flag-fall would have
    /// written `Outcome::Win { reason: Flag }` for a player who provably could
    /// not have mated — a stored result, and a rated one later. The two changes
    /// are one change; neither belongs in a Library without the other.
    ///
    /// Asked here rather than added to `Chess::can_possibly_mate`, where the
    /// bitboards would be empty and the answer never differ: that function is
    /// chess's rule, and the reason it is safe to reuse is that it is written
    /// about chess's pieces. A variant that brings its own counts them itself.
    fn can_possibly_mate(&self, position: &Position, color: Color) -> bool {
        let compounds = (position.by_role(Role::Hawk) | position.by_role(Role::Elephant))
            & position.by_color(color);
        crate::seirawan::can_still_gate(position, color)
            || !compounds.is_empty()
            || Chess.can_possibly_mate(position, color)
    }
}

/// Every variant this crate plays, as one type.
///
/// It exists for one reason: the seam in `bragd-games` holds open Games in a
/// single table, and a table cannot hold two types. Dispatching here rather
/// than duplicating every entry point is what keeps the ABI from growing a
/// parallel set of functions per variant.
///
/// Deliberately confined to the **chess family**. This is not the seam a tafl
/// engine arrives at — that seam is in Bragd side, and a tafl
/// game is a different crate with a different table.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Rules {
    #[default]
    Chess,
    Chess960,
    Shatranj,
    Chaturanga,
    KingOfTheHill,
    RacingKings,
    Seirawan,
}

impl Rules {
    /// The variant a stored key names, or nothing.
    ///
    /// Nothing rather than a fallback to chess: a Game whose rules are missing
    /// must fail loudly, because replaying it under different rules produces a
    /// different Game and writes it to a Library.
    ///
    /// **Read out of [`Rules::ALL`] rather than out of a second table.** This
    /// was a `match` over the same seven keys with a `_ => None` of its own,
    /// sitting four lines above the list — so a variant added to one and not the
    /// other is a variant Bragd offers and no Game can open, and the arm
    /// that answers for it is a `default:` that says nothing.
    pub fn from_key(key: &str) -> Option<Rules> {
        Rules::ALL.into_iter().find(|rules| rules.key() == key)
    }

    pub const ALL: [Rules; 7] = [
        Rules::Chess,
        Rules::Chess960,
        Rules::Shatranj,
        Rules::Chaturanga,
        Rules::KingOfTheHill,
        Rules::RacingKings,
        Rules::Seirawan,
    ];
}

impl Variant for Rules {
    fn key(&self) -> &'static str {
        match self {
            Rules::Chess => Chess.key(),
            Rules::Chess960 => Chess960.key(),
            Rules::Shatranj => Shatranj.key(),
            Rules::KingOfTheHill => KingOfTheHill.key(),
            Rules::Seirawan => Seirawan.key(),
            Rules::RacingKings => RacingKings.key(),
            Rules::Chaturanga => Chaturanga.key(),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Rules::Chess => Chess.name(),
            Rules::Chess960 => Chess960.name(),
            Rules::Shatranj => Shatranj.name(),
            Rules::KingOfTheHill => KingOfTheHill.name(),
            Rules::Seirawan => Seirawan.name(),
            Rules::RacingKings => RacingKings.name(),
            Rules::Chaturanga => Chaturanga.name(),
        }
    }

    fn castling_notation(&self) -> CastlingNotation {
        match self {
            Rules::Chess => Chess.castling_notation(),
            Rules::Chess960 => Chess960.castling_notation(),
            Rules::Shatranj => Shatranj.castling_notation(),
            Rules::KingOfTheHill => KingOfTheHill.castling_notation(),
            Rules::Seirawan => Seirawan.castling_notation(),
            Rules::RacingKings => RacingKings.castling_notation(),
            Rules::Chaturanga => Chaturanga.castling_notation(),
        }
    }

    fn starting_position(&self) -> Position {
        match self {
            Rules::Chess => Chess.starting_position(),
            Rules::Chess960 => Chess960.starting_position(),
            Rules::Shatranj => Shatranj.starting_position(),
            Rules::KingOfTheHill => KingOfTheHill.starting_position(),
            Rules::Seirawan => Seirawan.starting_position(),
            Rules::RacingKings => RacingKings.starting_position(),
            Rules::Chaturanga => Chaturanga.starting_position(),
        }
    }

    fn legal_moves(&self, position: &Position) -> MoveList {
        match self {
            Rules::Chess => Chess.legal_moves(position),
            Rules::Chess960 => Chess960.legal_moves(position),
            Rules::Shatranj => Shatranj.legal_moves(position),
            Rules::KingOfTheHill => KingOfTheHill.legal_moves(position),
            Rules::Seirawan => Seirawan.legal_moves(position),
            Rules::RacingKings => RacingKings.legal_moves(position),
            Rules::Chaturanga => Chaturanga.legal_moves(position),
        }
    }

    fn make_move(&self, position: &Position, mv: Move) -> Position {
        match self {
            Rules::Chess => Chess.make_move(position, mv),
            Rules::Chess960 => Chess960.make_move(position, mv),
            Rules::Shatranj => Shatranj.make_move(position, mv),
            Rules::KingOfTheHill => KingOfTheHill.make_move(position, mv),
            Rules::Seirawan => Seirawan.make_move(position, mv),
            Rules::RacingKings => RacingKings.make_move(position, mv),
            Rules::Chaturanga => Chaturanga.make_move(position, mv),
        }
    }

    fn outcome_given(&self, position: &Position, legal: &MoveList) -> Option<Outcome> {
        match self {
            Rules::Chess => Chess.outcome_given(position, legal),
            Rules::Chess960 => Chess960.outcome_given(position, legal),
            Rules::Shatranj => Shatranj.outcome_given(position, legal),
            Rules::KingOfTheHill => KingOfTheHill.outcome_given(position, legal),
            Rules::Seirawan => Seirawan.outcome_given(position, legal),
            Rules::RacingKings => RacingKings.outcome_given(position, legal),
            Rules::Chaturanga => Chaturanga.outcome_given(position, legal),
        }
    }

    fn is_insufficient_material(&self, position: &Position) -> bool {
        match self {
            Rules::Chess => Chess.is_insufficient_material(position),
            Rules::Chess960 => Chess960.is_insufficient_material(position),
            Rules::Shatranj => Shatranj.is_insufficient_material(position),
            Rules::KingOfTheHill => KingOfTheHill.is_insufficient_material(position),
            Rules::Seirawan => Seirawan.is_insufficient_material(position),
            Rules::RacingKings => RacingKings.is_insufficient_material(position),
            Rules::Chaturanga => Chaturanga.is_insufficient_material(position),
        }
    }

    fn can_possibly_mate(&self, position: &Position, color: Color) -> bool {
        match self {
            Rules::Chess => Chess.can_possibly_mate(position, color),
            Rules::Chess960 => Chess960.can_possibly_mate(position, color),
            Rules::Shatranj => Shatranj.can_possibly_mate(position, color),
            Rules::KingOfTheHill => KingOfTheHill.can_possibly_mate(position, color),
            Rules::Seirawan => Seirawan.can_possibly_mate(position, color),
            Rules::RacingKings => RacingKings.can_possibly_mate(position, color),
            Rules::Chaturanga => Chaturanga.can_possibly_mate(position, color),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outcome::DrawReason;

    fn position(fen: &str) -> Position {
        Position::from_fen(fen).expect("test FEN should parse")
    }

    #[test]
    fn checkmate_is_a_win_for_the_side_that_is_not_stuck() {
        // Fool's mate.
        let mated = position("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3");
        assert_eq!(
            Chess.outcome(&mated),
            Some(Outcome::Win {
                winner: Color::Black,
                reason: WinReason::Checkmate
            })
        );
    }

    #[test]
    fn stalemate_is_a_draw() {
        let stalemate = position("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1");
        assert!(!stalemate.is_check());
        assert_eq!(
            Chess.outcome(&stalemate),
            Some(Outcome::Draw {
                reason: DrawReason::Stalemate
            })
        );
    }

    #[test]
    fn a_position_with_moves_left_has_no_outcome() {
        assert_eq!(Chess.outcome(&Position::starting()), None);
    }

    #[test]
    fn insufficient_material_is_decided_per_side() {
        let cases: &[(&str, bool, bool)] = &[
            // FEN, White can mate, Black can mate.
            ("4k3/8/8/8/8/8/8/4K3 w - - 0 1", false, false),
            ("4k3/8/8/8/8/8/8/4KB2 w - - 0 1", false, false),
            ("4k3/8/8/8/8/8/8/4KN2 w - - 0 1", false, false),
            ("4k3/8/8/8/8/8/8/4KNN1 w - - 0 1", true, false),
            ("4k3/8/8/8/8/8/8/4KBB1 w - - 0 1", true, false),
            ("4k3/8/8/8/8/8/8/4KR2 w - - 0 1", true, false),
            ("4k3/8/8/8/8/8/8/4KQ2 w - - 0 1", true, false),
            ("4k3/8/8/8/8/8/P7/4K3 w - - 0 1", true, false),
            ("4kb2/8/8/8/8/8/8/4KB2 w - - 0 1", false, false),
            ("4kr2/8/8/8/8/8/8/4KB2 w - - 0 1", false, true),
        ];
        for (fen, white_can_mate, black_can_mate) in cases {
            let position = position(fen);
            assert_eq!(
                Chess.can_possibly_mate(&position, Color::White),
                *white_can_mate,
                "White in {fen}"
            );
            assert_eq!(
                Chess.can_possibly_mate(&position, Color::Black),
                *black_can_mate,
                "Black in {fen}"
            );
            assert_eq!(
                Chess.is_insufficient_material(&position),
                !white_can_mate && !black_can_mate,
                "{fen}"
            );
        }
    }

    #[test]
    fn a_bare_king_against_a_bare_king_is_drawn_immediately() {
        let bare = position("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        assert_eq!(
            Chess.outcome(&bare),
            Some(Outcome::Draw {
                reason: DrawReason::InsufficientMaterial
            }),
            "and not 'ongoing' — there are legal moves, but no game"
        );
    }

    #[test]
    fn checkmate_beats_insufficient_material_only_when_material_suffices() {
        // A side with a lone knight cannot mate, so a position that is
        // materially dead is drawn even though moves remain.
        let dead = position("4k3/8/8/8/8/8/8/4KN2 b - - 0 1");
        assert!(Chess.outcome(&dead).unwrap().is_draw());
    }

    #[test]
    fn the_variant_key_is_stable() {
        // Rating pools are keyed by this. Changing it orphans them.
        assert_eq!(Chess.key(), "chess");
        assert_eq!(Chess960.key(), "chess960");
    }

    #[test]
    fn arrangement_518_is_standard_chess() {
        // One assertion, two claims: that the numbering is the conventional
        // one everybody else uses, and that standard chess really is one of the
        // 960 rather than a case beside them.
        assert_eq!(
            Chess960::arrangement(Chess960::STANDARD),
            Position::starting()
        );
    }

    #[test]
    fn every_arrangement_is_a_legal_position() {
        // `arrangement` claims to be correct by construction rather than by
        // filtering, so this checks the construction: 960 distinct back ranks,
        // bishops on opposite colours, and the king between its rooks — which
        // is the invariant castling depends on.
        let mut seen = std::collections::HashSet::new();
        for index in 0..Chess960::ARRANGEMENTS {
            let position = Chess960::arrangement(index);
            assert!(
                seen.insert(position.to_fen()),
                "arrangement {index} is a repeat"
            );

            let file_of = |role: Role| -> Vec<u8> {
                position
                    .pieces(Color::White, role)
                    .into_iter()
                    .filter(|square| square.rank() == 0)
                    .map(|square| square.file())
                    .collect()
            };
            let bishops = file_of(Role::Bishop);
            assert_eq!(bishops.len(), 2, "arrangement {index}");
            assert_ne!(
                bishops[0] % 2,
                bishops[1] % 2,
                "arrangement {index}: bishops on the same colour"
            );

            let rooks = file_of(Role::Rook);
            let king = file_of(Role::King);
            assert_eq!(rooks.len(), 2, "arrangement {index}");
            assert!(
                rooks[0] < king[0] && king[0] < rooks[1],
                "arrangement {index}: the king is not between its rooks"
            );

            // Both colours get the same back rank, mirrored.
            for (white, black) in file_of(Role::Rook).into_iter().zip(
                position
                    .pieces(Color::Black, Role::Rook)
                    .into_iter()
                    .filter(|square| square.rank() == 7)
                    .map(|square| square.file()),
            ) {
                assert_eq!(white, black, "arrangement {index}: the ranks differ");
            }
        }
        assert_eq!(seen.len(), 960);
    }

    #[test]
    fn an_out_of_range_arrangement_wraps_rather_than_panicking() {
        // An index is data, and it may have come from anywhere. A panic here
        // would be on the authoritative path.
        assert_eq!(Chess960::arrangement(960), Chess960::arrangement(0));
        assert_eq!(
            Chess960::arrangement(u16::MAX),
            Chess960::arrangement(u16::MAX % 960)
        );
    }

    #[test]
    fn every_variant_is_reachable_by_its_stored_key() {
        // The key is what a stored Game carries. If a variant were not
        // reachable by it, that Game could never be replayed.
        for rules in Rules::ALL {
            assert_eq!(Rules::from_key(rules.key()), Some(rules));
        }
        assert_eq!(
            Rules::from_key("hnefatafl"),
            None,
            "a different engine's game"
        );
        assert_eq!(Rules::from_key(""), None);
    }

    #[test]
    fn the_dispatching_enum_agrees_with_what_it_dispatches_to() {
        // A hand-written match is exactly the sort of thing that goes stale one
        // arm at a time.
        let position = Chess960::arrangement(0);
        assert_eq!(Rules::Chess960.key(), Chess960.key());
        assert_eq!(Rules::Chess960.name(), Chess960.name());
        assert_eq!(
            Rules::Chess960.castling_notation(),
            Chess960.castling_notation()
        );
        assert_eq!(Rules::Chess.castling_notation(), Chess.castling_notation());
        assert_eq!(
            Rules::Chess960.legal_moves(&position).len(),
            Chess960.legal_moves(&position).len()
        );
        assert_eq!(Rules::Chess.starting_position(), Chess.starting_position());
    }

    #[test]
    fn chess960_changes_the_spelling_and_nothing_else() {
        // The whole claim of the second variant: it adds no rules. If any of
        // these ever diverges, the castling work went in the wrong place.
        let position = Chess960::arrangement(0);
        assert_eq!(
            Chess960.legal_moves(&position).len(),
            Chess.legal_moves(&position).len()
        );
        assert_eq!(Chess960.outcome(&position), Chess.outcome(&position));
        assert_eq!(
            Chess.castling_notation(),
            crate::chess_move::CastlingNotation::KingDestination,
            "standard chess's spelling is frozen: D1 is full of it"
        );
        assert_eq!(
            Chess960.castling_notation(),
            crate::chess_move::CastlingNotation::KingTakesRook
        );
    }

    #[test]
    #[should_panic(expected = "not legal")]
    fn making_an_illegal_move_through_the_variant_panics() {
        let start = Position::starting();
        let illegal = Move::new(
            crate::square::squares::E1,
            crate::square::squares::E8,
            crate::chess_move::MoveKind::Quiet,
        );
        Chess.make_move(&start, illegal);
    }
}
