//! Senet, through `rules-senet`.
//!
//! The record alternates throws (`1` to `5`, the sticks) and moves (`from-to`
//! in house numbers, `from-off` to bear off, a leading `<` for a move forced
//! backwards). A throw of 1, 4 or 5 earns another, so the sides do not simply
//! alternate. A throw that allows nothing passes the turn by itself.
//!
//! Evens always move first. The initial position is the crate's own picture,
//! or none for the opening array.
//!
//! On the drawn board the thirty houses run boustrophedon over three rows of
//! ten: `x` is the column (0 to 9) and `y` the row counted from the bottom, so
//! house 1 is top left. Cells are named by house number; `off` is borne off.

use crate::{
    Awaiting, Board, GameError, Hand, HandPiece, Highlight, HighlightKind, Label, LegalMove,
    Outcome, Placed, Seat, State, Variant,
};
use rules_senet::board::{grid, house_name};
use rules_senet::{Entry, Game, HOUSES, Needed, Position, ROWS, Side, Spot, WinReason};
use std::collections::BTreeMap;

pub(crate) fn start() -> String {
    Position::starting().to_picture()
}

pub(crate) fn replay(initial: Option<&str>, entries: &[&str]) -> Result<State, GameError> {
    let position = match initial {
        Some(text) => Position::from_picture(text)
            .ok_or_else(|| GameError::bad_position(format!("cannot read {text:?}")))?,
        None => Position::starting(),
    };
    let mut game = Game::from_position(position);
    let mut last = None;
    for (index, entry) in entries.iter().enumerate() {
        game.play_wire(entry)
            .map_err(|e| GameError::refused(index, entry, e))?;
        if let Some(Entry::Move(chosen)) = game.entries().last().copied() {
            last = Some(chosen);
        }
    }
    Ok(state(&game, last))
}

fn seat(side: Side) -> Seat {
    Seat::of(Variant::Senet, if side == Side::White { 0 } else { 1 })
}

pub(crate) fn sprite(side: Side) -> &'static str {
    match side {
        Side::White => "senet-e",
        Side::Black => "senet-o",
    }
}

fn xy(house: u8) -> (u8, u8) {
    let (row, column) = grid(house).expect("houses run 1 to 30");
    (column, ROWS as u8 - 1 - row)
}

fn spot_name(spot: Spot) -> String {
    match spot {
        Spot::House(house) => house.to_string(),
        Spot::Off => "off".to_string(),
    }
}

fn state(game: &Game, last: Option<rules_senet::Move>) -> State {
    let position = game.position();
    let needs = game.needs();

    let (awaiting, throws) = match needs {
        Needed::Throw { .. } => (Awaiting::Throw, (1..=5).map(|n| n.to_string()).collect()),
        Needed::Move { .. } => (Awaiting::Move, Vec::new()),
        Needed::Nothing => (Awaiting::Nothing, Vec::new()),
    };
    let moves = game
        .moves()
        .iter()
        .map(|offer| LegalMove {
            wire: offer.chosen.to_wire(),
            notation: offer.chosen.to_wire(),
            from: Some(offer.chosen.from.to_string()),
            to: Some(spot_name(offer.chosen.to)),
            drop: None,
        })
        .collect();

    let mut pieces = Vec::new();
    let mut highlights = Vec::new();
    let mut labels = Vec::new();
    for house in 1..=HOUSES as u8 {
        let (x, y) = xy(house);
        labels.push(Label {
            x,
            y,
            text: house.to_string(),
        });
        if house_name(house).is_some() {
            highlights.push(Highlight {
                x,
                y,
                kind: HighlightKind::Marked,
            });
        }
        if let Some(side) = position.occupant(house) {
            pieces.push(Placed {
                x,
                y,
                square: house.to_string(),
                sprite: sprite(side).to_string(),
                count: 1,
            });
        }
    }
    if let Some(chosen) = last {
        let to = match chosen.to {
            Spot::House(house) => Some(house),
            Spot::Off => None,
        };
        for house in [Some(chosen.from), to].into_iter().flatten() {
            let (x, y) = xy(house);
            highlights.push(Highlight {
                x,
                y,
                kind: HighlightKind::LastMove,
            });
        }
    }

    let off = [Side::White, Side::Black]
        .into_iter()
        .map(|side| Hand {
            seat: seat(side).index,
            pieces: vec![HandPiece {
                sprite: sprite(side).to_string(),
                count: position.off(side),
            }],
        })
        .collect();

    let info = Variant::Senet.info();
    State {
        variant: Variant::Senet,
        position: position.to_picture(),
        to_move: needs.side().map(seat),
        awaiting,
        moves,
        targets: BTreeMap::new(),
        throws,
        history: game.entries().iter().map(|entry| entry.to_wire()).collect(),
        check: false,
        outcome: game.outcome().map(|outcome| Outcome {
            winner: Some(seat(outcome.winner)),
            reason: match outcome.reason {
                WinReason::Home => "home",
                WinReason::Resignation => "resignation",
            }
            .to_string(),
            result: if outcome.winner == Side::White {
                "1-0"
            } else {
                "0-1"
            }
            .to_string(),
        }),
        board: Board {
            cells: (1..=HOUSES as u8).map(|h| { let (x, y) = xy(h); crate::Cell { x, y, square: h.to_string() } }).collect(),
            geometry: info.geometry,
            files: info.files,
            ranks: info.ranks,
            chequered: info.chequered,
            pieces,
            highlights,
            labels,
            off,
            dice: game
                .pending_throw()
                .map(|t| vec![t.steps()])
                .unwrap_or_default(),
            ..Board::default()
        },
    }
}
