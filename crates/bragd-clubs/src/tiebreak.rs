//! Tiebreaks: the FIDE set of C.07 (Play-Off and Tie-Break Regulations, applied from
//! 1 March 2026) for individual tournaments, ordered per Tournament.
//!
//! In a Swiss Tournament unplayed rounds follow Article 16: for an opponent's tiebreak a
//! player's requested byes that only VURs follow count as draws (16.3), and a player's own
//! unplayed rounds count as games against a dummy with the player's own score, capped by
//! the scheduled opponent's adjusted score for a forfeit and by a draw in every round
//! otherwise (16.4). The Cut modifiers cut a VUR first (16.5). In a round-robin, forfeits are
//! games, except for the rating tiebreaks and forfeit losses in Type B (15.2).
//!
//! Ratings of 0 are unrated players. The rating tiebreaks leave them out, as Article 10
//! lets a tournament's rules say.

use serde::{Deserialize, Serialize};

use crate::tournament::{Slot, Standing, System, Tournament};

/// One tiebreak, by its C.07 acronym.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tiebreak {
    /// Direct Encounter (6).
    #[serde(rename = "DE")]
    DirectEncounter,
    /// Number of Wins, with or without playing (7.1).
    #[serde(rename = "WIN")]
    Wins,
    /// Number of Games Won over the board (7.2).
    #[serde(rename = "WON")]
    GamesWon,
    /// Games played with Black (7.3).
    #[serde(rename = "BPG")]
    BlackGames,
    /// Games won with Black (7.4).
    #[serde(rename = "BWG")]
    BlackWins,
    /// Sum of Progressive Scores (7.5).
    #[serde(rename = "PS")]
    Progressive,
    /// Buchholz (8.1).
    #[serde(rename = "BH")]
    Buchholz,
    #[serde(rename = "BH-C1")]
    BuchholzCut1,
    #[serde(rename = "BH-C2")]
    BuchholzCut2,
    #[serde(rename = "BH-M1")]
    BuchholzMedian1,
    #[serde(rename = "BH-M2")]
    BuchholzMedian2,
    /// Average of Opponents' Buchholz (8.2).
    #[serde(rename = "AOB")]
    OpponentsBuchholz,
    /// Sonneborn-Berger (9.1).
    #[serde(rename = "SB")]
    SonnebornBerger,
    #[serde(rename = "SB-C1")]
    SonnebornBergerCut1,
    /// Koya, for round-robins (9.2).
    #[serde(rename = "KS")]
    Koya,
    /// Average Rating of Opponents (10.1).
    #[serde(rename = "ARO")]
    AverageRating,
    #[serde(rename = "ARO-C1")]
    AverageRatingCut1,
    /// Tournament Performance Rating (10.2).
    #[serde(rename = "TPR")]
    Performance,
    /// The player's own rating (10.6).
    #[serde(rename = "RTNG")]
    Rating,
}

use Tiebreak::*;

impl Tiebreak {
    /// Every tiebreak Bragd offers, in C.07's order.
    pub const ALL: [Tiebreak; 19] = [
        DirectEncounter,
        Wins,
        GamesWon,
        BlackGames,
        BlackWins,
        Progressive,
        Buchholz,
        BuchholzCut1,
        BuchholzCut2,
        BuchholzMedian1,
        BuchholzMedian2,
        OpponentsBuchholz,
        SonnebornBerger,
        SonnebornBergerCut1,
        Koya,
        AverageRating,
        AverageRatingCut1,
        Performance,
        Rating,
    ];

    /// The acronym, as crosstables head the column.
    pub fn code(self) -> &'static str {
        match self {
            DirectEncounter => "DE",
            Wins => "WIN",
            GamesWon => "WON",
            BlackGames => "BPG",
            BlackWins => "BWG",
            Progressive => "PS",
            Buchholz => "BH",
            BuchholzCut1 => "BH-C1",
            BuchholzCut2 => "BH-C2",
            BuchholzMedian1 => "BH-M1",
            BuchholzMedian2 => "BH-M2",
            OpponentsBuchholz => "AOB",
            SonnebornBerger => "SB",
            SonnebornBergerCut1 => "SB-C1",
            Koya => "KS",
            AverageRating => "ARO",
            AverageRatingCut1 => "ARO-C1",
            Performance => "TPR",
            Rating => "RTNG",
        }
    }

    /// The text key of its name: `tournaments.tb.bh_c1`.
    pub fn key(self) -> String {
        format!("tournaments.tb.{}", self.code().to_lowercase().replace('-', "_"))
    }
}

/// The tiebreaks a new Tournament starts with: the Danish Chess Union's (DSU EMT rules, §24).
/// A round-robin goes by Sonneborn-Berger. Every other group goes by *almindelig korrektion*:
/// Buchholz without the two lowest opponents, then with the second-lowest added back, then
/// with the lowest too.
pub fn defaults(system: System) -> Vec<Tiebreak> {
    match system {
        System::Dutch => vec![BuchholzCut2, BuchholzCut1, Buchholz],
        System::RoundRobin { .. } => vec![SonnebornBerger],
    }
}

/// A player's rounds and scores, worked out once for all their tiebreaks.
struct Table<'a> {
    t: &'a Tournament,
    rounds: usize,
    swiss: bool,
    /// Each player's slot in each round.
    slots: Vec<Vec<Slot>>,
    /// Scores in halves.
    score: Vec<u32>,
    /// Scores as opponents' tiebreaks see them (16.3), in halves.
    adjusted: Vec<u32>,
}

/// One addend of a sum: the opponent's value, its contribution, and whether it comes from a
/// voluntary unplayed round.
#[derive(Clone, Copy)]
struct Part {
    value: f64,
    contribution: f64,
    vur: bool,
}

fn vur(s: Slot) -> bool {
    matches!(s, Slot::HalfBye | Slot::Absent | Slot::Forfeit { points: 0, .. })
}

impl<'a> Table<'a> {
    fn new(t: &'a Tournament) -> Table<'a> {
        let rounds = t.played.len();
        let swiss = matches!(t.system, System::Dutch);
        let slots: Vec<Vec<Slot>> = (0..t.players.len()).map(|p| (0..rounds).map(|r| t.slot(r, p)).collect()).collect();
        let score: Vec<u32> = slots.iter().map(|s| s.iter().map(|x| x.points()).sum()).collect();
        let adjusted = slots
            .iter()
            .map(|s| {
                (0..rounds)
                    .map(|r| match s[r] {
                        // 16.2.5: a requested bye followed only by VURs, or in the last round.
                        Slot::HalfBye | Slot::Absent if swiss && s[r + 1..].iter().all(|&y| vur(y)) => 1,
                        x => x.points(),
                    })
                    .sum()
            })
            .collect();
        Table { t, rounds, swiss, slots, score, adjusted }
    }

    /// The opponents' parts of player `p`'s Buchholz, with their points scored in each round.
    fn buchholz_parts(&self, p: usize) -> Vec<(Part, u32)> {
        let own = self.score[p] as f64 / 2.0;
        let mut out = vec![];
        for &s in &self.slots[p] {
            let value = match s {
                Slot::Played { opponent, .. } => self.opponent(opponent),
                Slot::Forfeit { opponent, .. } if !self.swiss => self.opponent(opponent),
                Slot::Forfeit { opponent, .. } => own.min(self.adjusted[opponent] as f64 / 2.0),
                // A round-robin's free round is no game.
                Slot::Absent if !self.swiss => continue,
                Slot::PairingBye | Slot::HalfBye | Slot::Absent => own.min(self.rounds as f64 / 2.0),
                Slot::Pending { .. } => continue,
            };
            out.push((Part { value, contribution: value, vur: self.swiss && vur(s) }, s.points()));
        }
        out
    }

    fn opponent(&self, o: usize) -> f64 {
        (if self.swiss { self.adjusted[o] } else { self.score[o] }) as f64 / 2.0
    }

    fn buchholz(&self, p: usize, low: usize, high: usize) -> f64 {
        cut(self.buchholz_parts(p).into_iter().map(|(x, _)| x).collect(), low, high)
    }

    fn sonneborn_berger(&self, p: usize, low: usize) -> f64 {
        let parts = self.buchholz_parts(p).into_iter().map(|(x, pts)| Part { contribution: x.value * pts as f64 / 2.0, ..x }).collect();
        cut(parts, low, 0)
    }

    /// Opponents met over the board, by rating; unrated ones left out.
    fn ratings(&self, p: usize) -> Vec<(u32, u32)> {
        self.slots[p]
            .iter()
            .filter_map(|s| match *s {
                Slot::Played { opponent, points, .. } => Some((self.t.players[opponent].rating, points)),
                _ => None,
            })
            .filter(|&(r, _)| r > 0)
            .collect()
    }

    fn average_rating(&self, p: usize, low: usize) -> f64 {
        let mut r: Vec<u32> = self.ratings(p).into_iter().map(|(r, _)| r).collect();
        r.sort_unstable();
        let r = &r[low.min(r.len())..];
        if r.is_empty() {
            return 0.0;
        }
        // Rounded to the nearest whole number, 0.5 up.
        ((2 * r.iter().sum::<u32>() + r.len() as u32) / (2 * r.len() as u32)) as f64
    }

    fn performance(&self, p: usize) -> f64 {
        let games = self.ratings(p);
        if games.is_empty() {
            return 0.0;
        }
        let points: u32 = games.iter().map(|&(_, x)| x).sum();
        // The fractional score in hundredths, 0.005 up.
        let hundredths = (100 * points + games.len() as u32) / (2 * games.len() as u32);
        self.average_rating(p, 0) + rating_difference(hundredths) as f64
    }

    fn value(&self, tb: Tiebreak, p: usize) -> f64 {
        let won = |s: &Slot| s.points() == 2;
        let game = |s: &Slot| matches!(s, Slot::Played { .. }) || !self.swiss && matches!(s, Slot::Forfeit { points: 2, .. });
        let black = |s: &Slot| matches!(s, Slot::Played { white: false, .. } | Slot::Forfeit { white: false, .. });
        let count = |f: &dyn Fn(&Slot) -> bool| self.slots[p].iter().filter(|s| f(s)).count() as f64;
        match tb {
            // Worked out per tied group by `standings`.
            DirectEncounter => 0.0,
            Wins => count(&won),
            GamesWon => count(&|s| game(s) && won(s)),
            BlackGames => count(&|s| game(s) && black(s)),
            BlackWins => count(&|s| game(s) && black(s) && won(s)),
            Progressive => self.slots[p].iter().scan(0, |sum, s| { *sum += s.points(); Some(*sum) }).sum::<u32>() as f64 / 2.0,
            Buchholz => self.buchholz(p, 0, 0),
            BuchholzCut1 => self.buchholz(p, 1, 0),
            BuchholzCut2 => self.buchholz(p, 2, 0),
            BuchholzMedian1 => self.buchholz(p, 1, 1),
            BuchholzMedian2 => self.buchholz(p, 2, 2),
            OpponentsBuchholz => {
                let opps: Vec<usize> = self.slots[p].iter().filter_map(|s| if let Slot::Played { opponent, .. } = s { Some(*opponent) } else { None }).collect();
                if opps.is_empty() { 0.0 } else { opps.iter().map(|&o| self.buchholz(o, 0, 0)).sum::<f64>() / opps.len() as f64 }
            }
            SonnebornBerger => self.sonneborn_berger(p, 0),
            SonnebornBergerCut1 => self.sonneborn_berger(p, 1),
            Koya => {
                // Half the most a player can score, in halves: a game a round in a Swiss, and
                // in a round-robin a game against each other player in each cycle.
                let half = match self.t.system {
                    System::RoundRobin { double } => self.t.seeds.len().saturating_sub(1) * if double { 2 } else { 1 },
                    System::Dutch => self.rounds,
                };
                self.slots[p]
                    .iter()
                    .filter_map(|s| match *s {
                        Slot::Played { opponent, points, .. } | Slot::Forfeit { opponent, points, .. } if self.score[opponent] as usize >= half => Some(points),
                        _ => None,
                    })
                    .sum::<u32>() as f64
                    / 2.0
            }
            AverageRating => self.average_rating(p, 0),
            AverageRatingCut1 => self.average_rating(p, 1),
            Performance => self.performance(p),
            Rating => self.t.players[p].rating as f64,
        }
    }

    /// Direct Encounter (6) among `group`: each one's score against the others, and whether
    /// every pair of them met. Pairs that met more than once count their average (6.1.2).
    fn encounters(&self, group: &[usize]) -> (Vec<f64>, Vec<u32>) {
        let mut score = vec![0.0; group.len()];
        let mut missing = vec![0u32; group.len()];
        for (i, &a) in group.iter().enumerate() {
            for &b in group {
                if a == b {
                    continue;
                }
                let games: Vec<u32> = self.slots[a]
                    .iter()
                    .filter_map(|s| match *s {
                        Slot::Played { opponent, points, .. } if opponent == b => Some(points),
                        Slot::Forfeit { opponent, points, .. } if opponent == b && !self.swiss => Some(points),
                        _ => None,
                    })
                    .collect();
                if games.is_empty() {
                    missing[i] += 1;
                } else {
                    score[i] += games.iter().sum::<u32>() as f64 / 2.0 / games.len() as f64;
                }
            }
        }
        (score, missing)
    }

    /// Orders a tied `group` by Direct Encounter: runs of players still tied, best first.
    /// Each player's score among the whole group goes into `shown`.
    fn direct(&self, group: Vec<usize>, shown: &mut dyn FnMut(usize, f64)) -> Vec<Vec<usize>> {
        if group.len() < 2 {
            return vec![group];
        }
        let (score, missing) = self.encounters(&group);
        for (i, &p) in group.iter().enumerate() {
            shown(p, score[i]);
        }
        let mut ignore = |_: usize, _: f64| {};
        if missing.iter().all(|&m| m == 0) {
            // 6.2: everyone met; ties within are broken by DE again.
            let mut idx: Vec<usize> = (0..group.len()).collect();
            idx.sort_by(|&a, &b| score[b].total_cmp(&score[a]));
            let runs = runs(&idx, |&a, &b| score[a] == score[b]);
            if runs.len() == 1 {
                return vec![group];
            }
            return runs.into_iter().flat_map(|r| self.direct(r.iter().map(|&i| group[i]).collect(), &mut ignore)).collect();
        }
        // 6.3: someone alone at the top whatever the missing games bring is ranked first.
        let mut rest: Vec<usize> = (0..group.len()).collect();
        let mut out = vec![];
        while rest.len() > 1 {
            let max = |i: usize| score[i] + missing[i] as f64;
            let Some(&top) = rest.iter().find(|&&i| rest.iter().all(|&j| j == i || score[i] > max(j))) else { break };
            out.push(vec![group[top]]);
            rest.retain(|&i| i != top);
        }
        if out.is_empty() {
            return vec![group];
        }
        out.extend(self.direct(rest.iter().map(|&i| group[i]).collect(), &mut ignore));
        out
    }
}

/// Splits `xs` into runs of neighbours that are `same`.
fn runs<T: Copy>(xs: &[T], same: impl Fn(&T, &T) -> bool) -> Vec<Vec<T>> {
    let mut out: Vec<Vec<T>> = vec![];
    for &x in xs {
        match out.last_mut() {
            Some(r) if same(r.last().unwrap(), &x) => r.push(x),
            _ => out.push(vec![x]),
        }
    }
    out
}

/// The sum of `parts` after cutting `low` least significant ones, a VUR first (16.5), then
/// `high` most significant ones (14).
fn cut(mut parts: Vec<Part>, low: usize, high: usize) -> f64 {
    let key = |p: &Part| (p.value, p.contribution);
    let less = |a: &Part, b: &Part| key(a).partial_cmp(&key(b)).unwrap();
    for _ in 0..low {
        let Some(lsv) = (0..parts.len()).min_by(|&a, &b| less(&parts[a], &parts[b])) else { break };
        let vur = (0..parts.len()).filter(|&i| parts[i].vur).min_by(|&a, &b| parts[a].contribution.total_cmp(&parts[b].contribution));
        let pick = match vur {
            Some(v) if parts[v].contribution >= parts[lsv].contribution => v,
            _ => lsv,
        };
        parts.remove(pick);
    }
    for _ in 0..high {
        let Some(msv) = (0..parts.len()).max_by(|&a, &b| less(&parts[a], &parts[b])) else { break };
        parts.remove(msv);
    }
    // An empty sum is -0.0 in Rust; a tiebreak of nothing is 0.
    parts.iter().map(|p| p.contribution).sum::<f64>() + 0.0
}

/// The FIDE Rating Regulations' table 8.1.1: the rating difference for a fractional score in
/// hundredths.
fn rating_difference(hundredths: u32) -> i32 {
    const DP: [i32; 51] = [
        0, 7, 14, 21, 29, 36, 43, 50, 57, 65, 72, 80, 87, 95, 102, 110, 117, 125, 133, 141, 149, 158, 166, 175, 184, 193, 202, 211, 220, 230, 240, 251, 262, 273, 284, 296, 309, 322, 336, 351, 366, 383, 401, 422,
        444, 470, 501, 538, 589, 677, 800,
    ];
    let h = hundredths.min(100) as i32;
    if h >= 50 { DP[(h - 50) as usize] } else { -DP[(50 - h) as usize] }
}

/// The standings after the Rounds with every result in: points, then each tiebreak in turn
/// on the players still tied (4.2). Players left level share a place.
pub fn standings(t: &Tournament) -> Vec<Standing> {
    let done = t.played.iter().take_while(|r| r.complete()).count();
    let upto = Tournament { played: t.played[..done].to_vec(), ..t.clone() };
    let table = Table::new(&upto);
    let n = t.players.len();
    let mut values: Vec<Vec<f64>> = (0..n).map(|p| t.tiebreaks.iter().map(|&tb| table.value(tb, p)).collect()).collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| table.score[b].cmp(&table.score[a]).then(a.cmp(&b)));
    let mut groups = runs(&order, |&a, &b| table.score[a] == table.score[b]);
    for (k, &tb) in t.tiebreaks.iter().enumerate() {
        let mut next = vec![];
        for g in groups {
            if g.len() < 2 {
                next.push(g);
            } else if tb == DirectEncounter {
                next.extend(table.direct(g, &mut |p, v| values[p][k] = v));
            } else {
                let mut g = g;
                g.sort_by(|&a, &b| values[b][k].total_cmp(&values[a][k]));
                next.extend(runs(&g, |&a, &b| values[a][k] == values[b][k]));
            }
        }
        groups = next;
    }
    let mut out = vec![];
    for g in groups {
        let rank = out.len() as u32 + 1;
        for p in g {
            out.push(Standing { player: p, rank, points: table.score[p], tiebreaks: std::mem::take(&mut values[p]) });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tournament::{Game, Outcome, Player, Round};

    /// A crosstable row: the opponent by start number, `w`/`b`, and the result; `BYE` for the
    /// pairing-allocated bye, `HPB` a half-point bye, `--` absent, `F` a forfeit.
    fn crosstable(system: System, rows: &[(&str, u32, &[&str])]) -> Tournament {
        let mut t = Tournament { system, rounds: rows[0].2.len() as u32, ..Tournament::default() };
        for &(name, rating, _) in rows {
            t.players.push(Player { name: name.into(), rating, ..Player::default() });
        }
        if !matches!(system, System::Dutch) {
            t.seeds = (0..rows.len()).collect();
        }
        for r in 0..t.rounds as usize {
            let mut round = Round::default();
            for (p, &(_, _, cells)) in rows.iter().enumerate() {
                let c = cells[r];
                match c {
                    "BYE" => round.bye = Some(p),
                    "HPB" => round.half.push(p),
                    "--" => {}
                    _ => {
                        let (res, rest) = c.split_at(1);
                        let forfeit = rest.starts_with('F');
                        let opp: usize = rest[1..].parse::<usize>().unwrap() - 1;
                        let white = rest.starts_with('W') || forfeit && p < opp;
                        if !white {
                            continue;
                        }
                        let result = match (res, forfeit) {
                            ("+", false) => Outcome::White,
                            ("=", _) => Outcome::Draw,
                            ("-", false) => Outcome::Black,
                            ("+", true) => Outcome::WhiteForfeit,
                            ("-", true) => Outcome::BlackForfeit,
                            _ => panic!("{c}"),
                        };
                        round.games.push(Game { white: p, black: opp, result: Some(result), handicap: 0 });
                    }
                }
            }
            t.played.push(round);
        }
        t
    }

    /// The Swiss tournament of Mario Held's *Exercises in tie-breaking* (C.07-2023), 2.1.
    /// Forfeits are written `F` with the opponent; the lower number is White.
    fn swiss() -> Tournament {
        crosstable(
            System::Dutch,
            &[
                ("Alyx", 2200, &["+W9", "=B13", "=W2", "+B15", "=W4"]),
                ("Bruno", 2150, &["+B10", "+W7", "=B1", "+W16", "=B3"]),
                ("Charline", 2100, &["=W11", "+B6", "+W8", "=B4", "=W2"]),
                ("David", 2050, &["+B12", "HPB", "+W13", "=W3", "=B1"]),
                ("Helene", 2000, &["-W13", "-B15", "+W11", "=B7", "+W10"]),
                ("Franck", 1950, &["-B14", "-W3", "BYE", "+W10", "+B8"]),
                ("Genevieve", 1900, &["+W15", "-B2", "-B16", "=W5", "-B11"]),
                ("Irina", 1850, &["=B16", "+W14", "-B3", "+W13", "-W6"]),
                ("Jessica", 1800, &["-B1", "-W10", "HPB", "-F11", "BYE"]),
                ("Lais", 1750, &["-W2", "+B9", "-W15", "-B6", "-B5"]),
                ("Maria", 1700, &["=B3", "-W16", "-B5", "+F9", "+W7"]),
                ("Nick", 1650, &["-W4", "BYE", "+F14", "--", "--"]),
                ("Opal", 1600, &["+B5", "=W1", "-B4", "-B8", "-W14"]),
                ("Paul", 1550, &["+W6", "-B8", "-F12", "--", "+B13"]),
                ("Reine", 1500, &["-B7", "+W5", "+B10", "-W1", "-B16"]),
                ("Stephan", 1450, &["=W8", "+B11", "+W7", "-B2", "+W15"]),
            ],
        )
    }

    /// Held's round-robin, 2.2, in its Swiss form. Franck's round 4 loss to Helene is a forfeit.
    fn round_robin() -> Tournament {
        crosstable(
            System::RoundRobin { double: false },
            &[
                ("Alyx", 2200, &["=W5", "+W2", "+B3", "-W4", "+B6"]),
                ("Bruno", 2150, &["+W6", "-B1", "+W5", "=W3", "+B4"]),
                ("Charline", 2100, &["+W4", "+B6", "-W1", "=B2", "+W5"]),
                ("David", 2050, &["-B3", "-B5", "=W6", "+B1", "-W2"]),
                ("Franck", 1950, &["=B1", "+W4", "-B2", "-F6", "-B3"]),
                ("Helene", 2000, &["-B2", "-W3", "=B4", "+F5", "-W1"]),
            ],
        )
    }

    fn v(t: &Tournament, tb: Tiebreak, n: usize) -> f64 {
        Table::new(t).value(tb, n - 1)
    }

    #[test]
    fn the_crosstable_reads_as_printed() {
        let t = swiss();
        let pts: Vec<u32> = (0..16).map(|p| t.points(p)).collect();
        assert_eq!(pts, [7, 8, 7, 7, 5, 6, 3, 5, 3, 2, 5, 4, 3, 4, 4, 7]);
        let t = round_robin();
        assert_eq!((0..6).map(|p| t.points(p)).collect::<Vec<_>>(), [7, 7, 7, 3, 3, 3]);
    }

    #[test]
    fn buchholz_follows_held_exercises_1_to_4() {
        let t = swiss();
        for (n, bh) in [(2, 13.0), (1, 12.5), (3, 15.5), (5, 8.5), (8, 13.5), (16, 12.5)] {
            assert_eq!(v(&t, Buchholz, n), bh, "BH(#{n})");
        }
        // Nick withdrew after round 3: his last two rounds count as draws for his opponents.
        assert_eq!(Table::new(&t).adjusted[11], 6);
        // Under C.07-2023 David's half-point bye counted his own 3.5 (BH 15.0). Since March
        // 2026 the dummy scores at most a draw a round, 2.5 (16.4.2).
        assert_eq!(v(&t, Buchholz, 4), 14.0);
        // Maria's forfeit win: the dummy is capped by Jessica's adjusted 1.5 (16.4.1), not 2.5.
        assert_eq!(v(&t, Buchholz, 11), 12.5);
    }

    #[test]
    fn buchholz_cut_1_cuts_a_voluntary_unplayed_round_first() {
        let t = swiss();
        for (n, c1) in [(5, 7.5), (8, 12.0), (7, 12.5), (9, 7.5), (13, 12.0), (1, 11.0), (3, 13.0), (16, 11.0), (12, 9.5), (14, 9.0), (15, 11.0)] {
            assert_eq!(v(&t, BuchholzCut1, n), c1, "BH-C1(#{n})");
        }
        // David's bye is cut, not Opal's lower 1.5 (Held exercise 7): 3.0 + 1.5 + 3.5 + 3.5.
        assert_eq!(v(&t, BuchholzCut1, 4), 11.5);
        assert_eq!(v(&t, BuchholzMedian1, 1), 11.0 - 4.0);
        // Cutting every value leaves 0, not -0.
        assert!(cut(vec![Part { value: 1.0, contribution: 1.0, vur: false }], 2, 0).is_sign_positive());
    }

    #[test]
    fn sonneborn_berger_follows_held_exercises_11_and_14() {
        let t = swiss();
        for (n, sb) in [(1, 8.0), (3, 10.5), (16, 7.25)] {
            assert_eq!(v(&t, SonnebornBerger, n), sb, "SB(#{n})");
        }
        // 9.75 under C.07-2023; the bye's dummy now scores 2.5, not 3.5.
        assert_eq!(v(&t, SonnebornBerger, 4), 9.25);
        let t = round_robin();
        let sb: Vec<f64> = (1..=6).map(|n| v(&t, SonnebornBerger, n)).collect();
        assert_eq!(sb, [9.25, 6.25, 6.25, 4.25, 3.25, 2.25]);
        let c1: Vec<f64> = (1..=6).map(|n| v(&t, SonnebornBergerCut1, n)).collect();
        assert_eq!(c1, [9.25, 4.75, 4.75, 4.25, 3.25, 1.5]);
    }

    #[test]
    fn koya_counts_points_against_the_top_half() {
        let t = round_robin();
        let ks: Vec<f64> = (1..=6).map(|n| v(&t, Koya, n)).collect();
        assert_eq!(ks, [2.0, 0.5, 0.5, 1.0, 0.5, 0.0]);
    }

    #[test]
    fn rating_tiebreaks_follow_held_exercises_18_and_19() {
        let t = swiss();
        assert_eq!(v(&t, AverageRating, 2), 1880.0);
        assert_eq!(v(&t, AverageRatingCut1, 2), 1988.0);
        assert_eq!(v(&t, AverageRatingCut1, 13), 2025.0);
        assert_eq!(v(&t, Performance, 2), 2120.0);
        assert_eq!(v(&t, Performance, 6), 1813.0);
        assert_eq!(v(&t, Performance, 12), 1250.0);
        // Held's summary table.
        for (n, aro, tpr) in [(3, 1940.0, 2089.0), (4, 1888.0, 2081.0), (9, 1975.0, 1175.0), (14, 1800.0, 1925.0), (7, 1760.0, 1611.0)] {
            assert_eq!((v(&t, AverageRating, n), v(&t, Performance, n)), (aro, tpr), "#{n}");
        }
    }

    #[test]
    fn wins_games_won_black_games_and_progressive_scores() {
        let t = swiss();
        // Jessica: no game won, but a full-point bye.
        assert_eq!((v(&t, Wins, 9), v(&t, GamesWon, 9)), (1.0, 0.0));
        // Nick: a bye and a forfeit win.
        assert_eq!((v(&t, Wins, 12), v(&t, GamesWon, 12)), (2.0, 0.0));
        // Bruno: three wins, games with Black in rounds 1, 3 and 5, one won.
        assert_eq!((v(&t, Wins, 2), v(&t, BlackGames, 2), v(&t, BlackWins, 2)), (3.0, 3.0, 1.0));
        // Bruno: 1, 2, 2.5, 3.5, 4.
        assert_eq!(v(&t, Progressive, 2), 13.0);
    }

    #[test]
    fn direct_encounter_ranks_held_exercise_26() {
        let mut t = round_robin();
        t.tiebreaks = vec![DirectEncounter];
        let s = standings(&t);
        let names: Vec<(&str, u32)> = s.iter().map(|x| (t.players[x.player].name.as_str(), x.rank)).collect();
        assert_eq!(names, [("Alyx", 1), ("Bruno", 2), ("Charline", 2), ("Helene", 4), ("Franck", 5), ("David", 6)]);
        assert_eq!(s[0].tiebreaks, [2.0]);
    }

    #[test]
    fn direct_encounter_in_a_swiss_ranks_one_alone_at_the_top_whatever_the_missing_games() {
        // Alyx beat everyone, Bruno beat Charline and David, who never met (Held 6).
        let t = crosstable(
            System::Dutch,
            &[
                ("Alyx", 0, &["+W2", "+W3", "+W4"]),
                ("Bruno", 0, &["-B1", "+W4", "+W3"]),
                ("Charline", 0, &["+W4", "-B1", "-B2"]),
                ("David", 0, &["-B3", "-B2", "-B1"]),
            ],
        );
        let mut tb = Table::new(&t);
        tb.slots[2][0] = Slot::Absent;
        tb.slots[3][0] = Slot::Absent;
        let order = tb.direct(vec![0, 1, 2, 3], &mut |_, _| {});
        assert_eq!(order, [vec![0], vec![1], vec![2, 3]]);
        // Held exercise 23: no one is first whatever the missing games bring.
        let t = swiss();
        let tb = Table::new(&t);
        assert_eq!(tb.direct(vec![0, 2, 3, 15], &mut |_, _| {}).len(), 1);
    }

    #[test]
    fn standings_break_ties_in_the_tournaments_order() {
        let mut t = swiss();
        t.tiebreaks = vec![BuchholzCut1, Buchholz];
        let s = standings(&t);
        let top: Vec<(&str, u32)> = s[..5].iter().map(|x| (t.players[x.player].name.as_str(), x.rank)).collect();
        // 3.5 points: Charline 13.0, David 11.5, then Alyx and Stephan level on 11.0 and 12.5.
        assert_eq!(top, [("Bruno", 1), ("Charline", 2), ("David", 3), ("Alyx", 4), ("Stephan", 4)]);
    }

    #[test]
    fn every_tiebreak_has_its_code_in_json() {
        for tb in Tiebreak::ALL {
            assert_eq!(serde_json::to_string(&tb).unwrap(), format!("\"{}\"", tb.code()));
        }
    }
}
