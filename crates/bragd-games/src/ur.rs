//! The Royal Game of Ur, through `rules-ur`.
//!
//! Four Variants, one per rule set of `rules-ur`: Finkel's (`ur`), Masters'
//! (`ur-masters`), Blitz (`ur-blitz`) and Aseb (`ur-aseb`).
//!
//! The record alternates throws (the count of marked dice) and moves
//! (`from-to` in steps along the mover's path: `0` is waiting, one past the
//! path's length is home).
//! A rosette earns another throw, so the sides do not simply alternate. A throw
//! that allows nothing passes the turn by itself.
//!
//! The initial position is who throws first (`light` or `dark`) or the crate's
//! own picture; with none, Light throws first.
//!
//! On the drawn board `x` is the column (0 to 7, or 0 to 11 for Aseb) and `y` the row (0 to 2), with
//! Light's private row at `y = 0`. Cells are named by their index in the
//! crate's board order, `0` to `19`; `waiting` and `home` name the two ends.

use crate::{
    Awaiting, Board, GameError, Hand, HandPiece, Highlight, HighlightKind, LegalMove, Outcome,
    Placed, Seat, State, Variant,
};
use rules_ur::{Entry, Game, Needed, Position, Rules, Side, Spot, Throw, WinReason};
use std::collections::BTreeMap;

/// The rule set a Variant is played under.
pub(crate) fn rules(variant: Variant) -> &'static Rules {
    match variant {
        Variant::UrMasters => &rules_ur::MASTERS,
        Variant::UrBlitz => &rules_ur::BLITZ,
        Variant::UrAseb => &rules_ur::ASEB,
        _ => &rules_ur::FINKEL,
    }
}

pub(crate) fn start(variant: Variant) -> String {
    Position::starting_with(rules(variant), Side::White).to_picture()
}

fn opening(rules: &'static Rules, text: &str) -> Result<Position, GameError> {
    match text.trim().to_ascii_lowercase().as_str() {
        "light" | "white" => Ok(Position::starting_with(rules, Side::White)),
        "dark" | "black" => Ok(Position::starting_with(rules, Side::Black)),
        _ => Position::from_picture(rules, text)
            .ok_or_else(|| GameError::bad_position(format!("cannot read {text:?}"))),
    }
}

pub(crate) fn game(variant: Variant, initial: Option<&str>, entries: &[&str]) -> Result<(Game, Option<(Side, rules_ur::Move)>), GameError> {
    let rules = rules(variant);
    let position = initial
        .map(|text| opening(rules, text))
        .transpose()?
        .unwrap_or_else(|| Position::starting_with(rules, Side::White));
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
    Ok((game, last))
}

pub(crate) fn replay(variant: Variant, initial: Option<&str>, entries: &[&str]) -> Result<State, GameError> {
    let (game, last) = game(variant, initial, entries)?;
    Ok(state(variant, &game, last))
}

/// The move Bragd's Bot plays now, or `None` when the Game is not waiting for a move.
pub(crate) fn bot_move(variant: Variant, initial: Option<&str>, entries: &[&str]) -> Result<Option<String>, GameError> {
    let (game, _) = game(variant, initial, entries)?;
    let rules = game.rules();
    Ok(game
        .pending_throw()
        .and_then(|throw| rules_ur::bot::choose(game.position(), throw))
        .map(|chosen| chosen.to_wire(rules)))
}

fn seat(variant: Variant, side: Side) -> Seat {
    Seat::of(variant, if side == Side::White { 0 } else { 1 })
}

pub(crate) fn sprite(side: Side) -> &'static str {
    match side {
        Side::White => "ur-l",
        Side::Black => "ur-d",
    }
}

fn cell(rules: &Rules, side: Side, spot: Spot) -> String {
    match spot {
        Spot::Waiting => "waiting".to_string(),
        Spot::Off => "home".to_string(),
        Spot::Step(_) => rules
            .square_at(side, spot)
            .expect("a step is on the board")
            .to_string(),
    }
}

fn xy(rules: &Rules, square: usize) -> (u8, u8) {
    (rules.board[square].column, rules.board[square].row)
}

/// Which of the dice show a marked tip, for a throw of `marks`. Which ones is
/// not recorded, only how many, so they are spread by a fixed turn of the entry
/// number: the same record always draws the same dice.
fn pyramids(dice: u8, throw: Throw, entry: usize) -> Vec<bool> {
    let turn = entry * 3;
    (0..dice as usize)
        .map(|die| (die + turn) % dice as usize)
        .map(|place| place < throw.marks() as usize)
        .collect()
}

fn state(variant: Variant, game: &Game, last: Option<(Side, rules_ur::Move)>) -> State {
    let rules = game.rules();
    let position = game.position();
    let mover = position.to_move();
    let needs = game.needs();

    let (awaiting, throws) = match needs {
        Needed::Throw { .. } => (Awaiting::Throw, rules.throws().map(|t| t.to_wire()).collect()),
        Needed::Move { .. } => (Awaiting::Move, Vec::new()),
        Needed::Nothing => (Awaiting::Nothing, Vec::new()),
    };
    let moves = game
        .moves()
        .iter()
        .map(|offer| LegalMove {
            wire: offer.chosen.to_wire(rules),
            notation: offer.chosen.to_wire(rules),
            from: Some(cell(rules, mover, offer.chosen.from)),
            to: Some(cell(rules, mover, offer.chosen.to)),
            drop: None,
        })
        .collect();

    let mut pieces = Vec::new();
    for square in 0..rules.board.len() {
        if let Some(side) = position.occupant(square) {
            let (x, y) = xy(rules, square);
            pieces.push(Placed {
                x,
                y,
                square: square.to_string(),
                sprite: sprite(side).to_string(),
                count: 1,
            });
        }
    }

    let mut highlights: Vec<Highlight> = rules
        .rosettes
        .iter()
        .map(|&square| {
            let (x, y) = xy(rules, square);
            Highlight {
                x,
                y,
                kind: HighlightKind::Marked,
            }
        })
        .collect();
    if let Some((side, chosen)) = last {
        for spot in [chosen.from, chosen.to] {
            if let Some(square) = rules.square_at(side, spot) {
                let (x, y) = xy(rules, square);
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
                seat: seat(variant, side).index,
                pieces: vec![HandPiece {
                    sprite: sprite(side).to_string(),
                    count: count(position, side),
                }],
            })
            .collect::<Vec<_>>()
    };

    // The last throw in the record, and whether it was the record's last entry
    // with nothing to play: a throw that passed the turn by itself.
    let thrown = game.entries().iter().enumerate().rev().find_map(|(at, entry)| match entry {
        Entry::Throw(throw) => Some((at, *throw)),
        Entry::Move(_) => None,
    });
    let passed = thrown
        .filter(|&(at, _)| at + 1 == game.entries().len() && game.pending_throw().is_none())
        .map(|(_, throw)| rules.steps(throw).to_string());

    let info = variant.info();
    State {
        variant,
        position: position.to_picture(),
        to_move: needs.side().map(|side| seat(variant, side)),
        awaiting,
        moves,
        steps: Vec::new(),
        targets: BTreeMap::new(),
        throws,
        history: game.entries().iter().map(|entry| entry.to_wire(rules)).collect(),
        numbers: Vec::new(),
        check: false,
        forced: match game.moves().as_slice() {
            [only] => Some(only.chosen.to_wire(rules)),
            _ => None,
        },
        passed,
        outcome: game.outcome().map(|outcome| Outcome {
            winner: Some(seat(variant, outcome.winner)),
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
            cells: (0..rules.board.len()).map(|s| { let (x, y) = xy(rules, s); crate::Cell { x, y, square: s.to_string() } }).collect(),
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
                .map(|t| vec![rules.steps(t)])
                .unwrap_or_default(),
            pyramids: thrown.map(|(at, throw)| pyramids(rules.dice, throw, at)).unwrap_or_default(),
            paths: [Side::White, Side::Black]
                .into_iter()
                .map(|side| rules.path(side).iter().map(|&square| { let (x, y) = xy(rules, square); [x, y] }).collect())
                .collect(),
            ..Board::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pyramids_show_as_many_marks_as_the_throw_and_move_between_throws() {
        for marks in 0..=4 {
            let shown = pyramids(4, Throw::new(marks).unwrap(), 5);
            assert_eq!(shown.len(), 4);
            assert_eq!(shown.iter().filter(|&&m| m).count(), marks as usize);
        }
        assert_ne!(pyramids(4, Throw::new(1).unwrap(), 0), pyramids(4, Throw::new(1).unwrap(), 1));
        assert_eq!(pyramids(3, Throw::new(3).unwrap(), 2), [true; 3]);
    }
}
