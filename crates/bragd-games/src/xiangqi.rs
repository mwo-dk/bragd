//! Xiangqi and janggi, nine files by ten ranks of points, through `rules-xiangqi`.

use crate::chess::camel;
use crate::{
    Awaiting, Board, GameError, Highlight, HighlightKind, LegalMove, Outcome, Placed, Seat, State,
    Variant,
};
use rules_xiangqi::board::POINTS;
use rules_xiangqi::{Game, Point, Position, Ruleset, Side};
use std::collections::BTreeMap;

fn ruleset(variant: Variant) -> Ruleset {
    if variant == Variant::Janggi {
        Ruleset::Janggi
    } else {
        Ruleset::Xiangqi
    }
}

pub(crate) fn start(variant: Variant) -> String {
    ruleset(variant).starting_position().to_fen()
}

pub(crate) fn replay(
    variant: Variant,
    initial: Option<&str>,
    entries: &[&str],
) -> Result<State, GameError> {
    let rules = ruleset(variant);
    let position = match initial {
        Some(fen) => Position::from_fen_with(rules, fen)
            .ok_or_else(|| GameError::bad_position(format!("not a {} FEN", rules.key())))?,
        None => rules.starting_position(),
    };
    let mut game = Game::from_position(position);
    for (index, entry) in entries.iter().enumerate() {
        game.play_wire(entry)
            .map_err(|e| GameError::refused(index, entry, e))?;
    }
    Ok(state(variant, &game))
}

fn seat(variant: Variant, side: Side) -> Seat {
    Seat::of(variant, if side == Side::Red { 0 } else { 1 })
}

/// `xiangqi-rK`; janggi's pieces have their own sheet: `janggi-rK` is Cho, `janggi-bK` Han.
pub(crate) fn sprite(variant: Variant, side: Side, letter: char) -> String {
    let sheet = if variant == Variant::Janggi { "janggi" } else { "xiangqi" };
    let side = if side == Side::Red { 'r' } else { 'b' };
    format!("{sheet}-{side}{}", letter.to_ascii_uppercase())
}

fn highlight(point: Point, kind: HighlightKind) -> Highlight {
    Highlight {
        x: point.file(),
        y: point.rank(),
        kind,
    }
}

fn state(variant: Variant, game: &Game) -> State {
    let position = game.position();
    let over = game.outcome().is_some();
    let to_move = position.to_move();

    // The crate writes no figure notation, so the notation is the wire form.
    // A janggi pass is `e2e2`: from and to stay empty, so it never shows as a drag.
    let moves = game
        .legal_moves()
        .into_iter()
        .map(|mv| {
            let mut legal = LegalMove::plain(mv.to_wire());
            if mv.is_pass() {
                legal.notation = "pass".to_string();
            } else {
                legal.from = Some(mv.from.to_string());
                legal.to = Some(mv.to.to_string());
            }
            legal
        })
        .collect();

    let pieces = (0..POINTS as u8)
        .map(Point::at)
        .filter_map(|point| {
            position.piece_at(point).map(|piece| Placed {
                x: point.file(),
                y: point.rank(),
                square: point.to_string(),
                sprite: sprite(variant, piece.side, piece.role.letter()),
                count: 1,
            })
        })
        .collect();

    let mut highlights = Vec::new();
    if let Some(last) = game.moves().last().filter(|mv| !mv.is_pass()) {
        highlights.push(highlight(last.from, HighlightKind::LastMove));
        highlights.push(highlight(last.to, HighlightKind::LastMove));
    }
    let check = position.is_in_check(to_move);
    if check && let Some(general) = position.general(to_move) {
        highlights.push(highlight(general, HighlightKind::Check));
    }

    let info = variant.info();
    State {
        variant,
        position: position.to_fen(),
        to_move: (!over).then(|| seat(variant, to_move)),
        awaiting: if over { Awaiting::Nothing } else { Awaiting::Move },
        moves,
        targets: BTreeMap::new(),
        throws: Vec::new(),
        history: game
            .moves()
            .iter()
            .map(|mv| if mv.is_pass() { "pass".to_string() } else { mv.to_wire() })
            .collect(),
        check,
        outcome: game.outcome().map(|outcome| outcome_of(variant, outcome)),
        board: Board {
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

fn outcome_of(variant: Variant, outcome: rules_xiangqi::Outcome) -> Outcome {
    use rules_xiangqi::Outcome as O;
    match outcome {
        O::Win { winner, reason } => Outcome {
            winner: Some(seat(variant, winner)),
            reason: camel(&format!("{reason:?}")),
            result: if winner == Side::Red { "1-0" } else { "0-1" }.to_string(),
        },
        O::Draw { reason } => Outcome {
            winner: None,
            reason: camel(&format!("{reason:?}")),
            result: "1/2-1/2".to_string(),
        },
    }
}
