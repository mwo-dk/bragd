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
use pairing::{Board, Colour, Criterion, drop_in, dutch, knockout, monrad, nordic, round_robin, scheveningen};
use serde::{Deserialize, Serialize};

use crate::club::{Entry, Error, Result};
use crate::tiebreak::{self, Tiebreak};

/// How the Rounds are paired. Each system has one arm in [`Tournament::propose`] and one in
/// [`Tournament::check`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum System {
    /// Swiss, FIDE Dutch (C.04.3); `baku` adds Baku acceleration (C.04.7).
    Dutch {
        #[serde(default)]
        baku: bool,
    },
    /// Round-robin by FIDE's Berger tables; `double` plays every pair twice.
    RoundRobin { double: bool },
    /// DSU's Monrad: numbers drawn by lot, each player meets the nearest one below not yet met.
    Monrad,
    /// DSU's Nordic Swiss (Dansk Schweizer): the strong half of a score group meets the weak half.
    Nordic,
    /// Knockout by FIDE's World Cup bracket; a drawn match needs its play-off winner.
    Knockout,
    /// Team 1 against team 2, every player meeting every player of the other team.
    Scheveningen { double: bool },
    /// Whoever is here is paired; an absence counts half a point for the pairing only.
    DropIn {
        #[serde(default)]
        standings: DropIn,
    },
}

/// How a Drop-in tournament ranks its players.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropIn {
    /// Points as a share of the most that could have been scored in the Rounds played.
    #[default]
    Percentage,
    /// All points.
    Total,
    /// The best `n` results.
    Best(u32),
}

impl System {
    /// Paired by score from whoever is here, over a declared number of Rounds, with a bye worth a
    /// point. The others fix their field and numbers when Round 1 is paired.
    pub fn swiss(self) -> bool {
        matches!(self, System::Dutch { .. } | System::Monrad | System::Nordic | System::DropIn { .. })
    }

    /// The text key of its name.
    pub fn key(self) -> &'static str {
        match self {
            System::Dutch { baku: false } => "tournaments.system.dutch",
            System::Dutch { baku: true } => "tournaments.system.dutch_baku",
            System::RoundRobin { double: false } => "tournaments.system.round_robin",
            System::RoundRobin { double: true } => "tournaments.system.double_round_robin",
            System::Monrad => "tournaments.system.monrad",
            System::Nordic => "tournaments.system.nordic",
            System::Knockout => "tournaments.system.knockout",
            System::Scheveningen { double: false } => "tournaments.system.scheveningen",
            System::Scheveningen { double: true } => "tournaments.system.double_scheveningen",
            System::DropIn { .. } => "tournaments.system.drop_in",
        }
    }

    /// The tournament type of a FIDE report (TRF line 192), from FIDE's code table. Systems
    /// FIDE does not define are `CUSTOM_`: Bragd's Scheveningen rotation is its own.
    pub fn trf(self) -> &'static str {
        match self {
            System::Dutch { baku: false } => "FIDE_DUTCH",
            System::Dutch { baku: true } => "FIDE_DUTCH_BAKU",
            System::RoundRobin { double: false } => "FIDE_ROUNDROBIN",
            System::RoundRobin { double: true } => "FIDE_DOUBLEROUNDROBIN",
            System::Monrad | System::Nordic | System::DropIn { .. } => "CUSTOM_SWISS",
            System::Knockout => "CUSTOM_KNOCKOUT",
            System::Scheveningen { .. } => "CUSTOM_SCHEVENINGEN",
        }
    }
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
    /// A Scheveningen's team: 1 or 2; 0 for none.
    pub team: u8,
}

impl Player {
    /// Guests pasted as a list, one a line: a name, then a rating if there is one, after a
    /// space, tab, comma or semicolon (`Carlsen, Magnus\t2830`). Blank lines are skipped.
    pub fn list(text: &str) -> Vec<Player> {
        let seps = |c: char| c.is_whitespace() || c == ',' || c == ';';
        text.lines()
            .filter_map(|line| {
                let line = line.trim();
                let (name, rating) = match line.rsplit_once(seps) {
                    Some((name, last)) if !last.is_empty() && last.chars().all(|c| c.is_ascii_digit()) => (name.trim_end_matches(seps), last.parse().ok()?),
                    _ => (line, 0),
                };
                (!name.is_empty()).then(|| Player { name: name.into(), rating, ..Player::default() })
            })
            .collect()
    }
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
    /// A knockout only: who won the play-off of a match without a winner on the board.
    #[serde(default)]
    pub playoff: Option<usize>,
}

impl Game {
    /// Who goes through: the winner on the board, or else the play-off's.
    pub fn winner(&self) -> Option<usize> {
        match self.result? {
            Outcome::White | Outcome::WhiteForfeit => Some(self.white),
            Outcome::Black | Outcome::BlackForfeit => Some(self.black),
            Outcome::Draw | Outcome::DoubleForfeit => self.playoff,
        }
    }
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

/// A Scheveningen Round's score, team 1 then team 2: board points in halves, and match
/// points (2 for a match won, 1 drawn) once every result of the Round is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct TeamScore {
    pub board_points: [u32; 2],
    pub match_points: [u32; 2],
}

impl TeamScore {
    /// The Rounds added up.
    pub fn total(rounds: &[TeamScore]) -> TeamScore {
        rounds.iter().fold(TeamScore::default(), |a, r| TeamScore {
            board_points: [a.board_points[0] + r.board_points[0], a.board_points[1] + r.board_points[1]],
            match_points: [a.match_points[0] + r.match_points[0], a.match_points[1] + r.match_points[1]],
        })
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
    /// Not the pairing of the knockout bracket.
    NotBracket,
    /// Not the pairing of the Scheveningen schedule.
    NotSchedule,
    /// These pairs have met before: never allowed in Monrad and the Nordic Swiss, put off in a
    /// Drop-in.
    Rematch { boards: Vec<(String, String)> },
    /// The bye goes to a player who had one already.
    SecondBye { player: String },
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
            Warning::NotBracket => t!("tournaments.warn.not_bracket"),
            Warning::NotSchedule => t!("tournaments.warn.not_schedule"),
            Warning::Rematch { boards } => {
                t!("tournaments.warn.rematch", "boards" => boards.iter().map(|(w, b)| format!("{w} – {b}")).collect::<Vec<_>>().join(", "))
            }
            Warning::SecondBye { player } => t!("tournaments.warn.second_bye", "player" => player),
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
        pairing::Error::NoWinner { .. } => "tournaments.error.playoff",
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
    /// In halves: all of them, or in a Drop-in the ones that count.
    pub points: u32,
    /// A Drop-in's Rounds that count, for its percentage; otherwise the Rounds with results.
    pub rounds: u32,
    /// One value per tiebreak, in the Tournament's order.
    pub tiebreaks: Vec<f64>,
}

impl Standing {
    /// The points as the standings write them; under a Drop-in's percentage, out of the Rounds
    /// that count.
    pub fn points_text(&self, system: System) -> String {
        if system != (System::DropIn { standings: DropIn::Percentage }) {
            return half_text(self.points);
        }
        // Points are in halves and a Round is worth 2.
        let percent = if self.rounds == 0 { 0 } else { (self.points * 100 + self.rounds) / (2 * self.rounds) };
        t!("tournaments.percentage", "points" => half_text(self.points), "rounds" => self.rounds.to_string(), "percent" => percent.to_string())
    }
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
    /// Rounds declared. A round-robin, knockout or Scheveningen sets it when its first Round is
    /// paired.
    pub rounds: u32,
    /// The colour of the top seed on board 1 in Round 1, drawn by lot.
    pub white_first: bool,
    pub tiebreaks: Vec<Tiebreak>,
    pub players: Vec<Player>,
    pub played: Vec<Round>,
    /// Numbers fixed when Round 1 is paired, `seeds[k]` having number `k + 1`: a round-robin's
    /// or a knockout's seeds; a Scheveningen's team 1 by board, then team 2; Monrad's draw by lot,
    /// late entries numbered on; with Baku acceleration, group GA in seeding order.
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
            system: System::Dutch { baku: false },
            rounds: 7,
            white_first: true,
            tiebreaks: tiebreak::defaults(System::Dutch { baku: false }),
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
        if system.swiss() && !(1..=99).contains(&rounds) {
            return Err(Error::Invalid("tournaments.error.rounds"));
        }
        if matches!(system, System::DropIn { standings: DropIn::Best(n) } if !(1..=99).contains(&n)) {
            return Err(Error::Invalid("tournaments.error.best"));
        }
        // The other systems' Rounds follow from their players, when Round 1 is paired.
        let rounds = if system.swiss() { rounds } else { 0 };
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
        let Some(h) = self.handicap else { return Game { white, black, result: None, handicap: 0, playoff: None } };
        let (rw, rb) = (self.players[white].rating, self.players[black].rating);
        let handicap = h.of(rw, rb);
        let (white, black) = if handicap > 0 && rb > rw { (black, white) } else { (white, black) };
        Game { white, black, result: None, handicap, playoff: None }
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
        if !self.seeds.is_empty() && !self.system.swiss() {
            return Err(Error::Invalid("tournaments.error.round_robin_started"));
        }
        if p.team > 2 {
            return Err(Error::Invalid("tournaments.error.teams"));
        }
        if p.member.is_some() && self.players.iter().any(|q| q.member == p.member) {
            return Err(Error::Invalid("tournaments.error.registered"));
        }
        let joined = self.played.len() as u32;
        self.players.push(Player { name: p.name.trim().into(), status: Status::Away, joined, ..p });
        let p = self.players.len() - 1;
        if self.system == System::Monrad && !self.seeds.is_empty() {
            // A late entry takes the next number.
            self.seeds.push(p);
        }
        Ok(p)
    }

    /// Puts a player in a Scheveningen's team 1 or 2, or none (0), before Round 1.
    pub fn set_team(&mut self, p: usize, team: u8) -> Result<()> {
        self.player(p)?;
        if !self.played.is_empty() {
            return Err(Error::Invalid("tournaments.error.started"));
        }
        if team > 2 {
            return Err(Error::Invalid("tournaments.error.teams"));
        }
        self.players[p].team = team;
        Ok(())
    }

    /// Checks a player in or out, gives a half-point bye, or withdraws them. A withdrawal and
    /// a return are logged.
    pub fn set_status(&mut self, who: &str, p: usize, status: Status) -> Result<()> {
        let old = self.player(p)?.status;
        if old == status {
            return Ok(());
        }
        if status == Status::HalfBye && matches!(self.system, System::DropIn { .. }) {
            return Err(Error::Invalid("tournaments.error.drop_in_half_bye"));
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
            return if self.system.swiss() { Slot::PairingBye } else { Slot::Absent };
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
        if self.system == System::Knockout && self.played.last().is_some_and(|r| r.games.iter().any(|g| g.winner().is_none())) {
            return Err(Error::Invalid("tournaments.error.playoff"));
        }
        if (!self.played.is_empty() || self.system.swiss()) && self.played.len() as u32 >= self.rounds {
            return Err(Error::Invalid("tournaments.error.over"));
        }
        if self.played.is_empty() && matches!(self.system, System::Scheveningen { .. }) {
            self.first_seeds()?;
        }
        Ok(())
    }

    /// The numbers a system fixes when Round 1 is paired (see [`Tournament::seeds`]).
    fn first_seeds(&self) -> Result<Vec<usize>> {
        let field = || self.seeding().into_iter().filter(|&p| self.players[p].status != Status::Withdrawn);
        Ok(match self.system {
            System::Dutch { baku: true } => {
                let f: Vec<usize> = field().collect();
                let ga = (pairing::baku_group(f.len() as u32) as usize).min(f.len());
                f[..ga].to_vec()
            }
            System::Dutch { baku: false } | System::Nordic | System::DropIn { .. } => vec![],
            System::Monrad => draw(self.players.len()),
            System::RoundRobin { .. } | System::Knockout => field().collect(),
            System::Scheveningen { .. } => {
                let team = |n: u8| field().filter(move |&p| self.players[p].team == n);
                let (a, b): (Vec<usize>, Vec<usize>) = (team(1).collect(), team(2).collect());
                if a.is_empty() || a.len() != b.len() {
                    return Err(Error::Invalid("tournaments.error.teams"));
                }
                a.into_iter().chain(b).collect()
            }
        })
    }

    /// The pairing system's boards and bye for the next Round, by player index, given who is
    /// here now.
    pub fn propose(&self) -> Result<Proposal> {
        self.can_pair()?;
        let r = self.played.len() as u32;
        let seeds = if self.seeds.is_empty() { self.first_seeds()? } else { self.seeds.clone() };
        let too_few = || Error::Invalid("tournaments.error.too_few");
        let (order, pr) = match self.system {
            System::Dutch { .. } | System::Monrad | System::Nordic | System::DropIn { .. } => {
                // Monrad numbers by the draw, the others by strength.
                let order = if self.system == System::Monrad { seeds.clone() } else { self.seeding() };
                let players = self.histories(&order);
                let present: Vec<u32> = (0..order.len()).filter(|&k| self.players[order[k]].status == Status::Here).map(|k| k as u32 + 1).collect();
                if present.len() < 2 {
                    return Err(too_few());
                }
                let s = self.settings(&order, &seeds);
                let pr = match self.system {
                    System::Dutch { .. } => dutch::pair(&s, &players, &present),
                    System::Monrad => monrad::pair(&s, &players, &present),
                    System::Nordic => nordic::pair(&s, &players, &present),
                    _ => drop_in::pair(&s, &players, &present),
                };
                (order, pr.map_err(pairing_error)?)
            }
            _ if seeds.len() < 2 => return Err(too_few()),
            System::RoundRobin { double } => {
                let pr = round_robin::pair(seeds.len() as u32, if double { 2 } else { 1 }, double, r + 1).map_err(pairing_error)?;
                (seeds, pr)
            }
            System::Knockout => {
                let number = |p: usize| seeds.iter().position(|&q| q == p).map_or(0, |k| k as u32 + 1);
                let winners: Vec<Vec<u32>> = self.played.iter().map(|round| round.games.iter().filter_map(Game::winner).map(number).collect()).collect();
                let pr = knockout::pair(seeds.len() as u32, colour(self.white_first), &winners).map_err(pairing_error)?;
                (seeds, pr)
            }
            System::Scheveningen { double } => {
                let pr = scheveningen::pair(seeds.len() as u32 / 2, if double { 2 } else { 1 }, r + 1).map_err(pairing_error)?;
                (seeds, pr)
            }
        };
        let at = |t: u32| order[t as usize - 1];
        Ok((pr.boards.iter().map(|b| (at(b.white), at(b.black))).collect(), pr.bye.map(at)))
    }

    /// Each player's history as the Swiss systems read it, numbered by their place in `order`.
    fn histories(&self, order: &[usize]) -> Vec<pairing::Player> {
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
        order.iter().map(|&p| pairing::Player { tpn: tpn[p], history: history(p) }).collect()
    }

    /// The settings for players numbered by `order`; `seeds` holds group GA under Baku
    /// acceleration, whose last player marks the group as late entries are numbered in.
    fn settings(&self, order: &[usize], seeds: &[usize]) -> pairing::Settings {
        let baku = match self.system {
            System::Dutch { baku: true } => seeds.last().and_then(|&p| order.iter().position(|&q| q == p)).map(|k| k as u32 + 1),
            _ => None,
        };
        pairing::Settings { rounds: self.rounds, win: 2, loss: 0, initial_colour: colour(self.white_first), baku }
    }

    /// Pairs the next Round. Players with a half-point bye get it and are Here again after.
    /// Round 1 fixes the system's numbers and, where they follow from the field, the Rounds.
    pub fn pair_next(&mut self) -> Result<()> {
        let first = self.played.is_empty() && self.seeds.is_empty();
        if first {
            self.seeds = self.first_seeds()?;
        }
        let (pairs, bye) = match self.propose() {
            Ok(x) => x,
            Err(e) => {
                if first {
                    self.seeds.clear();
                }
                return Err(e);
            }
        };
        if first {
            let n = self.seeds.len() as u32;
            match self.system {
                System::RoundRobin { double } => self.rounds = (n + n % 2 - 1) * if double { 2 } else { 1 },
                System::Knockout => self.rounds = n.next_power_of_two().trailing_zeros(),
                System::Scheveningen { double } => self.rounds = n / 2 * if double { 2 } else { 1 },
                _ => {}
            }
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
            if pl.status == Status::Withdrawn && self.system.swiss() {
                return Err(Error::Invalid("tournaments.error.withdrawn"));
            }
        }
        let mut warnings = vec![];
        if self.system.swiss() {
            for (p, pl) in self.players.iter().enumerate() {
                if !seen[p] && !self.played[r].half.contains(&p) && pl.status == Status::Here {
                    warnings.push(Warning::Unpaired { player: pl.name.clone() });
                }
            }
        }
        // Judge it against the Round as it was before this pairing.
        let before = Tournament { played: self.played[..r].to_vec(), ..self.clone() };
        let name = |p: usize| self.players[p].name.clone();
        // Who meet again, and a second bye: named for every Swiss, as no Swiss allows them.
        if self.system.swiss() {
            let met = |a: usize, b: usize| (0..r).any(|k| matches!(self.slot(k, a), Slot::Played { opponent, .. } if opponent == b));
            let again: Vec<(String, String)> = pairs.iter().filter(|&&(w, b)| met(w, b)).map(|&(w, b)| (name(w), name(b))).collect();
            if !again.is_empty() {
                warnings.push(Warning::Rematch { boards: again });
            }
            if let Some(p) = bye
                && (0..r).any(|k| self.slot(k, p) == Slot::PairingBye)
            {
                warnings.push(Warning::SecondBye { player: name(p) });
            }
        }
        let named = warnings.iter().any(|w| matches!(w, Warning::Rematch { .. } | Warning::SecondBye { .. }));
        match self.system {
            System::Dutch { .. } => {
                let order = self.seeding();
                let players = before.histories(&order);
                let mut tpn = vec![0u32; self.players.len()];
                for (k, &p) in order.iter().enumerate() {
                    tpn[p] = k as u32 + 1;
                }
                let present: Vec<u32> = order.iter().filter(|&&p| seen[p]).map(|&p| tpn[p]).collect();
                let boards: Vec<Board> = pairs.iter().map(|&(w, b)| Board { white: tpn[w], black: tpn[b] }).collect();
                let at = |t: u32| name(order[t as usize - 1]);
                match dutch::assess(&self.settings(&order, &self.seeds), &players, &present, &boards) {
                    Ok(dutch::Verdict::Same | dutch::Verdict::Better { .. }) => {}
                    Ok(dutch::Verdict::Colours(bs)) => warnings.push(Warning::Colours { boards: bs.iter().map(|b| (at(b.white), at(b.black))).collect() }),
                    // A rematch or second bye named above is the broken rule; say it once.
                    Ok(dutch::Verdict::Illegal { .. }) if named => {}
                    Ok(dutch::Verdict::Illegal { bracket }) => warnings.push(Warning::Illegal { score: bracket }),
                    Ok(dutch::Verdict::Worse { bracket, criterion: c }) => warnings.push(Warning::Worse { score: bracket, criterion: criterion(c).into() }),
                    Ok(dutch::Verdict::GeneratedLater { bracket }) => warnings.push(Warning::Order { score: bracket }),
                    // The rules themselves find no pairing: any the director makes is theirs.
                    Err(pairing::Error::NoLegalPairing) => {}
                    Err(e) => return Err(pairing_error(e)),
                }
            }
            System::Monrad | System::Nordic | System::DropIn { .. } => {}
            System::RoundRobin { .. } | System::Knockout | System::Scheveningen { .. } => {
                let (ours, our_bye) = before.propose()?;
                let mut a: Vec<_> = pairs.to_vec();
                let mut b = ours;
                a.sort_unstable();
                b.sort_unstable();
                if a != b || bye != our_bye {
                    warnings.push(match self.system {
                        System::Knockout => Warning::NotBracket,
                        System::Scheveningen { .. } => Warning::NotSchedule,
                        _ => Warning::NotTable,
                    });
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
        self.settled(round, g, Game { result, ..g })?;
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

    /// A knockout's match winner cannot change once the next Round is paired from it.
    fn settled(&self, round: usize, old: Game, new: Game) -> Result<()> {
        if self.system == System::Knockout && round + 1 < self.played.len() && old.winner() != new.winner() {
            return Err(Error::Invalid("tournaments.error.knockout_settled"));
        }
        Ok(())
    }

    /// Enters who won the play-off of a drawn knockout match on `board` of Round `round`, or
    /// clears it. A change to a winner already entered needs a reason and is logged.
    pub fn set_playoff(&mut self, who: &str, round: usize, board: usize, winner: Option<usize>, why: &str) -> Result<()> {
        let g = *self.played.get(round).and_then(|r| r.games.get(board)).ok_or(Error::Invalid("tournaments.error.no_board"))?;
        if self.system != System::Knockout || winner.is_some_and(|w| w != g.white && w != g.black) {
            return Err(Error::Invalid("tournaments.error.playoff_player"));
        }
        if g.playoff == winner {
            return Ok(());
        }
        self.settled(round, g, Game { playoff: winner, ..g })?;
        if let Some(old) = g.playoff {
            if why.trim().is_empty() {
                return Err(Error::Invalid("clubs.error.why"));
            }
            let player = format!("{} – {}", self.players[g.white].name, self.players[g.black].name);
            let new = winner.map_or(String::new(), |w| self.players[w].name.clone());
            self.note(who, &player, "playoff", format!("{}: {}", round + 1, self.players[old].name), format!("{}: {new}", round + 1), why);
        }
        self.played[round].games[board].playoff = winner;
        Ok(())
    }

    /// A knockout's players who go to the next Round without a game in Round `r` (from 0), in
    /// seed order: those the bracket gives a bye. Empty for every other system.
    pub fn through(&self, r: usize) -> Vec<usize> {
        if self.system != System::Knockout || r >= self.played.len() {
            return vec![];
        }
        let on_board = |k: usize, p: usize| self.played[k].games.iter().any(|g| g.white == p || g.black == p);
        let mut alive: Vec<usize> = self.seeds.clone();
        for k in 0..r {
            alive.retain(|&p| !on_board(k, p) || self.played[k].games.iter().any(|g| g.winner() == Some(p)));
        }
        alive.retain(|&p| !on_board(r, p));
        alive
    }

    /// A Scheveningen's score by Round, team 1 against team 2. Empty for every other system.
    pub fn team_scores(&self) -> Vec<TeamScore> {
        if !matches!(self.system, System::Scheveningen { .. }) {
            return vec![];
        }
        self.played
            .iter()
            .enumerate()
            .map(|(r, round)| {
                let mut s = TeamScore::default();
                for p in 0..self.players.len() {
                    if let team @ 1..=2 = self.players[p].team {
                        s.board_points[team as usize - 1] += self.slot(r, p).points();
                    }
                }
                if round.complete() {
                    let [a, b] = s.board_points;
                    s.match_points = match a.cmp(&b) {
                        std::cmp::Ordering::Greater => [2, 0],
                        std::cmp::Ordering::Less => [0, 2],
                        std::cmp::Ordering::Equal => [1, 1],
                    };
                }
                s
            })
            .collect()
    }

    /// The standings after the Rounds with every result in, by points then the tiebreaks.
    pub fn standings(&self) -> Vec<Standing> {
        tiebreak::standings(self)
    }

    /// Each player's Drop-in points and the Rounds they count, by `by`. An absence counts for
    /// the pairing only.
    pub(crate) fn drop_in_points(&self, by: DropIn) -> Vec<(u32, u32)> {
        let by = match by {
            DropIn::Percentage => drop_in::Standing::Percentage,
            DropIn::Total => drop_in::Standing::Total,
            DropIn::Best(n) => drop_in::Standing::Best(n),
        };
        let order: Vec<usize> = (0..self.players.len()).collect();
        let mut out = vec![(0, 0); order.len()];
        for row in drop_in::standings(&self.histories(&order), by) {
            out[row.tpn as usize - 1] = (row.points, row.rounds);
        }
        out
    }

    /// Whether a Tournament read from a file holds together: every board, bye and seed names a
    /// player who is there, and no one plays themself.
    pub fn sound(&self) -> bool {
        let n = self.players.len();
        let ok = |p: &usize| *p < n;
        self.seeds.iter().all(ok) && self.played.iter().all(|r| r.games.iter().all(|g| ok(&g.white) && ok(&g.black) && g.white != g.black && g.playoff.iter().all(|w| *w == g.white || *w == g.black)) && r.bye.iter().all(ok) && r.half.iter().all(ok))
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

/// The numbers `0..n` in an order drawn by lot.
fn draw(n: usize) -> Vec<usize> {
    use std::hash::BuildHasher;
    // Each RandomState is keyed afresh from the system's random source.
    let lot = std::hash::RandomState::new();
    let mut v: Vec<usize> = (0..n).collect();
    v.sort_by_cached_key(|&p| lot.hash_one(p));
    v
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
        let mut t = Tournament::new("Club championship", System::Dutch { baku: false }, 5, "2026-11-05").unwrap();
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
        // A rematch is illegal, and the warning names who meet again.
        assert_eq!(t.check(&[(b, a), (c, d)], None).unwrap(), [Warning::Rematch { boards: vec![("Bo".into(), "Anna".into()), ("Carl".into(), "Dan".into())] }]);
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
        let mut t = Tournament::new("T", System::Dutch { baku: false }, 3, "").unwrap();
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

    /// A Tournament of `system` with these players checked in, each with a team when given.
    fn field(system: System, rounds: u32, names: &[(&str, u32, u8)]) -> Tournament {
        let mut t = Tournament::new("Club evening", system, rounds, "2026-11-05").unwrap();
        for &(n, r, team) in names {
            let p = t.register(Player { name: n.into(), rating: r, team, ..Player::default() }).unwrap();
            t.set_status("Mette", p, Status::Here).unwrap();
        }
        t
    }

    const FOUR: [(&str, u32, u8); 4] = [("Anna", 2100, 1), ("Bo", 1900, 2), ("Carl", 2000, 1), ("Dan", 1800, 2)];

    fn all_white_win(t: &mut Tournament) {
        let r = t.played.len() - 1;
        for b in 0..t.played[r].games.len() {
            t.set_result("Mette", r, b, Some(Outcome::White), "").unwrap();
        }
    }

    #[test]
    fn monrad_pairs_by_the_draw_and_numbers_a_late_entry_on() {
        let mut t = field(System::Monrad, 3, &FOUR);
        let [a, b, c, d] = ["Anna", "Bo", "Carl", "Dan"].map(|n| by_name(&t, n));
        // The draw gave Dan 1, Carl 2, Bo 3, Anna 4: 1 meets 2, 3 meets 4, the lower placed White.
        t.seeds = vec![d, c, b, a];
        t.pair_next().unwrap();
        assert_eq!(pairs(&t), [("Carl".into(), "Dan".into()), ("Anna".into(), "Bo".into())]);
        let eva = t.register(Player { name: "Eva".into(), ..Player::default() }).unwrap();
        assert_eq!(t.seeds, [d, c, b, a, eva]);
        // With no draw made, Round 1 draws every player a number.
        let mut t = field(System::Monrad, 3, &FOUR);
        t.pair_next().unwrap();
        let mut drawn = t.seeds.clone();
        drawn.sort_unstable();
        assert_eq!(drawn, [0, 1, 2, 3]);
        assert_eq!(t.system.trf(), "CUSTOM_SWISS");
    }

    #[test]
    fn a_hand_pairing_in_monrad_warns_of_a_rematch_a_second_bye_and_a_player_left_out() {
        let mut t = field(System::Monrad, 3, &[("Anna", 0, 0), ("Bo", 0, 0), ("Carl", 0, 0)]);
        let [a, b, c] = ["Anna", "Bo", "Carl"].map(|n| by_name(&t, n));
        t.seeds = vec![a, b, c];
        t.pair_next().unwrap();
        // 1 meets 2 and 3 has the bye.
        assert_eq!(t.played[0].bye, Some(c));
        all_white_win(&mut t);
        t.pair_next().unwrap();
        let ours: Vec<_> = t.played[1].games.iter().map(|g| (g.white, g.black)).collect();
        assert_eq!(t.check(&ours, t.played[1].bye).unwrap(), []);
        let w = t.check(&[(a, b)], Some(c)).unwrap();
        assert_eq!(w, [Warning::Rematch { boards: vec![("Anna".into(), "Bo".into())] }, Warning::SecondBye { player: "Carl".into() }]);
        assert_eq!(t.check(&[(a, c)], None).unwrap(), [Warning::Unpaired { player: "Bo".into() }]);
    }

    #[test]
    fn the_nordic_swiss_meets_the_strong_half_with_the_weak_half() {
        let mut t = field(System::Nordic, 3, &FOUR);
        t.pair_next().unwrap();
        // By strength a = Anna, Carl and b = Bo, Dan; in Round 1 the weakest of b has White.
        let mut met: Vec<(String, String)> = pairs(&t).into_iter().map(|(w, b)| if w < b { (w, b) } else { (b, w) }).collect();
        met.sort();
        assert_eq!(met, [("Anna".into(), "Bo".into()), ("Carl".into(), "Dan".into())]);
        assert!(pairs(&t).iter().any(|(w, _)| w == "Dan"), "the weakest of b has White: {:?}", pairs(&t));
        assert_eq!(t.tiebreaks, tiebreak::defaults(System::Dutch { baku: false }));
    }

    #[test]
    fn baku_acceleration_pairs_group_ga_among_itself_in_round_one() {
        let eight = [("A", 2400, 0), ("B", 2300, 0), ("C", 2200, 0), ("D", 2100, 0), ("E", 2000, 0), ("F", 1900, 0), ("G", 1800, 0), ("H", 1700, 0)];
        let mut plain = field(System::Dutch { baku: false }, 7, &eight);
        plain.pair_next().unwrap();
        assert!(plain.seeds.is_empty());
        let mut t = field(System::Dutch { baku: true }, 7, &eight);
        t.pair_next().unwrap();
        assert_eq!(t.seeds.len(), 4, "GA is the top half");
        let foe = |t: &Tournament, n: &str| match t.slot(0, by_name(t, n)).opponent() {
            Some(o) => t.players[o].name.clone(),
            None => String::new(),
        };
        assert_eq!((foe(&plain, "A"), foe(&t, "A")), ("E".into(), "C".into()));
        assert_eq!(t.system.trf(), "FIDE_DUTCH_BAKU");
    }

    #[test]
    fn a_knockout_follows_its_bracket_and_a_drawn_match_needs_its_play_off_winner() {
        let mut t = field(System::Knockout, 0, &[("Anna", 2400, 0), ("Bo", 2300, 0), ("Carl", 2200, 0), ("Dan", 2100, 0), ("Eva", 2000, 0)]);
        t.pair_next().unwrap();
        assert_eq!(t.rounds, 3);
        // Eight places: seeds 1-3 have byes, 4 meets 5.
        let mut met: Vec<String> = pairs(&t).into_iter().flat_map(|(w, b)| [w, b]).collect();
        met.sort();
        assert_eq!(met, ["Dan", "Eva"]);
        let [a, b, c, d, e] = ["Anna", "Bo", "Carl", "Dan", "Eva"].map(|n| by_name(&t, n));
        assert_eq!(t.check(&[(a, e)], None).unwrap(), [Warning::NotBracket]);
        t.set_result("Mette", 0, 0, Some(Outcome::Draw), "").unwrap();
        assert_eq!(t.can_pair(), Err(Error::Invalid("tournaments.error.playoff")));
        assert_eq!(t.set_playoff("Mette", 0, 0, Some(a), ""), Err(Error::Invalid("tournaments.error.playoff_player")));
        t.set_playoff("Mette", 0, 0, Some(e), "").unwrap();
        // The four still in share first place; Dan, out in Round 1, comes after them.
        let ranks: Vec<(usize, u32)> = tiebreak::standings(&t).iter().map(|s| (s.player, s.rank)).collect();
        assert_eq!(ranks.iter().filter(|x| x.1 == 1).count(), 4, "{ranks:?}");
        assert_eq!(ranks.last(), Some(&(d, 5)));
        t.pair_next().unwrap();
        // Round 2: 1 meets the winner of 4-5, 2 meets 3.
        let mut met: Vec<(usize, usize)> = t.played[1].games.iter().map(|g| (g.white.min(g.black), g.white.max(g.black))).collect();
        met.sort_unstable();
        assert_eq!(met, [(a, e), (b, c)]);
        assert_eq!(t.set_playoff("Mette", 0, 0, Some(d), "wrong sheet"), Err(Error::Invalid("tournaments.error.knockout_settled")));
        assert_eq!(t.set_result("Mette", 0, 0, Some(Outcome::White), "x").is_ok(), t.played[0].games[0].white == e, "only a result keeping Eva through stands");
        assert_eq!(Tournament::new("K", System::Knockout, 0, "").unwrap().tiebreaks, []);
    }

    #[test]
    fn a_pasted_list_gives_a_guest_a_line_with_the_rating_at_its_end() {
        let got: Vec<(String, u32)> = Player::list("Bo Kjær 2105\n\n  Carlsen, Magnus\t2830\nAnna Lind;1980\nEva Ravn\nJens Moe, 1760 \n2020").into_iter().map(|p| (p.name, p.rating)).collect();
        let want: Vec<(String, u32)> = [("Bo Kjær", 2105), ("Carlsen, Magnus", 2830), ("Anna Lind", 1980), ("Eva Ravn", 0), ("Jens Moe", 1760), ("2020", 0)].map(|(n, r)| (n.to_string(), r)).into();
        assert_eq!(got, want);
    }

    #[test]
    fn a_knockout_lists_the_players_through_by_bye_in_their_round() {
        let mut t = field(System::Knockout, 0, &[("Anna", 2400, 0), ("Bo", 2300, 0), ("Carl", 2200, 0), ("Dan", 2100, 0), ("Eva", 2000, 0)]);
        assert_eq!(t.through(0), Vec::<usize>::new(), "nothing paired yet");
        t.pair_next().unwrap();
        let [a, b, c, e] = ["Anna", "Bo", "Carl", "Eva"].map(|n| by_name(&t, n));
        assert_eq!(t.through(0), [a, b, c], "seeds 1-3 go straight to Round 2");
        let g = t.played[0].games[0];
        t.set_result("Mette", 0, 0, Some(if g.white == e { Outcome::White } else { Outcome::Black }), "").unwrap();
        t.pair_next().unwrap();
        assert_eq!(t.through(1), Vec::<usize>::new(), "four left, all on a board");
        let mut sw = field(System::Dutch { baku: false }, 3, &FOUR);
        sw.pair_next().unwrap();
        assert_eq!(sw.through(0), Vec::<usize>::new(), "only a knockout has players through");
    }

    #[test]
    fn a_scheveningen_scores_each_team_in_board_and_match_points() {
        let mut t = field(System::Scheveningen { double: false }, 0, &FOUR);
        assert_eq!(t.team_scores(), Vec::<TeamScore>::new());
        t.pair_next().unwrap();
        // Board 1 a win for White, board 2 a draw: White's team on board 1 wins 1½-½.
        t.set_result("Mette", 0, 0, Some(Outcome::White), "").unwrap();
        let first = t.players[t.played[0].games[0].white].team as usize - 1;
        assert_eq!(t.team_scores()[0].match_points, [0, 0], "no match points before every result is in");
        t.set_result("Mette", 0, 1, Some(Outcome::Draw), "").unwrap();
        let mut boards = [1, 1];
        boards[first] = 3;
        let mut matches = [0, 0];
        matches[first] = 2;
        assert_eq!(t.team_scores()[0], TeamScore { board_points: boards, match_points: matches });
        t.pair_next().unwrap();
        t.set_result("Mette", 1, 0, Some(Outcome::Draw), "").unwrap();
        t.set_result("Mette", 1, 1, Some(Outcome::Draw), "").unwrap();
        let s = t.team_scores();
        assert_eq!(s[1], TeamScore { board_points: [2, 2], match_points: [1, 1] });
        assert_eq!(TeamScore::total(&s), TeamScore { board_points: [boards[0] + 2, boards[1] + 2], match_points: [matches[0] + 1, matches[1] + 1] });
        let mut sw = field(System::Dutch { baku: false }, 3, &FOUR);
        sw.pair_next().unwrap();
        assert_eq!(sw.team_scores(), Vec::<TeamScore>::new(), "only a Scheveningen has teams");
    }

    #[test]
    fn a_scheveningen_needs_two_equal_teams_and_meets_each_with_the_other() {
        let mut t = field(System::Scheveningen { double: false }, 0, &[("Anna", 2100, 1), ("Bo", 1900, 2), ("Carl", 2000, 1)]);
        assert_eq!(t.pair_next(), Err(Error::Invalid("tournaments.error.teams")));
        assert_eq!(t.can_pair(), Err(Error::Invalid("tournaments.error.teams")), "said beside the Pair button");
        assert!(t.seeds.is_empty());
        let mut t = field(System::Scheveningen { double: true }, 0, &FOUR);
        t.pair_next().unwrap();
        assert_eq!(t.rounds, 4);
        let team = |p: usize| t.players[p].team;
        assert!(t.played[0].games.iter().all(|g| team(g.white) != team(g.black)));
        let [a, b, c, d] = ["Anna", "Bo", "Carl", "Dan"].map(|n| by_name(&t, n));
        assert_eq!(t.check(&[(a, c), (b, d)], None).unwrap(), [Warning::NotSchedule]);
        assert_eq!(t.set_team(a, 2), Err(Error::Invalid("tournaments.error.started")));
        assert_eq!(t.register(Player { name: "Eva".into(), ..Player::default() }), Err(Error::Invalid("tournaments.error.round_robin_started")));
    }

    #[test]
    fn a_drop_in_counts_an_absence_half_for_pairing_only_and_ranks_by_its_standings() {
        let mut t = field(System::DropIn { standings: DropIn::Percentage }, 30, &[("Anna", 0, 0), ("Bo", 0, 0), ("Carl", 0, 0), ("Dan", 0, 0)]);
        let [a, b, c, d] = ["Anna", "Bo", "Carl", "Dan"].map(|n| by_name(&t, n));
        assert_eq!(t.set_status("Mette", a, Status::HalfBye), Err(Error::Invalid("tournaments.error.drop_in_half_bye")));
        // Evening 1: Anna and Bo come; Anna wins.
        t.set_status("Mette", c, Status::Away).unwrap();
        t.set_status("Mette", d, Status::Away).unwrap();
        t.pair_next().unwrap();
        let g = t.played[0].games[0];
        t.set_result("Mette", 0, 0, Some(if g.white == a { Outcome::White } else { Outcome::Black }), "").unwrap();
        // Evening 2: everyone. Carl and Dan have half a point each for the pairing, so the
        // order is Anna 1, Carl ½, Dan ½, Bo 0: Anna meets Carl.
        for p in [c, d] {
            t.set_status("Mette", p, Status::Here).unwrap();
        }
        t.pair_next().unwrap();
        assert!(t.played[1].games.iter().any(|g| (g.white, g.black) == (a, c) || (g.white, g.black) == (c, a)), "{:?}", pairs(&t));
        assert_eq!(t.points(c), 0, "the half point is for the pairing only");
        // Carl beats Anna, Bo beats Dan: Carl 1/1, Anna 1/2, Bo 1/2, Dan 0/1.
        for (k, g) in t.played[1].games.clone().iter().enumerate() {
            let winner = if g.white == c || g.white == b { Outcome::White } else { Outcome::Black };
            t.set_result("Mette", 1, k, Some(winner), "").unwrap();
        }
        let s = t.standings();
        assert_eq!((s[0].player, s[0].points, s[0].rounds), (c, 2, 1));
        assert_eq!((s[3].player, s[3].rank), (d, 4));
        assert_eq!(s[1].rank, s[2].rank, "Anna and Bo level on 50 %");
        t.system = System::DropIn { standings: DropIn::Total };
        let s = t.standings();
        assert_eq!(s[0].rank, s[2].rank, "on total points three have 1");
        t.system = System::DropIn { standings: DropIn::Best(1) };
        assert_eq!(t.standings().iter().filter(|r| r.points == 2).count(), 3);
        let back: Tournament = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back.system, t.system);
        assert_eq!(serde_json::from_str::<System>(r#"{"kind":"dutch"}"#).unwrap(), System::Dutch { baku: false });
        assert_eq!(Tournament::new("D", System::DropIn { standings: DropIn::Best(0) }, 9, "").err(), Some(Error::Invalid("tournaments.error.best")));
    }
}
