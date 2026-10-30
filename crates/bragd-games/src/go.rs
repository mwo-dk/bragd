//! Go on three boards, under Chinese rules, through `rules-go`.
//!
//! Entries are a point (`d4`), `pass`, and after two passes the scoring
//! negotiation: `dead:c3,d4` proposes the whole set of dead stones (`dead:`
//! proposes none), `accept` agrees to the standing proposal and `resume` goes
//! back to play. A position is a picture and a side to move, with an optional
//! komi in halves: `X` black, `O` white, `.` empty, top row first, such as
//! `........./.../......... b 15`.

use crate::{
    Awaiting, Board, GameError, Highlight, HighlightKind, LegalMove, Outcome, Placed, Seat, State,
    Variant,
};
use rules_go::{Colour, Entry, Game, Move, Outcome as GoOutcome, Point, Position, Ruleset};
use std::collections::BTreeMap;

fn rules(variant: Variant) -> Ruleset {
    Ruleset::CHINESE.on(variant.info().files)
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
        Some(text) => {
            let (position, komi) = parse_position(text, rules.size)?;
            Game::from_position(komi.map_or(rules, |k| rules.with_komi(k)), position)
        }
        None => Game::new(rules),
    };
    for (index, entry) in entries.iter().enumerate() {
        game.play_wire(entry)
            .map_err(|e| GameError::refused(index, entry, e))?;
    }
    Ok(state(variant, &game))
}

/// Black moves first and is seat 0.
fn seat(variant: Variant, colour: Colour) -> Seat {
    Seat::of(variant, colour.index() as u8)
}

fn picture(position: &Position) -> String {
    let board = position.board();
    let size = board.size();
    let rows: Vec<String> = (0..size)
        .rev()
        .map(|row| {
            (0..size)
                .map(|column| match board.at(Point::at(column, row)) {
                    Some(Colour::Black) => 'X',
                    Some(Colour::White) => 'O',
                    None => '.',
                })
                .collect()
        })
        .collect();
    let side = if position.to_move() == Colour::Black {
        'b'
    } else {
        'w'
    };
    format!("{} {side}", rows.join("/"))
}

/// Reads a picture, refusing anything `Position::from_picture` would panic on.
fn parse_position(text: &str, size: u8) -> Result<(Position, Option<i16>), GameError> {
    let mut fields = text.split_whitespace();
    let picture = fields
        .next()
        .ok_or_else(|| GameError::bad_position("empty"))?;
    let to_move = match fields.next() {
        None | Some("b") => Colour::Black,
        Some("w") => Colour::White,
        Some(other) => {
            return Err(GameError::bad_position(format!(
                "{other:?} is not a colour"
            )));
        }
    };
    let komi = match fields.next() {
        None => None,
        Some(field) => {
            let halves: i16 = field.parse().map_err(|_| {
                GameError::bad_position(format!("{field:?} is not a komi in halves"))
            })?;
            if i32::from(halves).abs() > 2 * i32::from(size) * i32::from(size) {
                return Err(GameError::bad_position("komi larger than the whole board"));
            }
            Some(halves)
        }
    };
    if fields.next().is_some() {
        return Err(GameError::bad_position(
            "a picture, a colour and a komi at most",
        ));
    }
    let rows: Vec<&str> = picture.split('/').collect();
    if rows.len() != size as usize {
        return Err(GameError::bad_position(format!("a board has {size} rows")));
    }
    for row in &rows {
        if row.chars().count() != size as usize
            || !row
                .chars()
                .all(|c| matches!(c, '.' | '+' | 'X' | 'x' | '#' | 'O' | 'o'))
        {
            return Err(GameError::bad_position(format!(
                "{row:?} is not a row of {size} points"
            )));
        }
    }
    Ok((Position::from_picture(&rows, to_move), komi))
}

/// The conventional star points: corners and centre, and the sides on 19×19.
fn star_points(size: u8) -> Vec<(u8, u8)> {
    let edge = if size == 9 { 2 } else { 3 };
    let lines = [edge, size / 2, size - 1 - edge];
    let mut points = Vec::new();
    for x in lines {
        for y in lines {
            let corner_or_centre = (x != size / 2 && y != size / 2) || (x == y);
            if size == 19 || corner_or_centre {
                points.push((x, y));
            }
        }
    }
    points
}

fn state(variant: Variant, game: &Game) -> State {
    let position = game.position();
    let board = position.board();
    let over = game.outcome().is_some();

    let mut moves: Vec<LegalMove> = game
        .legal_moves()
        .into_iter()
        .map(|mv| match mv {
            Move::Play(point) => LegalMove {
                wire: point.name(),
                notation: point.name(),
                from: None,
                to: Some(point.name()),
                drop: None,
            },
            Move::Pass => LegalMove::plain("pass".to_string()),
        })
        .collect();
    let available = game.available();
    // `dead:` stands for the form: the front end fills in the set of stones.
    for (open, wire) in [
        (available.mark, "dead:"),
        (available.accept, "accept"),
        (available.resume, "resume"),
    ] {
        if open {
            moves.push(LegalMove::plain(wire.to_string()));
        }
    }

    let pieces = board
        .points()
        .filter_map(|point| {
            board.at(point).map(|colour| Placed {
                x: point.column(),
                y: point.row(),
                square: point.name(),
                sprite: if colour == Colour::Black {
                    "go-b"
                } else {
                    "go-w"
                }
                .to_string(),
                count: 1,
            })
        })
        .collect();

    let mut highlights: Vec<Highlight> = star_points(board.size())
        .into_iter()
        .map(|(x, y)| Highlight {
            x,
            y,
            kind: HighlightKind::Marked,
        })
        .collect();
    if let Some(Entry::Move(Move::Play(point))) = game.entries().last() {
        highlights.push(Highlight {
            x: point.column(),
            y: point.row(),
            kind: HighlightKind::LastMove,
        });
    }

    let info = variant.info();
    State {
        variant,
        position: picture(position),
        to_move: (!over).then(|| seat(variant, game.to_move())),
        awaiting: if over {
            Awaiting::Nothing
        } else {
            Awaiting::Move
        },
        moves,
        targets: BTreeMap::new(),
        throws: Vec::new(),
        history: game.entries().iter().map(Entry::to_wire).collect(),
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

/// `B+3.5`, `W+R` or `Jigo`, the way Go results are written.
fn outcome_of(variant: Variant, outcome: GoOutcome) -> Outcome {
    let letter = |colour: Colour| if colour == Colour::Black { 'B' } else { 'W' };
    let (reason, result) = match outcome {
        GoOutcome::Counted { score, winner } => {
            let margin = score.margin_halves();
            let points = if margin % 2 == 0 {
                format!("{}", margin / 2)
            } else {
                format!("{}.5", margin / 2)
            };
            let result = match winner {
                Some(colour) => format!("{}+{points}", letter(colour)),
                None => "Jigo".to_string(),
            };
            ("counted", result)
        }
        GoOutcome::Resigned { winner } => ("resignation", format!("{}+R", letter(winner))),
    };
    Outcome {
        winner: outcome.winner().map(|colour| seat(variant, colour)),
        reason: reason.to_string(),
        result,
    }
}
