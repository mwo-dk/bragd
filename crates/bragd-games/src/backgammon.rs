//! Backgammon, through `rules-backgammon`.
//!
//! The record is the crate's own entries: a roll (`5-3`), then the turn played
//! with it (`13/8 8/5`, `(no play)`), and the cube words `double`, `take` and
//! `drop`. A roll is written by whoever threw it; nothing here throws.
//!
//! The initial position names who the opening roll seated first, `white` or
//! `black`; with none, White moves first. The board has no standard text form,
//! so [`State::position`] is a picture for reading only:
//! `B1,-,…,W2 bar:0/0 off:0/0 cube:1- w`, points in White's numbering 1 to 24.
//!
//! On the drawn board `x` is the point number in White's numbering (1 to 24),
//! White's bar is `x = 0` and Black's bar is `x = 25`, and `y` is always 0.

use crate::{
    Awaiting, Board, CubeView, GameError, Hand, HandPiece, Highlight, HighlightKind, LegalMove,
    Outcome, Placed, Seat, State, Variant,
};
use rules_backgammon::board::{index_of, number_at};
use rules_backgammon::{
    Concession, Destination, Game, Needed, Origin, POINTS, Position, Side, Turn, WinReason,
};
use std::collections::BTreeMap;

pub(crate) fn start() -> String {
    "white".to_string()
}

fn first_mover(text: &str) -> Result<Side, GameError> {
    match text.trim().to_ascii_lowercase().as_str() {
        "white" | "w" => Ok(Side::White),
        "black" | "b" => Ok(Side::Black),
        other => Err(GameError::bad_position(format!(
            "{other:?} is not who moves first; say white or black"
        ))),
    }
}

pub(crate) fn replay(initial: Option<&str>, entries: &[&str]) -> Result<State, GameError> {
    let first = initial.map(first_mover).transpose()?.unwrap_or(Side::White);
    let mut game = Game::from_position(Position::starting_with(first));
    let mut resigned = None;
    for (index, entry) in entries.iter().enumerate() {
        if let Some(grade) = concession(entry) {
            let side = game
                .needs()
                .ok_or_else(|| GameError::refused(index, entry, "the Game is over"))?
                .side();
            game.resign(side, grade)
                .map_err(|e| GameError::refused(index, entry, e))?;
            resigned = Some(entry.to_string());
            continue;
        }
        game.play_wire(entry)
            .map_err(|e| GameError::refused(index, entry, e))?;
    }
    let mut state = state(&game);
    // A resignation is not one of the crate's entries, so the record carries it here.
    state.history.extend(resigned);
    Ok(state)
}

/// The three resignations, as entries: the side to act concedes at the grade it names.
const RESIGNATIONS: [(&str, Concession); 3] = [
    ("resign", Concession::Single),
    ("resign_gammon", Concession::Gammon),
    ("resign_backgammon", Concession::Backgammon),
];

fn concession(entry: &str) -> Option<Concession> {
    RESIGNATIONS.iter().find(|(word, _)| *word == entry.trim()).map(|&(_, grade)| grade)
}

fn seat(side: Side) -> Seat {
    Seat::of(Variant::Backgammon, side.index() as u8)
}

pub(crate) fn sprite(side: Side) -> &'static str {
    match side {
        Side::White => "backgammon-w",
        Side::Black => "backgammon-b",
    }
}

/// A place in White's numbering: a point number, `bar` or `off`.
fn origin(mover: Side, from: Origin) -> String {
    match from {
        Origin::Bar => "bar".to_string(),
        Origin::Point(n) => (index_of(mover, n) + 1).to_string(),
    }
}

fn destination(mover: Side, to: Destination) -> String {
    match to {
        Destination::Off => "off".to_string(),
        Destination::Point(n) => (index_of(mover, n) + 1).to_string(),
    }
}

fn bar_x(side: Side) -> u8 {
    match side {
        Side::White => 0,
        Side::Black => 25,
    }
}

fn turn_move(mover: Side, turn: &Turn) -> LegalMove {
    let first = turn.plays().first();
    LegalMove {
        wire: turn.to_string(),
        notation: turn.to_string(),
        from: first.map(|play| origin(mover, play.from)),
        to: first.map(|play| destination(mover, play.to)),
        drop: None,
    }
}

/// Every roll two dice can make, high die first: `6-6`, `6-5`, … `1-1`.
fn all_rolls() -> Vec<String> {
    (1..=6u8)
        .rev()
        .flat_map(|high| (1..=high).rev().map(move |low| format!("{high}-{low}")))
        .collect()
}

fn state(game: &Game) -> State {
    let position = game.position();
    let cube = game.cube();
    let needs = game.needs();

    let (awaiting, mut moves, throws) = match needs {
        None => (Awaiting::Nothing, Vec::new(), Vec::new()),
        Some(Needed::Roll { side }) => {
            let mut moves = Vec::new();
            if cube.may_double(side) {
                moves.push(LegalMove::plain("double".to_string()));
            }
            (Awaiting::Throw, moves, all_rolls())
        }
        Some(Needed::Turn { side, .. }) => (
            Awaiting::Move,
            game.turns()
                .iter()
                .map(|turn| turn_move(side, turn))
                .collect(),
            Vec::new(),
        ),
        Some(Needed::Answer { .. }) => (
            Awaiting::Move,
            vec![
                LegalMove::plain("take".to_string()),
                LegalMove::plain("drop".to_string()),
            ],
            Vec::new(),
        ),
    };

    // Offered on a roll or a turn; a Side answering a double concedes by dropping.
    if matches!(needs, Some(Needed::Roll { .. } | Needed::Turn { .. })) {
        moves.extend(RESIGNATIONS.iter().map(|(word, _)| LegalMove::plain(word.to_string())));
    }

    let mut pieces = Vec::new();
    for index in 0..POINTS {
        let point = position.point_at(index);
        if let Some(owner) = point.owner() {
            let number = number_at(Side::White, index);
            pieces.push(Placed {
                x: number,
                y: 0,
                square: number.to_string(),
                sprite: sprite(owner).to_string(),
                count: point.count(),
            });
        }
    }
    for side in Side::ALL {
        if position.bar(side) > 0 {
            pieces.push(Placed {
                x: bar_x(side),
                y: 0,
                square: "bar".to_string(),
                sprite: sprite(side).to_string(),
                count: position.bar(side),
            });
        }
    }

    let mut highlights = Vec::new();
    if let Some((mover, turn)) = game.last_turn() {
        for play in turn.plays() {
            let from = match play.from {
                Origin::Bar => Some(bar_x(mover)),
                Origin::Point(n) => Some(index_of(mover, n) as u8 + 1),
            };
            let to = match play.to {
                Destination::Point(n) => Some(index_of(mover, n) as u8 + 1),
                Destination::Off => None,
            };
            for x in [from, to].into_iter().flatten() {
                highlights.push(Highlight {
                    x,
                    y: 0,
                    kind: HighlightKind::LastMove,
                });
            }
        }
    }

    let off = Side::ALL
        .into_iter()
        .map(|side| Hand {
            seat: seat(side).index,
            pieces: vec![HandPiece {
                sprite: sprite(side).to_string(),
                count: position.off(side),
            }],
        })
        .collect();

    let info = Variant::Backgammon.info();
    State {
        variant: Variant::Backgammon,
        position: picture(game),
        to_move: needs.map(|needed| seat(needed.side())),
        awaiting,
        moves,
        targets: BTreeMap::new(),
        throws,
        history: game.entries().iter().map(|entry| entry.to_wire()).collect(),
        numbers: Vec::new(),
        check: false,
        outcome: game.outcome().map(|outcome| Outcome {
            winner: Some(seat(outcome.winner)),
            reason: match outcome.reason {
                WinReason::Single => "single",
                WinReason::Gammon => "gammon",
                WinReason::Backgammon => "backgammon",
                WinReason::DoubleDeclined => "doubleDeclined",
                WinReason::Resignation(_) => "resignation",
            }
            .to_string(),
            result: format!("{} points", outcome.points),
        }),
        board: Board {
            cells: (1..=POINTS as u8).map(|n| crate::Cell { x: n, y: 0, square: n.to_string() }).chain(Side::ALL.into_iter().map(|side| crate::Cell { x: bar_x(side), y: 0, square: "bar".to_string() })).collect(),
            geometry: info.geometry,
            files: info.files,
            ranks: info.ranks,
            chequered: info.chequered,
            pieces,
            highlights,
            off,
            cube: Some(CubeView {
                value: cube.value(),
                owner: cube.owner().map(|side| seat(side).index),
            }),
            dice: game
                .pending_roll()
                .map(|roll| roll.dice())
                .unwrap_or_default(),
            ..Board::default()
        },
    }
}

/// The board as one line, for reading: points in White's numbering 1 to 24.
fn picture(game: &Game) -> String {
    let position = game.position();
    let points: Vec<String> = (0..POINTS)
        .map(|index| {
            let point = position.point_at(index);
            match point.owner() {
                Some(Side::White) => format!("W{}", point.count()),
                Some(Side::Black) => format!("B{}", point.count()),
                None => "-".to_string(),
            }
        })
        .collect();
    let cube = game.cube();
    format!(
        "{} bar:{}/{} off:{}/{} cube:{}{} {}",
        points.join(","),
        position.bar(Side::White),
        position.bar(Side::Black),
        position.off(Side::White),
        position.off(Side::Black),
        cube.value(),
        match cube.owner() {
            Some(Side::White) => "w",
            Some(Side::Black) => "b",
            None => "-",
        },
        if position.to_move() == Side::White {
            "w"
        } else {
            "b"
        }
    )
}
