//! The four tafl Variants, through `rules-tafl`.
//!
//! A move is its two squares run together, `f6f9`. A position is a picture of
//! the board, top rank first, rows joined by `/`: `X` attacker, `O` defender,
//! `K` king, `.` empty. The attackers always move first from it.

use crate::{
    Awaiting, Board, GameError, Highlight, HighlightKind, LegalMove, Outcome, Placed, Seat, State,
    Variant,
};
use rules_tafl::{Game, Outcome as TaflOutcome, Position, Role, Ruleset, Side, Square};
use std::collections::BTreeMap;

fn rules(variant: Variant) -> Ruleset {
    Ruleset::from_key(variant.key()).expect("every tafl Variant key is a rules-tafl key")
}

pub(crate) fn start(variant: Variant) -> String {
    picture(&rules(variant).starting_position())
}

pub(crate) fn replay(
    variant: Variant,
    initial: Option<&str>,
    entries: &[&str],
) -> Result<State, GameError> {
    let rules = rules(variant);
    let mut game = match initial {
        Some(text) => Game::from_position(rules, parse_picture(text, rules.size)?),
        None => Game::new(rules),
    };
    for (index, entry) in entries.iter().enumerate() {
        game.play_wire(entry)
            .map_err(|e| GameError::refused(index, entry, e))?;
    }
    Ok(state(variant, &game))
}

fn picture(position: &Position) -> String {
    let size = position.size();
    (0..size)
        .rev()
        .map(|rank| {
            (0..size)
                .map(|file| {
                    let square = Square::from_file_rank(file, rank).expect("on the board");
                    match position.piece_at(square) {
                        Some(piece) if piece.side == Side::Attackers => 'X',
                        Some(piece) if piece.role == Role::King => 'K',
                        Some(_) => 'O',
                        None => '.',
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Checks a stored picture before the crate reads it, because the crate panics on a bad one.
fn parse_picture(text: &str, size: u8) -> Result<Position, GameError> {
    let rows: Vec<&str> = text.split('/').collect();
    if rows.len() != size as usize {
        return Err(GameError::bad_position(format!(
            "this board has {size} rows, not {}",
            rows.len()
        )));
    }
    let mut kings = 0;
    for row in &rows {
        if row.chars().count() != size as usize {
            return Err(GameError::bad_position(format!(
                "{row:?} is not {size} squares wide"
            )));
        }
        for character in row.chars() {
            match character {
                'X' | 'O' | '.' => {}
                'K' => kings += 1,
                other => {
                    return Err(GameError::bad_position(format!("{other:?} is not a piece")));
                }
            }
        }
    }
    if kings != 1 {
        return Err(GameError::bad_position(format!(
            "a board has one king, not {kings}"
        )));
    }
    Ok(Position::from_picture(&rows, Side::Attackers))
}

fn seat(variant: Variant, side: Side) -> Seat {
    Seat::of(variant, side.index() as u8)
}

fn highlight(square: Square, kind: HighlightKind) -> Highlight {
    Highlight {
        x: square.file(),
        y: square.rank(),
        kind,
    }
}

fn state(variant: Variant, game: &Game) -> State {
    let position = game.position();
    let size = position.size();
    let over = game.outcome().is_some();

    let moves = game
        .legal_moves()
        .into_iter()
        .map(|mv| LegalMove {
            wire: mv.to_wire(),
            notation: mv.to_wire(),
            from: Some(mv.from.to_string()),
            to: Some(mv.to.to_string()),
            drop: None,
        })
        .collect();

    let mut pieces = Vec::new();
    for rank in 0..size {
        for file in 0..size {
            let square = Square::from_file_rank(file, rank).expect("on the board");
            if let Some(piece) = position.piece_at(square) {
                let sprite = match (piece.side, piece.role) {
                    (Side::Attackers, _) => "tafl-aS",
                    (Side::Defenders, Role::King) => "tafl-dK",
                    (Side::Defenders, _) => "tafl-dS",
                };
                pieces.push(Placed {
                    x: file,
                    y: rank,
                    square: square.to_string(),
                    sprite: sprite.to_string(),
                    count: 1,
                });
            }
        }
    }

    let mut highlights: Vec<Highlight> = std::iter::once(position.throne())
        .chain(position.corners())
        .map(|square| highlight(square, HighlightKind::Marked))
        .collect();
    if let Some(last) = game.moves().last() {
        highlights.push(highlight(last.from, HighlightKind::LastMove));
        highlights.push(highlight(last.to, HighlightKind::LastMove));
    }

    let info = variant.info();
    State {
        variant,
        position: picture(position),
        to_move: (!over).then(|| seat(variant, position.to_move())),
        awaiting: if over { Awaiting::Nothing } else { Awaiting::Move },
        moves,
        targets: BTreeMap::new(),
        throws: Vec::new(),
        history: game.moves().iter().map(|mv| mv.to_wire()).collect(),
        check: false,
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

fn outcome_of(variant: Variant, outcome: TaflOutcome) -> Outcome {
    let reason = match outcome {
        TaflOutcome::Attackers(win) => format!("{win:?}"),
        TaflOutcome::Defenders(win) => format!("{win:?}"),
        TaflOutcome::Draw(reason) => format!("{reason:?}"),
    };
    let winner = outcome.winner();
    Outcome {
        winner: winner.map(|side| seat(variant, side)),
        reason: crate::chess::camel(&reason),
        result: match winner {
            Some(Side::Attackers) => "1-0",
            Some(Side::Defenders) => "0-1",
            None => "1/2-1/2",
        }
        .to_string(),
    }
}
