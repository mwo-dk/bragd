//! A Club's team Leagues on the Clubs page: set one up, add its Teams, pair each Round, line the
//! Teams up, enter board results and show the standings. Everything is decided in
//! `bragd_clubs::league`; this passes it through.

use bragd_clubs::club::{self, Club};
use bragd_clubs::league::{League, Team};
use bragd_clubs::tournament::{self, Outcome, Player};
use bragd_core::t;
use serde::Serialize;
use tauri::State;

use crate::clubs::Held;

type Res<T> = Result<T, String>;

/// A row of the standings, its points written out.
#[derive(Serialize)]
pub struct Row {
    rank: u32,
    team: usize,
    played: u32,
    won: u32,
    drawn: u32,
    lost: u32,
    match_points: String,
    board_points: String,
}

/// An open League as its part of the page shows it.
#[derive(Serialize)]
pub struct View {
    id: i64,
    league: League,
    rounds: u32,
    /// Each match's board points, home and away, written out: by Round, then match.
    scores: Vec<Vec<(String, String)>>,
    standings: Vec<Row>,
    /// Why the next Round cannot be paired, if it cannot.
    blocked: Option<String>,
}

fn view(id: i64, l: League) -> View {
    let scores = l.played.iter().map(|r| r.matches.iter().map(League::score).map(|(h, a)| (tournament::half_text(h), tournament::half_text(a))).collect()).collect();
    let standings = l
        .standings()
        .into_iter()
        .map(|s| Row { rank: s.rank, team: s.team, played: s.played, won: s.won, drawn: s.drawn, lost: s.lost, match_points: tournament::half_text(s.match_points), board_points: tournament::half_text(s.board_points) })
        .collect();
    let blocked = l.clone().pair_next().err().map(|e| e.to_string());
    View { id, rounds: l.rounds(), scores, standings, blocked, league: l }
}

/// Runs `f` on the open Club.
fn on<R>(held: &Held, f: impl FnOnce(&mut Club) -> club::Result<R>) -> Res<R> {
    let mut g = crate::lock(&held.0);
    let c = g.as_mut().ok_or_else(|| t!("clubs.none_open"))?;
    f(c).map_err(|e| e.to_string())
}

/// Changes League `id` as `by`, then gives its view.
fn change(held: &Held, by: i64, id: i64, f: impl FnOnce(&mut League, &str) -> club::Result<()>) -> Res<View> {
    on(held, |c| {
        c.change_league(by, id, f)?;
        Ok(view(id, c.league(id)?))
    })
}

/// A new League; the Club's view, with it listed.
#[tauri::command(async)]
pub fn league_new(by: i64, name: String, boards: u32, double: bool, date: String, held: State<'_, Held>) -> Res<club::View> {
    on(&held, |c| {
        c.add_league(by, &League::new(&name, boards, double, &date)?)?;
        bragd_core::tips::used("leagues");
        c.view()
    })
}

#[tauri::command(async)]
pub fn league_view(id: i64, held: State<'_, Held>) -> Res<View> {
    on(&held, |c| Ok(view(id, c.league(id)?)))
}

#[tauri::command(async)]
pub fn league_remove(by: i64, id: i64, held: State<'_, Held>) -> Res<club::View> {
    on(&held, |c| {
        c.remove_league(by, id)?;
        c.view()
    })
}

/// Adds a Team: its name and its players in board order, members of the Club or guests.
#[tauri::command(async)]
pub fn league_add_team(by: i64, id: i64, name: String, players: Vec<Player>, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |l, _| l.add_team(Team { name, players }).map(|_| ()))
}

#[tauri::command(async)]
pub fn league_pair(by: i64, id: i64, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |l, _| l.pair_next())
}

/// A Team's line-up for a match: one player a board, by place in the Team, or none.
#[tauri::command(async)]
pub fn league_line_up(by: i64, id: i64, round: usize, m: usize, away: bool, players: Vec<Option<usize>>, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |l, _| l.set_line_up(round, m, away, &players))
}

#[tauri::command(async)]
#[allow(clippy::too_many_arguments, reason = "the board, its result and the reason for a correction")]
pub fn league_result(by: i64, id: i64, round: usize, m: usize, board: usize, result: Option<Outcome>, why: String, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |l, who| l.set_result(who, round, m, board, result, &why))
}
