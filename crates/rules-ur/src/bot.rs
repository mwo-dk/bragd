//! Bragd's Bot for Ur: it chooses a move for the throw on the table.
//!
//! It looks one throw ahead. For each legal move it plays the move, then weighs
//! every throw that can come next by how likely it is, and assumes whoever throws
//! it picks their best reply by the same static count. That is enough to take a
//! piece when it can, keep off squares the other side reaches easily, and bear off
//! when that wins. It plays every rule set, because it asks the position and its
//! [`Rules`] everything and knows nothing of its own.
//!
//! Nothing here is random: the same position and throw always give the same move.

use crate::board::{Rules, Side, Spot};
use crate::dice::Throw;
use crate::position::{Move, Position};

/// The move the Bot plays for this throw, or `None` when the throw allows none.
pub fn choose(position: &Position, throw: Throw) -> Option<Move> {
    let me = position.to_move();
    let mut best: Option<(i32, Move)> = None;
    for offer in position.moves_for(throw) {
        let mut after = position.clone();
        after.play(offer.chosen);
        let value = ahead(&after, me);
        if best.is_none_or(|(v, _)| value > v) {
            best = Some((value, offer.chosen));
        }
    }
    best.map(|(_, chosen)| chosen)
}

/// The value of `position` for `me`, averaged over the next throw, with whoever
/// throws it choosing their best move by [`count`]. Scaled by the number of ways
/// the dice fall, so it stays an integer.
fn ahead(position: &Position, me: Side) -> i32 {
    if position.winner().is_some() {
        return count(position, me) * 64;
    }
    let rules = position.rules();
    let mover = position.to_move();
    let mut total = 0;
    for throw in rules.throws() {
        let replies: Vec<i32> = position
            .moves_for(throw)
            .iter()
            .map(|offer| {
                let mut after = position.clone();
                after.play(offer.chosen);
                count(&after, me)
            })
            .collect();
        let value = if mover == me {
            replies.iter().copied().max()
        } else {
            replies.iter().copied().min()
        }
        .unwrap_or_else(|| count(position, me));
        total += value * rules.ways(throw) as i32;
    }
    total * 64 / (1 << rules.dice)
}

/// A static count for `me`: how far each side's pieces have come, with a piece
/// home and a piece on a safe square worth a little more, and a won game worth
/// everything.
fn count(position: &Position, me: Side) -> i32 {
    match position.winner() {
        Some(side) if side == me => return 10_000,
        Some(_) => return -10_000,
        None => {}
    }
    side_count(position, me) - side_count(position, me.other())
}

fn side_count(position: &Position, side: Side) -> i32 {
    let rules: &Rules = position.rules();
    position
        .pieces(side)
        .iter()
        .map(|&spot| {
            let base = rules.distance(spot) as i32 * 4;
            match spot {
                Spot::Off => base + 8,
                Spot::Step(_) if rules.square_at(side, spot).is_some_and(|s| rules.is_safe(s)) => base + 6,
                _ => base,
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{ALL, FINKEL};
    use crate::game::{Game, Needed};

    fn throw(marks: u8) -> Throw {
        Throw::new(marks).unwrap()
    }

    #[test]
    fn it_takes_a_piece_rather_than_enter_one() {
        // White at step 4 (its private rosette), Black at White's step 6 on the shared row.
        let mut glyphs = vec!['.'; 20];
        glyphs[0] = 'W';
        glyphs[7] = 'B';
        let picture = format!("{} 66 00 w", glyphs.into_iter().collect::<String>());
        let position = Position::from_picture(&FINKEL, &picture).unwrap();
        let offers = position.moves_for(throw(2));
        assert!(offers.len() > 1, "a quiet move is on offer too");
        let chosen = choose(&position, throw(2)).unwrap();
        assert_eq!(chosen, Move { from: Spot::Step(4), to: Spot::Step(6) });
    }

    #[test]
    fn it_bears_off_the_last_piece_to_win() {
        // White's last piece at step 13 and nothing else left; a 2 bears it off.
        let mut glyphs = vec!['.'; 20];
        glyphs[5] = 'W';
        let picture = format!("{} 07 60 w", glyphs.into_iter().collect::<String>());
        let position = Position::from_picture(&FINKEL, &picture).unwrap();
        assert_eq!(choose(&position, throw(2)), Some(Move { from: Spot::Step(13), to: Spot::Off }));
    }

    #[test]
    fn a_throw_that_allows_nothing_gets_no_move() {
        let position = Position::starting_with(&FINKEL, Side::White);
        assert_eq!(choose(&position, throw(0)), None);
    }

    /// The Bot against itself in every rule set: every move it picks is legal, and
    /// every game ends with all of one side's pieces home.
    #[test]
    fn it_plays_every_rule_set_to_the_end() {
        for rules in ALL {
            let mut seed: u32 = 0x2468_ace1;
            let mut game = Game::new(rules, Side::White);
            let mut entries = 0;
            while !game.is_over() {
                entries += 1;
                assert!(entries < 5_000, "{} did not finish", rules.name);
                match game.needs() {
                    Needed::Throw { .. } => {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        let marks = (seed & ((1 << rules.dice) - 1)).count_ones() as u8;
                        game.throw(throw(marks)).unwrap();
                    }
                    Needed::Move { .. } => {
                        let chosen = choose(game.position(), game.pending_throw().unwrap())
                            .expect("a move is owed");
                        game.play(chosen).expect("the Bot's move is legal");
                    }
                    Needed::Nothing => unreachable!(),
                }
            }
            assert_eq!(game.home().0.max(game.home().1), rules.pieces, "{}", rules.name);
        }
    }
}
