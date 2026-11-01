//! A Tournament of a Club: its players, its Rounds and their results, and the log of every
//! change a director made by hand. It is kept in the Club file (`Club::tournament`), as one
//! JSON value per Tournament, so a copy of the Club carries its Tournaments.
//!
//! The night runs in this order: register players, check them in, pair the Round, show the
//! pairings, enter the results, correct them, and publish. Late entries and withdrawals can
//! come at any time. Pairing is done by the `pairing` crate; this module turns the Tournament
//! into what a pairing system needs and its answer back into boards.
//!
//! Points are kept in halves: a win is 2, a draw 1.

use bragd_core::t;
use pairing::{Board, Colour, Criterion, dutch, round_robin};
use serde::{Deserialize, Serialize};

use crate::club::{Entry, Error, Result};
use crate::tiebreak::{self, Tiebreak};

/// How the Rounds are paired. Other pairing systems join as variants with one arm in
/// [`Tournament::propose`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum System {
    /// Swiss, FIDE Dutch (C.04.3).
    Dutch,
    /// Round-robin by FIDE's Berger tables; `double` plays every pair twice.
    RoundRobin { double: bool },
}

/// Where a player stands for the next Round.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Registered but not checked in: not paired, no points.
    #[default]
    Away,
    /// Checked in: paired.
    Here,
    /// Asked for a half-point bye in the next Round. They are Here again after it.
    HalfBye,
    /// Left the Tournament: never paired again; their results stand.
    Withdrawn,
}

/// A player of the Tournament: a member of the Club, or a guest found in a rating list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Player {
    pub name: String,
    /// The rating the player is seeded by; 0 when unrated.
    pub rating: u32,
    pub title: String,
    pub fide_id: String,
    pub national_id: String,
    /// The three-letter federation.
    pub federation: String,
    /// The Club member this is, if any.
    pub member: Option<i64>,
    pub status: Status,
    /// Rounds paired before the player entered.
    pub joined: u32,
}

/// The result of a game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    White,
    Draw,
    Black,
    /// White wins because Black did not play.
    WhiteForfeit,
    /// Black wins because White did not play.
    BlackForfeit,
    /// Neither played: both lose.
    DoubleForfeit,
}

impl Outcome {
    /// White's and Black's points, in halves.
    pub fn points(self) -> (u32, u32) {
        match self {
            Outcome::White | Outcome::WhiteForfeit => (2, 0),
            Outcome::Draw => (1, 1),
            Outcome::Black | Outcome::BlackForfeit => (0, 2),
            Outcome::DoubleForfeit => (0, 0),
        }
    }

    pub fn played(self) -> bool {
        matches!(self, Outcome::White | Outcome::Draw | Outcome::Black)
    }

    /// As crosstables write it: `1-0`, `½-½`, `+--`, ….
    pub fn text(self) -> &'static str {
        match self {
            Outcome::White => "1-0",
            Outcome::Draw => "½-½",
            Outcome::Black => "0-1",
            Outcome::WhiteForfeit => "+--",
            Outcome::BlackForfeit => "--+",
            Outcome::DoubleForfeit => "---",
        }
    }
}

/// A board of a Round, by player index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    pub white: usize,
    pub black: usize,
    pub result: Option<Outcome>,
    /// Go with handicaps (`Handicap`): 0 an even game, 1 Black first with komi 0.5 and no
    /// stones, 2 to 9 that many stones with komi 0.5. Black is the weaker player.
    #[serde(default)]
    pub handicap: u32,
}

/// Go's handicap pairing: each 100 rating points between two players is one stone, as on the
/// European Go Federation's list, less `reduction`, and at most `max`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Handicap {
    pub reduction: u32,
    pub max: u32,
}

impl Default for Handicap {
    fn default() -> Handicap {
        Handicap { reduction: 0, max: 9 }
    }
}

impl Handicap {
    /// The handicap of a game between players rated `a` and `b`.
    pub fn of(self, a: u32, b: u32) -> u32 {
        (a.abs_diff(b).saturating_add(50) / 100).saturating_sub(self.reduction).min(self.max.min(9))
    }
}

/// A Round: its boards in order, the pairing-allocated bye, and the half-point byes asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Round {
    pub games: Vec<Game>,
    /// The player given the pairing-allocated bye: a whole point.
    pub bye: Option<usize>,
    /// Players who asked for a half-point bye.
    pub half: Vec<usize>,
}

impl Round {
    pub fn complete(&self) -> bool {
        self.games.iter().all(|g| g.result.is_some())
    }
}

/// What one player had in one Round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// A game was played; points in halves.
    Played { opponent: usize, white: bool, points: u32 },
    /// Paired, but the game was not played.
    Forfeit { opponent: usize, white: bool, points: u32 },
    /// Paired, no result yet.
    Pending { opponent: usize, white: bool },
    /// The pairing-allocated bye: a whole point.
    PairingBye,
    /// A half-point bye.
    HalfBye,
    /// Not paired: away, withdrawn or not yet entered.
    Absent,
}

impl Slot {
    /// Points in halves.
    pub fn points(self) -> u32 {
        match self {
            Slot::Played { points, .. } | Slot::Forfeit { points, .. } => points,
            Slot::PairingBye => 2,
            Slot::HalfBye => 1,
            Slot::Pending { .. } | Slot::Absent => 0,
        }
    }

    pub fn opponent(self) -> Option<usize> {
        match self {
            Slot::Played { opponent, .. } | Slot::Forfeit { opponent, .. } | Slot::Pending { opponent, .. } => Some(opponent),
            _ => None,
        }
    }
}

/// Something about a pairing made by hand that the rules would not do. The director may
/// still keep it, with a reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Warning {
    /// Breaks an absolute rule: players meet twice, a second pairing bye, or the rest of the
    /// Round could not be paired. `score` is the scoregroup's, in halves.
    Illegal { score: u32 },
    /// Legal, but the scoregroup meets this criterion of C.04.3 less well than the rules' pairing.
    Worse { score: u32, criterion: String },
    /// As good on every criterion, but the rules take another pairing first.
    Order { score: u32 },
    /// The same pairs with colours that Article 5 gives the other way.
    Colours { boards: Vec<(String, String)> },
    /// Not the pairing of the Berger table.
    NotTable,
    /// A player who is here is not paired.
    Unpaired { player: String },
}

impl Warning {
    /// In the user's language.
    pub fn text(&self) -> String {
        let pts = |s: u32| half_text(s);
        match self {
            Warning::Illegal { score } => t!("tournaments.warn.illegal", "score" => pts(*score)),
            Warning::Worse { score, criterion } => {
                t!("tournaments.warn.worse", "score" => pts(*score), "criterion" => t!(&format!("tournaments.criterion.{criterion}")))
            }
            Warning::Order { score } => t!("tournaments.warn.order", "score" => pts(*score)),
            Warning::Colours { boards } => {
                t!("tournaments.warn.colours", "boards" => boards.iter().map(|(w, b)| format!("{w} – {b}")).collect::<Vec<_>>().join(", "))
            }
            Warning::NotTable => t!("tournaments.warn.not_table"),
            Warning::Unpaired { player } => t!("tournaments.warn.unpaired", "player" => player),
        }
    }
}

/// Points in halves as a crosstable writes them: `3½`, `½`, `4`.
pub fn half_text(h: u32) -> String {
    match (h / 2, h % 2) {
        (0, 1) => "½".into(),
        (n, 1) => format!("{n}½"),
        (n, _) => n.to_string(),
    }
}

/// C.04.3's number of a criterion, as the texts name it: `c5` to `c21`.
fn criterion(c: Criterion) -> &'static str {
    use Criterion::*;
    match c {
        ByeScore => "c5",
        Downfloaters => "c6",
        DownfloaterScores => "c7",
        NextBracketDownfloaters | NextBracketDownfloaterScores => "c8",
        ByeUnplayedRounds => "c9",
        TopscorerColourDifference => "c10",
        TopscorerColourRepeat => "c11",
        ColourPreference => "c12",
        StrongColourPreference => "c13",
        RepeatDownfloat => "c14",
        RepeatUpfloat => "c15",
        RepeatDownfloatTwoRounds => "c16",
        RepeatUpfloatTwoRounds => "c17",
        RepeatMovedDown => "c18",
        RepeatMovedDownOpponent => "c19",
        RepeatMovedDownTwoRounds => "c20",
        RepeatMovedDownOpponentTwoRounds => "c21",
    }
}

fn pairing_error(e: pairing::Error) -> Error {
    Error::Invalid(match e {
        pairing::Error::TournamentOver => "tournaments.error.over",
        pairing::Error::NoLegalPairing => "tournaments.error.no_legal_pairing",
        pairing::Error::TooComplex => "tournaments.error.too_complex",
        _ => "tournaments.error.pairing",
    })
}

/// A Round's boards as (White, Black) and its pairing-allocated bye, by player index.
pub type Proposal = (Vec<(usize, usize)>, Option<usize>);

/// A player's place in the standings.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Standing {
    pub player: usize,
    /// 1 for the leader; players level on points and every tiebreak share a place.
    pub rank: u32,
    /// In halves.
    pub points: u32,
    /// One value per tiebreak, in the Tournament's order.
    pub tiebreaks: Vec<f64>,
}

/// A Tournament.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tournament {
    pub name: String,
    /// The first day, `YYYY-MM-DD`.
    pub date: String,
    pub place: String,
    /// The chief arbiter, as reports name them: whoever set the Tournament up.
    pub arbiter: String,
    pub system: System,
    /// Rounds declared. A round-robin sets it when its first Round is paired.
    pub rounds: u32,
    /// The colour of the top seed on board 1 in Round 1, drawn by lot.
    pub white_first: bool,
    pub tiebreaks: Vec<Tiebreak>,
    pub players: Vec<Player>,
    pub played: Vec<Round>,
    /// Round-robin numbers, fixed when Round 1 is paired: `seeds[k]` has number `k + 1`.
    pub seeds: Vec<usize>,
    /// Changes made by hand, oldest first.
    pub log: Vec<Entry>,
    /// The Club's ratings were worked out from its games.
    pub rated: bool,
    /// The Variant played, by its catalogue key: any Family can run a Tournament.
    pub variant: String,
    /// Go only: handicap games instead of even ones.
    pub handicap: Option<Handicap>,
    /// Backgammon only: each pairing is a match to this many points, and its result is the
    /// match's winner.
    pub match_to: Option<u32>,
}

impl Default for Tournament {
    fn default() -> Tournament {
        Tournament {
            name: String::new(),
            date: String::new(),
            place: String::new(),
            arbiter: String::new(),
            system: System::Dutch,
            rounds: 7,
            white_first: true,
            tiebreaks: tiebreak::defaults(System::Dutch),
            players: vec![],
            played: vec![],
            seeds: vec![],
            log: vec![],
            rated: false,
            variant: "chess".into(),
            handicap: None,
            match_to: None,
        }
    }
}

impl Tournament {
    /// A new Tournament with the Danish default tiebreaks for its system.
    pub fn new(name: &str, system: System, rounds: u32, date: &str) -> Result<Tournament> {
        if name.trim().is_empty() || name.chars().count() > 100 {
            return Err(Error::Invalid("tournaments.error.name"));
        }
        if matches!(system, System::Dutch) && !(1..=99).contains(&rounds) {
            return Err(Error::Invalid("tournaments.error.rounds"));
        }
        // A round-robin's Rounds follow from its players, when Round 1 is paired.
        let rounds = if matches!(system, System::Dutch) { rounds } else { 0 };
        Ok(Tournament { name: name.trim().into(), date: date.into(), system, rounds, tiebreaks: tiebreak::defaults(system), ..Tournament::default() })
    }

    /// Sets what is played, before Round 1: the Variant, go's handicaps and backgammon's
    /// match length, each only for its own Family.
    pub fn set_game(&mut self, variant: &str, handicap: Option<Handicap>, match_to: Option<u32>) -> Result<()> {
        if !self.played.is_empty() {
            return Err(Error::Invalid("tournaments.error.started"));
        }
        let family = bragd_games::variant(variant).map_err(|_| Error::Invalid("tournaments.error.variant"))?.family();
        if handicap.is_some() && family != bragd_games::Family::Go {
            return Err(Error::Invalid("tournaments.error.variant"));
        }
        if match_to.is_some() != (family == bragd_games::Family::Backgammon) || match_to.is_some_and(|n| !(1..=99).contains(&n)) {
            return Err(Error::Invalid("tournaments.error.match_to"));
        }
        self.variant = variant.into();
        self.handicap = handicap;
        self.match_to = match_to;
        Ok(())
    }

    /// The board of `white` and `black` as played: with handicaps, the weaker player is Black.
    fn board(&self, white: usize, black: usize) -> Game {
        let Some(h) = self.handicap else { return Game { white, black, result: None, handicap: 0 } };
        let (rw, rb) = (self.players[white].rating, self.players[black].rating);
        let handicap = h.of(rw, rb);
        let (white, black) = if handicap > 0 && rb > rw { (black, white) } else { (white, black) };
        Game { white, black, result: None, handicap }
    }

    fn note(&mut self, who: &str, player: &str, what: &str, old: String, new: String, why: &str) {
        self.log.push(Entry { at: bragd_core::fs::now(), who: who.into(), member: player.into(), what: what.into(), old, new, why: why.trim().into() });
    }

    fn player(&self, p: usize) -> Result<&Player> {
        self.players.get(p).ok_or(Error::NoMember)
    }

    /// Adds a player, Away until checked in. A late entry gets no points for the Rounds before.
    pub fn register(&mut self, p: Player) -> Result<usize> {
        if p.name.trim().is_empty() || p.name.chars().count() > 100 {
            return Err(Error::Invalid("clubs.error.name"));
        }
        if !self.seeds.is_empty() {
            return Err(Error::Invalid("tournaments.error.round_robin_started"));
        }
        if p.member.is_some() && self.players.iter().any(|q| q.member == p.member) {
            return Err(Error::Invalid("tournaments.error.registered"));
        }
        let joined = self.played.len() as u32;
        self.players.push(Player { name: p.name.trim().into(), status: Status::Away, joined, ..p });
        Ok(self.players.len() - 1)
    }

    /// Checks a player in or out, gives a half-point bye, or withdraws them. A withdrawal and
    /// a return are logged.
    pub fn set_status(&mut self, who: &str, p: usize, status: Status) -> Result<()> {
        let old = self.player(p)?.status;
        if old == status {
            return Ok(());
        }
        if old == Status::Withdrawn || status == Status::Withdrawn {
            let name = self.players[p].name.clone();
            self.note(who, &name, "status", status_id(old).into(), status_id(status).into(), "");
        }
        self.players[p].status = status;
        Ok(())
    }

    /// What player `p` had in Round `r` (from 0).
    pub fn slot(&self, r: usize, p: usize) -> Slot {
        let Some(round) = self.played.get(r) else { return Slot::Absent };
        if round.bye == Some(p) {
            // A round-robin's bye is a free round with no points.
            return if matches!(self.system, System::Dutch) { Slot::PairingBye } else { Slot::Absent };
        }
        if round.half.contains(&p) {
            return Slot::HalfBye;
        }
        for g in &round.games {
            let (opponent, white) = match (g.white == p, g.black == p) {
                (true, _) => (g.black, true),
                (_, true) => (g.white, false),
                _ => continue,
            };
            return match g.result {
                None => Slot::Pending { opponent, white },
                Some(o) => {
                    let points = if white { o.points().0 } else { o.points().1 };
                    if o.played() { Slot::Played { opponent, white, points } } else { Slot::Forfeit { opponent, white, points } }
                }
            };
        }
        Slot::Absent
    }

    /// Points so far, in halves.
    pub fn points(&self, p: usize) -> u32 {
        (0..self.played.len()).map(|r| self.slot(r, p).points()).sum()
    }

    /// Players in seeding order: rating, highest first, then title, then name.
    fn seeding(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.players.len()).collect();
        order.sort_by(|&a, &b| {
            let (pa, pb) = (&self.players[a], &self.players[b]);
            pb.rating.cmp(&pa.rating).then(title_rank(&pa.title).cmp(&title_rank(&pb.title))).then(pa.name.to_lowercase().cmp(&pb.name.to_lowercase())).then(a.cmp(&b))
        });
        order
    }

    /// Why the next Round cannot be paired yet, if it cannot.
    pub fn can_pair(&self) -> Result<()> {
        if self.played.last().is_some_and(|r| !r.complete()) {
            return Err(Error::Invalid("tournaments.error.results_missing"));
        }
        if (!self.seeds.is_empty() || matches!(self.system, System::Dutch)) && self.played.len() as u32 >= self.rounds {
            return Err(Error::Invalid("tournaments.error.over"));
        }
        Ok(())
    }

    /// The pairing system's boards and bye for the next Round, by player index, given who is
    /// here now. A round-robin fixes its numbers on its first Round.
    pub fn propose(&self) -> Result<Proposal> {
        self.can_pair()?;
        let r = self.played.len();
        match self.system {
            System::Dutch => {
                let (order, players) = self.dutch_players();
                let tpn = |p: usize| order.iter().position(|&q| q == p).unwrap() as u32 + 1;
                let present: Vec<u32> = order.iter().filter(|&&p| self.players[p].status == Status::Here).map(|&p| tpn(p)).collect();
                if present.len() < 2 {
                    return Err(Error::Invalid("tournaments.error.too_few"));
                }
                let pr = dutch::pair(&self.settings(), &players, &present).map_err(pairing_error)?;
                let at = |t: u32| order[t as usize - 1];
                Ok((pr.boards.iter().map(|b| (at(b.white), at(b.black))).collect(), pr.bye.map(at)))
            }
            System::RoundRobin { double } => {
                let seeds = if self.seeds.is_empty() { self.round_robin_seeds() } else { self.seeds.clone() };
                if seeds.len() < 2 {
                    return Err(Error::Invalid("tournaments.error.too_few"));
                }
                let pr = round_robin::pair(seeds.len() as u32, if double { 2 } else { 1 }, double, r as u32 + 1).map_err(pairing_error)?;
                let at = |n: u32| seeds[n as usize - 1];
                Ok((pr.boards.iter().map(|b| (at(b.white), at(b.black))).collect(), pr.bye.map(at)))
            }
        }
    }

    /// Players in seeding order, and each one's history as the Dutch system reads it, by
    /// Tournament Pairing Number: the place in that order.
    fn dutch_players(&self) -> (Vec<usize>, Vec<pairing::Player>) {
        let order = self.seeding();
        let mut tpn = vec![0u32; self.players.len()];
        for (k, &p) in order.iter().enumerate() {
            tpn[p] = k as u32 + 1;
        }
        let history = |p: usize| -> Vec<pairing::Round> {
            (0..self.played.len())
                .map(|k| match self.slot(k, p) {
                    Slot::Played { opponent, white, points } => pairing::Round::Played { opponent: tpn[opponent], colour: colour(white), points },
                    Slot::Forfeit { opponent, points, .. } => pairing::Round::Forfeit { opponent: tpn[opponent], points },
                    Slot::PairingBye => pairing::Round::PairingBye { points: 2 },
                    s => pairing::Round::Absent { points: s.points() },
                })
                .collect()
        };
        let players = order.iter().map(|&p| pairing::Player { tpn: tpn[p], history: history(p) }).collect();
        (order, players)
    }

    /// Everyone not withdrawn, in seeding order.
    fn round_robin_seeds(&self) -> Vec<usize> {
        self.seeding().into_iter().filter(|&p| self.players[p].status != Status::Withdrawn).collect()
    }

    fn settings(&self) -> pairing::Settings {
        pairing::Settings { rounds: self.rounds, win: 2, loss: 0, initial_colour: colour(self.white_first), baku: None }
    }

    /// Pairs the next Round. Players with a half-point bye get it and are Here again after.
    pub fn pair_next(&mut self) -> Result<()> {
        let (pairs, bye) = self.propose()?;
        if let System::RoundRobin { double } = self.system
            && self.seeds.is_empty()
        {
            self.seeds = self.round_robin_seeds();
            let n = self.seeds.len() as u32;
            self.rounds = (n + n % 2 - 1) * if double { 2 } else { 1 };
        }
        let half: Vec<usize> = (0..self.players.len()).filter(|&p| self.players[p].status == Status::HalfBye && !pairs.iter().any(|&(w, b)| w == p || b == p)).collect();
        for &p in &half {
            self.players[p].status = Status::Here;
        }
        self.played.push(Round { games: pairs.into_iter().map(|(white, black)| self.board(white, black)).collect(), bye, half });
        Ok(())
    }

    /// How a pairing of the last Round, made by hand, stands against the rules. Errors are
    /// pairings that cannot stand at all: a player twice, a withdrawn player, unknown players.
    pub fn check(&self, pairs: &[(usize, usize)], bye: Option<usize>) -> Result<Vec<Warning>> {
        let r = self.played.len().checked_sub(1).ok_or(Error::Invalid("tournaments.error.no_round"))?;
        let mut seen = vec![false; self.players.len()];
        for p in pairs.iter().flat_map(|&(w, b)| [w, b]).chain(bye) {
            let pl = self.player(p)?;
            if std::mem::replace(&mut seen[p], true) {
                return Err(Error::Invalid("tournaments.error.twice"));
            }
            if pl.status == Status::Withdrawn && matches!(self.system, System::Dutch) {
                return Err(Error::Invalid("tournaments.error.withdrawn"));
            }
        }
        let mut warnings = vec![];
        match self.system {
            System::Dutch => {
                for (p, pl) in self.players.iter().enumerate() {
                    if !seen[p] && !self.played[r].half.contains(&p) && pl.status == Status::Here {
                        warnings.push(Warning::Unpaired { player: pl.name.clone() });
                    }
                }
                // Judge it against the Round as it was before this pairing.
                let before = Tournament { played: self.played[..r].to_vec(), ..self.clone() };
                let (order, players) = before.dutch_players();
                let mut tpn = vec![0u32; self.players.len()];
                for (k, &p) in order.iter().enumerate() {
                    tpn[p] = k as u32 + 1;
                }
                let present: Vec<u32> = order.iter().filter(|&&p| seen[p]).map(|&p| tpn[p]).collect();
                let boards: Vec<Board> = pairs.iter().map(|&(w, b)| Board { white: tpn[w], black: tpn[b] }).collect();
                let name = |t: u32| self.players[order[t as usize - 1]].name.clone();
                match dutch::assess(&self.settings(), &players, &present, &boards) {
                    Ok(dutch::Verdict::Same | dutch::Verdict::Better { .. }) => {}
                    Ok(dutch::Verdict::Colours(bs)) => warnings.push(Warning::Colours { boards: bs.iter().map(|b| (name(b.white), name(b.black))).collect() }),
                    Ok(dutch::Verdict::Illegal { bracket }) => warnings.push(Warning::Illegal { score: bracket }),
                    Ok(dutch::Verdict::Worse { bracket, criterion: c }) => warnings.push(Warning::Worse { score: bracket, criterion: criterion(c).into() }),
                    Ok(dutch::Verdict::GeneratedLater { bracket }) => warnings.push(Warning::Order { score: bracket }),
                    // The rules themselves find no pairing: any the director makes is theirs.
                    Err(pairing::Error::NoLegalPairing) => {}
                    Err(e) => return Err(pairing_error(e)),
                }
            }
            System::RoundRobin { .. } => {
                let before = Tournament { played: self.played[..r].to_vec(), ..self.clone() };
                let (ours, our_bye) = before.propose()?;
                let mut a: Vec<_> = pairs.to_vec();
                let mut b = ours;
                a.sort_unstable();
                b.sort_unstable();
                if a != b || bye != our_bye {
                    warnings.push(Warning::NotTable);
                }
            }
        }
        Ok(warnings)
    }

    /// The director replaces the last Round's pairing, saying why, before any result is in.
    /// Gives the warnings it was kept with; the log keeps both pairings and the reason.
    pub fn set_pairing(&mut self, who: &str, pairs: &[(usize, usize)], bye: Option<usize>, why: &str) -> Result<Vec<Warning>> {
        if why.trim().is_empty() {
            return Err(Error::Invalid("clubs.error.why"));
        }
        let warnings = self.check(pairs, bye)?;
        let last = self.played.last().expect("check found a Round");
        if last.games.iter().any(|g| g.result.is_some()) {
            return Err(Error::Invalid("tournaments.error.has_results"));
        }
        let old = self.pairing_text(last);
        let half = last.half.clone();
        let new = Round { games: pairs.iter().map(|&(white, black)| self.board(white, black)).collect(), bye, half };
        let new_text = self.pairing_text(&new);
        *self.played.last_mut().unwrap() = new;
        let n = self.played.len();
        let why = if warnings.is_empty() { why.trim().to_string() } else { format!("{} ({})", why.trim(), warnings.iter().map(Warning::text).collect::<Vec<_>>().join("; ")) };
        self.note(who, "", "pairing", format!("{n}: {old}"), format!("{n}: {new_text}"), &why);
        Ok(warnings)
    }

    /// Pairs the last Round again from who is here now, for a late arrival or a withdrawal
    /// after pairing. Only before any result is in; logged.
    pub fn pair_again(&mut self, who: &str, why: &str) -> Result<()> {
        if why.trim().is_empty() {
            return Err(Error::Invalid("clubs.error.why"));
        }
        let last = self.played.pop().ok_or(Error::Invalid("tournaments.error.no_round"))?;
        if last.games.iter().any(|g| g.result.is_some()) {
            self.played.push(last);
            return Err(Error::Invalid("tournaments.error.has_results"));
        }
        for &p in &last.half {
            self.players[p].status = Status::HalfBye;
        }
        let seeds = self.seeds.clone();
        if self.played.is_empty() {
            // A round-robin draws its numbers again from who is in it now.
            self.seeds.clear();
        }
        if let Err(e) = self.pair_next() {
            self.seeds = seeds;
            self.played.push(last);
            return Err(e);
        }
        let old = self.pairing_text(&last);
        let new = self.pairing_text(self.played.last().unwrap());
        let n = self.played.len();
        self.note(who, "", "pairing", format!("{n}: {old}"), format!("{n}: {new}"), why);
        Ok(())
    }

    /// `Anna – Bo, Carl – Dan, bye Eva`.
    fn pairing_text(&self, r: &Round) -> String {
        let mut parts: Vec<String> = r.games.iter().map(|g| format!("{} – {}", self.players[g.white].name, self.players[g.black].name)).collect();
        if let Some(b) = r.bye {
            parts.push(format!("{} {}", t!("tournaments.bye"), self.players[b].name));
        }
        parts.join(", ")
    }

    /// Enters or corrects the result on `board` (from 0) of Round `round` (from 0). A change
    /// to a result already entered needs a reason and is logged.
    pub fn set_result(&mut self, who: &str, round: usize, board: usize, result: Option<Outcome>, why: &str) -> Result<()> {
        let g = *self.played.get(round).and_then(|r| r.games.get(board)).ok_or(Error::Invalid("tournaments.error.no_board"))?;
        if g.result == result {
            return Ok(());
        }
        if self.match_to.is_some() && result == Some(Outcome::Draw) {
            return Err(Error::Invalid("tournaments.error.match_draw"));
        }
        if let Some(old) = g.result {
            if why.trim().is_empty() {
                return Err(Error::Invalid("clubs.error.why"));
            }
            let player = format!("{} – {}", self.players[g.white].name, self.players[g.black].name);
            self.note(who, &player, "result", format!("{}: {}", round + 1, old.text()), format!("{}: {}", round + 1, result.map_or("", Outcome::text)), why);
        }
        self.played[round].games[board].result = result;
        Ok(())
    }

    /// The standings after the Rounds with every result in, by points then the tiebreaks.
    pub fn standings(&self) -> Vec<Standing> {
        tiebreak::standings(self)
    }

    /// Whether a Tournament read from a file holds together: every board, bye and seed names a
    /// player who is there, and no one plays themself.
    pub fn sound(&self) -> bool {
        let n = self.players.len();
        let ok = |p: &usize| *p < n;
        self.seeds.iter().all(ok) && self.played.iter().all(|r| r.games.iter().all(|g| ok(&g.white) && ok(&g.black) && g.white != g.black) && r.bye.iter().all(ok) && r.half.iter().all(ok))
    }

    /// Games between Club members with a result on the board, for the Club's rating:
    /// (white, black, White's score). Forfeits are not games.
    pub fn member_games(&self) -> Vec<(i64, i64, f64)> {
        self.played
            .iter()
            .flat_map(|r| &r.games)
            .filter_map(|g| {
                let o = g.result.filter(|o| o.played())?;
                Some((self.players[g.white].member?, self.players[g.black].member?, o.points().0 as f64 / 2.0))
            })
            .collect()
    }
}

fn colour(white: bool) -> Colour {
    if white { Colour::White } else { Colour::Black }
}

fn status_id(s: Status) -> &'static str {
    match s {
        Status::Away => "away",
        Status::Here => "here",
        Status::HalfBye => "half_bye",
        Status::Withdrawn => "withdrawn",
    }
}

/// FIDE titles first, then the rest, for seeding equal ratings (C.04.2 2.3.2).
fn title_rank(t: &str) -> u8 {
    match t {
        "GM" => 0,
        "IM" => 1,
        "WGM" => 2,
        "FM" => 3,
        "WIM" => 4,
        "CM" => 5,
        "WFM" => 6,
        "WCM" => 7,
        _ => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(names: &[(&str, u32)]) -> Tournament {
        let mut t = Tournament::new("Club championship", System::Dutch, 5, "2026-11-05").unwrap();
        for &(n, r) in names {
            let p = t.register(Player { name: n.into(), rating: r, ..Player::default() }).unwrap();
            t.set_status("Mette", p, Status::Here).unwrap();
        }
        t
    }

    fn by_name(t: &Tournament, n: &str) -> usize {
        t.players.iter().position(|p| p.name == n).unwrap()
    }

    fn pairs(t: &Tournament) -> Vec<(String, String)> {
        t.played.last().unwrap().games.iter().map(|g| (t.players[g.white].name.clone(), t.players[g.black].name.clone())).collect()
    }

    #[test]
    fn a_go_handicap_gives_the_weaker_player_black_and_a_stone_a_hundred_points() {
        let h = Handicap::default();
        assert_eq!([h.of(2000, 2000), h.of(2000, 1951), h.of(2000, 1950), h.of(1950, 2000), h.of(2400, 1000)], [0, 0, 1, 1, 9]);
        assert_eq!(Handicap { reduction: 2, max: 4 }.of(2000, 1400), 4);
        assert_eq!(Handicap { reduction: 2, max: 4 }.of(2000, 1850), 0);
        let mut t = named(&[("Anna", 2100), ("Bo", 1900), ("Carl", 2000), ("Dan", 1800)]);
        assert_eq!(t.set_game("chess", Some(h), None), Err(Error::Invalid("tournaments.error.variant")), "go only");
        t.set_game("go-19", Some(h), None).unwrap();
        t.pair_next().unwrap();
        // The Dutch system pairs Anna – Bo and Dan – Carl; Dan, the weaker, takes Black.
        assert_eq!(pairs(&t), [("Anna".into(), "Bo".into()), ("Carl".into(), "Dan".into())]);
        assert_eq!(t.played[0].games.iter().map(|g| g.handicap).collect::<Vec<_>>(), [2, 2]);
        assert_eq!(t.set_game("go-9", None, None), Err(Error::Invalid("tournaments.error.started")));
    }

    #[test]
    fn a_backgammon_tournament_plays_matches_that_cannot_be_drawn() {
        let mut t = named(&[("Anna", 2100), ("Bo", 1900)]);
        assert_eq!(t.set_game("backgammon", None, None), Err(Error::Invalid("tournaments.error.match_to")), "a match length is needed");
        assert_eq!(t.set_game("go-9", None, Some(7)), Err(Error::Invalid("tournaments.error.match_to")));
        assert_eq!(t.set_game("backgammon", None, Some(0)), Err(Error::Invalid("tournaments.error.match_to")));
        t.set_game("backgammon", None, Some(7)).unwrap();
        t.pair_next().unwrap();
        assert_eq!(t.set_result("Mette", 0, 0, Some(Outcome::Draw), ""), Err(Error::Invalid("tournaments.error.match_draw")));
        t.set_result("Mette", 0, 0, Some(Outcome::Black), "").unwrap();
        assert_eq!(t.points(by_name(&t, "Bo")), 2);
    }

    #[test]
    fn round_one_pairs_top_half_against_bottom_half_and_gives_the_lowest_the_bye() {
        let mut t = named(&[("Anna", 2100), ("Bo", 1900), ("Carl", 2000), ("Dan", 1800), ("Eva", 1700)]);
        t.pair_next().unwrap();
        assert_eq!(pairs(&t), [("Anna".into(), "Bo".into()), ("Dan".into(), "Carl".into())]);
        assert_eq!(t.played[0].bye, Some(by_name(&t, "Eva")));
        assert_eq!(t.points(by_name(&t, "Eva")), 2);
        assert_eq!(t.can_pair(), Err(Error::Invalid("tournaments.error.results_missing")));
    }

    #[test]
    fn away_players_and_half_byes_are_not_paired_and_a_late_entry_starts_from_nothing() {
        let mut t = named(&[("Anna", 2100), ("Bo", 1900), ("Carl", 2000), ("Dan", 1800)]);
        t.set_status("Mette", by_name(&t, "Dan"), Status::Away).unwrap();
        t.set_status("Mette", by_name(&t, "Bo"), Status::HalfBye).unwrap();
        t.pair_next().unwrap();
        assert_eq!(pairs(&t), [("Anna".into(), "Carl".into())]);
        assert_eq!(t.played[0].bye, None);
        assert_eq!(t.slot(0, by_name(&t, "Bo")), Slot::HalfBye);
        assert_eq!(t.players[by_name(&t, "Bo")].status, Status::Here, "here again after the bye");
        assert_eq!(t.slot(0, by_name(&t, "Dan")), Slot::Absent);
        t.set_result("Mette", 0, 0, Some(Outcome::White), "").unwrap();
        let eva = t.register(Player { name: "Eva".into(), rating: 2200, ..Player::default() }).unwrap();
        assert_eq!(t.players[eva].joined, 1);
        t.set_status("Mette", eva, Status::Here).unwrap();
        t.pair_next().unwrap();
        assert_eq!(t.points(eva), 0);
        assert_eq!(t.played[1].games.len() + t.played[1].bye.iter().count(), 2, "Anna, Bo, Carl and Eva are paired: {:?}", pairs(&t));
    }

    #[test]
    fn a_withdrawn_player_keeps_their_results_and_is_not_paired_again() {
        let mut t = named(&[("Anna", 2100), ("Bo", 1900), ("Carl", 2000), ("Dan", 1800)]);
        t.pair_next().unwrap();
        for b in 0..2 {
            t.set_result("Mette", 0, b, Some(Outcome::White), "").unwrap();
        }
        let dan = by_name(&t, "Dan");
        let before = t.points(dan);
        t.set_status("Mette", dan, Status::Withdrawn).unwrap();
        assert_eq!(t.log.last().unwrap().what, "status");
        t.pair_next().unwrap();
        assert!(t.played[1].games.iter().all(|g| g.white != dan && g.black != dan));
        assert_eq!(t.points(dan), before);
    }

    #[test]
    fn a_corrected_result_needs_a_reason_and_is_logged() {
        let mut t = named(&[("Anna", 2100), ("Bo", 1900)]);
        t.pair_next().unwrap();
        t.set_result("Mette", 0, 0, Some(Outcome::White), "").unwrap();
        assert!(t.log.is_empty(), "entering a result is not a correction");
        assert_eq!(t.set_result("Mette", 0, 0, Some(Outcome::Draw), " "), Err(Error::Invalid("clubs.error.why")));
        t.set_result("Mette", 0, 0, Some(Outcome::Draw), "Score sheets say draw").unwrap();
        let e = t.log.last().unwrap();
        assert_eq!((e.what.as_str(), e.old.as_str(), e.new.as_str(), e.why.as_str(), e.who.as_str()), ("result", "1: 1-0", "1: ½-½", "Score sheets say draw", "Mette"));
        assert_eq!(t.points(by_name(&t, "Bo")), 1);
    }

    #[test]
    fn a_hand_pairing_that_breaks_a_rule_warns_and_is_logged_with_its_reason() {
        let mut t = named(&[("Anna", 2100), ("Bo", 1900), ("Carl", 2000), ("Dan", 1800)]);
        t.pair_next().unwrap();
        let [a, b, c, d] = ["Anna", "Bo", "Carl", "Dan"].map(|n| by_name(&t, n));
        // Round 1: Anna – Bo, Dan – Carl. White wins both.
        t.set_result("Mette", 0, 0, Some(Outcome::White), "").unwrap();
        t.set_result("Mette", 0, 1, Some(Outcome::White), "").unwrap();
        t.pair_next().unwrap();
        // The rules' own pairing raises nothing.
        let r = &t.played[1];
        let ours: Vec<_> = r.games.iter().map(|g| (g.white, g.black)).collect();
        assert_eq!(t.check(&ours, None).unwrap(), []);
        // A rematch is illegal.
        assert!(matches!(t.check(&[(b, a), (c, d)], None).unwrap()[..], [Warning::Illegal { .. }]));
        // Players twice, or a reason missing, cannot stand at all.
        assert_eq!(t.check(&[(a, b), (a, c)], None), Err(Error::Invalid("tournaments.error.twice")));
        assert_eq!(t.set_pairing("Mette", &[(b, a), (c, d)], None, ""), Err(Error::Invalid("clubs.error.why")));
        let w = t.set_pairing("Mette", &[(b, a), (c, d)], None, "Anna must leave early").unwrap();
        assert_eq!(w.len(), 1);
        let e = t.log.last().unwrap();
        assert_eq!(e.what, "pairing");
        assert!(e.new.starts_with("2: Bo – Anna"), "{}", e.new);
        assert!(e.why.starts_with("Anna must leave early ("), "{}", e.why);
        // With a result in, the pairing stands.
        t.set_result("Mette", 1, 0, Some(Outcome::Draw), "").unwrap();
        assert_eq!(t.set_pairing("Mette", &ours, None, "x"), Err(Error::Invalid("tournaments.error.has_results")));
    }

    #[test]
    fn a_colour_swap_on_the_rules_pairs_is_a_colour_warning() {
        let mut t = named(&[("Anna", 2100), ("Bo", 1900), ("Carl", 2000), ("Dan", 1800)]);
        t.pair_next().unwrap();
        let swapped: Vec<_> = t.played[0].games.iter().map(|g| (g.black, g.white)).collect();
        assert!(matches!(&t.check(&swapped, None).unwrap()[..], [Warning::Colours { boards }] if boards.len() == 2));
    }

    #[test]
    fn pairing_again_takes_in_a_late_arrival() {
        let mut t = named(&[("Anna", 2100), ("Bo", 1900), ("Carl", 2000)]);
        t.pair_next().unwrap();
        assert!(t.played[0].bye.is_some());
        let dan = t.register(Player { name: "Dan".into(), rating: 1800, ..Player::default() }).unwrap();
        assert_eq!(t.players[dan].joined, 1, "registered after pairing");
        t.set_status("Mette", dan, Status::Here).unwrap();
        assert_eq!(t.pair_again("Mette", ""), Err(Error::Invalid("clubs.error.why")));
        t.pair_again("Mette", "Dan came late").unwrap();
        assert_eq!(t.played.len(), 1);
        assert_eq!((t.played[0].games.len(), t.played[0].bye), (2, None));
        assert_eq!(t.log.last().unwrap().what, "pairing");
    }

    #[test]
    fn a_round_robin_follows_the_berger_table_and_takes_no_late_entries() {
        let mut t = Tournament::new("Closed", System::RoundRobin { double: false }, 0, "").unwrap();
        for (n, r) in [("Anna", 2100), ("Bo", 1900), ("Carl", 2000), ("Dan", 1800)] {
            t.register(Player { name: n.into(), rating: r, ..Player::default() }).unwrap();
        }
        t.pair_next().unwrap();
        assert_eq!(t.rounds, 3);
        // Seeds by rating: Anna 1, Carl 2, Bo 3, Dan 4. Round 1 of the table: 1-4, 2-3.
        assert_eq!(pairs(&t), [("Anna".into(), "Dan".into()), ("Carl".into(), "Bo".into())]);
        assert_eq!(t.register(Player { name: "Eva".into(), ..Player::default() }), Err(Error::Invalid("tournaments.error.round_robin_started")));
        let [a, b, c, d] = ["Anna", "Bo", "Carl", "Dan"].map(|n| by_name(&t, n));
        assert_eq!(t.check(&[(a, b), (c, d)], None).unwrap(), [Warning::NotTable]);
        for b in 0..2 {
            t.set_result("Mette", 0, b, Some(Outcome::Draw), "").unwrap();
        }
        t.pair_next().unwrap();
        t.pair_next().unwrap_err();
    }

    #[test]
    fn standings_rank_by_points_then_tiebreaks_and_share_a_place_when_level() {
        let mut t = named(&[("Anna", 2100), ("Bo", 1900)]);
        t.tiebreaks = vec![];
        t.pair_next().unwrap();
        t.set_result("Mette", 0, 0, Some(Outcome::Draw), "").unwrap();
        let s = t.standings();
        assert_eq!((s[0].rank, s[1].rank, s[0].points), (1, 1, 1));
        t.set_result("Mette", 0, 0, Some(Outcome::Black), "x").unwrap();
        let s = t.standings();
        assert_eq!((t.players[s[0].player].name.as_str(), s[0].rank, s[1].rank), ("Bo", 1, 2));
    }

    #[test]
    fn member_games_leave_out_guests_and_forfeits() {
        let mut t = Tournament::new("T", System::Dutch, 3, "").unwrap();
        for (n, m) in [("A", Some(1)), ("B", Some(2)), ("C", Some(3)), ("D", None)] {
            let p = t.register(Player { name: n.into(), member: m, ..Player::default() }).unwrap();
            t.set_status("x", p, Status::Here).unwrap();
        }
        assert_eq!(t.register(Player { name: "A again".into(), member: Some(1), ..Player::default() }), Err(Error::Invalid("tournaments.error.registered")));
        t.pair_next().unwrap();
        t.set_result("x", 0, 0, Some(Outcome::Black), "").unwrap();
        t.set_result("x", 0, 1, Some(Outcome::WhiteForfeit), "").unwrap();
        let g = t.member_games();
        assert_eq!(g.len(), 1, "{g:?}");
        assert_eq!(g[0].2, 0.0);
    }

    #[test]
    fn a_tournament_reads_back_from_its_json() {
        let mut t = named(&[("Anna", 2100), ("Bo", 1900)]);
        t.pair_next().unwrap();
        let back: Tournament = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
        assert!(t.sound());
        let mut bad = t.clone();
        bad.played[0].games[0].black = bad.played[0].games[0].white;
        assert!(!bad.sound());
        bad.played[0].games[0].black = 7;
        assert!(!bad.sound());
    }
}
