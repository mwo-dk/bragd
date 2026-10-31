//! Chess and the six Variants that share its board, through `rules-chess`.

use crate::{
    Awaiting, Board, GameError, Hand, HandPiece, Highlight, HighlightKind, LegalMove, Outcome,
    Placed, Seat, State, Variant,
};
use rules_chess::outcome::Outcome as ChessOutcome;
use rules_chess::{CastlingNotation, Chess960, Color, Game, Position, Role, Rules, Square, Variant as _};
use std::collections::BTreeMap;

fn rules(variant: Variant) -> Rules {
    Rules::from_key(variant.key()).expect("every chess Variant key is a rules-chess key")
}

pub(crate) fn start(variant: Variant) -> String {
    rules(variant).starting_position().to_fen()
}

/// The start of one of the 960 arrangements; 518 is standard chess.
pub(crate) fn chess960(index: u16) -> String {
    Chess960::arrangement(index % 960).to_fen()
}

pub(crate) fn replay(
    variant: Variant,
    initial: Option<&str>,
    entries: &[&str],
) -> Result<State, GameError> {
    let rules = rules(variant);
    let mut game = match initial {
        // shortcut: a FEN carries no Seirawan hands, so a Seirawan Game from a FEN starts with none.
        Some(fen) => Game::from_position(
            rules,
            Position::from_fen(fen).map_err(|e| GameError::bad_position(format!("{e:?}")))?,
        ),
        None => Game::start(rules),
    };
    for (index, entry) in entries.iter().enumerate() {
        game.play_uci(entry)
            .map_err(|e| GameError::refused(index, entry, e))?;
    }
    Ok(state(variant, &game))
}

fn seat(variant: Variant, color: Color) -> Seat {
    Seat::of(variant, if color == Color::White { 0 } else { 1 })
}

/// `chess-wK`; Seirawan's pieces come from their own sheet, `seirawan-wH`.
pub(crate) fn sprite(variant: Variant, color: Color, role: Role) -> String {
    let piece = rules_chess::Piece::new(color, role);
    let sheet = if variant == Variant::Seirawan { "seirawan" } else { "chess" };
    let side = if color == Color::White { 'w' } else { 'b' };
    format!("{sheet}-{side}{}", piece.fen_char().to_ascii_uppercase())
}

fn state(variant: Variant, game: &Game<Rules>) -> State {
    let notation = game.variant().castling_notation();
    let position = game.position();
    let over = game.outcome().is_some();
    let to_move = position.side_to_move();

    let moves: Vec<LegalMove> = game
        .legal_moves()
        .into_iter()
        .map(|mv| {
            let wire = mv.to_uci(notation);
            // Where a Player drags the king: two squares on an 8×8 board, onto its rook in Chess960.
            let to = if mv.kind().is_castle() && notation == CastlingNotation::KingDestination { wire[2..4].to_string() } else { mv.to().to_string() };
            LegalMove {
                notation: game.san(mv),
                from: Some(mv.from().to_string()),
                to: Some(to),
                drop: mv.gate().map(|role| sprite(variant, to_move, role)),
                wire,
            }
        })
        .collect();

    let mut pieces = Vec::new();
    for index in 0..64 {
        let square = Square::new(index);
        if let Some(piece) = position.piece_at(square) {
            pieces.push(Placed {
                x: square.file(),
                y: square.rank(),
                square: square.to_string(),
                sprite: sprite(variant, piece.color, piece.role),
                count: 1,
            });
        }
    }

    let mut highlights = Vec::new();
    if let Some(last) = game.moves().last() {
        for square in [last.from(), last.to()] {
            highlights.push(Highlight {
                x: square.file(),
                y: square.rank(),
                kind: HighlightKind::LastMove,
            });
        }
    }
    let check = position.is_check();
    if check {
        let king = position.king_square(to_move);
        highlights.push(Highlight {
            x: king.file(),
            y: king.rank(),
            kind: HighlightKind::Check,
        });
    }

    let hands = if variant == Variant::Seirawan {
        Color::ALL
            .into_iter()
            .map(|color| Hand {
                seat: seat(variant, color).index,
                pieces: rules_chess::seirawan::IN_HAND
                    .into_iter()
                    .map(|role| HandPiece {
                        sprite: sprite(variant, color, role),
                        count: u8::from(position.in_hand(color, role)),
                    })
                    .collect(),
            })
            .collect()
    } else {
        Vec::new()
    };

    let info = variant.info();
    State {
        variant,
        position: position.to_fen(),
        to_move: (!over).then(|| seat(variant, to_move)),
        awaiting: if over { Awaiting::Nothing } else { Awaiting::Move },
        moves,
        targets: BTreeMap::new(),
        throws: Vec::new(),
        history: game.san_history(),
        check,
        outcome: game.outcome().map(|outcome| outcome_of(variant, outcome)),
        board: Board {
            cells: (0..64).map(Square::new).map(|s| crate::Cell { x: s.file(), y: s.rank(), square: s.to_string() }).collect(),
            geometry: info.geometry,
            files: info.files,
            ranks: info.ranks,
            chequered: info.chequered,
            pieces,
            highlights,
            hands,
            ..Board::default()
        },
    }
}

fn outcome_of(variant: Variant, outcome: ChessOutcome) -> Outcome {
    let reason = match outcome {
        ChessOutcome::Win { reason, .. } => format!("{reason:?}"),
        ChessOutcome::Draw { reason } => format!("{reason:?}"),
    };
    Outcome {
        winner: outcome.winner().map(|color| seat(variant, color)),
        reason: camel(&reason),
        result: outcome.scoreline().to_string(),
    }
}

/// `Checkmate` to `checkmate`: the crate's variant name as a camelCase token.
pub(crate) fn camel(debug: &str) -> String {
    let mut chars = debug.chars();
    match chars.next() {
        Some(first) => first.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}
