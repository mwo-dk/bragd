//! Shogi, through `rules-shogi`. Moves travel as USI (`7g7f`, `7g7f+`, `P*5e`)
//! and positions as SFEN.

use crate::chess::camel;
use crate::{
    Awaiting, Board, GameError, Hand, HandPiece, Highlight, HighlightKind, LegalMove, Outcome,
    Placed, Seat, State, Variant,
};
use rules_shogi::board::SQUARES;
use rules_shogi::{Game, Move, Outcome as ShogiOutcome, Piece, Position, Side, Square};
use std::collections::BTreeMap;

pub(crate) fn start() -> String {
    Position::starting().to_sfen()
}

pub(crate) fn replay(initial: Option<&str>, entries: &[&str]) -> Result<State, GameError> {
    let mut game = match initial {
        Some(sfen) => Game::from_position(
            Position::from_sfen(sfen).ok_or_else(|| GameError::bad_position("not an SFEN"))?,
        ),
        None => Game::new(),
    };
    for (index, entry) in entries.iter().enumerate() {
        let mv =
            Move::parse(entry).ok_or_else(|| GameError::refused(index, entry, "not a USI move"))?;
        game.play(mv)
            .map_err(|e| GameError::refused(index, entry, e))?;
    }
    Ok(state(&game))
}

/// Sente (Black in the rules crate) is seat 0.
fn seat(side: Side) -> Seat {
    Seat::of(Variant::Shogi, if side == Side::Black { 0 } else { 1 })
}

/// `shogi-sP` for a Sente pawn, `shogi-gpR` for a Gote dragon: a promoted
/// piece is its base letter after a lower-case `p`, since `+` cannot go in an id.
fn sprite(piece: Piece) -> String {
    let side = if piece.side == Side::Black { 's' } else { 'g' };
    let letter = piece.role.letter();
    match letter.strip_prefix('+') {
        Some(base) => format!("shogi-{side}p{base}"),
        None => format!("shogi-{side}{letter}"),
    }
}

fn mark(square: Square, kind: HighlightKind) -> Highlight {
    Highlight {
        x: square.file(),
        y: square.rank(),
        kind,
    }
}

fn state(game: &Game) -> State {
    let position = game.position();
    let to_move = position.to_move();
    let over = game.outcome().is_some();

    let moves = game
        .legal_moves()
        .into_iter()
        .map(|mv| {
            let (from, drop) = match mv {
                Move::Board { from, .. } => (Some(from.name()), None),
                Move::Drop { role, .. } => (None, Some(sprite(Piece::new(to_move, role)))),
            };
            LegalMove {
                wire: mv.to_usi(),
                notation: mv.to_usi(),
                from,
                to: Some(mv.to().name()),
                drop,
            }
        })
        .collect();

    let pieces = (0..SQUARES as u8)
        .map(Square::at)
        .filter_map(|square| {
            position.piece_at(square).map(|piece| Placed {
                x: square.file(),
                y: square.rank(),
                square: square.name(),
                sprite: sprite(piece),
                count: 1,
            })
        })
        .collect();

    let mut highlights = Vec::new();
    if let Some(last) = game.moves().last() {
        if let Move::Board { from, .. } = last {
            highlights.push(mark(*from, HighlightKind::LastMove));
        }
        highlights.push(mark(last.to(), HighlightKind::LastMove));
    }
    let check = position.is_check();
    if check && let Some(king) = position.king_square(to_move) {
        highlights.push(mark(king, HighlightKind::Check));
    }

    let hands = Side::ALL
        .into_iter()
        .map(|side| Hand {
            seat: seat(side).index,
            pieces: game
                .hand(side)
                .into_iter()
                .map(|(role, count)| HandPiece {
                    sprite: sprite(Piece::new(side, role)),
                    count,
                })
                .collect(),
        })
        .collect();

    let info = Variant::Shogi.info();
    State {
        variant: Variant::Shogi,
        position: position.to_sfen(),
        to_move: (!over).then(|| seat(to_move)),
        awaiting: if over {
            Awaiting::Nothing
        } else {
            Awaiting::Move
        },
        moves,
        targets: BTreeMap::new(),
        throws: Vec::new(),
        history: game.moves().iter().map(|mv| mv.to_usi()).collect(),
        check,
        outcome: game.outcome().map(outcome_of),
        board: Board {
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

fn outcome_of(outcome: ShogiOutcome) -> Outcome {
    match outcome {
        ShogiOutcome::Win { winner, reason } => Outcome {
            winner: Some(seat(winner)),
            // Stalemate is a loss in shogi, so it is named for what happened.
            reason: match reason {
                rules_shogi::WinReason::Stalemate => "noMoves".to_string(),
                other => camel(&format!("{other:?}")),
            },
            result: if winner == Side::Black { "1-0" } else { "0-1" }.to_string(),
        },
        ShogiOutcome::Draw { reason } => Outcome {
            winner: None,
            reason: match reason {
                rules_shogi::DrawReason::Repetition => "sennichite".to_string(),
                other => camel(&format!("{other:?}")),
            },
            result: "½-½".to_string(),
        },
    }
}
