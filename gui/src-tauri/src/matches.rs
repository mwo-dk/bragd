//! The Engine matches item of each Family with engines: what a match can be set up with (the
//! Variants on, their engines there to run, the Libraries), starting one in the Helper, and
//! each match's standings, Elo and SPRT as it goes. The match itself runs in the Helper
//! (`bragd_helper::matches`).

use std::path::{Path, PathBuf};

use bragd_core::config::Config;
use bragd_core::{t, tips};
use bragd_engines::matches::{self, Kind, Setup};
use bragd_engines::{Limit, known};
use bragd_games::Family;
use bragd_helper::helper::Request;
use bragd_helper::jobs::{Job, Status, Work};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::knowledge::Knowledge;
use crate::{Ctx, databases, engines, files, helper_link};

type Res<T> = Result<T, String>;

/// An engine as the form offers it.
#[derive(Serialize)]
pub struct Choice {
    key: String,
    name: String,
}

/// A Variant the user has on, with its engines there to run.
#[derive(Serialize)]
pub struct VariantChoice {
    key: String,
    name: String,
    engines: Vec<Choice>,
}

/// One engine's line in a match's standings.
#[derive(Serialize)]
pub struct Row {
    name: String,
    games: u32,
    /// Wins, draws and losses, as shown: `3–2–1`.
    score: String,
    points: String,
    /// `+12 ± 30`; empty before its first Game.
    elo: String,
}

/// A match as the page shows it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchView {
    id: u64,
    title: String,
    kind: String,
    status_text: String,
    count: String,
    progress: Option<f64>,
    live: bool,
    error: String,
    rows: Vec<Row>,
    /// The SPRT's ratio, bounds and verdict; empty for other kinds.
    sprt: String,
    library: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    variants: Vec<VariantChoice>,
    /// The Libraries there are, by path and name; the Games go into one of them.
    libraries: Vec<(String, String)>,
    /// The Library offered first: `matches.bragd` in the Library folder.
    library: String,
    /// The Family's endgames have tablebases (chess).
    tablebases: bool,
    /// The processors here: no more Games at once than this.
    cpus: usize,
    matches: Vec<MatchView>,
    /// Why the Helper could not be asked; empty when it was.
    error: String,
}

fn name_of(key: &str) -> String {
    known::known(key).map_or_else(|| key.to_string(), |k| k.name.to_string())
}

fn variant_name(key: &str) -> String {
    t!(&format!("variant.{key}"))
}

fn shown(v: f64, places: usize) -> String {
    bragd_core::i18n::decimal(v, places)
}

fn signed(v: f64) -> String {
    if v >= 0.5 { format!("+{}", shown(v, 0)) } else { shown(v, 0) }
}

/// The page's view of the match job `j`.
pub fn view(j: &Job, setup: &Setup) -> MatchView {
    let games = matches::schedule(&setup.kind, setup.engines.len(), setup.rounds);
    let results = j.report.as_ref().and_then(|r| r.results.clone()).unwrap_or_else(|| matches::unplayed(games.len()));
    let names: Vec<String> = setup.engines.iter().map(|k| name_of(k)).collect();
    let rows = matches::standings(&games, &results, names.len())
        .into_iter()
        .map(|s| Row {
            name: names[s.engine].clone(),
            games: s.wins + s.draws + s.losses,
            score: format!("{}–{}–{}", s.wins, s.draws, s.losses),
            points: shown(s.points, 1),
            elo: s.elo.map(|(e, m)| t!("matches.elo", "elo" => signed(e), "margin" => shown(m, 0))).unwrap_or_default(),
        })
        .collect();
    let sprt = matches::sprt_state(&setup.kind, &games, &results).map_or_else(String::new, |s| {
        let verdict = match s.accepted {
            Some(true) => t!("matches.sprt.h1"),
            Some(false) => t!("matches.sprt.h0"),
            None => t!("matches.sprt.going"),
        };
        let elo = s.elo.map(|(e, m)| t!("matches.elo", "elo" => signed(e), "margin" => shown(m, 0))).unwrap_or_default();
        t!("matches.sprt", "llr" => shown(s.llr, 2), "lower" => shown(s.lower, 2), "upper" => shown(s.upper, 2), "verdict" => verdict, "elo" => elo)
    });
    let base = helper_link::job_view(j);
    MatchView {
        id: j.id,
        title: t!("matches.title", "variant" => variant_name(&setup.variant), "engines" => names.join(" – ")),
        kind: kind_text(&setup.kind),
        status_text: base.status_text,
        count: base.count,
        progress: base.progress,
        live: base.live,
        error: base.error,
        rows,
        sprt,
        library: setup.library.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
    }
}

fn kind_text(k: &Kind) -> String {
    match k {
        Kind::RoundRobin => t!("matches.kind.round_robin"),
        Kind::Gauntlet => t!("matches.kind.gauntlet"),
        Kind::Sprt { elo0, elo1, alpha, beta } => t!("matches.kind.sprt_with", "elo0" => shown(*elo0, 1), "elo1" => shown(*elo1, 1), "alpha" => shown(*alpha, 2), "beta" => shown(*beta, 2)),
    }
}

/// The Library folder's `matches.bragd`.
fn default_library(cfg: &Config) -> Option<PathBuf> {
    Some(databases::library_folder(cfg)?.join("matches.bragd"))
}

pub fn page(cfg: &Config, family: Family, jobs: Result<Vec<Job>, String>) -> Page {
    let variants = family
        .variants()
        .filter(|v| cfg.variant_on(v.key))
        .map(|v| {
            let engines = engines::engines_for(cfg, v.key).into_iter().filter(|k| engines::availability(cfg, k).ready()).map(|key| Choice { name: name_of(&key), key }).collect();
            VariantChoice { key: v.key.to_string(), name: variant_name(v.key), engines }
        })
        .collect();
    let (jobs, error) = match jobs {
        Ok(j) => (j, String::new()),
        Err(e) => (vec![], e),
    };
    let ours = |s: &Setup| bragd_games::Variant::from_key(&s.variant).map(|v| v.family()) == Some(family);
    let matches = jobs
        .iter()
        .rev()
        .filter_map(|j| match &j.work {
            Work::Match { setup } if ours(setup) => Some(view(j, setup)),
            _ => None,
        })
        .collect();
    Page {
        variants,
        libraries: databases::libraries(cfg).into_iter().map(|l| (l.path, l.name)).collect(),
        library: default_library(cfg).map(|p| p.display().to_string()).unwrap_or_default(),
        tablebases: family == Family::Chess,
        cpus: std::thread::available_parallelism().map_or(1, |n| n.get()),
        matches,
        error,
    }
}

/// `setup` checked before it goes to the Helper, with a seed of its own.
pub fn checked(cfg: &Config, mut setup: Setup) -> Res<Setup> {
    if !cfg.variant_on(&setup.variant) {
        return Err(t!("engines.unknown", "name" => setup.variant));
    }
    setup.engines.dedup();
    let mut seen = std::collections::BTreeSet::new();
    if !setup.engines.iter().all(|e| seen.insert(e.clone())) || setup.engines.len() < 2 {
        return Err(t!("matches.need_two"));
    }
    if matches!(setup.kind, Kind::Sprt { .. }) && setup.engines.len() != 2 {
        return Err(t!("matches.sprt_two"));
    }
    if let Kind::Sprt { elo0, elo1, alpha, beta } = setup.kind
        && !(elo0 < elo1 && (0.0..0.5).contains(&alpha) && alpha > 0.0 && (0.0..0.5).contains(&beta) && beta > 0.0)
    {
        return Err(t!("matches.sprt_bounds"));
    }
    if let Limit::Time(ms) = setup.limit {
        setup.limit = Limit::Time(ms.clamp(10, 600_000));
    }
    setup.rounds = setup.rounds.clamp(1, 100_000);
    setup.parallel = setup.parallel.clamp(1, std::thread::available_parallelism().map_or(1, |n| n.get()) as u32);
    if setup.library.as_os_str().is_empty() {
        setup.library = default_library(cfg).ok_or_else(engines::no_folder)?;
    }
    setup.seed = bragd_core::throw::seed();
    // Checked here too, so a wrong book is said at once rather than in the job.
    let _ = bragd_helper::matches::openings(&setup)?;
    Ok(setup)
}

#[tauri::command(async)]
pub fn matches_page(family_key: String, ctx: State<'_, Ctx>, k: State<'_, Knowledge>) -> Res<Page> {
    let family = Family::from_key(&family_key).ok_or_else(|| t!("engines.unknown", "name" => family_key))?;
    Ok(page(&ctx.cfg(), family, k.link.jobs()))
}

#[tauri::command(async)]
pub fn match_start(setup: Setup, ctx: State<'_, Ctx>, k: State<'_, Knowledge>) -> Res<u64> {
    let setup = checked(&ctx.cfg(), setup)?;
    let id = k.link.start(&Request::Start { work: Work::Match { setup } })?;
    tips::used("matches");
    Ok(id)
}

/// Asks for an opening book or a file of opening lines; None when none was chosen.
#[tauri::command(async)]
pub fn match_book_pick(app: AppHandle, ctx: State<'_, Ctx>) -> Res<Option<String>> {
    let ask = files::Ask { title: t!("matches.book_pick"), filters: vec![(t!("matches.book_files"), ["bin", "txt", "epd", "pgn"].iter().map(|e| e.to_string()).collect())], ..Default::default() };
    Ok(files::pick(&app, &ctx.cfg(), &ask)?.into_iter().next().map(|p: PathBuf| p.display().to_string()))
}

/// Whether the job is a match that is still to end: the page asks again while one is.
pub fn live(j: &Job) -> bool {
    matches!(j.work, Work::Match { .. }) && matches!(j.status, Status::Queued | Status::Running)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bragd_helper::jobs::Report;

    fn setup(kind: Kind) -> Setup {
        Setup { variant: "chess".into(), engines: vec!["stockfish".into(), "fruit".into()], kind, rounds: 10, limit: Limit::Time(100), parallel: 1, book: None, book_depth: 8, adjudication: Default::default(), library: Path::new("/x/matches.bragd").into(), seed: 0 }
    }

    #[test]
    fn a_match_shows_its_standings_and_sprt() {
        let s = setup(Kind::Sprt { elo0: 0.0, elo1: 5.0, alpha: 0.05, beta: 0.05 });
        let mut results = matches::unplayed(20);
        results.replace_range(0..4, "1=10");
        let j = Job { id: 3, work: Work::Match { setup: s.clone() }, status: Status::Running, done: 4, total: Some(20), error: None, resumed: 0, report: Some(Report { results: Some(results), ..Default::default() }) };
        let v = view(&j, &s);
        assert_eq!(v.rows.len(), 2);
        // Stockfish: won with White, drew with Black, then won both.
        assert_eq!((v.rows[0].name.as_str(), v.rows[0].score.as_str(), v.rows[0].games), ("Stockfish", "3–1–0", 4));
        assert!(v.rows[0].elo.starts_with('+'), "{}", v.rows[0].elo);
        assert!(v.sprt.contains("2.94"), "{}", v.sprt);
        assert!(v.live && v.library == "matches.bragd");
        assert!(live(&j));
        // A round robin has no SPRT.
        let rr = setup(Kind::RoundRobin);
        assert_eq!(view(&Job { work: Work::Match { setup: rr.clone() }, report: None, ..j }, &rr).sprt, "");
    }

    #[test]
    fn a_setup_is_checked_before_it_goes() {
        let cfg = Config::default();
        assert_eq!(checked(&cfg, Setup { engines: vec!["fruit".into(), "fruit".into()], ..setup(Kind::RoundRobin) }).unwrap_err(), t!("matches.need_two"));
        let three = Setup { engines: vec!["a".into(), "b".into(), "c".into()], ..setup(Kind::Sprt { elo0: 0.0, elo1: 5.0, alpha: 0.05, beta: 0.05 }) };
        assert_eq!(checked(&cfg, three).unwrap_err(), t!("matches.sprt_two"));
        assert_eq!(checked(&cfg, setup(Kind::Sprt { elo0: 5.0, elo1: 0.0, alpha: 0.05, beta: 0.05 })).unwrap_err(), t!("matches.sprt_bounds"));
        let ok = checked(&cfg, Setup { parallel: 10_000, rounds: 0, limit: Limit::Time(1), ..setup(Kind::Gauntlet) }).unwrap();
        assert!(ok.parallel >= 1 && ok.parallel as usize <= std::thread::available_parallelism().unwrap().get());
        assert_eq!((ok.rounds, ok.limit), (1, Limit::Time(10)));
        let book = Setup { book: Some("/nowhere/book.txt".into()), ..setup(Kind::Gauntlet) };
        assert!(checked(&cfg, book).is_err());
    }
}
