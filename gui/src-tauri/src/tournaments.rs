//! A Club's Tournaments on the Clubs page: set one up, register and check in players, pair,
//! change a pairing, enter results, order the tiebreaks, show the pairings on a second screen,
//! and export. Everything is decided in `bragd_clubs::tournament`; this passes it through.

use std::path::PathBuf;

use bragd_clubs::club::{self, Club};
use bragd_clubs::export;
use bragd_clubs::tiebreak::Tiebreak;
use bragd_clubs::tournament::{self, Handicap, Outcome, Player, Status, TeamScore, Tournament, Warning};
use bragd_core::t;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::clubs::Held;
use crate::{Ctx, files};

type Res<T> = Result<T, String>;

/// A tiebreak as the page lists it.
#[derive(Serialize)]
pub struct Named {
    code: &'static str,
    name: String,
}

fn named(tb: Tiebreak) -> Named {
    Named { code: tb.code(), name: t!(&tb.key()) }
}

/// A row of the standings, its numbers written out.
#[derive(Serialize)]
pub struct Row {
    rank: u32,
    player: usize,
    points: String,
    tiebreaks: Vec<String>,
}

/// An open Tournament as its part of the page shows it.
#[derive(Serialize)]
pub struct View {
    id: i64,
    tournament: Tournament,
    /// The pairing system's name.
    system: String,
    /// Each player's points so far, written out.
    points: Vec<String>,
    standings: Vec<Row>,
    tiebreaks: Vec<Named>,
    /// Every tiebreak there is, for adding one.
    all: Vec<Named>,
    /// Why the next Round cannot be paired, if it cannot.
    blocked: Option<String>,
    /// The Variant played, by name.
    variant: String,
    /// A knockout's players through to the next Round by bye, for each Round.
    through: Vec<Vec<usize>>,
    /// A Scheveningen's team score: each Round's, then the total. Empty for other systems.
    teams: Vec<TeamRow>,
}

/// A Scheveningen Round's team score, team 1 then team 2, written out.
#[derive(Serialize)]
pub struct TeamRow {
    boards: [String; 2],
    matches: [u32; 2],
}

fn team_row(s: TeamScore) -> TeamRow {
    TeamRow { boards: s.board_points.map(tournament::half_text), matches: s.match_points }
}

/// A Variant a Tournament can be played in: those of the Families and Variants turned on.
#[derive(Serialize)]
pub struct VariantChoice {
    key: &'static str,
    name: String,
    family: &'static str,
}

#[tauri::command]
pub fn tournament_variants(ctx: State<'_, Ctx>) -> Vec<VariantChoice> {
    let cfg = ctx.cfg();
    bragd_games::VARIANTS.iter().filter(|v| cfg.variant_on(v.key)).map(|v| VariantChoice { key: v.key, name: t!(&format!("variant.{}", v.key)), family: v.family.key() }).collect()
}

/// A tiebreak written as points are: quarters and halves as fractions (`1½`, `¼`), anything
/// else, such as an average rating, to two decimals at most.
fn value(v: f64) -> String {
    let q = v * 4.0;
    if q.fract() == 0.0 && v >= 0.0 {
        let (n, f) = ((q as u64) / 4, ["", "¼", "½", "¾"][(q as u64 % 4) as usize]);
        return if n == 0 && !f.is_empty() { f.into() } else { format!("{n}{f}") };
    }
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_tiebreak_reads_like_the_points_beside_it() {
        let v: Vec<String> = [0.0, 0.25, 0.5, 1.75, 12.0, 1234.567, -0.5].map(super::value).into();
        assert_eq!(v, ["0", "¼", "½", "1¾", "12", "1234.57", "-0.5"]);
    }
}

fn view(id: i64, t: Tournament) -> View {
    let standings = t
        .standings()
        .into_iter()
        .map(|s| Row { rank: s.rank, player: s.player, points: s.points_text(t.system), tiebreaks: s.tiebreaks.iter().map(|&v| value(v)).collect() })
        .collect();
    View {
        id,
        system: t!(t.system.key()),
        points: (0..t.players.len()).map(|p| tournament::half_text(t.points(p))).collect(),
        standings,
        tiebreaks: t.tiebreaks.iter().map(|&tb| named(tb)).collect(),
        all: Tiebreak::ALL.iter().map(|&tb| named(tb)).collect(),
        blocked: t.can_pair().err().map(|e| e.to_string()),
        variant: t!(&format!("variant.{}", t.variant)),
        through: (0..t.played.len()).map(|r| t.through(r)).collect(),
        teams: {
            let rounds = t.team_scores();
            let total = (!rounds.is_empty()).then(|| TeamScore::total(&rounds));
            rounds.into_iter().chain(total).map(team_row).collect()
        },
        tournament: t,
    }
}

/// Runs `f` on the open Club.
fn on<R>(held: &Held, f: impl FnOnce(&mut Club) -> club::Result<R>) -> Res<R> {
    let mut g = crate::lock(&held.0);
    let c = g.as_mut().ok_or_else(|| t!("clubs.none_open"))?;
    f(c).map_err(|e| e.to_string())
}

/// Changes Tournament `id` as `by`, then gives its view.
fn change(held: &Held, by: i64, id: i64, f: impl FnOnce(&mut Tournament, &str) -> club::Result<()>) -> Res<View> {
    on(held, |c| {
        c.change_tournament(by, id, f)?;
        Ok(view(id, c.tournament(id)?))
    })
}

/// A new Tournament; the Club's view, with it listed.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments, reason = "one argument per field of the New Tournament form")]
pub fn tournament_new(by: i64, name: String, system: tournament::System, rounds: u32, date: String, place: String, variant: String, handicap: Option<Handicap>, match_to: Option<u32>, held: State<'_, Held>) -> Res<club::View> {
    on(&held, |c| {
        let mut t = Tournament::new(&name, system, rounds, &date)?;
        t.place = place.trim().into();
        t.set_game(&variant, handicap, match_to)?;
        c.add_tournament(by, &t)?;
        bragd_core::tips::used("tournaments");
        c.view()
    })
}

#[tauri::command(async)]
pub fn tournament_view(id: i64, held: State<'_, Held>) -> Res<View> {
    on(&held, |c| Ok(view(id, c.tournament(id)?)))
}

#[tauri::command(async)]
pub fn tournament_remove(by: i64, id: i64, held: State<'_, Held>) -> Res<club::View> {
    on(&held, |c| {
        c.remove_tournament(by, id)?;
        c.view()
    })
}

/// Registers a player: a member of the Club, or a guest found in a rating list or typed in.
#[tauri::command(async)]
pub fn tournament_register(by: i64, id: i64, player: Player, here: bool, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |t, who| {
        let p = t.register(player)?;
        if here { t.set_status(who, p, Status::Here) } else { Ok(()) }
    })
}

/// The tiebreaks a new Tournament of `system` starts with, by name.
#[tauri::command]
pub fn tournament_defaults(system: tournament::System) -> Vec<Named> {
    bragd_clubs::tiebreak::defaults(system).into_iter().map(named).collect()
}

/// Registers the guests of a pasted list, a line each (`Player::list`): all of them or none.
#[tauri::command(async)]
pub fn tournament_register_list(by: i64, id: i64, text: String, here: bool, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |t, who| {
        for player in Player::list(&text) {
            let p = t.register(player)?;
            if here {
                t.set_status(who, p, Status::Here)?;
            }
        }
        Ok(())
    })
}

/// Checks a player in or out, gives a half-point bye, or withdraws them.
#[tauri::command(async)]
pub fn tournament_status(by: i64, id: i64, player: usize, status: Status, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |t, who| t.set_status(who, player, status))
}

/// Checks in everyone who is away.
#[tauri::command(async)]
pub fn tournament_check_in_all(by: i64, id: i64, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |t, who| {
        for p in 0..t.players.len() {
            if t.players[p].status == Status::Away {
                t.set_status(who, p, Status::Here)?;
            }
        }
        Ok(())
    })
}

#[tauri::command(async)]
pub fn tournament_pair(by: i64, id: i64, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |t, _| t.pair_next())
}

#[tauri::command(async)]
pub fn tournament_pair_again(by: i64, id: i64, why: String, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |t, who| t.pair_again(who, &why))
}

/// What the rules say about a pairing of the last Round made by hand.
#[tauri::command(async)]
pub fn tournament_check(id: i64, pairs: Vec<(usize, usize)>, bye: Option<usize>, held: State<'_, Held>) -> Res<Vec<String>> {
    on(&held, |c| Ok(c.tournament(id)?.check(&pairs, bye)?.iter().map(Warning::text).collect()))
}

#[tauri::command(async)]
pub fn tournament_set_pairing(by: i64, id: i64, pairs: Vec<(usize, usize)>, bye: Option<usize>, why: String, held: State<'_, Held>) -> Res<View> {
    on(&held, |c| {
        c.set_pairing(by, id, &pairs, bye, &why)?;
        Ok(view(id, c.tournament(id)?))
    })
}

#[tauri::command(async)]
pub fn tournament_result(by: i64, id: i64, round: usize, board: usize, result: Option<Outcome>, why: String, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |t, who| t.set_result(who, round, board, result, &why))
}

/// Enters who won the play-off of a drawn knockout match.
#[tauri::command(async)]
pub fn tournament_playoff(by: i64, id: i64, round: usize, board: usize, winner: Option<usize>, why: String, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |t, who| t.set_playoff(who, round, board, winner, &why))
}

/// Puts a player in a Scheveningen's team 1 or 2, or none.
#[tauri::command(async)]
pub fn tournament_team(by: i64, id: i64, player: usize, team: u8, held: State<'_, Held>) -> Res<View> {
    change(&held, by, id, |t, _| t.set_team(player, team))
}

/// Sets the tiebreaks, in order, by their codes.
#[tauri::command(async)]
pub fn tournament_tiebreaks(by: i64, id: i64, codes: Vec<String>, held: State<'_, Held>) -> Res<View> {
    let list: Vec<Tiebreak> = codes.iter().filter_map(|c| Tiebreak::ALL.into_iter().find(|tb| tb.code() == c)).collect();
    change(&held, by, id, |t, _| {
        t.tiebreaks = list;
        Ok(())
    })
}

/// Rates the Club's members from the games they played each other in the Tournament, once.
#[tauri::command(async)]
pub fn tournament_rate(by: i64, id: i64, held: State<'_, Held>) -> Res<club::View> {
    on(&held, |c| {
        c.rate_tournament(by, id)?;
        c.view()
    })
}

/// The file name a Tournament's exports are offered under.
fn file_name(t: &Tournament, ext: &str) -> String {
    let stem: String = t.name.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ') { c } else { '_' }).collect();
    format!("{}.{ext}", stem.trim())
}

/// Saves an export: `trf`, `pgn`, `html` (standings and crosstable, to publish) or the
/// `bundle`. The path written, or `None` when no file was chosen.
#[tauri::command(async)]
pub fn tournament_export(id: i64, kind: String, app: AppHandle, held: State<'_, Held>, ctx: State<'_, Ctx>) -> Res<Option<PathBuf>> {
    let t = on(&held, |c| c.tournament(id))?;
    let (ext, label) = match kind.as_str() {
        "trf" => ("trf", t!("tournaments.export.trf")),
        "pgn" => ("pgn", t!("tournaments.export.pgn")),
        "html" => ("html", t!("tournaments.export.html")),
        "bundle" => (export::EXT, t!("tournaments.export.bundle")),
        _ => return Err(format!("no export {kind}")),
    };
    let ask = files::Ask { title: label.clone(), save: Some(file_name(&t, ext)), filters: vec![(label, vec![ext.into()])], ..Default::default() };
    let Some(path) = files::pick_one(&app, &ctx.cfg(), &ask)? else { return Ok(None) };
    let text = match kind.as_str() {
        "trf" => export::trf(&t),
        "pgn" => export::pgn(&t),
        "html" => export::standings_html(&t),
        _ => {
            export::bundle(&t, &path).map_err(|e| e.to_string())?;
            return Ok(Some(path));
        }
    };
    bragd_core::fs::replace(&path, text.as_bytes(), false).map_err(|e| e.to_string())?;
    Ok(Some(path))
}

/// Adds the Tournament in a bundle to the open Club.
#[tauri::command(async)]
pub fn tournament_import(by: i64, app: AppHandle, held: State<'_, Held>, ctx: State<'_, Ctx>) -> Res<Option<club::View>> {
    let ask = files::Ask { title: t!("tournaments.import"), filters: vec![(t!("tournaments.export.bundle"), vec![export::EXT.into()])], ..Default::default() };
    let Some(path) = files::pick_one(&app, &ctx.cfg(), &ask)? else { return Ok(None) };
    let t = export::read_bundle(&path).map_err(|e| e.to_string())?;
    on(&held, |c| {
        c.add_tournament(by, &t)?;
        c.view()
    })
    .map(Some)
}

/// The label of the second-screen window.
const SHOW: &str = "pairings";

/// Shows Round `round` (from 0) in large type in a window of its own, to drag to a second
/// screen or print. A window already open shows the new pairings. It is a static page with no
/// access to Bragd.
#[tauri::command(async)]
pub fn tournament_show(id: i64, round: usize, app: AppHandle, held: State<'_, Held>) -> Res<()> {
    let t = on(&held, |c| c.tournament(id))?;
    let dir = bragd_core::dirs::app_cache().ok_or("no cache folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("pairings.html");
    bragd_core::fs::replace(&path, export::pairings_html(&t, round).as_bytes(), false).map_err(|e| e.to_string())?;
    let url = tauri::Url::from_file_path(&path).map_err(|_| "bad path".to_string())?;
    if let Some(w) = app.get_webview_window(SHOW) {
        w.navigate(url).map_err(|e| e.to_string())?;
        let _ = w.set_focus();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(&app, SHOW, tauri::WebviewUrl::External(url))
        .title(format!("{} – {}", t.name, t!("tournaments.round_n", "n" => round + 1)))
        .inner_size(1000.0, 700.0)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}
