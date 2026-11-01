//! A team League of a Club: Teams that meet each other in a round-robin, each match played on
//! the same number of boards. A board is won, drawn or lost as in a Tournament; the boards add
//! up to the match's board points, and the match gives match points: 2 for a win, 1 for a draw.
//! The League standings rank by match points, then board points. It is kept in the Club file
//! (`Club::league`), as one JSON value per League, like a Tournament.
//!
//! Points are kept in halves, as in a Tournament: a board won is 2, a match won is 4 match
//! points in halves (2 match points).

use pairing::round_robin;
use serde::{Deserialize, Serialize};

use crate::club::{Entry, Error, Result};
use crate::tournament::{Outcome, Player};

/// A Team: its players in board order. The first `League::boards` of them play unless a
/// line-up says otherwise.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Team {
    pub name: String,
    pub players: Vec<Player>,
}

/// One board of a match: the home Team's player and the away Team's, by place in their Team
/// (None: no one, a forfeit), and the result by colour. The home Team has White on boards 1,
/// 3, 5, …, the away Team on the others.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub home: Option<usize>,
    pub away: Option<usize>,
    pub result: Option<Outcome>,
}

/// A match between two Teams, by their place in the League.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Match {
    pub home: usize,
    pub away: usize,
    pub boards: Vec<Board>,
}

/// A Round: its matches, and the Team that sits it out when there is an odd number.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Round {
    pub matches: Vec<Match>,
    pub bye: Option<usize>,
}

/// A Team's place in the League.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Standing {
    pub team: usize,
    /// Shared by Teams level on match and board points.
    pub rank: u32,
    pub played: u32,
    pub won: u32,
    pub drawn: u32,
    pub lost: u32,
    /// In halves.
    pub match_points: u32,
    /// In halves.
    pub board_points: u32,
}

/// A team League.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct League {
    pub name: String,
    /// The first day, `YYYY-MM-DD`.
    pub date: String,
    /// Boards per match.
    pub boards: u32,
    /// Every pair of Teams meets twice, home and away.
    pub double: bool,
    pub teams: Vec<Team>,
    pub played: Vec<Round>,
    /// Changes made by hand, oldest first.
    pub log: Vec<Entry>,
}

impl Default for League {
    fn default() -> League {
        League { name: String::new(), date: String::new(), boards: 4, double: false, teams: vec![], played: vec![], log: vec![] }
    }
}

impl League {
    pub fn new(name: &str, boards: u32, double: bool, date: &str) -> Result<League> {
        if name.trim().is_empty() || name.chars().count() > 100 {
            return Err(Error::Invalid("leagues.error.name"));
        }
        if !(1..=20).contains(&boards) {
            return Err(Error::Invalid("leagues.error.boards"));
        }
        Ok(League { name: name.trim().into(), date: date.into(), boards, double, ..League::default() })
    }

    /// Adds a Team, before Round 1 is paired.
    pub fn add_team(&mut self, team: Team) -> Result<usize> {
        if !self.played.is_empty() {
            return Err(Error::Invalid("leagues.error.started"));
        }
        let name = team.name.trim();
        if name.is_empty() || name.chars().count() > 100 || self.teams.iter().any(|t| t.name.eq_ignore_ascii_case(name)) {
            return Err(Error::Invalid("leagues.error.team_name"));
        }
        if team.players.iter().any(|p| p.name.trim().is_empty()) {
            return Err(Error::Invalid("clubs.error.name"));
        }
        self.teams.push(Team { name: name.into(), ..team });
        Ok(self.teams.len() - 1)
    }

    /// Rounds in all: every Team meets every other once, or twice.
    pub fn rounds(&self) -> u32 {
        let n = self.teams.len() as u32;
        (n + n % 2).saturating_sub(1) * if self.double { 2 } else { 1 }
    }

    /// Pairs the next Round by the Berger tables, Teams numbered in the order they were added,
    /// each Team lined up in its board order.
    pub fn pair_next(&mut self) -> Result<()> {
        if self.teams.len() < 2 {
            return Err(Error::Invalid("leagues.error.too_few"));
        }
        if self.played.last().is_some_and(|r| r.matches.iter().any(|m| m.boards.iter().any(|b| b.result.is_none()))) {
            return Err(Error::Invalid("tournaments.error.results_missing"));
        }
        let r = self.played.len() as u32 + 1;
        let cycles = if self.double { 2 } else { 1 };
        let p = round_robin::pair(self.teams.len() as u32, cycles, self.double, r).map_err(|_| Error::Invalid("tournaments.error.over"))?;
        let matches = p.boards.iter().map(|b| (b.white as usize - 1, b.black as usize - 1)).map(|(home, away)| Match { home, away, boards: self.line_up(home, away) }).collect();
        self.played.push(Round { matches, bye: p.bye.map(|b| b as usize - 1) });
        Ok(())
    }

    fn line_up(&self, home: usize, away: usize) -> Vec<Board> {
        let first = |t: usize, k: usize| (k < self.teams[t].players.len()).then_some(k);
        (0..self.boards as usize).map(|k| Board { home: first(home, k), away: first(away, k), result: None }).collect()
    }

    /// A Team's line-up for a match of the last Round, before any of its results: its players
    /// by place in the Team, one a board (None: the board is not played), in board order.
    pub fn set_line_up(&mut self, round: usize, m: usize, away: bool, players: &[Option<usize>]) -> Result<()> {
        let mt = self.played.get(round).and_then(|r| r.matches.get(m)).ok_or(Error::Invalid("tournaments.error.no_board"))?;
        if mt.boards.iter().any(|b| b.result.is_some()) {
            return Err(Error::Invalid("tournaments.error.has_results"));
        }
        let team = &self.teams[if away { mt.away } else { mt.home }];
        let named: Vec<usize> = players.iter().flatten().copied().collect();
        if players.len() != self.boards as usize || named.iter().any(|&p| p >= team.players.len()) || !named.windows(2).all(|w| w[0] < w[1]) {
            return Err(Error::Invalid("leagues.error.line_up"));
        }
        let boards = &mut self.played[round].matches[m].boards;
        for (b, &p) in boards.iter_mut().zip(players) {
            if away {
                b.away = p;
            } else {
                b.home = p;
            }
        }
        Ok(())
    }

    /// Whether the home Team has White on board `k` (from 0).
    pub fn home_white(k: usize) -> bool {
        k.is_multiple_of(2)
    }

    /// Enters or corrects a board's result; a correction needs a reason and is logged. A board
    /// with a player missing can only be a forfeit.
    pub fn set_result(&mut self, who: &str, round: usize, m: usize, board: usize, result: Option<Outcome>, why: &str) -> Result<()> {
        let mt = self.played.get(round).and_then(|r| r.matches.get(m)).ok_or(Error::Invalid("tournaments.error.no_board"))?;
        let b = *mt.boards.get(board).ok_or(Error::Invalid("tournaments.error.no_board"))?;
        if b.result == result {
            return Ok(());
        }
        let (white, black) = if League::home_white(board) { (b.home, b.away) } else { (b.away, b.home) };
        let allowed = match (white, black) {
            (Some(_), Some(_)) => true,
            (Some(_), None) => matches!(result, None | Some(Outcome::WhiteForfeit)),
            (None, Some(_)) => matches!(result, None | Some(Outcome::BlackForfeit)),
            (None, None) => matches!(result, None | Some(Outcome::DoubleForfeit)),
        };
        if !allowed {
            return Err(Error::Invalid("leagues.error.forfeit"));
        }
        if let Some(old) = b.result {
            if why.trim().is_empty() {
                return Err(Error::Invalid("clubs.error.why"));
            }
            let teams = format!("{} – {} {}", self.teams[mt.home].name, self.teams[mt.away].name, board + 1);
            let entry = Entry { at: bragd_core::fs::now(), who: who.into(), member: teams, what: "result".into(), old: format!("{}: {}", round + 1, old.text()), new: format!("{}: {}", round + 1, result.map_or("", Outcome::text)), why: why.trim().into() };
            self.log.push(entry);
        }
        self.played[round].matches[m].boards[board].result = result;
        Ok(())
    }

    /// The home Team's and the away Team's board points in a match, in halves, from the
    /// results in so far.
    pub fn score(m: &Match) -> (u32, u32) {
        m.boards.iter().enumerate().fold((0, 0), |(h, a), (k, b)| {
            let Some(o) = b.result else { return (h, a) };
            let (w, bl) = o.points();
            if League::home_white(k) { (h + w, a + bl) } else { (h + bl, a + w) }
        })
    }

    /// The standings from the matches with every board's result in: match points, then board
    /// points.
    pub fn standings(&self) -> Vec<Standing> {
        let mut s: Vec<Standing> = (0..self.teams.len()).map(|team| Standing { team, ..Standing::default() }).collect();
        for m in self.played.iter().flat_map(|r| &r.matches) {
            if m.boards.iter().any(|b| b.result.is_none()) {
                continue;
            }
            let (h, a) = League::score(m);
            for (t, mine, theirs) in [(m.home, h, a), (m.away, a, h)] {
                let st = &mut s[t];
                st.played += 1;
                st.board_points += mine;
                match mine.cmp(&theirs) {
                    std::cmp::Ordering::Greater => (st.won, st.match_points) = (st.won + 1, st.match_points + 4),
                    std::cmp::Ordering::Equal => (st.drawn, st.match_points) = (st.drawn + 1, st.match_points + 2),
                    std::cmp::Ordering::Less => st.lost += 1,
                }
            }
        }
        s.sort_by(|a, b| (b.match_points, b.board_points).cmp(&(a.match_points, a.board_points)).then(a.team.cmp(&b.team)));
        for i in 0..s.len() {
            s[i].rank = if i > 0 && (s[i].match_points, s[i].board_points) == (s[i - 1].match_points, s[i - 1].board_points) { s[i - 1].rank } else { i as u32 + 1 };
        }
        s
    }

    /// Whether a League read from a file holds together: every match names Teams that are
    /// there, two different ones, and every board players of theirs.
    pub fn sound(&self) -> bool {
        let n = self.teams.len();
        self.played.iter().all(|r| {
            r.bye.is_none_or(|b| b < n)
                && r.matches.iter().all(|m| {
                    m.home < n
                        && m.away < n
                        && m.home != m.away
                        && m.boards.iter().all(|b| b.home.is_none_or(|p| p < self.teams[m.home].players.len()) && b.away.is_none_or(|p| p < self.teams[m.away].players.len()))
                })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team(name: &str, players: &[&str]) -> Team {
        Team { name: name.into(), players: players.iter().map(|n| Player { name: (*n).into(), ..Player::default() }).collect() }
    }

    fn league(teams: usize) -> League {
        let mut l = League::new("Division 1", 2, false, "2026-11-05").unwrap();
        for t in 0..teams {
            let n = format!("T{t}");
            l.add_team(team(&n, &[&format!("{n}a"), &format!("{n}b"), &format!("{n}c")])).unwrap();
        }
        l
    }

    fn play(l: &mut League, r: usize, results: &[[Outcome; 2]]) {
        for (m, rs) in results.iter().enumerate() {
            for (b, &o) in rs.iter().enumerate() {
                l.set_result("Mette", r, m, b, Some(o), "").unwrap();
            }
        }
    }

    #[test]
    fn teams_meet_once_each_and_an_odd_team_out_sits_a_round() {
        let mut l = league(3);
        assert_eq!(l.rounds(), 3);
        let mut met = vec![];
        for r in 0..3 {
            l.pair_next().unwrap();
            let round = &l.played[r];
            assert_eq!((round.matches.len(), round.bye.is_some()), (1, true));
            let m = &round.matches[0];
            met.push((m.home.min(m.away), m.home.max(m.away)));
            assert_eq!(m.boards.iter().map(|b| (b.home, b.away)).collect::<Vec<_>>(), [(Some(0), Some(0)), (Some(1), Some(1))], "the first two of each Team");
            assert_eq!(l.pair_next(), Err(Error::Invalid("tournaments.error.results_missing")));
            play(&mut l, r, &[[Outcome::Draw, Outcome::Draw]]);
        }
        met.sort();
        assert_eq!(met, [(0, 1), (0, 2), (1, 2)]);
        assert_eq!(l.pair_next(), Err(Error::Invalid("tournaments.error.over")));
        assert_eq!(l.add_team(team("Late", &["x"])), Err(Error::Invalid("leagues.error.started")));
    }

    #[test]
    fn match_points_come_from_board_points_and_rank_first() {
        let mut l = league(4);
        l.pair_next().unwrap();
        // Board 1: home White; board 2: home Black.
        play(&mut l, 0, &[[Outcome::White, Outcome::White], [Outcome::Draw, Outcome::Draw]]);
        let m = &l.played[0].matches;
        assert_eq!(League::score(&m[0]), (2, 2), "1-0 for home, then 1-0 for away");
        assert_eq!(League::score(&m[1]), (2, 2));
        l.pair_next().unwrap();
        play(&mut l, 1, &[[Outcome::White, Outcome::Black], [Outcome::Draw, Outcome::Black]]);
        let s = l.standings();
        assert_eq!(s.iter().map(|s| s.played).collect::<Vec<_>>(), [2; 4]);
        let first = &s[0];
        assert_eq!((first.rank, first.won, first.drawn, first.match_points), (1, 1, 1, 6));
        // Board points break a tie on match points; level on both shares the rank.
        let by_team = |t: usize| s.iter().find(|x| x.team == t).unwrap().clone();
        let (h, a) = (l.played[1].matches[0].home, l.played[1].matches[0].away);
        assert_eq!((by_team(h).board_points, by_team(a).board_points), (2 + 4, 2));
        let last = s.last().unwrap();
        assert!(s.iter().filter(|x| x.match_points == last.match_points && x.board_points == last.board_points).all(|x| x.rank == last.rank));
    }

    #[test]
    fn a_line_up_keeps_board_order_and_an_empty_board_is_a_forfeit() {
        let mut l = league(2);
        l.pair_next().unwrap();
        assert_eq!(l.set_line_up(0, 0, true, &[Some(2), Some(0)]), Err(Error::Invalid("leagues.error.line_up")), "out of order");
        assert_eq!(l.set_line_up(0, 0, true, &[Some(5), None]), Err(Error::Invalid("leagues.error.line_up")));
        l.set_line_up(0, 0, true, &[Some(1), None]).unwrap();
        // Board 2: the away Team has White and no one there.
        assert_eq!(l.set_result("Mette", 0, 0, 1, Some(Outcome::Draw), ""), Err(Error::Invalid("leagues.error.forfeit")));
        l.set_result("Mette", 0, 0, 1, Some(Outcome::BlackForfeit), "").unwrap();
        l.set_result("Mette", 0, 0, 0, Some(Outcome::Black), "").unwrap();
        assert_eq!(League::score(&l.played[0].matches[0]), (2, 2));
        assert_eq!(l.set_line_up(0, 0, false, &[Some(0), Some(1)]), Err(Error::Invalid("tournaments.error.has_results")));
        assert_eq!(l.set_result("Mette", 0, 0, 0, Some(Outcome::Draw), ""), Err(Error::Invalid("clubs.error.why")));
        l.set_result("Mette", 0, 0, 0, Some(Outcome::Draw), "Scoresheet misread").unwrap();
        assert_eq!((l.log[0].old.as_str(), l.log[0].new.as_str()), ("1: 0-1", "1: ½-½"));
        assert!(l.sound());
        l.played[0].matches[0].boards[0].away = Some(9);
        assert!(!l.sound());
    }
}
