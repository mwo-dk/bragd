//! The four draughts Variants, through `rules-draughts`.
//!
//! Squares are named by their printed numbers. A move is written
//! `from-to` or `fromxto`, and a capture adds every taken square after a
//! colon (`23x14:18:9`), because two capture sequences can share both ends
//! and differ in what they take. Its notation is the same without the colons.

use crate::{
    Awaiting, Board, GameError, Highlight, HighlightKind, Label, LegalMove, Outcome, Placed, Seat,
    State, Variant,
};
use rules_draughts::game::Outcome as DraughtsOutcome;
use rules_draughts::{Game, Geometry, Move, Position, Ruleset, Side, Square};
use std::collections::BTreeMap;

fn rules(variant: Variant) -> Ruleset {
    Ruleset::from_key(variant.key()).expect("every draughts Variant key is a rules-draughts key")
}

pub(crate) fn start(variant: Variant) -> String {
    let rules = rules(variant);
    pdn(&Position::start(rules))
}

pub(crate) fn replay(
    variant: Variant,
    initial: Option<&str>,
    entries: &[&str],
) -> Result<State, GameError> {
    let rules = rules(variant);
    // shortcut: rules-draughts builds only its opening array, so any other initial position is refused; upgrade when the crate can build one.
    if let Some(text) = initial
        && text != start(variant)
    {
        return Err(GameError::bad_position(
            "draughts Games start from the opening array",
        ));
    }
    let mut game = Game::new(rules);
    let geometry = rules.geometry;
    for (index, entry) in entries.iter().enumerate() {
        let mv = game
            .legal_moves()
            .into_iter()
            .find(|&mv| wire(geometry, mv) == *entry)
            .ok_or_else(|| GameError::refused(index, entry, "not a legal move here"))?;
        game.play(mv)
            .map_err(|e| GameError::refused(index, entry, e))?;
    }
    Ok(state(variant, &game))
}

fn wire(geometry: Geometry, mv: Move) -> String {
    let mut text = notation(geometry, mv);
    for index in 0..geometry.squares() as u8 {
        let square = Square::new(index);
        if mv.captured.contains(square) {
            text.push(':');
            text.push_str(&geometry.printed_number(square).to_string());
        }
    }
    text
}

fn notation(geometry: Geometry, mv: Move) -> String {
    format!(
        "{}{}{}",
        geometry.printed_number(mv.from),
        if mv.is_capture() { "x" } else { "-" },
        geometry.printed_number(mv.to)
    )
}

/// The position in PDN's FEN form: side to move, then each side's squares, kings with `K`.
fn pdn(position: &Position) -> String {
    let geometry = position.geometry();
    let mut text = String::from(if position.to_move() == Side::White { "W" } else { "B" });
    for (side, letter) in [(Side::White, "W"), (Side::Black, "B")] {
        let mut squares: Vec<(u16, bool)> = (0..geometry.squares() as u8)
            .map(Square::new)
            .filter(|&square| position.side_at(square) == Some(side))
            .map(|square| (geometry.printed_number(square), position.is_king(square)))
            .collect();
        squares.sort();
        let list: Vec<String> = squares
            .into_iter()
            .map(|(number, king)| format!("{}{number}", if king { "K" } else { "" }))
            .collect();
        text.push(':');
        text.push_str(letter);
        text.push_str(&list.join(","));
    }
    text
}

fn seat(variant: Variant, side: Side) -> Seat {
    Seat::of(variant, side.index() as u8)
}

fn state(variant: Variant, game: &Game) -> State {
    let position = game.position();
    let geometry = position.geometry();
    let over = game.outcome().is_some();
    let name = |square: Square| geometry.printed_number(square).to_string();

    let moves = game
        .legal_moves()
        .into_iter()
        .map(|mv| LegalMove {
            wire: wire(geometry, mv),
            notation: notation(geometry, mv),
            from: Some(name(mv.from)),
            to: Some(name(mv.to)),
            drop: None,
        })
        .collect();

    let mut pieces = Vec::new();
    let mut labels = Vec::new();
    for index in 0..geometry.squares() as u8 {
        let square = Square::new(index);
        let (x, y) = geometry.file_rank(square);
        labels.push(Label {
            x,
            y,
            text: name(square),
        });
        if let Some(side) = position.side_at(square) {
            let colour = if side == Side::White { 'w' } else { 'b' };
            let kind = if position.is_king(square) { 'K' } else { 'M' };
            pieces.push(Placed {
                x,
                y,
                square: name(square),
                sprite: format!("draughts-{colour}{kind}"),
                count: 1,
            });
        }
    }

    let highlights = game
        .moves()
        .last()
        .map(|last| {
            [last.from, last.to]
                .into_iter()
                .map(|square| {
                    let (x, y) = geometry.file_rank(square);
                    Highlight {
                        x,
                        y,
                        kind: HighlightKind::LastMove,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let info = variant.info();
    State {
        variant,
        position: pdn(position),
        to_move: (!over).then(|| seat(variant, position.to_move())),
        awaiting: if over { Awaiting::Nothing } else { Awaiting::Move },
        moves,
        targets: BTreeMap::new(),
        throws: Vec::new(),
        history: game.moves().iter().map(|&mv| notation(geometry, mv)).collect(),
        check: false,
        outcome: game.outcome().map(|outcome| outcome_of(variant, outcome)),
        board: Board {
            cells: labels.iter().map(|l| crate::Cell { x: l.x, y: l.y, square: l.text.clone() }).collect(),
            geometry: info.geometry,
            files: info.files,
            ranks: info.ranks,
            chequered: info.chequered,
            pieces,
            highlights,
            labels,
            ..Board::default()
        },
    }
}

fn outcome_of(variant: Variant, outcome: DraughtsOutcome) -> Outcome {
    let (winner, reason) = match outcome {
        DraughtsOutcome::Win { winner, reason } => (Some(winner), format!("{reason:?}")),
        DraughtsOutcome::Draw { reason } => (None, format!("{reason:?}")),
    };
    Outcome {
        winner: winner.map(|side| seat(variant, side)),
        reason: crate::chess::camel(&reason),
        result: match winner {
            Some(Side::White) => "1-0",
            Some(Side::Black) => "0-1",
            None => "1/2-1/2",
        }
        .to_string(),
    }
}
