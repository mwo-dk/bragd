//! The Royal Game of Ur, through `rules-ur`.
//!
//! The record alternates throws (`0` to `4`, the marked dice) and moves
//! (`from-to` in steps along the mover's path: `0` is waiting, `15` is home).
//! A rosette earns another throw, so the sides do not simply alternate. A throw
//! that allows nothing passes the turn by itself.
//!
//! The initial position is who throws first (`light` or `dark`) or the crate's
//! own picture; with none, Light throws first.
//!
//! On the drawn board `x` is the column (0 to 7) and `y` the row (0 to 2), with
//! Light's private row at `y = 0`. Cells are named by their index in the
//! crate's board order, `0` to `19`; `waiting` and `home` name the two ends.

use crate::{
    Awaiting, Board, GameError, Hand, HandPiece, Highlight, HighlightKind, LegalMove, Outcome,
    Placed, Seat, State, Variant,
};
use rules_ur::board::square_at;
use rules_ur::{BOARD, Entry, Game, Needed, Position, ROSETTES, SQUARES, Side, Spot, WinReason};
use std::collections::BTreeMap;

pub(crate) fn start() -> String {
    Position::starting_with(Side::White).to_picture()
}

fn opening(text: &str) -> Result<Position, GameError> {
    match text.trim().to_ascii_lowercase().as_str() {
        "light" | "white" => Ok(Position::starting_with(Side::White)),
        "dark" | "black" => Ok(Position::starting_with(Side::Black)),
        _ => Position::from_picture(text)
            .ok_or_else(|| GameError::bad_position(format!("cannot read {text:?}"))),
    }
}

pub(crate) fn replay(initial: Option<&str>, entries: &[&str]) -> Result<State, GameError> {
    let position = initial
        .map(opening)
        .transpose()?
        .unwrap_or_else(|| Position::starting_with(Side::White));
    let mut game = Game::from_position(position);
    let mut last = None;
    for (index, entry) in entries.iter().enumerate() {
        let mover = game.position().to_move();
        game.play_wire(entry)
            .map_err(|e| GameError::refused(index, entry, e))?;
        if let Some(Entry::Move(chosen)) = game.entries().last().copied() {
            last = Some((mover, chosen));
        }
    }
    Ok(state(&game, last))
}

fn seat(side: Side) -> Seat {
    Seat::of(Variant::Ur, if side == Side::White { 0 } else { 1 })
}

pub(crate) fn sprite(side: Side) -> &'static str {
    match side {
        Side::White => "ur-l",
        Side::Black => "ur-d",
    }
}

fn cell(side: Side, spot: Spot) -> String {
    match spot {
        Spot::Waiting => "waiting".to_string(),
        Spot::Off => "home".to_string(),
        Spot::Step(_) => square_at(side, spot)
            .expect("a step is on the board")
            .to_string(),
    }
}

fn xy(square: usize) -> (u8, u8) {
    (BOARD[square].column, BOARD[square].row)
}

fn state(game: &Game, last: Option<(Side, rules_ur::Move)>) -> State {
    let position = game.position();
    let mover = position.to_move();
    let needs = game.needs();

    let (awaiting, throws) = match needs {
        Needed::Throw { .. } => (Awaiting::Throw, (0..=4).map(|n| n.to_string()).collect()),
        Needed::Move { .. } => (Awaiting::Move, Vec::new()),
        Needed::Nothing => (Awaiting::Nothing, Vec::new()),
    };
    let moves = game
        .moves()
        .iter()
        .map(|offer| LegalMove {
            wire: offer.chosen.to_wire(),
            notation: offer.chosen.to_wire(),
            from: Some(cell(mover, offer.chosen.from)),
            to: Some(cell(mover, offer.chosen.to)),
            drop: None,
        })
        .collect();

    let mut pieces = Vec::new();
    for square in 0..SQUARES {
        if let Some(side) = position.occupant(square) {
            let (x, y) = xy(square);
            pieces.push(Placed {
                x,
                y,
                square: square.to_string(),
                sprite: sprite(side).to_string(),
                count: 1,
            });
        }
    }

    let mut highlights: Vec<Highlight> = ROSETTES
        .iter()
        .map(|&square| {
            let (x, y) = xy(square);
            Highlight {
                x,
                y,
                kind: HighlightKind::Marked,
            }
        })
        .collect();
    if let Some((side, chosen)) = last {
        for spot in [chosen.from, chosen.to] {
            if let Some(square) = square_at(side, spot) {
                let (x, y) = xy(square);
                highlights.push(Highlight {
                    x,
                    y,
                    kind: HighlightKind::LastMove,
                });
            }
        }
    }

    let per_side = |count: fn(&Position, Side) -> u8| {
        [Side::White, Side::Black]
            .into_iter()
            .map(|side| Hand {
                seat: seat(side).index,
                pieces: vec![HandPiece {
                    sprite: sprite(side).to_string(),
                    count: count(position, side),
                }],
            })
            .collect::<Vec<_>>()
    };

    let info = Variant::Ur.info();
    State {
        variant: Variant::Ur,
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
            geometry: info.geometry,
            files: info.files,
            ranks: info.ranks,
            chequered: info.chequered,
            pieces,
            highlights,
            hands: per_side(Position::waiting),
            off: per_side(Position::off),
            dice: game
                .pending_throw()
                .map(|t| vec![t.steps()])
                .unwrap_or_default(),
            ..Board::default()
        },
    }
}
