//! Bragd's Bot for Ur: it chooses a move for the throw on the table.
//!
//! It looks one throw ahead. For each legal move it plays the move, then weighs
//! every throw that can come next by how likely it is, and assumes whoever throws
//! it picks their best reply by the same static count. That is enough to take a
//! piece when it can, keep off squares the other side reaches easily, and bear off
//! when that wins. It plays every rule set, because it asks the position and its
//! [`Rules`] everything and knows nothing of its own.
//!
//! Nothing in [`choose`] is random: the same position and throw always give the same move.
//!
//! With a solved table ([`crate::solve`]) the Bot plays [`choose_solved`] instead: the move
//! that leaves the best chance to win, or at a lower strength a move drawn at random, each
//! weighted by how little it gives away.

use crate::board::{Rules, Side, Spot};
use crate::dice::Throw;
use crate::position::{Move, Position};
use crate::solve::{Chances, Space};

/// The strongest a Bot plays: always the best move.
pub const PERFECT: u8 = 20;

/// The move a solved table plays for this throw at `strength` (0 to [`PERFECT`]), from `seed`;
/// None when the throw allows no move or the table cannot be read.
///
/// Below [`PERFECT`] each move is drawn with weight `exp(-k × loss)`, the loss being the chance
/// to win it gives away against the best move and `k = 2^(strength / 2)`. At 0 a move that
/// loses everything is still drawn a third as often as the best one; at 19 one that loses a
/// hundredth of the chance is drawn once for about 1,300 draws of the best.
pub fn choose_solved(space: &Space, table: &dyn Chances, position: &Position, throw: Throw, strength: u8, seed: u64) -> Option<Move> {
    let moves = space.moves(table, position, throw)?;
    let best = moves.first()?.1;
    if strength >= PERFECT {
        return Some(moves[0].0);
    }
    let k = 2f64.powf(strength as f64 / 2.0);
    let weights: Vec<f64> = moves.iter().map(|(_, chance)| (-(best - chance) * k).exp()).collect();
    let mut pick = unit(seed) * weights.iter().sum::<f64>();
    for ((chosen, _), weight) in moves.iter().zip(&weights) {
        if pick < *weight {
            return Some(*chosen);
        }
        pick -= weight;
    }
    moves.last().map(|m| m.0)
}

/// A number in [0, 1) from `seed` (SplitMix64).
fn unit(seed: u64) -> f64 {
    let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)) as f64 / (u64::MAX as f64 + 1.0)
}

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

    /// A table that gives each position some chance: the moves are then worth different
    /// amounts. Perfect play always takes the best; the weakest mixes others in often;
    /// strength in between less often.
    #[test]
    fn strength_mixes_in_weaker_moves() {
        struct Scattered;
        impl Chances for Scattered {
            fn chance(&self, index: u64) -> Option<f64> {
                Some((index.wrapping_mul(2_654_435_761) % 1000) as f64 / 1000.0)
            }
        }
        // White on steps 4, 5 and 7, four waiting: a 1 enters, or moves from 5 or 7.
        let mut glyphs = vec!['.'; 20];
        for square in [0, 6, 8] {
            glyphs[square] = 'W';
        }
        let position = Position::from_picture(&FINKEL, &format!("{} 47 00 w", glyphs.into_iter().collect::<String>())).unwrap();
        let space = Space::new(&FINKEL);
        let moves = space.moves(&Scattered, &position, throw(1)).unwrap();
        assert_eq!(moves.len(), 3, "{moves:?}");
        assert!(moves[0].1 > moves[2].1 + 0.05, "{moves:?}");
        let best = moves[0].0;
        let others = |strength: u8| (0..400u64).filter(|&seed| choose_solved(&space, &Scattered, &position, throw(1), strength, seed) != Some(best)).count();
        assert_eq!(others(PERFECT), 0);
        let (weak, middling) = (others(0), others(12));
        assert!(weak > 100, "{weak}");
        assert!(middling < weak, "{middling} against {weak}");
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
