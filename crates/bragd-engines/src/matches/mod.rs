//! Engine matches: a round robin, a gauntlet (one engine against each of the others), or two
//! engines in a match the SPRT stops (`crate::sprt`). Every pairing plays game pairs: two
//! Games from the same opening, each engine taking each Side once.
//!
//! A Game is played out in `bragd-games`, so every move an engine names is checked against the
//! rules, and it ends by the rules, by an engine that names no move, an illegal one or none in
//! time (it loses), or by adjudication: an engine sure it is lost resigns, a long quiet stretch
//! is a draw, an endless Game is a draw, and a tablebase ends a chess endgame it knows.
//!
//! What a match has played is kept as one character per Game of its schedule, so a match the
//! Helper was stopped in goes on where it was.

pub mod berger;

use std::path::PathBuf;
use std::sync::Mutex;

use bragd_games::{Awaiting, State};
use serde::{Deserialize, Serialize};

use crate::seam::{self, Notation};
use crate::session::{Session, think};
use crate::{Limit, sprt};

/// What kind of match.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    /// Every engine against every other.
    RoundRobin,
    /// The first engine against each of the others.
    Gauntlet,
    /// The first engine (the one tested) against the second, until the SPRT decides between an
    /// Elo difference of `elo0` (H0) and `elo1` (H1), with error chances `alpha` and `beta`.
    Sprt { elo0: f64, elo1: f64, alpha: f64, beta: f64 },
}

/// When a Game is ended before the rules end it. Scores are centipawns from the engine's own
/// view; a Family whose engines do not score in centipawns (go, backgammon) ends only by its
/// rules and by `max_moves`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Adjudication {
    /// An engine whose score is this far down for `resign_moves` of its moves in a row loses.
    pub resign_cp: Option<u32>,
    pub resign_moves: u32,
    /// From move `draw_after` on, `draw_moves` moves of each engine within this many centipawns
    /// of level are a draw.
    pub draw_cp: Option<u32>,
    pub draw_after: u32,
    pub draw_moves: u32,
    /// A Game this many full moves long (a move of each Side) after its opening is a draw.
    pub max_moves: Option<u32>,
    /// A chess endgame a tablebase knows ends with the tablebase's result.
    pub tablebases: bool,
}

impl Default for Adjudication {
    fn default() -> Self {
        Adjudication { resign_cp: Some(700), resign_moves: 4, draw_cp: Some(10), draw_after: 40, draw_moves: 8, max_moves: Some(200), tablebases: true }
    }
}

/// A match as it is asked for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Setup {
    /// The Variant, by its catalogue key.
    pub variant: String,
    /// The engines, by key (`local::resolve`); the first is the one a gauntlet or an SPRT tests.
    pub engines: Vec<String>,
    pub kind: Kind,
    /// Game pairs per pairing; for an SPRT, the most game pairs it plays.
    pub rounds: u32,
    /// How long each move may take.
    pub limit: Limit,
    /// Games played at once.
    pub parallel: u32,
    /// Openings: a Polyglot book (chess), or a file of opening lines, one per line, each a set-up
    /// position or moves from the start. None: every Game starts from the Variant's start.
    #[serde(default)]
    pub book: Option<PathBuf>,
    /// How many moves deep a Polyglot book is followed.
    #[serde(default = "book_depth")]
    pub book_depth: u32,
    #[serde(default)]
    pub adjudication: Adjudication,
    /// The Library the Games go into.
    pub library: PathBuf,
    /// Chooses the openings, so a match started again plays the same ones.
    #[serde(default)]
    pub seed: u64,
}

fn book_depth() -> u32 {
    8
}

/// One Game of a match's schedule: its opening (one per game pair), and the engines (by their
/// place in `Setup::engines`) playing the first Side and the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Game {
    pub opening: u32,
    pub first: usize,
    pub second: usize,
}

/// Every Game of a match of `n` engines, in the order they are played.
pub fn schedule(kind: &Kind, n: usize, rounds: u32) -> Vec<Game> {
    let pairings: Vec<(usize, usize)> = match kind {
        Kind::RoundRobin => berger::rounds(n).into_iter().flatten().collect(),
        Kind::Gauntlet => (1..n).map(|o| (0, o)).collect(),
        Kind::Sprt { .. } if n >= 2 => vec![(0, 1)],
        Kind::Sprt { .. } => vec![],
    };
    let mut out = vec![];
    for _ in 0..rounds {
        for &(a, b) in &pairings {
            let opening = (out.len() / 2) as u32;
            out.push(Game { opening, first: a, second: b });
            out.push(Game { opening, first: b, second: a });
        }
    }
    out
}

/// How a Game went for its first Side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Ended {
    First,
    Second,
    Draw,
}

impl Ended {
    /// As one character of a match's results: `1`, `0` or `=`.
    pub fn mark(self) -> char {
        match self {
            Ended::First => '1',
            Ended::Second => '0',
            Ended::Draw => '=',
        }
    }

    /// The first Side's points.
    fn points(self) -> f64 {
        match self {
            Ended::First => 1.0,
            Ended::Second => 0.0,
            Ended::Draw => 0.5,
        }
    }

    fn of(mark: char) -> Option<Ended> {
        match mark {
            '1' => Some(Ended::First),
            '0' => Some(Ended::Second),
            '=' => Some(Ended::Draw),
            _ => None,
        }
    }

    /// The result as a chess record writes it.
    pub fn result(self) -> &'static str {
        match self {
            Ended::First => "1-0",
            Ended::Second => "0-1",
            Ended::Draw => "1/2-1/2",
        }
    }
}

/// The results of a match not yet begun: one `.` per Game of its schedule.
pub fn unplayed(games: usize) -> String {
    ".".repeat(games)
}

/// Where a Game starts: a set-up position (None: the Variant's start) and moves played from it,
/// as `bragd-games` entries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Opening {
    pub initial: Option<String>,
    pub moves: Vec<String>,
}

/// A Game played out.
#[derive(Clone, Debug, PartialEq)]
pub struct Played {
    pub initial: Option<String>,
    /// Every entry, the opening's included.
    pub entries: Vec<String>,
    /// The entries as a Player writes them (SAN for chess).
    pub history: Vec<String>,
    pub ended: Ended,
    /// The result as the record writes it: the rules' own (`W+7.5`) or `Ended::result`.
    pub result: String,
    /// Why it ended: the rules' reason (`checkmate`), `resignation`, `adjudication`,
    /// `tablebase`, `length`, `illegal move`, `no move` or `engine failed`.
    pub reason: String,
}

/// An engine playing a Side, and how it writes moves.
pub struct Player {
    pub session: Box<dyn Session + Send>,
    pub notation: Notation,
}

/// Entries that are no move of the board: offers, concessions and the scoring talk of go.
fn talk(wire: &str) -> bool {
    matches!(wire, "double" | "take" | "drop" | "resign" | "resign_gammon" | "resign_backgammon" | "accept" | "resume") || wire.starts_with("dead:")
}

/// What a tablebase says about a chess position, given the engine's last score (centipawns from
/// the first Side's view): None when it does not know it.
pub type Table<'a> = &'a (dyn Fn(&State, Option<i32>) -> Option<Ended> + Sync);

/// Plays one Game of `variant` from `opening`, `players[0]` on the first Side. Err only when the
/// opening cannot be played or `stop` turned true.
pub fn play(variant: &str, opening: &Opening, players: &mut [Player; 2], limit: Limit, adj: &Adjudication, table: Table, stop: &dyn Fn() -> bool) -> Result<Played, String> {
    let family = bragd_games::variant(variant).map_err(|e| e.to_string())?.family();
    let initial = opening.initial.as_deref();
    let mut entries = opening.moves.clone();
    // Each engine move's score, centipawns from the first Side's view, with whose move it was.
    let mut scores: Vec<(usize, Option<i32>)> = vec![];
    let mut moves = 0u32;
    let end = |entries: Vec<String>, ended: Ended, reason: &str| -> Result<Played, String> {
        let state = bragd_games::replay(variant, initial, &entries).map_err(|e| e.to_string())?;
        Ok(Played { initial: opening.initial.clone(), history: state.history, entries, ended, result: ended.result().into(), reason: reason.into() })
    };
    loop {
        if stop() {
            return Err("stopped".into());
        }
        let state = bragd_games::replay(variant, initial, &entries).map_err(|e| e.to_string())?;
        if let Some(o) = &state.outcome {
            let ended = match o.winner {
                None => Ended::Draw,
                Some(s) if s.index == 0 => Ended::First,
                Some(_) => Ended::Second,
            };
            return Ok(Played { initial: opening.initial.clone(), history: state.history, entries, ended, result: o.result.clone(), reason: o.reason.clone() });
        }
        let Some(seat) = state.to_move else { return end(entries, Ended::Draw, "length") };
        let me = seat.index as usize;
        let lose = if me == 0 { Ended::Second } else { Ended::First };
        if state.awaiting == Awaiting::Throw {
            let t = bragd_core::throw::throw(family, &state.throws).ok_or("no throw")?;
            entries.push(t);
            continue;
        }
        if adj.max_moves.is_some_and(|m| moves >= m * 2) {
            return end(entries, Ended::Draw, "length");
        }
        // Go's count after two passes: no stones marked dead, and the other Side agrees.
        if let Some(m) = state.moves.iter().find(|m| m.wire == "accept").or_else(|| state.moves.iter().find(|m| m.wire.starts_with("dead:"))) {
            entries.push(m.wire.clone());
            continue;
        }
        let real: Vec<&String> = state.moves.iter().map(|m| &m.wire).filter(|w| !talk(w)).collect();
        moves += 1;
        if real.len() == 1 {
            entries.push(real[0].clone());
            continue;
        }
        let p = &mut players[me];
        let (asked, _) = seam::position(variant, initial, &entries, p.notation)?;
        let a = match think(p.session.as_mut(), &asked, limit, 1, stop, &mut |_| {}) {
            Ok(a) => a,
            Err(_) if stop() => return Err("stopped".into()),
            Err(_) => return end(entries, lose, "engine failed"),
        };
        let Some(best) = a.best else { return end(entries, lose, "no move") };
        if best.eq_ignore_ascii_case("resign") {
            return end(entries, lose, "resignation");
        }
        let Some(m) = seam::entry(&state, p.notation, &best).filter(|m| !talk(&m.wire)) else { return end(entries, lose, "illegal move") };
        entries.push(m.wire.clone());
        let score = a.lines.first().and_then(|l| l.score).and_then(|s| s.cp());
        scores.push((me, score));
        if adj.tablebases
            && let Some(after) = bragd_games::replay(variant, initial, &entries).ok().filter(|s| s.outcome.is_none())
            && let Some(ended) = table(&after, score)
        {
            return end(entries, ended, "tablebase");
        }
        // The mover's own view of its last moves.
        let own = |who: usize, n: usize| -> Option<Vec<i32>> {
            let v: Vec<i32> = scores.iter().rev().filter(|(w, _)| *w == who).take(n).map(|(w, s)| s.map(|c| if *w == 0 { c } else { -c })).collect::<Option<_>>()?;
            (n > 0 && v.len() == n).then_some(v)
        };
        if let Some(cp) = adj.resign_cp
            && own(me, adj.resign_moves as usize).is_some_and(|v| v.iter().all(|s| *s <= -(cp as i32)))
        {
            return end(entries, lose, "resignation");
        }
        if let Some(cp) = adj.draw_cp
            && moves / 2 >= adj.draw_after
            && [0, 1].iter().all(|&w| own(w, adj.draw_moves as usize).is_some_and(|v| v.iter().all(|s| s.unsigned_abs() <= cp)))
        {
            return end(entries, Ended::Draw, "adjudication");
        }
    }
}

/// One engine's tally in a match.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Standing {
    pub engine: usize,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub points: f64,
    /// Its Elo against the field and the 95 % margin, from its own Games.
    pub elo: Option<(f64, f64)>,
}

/// The tally of each of `n` engines, by points.
pub fn standings(games: &[Game], results: &str, n: usize) -> Vec<Standing> {
    let mut s: Vec<Standing> = (0..n).map(|engine| Standing { engine, ..Default::default() }).collect();
    for (g, mark) in games.iter().zip(results.chars()) {
        let Some(e) = Ended::of(mark) else { continue };
        for (who, pts) in [(g.first, e.points()), (g.second, 1.0 - e.points())] {
            let st = &mut s[who];
            st.points += pts;
            match pts {
                1.0 => st.wins += 1,
                0.0 => st.losses += 1,
                _ => st.draws += 1,
            }
        }
    }
    for st in &mut s {
        st.elo = sprt::elo(&[st.losses as u64, st.draws as u64, st.wins as u64]);
    }
    s.sort_by(|a, b| b.points.total_cmp(&a.points).then(a.engine.cmp(&b.engine)));
    s
}

/// The first engine's game pairs, both Games played, as the five frequencies of
/// `sprt::llr`: from both lost to both won.
pub fn pentanomial(games: &[Game], results: &str) -> [u64; 5] {
    let marks: Vec<char> = results.chars().collect();
    let mut out = [0u64; 5];
    for (pair, two) in games.chunks(2).zip(marks.chunks(2)) {
        let mut half_points = 0;
        for (g, m) in pair.iter().zip(two) {
            let Some(e) = Ended::of(*m) else { break };
            let mine = if g.first == 0 { e.points() } else { 1.0 - e.points() };
            half_points += (mine * 2.0) as usize;
        }
        if two.iter().all(|m| Ended::of(*m).is_some()) && pair.len() == 2 {
            out[half_points] += 1;
        }
    }
    out
}

/// Where an SPRT stands.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SprtState {
    pub llr: f64,
    pub lower: f64,
    pub upper: f64,
    /// Decided: true when H1 (`elo1`) is accepted, false when H0 is.
    pub accepted: Option<bool>,
    /// The first engine's Elo from its game pairs, with the 95 % margin.
    pub elo: Option<(f64, f64)>,
}

/// The SPRT of a match of that kind, as far as its results go.
pub fn sprt_state(kind: &Kind, games: &[Game], results: &str) -> Option<SprtState> {
    let Kind::Sprt { elo0, elo1, alpha, beta } = *kind else { return None };
    let pairs = pentanomial(games, results);
    let llr = if pairs.iter().sum::<u64>() == 0 { 0.0 } else { sprt::llr(&pairs, elo0, elo1) };
    let (lower, upper) = sprt::bounds(alpha, beta);
    let accepted = if llr >= upper { Some(true) } else if llr <= lower { Some(false) } else { None };
    Some(SprtState { llr, lower, upper, accepted, elo: sprt::elo(&pairs) })
}

/// What a running match needs from where it runs.
pub struct Run<'a> {
    pub setup: &'a Setup,
    pub games: &'a [Game],
    /// The opening of game pair `n`.
    pub opening: &'a (dyn Fn(u32) -> Opening + Sync),
    /// Starts engine `n` of `Setup::engines`.
    pub start: &'a (dyn Fn(usize) -> Result<Player, String> + Sync),
    pub table: Table<'a>,
    pub stop: &'a (dyn Fn() -> bool + Sync),
    /// Hears each Game played: its place in the schedule, the Game, and the results so far.
    pub done: &'a (dyn Fn(usize, &Played, &str) + Sync),
}

/// Plays the Games of `results` still to play (`.`), `Setup::parallel` at a time, until all are
/// played, the SPRT decides, `stop` turns true or an engine cannot be started (its error). The
/// results as they stand at the end.
pub fn run(r: &Run, results: String) -> Result<String, String> {
    struct Shared {
        results: Vec<char>,
        taken: Vec<bool>,
        error: Option<String>,
    }
    let shared = Mutex::new(Shared { taken: results.chars().map(|c| c != '.').collect(), results: results.chars().collect(), error: None });
    let lock = || shared.lock().unwrap_or_else(|e| e.into_inner());
    let decided = |s: &Shared| sprt_state(&r.setup.kind, r.games, &s.results.iter().collect::<String>()).is_some_and(|st| st.accepted.is_some());
    let worker = || {
        loop {
            let next = {
                let mut s = lock();
                if s.error.is_some() || (r.stop)() || decided(&s) {
                    return;
                }
                let Some(i) = s.taken.iter().position(|t| !t) else { return };
                s.taken[i] = true;
                i
            };
            let g = r.games[next];
            let started = (r.start)(g.first).and_then(|a| Ok([a, (r.start)(g.second)?]));
            let mut players = match started {
                Ok(p) => p,
                Err(e) => {
                    lock().error = Some(e);
                    return;
                }
            };
            let opening = (r.opening)(g.opening);
            let played = play(&r.setup.variant, &opening, &mut players, r.setup.limit, &r.setup.adjudication, r.table, r.stop);
            drop(players);
            let mut s = lock();
            match played {
                Ok(p) => {
                    s.results[next] = p.ended.mark();
                    (r.done)(next, &p, &s.results.iter().collect::<String>());
                }
                // Stopped, or an opening the rules refuse: left to play another time.
                Err(e) => {
                    if !(r.stop)() {
                        s.error = Some(e);
                    }
                    return;
                }
            }
        }
    };
    std::thread::scope(|sc| {
        for _ in 0..r.setup.parallel.max(1) {
            sc.spawn(worker);
        }
    });
    let s = shared.into_inner().unwrap_or_else(|e| e.into_inner());
    match s.error {
        Some(e) => Err(e),
        None => Ok(s.results.into_iter().collect()),
    }
}

#[cfg(test)]
mod tests;
