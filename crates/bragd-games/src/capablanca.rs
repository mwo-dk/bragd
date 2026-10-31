//! Capablanca chess, ten files by eight, through `rules-capablanca`.

use crate::chess::camel;
use crate::{
    Awaiting, Board, GameError, Highlight, HighlightKind, LegalMove, Outcome, Placed, Seat, State,
    Variant,
};
use rules_capablanca::{Color, Game, Position, Role, SQUARES, Square};
use std::collections::BTreeMap;

pub(crate) fn start() -> String {
    Position::starting().to_fen()
}

pub(crate) fn replay(initial: Option<&str>, entries: &[&str]) -> Result<State, GameError> {
    let mut game = match initial {
        Some(fen) => Game::from_position(
            Position::from_fen(fen).map_err(|e| GameError::bad_position(format!("{e:?}")))?,
        ),
        None => Game::start(),
    };
    let first = game.position().side_to_move();
    for (index, entry) in entries.iter().enumerate() {
        game.play_uci(entry)
            .map_err(|e| GameError::refused(index, entry, format!("{e:?}")))?;
    }
    let mut state = state(&game);
    // shortcut: numbering starts at move 1 whatever the FEN's move number; read it when a set-up position needs it.
    state.numbers = crate::full_moves(seat(first).index.into(), state.history.len());
    Ok(state)
}

fn seat(color: Color) -> Seat {
    Seat::of(Variant::Capablanca, if color == Color::White { 0 } else { 1 })
}

pub(crate) fn sprite(color: Color, role: Role) -> String {
    let side = if color == Color::White { 'w' } else { 'b' };
    format!("capablanca-{side}{}", role.letter().to_ascii_uppercase())
}

fn highlight(square: Square, kind: HighlightKind) -> Highlight {
    Highlight {
        x: square.file(),
        y: square.rank(),
        kind,
    }
}

fn state(game: &Game) -> State {
    let position = game.position();
    let over = game.outcome().is_some();
    let to_move = position.side_to_move();

    // The crate writes no SAN, so the notation is the coordinate form.
    let moves = game
        .legal_moves()
        .as_slice()
        .iter()
        .map(|mv| LegalMove {
            notation: mv.to_uci(),
            wire: mv.to_uci(),
            from: Some(mv.from.to_string()),
            to: Some(mv.to.to_string()),
            drop: None,
        })
        .collect();

    let pieces = (0..SQUARES as u8)
        .map(Square::at)
        .filter_map(|square| {
            position.piece_at(square).map(|piece| Placed {
                x: square.file(),
                y: square.rank(),
                square: square.to_string(),
                sprite: sprite(piece.color, piece.role),
                count: 1,
            })
        })
        .collect();

    let mut highlights = Vec::new();
    if let Some(last) = game.moves().last() {
        highlights.push(highlight(last.from, HighlightKind::LastMove));
        highlights.push(highlight(last.to, HighlightKind::LastMove));
    }
    let check = position.is_check();
    if check {
        highlights.push(highlight(position.king_square(to_move), HighlightKind::Check));
    }

    let info = Variant::Capablanca.info();
    State {
        variant: Variant::Capablanca,
        position: position.to_fen(),
        to_move: (!over).then(|| seat(to_move)),
        awaiting: if over { Awaiting::Nothing } else { Awaiting::Move },
        moves,
        targets: BTreeMap::new(),
        throws: Vec::new(),
        history: game.moves().iter().map(|mv| mv.to_uci()).collect(),
        numbers: Vec::new(),
        check,
        outcome: game.outcome().map(outcome_of),
        board: Board {
            cells: (0..SQUARES as u8).map(Square::at).map(|s| crate::Cell { x: s.file(), y: s.rank(), square: s.to_string() }).collect(),
            geometry: info.geometry,
            files: info.files,
            ranks: info.ranks,
            chequered: info.chequered,
            pieces,
            highlights,
            ..Board::default()
        },
    }
}

fn outcome_of(outcome: rules_capablanca::Outcome) -> Outcome {
    use rules_capablanca::Outcome as O;
    match outcome {
        O::Win { winner, reason } => Outcome {
            winner: Some(seat(winner)),
            reason: camel(&format!("{reason:?}")),
            result: if winner == Color::White { "1-0" } else { "0-1" }.to_string(),
        },
        O::Draw { reason } => Outcome {
            winner: None,
            reason: camel(&format!("{reason:?}")),
            result: "1/2-1/2".to_string(),
        },
    }
}
