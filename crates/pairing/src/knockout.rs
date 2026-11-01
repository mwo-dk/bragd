//! Knockout brackets as the FIDE World Cup regulations pair them (2019 art.
//! 4.5-4.6, 2025 art. 4.2-4.3): "top half vs. lower half reversed".
//!
//! Seeds run from 1. The bracket has the next power of two of places; the top
//! seeds fill the places beyond the field as byes (206 players in 2025: 256
//! places, the top 50 seeds start in round 2). In each round, match `k` of `M`
//! pairs the holders of places `k` and `2M + 1 - k`; the winner of match `k`
//! takes place `k` in the next round, "the position of his/her eliminated
//! opponent". So seeds 1 and 2 can only meet in the final.
//!
//! Colours (art. 4.3): in round 1 the higher seed of an odd match has the colour
//! drawn at the opening and of an even match the other one; each round after,
//! every place has the colour opposite to its last. The board names the first
//! game; the match itself, its tie-breaks and a match for third place are the
//! event's own.

use crate::{Board, Bracket, Colour, Error, Pairing, Rule};

/// Pairs the next round of a knockout of `players` seeds. `winners[r]` holds
/// the seeds who won their match in round `r + 1`; byes need no entry. The
/// round's byes are in the bracket's `floaters`.
pub fn pair(players: u32, initial_colour: Colour, winners: &[Vec<u32>]) -> Result<Pairing, Error> {
    if players < 2 {
        return Err(Error::NoLegalPairing);
    }
    let size = players.next_power_of_two();
    let mut places: Vec<Option<u32>> = (1..=size).map(|s| (s <= players).then_some(s)).collect();
    for (r, won) in winners.iter().enumerate() {
        if places.len() < 2 {
            return Err(Error::TournamentOver);
        }
        places = matches(&places)
            .enumerate()
            .map(|(k, (a, b))| match (a, b) {
                (Some(a), Some(b)) => match (won.contains(&a), won.contains(&b)) {
                    (true, false) => Ok(Some(a)),
                    (false, true) => Ok(Some(b)),
                    _ => Err(Error::NoWinner {
                        round: r as u32 + 1,
                        board: k as u32 + 1,
                    }),
                },
                (a, b) => Ok(a.or(b)),
            })
            .collect::<Result<_, _>>()?;
    }
    if places.len() < 2 {
        return Err(Error::TournamentOver);
    }
    let round = winners.len() as u32 + 1;
    let mut boards = Vec::new();
    let mut pairs = Vec::new();
    let mut byes = Vec::new();
    for (k, (a, b)) in matches(&places).enumerate() {
        let (Some(a), Some(b)) = (a, b) else {
            byes.extend(a.or(b));
            continue;
        };
        // Odd matches in odd rounds give the upper place the drawn colour.
        let upper = if (k as u32 + round).is_multiple_of(2) {
            initial_colour.opposite()
        } else {
            initial_colour
        };
        boards.push(if upper == Colour::White {
            Board { white: a, black: b }
        } else {
            Board { white: b, black: a }
        });
        pairs.push((a.min(b), a.max(b)));
    }
    Ok(Pairing {
        boards,
        bye: None,
        brackets: vec![Bracket {
            rule: Rule::Knockout { round },
            score: 0,
            residents: places.iter().flatten().copied().collect(),
            moved_down: Vec::new(),
            pairs,
            floaters: byes,
            natural: true,
            exchange: None,
            failures: Vec::new(),
        }],
    })
}

/// A round's matches in order: place `k` against place `2M + 1 - k`.
fn matches(places: &[Option<u32>]) -> impl Iterator<Item = (Option<u32>, Option<u32>)> + '_ {
    let n = places.len();
    (0..n / 2).map(move |k| (places[k], places[n - 1 - k]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeds(p: &Pairing) -> Vec<(u32, u32)> {
        p.boards
            .iter()
            .map(|b| (b.white.min(b.black), b.white.max(b.black)))
            .collect()
    }

    #[test]
    fn world_cup_2019_pairs_1_128_then_2_127() {
        // Art. 4.5.3: "(1-128, 2-127...)".
        let p = pair(128, Colour::White, &[]).unwrap();
        let s = seeds(&p);
        assert_eq!(s.len(), 64);
        assert_eq!(&s[..2], &[(1, 128), (2, 127)]);
        assert_eq!(s[63], (64, 65));
    }

    #[test]
    fn world_cup_2025_gives_the_top_fifty_byes() {
        // Art. 4.2.3: "Match 1.1: P 51 vs P 206, Match 1.2: P 52 vs P 205, ….
        // Match 1.78: P 128 vs P 129."
        let p = pair(206, Colour::White, &[]).unwrap();
        let s = seeds(&p);
        assert_eq!(s.len(), 78);
        assert_eq!(s[0], (51, 206));
        assert_eq!(s[1], (52, 205));
        assert_eq!(s[77], (128, 129));
        assert_eq!(p.brackets[0].floaters, (1..=50).collect::<Vec<_>>());
        // Art. 4.2.4: "Match 2.1: P 1 vs Match 1.78 winner, Match 2.2: P 2 vs
        // Match 1.77 winner, …. Match 2.64: Match 1.14 winner vs Match 1.15 winner."
        // Let the lower seed win 1.78 and the higher one win every other match.
        let won: Vec<u32> = s
            .iter()
            .map(|&(a, b)| if a == 128 { b } else { a })
            .collect();
        let p2 = pair(206, Colour::White, &[won]).unwrap();
        let s2 = seeds(&p2);
        assert_eq!(s2.len(), 64);
        assert_eq!(s2[0], (1, 129));
        assert_eq!(s2[1], (2, 127));
        assert_eq!(s2[63], (64, 65));
    }

    #[test]
    fn colours_alternate_down_the_list_and_flip_each_round() {
        // Art. 4.6.2 (2019): match 2 has the colour match 1 had; the next round
        // gives the winner of match 1 the opposite.
        let p = pair(8, Colour::White, &[]).unwrap();
        let white_is_higher: Vec<bool> = p.boards.iter().map(|b| b.white < b.black).collect();
        assert_eq!(white_is_higher, [true, false, true, false]);
        let p2 = pair(8, Colour::White, &[vec![1, 2, 3, 4]]).unwrap();
        let white_is_higher: Vec<bool> = p2.boards.iter().map(|b| b.white < b.black).collect();
        assert_eq!(white_is_higher, [false, true]);
    }

    #[test]
    fn a_match_without_a_winner_is_refused() {
        assert_eq!(
            pair(4, Colour::White, &[vec![1]]),
            Err(Error::NoWinner { round: 1, board: 2 })
        );
        assert_eq!(
            pair(2, Colour::White, &[vec![1]]),
            Err(Error::TournamentOver)
        );
    }
}
